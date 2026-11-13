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
//! **What writes them**: the recorder writes a block beside the tool event of a hook's line, for
//! a guard's deny (`guard_block`, R-316) or for the harness's own check's refusal
//! (`harness_check_block`, R-094, R-325), at most one per line
//! ([`block_of_line`]). The sandbox refusals, the merge hold, the loop rule and the asks
//! each write their kind in their own slice; the window's entry carries them
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
/// guard's arm, and nothing else: never a command, its arguments or its output. Each value is
/// checked as it is built and as it is read off the wire ([`Host`], [`Folder`], [`Token`]), so
/// text shaped like a command or an address with a query is refused, never clipped into the
/// log; what passes is clipped to one short line as it is written ([`TARGET_CHARS`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Host(Host),
    Folder(Folder),
    Service(Token),
    GuardArm(Token),
}

/// The most characters of a target summary, or of a tool's name, written: past it, what is
/// written is the first 120 characters of its escaped form and `...`.
pub const TARGET_CHARS: usize = 120;

/// The most characters of a [`Token`].
pub const TOKEN_CHARS: usize = 64;

/// The most characters of a [`Folder`] as it is given; it is clipped to [`TARGET_CHARS`] as it
/// is written.
pub const FOLDER_CHARS: usize = 1024;

/// **A value refused as a stop's target.** It names the kind of target and never the value,
/// which may be the very secret it was refused for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a {0} a stop can name")]
pub struct InvalidTarget(&'static str);

/// **A host**: a hostname or an address, with a port where there is one (`api.example.com`,
/// `localhost:8080`, `[::1]:443`), and nothing else: no path, query, user, fragment or space.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Host(String);

/// **A folder, relative to the project** (ADR 0075 §4: the audit holds a path relative to a
/// repo or a project, never an absolute one): `.` for the project itself, else its path inside
/// it. Never absolute (`/`, `\`, `~` or a drive), never with a `..` part, and with no control
/// character or any of `' " ` $ ; | & < > ? # *`, so a command cannot pass for one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Folder(String);

/// **A short token**: a service's or a guard's arm's name, 1 to [`TOKEN_CHARS`] of
/// `A-Z a-z 0-9 . _ : -`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Token(String);

macro_rules! checked {
    ($type:ident, $what:literal, $check:path) => {
        impl TryFrom<String> for $type {
            type Error = InvalidTarget;
            fn try_from(value: String) -> Result<Self, InvalidTarget> {
                if $check(&value) {
                    Ok(Self(value))
                } else {
                    Err(InvalidTarget($what))
                }
            }
        }

        impl TryFrom<&str> for $type {
            type Error = InvalidTarget;
            fn try_from(value: &str) -> Result<Self, InvalidTarget> {
                Self::try_from(value.to_owned())
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> String {
                value.0
            }
        }

        impl $type {
            /// The value, as checked.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

checked!(Host, "host", is_host);
checked!(Folder, "folder", is_folder);
checked!(Token, "service or guard arm", is_token);

fn is_host(value: &str) -> bool {
    let (name, port) = if let Some(rest) = value.strip_prefix('[') {
        let Some((address, after)) = rest.split_once(']') else {
            return false;
        };
        let port = match after {
            "" => None,
            _ => match after.strip_prefix(':') {
                Some(port) => Some(port),
                None => return false,
            },
        };
        if address.is_empty()
            || !address
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return false;
        }
        (None, port)
    } else {
        let (name, port) = match value.split_once(':') {
            Some((name, port)) => (name, Some(port)),
            None => (value, None),
        };
        (Some(name), port)
    };
    let name_ok = name.is_none_or(|name| {
        !name.is_empty()
            && name.len() <= 253
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    let port_ok = port.is_none_or(|port| {
        !port.is_empty()
            && port.len() <= 5
            && port.chars().all(|c| c.is_ascii_digit())
            && port.parse::<u16>().is_ok()
    });
    name_ok && port_ok
}

fn is_folder(value: &str) -> bool {
    const REFUSED: &[char] = &['\'', '"', '`', '$', ';', '|', '&', '<', '>', '?', '#', '*'];
    let absolute = value.starts_with(['/', '\\', '~'])
        || value.as_bytes().get(1) == Some(&b':') && value.as_bytes()[0].is_ascii_alphabetic();
    !value.is_empty()
        && value.chars().count() <= FOLDER_CHARS
        && !absolute
        && !value.split(['/', '\\']).any(|part| part == "..")
        && !value
            .chars()
            .any(|c| c.is_control() || REFUSED.contains(&c))
}

fn is_token(value: &str) -> bool {
    (1..=TOKEN_CHARS).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

impl Target {
    /// A host's target ([`Host`]).
    pub fn host(value: &str) -> Result<Self, InvalidTarget> {
        Host::try_from(value).map(Self::Host)
    }

    /// A folder's target, relative to the project ([`Folder`]).
    pub fn folder(value: &str) -> Result<Self, InvalidTarget> {
        Folder::try_from(value).map(Self::Folder)
    }

    /// A service's target ([`Token`]).
    pub fn service(value: &str) -> Result<Self, InvalidTarget> {
        Token::try_from(value).map(Self::Service)
    }

    /// A guard's arm's target ([`Token`]).
    pub fn guard_arm(value: &str) -> Result<Self, InvalidTarget> {
        Token::try_from(value).map(Self::GuardArm)
    }

    /// The target as it is written: one short line. Only a folder can be longer than that;
    /// clipping a checked value never makes it another kind of text.
    fn shortened(&self) -> Self {
        let short = |value: &str| crate::shown::readable(value, TARGET_CHARS);
        match self {
            Self::Host(Host(value)) => Self::Host(Host(short(value))),
            Self::Folder(Folder(value)) => Self::Folder(Folder(short(value))),
            Self::Service(Token(value)) => Self::Service(Token(short(value))),
            Self::GuardArm(Token(value)) => Self::GuardArm(Token(short(value))),
        }
    }
}

/// **A block** (R-169, R-220): the tool (written as one short line, as a target is), who
/// refused it, how much the report is worth, what it was aimed at, and the hash of its
/// arguments as the hook sent it (`args`, a SHA-256 hex, [`super::args_hash`]), written as this
/// device's keyed digest: the target the loop matches on.
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
    /// Blocks on the same target ([`crate::looping::Why::Blocked`]).
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
    ///
    /// **A sandbox block its chat's own hook reported** comes `unverified` from the app
    /// (`sandboxblock::stop::Reported::ByItsHook`, T1-10), and is kept so on the rule a guard's
    /// report takes (Q235): plain only where the chat's run reports from outside its reach
    /// ([`reported_from_outside_its_reach`]), which the app does not know and the run here says.
    pub fn stop(&mut self, plane: &Path, number: Option<u32>, stop: &Stop) -> io::Result<Event> {
        let tool = |tool: &str| crate::shown::readable(tool, TARGET_CHARS);
        let outside = match (stop, number) {
            (Stop::Block(block), Some(number))
                if block.source == Source::Sandbox
                    && block.qualifier == Some(Qualifier::Unverified) =>
            {
                self.identity(plane, number)?
                    .is_some_and(|who| reported_from_outside_its_reach(&who.attributes))
            }
            _ => false,
        };
        let body = match stop {
            Stop::Block(block) => serde_json::to_value(Block {
                tool: tool(&block.tool),
                target: block.target.as_ref().map(Target::shortened),
                args: block.args.as_deref().map(|hash| self.key.digest(hash)),
                qualifier: if outside { None } else { block.qualifier },
                ..block.clone()
            }),
            Stop::Hold(hold) => serde_json::to_value(Hold {
                tool: tool(&hold.tool),
                target: hold.target.as_ref().map(Target::shortened),
                args: hold.args.as_deref().map(|hash| self.key.digest(hash)),
            }),
            Stop::LoopPause(pause) => serde_json::to_value(pause),
            Stop::AskAnswered(answered) => serde_json::to_value(answered),
        }
        .map_err(io::Error::other)?;
        self.audited(plane, number, stop.kind(), body)
    }

    /// **The one block a hook's line is, if any, as its body** ([`block_of_line`]), the
    /// arguments' hash keyed as the tool event's is. `verified` on the rule a guard's report
    /// takes (`attributes`, none for a chat of no run; [`reported_from_outside_its_reach`]).
    ///
    /// Written just after the line's own tool event, and answered last, so the writer that makes
    /// the tool event durable makes the block durable with it (the hook lines' one `fsync`),
    /// never under the recorder's lock as a person's act is: denies come at hook pace.
    pub(super) fn block_of(
        &self,
        call: &crate::hookwire::ToolCall,
        attributes: Option<&super::Attributes>,
    ) -> Option<serde_json::Value> {
        let verified = attributes.is_some_and(reported_from_outside_its_reach);
        let block = block_of_line(call, verified)?;
        let source = block.source;
        let block = Block {
            args: block.args.as_deref().map(|hash| self.key.digest(hash)),
            ..block
        };
        match serde_json::to_value(block) {
            Ok(body) => Some(body),
            Err(why) => {
                tracing::warn!("purlis: a {source:?} block was not written ({why})");
                None
            }
        }
    }
}

/// **The one block a hook's line is, if any** (R-316, R-094, R-325): the harness's own check's
/// (`harness_check_block`), else a guard's deny (`guard_block`), so a line gives at most
/// one block. The arguments' hash as the hook sent it, unkeyed: the recorder keys it as it
/// writes it (`Recorder::block_of`), and the block loop matches on it in memory
/// ([`crate::looping::Loops::called`]). `verified` where the chat's hooks report from outside
/// its reach ([`reported_from_outside_its_reach`]); otherwise the block is `unverified`.
pub fn block_of_line(call: &crate::hookwire::ToolCall, verified: bool) -> Option<Block> {
    harness_check_block(call, verified).or_else(|| guard_block(call, verified))
}

/// **The block a guard's deny is** (R-316, R-101, R-102): for a tool call purlis's hook
/// refused, source guard, with the gate's arm as its target where the line names one the gate
/// has; none for a call that was not refused, and for one that handed its write to the person
/// (a proposed change or a draft skill, Q235: [`crate::toolhooks::hands_the_write_over`]), and
/// for a merge the hook held, which is a hold ([`crate::mergehold::REASON`]). `unverified`
/// unless `verified`: at No sandbox the chat can reach whatever reports for it.
fn guard_block(call: &crate::hookwire::ToolCall, verified: bool) -> Option<Block> {
    if call.decision != crate::hookwire::Decision::Deny
        || call
            .rule
            .as_deref()
            .is_some_and(crate::toolhooks::hands_the_write_over)
        // A held merge is a hold, which the app writes once it raises the ask (R-114).
        || call.rule.as_deref() == Some(crate::mergehold::REASON)
    {
        return None;
    }
    Some(Block {
        tool: line_tool(call),
        source: Source::Guard,
        qualifier: (!verified).then_some(Qualifier::Unverified),
        target: call
            .arm
            .as_deref()
            .and_then(crate::toolgate::Arm::of_id)
            .and_then(|arm| Target::guard_arm(arm.id()).ok()),
        args: call.args.clone(),
    })
}

/// **The block the harness's own check is** (R-094, R-325): for the line of the hook that hears
/// a harness's own refusal ([`crate::hookreg::HARNESS_DENIED`]), source
/// [`Source::HarnessCheck`], with the tool by the harness's name; none for any other line, so a
/// harness whose hooks report no refusal gets no block guessed for it. No target: the line
/// names nothing the writer resolved itself. The check's `reason` is not on the line at all.
/// `unverified` on the rule a guard's report takes (Q237, Q235): the line comes over the same
/// channel with the same token, so a chat that can reach its hooks can send it itself.
fn harness_check_block(call: &crate::hookwire::ToolCall, verified: bool) -> Option<Block> {
    (call.tool_hook == crate::hookreg::HARNESS_DENIED).then(|| Block {
        tool: line_tool(call),
        source: Source::HarnessCheck,
        qualifier: (!verified).then_some(Qualifier::Unverified),
        target: None,
        args: call.args.clone(),
    })
}

/// A line's tool, as a block writes it: one short line.
pub fn line_tool(call: &crate::hookwire::ToolCall) -> String {
    crate::shown::readable(call.tool.as_deref().unwrap_or(super::UNKNOWN), TARGET_CHARS)
}

/// The sandbox backends, as a run's `sandbox` attribute names them, that leave a harness's
/// hooks outside the sandbox they give its tools (Q235): Seatbelt on macOS and bubblewrap,
/// Claude Code's own on Linux before purlis wrapped it. An allowlist: `landlock`, purlis's own
/// wrap, holds a harness whole, a Claude Code chat's included (#4083, ADR 0067 as amended
/// 2026-11-10), and a word not named here is not trusted to be either.
const HOOKS_LEFT_OUTSIDE: [&str; 2] = ["seatbelt", "bubblewrap"];

/// Whether a run with `attributes` reports from outside the chat's reach: a harness whose hooks
/// run outside it ([`crate::harness::Harness::guard_reports_from_outside_the_chats_reach`]), under
/// a backend the host recorded that leaves them there ([`HOOKS_LEFT_OUTSIDE`]).
pub fn reported_from_outside_its_reach(attributes: &super::Attributes) -> bool {
    let sandboxed = attributes
        .sandbox
        .as_deref()
        .is_some_and(|word| HOOKS_LEFT_OUTSIDE.contains(&word));
    let outside = attributes
        .harness
        .as_deref()
        .and_then(crate::harness::Harness::of_kind)
        .is_some_and(crate::harness::Harness::guard_reports_from_outside_the_chats_reach);
    sandboxed && outside
}

#[cfg(test)]
#[path = "stops/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "stops/harness_check_tests.rs"]
mod harness_check_tests;

#[cfg(test)]
#[path = "stops/guard_tests.rs"]
mod guard_tests;
