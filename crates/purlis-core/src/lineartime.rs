// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a test says a reading takes time linear in its input, in a way a loaded runner does not
//! break and a quadratic reading still fails (#1800).
//!
//! A wall-clock bound alone says neither: a runner three times busier than the one the bound
//! was set on fails a linear reading, and a quadratic one passes on a fast machine. So the
//! reading is timed at a size and at four times it, and the two are compared: a linear reading
//! takes about four times as long, a quadratic one sixteen. Each size is timed a few times,
//! alternately, and its fastest time is the one compared, so a pause of the runner during one
//! round is not counted as the reading's.

use std::time::{Duration, Instant};

/// How many times each size is timed.
const ROUNDS: usize = 3;

/// How much the input grows between the two sizes.
pub(crate) const GROWTH: usize = 4;

/// The most the larger size may take, as a multiple of the smaller's time: a linear reading is
/// [`GROWTH`] (4), a quadratic one 16. Halfway between the two on a log scale.
const MOST_RATIO: f64 = 8.0;

/// What the whole check may take at most, however busy the runner: well past what a linear
/// reading takes on a loaded CI runner, so only a reading gone badly wrong reaches it.
const CEILING: Duration = Duration::from_secs(120);

/// Fails unless `read(n * GROWTH)` takes under [`MOST_RATIO`] times what `read(n)` takes.
/// `read` does its own asserts, so what is timed is checked too.
pub(crate) fn reads_in_linear_time(n: usize, mut read: impl FnMut(usize)) {
    let began = Instant::now();
    let mut small = Duration::MAX;
    let mut large = Duration::MAX;
    for _ in 0..ROUNDS {
        for (size, best) in [(n, &mut small), (n * GROWTH, &mut large)] {
            let started = Instant::now();
            read(size);
            *best = (*best).min(started.elapsed());
        }
    }
    // A floor under the smaller time, so a reading too fast to time is not a division by zero.
    let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
    assert!(
        ratio < MOST_RATIO,
        "{GROWTH} times the input took {ratio:.1} times as long ({small:?}, then {large:?}): not linear"
    );
    let took = began.elapsed();
    assert!(took < CEILING, "{took:?}");
}
