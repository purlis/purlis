// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's Chat log, for its tab** (R-224, R-227, R-233, R-310; ADR 0124 decision 11): the
//! rows of its story, read from this machine's event log through `purlis_core::eventlog::
//! chatlog::rows`, oldest first. Phase 1 draws its start, its end and its blocks (T1-29).
//!
//! **Pull-only** (R-225): read when the tab opens, told to no window, and nothing here counts
//! or marks a row as new. It holds nothing of its own: the Chat log is a query of the event
//! log, and the event log holds no chat's words.
//!
//! **A block's reason comes from memory, never the log** (R-340, Q185): what a harness's own
//! check said as it refused a call is held by the app as the hook channel hears it
//! ([`HARNESS_REASONS`]) and put on the block's row here, as written. A restarted app holds
//! none, so the row then has none.

use std::sync::LazyLock;

use purlis_core::eventlog::chatlog::{self, Row};
use purlis_core::eventlog::reasons::Reasons;
use purlis_core::eventlog::stops::{Block, Target};
use purlis_core::sandbox::protection::InForce;

use crate::planes::PlaneId;

/// One layer's protection at a start, as the start row says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProtectionLayer {
    /// The layer: `sandbox`, the one a tier sets in phase 1.
    pub layer: String,
    /// Its level's word: `on`, `floor` or `off`.
    pub level: String,
    /// Who set it: `policy`, `project`, `person`, `unconfirmed`, `chat`, `purlis` or
    /// `unknown`.
    pub tier: String,
}

/// The protection a start recorded (R-227): the four fields of its `run.started`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct StartProtection {
    pub layers: Vec<ProtectionLayer>,
    /// The last commit of `charter.toml`: a commit, `none` or `unknown`.
    pub charter_commit: String,
    /// The working `charter.toml` differed from that commit.
    pub charter_dirty: bool,
    /// How many answers were in force: none until phase 2.
    pub answers_in_force: u32,
}

/// One row of a chat's Chat log.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatLogRow {
    /// The chat's run started (`cause`: `start`, `reopen`, `fresh`, ...), with the protection
    /// it recorded, or none where its event has none this version can read.
    Started {
        /// When (UTC, RFC 3339), where the event's id says.
        at: Option<String>,
        cause: String,
        protection: Option<StartProtection>,
    },
    /// The chat's run ended: `completed`, `failed` or `stopped`, why, and its exit code.
    Ended {
        at: Option<String>,
        state: String,
        cause: String,
        code: Option<i32>,
    },
    /// A call was blocked: by whom, how much its report is worth, at what, and why it said.
    Blocked {
        at: Option<String>,
        tool: String,
        source: String,
        qualifier: Option<String>,
        target_kind: Option<String>,
        target: Option<String>,
        harness: String,
        reason: Option<String>,
    },
}

/// **The reasons the harnesses' own checks gave, for the app's life** (R-340): held as the
/// hook channel hears them (`hooks.rs`), read as a Chat log is, never written.
pub(crate) static HARNESS_REASONS: LazyLock<Reasons> = LazyLock::new(Reasons::default);

/// A chat's Chat log: whether purlis knows the chat, and its rows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatLog {
    /// purlis knows the chat by its number: open, or ended here and still kept.
    pub known: bool,
    pub rows: Vec<ChatLogRow>,
}

fn at(ms: Option<u64>) -> Option<String> {
    let ms = i64::try_from(ms?).ok()?;
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|when| when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn protection_of(in_force: InForce) -> StartProtection {
    StartProtection {
        layers: in_force
            .layers
            .into_iter()
            .map(|layer| ProtectionLayer {
                layer: layer.layer,
                level: layer.level,
                tier: serde_json::to_value(layer.tier)
                    .ok()
                    .and_then(|word| word.as_str().map(str::to_owned))
                    .unwrap_or_else(|| purlis_core::eventlog::UNKNOWN.to_owned()),
            })
            .collect(),
        charter_commit: in_force.charter_commit,
        charter_dirty: in_force.charter_dirty,
        answers_in_force: u32::try_from(in_force.answers_in_force.len()).unwrap_or(u32::MAX),
    }
}

/// A closed word's own spelling, as the log writes it.
fn word_of(value: impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|word| word.as_str().map(str::to_owned))
        .unwrap_or_else(|| purlis_core::eventlog::UNKNOWN.to_owned())
}

/// A block's target, as its kind's word and its value.
fn target_of(target: &Target) -> (String, String) {
    match target {
        Target::Host(host) => ("host".to_owned(), host.as_str().to_owned()),
        Target::Folder(folder) => ("folder".to_owned(), folder.as_str().to_owned()),
        Target::Service(service) => ("service".to_owned(), service.as_str().to_owned()),
        Target::GuardArm(arm) => ("guard_arm".to_owned(), arm.as_str().to_owned()),
    }
}

/// The window's row for the core's `row`, a block's with the reason `reason` finds for its
/// call.
fn row_of(row: Row, reason: &dyn Fn(&str) -> Option<String>) -> ChatLogRow {
    match row {
        Row::Started {
            at_ms,
            cause,
            protection,
        } => ChatLogRow::Started {
            at: at(at_ms),
            cause,
            protection: protection.map(protection_of),
        },
        Row::Ended {
            at_ms,
            state,
            cause,
            code,
        } => ChatLogRow::Ended {
            at: at(at_ms),
            state,
            cause,
            code: code.and_then(|code| i32::try_from(code).ok()),
        },
        Row::Blocked {
            at_ms,
            block,
            call,
            harness,
        } => {
            let (tool, source, qualifier, target) = match block {
                Some(Block {
                    tool,
                    source,
                    qualifier,
                    target,
                    ..
                }) => (
                    tool,
                    word_of(source),
                    qualifier.map(word_of),
                    target.as_ref().map(target_of),
                ),
                None => (
                    purlis_core::eventlog::UNKNOWN.to_owned(),
                    purlis_core::eventlog::UNKNOWN.to_owned(),
                    None,
                    None,
                ),
            };
            let (target_kind, target) = target.unzip();
            ChatLogRow::Blocked {
                at: at(at_ms),
                tool,
                source,
                qualifier,
                target_kind,
                target,
                harness,
                reason: call.as_deref().and_then(reason),
            }
        }
    }
}

/// The Chat log of the chat with id `chat` (`None`: purlis does not know it) in `events`, with
/// no reason for any block.
#[cfg(test)]
fn chat_log_of(events: &[purlis_core::eventlog::Event], chat: Option<&str>) -> ChatLog {
    chat_log_with(events, chat, &|_| None)
}

/// [`chat_log_of`], each block with the reason `reason` finds for its call (R-340).
fn chat_log_with(
    events: &[purlis_core::eventlog::Event],
    chat: Option<&str>,
    reason: &dyn Fn(&str) -> Option<String>,
) -> ChatLog {
    match chat {
        None => ChatLog {
            known: false,
            rows: Vec::new(),
        },
        Some(chat) => ChatLog {
            known: true,
            rows: chatlog::rows(events, chat)
                .into_iter()
                .map(|row| row_of(row, reason))
                .collect(),
        },
    }
}

/// **Chat `session`'s Chat log** (R-224): its rows, from this machine's event log, oldest
/// first. Read when its tab opens, off the window's thread, as it reads the log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_log(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<ChatLog, String> {
    let held = planes.held(&plane)?;
    let chat = held.chats().logged_id_of(session);
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        Some(Err(refused)) => {
            return Err(format!(
                "purlis keeps no event log on this machine: {}",
                refused.why
            ));
        }
        None => return Err("purlis keeps no event log on this machine.".to_owned()),
    };
    crate::off_the_window("reading a chat's log", move || {
        let events = purlis_core::eventlog::read(&dir).map_err(|why| why.to_string())?;
        let root = plane.root();
        Ok(chat_log_with(&events, chat.as_deref(), &|call| {
            HARNESS_REASONS.of(root, session, call)
        }))
    })
    .await
}

#[cfg(test)]
mod tests;
