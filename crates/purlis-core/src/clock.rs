// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one clock that deadlines are read against** (R-696, design §5.4).
//!
//! The key proxy's deadlines, idle timeouts, its 24-hour cache, a token's refresh and its rate
//! limits all ask a [`Clock`] what time it is, and wait on it. A build that ships uses
//! [`Monotonic`], the machine's own monotonic clock. A test hands in the injected clock
//! (`test-support` only) and moves it by hand, so no test sleeps on the wall clock for a
//! budget's sake.

use std::time::{Duration, Instant};

/// What time it is, and a way to wait until a time.
pub trait Clock: Send + Sync {
    /// Now, on this clock.
    fn now(&self) -> Instant;

    /// Returns once this clock has reached `at`; at once when it already has.
    fn sleep_until(&self, at: Instant);
}

/// The machine's monotonic clock: what every build that ships reads.
#[derive(Debug, Default, Clone, Copy)]
pub struct Monotonic;

impl Clock for Monotonic {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, at: Instant) {
        let now = Instant::now();
        if at > now {
            std::thread::sleep(at - now);
        }
    }
}

/// A moment on a clock after which something is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    at: Instant,
}

impl Deadline {
    /// The deadline `wait` from now on `clock`.
    pub fn after(clock: &dyn Clock, wait: Duration) -> Deadline {
        Deadline {
            at: clock.now() + wait,
        }
    }

    /// When it falls due.
    pub fn at(&self) -> Instant {
        self.at
    }

    /// Whether `clock` has reached it.
    pub fn passed(&self, clock: &dyn Clock) -> bool {
        clock.now() >= self.at
    }
}

#[cfg(feature = "test-support")]
pub use injected::InjectedClock;

#[cfg(feature = "test-support")]
mod injected {
    use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
    use std::time::{Duration, Instant};

    use super::Clock;

    /// **A clock a test moves by hand.** It stands still until [`InjectedClock::advance`] moves it,
    /// and wakes whoever waits on it then. Only tests build it (`test-support`).
    #[derive(Debug)]
    pub struct InjectedClock {
        start: Instant,
        state: Mutex<State>,
        moved: Condvar,
    }

    #[derive(Debug, Default)]
    struct State {
        elapsed: Duration,
        /// The time each caller in [`Clock::sleep_until`] waits for, one entry per caller.
        waiting: Vec<Instant>,
    }

    impl Default for InjectedClock {
        fn default() -> Self {
            InjectedClock::new()
        }
    }

    impl InjectedClock {
        /// A clock at its start, standing still.
        pub fn new() -> InjectedClock {
            InjectedClock {
                start: Instant::now(),
                state: Mutex::new(State::default()),
                moved: Condvar::new(),
            }
        }

        /// Moves it on by `by`, waking everyone waiting for a time it has now reached.
        pub fn advance(&self, by: Duration) {
            self.lock().elapsed += by;
            self.moved.notify_all();
        }

        /// How far it has been moved since it was made.
        pub fn elapsed(&self) -> Duration {
            self.lock().elapsed
        }

        /// How many callers are waiting on it in [`Clock::sleep_until`] for a time it has not
        /// reached. One that [`InjectedClock::advance`] has woken no longer counts, even before
        /// it has run again, so a test that waits for a count sees the next wait, not the last.
        pub fn sleepers(&self) -> usize {
            let state = self.lock();
            let now = self.start + state.elapsed;
            state.waiting.iter().filter(|at| **at > now).count()
        }

        fn lock(&self) -> MutexGuard<'_, State> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    impl Clock for InjectedClock {
        fn now(&self) -> Instant {
            self.start + self.lock().elapsed
        }

        fn sleep_until(&self, at: Instant) {
            let mut state = self.lock();
            if self.start + state.elapsed >= at {
                return;
            }
            state.waiting.push(at);
            while self.start + state.elapsed < at {
                state = self
                    .moved
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if let Some(place) = state.waiting.iter().position(|waits| *waits == at) {
                state.waiting.swap_remove(place);
            }
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::{Clock, InjectedClock};

    #[test]
    fn a_waiter_the_injected_clock_has_woken_is_no_longer_counted_as_a_sleeper() {
        const WAITERS: usize = 8;
        let clock = Arc::new(InjectedClock::new());
        let at = clock.now() + Duration::from_millis(10);
        let waiters: Vec<_> = (0..WAITERS)
            .map(|_| {
                let clock = Arc::clone(&clock);
                std::thread::spawn(move || clock.sleep_until(at))
            })
            .collect();
        while clock.sleepers() < WAITERS {
            std::thread::yield_now();
        }

        clock.advance(Duration::from_millis(10));

        assert_eq!(
            clock.sleepers(),
            0,
            "a woken waiter still counted before it ran again"
        );
        for waiter in waiters {
            waiter.join().unwrap();
        }
    }

    #[test]
    fn a_waiter_for_a_time_not_yet_reached_still_counts_after_an_advance() {
        let clock = Arc::new(InjectedClock::new());
        let at = clock.now() + Duration::from_millis(20);
        let waiter = {
            let clock = Arc::clone(&clock);
            std::thread::spawn(move || clock.sleep_until(at))
        };
        while clock.sleepers() < 1 {
            std::thread::yield_now();
        }

        clock.advance(Duration::from_millis(10));
        assert_eq!(clock.sleepers(), 1);

        clock.advance(Duration::from_millis(10));
        assert_eq!(clock.sleepers(), 0);
        waiter.join().unwrap();
    }
}
