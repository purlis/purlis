// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Stuck? is a question** (ADR 0121 decision 19, ADR 0076 N8, #6014): whether a working chat
//! may not be getting anywhere. Three rules, each a setting (`crate::dispatchlimits`):
//!
//! - **Silent**: no new command or output for `stuck-silent-minutes` (15 by default).
//! - **Failing**: the same command failing `stuck-failing-times` times in a row (3).
//! - **Long**: one turn running longer than `stuck-long-minutes` (60).
//!
//! **What it hears is what already flows** (ADR 0076): the chat's state as the board moves it
//! (working or not), a tool of its own beginning or coming back and what its turn put out, and
//! each command it ran with whether it failed, as its harness's hooks said. Never its
//! terminal's bytes, which a spinner redraws whatever happens.
//!
//! **"It's fine" clears it until the chat changes state** ([`Watch::fine`]): the next time the
//! board moves it into or out of working. A chat that is not working is never stuck.
//!
//! **A question, never an act.** Nothing here stops, pauses or tells anything; the app shows
//! the answer on the chat's mark and as an update. Plain Rust with no clock of its own: the app
//! passes the time, in seconds, and asks [`Watch::due`] when next to look, so a timer per chat
//! wakes only when a rule of time can come true.

use std::collections::HashMap;

/// The three thresholds in force for one chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub silent_minutes: u32,
    pub failing_times: u32,
    pub long_minutes: u32,
}

impl Limits {
    /// The thresholds as `limits` resolve them for a chat.
    pub fn of(limits: &crate::dispatchlimits::Limits) -> Self {
        Self {
            silent_minutes: limits.stuck_silent_minutes,
            failing_times: limits.stuck_failing_times,
            long_minutes: limits.stuck_long_minutes,
        }
    }

    fn silent(&self) -> i64 {
        i64::from(self.silent_minutes) * 60
    }

    fn long(&self) -> i64 {
        i64::from(self.long_minutes) * 60
    }
}

/// Why a chat reads Stuck?, the first true of these: a command that keeps failing says the
/// most, then a turn that runs long, then silence. Each says the threshold it passed, so what
/// a chat is said to be stuck for stays the same while it stays stuck.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// No new command or output for at least this many minutes.
    Silent { minutes: u32 },
    /// The same command failed at least this many times in a row.
    Failing { times: u32 },
    /// One turn has run at least this many minutes.
    Long { minutes: u32 },
}

impl Why {
    /// What it says, in purlis's words, after the chat's name.
    pub fn said(&self) -> String {
        match self {
            Self::Silent { minutes } => {
                format!("no new command or output for {}", minutes_said(*minutes))
            }
            Self::Failing { times } => format!("the same command failed {times} times in a row"),
            Self::Long { minutes } => format!("one turn running for {}", minutes_said(*minutes)),
        }
    }
}

fn minutes_said(minutes: u32) -> String {
    if minutes == 1 {
        "1 minute".to_owned()
    } else {
        format!("{minutes} minutes")
    }
}

/// What has been heard of one chat in its current state.
#[derive(Debug, Clone, Default)]
struct Heard {
    /// When its turn began, while it works.
    working_since: Option<i64>,
    /// When a new command or output was last heard.
    last: i64,
    /// The command that failed last, and how many times in a row.
    failing: Option<(String, u32)>,
    /// The person said "It's fine" in this state.
    fine: bool,
}

/// What has been heard of each chat of one project, in memory.
#[derive(Debug, Default)]
pub struct Watch {
    chats: HashMap<u32, Heard>,
}

impl Watch {
    /// Chat `chat` is `working` or not, as the board moved it, at `now`. A change of state
    /// starts it afresh: a new turn's clocks and count, and "It's fine" let go. Answers whether
    /// it changed, which the first state told of a chat is.
    pub fn state(&mut self, chat: u32, working: bool, now: i64) -> bool {
        let first = !self.chats.contains_key(&chat);
        let heard = self.chats.entry(chat).or_default();
        if !first && heard.working_since.is_some() == working {
            return false;
        }
        *heard = Heard {
            working_since: working.then_some(now),
            last: now,
            failing: None,
            fine: false,
        };
        true
    }

    /// When chat `chat`'s current turn began, while it works: the turn a timer is for.
    pub fn working_since(&self, chat: u32) -> Option<i64> {
        self.chats.get(&chat)?.working_since
    }

    /// Chat `chat` began or got back a command, or its turn put something out, at `now`.
    pub fn heard(&mut self, chat: u32, now: i64) {
        if let Some(heard) = self.chats.get_mut(&chat) {
            heard.last = heard.last.max(now);
        }
    }

    /// Chat `chat` ran `command`, which `failed` or not, at `now`. Its failures in a row are
    /// counted while it is the same command; another command, or a success, starts again.
    pub fn ran(&mut self, chat: u32, command: &str, failed: bool, now: i64) {
        self.heard(chat, now);
        let Some(heard) = self.chats.get_mut(&chat) else {
            return;
        };
        let command = command.trim();
        heard.failing = match heard.failing.take() {
            _ if !failed => None,
            Some((same, times)) if same == command => Some((same, times.saturating_add(1))),
            _ => Some((command.to_owned(), 1)),
        };
    }

    /// **"It's fine"**: chat `chat` is not stuck until it changes state.
    pub fn fine(&mut self, chat: u32) {
        if let Some(heard) = self.chats.get_mut(&chat) {
            heard.fine = true;
        }
    }

    /// Chat `chat` is gone: nothing is kept of it.
    pub fn forget(&mut self, chat: u32) {
        self.chats.remove(&chat);
    }

    /// **Why chat `chat` reads Stuck? at `now`**, held to `limits`, or nothing.
    pub fn stuck(&self, chat: u32, now: i64, limits: &Limits) -> Option<Why> {
        let heard = self.chats.get(&chat)?;
        let since = heard.working_since?;
        if heard.fine {
            return None;
        }
        if let Some((_, times)) = &heard.failing
            && *times >= limits.failing_times
        {
            return Some(Why::Failing {
                times: limits.failing_times,
            });
        }
        if now - since >= limits.long() {
            return Some(Why::Long {
                minutes: limits.long_minutes,
            });
        }
        (now - heard.last >= limits.silent()).then_some(Why::Silent {
            minutes: limits.silent_minutes,
        })
    }

    /// **When next a rule of time can come true for chat `chat`**, after `now`: what a timer
    /// sleeps until. Nothing where it is not working, the person said it is fine, or both rules
    /// of time have come true already.
    pub fn due(&self, chat: u32, now: i64, limits: &Limits) -> Option<i64> {
        let heard = self.chats.get(&chat)?;
        let since = heard.working_since?;
        if heard.fine {
            return None;
        }
        [heard.last + limits.silent(), since + limits.long()]
            .into_iter()
            .filter(|at| *at > now)
            .min()
    }
}

#[cfg(test)]
mod tests;
