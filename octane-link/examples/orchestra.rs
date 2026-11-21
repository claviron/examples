//! An orchestra with no conductor: Rust and PHP workers claim from one
//! shared Orbit atomic, with nothing coordinating them, and the run shows
//! that the value never went wrong.
//!
//! The application must be running; this is a client.
//!
//! ```sh
//! cargo run --release --example orchestra -- --claims 1000000
//! ```
//!
//! The shared value is the application's declared `tiles` Orbital: the round
//! in its high 32 bits, the next index in its low 32. A claim is one atomic
//! add of 1, and the value before it names the round and the index taken.
//! The run starts a round (`DELETE /_lumenize/rust/tiles`), fetches the
//! value's id (`/_lumenize/rust/tiles/id`) and runs its lanes: each lane is
//! one Rust or PHP worker with one burst in flight at a time
//! (`/_lumenize/rust/burst?n=N`, `/_lumenize/php/burst?n=N&id=ID`). A burst
//! is N claims in a loop on the server, with no request per claim, and its
//! answer says what it took. Nothing orders the lanes.
//!
//! The evidence at the end is three totals; see [`Evidence`].

use std::fmt;
use std::sync::atomic::Ordering::{AcqRel, Relaxed};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// How many times a second the screen is drawn: a display setting of this
/// client. The lanes never wait for it; the drawing reads their counters.
const FRAMES_PER_SECOND: u32 = 30;

/// The most claims the server performs in one burst, on either side.
const BURST_MAX: u32 = 1_000_000;

/// The most claims one run may make: a round holds 2^32 indices, and the
/// closing claim that reads the value takes one of them.
const CLAIMS_MAX: u64 = u32::MAX as u64;

/// The web page's two flat colours, in its dark theme: Rust
/// `hsl(22 70% 60%)`, PHP `hsl(236 62% 64%)`.
const RUST: Color = Color::Rgb(224, 134, 82);
const PHP: Color = Color::Rgb(106, 114, 220);
const GOOD: Color = Color::Rgb(108, 198, 138);
const BAD: Color = Color::Rgb(240, 100, 90);
const MUTED: Color = Color::Rgb(155, 154, 147);
const UNLIT: Color = Color::Rgb(78, 78, 72);

#[derive(Parser)]
#[command(
    name = "orchestra",
    about = "An orchestra with no conductor: Rust and PHP workers claim from one \
             shared Orbit atomic in bursts, and the run proves the value never went wrong."
)]
struct Options {
    /// The application's base URL.
    #[arg(long, default_value = "http://127.0.0.1:8102")]
    url: String,
    /// Claims to make in the run, at most 4,294,967,295 (one round's indices,
    /// less the closing claim).
    #[arg(long, default_value_t = 50_000_000)]
    claims: u64,
    /// Claims per burst, at most 1,000,000. The last burst may be smaller.
    #[arg(long, default_value_t = 10_000)]
    burst: u32,
    /// Rust lanes: workers with one burst in flight each.
    #[arg(long, default_value_t = 8)]
    rust: usize,
    /// PHP lanes. Keep it at or below the application's PHP instances
    /// (`php.application.instances` in lumenize.yaml, 8): a lane beyond them
    /// waits in the admission queue, and is refused (503, sent again) when
    /// the queue is full.
    #[arg(long, default_value_t = 8)]
    php: usize,
    /// No screen: print progress lines and the verdict.
    #[arg(long)]
    plain: bool,
    /// Test of the checker: corrupt one burst's answer after it is received.
    /// `reported` makes it report one index twice, as the server's own check
    /// would; `silent` lowers its index sum as if one index had been taken
    /// twice and another never, with no fault reported.
    #[arg(long, value_enum, hide = true)]
    test_checker: Option<Corruption>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Corruption {
    Reported,
    Silent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Rust = 0,
    Php = 1,
}

impl Side {
    const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Php => "php",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Php => "PHP",
        }
    }

    const fn colour(self) -> Color {
        match self {
            Self::Rust => RUST,
            Self::Php => PHP,
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "rust" => Some(Self::Rust),
            "php" => Some(Self::Php),
            _ => None,
        }
    }
}

// What a lane is doing, for its activity mark.
const WAITING: u8 = 0;
const WORKING: u8 = 1;
const DONE: u8 = 2;
const STOPPED: u8 = 3;

/// One worker of one kind, with one burst in flight at a time.
struct Lane {
    side: Side,
    /// Its number within its side, from 1.
    number: usize,
    bursts: AtomicU64,
    claims: AtomicU64,
    state: AtomicU8,
}

// What a light shows.
const DARK: u8 = 0;
const LIT_RUST: u8 = 1;
const LIT_PHP: u8 = 2;
const FAULT: u8 = 3;

/// The fireflies: one light for a group of bursts handed out one after the
/// other, lit once all of them have verified, in the colour of the side that
/// performed the group's first burst. That is a sample, so the two colours
/// stand in the proportion of the bursts each side took. Groups are placed
/// in a scattered order, so the field twinkles in.
struct Field {
    bursts: u64,
    /// The number of each group's first burst.
    start: Vec<u64>,
    /// Bursts still to verify, per group.
    remaining: Vec<AtomicU64>,
    /// The colour of each group's first burst, once it has verified.
    lead: Vec<AtomicU8>,
    /// Where each group shows, as a position in the field.
    position: Vec<u32>,
    /// What each position shows.
    shown: Vec<AtomicU8>,
}

impl Field {
    fn new(
        bursts: u64,
        lights: usize,
    ) -> Self {
        let lights = lights.max(1).min(bursts as usize);
        let (b, l) = (u128::from(bursts), lights as u128);
        let start: Vec<u64> = (0..=lights as u128)
            .map(|group| (group * b).div_ceil(l) as u64)
            .collect();
        let remaining = start
            .windows(2)
            .map(|pair| AtomicU64::new(pair[1] - pair[0]))
            .collect();
        // A fixed shuffle (Fisher-Yates over xorshift64): scattered, and the
        // same on every run.
        let mut position: Vec<u32> = (0..lights as u32).collect();
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for i in (1..lights).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            position.swap(i, (state % (i as u64 + 1)) as usize);
        }
        Self {
            bursts,
            lead: (0..lights).map(|_| AtomicU8::new(DARK)).collect(),
            start,
            remaining,
            position,
            shown: (0..lights).map(|_| AtomicU8::new(DARK)).collect(),
        }
    }

    fn lights(&self) -> usize {
        self.shown.len()
    }

    fn group(
        &self,
        burst: u64,
    ) -> usize {
        (u128::from(burst) * self.lights() as u128 / u128::from(self.bursts))
            as usize
    }

    fn verified(
        &self,
        burst: u64,
        side: Side,
    ) {
        let group = self.group(burst);
        if burst == self.start[group] {
            let lit = if side == Side::Rust { LIT_RUST } else { LIT_PHP };
            self.lead[group].store(lit, Relaxed);
        }
        // The last burst's decrement sees what every earlier one stored.
        if self.remaining[group].fetch_sub(1, AcqRel) == 1 {
            let lit = self.lead[group].load(Relaxed);
            let _ = self.shown[self.position[group] as usize]
                .compare_exchange(DARK, lit, Relaxed, Relaxed);
        }
    }

    fn fault(
        &self,
        burst: u64,
    ) {
        self.shown[self.position[self.group(burst)] as usize]
            .store(FAULT, Relaxed);
    }

    /// The fewest and the most bursts one light stands for.
    fn per_light(&self) -> (u64, u64) {
        let lights = self.lights() as u64;
        (self.bursts / lights, self.bursts.div_ceil(lights))
    }
}

/// A run: what it is asked for, and the counters the lanes keep.
struct Run {
    target: u64,
    burst: u32,
    bursts: u64,
    round: i64,
    id: Box<str>,
    /// Claims handed out to lanes so far.
    handed: AtomicU64,
    /// Set by a rejection or by the user leaving: no more bursts are handed out.
    stop: AtomicBool,
    /// Claims in verified bursts, per side and together.
    answered: [AtomicU64; 2],
    total: AtomicU64,
    /// Bursts sent again because nothing ran: PHP's admission answered 503,
    /// or the listener refused the connection before anything was sent.
    refused: [AtomicU64; 2],
    /// The sum of every index the verified bursts took. At most 2^32 - 1
    /// claims below 2^32 each, so it stays below 2^64.
    index_sum: AtomicU64,
    /// Claims performed by bursts whose answer was rejected.
    rejected: AtomicU64,
    /// Rust lanes first, then PHP.
    lanes: Vec<Lane>,
    rust_lanes: usize,
    php_lanes: usize,
    /// Lanes that have not ended yet.
    running: AtomicUsize,
    field: Option<Field>,
    /// What rejected the run, first come.
    rejection: OnceLock<String>,
    started: Instant,
    ended: OnceLock<Duration>,
    plain: bool,
    /// The burst whose answer the checker test corrupts, and how.
    test: Option<(Corruption, u64)>,
}

impl Run {
    /// The next burst to perform: its number and its size, until the bursts
    /// handed out add up to the target.
    fn take(&self) -> Option<(u64, u32)> {
        let mut handed = self.handed.load(Relaxed);
        loop {
            if self.stop.load(Relaxed) || handed >= self.target {
                return None;
            }
            let size = u64::from(self.burst).min(self.target - handed);
            match self.handed.compare_exchange_weak(
                handed,
                handed + size,
                Relaxed,
                Relaxed,
            ) {
                // Every burst but the last is full, so its number is this.
                Ok(_) => {
                    return Some((handed / u64::from(self.burst), size as u32));
                }
                Err(actual) => handed = actual,
            }
        }
    }

    fn reject(
        &self,
        reason: String,
    ) {
        let _ = self.rejection.set(reason);
        self.stop.store(true, Relaxed);
    }

    fn elapsed(&self) -> Duration {
        self.ended.get().copied().unwrap_or_else(|| self.started.elapsed())
    }

    fn finished(&self) -> bool {
        self.ended.get().is_some()
    }

    fn rate(
        &self,
        claims: u64,
    ) -> u64 {
        let seconds = self.elapsed().as_secs_f64();
        if seconds > 0.0 { (claims as f64 / seconds) as u64 } else { 0 }
    }
}

/// The burst routes of both sides, built once; only a smaller last burst
/// formats its own.
struct Routes {
    base: Box<str>,
    id: Box<str>,
    rust_full: Box<str>,
    php_full: Box<str>,
}

impl Routes {
    fn new(
        base: &str,
        id: &str,
        burst: u32,
    ) -> Self {
        Self {
            base: base.into(),
            id: id.into(),
            rust_full: Self::rust(base, burst).into_boxed_str(),
            php_full: Self::php(base, id, burst).into_boxed_str(),
        }
    }

    fn rust(
        base: &str,
        n: u32,
    ) -> String {
        format!("{base}/_lumenize/rust/burst?n={n}")
    }

    fn php(
        base: &str,
        id: &str,
        n: u32,
    ) -> String {
        format!("{base}/_lumenize/php/burst?n={n}&id={id}")
    }
}

/// A burst's answer, as both sides send it.
#[derive(Deserialize)]
struct Answer<'a> {
    side: &'a str,
    round: i64,
    count: u64,
    first: u64,
    last: u64,
    sum: &'a str,
    fault: Option<ReportedFault<'a>>,
}

#[derive(Deserialize)]
struct ReportedFault<'a> {
    check: &'a str,
    previous: Claim,
    taken: Claim,
}

#[derive(Deserialize)]
struct Claim {
    round: i64,
    index: u64,
}

/// A burst that passed every check, ready to count.
struct Verified {
    count: u64,
    sum: u64,
}

/// Check one burst's answer: its own checks passed, it is of the lane's
/// side, in the run's round, performed the claims asked, its first and last
/// index leave room for strictly increasing indices, and its sum is one that
/// so many indices below 2^32 can make.
fn check(
    answer: &Answer<'_>,
    sum: u64,
    side: Side,
    round: i64,
    size: u32,
) -> Result<Verified, String> {
    if let Some(fault) = &answer.fault {
        let (previous, taken) = (&fault.previous, &fault.taken);
        return Err(
            if fault.check == "order" && previous.index == taken.index {
                format!(
                    "the burst's own order check failed: it was given index {} twice",
                    Grouped(taken.index)
                )
            } else if fault.check == "order" {
                format!(
                    "the burst's own order check failed: it took index {} after index {}",
                    Grouped(taken.index),
                    Grouped(previous.index)
                )
            } else {
                format!(
                    "the burst's own round check failed: after round {} it took a claim in round {}",
                    previous.round, taken.round
                )
            },
        );
    }
    if Side::parse(answer.side) != Some(side) {
        return Err(format!("it answered as side {:?}", answer.side));
    }
    if answer.round != round {
        return Err(format!(
            "it took claims in round {}, not the run's round {round}",
            answer.round
        ));
    }
    if answer.count != u64::from(size) {
        return Err(format!(
            "it performed {} claims of the {size} asked",
            answer.count
        ));
    }
    if answer.last < answer.first + (answer.count - 1) {
        return Err(format!(
            "it reports {} claims from index {} to index {}, too few indices to be strictly increasing",
            answer.count,
            Grouped(answer.first),
            Grouped(answer.last)
        ));
    }
    if sum > answer.count * u64::from(u32::MAX) {
        return Err(format!(
            "its index sum {} is more than {} indices below 2^32 can make",
            Grouped(sum),
            answer.count
        ));
    }
    Ok(Verified { count: answer.count, sum })
}

/// Perform one burst and count it, or say why it is rejected.
async fn perform(
    run: &Run,
    client: &Client,
    url: &str,
    lane: &Lane,
    burst: u64,
    size: u32,
) -> Result<(), String> {
    let body = loop {
        let response = match client.get(url).send().await {
            Ok(response) => response,
            // No connection, so nothing was sent and nothing taken: the
            // listener's backlog was full, as it can be with many more lanes
            // than the application runs. Counted as refused; send it again.
            Err(error) if error.is_connect() => {
                run.refused[lane.side as usize].fetch_add(1, Relaxed);
                continue;
            }
            Err(error) => {
                return Err(format!("the request failed: {}", Chain(&error)));
            }
        };
        let status = response.status();
        let body = response.bytes().await.map_err(|error| {
            format!("its answer was cut off: {}", Chain(&error))
        })?;
        if status == StatusCode::SERVICE_UNAVAILABLE && lane.side == Side::Php {
            // The application's admission was full: nothing ran, nothing
            // was taken. Send it again.
            run.refused[lane.side as usize].fetch_add(1, Relaxed);
            continue;
        }
        if !status.is_success() {
            return Err(format!(
                "it was answered {status}: {}",
                String::from_utf8_lossy(&body).trim()
            ));
        }
        break body;
    };
    let mut answer: Answer<'_> = serde_json::from_slice(&body)
        .map_err(|error| format!("its answer is not a burst ({error})"))?;
    let mut sum: u64 = answer.sum.parse().map_err(|_| {
        format!("its index sum {:?} is not a number", answer.sum)
    })?;
    if let Some((corruption, chosen)) = run.test
        && chosen == burst
    {
        corrupt(&mut answer, &mut sum, corruption);
    }
    let verified = check(&answer, sum, lane.side, run.round, size)
        .inspect_err(|_| {
            run.rejected.fetch_add(answer.count, Relaxed);
        })?;

    run.answered[lane.side as usize].fetch_add(verified.count, Relaxed);
    run.index_sum.fetch_add(verified.sum, Relaxed);
    lane.bursts.fetch_add(1, Relaxed);
    lane.claims.fetch_add(verified.count, Relaxed);
    let before = run.total.fetch_add(verified.count, Relaxed);
    if let Some(field) = &run.field {
        field.verified(burst, lane.side);
    }
    if run.plain {
        let after = before + verified.count;
        let tenth = after * 10 / run.target;
        if tenth != before * 10 / run.target {
            println!(
                "{:>4}%  {:>15} claims  {:>8.2} s  rust {} · php {}",
                tenth * 10,
                Grouped(after),
                run.elapsed().as_secs_f64(),
                Grouped(run.answered[0].load(Relaxed)),
                Grouped(run.answered[1].load(Relaxed)),
            );
        }
    }
    Ok(())
}

/// The checker test: change one answer after it was received.
fn corrupt(
    answer: &mut Answer<'_>,
    sum: &mut u64,
    corruption: Corruption,
) {
    match corruption {
        // As the server would report being given its last index twice.
        Corruption::Reported => {
            let claim = || Claim { round: answer.round, index: answer.last };
            answer.fault = Some(ReportedFault {
                check: "order",
                previous: claim(),
                taken: claim(),
            });
        }
        // The first index counted a second time in place of the last, with
        // nothing reported: the burst's own fields still look right.
        Corruption::Silent => *sum -= answer.last - answer.first,
    }
}

async fn lane(
    run: Arc<Run>,
    client: Client,
    routes: Arc<Routes>,
    index: usize,
) {
    let lane = &run.lanes[index];
    let mut state = DONE;
    while let Some((burst, size)) = run.take() {
        lane.state.store(WORKING, Relaxed);
        let sized;
        let url: &str = match (lane.side, size == run.burst) {
            (Side::Rust, true) => &routes.rust_full,
            (Side::Php, true) => &routes.php_full,
            (Side::Rust, false) => {
                sized = Routes::rust(&routes.base, size);
                &sized
            }
            (Side::Php, false) => {
                sized = Routes::php(&routes.base, &routes.id, size);
                &sized
            }
        };
        if let Err(reason) =
            perform(&run, &client, url, lane, burst, size).await
        {
            if let Some(field) = &run.field {
                field.fault(burst);
            }
            run.reject(format!(
                "{} lane {}, burst {}: {reason}",
                lane.side.title(),
                lane.number,
                Grouped(burst + 1)
            ));
            state = STOPPED;
            break;
        }
    }
    if state == DONE && run.stop.load(Relaxed) {
        state = STOPPED;
    }
    lane.state.store(state, Relaxed);
    if run.running.fetch_sub(1, AcqRel) == 1 {
        let _ = run.ended.set(run.started.elapsed());
    }
}

/// The evidence, read when every lane has ended. It rests on totals, which
/// scale to any run:
///
/// - **Answered**: the claims in verified bursts, which must be the target.
/// - **Shared value**: one more claim reads the value. It must be in the
///   run's round with index N, the claims answered. Every add writes the
///   value it read plus one, so reaching N from 0 in N adds needs each of the
///   values 1..N written once: each add read a different value, and the N
///   indices handed out were 0..N-1, once each. A lost add leaves the value
///   short; an add from outside the run leaves it over.
/// - **Index sum**: the sum of the indices the answers report must be
///   N(N-1)/2, the sum of 0..N-1. This checks the answers against the value:
///   an answer that reports an index it did not take moves the sum.
///
/// With the per-burst checks (each burst strictly increasing, in the run's
/// round), a run that passes all three was exactly once.
struct Evidence {
    target: u64,
    answered: u64,
    round: i64,
    value: Result<(i64, u64), String>,
    sum: u64,
    expected: u128,
    /// Claims performed by bursts that were rejected: in the value, not
    /// in the claims answered.
    rejected: u64,
    rejection: Option<String>,
}

impl Evidence {
    async fn read(
        run: &Run,
        client: &Client,
        base: &str,
    ) -> Self {
        let answered = run.total.load(Relaxed);
        let value = closing_claim(client, base).await;
        Self {
            target: run.target,
            answered,
            round: run.round,
            value,
            sum: run.index_sum.load(Relaxed),
            expected: u128::from(answered)
                * u128::from(answered.saturating_sub(1))
                / 2,
            rejected: run.rejected.load(Relaxed),
            rejection: run.rejection.get().cloned(),
        }
    }

    fn answered_holds(&self) -> bool {
        self.answered == self.target
    }

    fn value_holds(&self) -> bool {
        self.value == Ok((self.round, self.answered))
    }

    fn sum_holds(&self) -> bool {
        u128::from(self.sum) == self.expected
    }

    fn verified(&self) -> bool {
        self.rejection.is_none()
            && self.answered_holds()
            && self.value_holds()
            && self.sum_holds()
    }

    /// The three lines of evidence: whether each holds, and what it says.
    fn lines(&self) -> [(bool, &'static str, String); 3] {
        let value = match &self.value {
            Ok((round, index)) if *round != self.round => format!(
                "{} in round {round}, not the run's round {}",
                Grouped(*index),
                self.round
            ),
            Ok((_, index)) if *index == self.answered => format!(
                "{} in round {}, the claims answered",
                Grouped(*index),
                self.round
            ),
            Ok((_, index)) => format!(
                "{} in round {}, {} {} the claims answered{}",
                Grouped(*index),
                self.round,
                Grouped(index.abs_diff(self.answered)),
                if *index > self.answered { "over" } else { "short of" },
                match self.rejected {
                    0 => String::new(),
                    claims => {
                        format!(" ({} in rejected bursts)", Grouped(claims))
                    }
                }
            ),
            Err(error) => format!("unread: {error}"),
        };
        let sum = if self.sum_holds() {
            format!(
                "{} = N(N-1)/2 for N = {}",
                Grouped(self.sum),
                Grouped(self.answered)
            )
        } else {
            format!(
                "{}, expected N(N-1)/2 = {} ({} {})",
                Grouped(self.sum),
                Grouped128(self.expected),
                Grouped128(u128::from(self.sum).abs_diff(self.expected)),
                if u128::from(self.sum) > self.expected {
                    "over"
                } else {
                    "short"
                }
            )
        };
        [
            (
                self.answered_holds(),
                "answered",
                format!(
                    "{} of {} claims",
                    Grouped(self.answered),
                    Grouped(self.target)
                ),
            ),
            (self.value_holds(), "shared value", value),
            (self.sum_holds(), "index sum", sum),
        ]
    }

    fn verdict(&self) -> String {
        if self.verified() {
            return format!(
                "VERIFIED: every index from 0 to {} was taken exactly once, by Rust or by PHP",
                Grouped(self.answered.saturating_sub(1))
            );
        }
        let reason = if let Some(rejection) = &self.rejection {
            rejection.clone()
        } else if !self.value_holds() {
            match &self.value {
                Ok((round, _)) if *round != self.round => {
                    "something started another round during the run".to_owned()
                }
                Ok((_, index)) if *index > self.answered => {
                    "the value moved more than the claims answered: something else claimed during the run".to_owned()
                }
                Ok(_) => "the value moved less than the claims answered: an add was lost".to_owned(),
                Err(error) => format!("the value could not be read: {error}"),
            }
        } else if !self.sum_holds() {
            "the indices answered do not add up to 0..N-1: some index was reported more than once and another never".to_owned()
        } else {
            "the run ended before its target".to_owned()
        };
        format!("REJECTED: {reason}")
    }
}

/// One more claim: the value it takes is how many were taken before it.
async fn closing_claim(
    client: &Client,
    base: &str,
) -> Result<(i64, u64), String> {
    #[derive(Deserialize)]
    struct Taken {
        round: i64,
        index: u64,
    }
    let body = client
        .get(format!("{base}/_lumenize/rust/tiles"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let taken: Taken =
        serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    Ok((taken.round, taken.index))
}

/// Start a round and fetch the value's id.
async fn prepare(
    client: &Client,
    base: &str,
) -> Result<(i64, Box<str>), String> {
    #[derive(Deserialize)]
    struct Round {
        round: i64,
    }
    let unreachable = |error: reqwest::Error| {
        format!("{base} does not answer ({error}); is the application running?")
    };
    let body = client
        .delete(format!("{base}/_lumenize/rust/tiles"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(unreachable)?
        .bytes()
        .await
        .map_err(unreachable)?;
    let round: Round = serde_json::from_slice(&body).map_err(|error| {
        format!("starting a round answered no round: {error}")
    })?;
    let id = client
        .get(format!("{base}/_lumenize/rust/tiles/id"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(unreachable)?
        .text()
        .await
        .map_err(unreachable)?;
    Ok((round.round, id.trim().into()))
}

#[tokio::main]
async fn main() {
    let options = Options::parse();
    if let Err(message) = validate(&options) {
        eprintln!("orchestra: {message}");
        std::process::exit(2);
    }
    let base = options.url.trim_end_matches('/');
    let client = Client::new();
    let (round, id) = match prepare(&client, base).await {
        Ok(prepared) => prepared,
        Err(message) => {
            eprintln!("orchestra: {message}");
            std::process::exit(2);
        }
    };

    let mut terminal = (!options.plain).then(ratatui::init);
    let bursts = options.claims.div_ceil(u64::from(options.burst));
    let lights = match &terminal {
        Some(terminal) => {
            let size = terminal.size().unwrap_or_default();
            let areas = layout(
                Rect::new(0, 0, size.width, size.height),
                options.rust,
                options.php,
            );
            field_capacity(areas.field)
        }
        None => 0,
    };
    let lanes = (1..=options.rust)
        .map(|number| (Side::Rust, number))
        .chain((1..=options.php).map(|number| (Side::Php, number)))
        .map(|(side, number)| Lane {
            side,
            number,
            bursts: AtomicU64::new(0),
            claims: AtomicU64::new(0),
            state: AtomicU8::new(WAITING),
        })
        .collect::<Vec<_>>();
    let run = Arc::new(Run {
        target: options.claims,
        burst: options.burst,
        bursts,
        round,
        id: id.clone(),
        handed: AtomicU64::new(0),
        stop: AtomicBool::new(false),
        answered: [AtomicU64::new(0), AtomicU64::new(0)],
        total: AtomicU64::new(0),
        refused: [AtomicU64::new(0), AtomicU64::new(0)],
        index_sum: AtomicU64::new(0),
        rejected: AtomicU64::new(0),
        running: AtomicUsize::new(lanes.len()),
        lanes,
        rust_lanes: options.rust,
        php_lanes: options.php,
        field: (lights > 0).then(|| Field::new(bursts, lights)),
        rejection: OnceLock::new(),
        started: Instant::now(),
        ended: OnceLock::new(),
        plain: options.plain,
        test: options.test_checker.map(|corruption| (corruption, bursts / 2)),
    });

    if options.plain {
        println!(
            "An orchestra with no conductor: Rust and PHP on one Orbit atomic"
        );
        println!(
            "round {round}, Orbital {id}; {} claims in {} bursts of {}; {} Rust lanes, {} PHP lanes",
            Grouped(run.target),
            Grouped(bursts),
            Grouped(u64::from(run.burst)),
            options.rust,
            options.php,
        );
        if let Some((_, burst)) = run.test {
            println!(
                "TEST OF THE CHECKER: the answer of burst {} will be corrupted after it is received",
                Grouped(burst + 1)
            );
        }
    }

    let routes = Arc::new(Routes::new(base, &id, options.burst));
    let handles = (0..run.lanes.len())
        .map(|index| {
            tokio::spawn(lane(
                Arc::clone(&run),
                client.clone(),
                Arc::clone(&routes),
                index,
            ))
        })
        .collect::<Vec<_>>();

    let evidence = match terminal.as_mut() {
        None => {
            for handle in handles {
                let _ = handle.await;
            }
            Some(Evidence::read(&run, &client, base).await)
        }
        Some(terminal) => {
            let shown = show(terminal, &run, &client, base).await;
            ratatui::restore();
            shown
        }
    };
    let Some(evidence) = evidence else {
        println!(
            "Left before the end: {} claims answered of {}.",
            Grouped(run.total.load(Relaxed)),
            Grouped(run.target)
        );
        std::process::exit(130);
    };
    report(&run, &evidence);
    std::process::exit(if evidence.verified() { 0 } else { 1 });
}

fn validate(options: &Options) -> Result<(), String> {
    if options.claims == 0 {
        return Err("--claims must be at least 1".into());
    }
    if options.claims > CLAIMS_MAX {
        return Err(format!(
            "--claims {} is more than one round holds: its index is 32 bits, so a run makes at most {} claims (and one more reads the value)",
            Grouped(options.claims),
            Grouped(CLAIMS_MAX)
        ));
    }
    if options.burst == 0 || options.burst > BURST_MAX {
        return Err(format!(
            "--burst must be from 1 to {}, the most the server performs in one burst",
            Grouped(u64::from(BURST_MAX))
        ));
    }
    if options.rust + options.php == 0 {
        return Err("at least one lane is needed: --rust or --php".into());
    }
    Ok(())
}

/// What stays on the terminal after the run, and all of `--plain`'s ending.
fn report(
    run: &Run,
    evidence: &Evidence,
) {
    let seconds = run.elapsed().as_secs_f64();
    let side = |side: Side| {
        let claims = run.answered[side as usize].load(Relaxed);
        format!(
            "{} {}/s ({} claims, {} refused)",
            side.name(),
            Grouped(run.rate(claims)),
            Grouped(claims),
            Grouped(run.refused[side as usize].load(Relaxed)),
        )
    };
    println!("wall time      {seconds:.3} s");
    println!(
        "claims/s       {} overall; {}; {}",
        Grouped(run.rate(evidence.answered)),
        side(Side::Rust),
        side(Side::Php)
    );
    for (holds, name, text) in evidence.lines() {
        println!("{} {name:<13} {text}", if holds { "✓" } else { "✗" });
    }
    println!("{}", evidence.verdict());
}

/// Draw at the frame rate until the run has ended and a key is pressed, or
/// the user leaves. `None` when the user left before the evidence was read.
async fn show(
    terminal: &mut DefaultTerminal,
    run: &Arc<Run>,
    client: &Client,
    base: &str,
) -> Option<Evidence> {
    let mut reading: Option<tokio::task::JoinHandle<Evidence>> = None;
    let mut evidence: Option<Evidence> = None;
    let mut frames =
        tokio::time::interval(Duration::from_secs(1) / FRAMES_PER_SECOND);
    frames.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        frames.tick().await;
        if run.finished() && reading.is_none() && evidence.is_none() {
            let (run, client, base) =
                (Arc::clone(run), client.clone(), Box::<str>::from(base));
            reading = Some(tokio::spawn(async move {
                Evidence::read(&run, &client, &base).await
            }));
        }
        if let Some(handle) = reading.take_if(|handle| handle.is_finished()) {
            evidence = handle.await.ok();
        }
        let _ = terminal.draw(|frame| draw(frame, run, evidence.as_ref()));
        // Zero: take the keys already pressed, never wait for one.
        while event::poll(Duration::ZERO).unwrap_or(false) {
            let Ok(Event::Key(key)) = event::read() else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if evidence.is_some() {
                return evidence;
            }
            let leave = matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                || (key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL));
            if leave {
                run.stop.store(true, Relaxed);
                return None;
            }
        }
    }
}

struct Areas {
    title: Rect,
    stats: Rect,
    rust: Rect,
    php: Rect,
    field: Rect,
    evidence: Rect,
    footer: Rect,
}

/// The width from which the run's numbers stand beside the lanes.
const WIDE: u16 = 100;
/// The run's numbers in full (seven lines) and compact (four), with borders.
const STATS_FULL: u16 = 9;
const STATS_COMPACT: u16 = 6;
/// The evidence's four lines with borders, and the least field kept.
const EVIDENCE_HEIGHT: u16 = 6;
const FIELD_LEAST: u16 = 3;

/// The screen: a title line, the run's numbers beside the lanes (above them,
/// compact, when narrow), the field, the evidence and a footer. Laid out for
/// 120x36. A shorter terminal keeps the evidence and a field of three rows,
/// and shows fewer lanes; a smaller one clips.
fn layout(
    area: Rect,
    rust_lanes: usize,
    php_lanes: usize,
) -> Areas {
    let lane_rows =
        rust_lanes.max(php_lanes).min(usize::from(u16::MAX - 3)) as u16;
    // Borders and a header line around the lanes.
    let lanes_height = lane_rows + 3;
    let wide = area.width >= WIDE;
    let stats_height = if wide { STATS_FULL } else { STATS_COMPACT };
    let spare =
        area.height.saturating_sub(1 + EVIDENCE_HEIGHT + 1 + FIELD_LEAST);
    let top_height = if wide {
        lanes_height.max(stats_height)
    } else {
        stats_height + lanes_height
    }
    .min(spare);
    let [title, top, field, evidence, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(top_height),
        Constraint::Fill(1),
        Constraint::Length(EVIDENCE_HEIGHT),
        Constraint::Length(1),
    ])
    .areas(area);
    let (stats, lanes) = if wide {
        let [stats, lanes] =
            Layout::horizontal([Constraint::Length(44), Constraint::Fill(1)])
                .areas(top);
        (stats, lanes)
    } else {
        let [stats, lanes] = Layout::vertical([
            Constraint::Length(stats_height.min(top.height)),
            Constraint::Fill(1),
        ])
        .areas(top);
        (stats, lanes)
    };
    let [rust, php] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
            .areas(lanes);
    Areas { title, stats, rust, php, field, evidence, footer }
}

/// Lights that fit the field: one every second column, inside its border.
fn field_capacity(field: Rect) -> usize {
    usize::from(field.width.saturating_sub(2) / 2)
        * usize::from(field.height.saturating_sub(2))
}

fn draw(
    frame: &mut Frame<'_>,
    run: &Run,
    evidence: Option<&Evidence>,
) {
    let areas = layout(frame.area(), run.rust_lanes, run.php_lanes);
    let muted = Style::new().fg(MUTED);

    let mut title = vec![
        Span::styled(
            "An orchestra with no conductor",
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::styled("   Rust and PHP on one Orbit atomic", muted),
    ];
    if run.test.is_some() {
        title.push(Span::styled(
            "   TEST OF THE CHECKER",
            Style::new().fg(BAD).add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(title)), areas.title);

    draw_stats(frame, run, areas.stats);
    for (side, area) in [(Side::Rust, areas.rust), (Side::Php, areas.php)] {
        draw_lanes(frame, run, side, area);
    }
    draw_field(frame, run, areas.field);
    draw_evidence(frame, run, evidence, areas.evidence);

    let footer = if evidence.is_some() {
        "the run has ended · press any key to leave"
    } else if run.finished() {
        "reading the shared value… · q leaves"
    } else {
        "q leaves"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(footer, muted)),
        areas.footer,
    );
}

fn draw_stats(
    frame: &mut Frame<'_>,
    run: &Run,
    area: Rect,
) {
    let muted = Style::new().fg(MUTED);
    let total = run.total.load(Relaxed);
    let label = |text: &'static str| Span::styled(format!("{text:<10}"), muted);
    let number = |value: u64| format!("{:>13}", Grouped(value));
    let percent = total as f64 * 100.0 / run.target as f64;
    let bar_width = area.width.saturating_sub(2) as usize;
    let (rust, php) =
        (run.answered[0].load(Relaxed), run.answered[1].load(Relaxed));
    let refused = run.refused[0].load(Relaxed) + run.refused[1].load(Relaxed);
    let bar = |width: usize| {
        let filled = ((u128::from(total) * width as u128)
            / u128::from(run.target)) as usize;
        [
            Span::raw("█".repeat(filled.min(width))),
            Span::styled("░".repeat(width - filled.min(width)), muted),
        ]
    };
    let compact = area.height < STATS_FULL;
    let lines = if compact {
        let head = format!("claims {} {percent:>5.1}% ", Grouped(total));
        let mut claims = vec![Span::raw(head.clone())];
        claims.extend(bar(bar_width.saturating_sub(head.chars().count())));
        vec![
            Line::from(vec![Span::styled(
                format!(
                    "target {} claims · {} bursts of {}",
                    Grouped(run.target),
                    Grouped(run.bursts),
                    Grouped(u64::from(run.burst))
                ),
                muted,
            )]),
            Line::from(claims),
            Line::from(vec![
                Span::raw(format!("claims/s {} · ", Grouped(run.rate(total)))),
                Span::styled(
                    format!("rust {}", Grouped(run.rate(rust))),
                    Style::new().fg(RUST),
                ),
                Span::raw(" · "),
                Span::styled(
                    format!("php {}", Grouped(run.rate(php))),
                    Style::new().fg(PHP),
                ),
            ]),
            Line::styled(
                format!(
                    "elapsed {:.2} s · refused {}",
                    run.elapsed().as_secs_f64(),
                    Grouped(refused)
                ),
                muted,
            ),
        ]
    } else {
        vec![
            Line::from(vec![
                label("target"),
                Span::raw(number(run.target)),
                Span::styled(" claims", muted),
            ]),
            Line::from(vec![
                label("bursts"),
                Span::raw(number(run.bursts)),
                Span::styled(
                    format!(" of {}", Grouped(u64::from(run.burst))),
                    muted,
                ),
            ]),
            Line::from(vec![
                label("claims"),
                Span::raw(number(total)),
                Span::raw(format!(" {percent:>5.1}%")),
            ]),
            Line::from(bar(bar_width).to_vec()),
            Line::from(vec![
                label("claims/s"),
                Span::raw(number(run.rate(total))),
                Span::styled(" overall", muted),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("{:>23}", Grouped(run.rate(rust))),
                    Style::new().fg(RUST),
                ),
                Span::raw(" rust "),
                Span::styled(
                    format!("{}", Grouped(run.rate(php))),
                    Style::new().fg(PHP),
                ),
                Span::raw(" php"),
            ]),
            Line::from(vec![
                label("elapsed"),
                Span::raw(format!("{:>11.2} s", run.elapsed().as_secs_f64())),
                Span::styled(format!(" · refused {}", Grouped(refused)), muted),
            ]),
        ]
    };
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered().border_style(muted).title(Span::raw(format!(
                " round {} · Orbital {} ",
                run.round, run.id
            ))),
        ),
        area,
    );
}

fn draw_lanes(
    frame: &mut Frame<'_>,
    run: &Run,
    side: Side,
    area: Rect,
) {
    let lanes: Vec<&Lane> =
        run.lanes.iter().filter(|lane| lane.side == side).collect();
    let rows = area.height.saturating_sub(3) as usize;
    let muted = Style::new().fg(MUTED);
    let colour = Style::new().fg(side.colour());
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(rows + 1);
    if lanes.is_empty() {
        lines.push(Line::styled(format!(" no {} lanes", side.title()), muted));
    } else {
        lines.push(Line::styled(
            format!("{:>19}{:>14}", "bursts", "claims"),
            muted,
        ));
    }
    let shown =
        if lanes.len() > rows { rows.saturating_sub(1) } else { lanes.len() };
    for lane in &lanes[..shown] {
        let bursts = lane.bursts.load(Relaxed);
        let (mark, style) = match lane.state.load(Relaxed) {
            WORKING => (["◐", "◓", "◑", "◒"][(bursts % 4) as usize], colour),
            DONE => ("✓", colour),
            STOPPED => ("■", muted),
            _ => ("·", muted),
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {mark} "), style),
            Span::styled(
                format!("{:<4} {:<3}", side.name(), lane.number),
                colour,
            ),
            Span::raw(format!("{:>8}", Grouped(bursts))),
            Span::raw(format!("{:>14}", Grouped(lane.claims.load(Relaxed)))),
        ]));
    }
    if shown < lanes.len() {
        lines.push(Line::styled(
            format!("   … {} more lanes", lanes.len() - shown),
            muted,
        ));
    }
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered().border_style(muted).title(Span::styled(
                format!(" {} · {} lanes ", side.title(), lanes.len()),
                colour,
            )),
        ),
        area,
    );
}

fn draw_field(
    frame: &mut Frame<'_>,
    run: &Run,
    area: Rect,
) {
    let muted = Style::new().fg(MUTED);
    let Some(field) = &run.field else { return };
    let (fewest, most) = field.per_light();
    let claims = u64::from(run.burst);
    let stands = if most == 1 {
        format!(
            " fireflies · one light = one burst of {} claims ",
            Grouped(claims)
        )
    } else if fewest == most {
        format!(
            " fireflies · one light = {} bursts ({} claims) ",
            Grouped(most),
            Grouped(most * claims)
        )
    } else {
        format!(
            " fireflies · one light = {} or {} bursts (up to {} claims) ",
            Grouped(fewest),
            Grouped(most),
            Grouped(most * claims)
        )
    };
    let block = Block::bordered().border_style(muted).title(Span::raw(stands));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let columns = usize::from(inner.width / 2);
    if columns == 0 || inner.height == 0 {
        return;
    }
    let buffer: &mut Buffer = frame.buffer_mut();
    let fits = columns * usize::from(inner.height);
    for (position, shown) in field.shown.iter().enumerate().take(fits) {
        let x = inner.x + (position % columns) as u16 * 2;
        let y = inner.y + (position / columns) as u16;
        let (symbol, style) = match shown.load(Relaxed) {
            LIT_RUST => ("●", Style::new().fg(RUST)),
            LIT_PHP => ("●", Style::new().fg(PHP)),
            FAULT => ("✕", Style::new().fg(BAD).add_modifier(Modifier::BOLD)),
            _ => ("·", Style::new().fg(UNLIT)),
        };
        buffer[(x, y)].set_symbol(symbol).set_style(style);
    }
    if fits < field.lights() {
        let note = format!(
            " {} lights not shown: enlarge the terminal ",
            field.lights() - fits
        );
        let width = (note.chars().count() as u16).min(area.width);
        let x = area.x + area.width.saturating_sub(width + 1);
        buffer.set_stringn(
            x,
            area.y + area.height - 1,
            &note,
            width as usize,
            muted,
        );
    }
}

fn draw_evidence(
    frame: &mut Frame<'_>,
    run: &Run,
    evidence: Option<&Evidence>,
    area: Rect,
) {
    let muted = Style::new().fg(MUTED);
    let lines: Vec<Line<'_>> = match evidence {
        None => {
            let mut lines = vec![
                Line::styled(
                    "The evidence is read when every lane has ended:",
                    muted,
                ),
                Line::styled(
                    "the claims answered, the shared value, and the index sum against N(N-1)/2.",
                    muted,
                ),
            ];
            if let Some(rejection) = run.rejection.get() {
                lines.push(Line::styled(
                    format!("REJECTED: {rejection}"),
                    Style::new().fg(BAD).add_modifier(Modifier::BOLD),
                ));
            }
            lines
        }
        Some(evidence) => {
            let mut lines: Vec<Line<'_>> = evidence
                .lines()
                .into_iter()
                .map(|(holds, name, text)| {
                    let (mark, style) = if holds {
                        ("✓", Style::new().fg(GOOD))
                    } else {
                        ("✗", Style::new().fg(BAD))
                    };
                    Line::from(vec![
                        Span::styled(format!("{mark} {name:<13}"), style),
                        Span::raw(text),
                    ])
                })
                .collect();
            let style = Style::new()
                .fg(if evidence.verified() { GOOD } else { BAD })
                .add_modifier(Modifier::BOLD);
            lines.push(Line::styled(evidence.verdict(), style));
            lines
        }
    };
    let border = match evidence {
        Some(evidence) if evidence.verified() => Style::new().fg(GOOD),
        Some(_) => Style::new().fg(BAD),
        None => muted,
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().border_style(border).title(" evidence ")),
        area,
    );
}

/// An error and every error beneath it, as one line.
struct Chain<'a>(&'a dyn std::error::Error);

impl fmt::Display for Chain<'_> {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(formatter, "{}", self.0)?;
        let mut source = self.0.source();
        while let Some(error) = source {
            write!(formatter, ": {error}")?;
            source = error.source();
        }
        Ok(())
    }
}

/// A count with thousands separators.
struct Grouped(u64);

impl fmt::Display for Grouped {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        Grouped128(u128::from(self.0)).fmt(formatter)
    }
}

struct Grouped128(u128);

impl fmt::Display for Grouped128 {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let digits = self.0.to_string();
        let mut grouped = String::with_capacity(digits.len() * 4 / 3);
        for (i, digit) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(digit);
        }
        formatter.pad(&grouped)
    }
}
