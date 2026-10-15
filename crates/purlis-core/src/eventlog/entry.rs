// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the window asks the event log's one writer to write** (#2003): one [`Entry`] for each
//! kind of write the window makes, applied by the [`Recorder`] that holds the log
//! ([`Recorder::apply`]).
//!
//! **One writer per device** (ADR 0066, ADR 0068): the window, or, with chats in the session
//! host, the host. While chats run in the host the window opens no log at all; what it decided
//! (the run a start begins, a person's grant, what the board made of a hook line) travels to the
//! host as an `Entry` and is written there, by the same code the window's own path runs. So the
//! two paths write the same events, and never both.
//!
//! An entry names a chat by its project and number, or by its ids where the window holds them,
//! never by anything a chat sent: the window composes each one from its own state, as it
//! composed the call it stands for.

use std::io;
use std::path::PathBuf;
use std::time::Instant;

use super::{Attributes, Began, Event, Followed, Recorder, RunOf};

/// One write the window asks of the event log, as it travels to the log's writer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case")]
pub enum Entry {
    /// A state hook's line, with what the board made of its conversation and, for a run a
    /// `/clear` begins, that run's id ([`Recorder::report_with`]).
    Report {
        plane: PathBuf,
        report: crate::hookwire::Report,
        followed: Followed,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        begun: Option<String>,
    },
    /// A tool hook's line ([`Recorder::tool`]), timed as the writer takes it.
    Tool {
        plane: PathBuf,
        call: crate::hookwire::ToolCall,
    },
    /// A commit purlis's git hook refused, of chat `chat` ([`Recorder::refused`]).
    Refused { plane: PathBuf, chat: u32 },
    /// The run a start begins ([`Recorder::begin`]).
    Begin {
        plane: PathBuf,
        number: u32,
        chat: String,
        run: String,
        cause: Began,
        attributes: Attributes,
    },
    /// What a start decided about a chat's sandbox, under the run it begins
    /// ([`Recorder::trust`]): its kind and body as the window composed them.
    Trust {
        chat: String,
        run: String,
        kind: String,
        body: serde_json::Value,
    },
    /// A person's act the log audits (a sandbox or dispatch grant, a proposed change applied),
    /// under chat `number` where it came from one: its kind and body as the window composed
    /// them ([`Recorder::sandbox_grant`], [`Recorder::dispatch_grant`],
    /// [`Recorder::proposed_applied`]).
    Audited {
        plane: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        number: Option<u32>,
        kind: String,
        body: serde_json::Value,
    },
    /// A line the window wrote in the kill switch's journal ([`Recorder::kill_switch_wrote`]).
    KillSwitchWrote { line: crate::halt::Entry },
    /// The kill switch's journal, read again ([`Recorder::kill_switch_read`]).
    KillSwitchRead,
}

impl Entry {
    /// The chat this entry is a hook line of, by its number, where it is one: whose hook spool
    /// the writer looks at next.
    pub fn hook_line_of(&self) -> Option<u32> {
        match self {
            Entry::Report { report, .. } => Some(report.chat),
            Entry::Tool { call, .. } => Some(call.chat),
            Entry::Refused { chat, .. } => Some(*chat),
            _ => None,
        }
    }
}

impl Recorder {
    /// **Writes `entry`**, as the call it stands for writes it, and answers the events written,
    /// in order. Not yet durable: the caller makes them so ([`Recorder::durable`]) outside the
    /// lock it holds this under, as a live hook line's writer does, so lines written at once
    /// share one `fsync`. The writes that audit a person's act are made durable here, as their
    /// own calls make them.
    pub fn apply(&mut self, entry: Entry) -> io::Result<Vec<Event>> {
        let one = |event: io::Result<Event>| event.map(|event| vec![event]);
        match entry {
            Entry::Report {
                plane,
                report,
                followed,
                begun,
            } => one(self.report_with(&plane, &report, followed, begun.as_deref())),
            Entry::Tool { plane, call } => one(self.tool(&plane, &call, Instant::now())),
            Entry::Refused { plane, chat } => one(self.refused(&plane, chat)),
            Entry::Begin {
                plane,
                number,
                chat,
                run,
                cause,
                attributes,
            } => one(self.begin(
                &plane,
                number,
                RunOf {
                    chat: &chat,
                    run: &run,
                },
                cause,
                &attributes,
            )),
            Entry::Trust {
                chat,
                run,
                kind,
                body,
            } => one(trust_kind(&kind).and_then(|()| {
                self.trust_said(
                    RunOf {
                        chat: &chat,
                        run: &run,
                    },
                    &kind,
                    body,
                )
            })),
            Entry::Audited {
                plane,
                number,
                kind,
                body,
            } => one(trust_kind(&kind).and_then(|()| self.audited(&plane, number, &kind, body))),
            Entry::KillSwitchWrote { line } => {
                let config = self.config()?;
                self.kill_switch_wrote(line, &config)
            }
            Entry::KillSwitchRead => {
                let config = self.config()?;
                self.kill_switch_read(&config)
            }
        }
    }

    /// The machine store this recorder read its device id from, where it was opened from one
    /// ([`Recorder::open_in`]): where the kill switch's journal is.
    fn config(&self) -> io::Result<PathBuf> {
        self.config.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "this event log was opened without the machine store the kill switch's journal \
                 is in",
            )
        })
    }
}

/// What a trust or audited entry may be written as: one of the log's `trust.` kinds (a sandbox
/// decision, a person's grant or revoke, an applied change), never any other.
fn trust_kind(kind: &str) -> io::Result<()> {
    let trust = kind.strip_prefix(TRUST).is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_')
    });
    if trust {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{kind:?} is not a kind of trust event, so nothing was written"),
        ))
    }
}

/// Where every trust event's kind begins (ADR 0075's registry).
const TRUST: &str = "trust.";

#[cfg(test)]
mod tests;
