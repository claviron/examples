use lumenize::claviron::orbit::{Declare, orbital};

pub type HitsService = Declare<Hits>;
pub type TilesService = Declare<Tiles>;
orbital! {
    /// The fleet's hit count, one value in the Orbitals table for every
    /// process of the fleet.
    ///
    /// Declared kept, as a counter is, so it is also the counter `hits`: PHP's
    /// `orbit_counter_inc('hits', 1)` adds to this same value. Signed, because
    /// PHP has no unsigned integer.
    pub Hits: i64 = 0, "hits", kept

    /// The tiles page's claims: the round in the high 32 bits, the next tile
    /// in the low 32. A claim is one add of 1 and takes the value before it,
    /// so the round and the tile it names come from one word and cannot
    /// disagree.
    ///
    /// Kept, so it is also the counter `tiles`: PHP's
    /// `orbit_counter_inc('tiles', 1)` claims from this same value. PHP's
    /// bursts open it by id, which `/_lumenize/rust/tiles/id` hands out.
    pub Tiles: i64 = 0, "tiles", kept
}

/// The round a value of [`Tiles`] belongs to.
pub const fn tiles_round(value: i64) -> i64 {
    value >> 32
}

/// The tile a value of [`Tiles`] names.
pub const fn tiles_index(value: i64) -> i64 {
    value & 0xffff_ffff
}

/// The first value of the round after the one `value` belongs to: that
/// round, with no tile taken yet.
pub const fn tiles_next_round(value: i64) -> i64 {
    (tiles_round(value) + 1) << 32
}

/// The most claims one burst performs. A burst runs on the thread that
/// serves its request and holds it until it is done, so a request may not
/// ask for an unbounded loop. PHP's burst has the same bound.
pub const BURST_MAX: u32 = 1_000_000;

/// What one burst of claims took, and whether its own checks held.
pub struct Burst {
    /// The round of the first claim; every other claim must be in it.
    pub round: i64,
    /// The claims performed: the count asked for, or fewer when a check
    /// failed, counting the claim that failed it.
    pub count: u32,
    /// The index of the first claim.
    pub first: i64,
    /// The index of the last claim.
    pub last: i64,
    /// The sum of the indices taken. At most [`BURST_MAX`] indices below
    /// 2^32 each, so it stays below 2^52.
    pub sum: u64,
    pub fault: Option<Fault>,
}

/// A check a burst failed: the claim before and the claim that broke it,
/// both as whole values of [`Tiles`].
pub enum Fault {
    /// A claim from another round than the first claim's.
    Round { previous: i64, taken: i64 },
    /// A claim that did not take a greater index than the one before it.
    Order { previous: i64, taken: i64 },
}

/// Perform `count` claims (at least one) in a tight loop, one `fetch_add(1)`
/// each, checking as it goes that every claim is in the first claim's round
/// and takes a greater index than the one before it. An atomic add never
/// gives one caller the same or a smaller index twice, so in a correct run
/// the check never fires.
///
/// Within one round the round bits are equal, so "greater index" is "greater
/// value"; an index that wraps past 2^32 carries into the round bits and is
/// caught as another round. Both tests fold into one branch that is never
/// taken, so the loop costs the atomic add (a locked read-modify-write on a
/// cache line every claimer shares, and one load of the waiter word on that
/// line) and a few register operations.
pub fn burst(
    tiles: &Tiles,
    count: u32,
) -> Burst {
    let first = tiles.fetch_add(1);
    let round = tiles_round(first);
    let mut previous = first;
    let mut sum = tiles_index(first) as u64;
    let mut done = 1;
    let mut fault = None;
    while done < count {
        let taken = tiles.fetch_add(1);
        sum += tiles_index(taken) as u64;
        done += 1;
        if (taken <= previous) | (tiles_round(taken) != round) {
            fault = Some(if tiles_round(taken) == round {
                Fault::Order { previous, taken }
            } else {
                Fault::Round { previous, taken }
            });
            previous = taken;
            break;
        }
        previous = taken;
    }
    Burst {
        round,
        count: done,
        first: tiles_index(first),
        last: tiles_index(previous),
        sum,
        fault,
    }
}
