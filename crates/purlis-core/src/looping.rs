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
//! - **Blocked** (R-206, Q133): [`BLOCKED_TIMES`] blocks on the same target, the tool plus its
//!   argument digest, with no different call succeeding and no Allow between. Only a block that
//!   is proof counts (Q235, Q237): a guard's, the sandbox's, a policy's or a project's, reported
//!   plainly. The harness's own check never counts (R-208, Q153), nor does a report that is
//!   `unverified` or `cant_tell` (R-209), which neither resets nor suppresses the count
//!   (R-103); a refusal whose ask is open, a hold, is no block (R-215). Counted apart from the
//!   person's refusals and from the cool-off (R-350). Counted per aim, in a map of at most
//!   [`AIMS_KEPT`] aims, the oldest dropped, so blocked calls that alternate still pause; and a
//!   call counts once, however many connections it held (Q253). An Allow on any path (a hook's
//!   ask, a held connection, the chooser, a late answer), a different call that came back and
//!   Let it keep trying clear every aim (R-210, R-211, R-218).
//!
//!   **Accepted limits** (Q133, Q235, Q253): a reworded retry is a different target. Only a
//!   tool with a post-tool line resets the count by succeeding (R-210 is narrower than "any
//!   different success": a call to a tool no post-tool hook names, Grep say, is never heard to
//!   come back, so the rule errs toward pausing). A success is taken at its hook line's word,
//!   so an unverified chat's own lines can reset its count; and a sandboxed chat holds its own
//!   hook token, so it can send a "verified" deny about itself, which can only pause itself. A
//!   chat holding its hook token could also reuse one call id for every call, so each block
//!   after the first is taken as the same call's and never counted (the Q235 class). A chat
//!   that dodges the pause so only keeps being blocked, and each block is purlis's own
//!   observation.
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

use crate::eventlog::stops::{self, Block, Source, Stop, Target};
use crate::stuck::Failing;

/// **How many times what reads Stuck? a loop is**: a chat reads Stuck? at the same command's
/// `stuck-failing-times` failures, and is paused as looping at this many times that. One setting
/// for both, so a person who raises one raises the other.
pub const LOOPING_IS_STUCK_TIMES: u32 = 2;

/// **How many blocks on the same target are a loop** (R-206, Q133): three.
pub const BLOCKED_TIMES: u32 = 3;

/// **How many aims' block counts a chat keeps** (Q253): past this, the oldest is dropped.
pub const AIMS_KEPT: usize = 16;

/// **How many calls a chat keeps as counted already** (Q253): a call counts once toward the
/// block loop, however many connections it held; past this, the oldest is forgotten.
const CALLS_KEPT: usize = 16;

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
    /// Each target it was blocked on, and how many times with no reset between, oldest
    /// first: at most [`AIMS_KEPT`] (Q253).
    blocked: Vec<(Aim, u32)>,
    /// The calls a block was counted for already, oldest first: at most [`CALLS_KEPT`].
    calls: Vec<String>,
}

impl Counted {
    /// One more block on `aim`: how many it makes, with `aim` the newest kept.
    fn blocked_on(&mut self, aim: Aim) -> u32 {
        let times = match self.blocked.iter().position(|(was, _)| *was == aim) {
            Some(at) => self.blocked.remove(at).1.saturating_add(1),
            None => 1,
        };
        self.blocked.push((aim, times));
        if self.blocked.len() > AIMS_KEPT {
            self.blocked.remove(0);
        }
        times
    }

    /// Whether a block under `call` was counted already; remembered as counted from now.
    fn counted_already(&mut self, call: &str) -> bool {
        if self.calls.iter().any(|one| one == call) {
            return true;
        }
        self.calls.push(call.to_owned());
        if self.calls.len() > CALLS_KEPT {
            self.calls.remove(0);
        }
        false
    }
}

/// **What a block was aimed at** (R-206): the tool plus its argument digest, as its hook sent
/// it. Where a block names no digest (a held connection no call was heard for), its target
/// summary stands in for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Aim {
    tool: String,
    args: Option<String>,
    target: Option<Target>,
}

impl Aim {
    /// The target of `block`; none for one that names neither a digest nor a target, which
    /// nothing tells apart from another.
    fn of(block: &Block) -> Option<Self> {
        let target = block.target.clone().filter(|_| block.args.is_none());
        (block.args.is_some() || target.is_some()).then(|| Self {
            tool: crate::shown::readable(&block.tool, stops::TARGET_CHARS),
            args: block.args.clone(),
            target,
        })
    }

    /// The target of a call a hook line names.
    fn of_call(call: &crate::hookwire::ToolCall) -> Self {
        Self {
            tool: stops::line_tool(call),
            args: call.args.clone(),
            target: None,
        }
    }
}

/// **Whether `block` is proof toward the block loop** (Q235, Q237): a guard's, the sandbox's, a
/// policy's or a project's, reported plainly. Never the harness's own check (R-208), never a
/// report that is qualified (R-209).
fn counts(block: &Block) -> bool {
    block.qualifier.is_none()
        && match block.source {
            Source::Guard | Source::Sandbox | Source::Policy | Source::ProjectSetting => true,
            Source::HarnessCheck => false,
        }
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
    /// count again, and the block count with it (R-211); a refusal leaves the block count as it
    /// is (R-350).
    pub fn answered(&mut self, chat: u32, refused: bool, limits: &Limits) -> Option<Why> {
        let counted = self.chats.entry(chat).or_default();
        counted.refused = if refused {
            counted.refused.saturating_add(1)
        } else {
            counted.blocked.clear();
            0
        };
        let times = counted.refused;
        (refused && times == limits.refused_times).then_some(Why::Refused { times })
    }

    /// **Chat `chat` had `stop` written**: whether that makes it a loop, told only as the count
    /// reaches [`BLOCKED_TIMES`] (R-206). A block that is no proof ([`counts`]), and any stop
    /// that is no block (a hold, whose ask is open, R-215), leave the count as it is.
    pub fn stopped(&mut self, chat: u32, stop: &Stop) -> Option<Why> {
        self.stopped_under(chat, stop, None)
    }

    /// [`Loops::stopped`] for a block written under `call`, the harness's id for the call it was
    /// held under, where one was heard: a call counts once, however many connections it held
    /// (Q253).
    pub fn stopped_under(&mut self, chat: u32, stop: &Stop, call: Option<&str>) -> Option<Why> {
        let Stop::Block(block) = stop else {
            return None;
        };
        if !counts(block) {
            return None;
        }
        let aim = Aim::of(block)?;
        let counted = self.chats.entry(chat).or_default();
        if call.is_some_and(|call| counted.counted_already(call)) {
            return None;
        }
        let times = counted.blocked_on(aim);
        (times == BLOCKED_TIMES).then_some(Why::Blocked { times })
    }

    /// **The person allowed a sandbox ask of chat `chat`'s** (R-211, Q253): a held connection's,
    /// the chooser's or one answered late. Its block count starts again; its refusals' count is
    /// [`Loops::answered`]'s alone.
    pub fn allowed(&mut self, chat: u32) {
        if let Some(counted) = self.chats.get_mut(&chat) {
            counted.blocked.clear();
        }
    }

    /// **Chat `chat`'s hook line said it made or ended `call`**, reported from outside the
    /// chat's reach where `verified` (`stops::reported_from_outside_its_reach`): the block it is,
    /// if any, counted as [`Loops::stopped`] counts one (`stops::block_of_line`, the recorder's
    /// own rule); otherwise, where the call came back, it resets the block count of every aim
    /// but its own (R-210). A call allowed but not yet come back is no success, nor is one a
    /// block was counted under (Q253).
    pub fn called(
        &mut self,
        chat: u32,
        call: &crate::hookwire::ToolCall,
        verified: bool,
    ) -> Option<Why> {
        if let Some(block) = stops::block_of_line(call, verified) {
            return self.stopped(chat, &Stop::Block(block));
        }
        let came_back = crate::hookreg::find(&call.tool_hook)
            .is_some_and(crate::hookreg::Handler::hears_a_call_come_back);
        if came_back && let Some(counted) = self.chats.get_mut(&chat) {
            // A call a block was counted under is no success, even where it exits 0, as
            // `curl … || true` does: it clears no other aim (Q253).
            let blocked_under_it = call
                .call
                .as_deref()
                .is_some_and(|id| counted.calls.iter().any(|one| one == id));
            if !blocked_under_it {
                let aim = Aim::of_call(call);
                counted.blocked.retain(|(was, _)| *was == aim);
            }
        }
        None
    }

    /// **"It's fine, continue"**: every count of chat `chat` starts again; for a block loop, this
    /// is Let it keep trying (R-218).
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
