// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's Chat log, for its tab** (R-224, R-227, R-233, R-310; ADR 0124 decision 11): the
//! rows of its story, read from this machine's event log through `purlis_core::eventlog::
//! chatlog::rows`, oldest first. Phase 1 draws its start and its end.
//!
//! **Pull-only** (R-225): read when the tab opens, told to no window, and nothing here counts
//! or marks a row as new. It holds nothing of its own: the Chat log is a query of the event
//! log, and the event log holds no chat's words.

use purlis_core::eventlog::chatlog::{self, Row};
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
}

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

/// The window's row for the core's `row`.
fn row_of(row: Row) -> ChatLogRow {
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
    }
}

/// The Chat log of the chat with id `chat` (`None`: purlis does not know it) in `events`.
fn chat_log_of(events: &[purlis_core::eventlog::Event], chat: Option<&str>) -> ChatLog {
    match chat {
        None => ChatLog {
            known: false,
            rows: Vec::new(),
        },
        Some(chat) => ChatLog {
            known: true,
            rows: chatlog::rows(events, chat)
                .into_iter()
                .map(row_of)
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
        Ok(chat_log_of(&events, chat.as_deref()))
    })
    .await
}

#[cfg(test)]
mod tests;
