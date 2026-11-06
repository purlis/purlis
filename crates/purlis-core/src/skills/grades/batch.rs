// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Several skills graded under one limit** (#5356, the operator's ruling of 2026-11-05): the
//! Library's Select mode grades the skills chosen a few at a time, each a grade of its own as a
//! press of Grade starts it, and the person's dollar limit holds them all.
//!
//! A [`Batch`] decides, and its host does: the host asks [`Batch::start_next`] which grade to start,
//! tells it what each grade has spent so far ([`Batch::spent`]) and when one ends
//! ([`Batch::ended`]). Once what they spent together reaches the limit, no grade starts and those
//! running are to be stopped, which scores and records what they ran: **a grade that ended is
//! kept**, whatever stopped the batch. A chat that did not say what it spent stops it too, as a
//! limit cannot hold what it is not told. The figures are the grades' own (`super::Spent`), so a
//! batch goes past its limit by no more than what the chats running at that moment spend, as a
//! grade goes past its own.
//!
//! It holds no clock, file or thread: the host's are the window's (`purlis-app`'s
//! `skillgrade::batch`).

use serde::Serialize;

/// **Where one skill of a batch is.**
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum State {
    /// Not started yet.
    Waiting,
    /// Its grade runs.
    Running,
    /// Its grade ended, and was recorded as a grade ends: its host says how.
    Ended,
    /// It was never graded, and why.
    NotRun { why: NotRun },
}

/// **Why a skill of a batch was never graded.**
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "why", content = "said", rename_all = "kebab-case")]
pub enum NotRun {
    /// The limit was reached first.
    Limit,
    /// The person stopped the batch first.
    Stopped,
    /// A chat of another grade did not say what it spent, so the limit could not hold.
    SpendNotReported,
    /// Its grade was refused as it started, with the host's sentence.
    Refused(String),
}

/// **How a batch ended**, once nothing in it runs and nothing more will start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ending {
    /// Every skill was started.
    Whole,
    /// What its grades spent reached its limit.
    Limit,
    /// The person stopped it.
    Stopped,
    /// A chat did not say what it spent.
    SpendNotReported,
}

/// **Several grades under one limit**: see the module.
#[derive(Debug, Clone)]
pub struct Batch {
    limit_usd: f64,
    at_once: usize,
    states: Vec<State>,
    /// What each grade has spent so far, in dollars, as its latest step said.
    spent: Vec<f64>,
    /// Why no more starts, once something said so.
    stopping: Option<Ending>,
}

impl Batch {
    /// A batch of `count` skills, in the order they are to start, held to `limit_usd`, with at
    /// most `at_once` of them running together (at least one).
    pub fn new(count: usize, limit_usd: f64, at_once: usize) -> Self {
        Self {
            limit_usd,
            at_once: at_once.max(1),
            states: vec![State::Waiting; count],
            spent: vec![0.0; count],
            stopping: None,
        }
    }

    /// **The next skill to start**, by its place, marked running; `None` while as many run as
    /// may, or once nothing more starts.
    pub fn start_next(&mut self) -> Option<usize> {
        if self.stopping.is_some() || self.running() >= self.at_once {
            return None;
        }
        let at = self.states.iter().position(|one| *one == State::Waiting)?;
        self.states[at] = State::Running;
        Some(at)
    }

    /// The grade at `at` was refused as it started: it is named with `why`, and spent nothing.
    pub fn refused(&mut self, at: usize, why: String) {
        if let Some(state) = self.states.get_mut(at) {
            *state = State::NotRun {
                why: NotRun::Refused(why),
            };
        }
    }

    /// **The grade at `at` has run `chats` chats and spent `cost_usd` so far**, `None` where a
    /// chat did not say. `true` where the grades running are to be stopped now: the limit is
    /// reached, or it cannot be held.
    pub fn spent(&mut self, at: usize, chats: u32, cost_usd: Option<f64>) -> bool {
        if self.states.get(at) != Some(&State::Running) {
            return false;
        }
        match cost_usd {
            Some(cost) => self.spent[at] = cost.max(0.0),
            None if chats > 0 => return self.stop_for(Ending::SpendNotReported),
            None => {}
        }
        self.over() && self.stop_for(Ending::Limit)
    }

    /// **The grade at `at` ended**, having spent `cost_usd` in all: it is kept, and its place
    /// may go to the next.
    pub fn ended(&mut self, at: usize, cost_usd: f64) {
        let Some(state) = self.states.get_mut(at) else {
            return;
        };
        *state = State::Ended;
        self.spent[at] = cost_usd.max(0.0);
        if self.over() {
            self.stop_for(Ending::Limit);
        }
    }

    /// **The person stopped the batch**: none waiting starts, and those running are to be
    /// stopped.
    pub fn stop(&mut self) {
        self.stop_for(Ending::Stopped);
    }

    /// What its grades spent together so far, in dollars.
    pub fn spent_usd(&self) -> f64 {
        self.spent.iter().sum()
    }

    /// The limit it is held to, in dollars.
    pub fn limit_usd(&self) -> f64 {
        self.limit_usd
    }

    /// Where each skill is, in the batch's order.
    pub fn states(&self) -> &[State] {
        &self.states
    }

    /// **How it ended**, once nothing runs and nothing more starts; `None` before.
    pub fn ending(&self) -> Option<Ending> {
        if self.running() > 0 {
            return None;
        }
        match self.stopping {
            Some(ending) => Some(ending),
            None if self.states.contains(&State::Waiting) => None,
            None => Some(Ending::Whole),
        }
    }

    fn running(&self) -> usize {
        self.states
            .iter()
            .filter(|one| **one == State::Running)
            .count()
    }

    fn over(&self) -> bool {
        self.spent_usd() >= self.limit_usd
    }

    /// Nothing more starts, for `why` (the first reason holds); `true` where it was not
    /// stopping before. Those waiting are named with it.
    fn stop_for(&mut self, why: Ending) -> bool {
        if self.stopping.is_some() {
            return false;
        }
        self.stopping = Some(why);
        let not_run = match why {
            Ending::Limit => NotRun::Limit,
            Ending::Stopped => NotRun::Stopped,
            Ending::SpendNotReported => NotRun::SpendNotReported,
            Ending::Whole => return true,
        };
        for state in &mut self.states {
            if *state == State::Waiting {
                *state = State::NotRun {
                    why: not_run.clone(),
                };
            }
        }
        true
    }
}

/// **What a batch should spend, before anything runs**: the estimates known, added, and how many
/// skills have none to go on (no grade before, or one whose chats did not say).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Estimate {
    pub known_usd: f64,
    pub known: u32,
    pub unknown: u32,
}

/// The [`Estimate`] of grades whose own estimates, in dollars, are `each`.
pub fn estimate(each: &[Option<f64>]) -> Estimate {
    let known: Vec<f64> = each.iter().flatten().copied().collect();
    Estimate {
        known_usd: known.iter().sum(),
        known: known.len() as u32,
        unknown: (each.len() - known.len()) as u32,
    }
}

#[cfg(test)]
mod tests;
