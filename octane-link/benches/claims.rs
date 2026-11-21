//! What one claim costs through each language, and whether the shared value
//! still adds up afterwards.
//!
//! The application must be running: this is a client. Start it, then
//!
//! ```sh
//! cargo bench
//! ```
//!
//! `HARMONY_URL` names the application (default `http://127.0.0.1:8102`, the
//! example's plain listener). Each benchmark claims tiles from the shared
//! atomic `tiles`, through the Rust route, the PHP route, or both at once.
//! Criterion reports the time of one iteration; this file counts the claims
//! the server answered with success. When the benchmarks are done the value
//! must equal that count, in the round the run started.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput};
use futures_util::future::join_all;
use reqwest::{Client, StatusCode};
use tokio::runtime::Runtime;

/// Claims the server answered with success.
static ANSWERED: AtomicU64 = AtomicU64::new(0);
/// Claims the PHP application refused before running them; they take nothing.
static REFUSED: AtomicU64 = AtomicU64::new(0);

struct Routes {
    rust: String,
    php: String,
}

async fn claim(
    client: &Client,
    url: &str,
) {
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => {
            // The body carries the tile; read it so the connection is reused.
            let _ = response.bytes().await;
            ANSWERED.fetch_add(1, Ordering::Relaxed);
        }
        Ok(response)
            if response.status() == StatusCode::SERVICE_UNAVAILABLE =>
        {
            REFUSED.fetch_add(1, Ordering::Relaxed);
        }
        Ok(response) => panic!("claim answered {}", response.status()),
        Err(error) => panic!("claim failed: {error}"),
    }
}

/// The value's two halves, as the claim routes report them.
async fn read(
    client: &Client,
    url: &str,
    method: reqwest::Method,
) -> (u64, Option<u64>) {
    let text = client
        .request(method, url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .expect("the application answers; is it running?")
        .text()
        .await
        .expect("a body");
    let field = |name: &str| {
        let key = format!("\"{name}\":");
        let rest = &text[text.find(&key)? + key.len()..];
        let end =
            rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        rest[..end].parse::<u64>().ok()
    };
    (field("round").expect("a round"), field("index"))
}

fn benches(
    criterion: &mut Criterion,
    runtime: &Runtime,
    client: &Client,
    routes: &Routes,
) {
    let mut one = criterion.benchmark_group("one claim");
    one.throughput(Throughput::Elements(1));
    for (side, url) in [("rust", &routes.rust), ("php", &routes.php)] {
        one.bench_function(side, |bench| {
            bench.to_async(runtime).iter(|| claim(client, url));
        });
    }
    one.finish();

    // Rust and PHP at the same moment, on the same value.
    let mut together = criterion.benchmark_group("both at once");
    for pairs in [1_u64, 4] {
        together.throughput(Throughput::Elements(pairs * 2));
        together.bench_with_input(
            BenchmarkId::from_parameter(format!("{pairs} rust + {pairs} php")),
            &pairs,
            |bench, &pairs| {
                bench.to_async(runtime).iter(|| {
                    join_all((0..pairs).flat_map(|_| {
                        [
                            claim(client, &routes.rust),
                            claim(client, &routes.php),
                        ]
                    }))
                });
            },
        );
    }
    together.finish();
}

fn main() {
    let base = std::env::var("HARMONY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8102".into());
    let routes = Routes {
        rust: format!("{base}/_lumenize/rust/tiles"),
        php: format!("{base}/_lumenize/php/tiles"),
    };
    let runtime = Runtime::new().expect("a runtime");
    let client = Client::builder().build().expect("a client");

    // A new round sets the value to zero.
    let (round, _) =
        runtime.block_on(read(&client, &routes.rust, reqwest::Method::DELETE));

    let mut criterion = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .configure_from_args();
    benches(&mut criterion, &runtime, &client, &routes);
    criterion.final_summary();

    // One more claim reads the value: the tile it takes is how many were
    // taken before it.
    let (ended, value) =
        runtime.block_on(read(&client, &routes.rust, reqwest::Method::GET));
    let value = value.expect("a tile");
    let answered = ANSWERED.load(Ordering::Relaxed);
    let refused = REFUSED.load(Ordering::Relaxed);

    println!();
    println!("claims answered with success: {answered}");
    println!("claims refused (took nothing): {refused}");
    println!("shared value:                 {value}");
    if ended != round {
        println!(
            "NOT VERIFIED: the round changed from {round} to {ended}; \
             something else started one during the run"
        );
        std::process::exit(1);
    }
    if value != answered {
        println!(
            "NOT VERIFIED: the value and the claims answered differ by {}",
            value.abs_diff(answered)
        );
        std::process::exit(1);
    }
    println!(
        "VERIFIED: every claim answered with success is in the value, \
         and nothing else is"
    );
}
