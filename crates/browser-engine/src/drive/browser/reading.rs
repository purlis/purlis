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
    /// within `within`. A read past that limit, or one that cannot begin, answers `slow`.
    pub(super) fn begin(
        self: &Arc<Self>,
        devtools: Arc<dyn DevTools>,
        frames: Result<Vec<ToRead>, Refusal>,
        within: Duration,
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
                    read_trees(&*devtools, frames, within)
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
