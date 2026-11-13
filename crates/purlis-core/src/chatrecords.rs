// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every chat that closed, by its number** (#6304, parent #5820): number → ULID → what its
//! closed-chat card shows, for a chat no dispatch started. A dispatched task's chat has its
//! dispatch record ([`crate::dispatchrecord`]), which [`crate::chatlookup`] reads first; this is
//! what lets a chat the person started by hand, or a handoff, still open its card when its
//! number is looked up after it closed.
//!
//! - **Only what the card shows**: its number, its ULID, its title as it closed, when it started
//!   and ended, who started it and where it ran. **No screen**: the person's ruling of
//!   2026-11-11 keeps a screen for dispatched tasks only ([`crate::chatscreens`]), so *Read it*
//!   on an ordinary chat's card is always disabled.
//! - **Kept as the chat closes for good**, by the app: not at a restart (the chat goes on), not at
//!   a quit (the record of open chats brings it back), and never from anything a chat sent. A
//!   chat closed again under a later number replaces its entry, by its ULID.
//! - **Why a store of its own.** The record of open chats ([`crate::reopen`]) holds a chat only
//!   while it is open, and drops it as it closes. The event log ([`crate::eventlog`]) is the
//!   device's, outside the project, keyed by ULID with no number or title in a run's start, and
//!   would be scanned whole for one lookup. Neither holds number → ULID → title once a chat
//!   has closed.
//! - **In the project's state folder**, `<state>/app/chat-records/closed.json` ([`path`]): Clone
//!   state, never committed. **Out of a chat's reach**: [`crate::sandbox`] denies the folder to
//!   every sandboxed chat for reading and for writing, as it denies the dispatch records and the
//!   kept screens. A chat's title is the person's, and so is the list of what they ran.
//! - **Swept like a dispatch record** ([`sweep`], run at the project's open from
//!   [`crate::retention::on_open`]): a chat that ended more than
//!   [`crate::retention::KEEP_FOR`] (30 days) ago goes. Held to [`MOST_KEPT`] besides, the
//!   oldest close going first.
//! - **Read as the app wrote it**: one file of at most [`MOST_FILE_BYTES`], opened following no
//!   link, written whole through a temp file at 0600 under the folder's lock. An entry that does
//!   not hold together (an id that is not a ULID, a time that is not one) is left out, and a
//!   file that is not one purlis wrote reads as empty. Losing it only costs those chats their
//!   card: nothing starts, ends or allows anything by it.
//!
//! **Tier:** Clone state (ADR 0069), never committed. Device-bound and transient are marks of
//! a Machine store and of a store living one session; this one is neither, and is swept at 30
//! days.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The store's folder under the state folder's `app/`.
pub const DIR_NAME: &str = "chat-records";

/// The file in [`DIR_NAME`] that holds the closed chats.
pub const FILE_NAME: &str = "closed.json";

/// The version of the file this purlis writes and reads.
const VERSION: u32 = 1;

/// The most chats kept: past them, the oldest close goes first. A month of a busy project's
/// chats, with room to spare.
pub const MOST_KEPT: usize = 2000;

/// The most bytes of the file read: [`MOST_KEPT`] entries of a long title each.
pub const MOST_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// The longest title kept, in characters: a tab's name is far shorter.
const MOST_TITLE_CHARS: usize = 200;

/// **A chat that closed**, as its card shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRecord {
    /// The number it answered to as it closed: what it is looked up by.
    pub number: u32,
    /// Its ULID (ADR 0066), which a restart keeps: what tells it is open again.
    pub id: String,
    /// What its tab said as it closed.
    pub title: String,
    /// When it started, UTC, RFC 3339: its ULID's time, where the chat was first minted.
    pub started: String,
    /// When it closed, UTC, RFC 3339.
    pub ended: String,
    pub started_by: StartedBy,
    /// Where it ran: its workspace's name, or `project root`.
    pub place: String,
}

/// **Who started a chat**, as the app knows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum StartedBy {
    /// The person: in the window, from a terminal, or with Ask from a chat's tab.
    Person,
    /// Another chat handed off to it: that chat's name as the person saw it then.
    Chat { name: String },
}

/// One entry as the file keeps it.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct OnDisk {
    number: u32,
    id: String,
    title: String,
    started: String,
    ended: String,
    started_by: StartedBy,
    place: String,
}

impl OnDisk {
    pub(crate) fn of(chat: &ChatRecord) -> Self {
        Self {
            number: chat.number,
            id: chat.id.clone(),
            title: chat.title.clone(),
            started: chat.started.clone(),
            ended: chat.ended.clone(),
            started_by: chat.started_by.clone(),
            place: chat.place.clone(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    v: u32,
    #[serde(default)]
    chats: Vec<serde_json::Value>,
}

/// The store's folder in the project at `root`.
pub fn dir(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(DIR_NAME)
}

/// The store's file in the project at `root`.
pub fn path(root: &Path) -> PathBuf {
    dir(root).join(FILE_NAME)
}

fn an_id(id: &str) -> bool {
    ulid::Ulid::from_string(id).is_ok_and(|read| read.to_string() == id)
}

fn a_time(time: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(time).is_ok()
}

/// Whether `chat` is one this store keeps: a ULID, two times, a title.
fn holds_together(chat: &ChatRecord) -> bool {
    an_id(&chat.id) && a_time(&chat.started) && a_time(&chat.ended)
}

/// One entry as it reads, or `None` where it does not hold together.
fn sound(value: serde_json::Value) -> Option<ChatRecord> {
    let one: OnDisk = serde_json::from_value(value).ok()?;
    let chat = ChatRecord {
        number: one.number,
        id: one.id,
        title: one.title,
        started: one.started,
        ended: one.ended,
        started_by: one.started_by,
        place: one.place,
    };
    holds_together(&chat).then_some(chat)
}

/// The file's entries as they read, newest close first: none where it is absent, not a file of
/// its own, too large, or not what purlis writes.
fn read_values(path: &Path) -> Vec<serde_json::Value> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return Vec::new();
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let Ok(file) = crate::contain::nofollow(&mut options).open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    if file
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Vec::new();
    }
    match serde_json::from_str::<File>(&text) {
        Ok(kept) if kept.v == VERSION => kept.chats,
        _ => Vec::new(),
    }
}

/// **Every chat closed in the project at `root`** that this store keeps, newest close first.
pub fn every(root: &Path) -> Vec<ChatRecord> {
    read_values(&path(root))
        .into_iter()
        .filter_map(sound)
        .take(MOST_KEPT)
        .collect()
}

/// **The chat that closed under `number`**, in `chats` as [`every`] read them: the newest close,
/// as a number is never dealt twice in a project (`reopen::Record::dealt`).
pub fn of_number(chats: &[ChatRecord], number: u32) -> Option<&ChatRecord> {
    chats.iter().find(|chat| chat.number == number)
}

/// Writes `chats`, newest first, whole, under the folder's lock already held.
fn write_all(root: &Path, chats: impl Iterator<Item = serde_json::Value>) -> std::io::Result<()> {
    let chats: Vec<serde_json::Value> = chats.take(MOST_KEPT).collect();
    let text = serde_json::to_string(&File { v: VERSION, chats }).map_err(std::io::Error::other)?;
    crate::plane::write_private(&dir(root), &path(root), text.as_bytes())
}

/// The folder made, the person's alone, and locked while the caller changes the file.
fn locked(root: &Path) -> std::io::Result<crate::rewrite::Lock> {
    let dir = dir(root);
    crate::rewrite::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if std::fs::symlink_metadata(&dir)?.is_symlink() {
            return Err(std::io::Error::other("the chat records' folder is a link"));
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    crate::rewrite::Lock::bounded(&dir)
}

/// **Keeps `chat`, which has just closed for good**, in the project at `root`: the newest entry,
/// replacing one of the same ULID (a chat closed again) or the same number, then held to
/// [`MOST_KEPT`]. Refused for an id that is not a ULID or a time that is not one.
pub fn keep(root: &Path, chat: &ChatRecord) -> std::io::Result<()> {
    if !holds_together(chat) {
        return Err(std::io::Error::other(
            "a closed chat is kept by its ULID, with when it started and ended",
        ));
    }
    let chat = ChatRecord {
        title: chat.title.chars().take(MOST_TITLE_CHARS).collect(),
        ..chat.clone()
    };
    let entry = serde_json::to_value(OnDisk::of(&chat)).map_err(std::io::Error::other)?;
    let _held = locked(root)?;
    let others = read_values(&path(root)).into_iter().filter(|one| {
        let same_id = one.get("id").and_then(serde_json::Value::as_str) == Some(chat.id.as_str());
        let same_number =
            one.get("number").and_then(serde_json::Value::as_u64) == Some(u64::from(chat.number));
        !same_id && !same_number
    });
    write_all(root, std::iter::once(entry).chain(others))
}

/// **Takes away every chat that ended more than [`crate::retention::KEEP_FOR`] before `now`**,
/// and every entry that does not hold together, in the project at `root`: what a dispatch
/// record is swept at. Answers how many went.
pub fn sweep(root: &Path, now: SystemTime) -> usize {
    let file = path(root);
    if !std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file()) {
        return 0;
    }
    let Ok(_held) = locked(root) else {
        return 0;
    };
    let values = read_values(&file);
    let before = values.len();
    let now: chrono::DateTime<chrono::Utc> = now.into();
    let kept: Vec<serde_json::Value> = values
        .into_iter()
        .filter(|value| {
            sound(value.clone()).is_some_and(|chat| {
                chrono::DateTime::parse_from_rfc3339(&chat.ended).is_ok_and(|ended| {
                    (now - ended.with_timezone(&chrono::Utc))
                        .to_std()
                        .map_or(true, |age| age <= crate::retention::KEEP_FOR)
                })
            })
        })
        .collect();
    let gone = before - kept.len();
    if gone == 0 {
        return 0;
    }
    match write_all(root, kept.into_iter()) {
        Ok(()) => gone,
        Err(why) => {
            tracing::warn!(error = %why, "the closed chats' records were not swept");
            0
        }
    }
}

#[cfg(test)]
mod tests;
