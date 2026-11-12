// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each phase of a chat put back at a launch took** (#4988): one line in the app's
//! log per start, and one more when its harness first reports.
//!
//! The wait for a place at the restore gate is its own field, measured from the start's own
//! ask, and not counted in how long the start took (felt speed round 2, H4). Measured from the
//! launch, as it was, it read as a slow start where it was only a place in the queue.
//!
//! A launch that takes minutes to bring its chats back said only "N of M chats back" per
//! project, once every start had been tried, so nobody could tell which phase held them. A
//! start is timed on the thread that makes it ([`timed`]), and each phase it passes marks the
//! clock ([`mark`]): a mark on a thread with no clock does nothing, so a start that is not a
//! put-back costs nothing and says nothing.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The phases a start passes, in the order it passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// The core read what it starts as: its profile, persona and plan (`purlis_core::start`).
    Ready,
    /// What its sandbox needs beside it, the egress proxy among them, is running.
    Proxy,
    /// Its sandboxed line is composed, the project's package caches made with it.
    Line,
    /// The session host (or the runner) answered that its program runs.
    Host,
}

impl Phase {
    fn word(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Proxy => "proxy",
            Self::Line => "line",
            Self::Host => "host",
        }
    }
}

/// One start's clock: when it was asked for, when the restore gate let it go ahead
/// (`crate::restoring`), and when it passed each phase.
#[derive(Debug)]
pub(crate) struct Clock {
    asked: Instant,
    placed: Instant,
    passed: Vec<(Phase, Instant)>,
}

impl Clock {
    pub(crate) fn asked_at(asked: Instant, placed: Instant) -> Self {
        Self {
            asked,
            placed,
            passed: Vec::new(),
        }
    }

    pub(crate) fn passed(&mut self, phase: Phase, at: Instant) {
        self.passed.push((phase, at));
    }

    /// The line the log is told: the whole start from its place at the gate, the wait for that
    /// place, and each phase with the time since the one before it, in milliseconds. `outcome`
    /// is the chat's number, or why it did not start.
    pub(crate) fn line(&self, name: &str, outcome: &Result<u32, String>, ended: Instant) -> String {
        let mut since = self.placed;
        let phases: Vec<String> = self
            .passed
            .iter()
            .map(|(phase, at)| {
                let took = ms(at.saturating_duration_since(since));
                since = *at;
                format!("{} +{took}", phase.word())
            })
            .collect();
        let total = ms(ended.saturating_duration_since(self.placed));
        let gate = ms(self.placed.saturating_duration_since(self.asked));
        let ended = match outcome {
            Ok(session) => format!("live as chat {session}"),
            Err(_) => "did not start".to_owned(),
        };
        format!(
            "purlis: start of {name}: {total} ms after {gate} ms at the gate ({}); {ended}",
            if phases.is_empty() {
                "no phase passed".to_owned()
            } else {
                phases.join(", ")
            }
        )
    }
}

fn ms(took: Duration) -> u128 {
    took.as_millis()
}

thread_local! {
    static CLOCK: RefCell<Option<Clock>> = const { RefCell::new(None) };
}

/// Runs `start` with a clock on this thread, asked for at `asked` and let through the gate at
/// `placed`, and logs its line once it answers. A chat that started is remembered until its
/// harness first reports ([`heard`]).
pub(crate) fn timed(
    name: &str,
    asked: Instant,
    placed: Instant,
    start: impl FnOnce() -> Result<u32, String>,
) -> Result<u32, String> {
    CLOCK.set(Some(Clock::asked_at(asked, placed)));
    let started = start();
    let ended = Instant::now();
    if let Some(clock) = CLOCK.take() {
        tracing::info!("{}", clock.line(name, &started, ended));
        if let Ok(session) = started {
            waiting().asked(session, clock.asked);
        }
    }
    started
}

/// Marks `phase` passed now on this thread's clock, where it has one.
pub(crate) fn mark(phase: Phase) {
    CLOCK.with_borrow_mut(|clock| {
        if let Some(clock) = clock {
            clock.passed(phase, Instant::now());
        }
    });
}

/// The chats a timed start brought up whose harness has not reported yet.
#[derive(Debug, Default)]
pub(crate) struct Waiting {
    asked: HashMap<u32, Instant>,
}

/// More chats than a launch brings back: past it, the oldest are dropped (a chat whose harness
/// never reports is never said to have).
const HELD: usize = 512;

impl Waiting {
    pub(crate) fn asked(&mut self, session: u32, at: Instant) {
        if self.asked.len() >= HELD
            && let Some(oldest) = self
                .asked
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(session, _)| *session)
        {
            self.asked.remove(&oldest);
        }
        self.asked.insert(session, at);
    }

    /// How long after its start was asked chat `session`'s harness first reported: once per
    /// timed start, and never for a chat no timed start brought up.
    pub(crate) fn heard(&mut self, session: u32, at: Instant) -> Option<Duration> {
        self.asked
            .remove(&session)
            .map(|asked| at.saturating_duration_since(asked))
    }
}

static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);

fn waiting() -> WaitingGuard {
    WaitingGuard(WAITING.lock().unwrap_or_else(PoisonError::into_inner))
}

struct WaitingGuard(std::sync::MutexGuard<'static, Option<Waiting>>);

impl WaitingGuard {
    fn asked(&mut self, session: u32, at: Instant) {
        self.0
            .get_or_insert_with(Waiting::default)
            .asked(session, at);
    }

    fn heard(&mut self, session: u32, at: Instant) -> Option<Duration> {
        self.0.as_mut()?.heard(session, at)
    }
}

/// Chat `session`'s harness reported: the first report after a timed start says how long the
/// chat took to come alive.
pub(crate) fn heard(session: u32) {
    if let Some(took) = waiting().heard(session, Instant::now()) {
        tracing::info!(
            "purlis: chat {session}'s harness first reported {} ms after its start was asked",
            ms(took)
        );
    }
}

#[cfg(test)]
mod tests;
