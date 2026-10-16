// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The crash-loop breaker** (#2007, ADR 0068 §3): the one piece of supervision code purlis
//! writes. The window starts its session host again after each unexpected exit, until three
//! have come within five minutes; then it stops and says the host is failing, and only the
//! person's Try again starts it once more. The same limits a service manager's own breaker is
//! given (systemd's `StartLimitBurst` and `StartLimitIntervalSec`).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How many unexpected exits within [`WITHIN`] trip the breaker.
pub const EXITS: usize = 3;

/// How far back an unexpected exit counts.
pub const WITHIN: Duration = Duration::from_secs(5 * 60);

/// What follows an unexpected exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// The host is started again.
    StartAgain,
    /// The breaker tripped: nothing starts it until the person tries again.
    Tripped,
}

/// The unexpected exits that still count, oldest first. In memory, for this window's run: a
/// launch starts with none, as a service manager's count starts with its own.
#[derive(Debug, Default)]
pub struct Breaker {
    exits: VecDeque<Instant>,
}

impl Breaker {
    /// The host ended unexpectedly `at`: whether it is started again.
    pub fn exited(&mut self, at: Instant) -> After {
        while self
            .exits
            .front()
            .is_some_and(|first| at.saturating_duration_since(*first) >= WITHIN)
        {
            self.exits.pop_front();
        }
        self.exits.push_back(at);
        if self.exits.len() >= EXITS {
            After::Tripped
        } else {
            After::StartAgain
        }
    }

    /// The person pressed Try again: the exits before count no more.
    pub fn reset(&mut self) {
        self.exits.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    #[test]
    fn a_host_that_ends_unexpectedly_is_started_again_until_the_third_exit_in_five_minutes() {
        let start = Instant::now();
        let mut breaker = Breaker::default();

        assert_eq!(breaker.exited(start), After::StartAgain);
        assert_eq!(breaker.exited(start + minutes(1)), After::StartAgain);
        assert_eq!(breaker.exited(start + minutes(4)), After::Tripped);
    }

    #[test]
    fn exits_further_apart_than_five_minutes_never_trip_it() {
        let start = Instant::now();
        let mut breaker = Breaker::default();

        for n in 0..10 {
            assert_eq!(
                breaker.exited(start + minutes(3 * n)),
                After::StartAgain,
                "exit {n}"
            );
        }
    }

    #[test]
    fn an_exit_five_minutes_old_no_longer_counts() {
        let start = Instant::now();
        let mut breaker = Breaker::default();
        breaker.exited(start);
        breaker.exited(start + minutes(2));

        assert_eq!(breaker.exited(start + minutes(5)), After::StartAgain);
    }

    #[test]
    fn try_again_starts_the_count_over() {
        let start = Instant::now();
        let mut breaker = Breaker::default();
        for n in 0..3 {
            breaker.exited(start + Duration::from_secs(n));
        }

        breaker.reset();

        assert_eq!(breaker.exited(start + minutes(1)), After::StartAgain);
        assert_eq!(breaker.exited(start + minutes(2)), After::StartAgain);
    }
}
