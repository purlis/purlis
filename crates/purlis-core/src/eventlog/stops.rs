// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The kinds of stop in the event log** (R-169, R-219, R-220): a block, a hold, a loop pause
//! and an answer to an ask, each one event under the chat it is of, beside the grants and the
//! protection changes the log already writes (`trust.sandbox.*`, [`Recorder::trust`],
//! [`Recorder::sandbox_grant`]). No store and no file of their own: they are kinds of the one
//! log (ADR 0069).
//!
//! **Metadata only, as every event here is** ("an args digest, never raw args"): a [`Stop`]
//! holds a tool's name, closed words, a short target summary and the hash of the arguments a
//! hook sent, which the recorder keys by this device ([`super::ArgsKey`]) before it is written.
//! There is no field for a command, its arguments or output, or the harness's `reason` text:
//! the reason is held in memory for the row's detail while the app runs, and never written.
//!
//! **What writes them comes later**: the guard denies, the harness's own check, the sandbox
//! refusals, the merge hold, the loop rule and the asks each write their kind in their own
//! slice. Here the kinds exist, with their shapes, and the window's entry carries them
//! ([`super::Entry::Stop`]).

use std::io;
use std::path::Path;

use super::{Event, Recorder};

/// A block: a refusal that does not stop the chat on its own.
pub const BLOCK: &str = "stop.block";
/// A hold: an action purlis refuses to the chat and raises as an ask.
pub const HOLD: &str = "stop.hold";
/// A loop pause: the chat paused for going round and round.
pub const LOOP_PAUSE: &str = "stop.loop_pause";
/// The person's answer to an ask.
pub const ASK_ANSWERED: &str = "ask.answered";

/// **Who refused a block**: one of five (R-169). The policy and project values exist as values
/// only until the slice that writes them ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// One of purlis's guards.
    Guard,
    /// The harness's own permission check.
    HarnessCheck,
    /// The sandbox.
    Sandbox,
    /// An organisation's policy.
    Policy,
    /// A project's setting.
    ProjectSetting,
}

/// **How much a block's report is worth**, where it is not plain: from inside the chat's reach
/// (`unverified`), or where purlis cannot tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qualifier {
    Unverified,
    CantTell,
}

/// **The short summary of what a stop was aimed at** (Q206): a host, a folder, a service or a
/// guard's arm, and nothing else: never a command, its arguments or its output. Clipped to one
/// short line as it is written ([`TARGET_CHARS`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Host(String),
    Folder(String),
    Service(String),
    GuardArm(String),
}

/// The most characters of a target summary written.
pub const TARGET_CHARS: usize = 120;

impl Target {
    fn shortened(&self) -> Self {
        let short = |value: &str| crate::shown::readable(value, TARGET_CHARS);
        match self {
            Self::Host(value) => Self::Host(short(value)),
            Self::Folder(value) => Self::Folder(short(value)),
            Self::Service(value) => Self::Service(short(value)),
            Self::GuardArm(value) => Self::GuardArm(short(value)),
        }
    }
}

/// **A block** (R-169, R-220): the tool, who refused it, how much the report is worth, what it
/// was aimed at, and the hash of its arguments as the hook sent it (`args`, a SHA-256 hex,
/// [`super::args_hash`]), written as this device's keyed digest: the target the loop matches on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub tool: String,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<Qualifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
}

/// **A hold**: the tool purlis refused to the chat and raised as an ask, what it was aimed at,
/// and its arguments' hash, keyed as a block's is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hold {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
}

/// Which loop rule paused a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopRule {
    /// The same command failing again and again ([`crate::looping::Why::Failing`]).
    Failing,
    /// The person refusing its asks again and again ([`crate::looping::Why::Refused`]).
    Refused,
    /// Blocks on the same target.
    Blocked,
}

/// **A loop pause**: the rule that paused the chat, and at how many times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LoopPause {
    pub rule: LoopRule,
    pub times: u32,
}

/// The group of ask a person answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskGroup {
    Sandbox,
    HarnessPrompt,
    Consent,
    Hold,
    LoopPause,
    Budget,
    HookGiveUp,
}

/// What the person answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Allowed it, or let it go on.
    Allowed,
    /// Refused it, or stopped the chat.
    Refused,
}

/// **The person's answer to an ask**: its group and the answer, closed words alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AskAnswered {
    pub ask: AskGroup,
    pub answer: Answer,
}

/// **One stop, as the log writes it.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "stop", rename_all = "snake_case")]
pub enum Stop {
    Block(Block),
    Hold(Hold),
    LoopPause(LoopPause),
    AskAnswered(AskAnswered),
}

impl Stop {
    /// The event's kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Block(_) => BLOCK,
            Self::Hold(_) => HOLD,
            Self::LoopPause(_) => LOOP_PAUSE,
            Self::AskAnswered(_) => ASK_ANSWERED,
        }
    }
}

impl Recorder {
    /// **Writes one stop** under chat `number`'s current run, where it is of one, made durable
    /// before it answers: a block or a hold with its target shortened and its arguments' hash
    /// keyed by this device, never as the hook sent it.
    pub fn stop(&mut self, plane: &Path, number: Option<u32>, stop: &Stop) -> io::Result<Event> {
        let body = match stop {
            Stop::Block(block) => serde_json::to_value(Block {
                target: block.target.as_ref().map(Target::shortened),
                args: block.args.as_deref().map(|hash| self.key.digest(hash)),
                ..block.clone()
            }),
            Stop::Hold(hold) => serde_json::to_value(Hold {
                target: hold.target.as_ref().map(Target::shortened),
                args: hold.args.as_deref().map(|hash| self.key.digest(hash)),
                ..hold.clone()
            }),
            Stop::LoopPause(pause) => serde_json::to_value(pause),
            Stop::AskAnswered(answered) => serde_json::to_value(answered),
        }
        .map_err(io::Error::other)?;
        self.audited(plane, number, stop.kind(), body)
    }
}

#[cfg(test)]
#[path = "stops/tests.rs"]
mod tests;
