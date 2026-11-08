// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A call that waits on the person, and notices its asker hang up.** A browser login's notice,
//! a secret fill's and the Secret ask a fill raises all wait here: for an answer, at most as
//! long as they may, looking every [`LOOKS_AGAIN`] whether the call that waits is still there.

use std::sync::mpsc;
use std::time::{Duration, Instant};

/// **How a wait on the person ended** ([`waited`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Ended<H> {
    /// The answer.
    Heard(H),
    /// What answers it went away, and its wait with it.
    Closed,
    /// Nobody answered in time.
    NotAnswered,
    /// The call's asker hung up.
    AskerGone,
}

/// How often a waiting call looks whether its asker hung up.
pub const LOOKS_AGAIN: Duration = Duration::from_secs(1);

/// **The answer `heard` brings**, waited for `at_most`, looking every [`LOOKS_AGAIN`] whether
/// the call's asker is `gone`: past the time, or with nobody waiting any more, `settle` ends the
/// wait. It answers the answer that came in that very moment, where one did, and that answer
/// wins.
pub fn waited<H>(
    heard: &mpsc::Receiver<H>,
    at_most: Duration,
    gone: &dyn Fn() -> bool,
    settle: &dyn Fn() -> Option<H>,
) -> Ended<H> {
    let until = Instant::now() + at_most;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match heard.recv_timeout(left.min(LOOKS_AGAIN)) {
            Ok(one) => return Ended::Heard(one),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ended::Closed,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let hung_up = gone();
                if !hung_up && Instant::now() < until {
                    continue;
                }
                if let Some(one) = settle() {
                    return Ended::Heard(one);
                }
                return if hung_up {
                    Ended::AskerGone
                } else {
                    Ended::NotAnswered
                };
            }
        }
    }
}
