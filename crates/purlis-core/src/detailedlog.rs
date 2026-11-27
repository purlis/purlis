// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Detailed remote log for one hour** (#7864, decided in #7189 §5): Settings › Diagnostics'
//! switch that lets the steps of a remote start (`purlis::step`) through to the diagnostic log at
//! `debug`, every `docker` call among them, and turns itself off an hour after it was turned on.
//!
//! **Only the steps.** The rest of purlis, and every library, is kept as the log always keeps it
//! ([`crate::applog`]). **Never ssh's own verbose output**: the forward is spawned with
//! `LogLevel=ERROR` whatever this says, and nothing here reaches ssh's arguments.
//!
//! **No store.** The switch is this process's alone, held in memory: purlis that starts again
//! starts with it off. Nothing leaves the machine (ADR 0075): the lines go to the 7-day local log.
//!
//! The clock is the caller's: [`Switch::turn_on`] and [`Switch::at`] are handed the time, so a
//! test turns the hour over without waiting it out, and the app asks again when the hour is up.

use std::sync::{Mutex, OnceLock};

use jiff::{SignedDuration, Timestamp};
use tracing::Level;
use tracing_subscriber::filter::Targets;

/// How long the switch stays on once it is turned on.
pub const LASTS: SignedDuration = SignedDuration::from_hours(1);

/// What the switch says: on until `until`, or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detailed {
    /// When it turns itself off, while it is on.
    pub until: Option<Timestamp>,
}

/// The switch, and what it changes as it turns on and off.
pub struct Switch {
    until: Mutex<Option<Timestamp>>,
    apply: Box<dyn Fn(bool) + Send + Sync>,
}

impl Switch {
    /// A switch that is off, which calls `apply` with whether the steps' `debug` lines are let
    /// through each time that changes.
    pub fn new(apply: Box<dyn Fn(bool) + Send + Sync>) -> Switch {
        Switch {
            until: Mutex::new(None),
            apply,
        }
    }

    fn until(&self) -> std::sync::MutexGuard<'_, Option<Timestamp>> {
        self.until
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Turns it on at `now`, for [`LASTS`] from `now`: turned on again, it lasts an hour from then.
    pub fn turn_on(&self, now: Timestamp) -> Detailed {
        let until = now + LASTS;
        let was = self.until().replace(until);
        if was.is_none() {
            (self.apply)(true);
        }
        Detailed { until: Some(until) }
    }

    /// Turns it off now.
    pub fn turn_off(&self) -> Detailed {
        if self.until().take().is_some() {
            (self.apply)(false);
        }
        Detailed { until: None }
    }

    /// What it says at `now`: off once its hour is up, which turns it off.
    pub fn at(&self, now: Timestamp) -> Detailed {
        let mut until = self.until();
        if until.is_some_and(|until| now >= until) {
            *until = None;
            drop(until);
            (self.apply)(false);
            return Detailed { until: None };
        }
        Detailed { until: *until }
    }
}

/// This process's switch, which changes the diagnostic log's filter ([`crate::applog`]).
pub fn this_process() -> &'static Switch {
    static SWITCH: OnceLock<Switch> = OnceLock::new();
    SWITCH.get_or_init(|| Switch::new(Box::new(crate::applog::keep_step_detail)))
}

/// What the diagnostic log keeps, with the steps' `debug` lines where `on`.
pub(crate) fn kept(on: bool) -> Targets {
    let kept = crate::applog::what_a_log_keeps();
    if on {
        kept.with_target(purlis_session_protocol::steps::TARGET, Level::DEBUG)
    } else {
        kept
    }
}

#[cfg(test)]
mod tests;
