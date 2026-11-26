// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each phase of opening a new chat takes** (H12): the phases a start passes
//! through before its program runs, each named and timed, so a report of "a new chat takes
//! minutes" names the phase.
//!
//! A start runs on one blocking thread, so the phases are kept in a thread-local list rather
//! than threaded through every signature: [`recording`] opens the list for the closure it
//! runs, [`phase`] adds one to the list that is open (and costs nothing when none is), and
//! [`recording`] writes one line to the slow log ([`crate::slowlog::Slow::Opening`]) when the
//! whole took [`SLOW_MS`] or longer.
//!
//! **Never a chat's content**: a phase is a name from this file's callers, and its milliseconds.

use std::cell::RefCell;
use std::time::Instant;

use crate::slowlog::{Slow, Took};

/// An opening shorter than this is not written to the slow log.
pub const SLOW_MS: u32 = 1_000;

thread_local! {
    static OPEN: RefCell<Option<Vec<Took>>> = const { RefCell::new(None) };
}

fn millis(since: Instant) -> u32 {
    u32::try_from(since.elapsed().as_millis()).unwrap_or(u32::MAX)
}

/// Runs `work` as the phase `name` of the opening that is being [`recording`] on this thread,
/// adding its milliseconds to that opening's list. Where none is open, it only runs `work`.
pub fn phase<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let began = Instant::now();
    let out = work();
    let ms = millis(began);
    OPEN.with(|open| {
        if let Some(phases) = open.borrow_mut().as_mut() {
            phases.push(Took {
                name: name.to_owned(),
                ms,
            });
        }
    });
    out
}

/// The opening `work` is, with the phases it passed through, and its whole in milliseconds.
pub struct Timed<T> {
    pub out: T,
    pub ms: u32,
    pub phases: Vec<Took>,
}

/// Runs `work`, keeping the phases it passes through on this thread.
pub fn timed<T>(work: impl FnOnce() -> T) -> Timed<T> {
    let before = OPEN.with(|open| open.borrow_mut().replace(Vec::new()));
    let began = Instant::now();
    let out = work();
    let ms = millis(began);
    let phases = OPEN.with(|open| std::mem::replace(&mut *open.borrow_mut(), before));
    Timed {
        out,
        ms,
        phases: phases.unwrap_or_default(),
    }
}

/// [`timed`], and a line in the slow log when the opening took [`SLOW_MS`] or longer, with
/// `hot` chats open beside it. A log that cannot be written costs the line and nothing else.
pub fn recording<T>(hot: u32, work: impl FnOnce() -> T) -> T {
    let Timed { out, ms, phases } = timed(work);
    if ms >= SLOW_MS {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
        let _ = crate::slowlog::record(&Slow::Opening {
            ms,
            at,
            hot,
            phases,
        });
    }
    out
}

#[cfg(test)]
mod tests;
