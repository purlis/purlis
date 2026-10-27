// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A read of a page's whole tree that goes on past the call it was made for** (#3043, #3277).
//!
//! A large page's tree takes long to read (17 to 46 s on macOS real Chrome for the 20,000-row
//! page of #2295), and a DevTools call whose caller stopped waiting still runs inside the
//! engine: a read made anew for the next call would queue behind it and, on a busy machine,
//! might never finish. So the read runs on a thread of its own, each frame within its own limit,
//! and whoever needs the page next waits on that one read rather than asking for another. The
//! person's keys keep one ([`super::takeover`]), the agent's calls another
//! ([`Browser::agent_trees`](super::Browser)): a read begun under one lease is never used under
//! another.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::super::tree::FrameAx;
use super::{DevTools, ToRead, read_trees, refusal};
use crate::cdp::CallError;
use crate::drive::{Refusal, lock};

/// How often a wait on a read wakes to ask whether its call was cut.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// What a read of a page's trees came to.
pub(super) type ReadAnswer = Result<Arc<Vec<FrameAx>>, Refusal>;

/// A read of every frame's tree of a page: its page's target, the lease's epoch and the page's
/// document number when it began, and its answer once it came.
pub(super) struct Reading {
    pub(super) target: String,
    pub(super) epoch: u64,
    pub(super) shows: u64,
    /// When it began.
    pub(super) began: Instant,
    /// The answer, and when it came.
    answer: Mutex<Option<(Instant, ReadAnswer)>>,
    came: Condvar,
}

impl Reading {
    pub(super) fn new(target: &str, epoch: u64, shows: u64) -> Arc<Self> {
        Arc::new(Self {
            target: target.to_owned(),
            epoch,
            shows,
            began: Instant::now(),
            answer: Mutex::new(None),
            came: Condvar::new(),
        })
    }

    /// Whether it is a read of `target`'s page under `epoch` while it shows document `shows`.
    pub(super) fn is_of(&self, target: &str, epoch: u64, shows: u64) -> bool {
        self.target == target && self.epoch == epoch && self.shows == shows
    }

    /// The read's answer is `answer`: every wait on it is told.
    pub(super) fn answered(&self, answer: ReadAnswer) {
        *lock(&self.answer) = Some((Instant::now(), answer));
        self.came.notify_all();
    }

    /// When the read came, where it has.
    pub(super) fn came(&self) -> Option<Instant> {
        lock(&self.answer).as_ref().map(|(came, _)| *came)
    }

    /// The read's answer and when it came, waited for until `by`; `None` where it has not come
    /// by then, or `cut` says the wait is over first.
    pub(super) fn wait(
        &self,
        by: Instant,
        cut: impl Fn() -> bool,
    ) -> Option<(Instant, ReadAnswer)> {
        let mut answer = lock(&self.answer);
        loop {
            if let Some((came, answer)) = answer.as_ref() {
                return Some((*came, answer.clone()));
            }
            let left = by.saturating_duration_since(Instant::now());
            if left.is_zero() || cut() {
                return None;
            }
            answer = self
                .came
                .wait_timeout(answer, left.min(LOOK_EVERY))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Begins reading `frames` through `devtools` on a thread of its own, each frame's tree
    /// within `within`, holding at most `most` bytes of the engine's answers in all. A read past
    /// that limit, or one that cannot begin, answers `slow`.
    pub(super) fn begin(
        self: &Arc<Self>,
        devtools: Arc<dyn DevTools>,
        frames: Result<Vec<ToRead>, Refusal>,
        within: Duration,
        most: usize,
        slow: Refusal,
    ) {
        let frames = match frames {
            Ok(frames) => frames,
            Err(why) => return self.answered(Err(why)),
        };
        let told = Arc::clone(self);
        let timed_out = slow.clone();
        let begun = std::thread::Builder::new()
            .name("purlis-tree-read".into())
            .spawn(move || {
                told.answered(
                    read_trees(&*devtools, frames, within, most)
                        .map(Arc::new)
                        .map_err(|why| match why {
                            CallError::TimedOut => timed_out,
                            other => refusal(other),
                        }),
                );
            });
        if begun.is_err() {
            self.answered(Err(slow));
        }
    }
}

/// **Where a read is kept for whoever needs the page next** (#3043, #3277): the read kept for
/// the next caller, and the read the engine is still asked for, which may be one no caller will
/// use any more (an act, or a key that may move the focus, left it). A slot asks the engine for
/// one whole-tree read at a time: a read is begun only once the one before it has come.
#[derive(Default)]
pub(super) struct Slot {
    kept: Option<Arc<Reading>>,
    running: Option<Arc<Reading>>,
}

/// What a caller that needs the page does, as its slot says.
pub(super) enum Next {
    /// Waits on this read, begun by an earlier caller.
    Join(Arc<Reading>),
    /// Waits on this read, which it begins now.
    Begin(Arc<Reading>),
    /// Waits for this read, no longer kept for anyone, to end before it asks for another.
    After(Arc<Reading>),
}

impl Slot {
    /// What a caller that needs `target`'s page under `epoch`, while it shows `shows`, does:
    /// join the kept read where `usable` says it may, else wait for a read still running to
    /// end, else begin a new one, kept from now.
    pub(super) fn next(
        &mut self,
        target: &str,
        epoch: u64,
        shows: u64,
        usable: impl Fn(&Reading) -> bool,
    ) -> Next {
        if let Some(kept) = &self.kept {
            if kept.is_of(target, epoch, shows) && usable(kept) {
                return Next::Join(Arc::clone(kept));
            }
            // Not to be used: kept for no one, whether it comes later or not.
            self.kept = None;
        }
        if let Some(running) = &self.running
            && running.came().is_none()
        {
            return Next::After(Arc::clone(running));
        }
        let reading = Reading::new(target, epoch, shows);
        self.kept = Some(Arc::clone(&reading));
        self.running = Some(Arc::clone(&reading));
        Next::Begin(reading)
    }

    /// Keeps no read for any later caller; one still running goes on to its end.
    pub(super) fn forget(&mut self) {
        self.kept = None;
    }

    /// `reading` was used, or failed: the next caller reads anew.
    pub(super) fn used(&mut self, reading: &Arc<Reading>) {
        if self
            .kept
            .as_ref()
            .is_some_and(|kept| Arc::ptr_eq(kept, reading))
        {
            self.kept = None;
        }
    }

    /// Whether a read is kept for a later caller.
    #[cfg(test)]
    pub(super) fn keeps(&self) -> bool {
        self.kept.is_some()
    }
}
