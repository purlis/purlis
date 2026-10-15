// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every call carries an id, and a retry never acts twice.
//!
//! The ledger keeps the last answers by id. A call whose id it has answered gets that answer
//! again and acts on nothing; one whose id is still running waits for the first to answer; one
//! whose id was given to a different call is refused. A call that ends without an answer (its
//! thread ended early) leaves its id free, because it reached no answer anyone was given.

use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};

use super::CallId;

/// What a call's id says about it.
pub(crate) enum Begin<'l, C: PartialEq + Clone, A: Clone> {
    /// Never seen: act, then [`Ticket::finish`] with the answer.
    Fresh(Ticket<'l, C, A>),
    /// Already answered (or answered while this waited): this is the answer, and nothing acts.
    Repeat(A),
    /// The id belongs to another call.
    Reused,
}

enum Entry<C, A> {
    Running(C),
    Done(C, A),
}

struct Inner<C, A> {
    entries: HashMap<CallId, Entry<C, A>>,
    /// Answered ids, oldest first, for forgetting past the capacity.
    answered: VecDeque<CallId>,
}

/// The last answers, by call id.
pub(crate) struct Ledger<C, A> {
    inner: Mutex<Inner<C, A>>,
    changed: Condvar,
    capacity: usize,
}

impl<C: PartialEq + Clone, A: Clone> Ledger<C, A> {
    /// A ledger that remembers the latest `capacity` answers.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                entries: HashMap::new(),
                answered: VecDeque::new(),
            }),
            changed: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    /// What `id` says about `call`; waits while another call with the same id runs.
    pub(crate) fn begin(&self, id: &CallId, call: &C) -> Begin<'_, C, A> {
        let mut inner = super::lock(&self.inner);
        loop {
            match inner.entries.get(id) {
                None => {
                    inner
                        .entries
                        .insert(id.clone(), Entry::Running(call.clone()));
                    return Begin::Fresh(Ticket {
                        ledger: self,
                        id: id.clone(),
                        finished: false,
                    });
                }
                Some(Entry::Running(running)) if running != call => return Begin::Reused,
                Some(Entry::Done(done, _)) if done != call => return Begin::Reused,
                Some(Entry::Done(_, answer)) => return Begin::Repeat(answer.clone()),
                Some(Entry::Running(_)) => {
                    inner = self
                        .changed
                        .wait(inner)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            }
        }
    }

    fn finish(&self, id: &CallId, answer: A) {
        let mut inner = super::lock(&self.inner);
        if let Some(Entry::Running(call)) = inner.entries.remove(id) {
            inner.entries.insert(id.clone(), Entry::Done(call, answer));
            inner.answered.push_back(id.clone());
            while inner.answered.len() > self.capacity {
                if let Some(old) = inner.answered.pop_front() {
                    inner.entries.remove(&old);
                }
            }
        }
        self.changed.notify_all();
    }

    fn abandon(&self, id: &CallId) {
        let mut inner = super::lock(&self.inner);
        if matches!(inner.entries.get(id), Some(Entry::Running(_))) {
            inner.entries.remove(id);
        }
        self.changed.notify_all();
    }
}

/// A fresh call's place in the ledger. Dropped without an answer, it frees the id.
pub(crate) struct Ticket<'l, C: PartialEq + Clone, A: Clone> {
    ledger: &'l Ledger<C, A>,
    id: CallId,
    finished: bool,
}

impl<C: PartialEq + Clone, A: Clone> Ticket<'_, C, A> {
    /// Keeps `answer` as the answer to this call's id.
    pub(crate) fn finish(mut self, answer: A) {
        self.finished = true;
        self.ledger.finish(&self.id, answer);
    }
}

impl<C: PartialEq + Clone, A: Clone> Drop for Ticket<'_, C, A> {
    fn drop(&mut self) {
        if !self.finished {
            self.ledger.abandon(&self.id);
        }
    }
}

#[cfg(test)]
mod tests;
