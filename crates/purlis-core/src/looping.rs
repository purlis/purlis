// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Looping is a pause** (ADR 0121 decision 20, ADR 0076 N8, #6015): whether a chat is going
//! round and round, so the app pauses it and asks the person "It's fine, continue" or "Stop".
//! Two rules:
//!
//! - **Failing**: the same command failing again and again, Stuck?'s rule
//!   ([`crate::stuck::Failing`]) at twice its threshold ([`LOOPING_IS_STUCK_TIMES`]).
//! - **Refused**: the person refusing its asks again and again, with no allow between, at the
//!   same threshold.
//!
//! **What it hears is what already flows** (ADR 0076): each command a chat ran and whether it
//! failed, as its harness's hooks said, and each answer the person gave its asks. Never its
//! terminal's bytes. The counts are not reset by the chat's state moving, as Stuck?'s are: a
//! permission prompt moves a chat out of working and back, and a refusal comes on just such a
//! prompt.
//!
//! **Told once per loop** ([`Loops::ran`], [`Loops::answered`]): the answer is `Some` only as a
//! count reaches its threshold, which is when the app pauses the chat. "It's fine, continue"
//! ([`Loops::fine`]) starts every count again, so a loop that goes on is told afresh. Plain
//! Rust with no clock: nothing here is a rule of time.

use std::collections::HashMap;

use crate::stuck::Failing;

/// **How many times what reads Stuck? a loop is**: a chat reads Stuck? at the same command's
/// `stuck-failing-times` failures, and is paused as looping at this many times that. One setting
/// for both, so a person who raises one raises the other.
pub const LOOPING_IS_STUCK_TIMES: u32 = 2;

/// **How many blocks on the same target are a loop** (R-206, Q133): three.
pub const BLOCKED_TIMES: u32 = 3;

/// The two thresholds in force for one chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The same command's failures in a row that are a loop.
    pub failing_times: u32,
    /// The person's refusals of its asks in a row that are a loop.
    pub refused_times: u32,
}

impl Limits {
    /// The thresholds Stuck?'s `stuck` imply: [`LOOPING_IS_STUCK_TIMES`] its failures, for each.
    pub fn of(stuck: &crate::stuck::Limits) -> Self {
        let times = stuck
            .failing_times
            .saturating_mul(LOOPING_IS_STUCK_TIMES)
            .max(2);
        Self {
            failing_times: times,
            refused_times: times,
        }
    }
}

/// Why a chat is looping. Each says the threshold it passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The same command failed this many times in a row.
    Failing { times: u32 },
    /// The person refused this many of its asks in a row.
    Refused { times: u32 },
    /// It was blocked this many times on the same target.
    Blocked { times: u32 },
}

impl Why {
    /// What it says, in purlis's words, after the chat's name.
    pub fn said(&self) -> String {
        match self {
            Self::Failing { times } => format!("the same command failed {times} times in a row"),
            Self::Refused { times } => format!("{times} of its asks were refused in a row"),
            Self::Blocked { times } => format!("it was blocked {times} times on the same thing"),
        }
    }
}

/// What has been counted of one chat.
#[derive(Debug, Clone, Default)]
struct Counted {
    failing: Failing,
    refused: u32,
    /// The target of its latest blocks, and how many in a row.
    blocked: Option<(Aim, u32)>,
}

/// **What a block was aimed at** (R-206): the tool plus its argument digest.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aim {
    tool: String,
    args: Option<String>,
}

/// What has been counted of each chat of one project, in memory.
#[derive(Debug, Default)]
pub struct Loops {
    chats: HashMap<u32, Counted>,
}

impl Loops {
    /// Chat `chat` ran `command`, which `failed` or not: whether that makes it a loop, held to
    /// `limits`, told only as the count reaches it.
    pub fn ran(&mut self, chat: u32, command: &str, failed: bool, limits: &Limits) -> Option<Why> {
        let times = self
            .chats
            .entry(chat)
            .or_default()
            .failing
            .ran(command, failed);
        (times == limits.failing_times).then_some(Why::Failing { times })
    }

    /// The person answered one of chat `chat`'s asks, and `refused` it or not: whether that
    /// makes it a loop, held to `limits`, told only as the count reaches it. An allow starts the
    /// count again.
    pub fn answered(&mut self, chat: u32, refused: bool, limits: &Limits) -> Option<Why> {
        let counted = self.chats.entry(chat).or_default();
        counted.refused = if refused {
            counted.refused.saturating_add(1)
        } else {
            0
        };
        let times = counted.refused;
        (refused && times == limits.refused_times).then_some(Why::Refused { times })
    }

    /// Chat `chat` had `stop` written: whether that makes it a loop, told only as the count
    /// reaches [`BLOCKED_TIMES`].
    pub fn stopped(&mut self, chat: u32, stop: &crate::eventlog::stops::Stop) -> Option<Why> {
        let crate::eventlog::stops::Stop::Block(block) = stop else {
            return None;
        };
        let aim = Aim {
            tool: block.tool.clone(),
            args: block.args.clone(),
        };
        let counted = self.chats.entry(chat).or_default();
        let times = match counted.blocked.take() {
            Some((was, times)) if was == aim => times.saturating_add(1),
            _ => 1,
        };
        counted.blocked = Some((aim, times));
        (times == BLOCKED_TIMES).then_some(Why::Blocked { times })
    }

    /// Chat `chat`'s hook line said it made or ended `call`. Not yet counted.
    pub fn called(
        &mut self,
        _chat: u32,
        _call: &crate::hookwire::ToolCall,
        _verified: bool,
    ) -> Option<Why> {
        None
    }

    /// **"It's fine, continue"**: every count of chat `chat` starts again.
    pub fn fine(&mut self, chat: u32) {
        self.chats.remove(&chat);
    }

    /// Chat `chat` is gone: nothing is kept of it.
    pub fn forget(&mut self, chat: u32) {
        self.chats.remove(&chat);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod blocked_tests;
