// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner chat's skill runs, logged in its desktop's plane** (SK-20b, #3084, decision 28).
//!
//! A chat on a pool's runner is recorded by the runner's own event log writer, which logs its
//! skill runs in the runner's checkout of the project: nothing brings that back. The window reads
//! the runner's event log over the link (#3393), so each `skill.started` and `skill.ended` line of
//! it is read here into a [`Relayed`] and logged in the desktop's own skill log, as the runner's
//! writer logs a local run's ([`super::started`], [`super::ended`]): under the runner's device,
//! so a run says where it ran, at the time its line was written.
//!
//! **Only what the studio reads is taken of a line**, each field held to a few bytes: the run's
//! id, its skill, what started it, its copy's level, how it ended and what it counted. A line is
//! the runner's own word, so nothing else of it is kept. Where the chat ran, its persona, harness
//! and workspace, is the window's own word for the chat ([`RanAs`]), never the runner's.

use std::path::{Path, PathBuf};

use purlis_session_protocol::session::Event;
use purlis_skills::runs::{Ended, Level, Signals, StartedBy};
use serde_json::Value;

/// The longest id held: a chat's, a run's or a device's. A ULID is 26 characters.
const MOST_ID: usize = 32;

/// The longest skill name held, its harness's namespace included.
const MOST_SKILL: usize = 200;

/// **A skill run's start or end, as a runner's event log says it.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relayed {
    /// The chat's id (ADR 0066): which of the window's chats it is of.
    pub chat: String,
    /// The runner's device, whose skill log file the row goes in.
    pub device: String,
    /// The skill run's id.
    pub run: String,
    /// When the line was written, in milliseconds since 1970.
    pub at_ms: u64,
    pub line: Line,
}

/// What a [`Relayed`] says of its run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Started {
        skill: String,
        started_by: StartedBy,
        /// The copy's level, where the runner's start knew it.
        level: Option<Level>,
    },
    Ended {
        ended: Ended,
        ms: u64,
        signals: Signals,
        tokens: Option<u64>,
        /// The run it started inside, where a runner that says so said it.
        parent: Option<String>,
    },
}

/// An id as a line names it: short, and of letters, digits, `-` and `_` alone, so it never names
/// a path.
fn an_id(text: &str) -> Option<String> {
    (!text.is_empty()
        && text.len() <= MOST_ID
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then(|| text.to_owned())
}

impl Relayed {
    /// **The skill run `event` starts or ends**, or `None` for any other line, and for one whose
    /// ids, time or skill are not ones a run has.
    pub fn of(event: &Event) -> Option<Relayed> {
        let started = match event.kind.as_str() {
            "skill.started" => true,
            "skill.ended" => false,
            _ => return None,
        };
        let body = &event.body;
        let text = |key: &str| body.get(key).and_then(Value::as_str);
        let count = |key: &str| body.get(key).and_then(Value::as_u64);
        let small = |key: &str| count(key).map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX));
        let line = if started {
            let skill = text("skill").filter(|skill| {
                !skill.trim().is_empty()
                    && skill.len() <= MOST_SKILL
                    && !skill.chars().any(char::is_control)
            })?;
            Line::Started {
                skill: skill.to_owned(),
                started_by: text("started_by")
                    .and_then(StartedBy::parse)
                    .unwrap_or(StartedBy::Unknown),
                level: text("skill_level").and_then(Level::parse),
            }
        } else {
            Line::Ended {
                ended: text("ended")
                    .and_then(Ended::parse)
                    .unwrap_or(Ended::Unknown),
                ms: count("ms").unwrap_or_default(),
                signals: Signals {
                    tool_calls: small("tool_calls"),
                    tool_errors: small("tool_errors"),
                    refusals: small("refusals"),
                    asks: small("asks"),
                },
                tokens: count("tokens"),
                parent: text("parent_skill_run").and_then(an_id),
            }
        };
        Some(Relayed {
            chat: an_id(event.chat.as_deref()?)?,
            device: an_id(&event.device_id)?,
            run: an_id(text("skill_run")?)?,
            at_ms: ulid::Ulid::from_string(&event.ulid).ok()?.timestamp_ms(),
            line,
        })
    }
}

/// **Where the window says a runner's chat ran**: the persona it runs as, its harness by the
/// word the plane calls it (`claude`), and its workspace, `None` at the plane root.
#[derive(Debug, Clone, Copy, Default)]
pub struct RanAs<'a> {
    pub persona: Option<&'a str>,
    pub harness: Option<&'a str>,
    pub workspace: Option<&'a str>,
}

/// **Logs `relayed` in the skill log of the plane at `root`**, as a run of a chat that ran as
/// `ran_as`: its start row as [`super::started`] writes a local run's, its end row as
/// [`super::ended`] does. Nothing is logged where the plane may not be written, or the line's
/// time is not one.
pub fn logged(root: &Path, relayed: &Relayed, ran_as: &RanAs<'_>) -> Option<PathBuf> {
    let when = chrono::DateTime::from_timestamp_millis(i64::try_from(relayed.at_ms).ok()?)?;
    match &relayed.line {
        Line::Started {
            skill,
            started_by,
            level,
        } => super::started(
            root,
            &super::Starting {
                id: &relayed.run,
                skill,
                started_by: *started_by,
                workspace: ran_as.workspace,
                level: *level,
                harness: ran_as.harness,
            },
            ran_as.persona,
            when,
            &relayed.device,
        ),
        Line::Ended {
            ended,
            ms,
            signals,
            tokens,
            parent,
        } => super::ended_as(
            root,
            &super::Ending {
                id: &relayed.run,
                ended: *ended,
                ms: *ms,
                signals: *signals,
                tokens: *tokens,
                parent: parent.as_deref(),
            },
            when,
            &relayed.device,
        ),
    }
}

#[cfg(test)]
mod tests;
