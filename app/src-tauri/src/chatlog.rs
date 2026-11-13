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

/// One chat the window asks the blocked marker of, and its last look: the `seq` of the latest
/// block the person looked at, read from its key in the seen record (decimal, `None` for
/// never).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct BlocksAsked {
    pub session: u32,
    pub after: Option<String>,
}

/// **A chat's blocks since the person last looked** (R-172, R-174, R-299): what its marker
/// shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatBlocks {
    pub session: u32,
    /// The verified blocks after the look: the marker's count, "2 new blocked".
    pub blocks: u32,
    /// The unverified ones after it, which never count on the sidebar: a muted dot.
    pub unverified: u32,
    /// The seen record's key for a look at the chat's latest block, `blocks:<chat>:<seq>`, the
    /// chat by its logged id, stable across restarts (Q258): what the window keeps once the chat
    /// has been looked at. `None` for a chat with none, or one the log does not know.
    pub key: Option<String>,
}

/// **The markers' running tally** (R-174): fed from the event log's subscription, so a refresh
/// reads only what was written since the last one. One per log; a log opened elsewhere starts
/// it again.
static BLOCKS: std::sync::Mutex<Option<Feed>> = std::sync::Mutex::new(None);

/// The log a tally reads, where it has read it to, and what it has counted.
struct Feed {
    dir: std::path::PathBuf,
    read: purlis_core::eventlog::Subscription,
    blocks: chatlog::Blocks,
}

/// One asked chat: its number, the id the event log knows it by, and its last look.
struct Asked {
    session: u32,
    chat: Option<String>,
    after: Option<u64>,
}

/// Each of `asked`'s markers, after reading on from the log in `dir`.
fn blocks_of(dir: &std::path::Path, asked: &[Asked]) -> Result<Vec<ChatBlocks>, String> {
    let mut held = BLOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.as_ref().is_none_or(|feed| feed.dir != dir) {
        *held = Some(Feed {
            dir: dir.to_path_buf(),
            read: purlis_core::eventlog::subscribe(dir, 0),
            blocks: chatlog::Blocks::default(),
        });
    }
    let Some(Feed { read, blocks, .. }) = held.as_mut() else {
        return Ok(Vec::new());
    };
    for delivery in read.poll().map_err(|why| why.to_string())? {
        blocks.take(delivery);
    }
    Ok(asked
        .iter()
        .map(|one| {
            let Some(chat) = one.chat.as_deref() else {
                return ChatBlocks {
                    session: one.session,
                    blocks: 0,
                    unverified: 0,
                    key: None,
                };
            };
            let marker = blocks.since(chat, one.after);
            ChatBlocks {
                session: one.session,
                blocks: marker.blocks,
                unverified: marker.unverified,
                key: marker
                    .latest
                    .map(|seq| format!("{}{chat}:{seq}", purlis_core::inboxupdates::BLOCKS)),
            }
        })
        .collect())
}

/// **Each asked chat's blocked marker** (R-172, R-174, R-299; ADR 0124 decision 9.1): its
/// verified and unverified blocks after the last look the window keeps for it, from this
/// machine's event log. No notification and no announcement is made of them (R-176), and
/// nothing of Quiet or snooze is touched (R-177): it only reads.
#[tauri::command]
#[specta::specta]
pub(crate) async fn chat_blocks(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    asked: Vec<BlocksAsked>,
) -> Result<Vec<ChatBlocks>, String> {
    let held = planes.held(&plane)?;
    let asked: Vec<Asked> = asked
        .into_iter()
        .map(|one| Asked {
            session: one.session,
            chat: held.chats().logged_id_of(one.session),
            after: one.after.and_then(|after| after.parse().ok()),
        })
        .collect();
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        // No log on this machine: no block is counted, and no marker is drawn.
        _ => return Ok(Vec::new()),
    };
    crate::off_the_window("counting a chat's blocks", move || blocks_of(&dir, &asked)).await
}

#[cfg(test)]
mod tests;
