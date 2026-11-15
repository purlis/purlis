// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every chat that closed, by its number** (#6304, parent #5820): number → ULID → what its
//! closed-chat card shows, for a chat no dispatch started. A dispatched task's chat has its
//! dispatch record ([`crate::dispatchrecord`]), which [`crate::chatlookup`] reads first; this is
//! what lets a chat the person started by hand, or a handoff, still open its card when its
//! number is looked up after it closed.
//!
//! - **What the card shows, and what its Reopen starts**: its number (and those it had before a
//!   restart, #6486), its ULID, its title as it closed, when it started and ended, who started
//!   it and where it ran; and the chat as it closed, held to what a reopen needs (#6485,
//!   [`ChatRecord::reopened`]). **No screen**: the person's ruling of
//!   2026-11-11 keeps a screen for dispatched tasks only ([`crate::chatscreens`]), so *Read it*
//!   on an ordinary chat's card is always disabled.
//! - **Kept as the chat closes for good**, by the app: closed in the window, dropped at a
//!   launch's Start fresh, or left out of a launch's put back (#6487, [`keep_all`]). Not at a
//!   restart (the chat goes on), not at a quit (the record of open chats brings it back), and
//!   never from anything a chat sent. A
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
//!   [`crate::retention::on_open`]): a chat that ended
//!   [`crate::retention::KEEP_FOR`] (30 days) or more ago goes. Held to [`MOST_KEPT`] besides, the
//!   oldest close going first. A write killed before its rename leaves a temp file holding
//!   titles, and the sweep removes it.
//! - **Read as the app wrote it**: one file of at most [`MOST_FILE_BYTES`], opened following no
//!   link, written whole through a temp file at 0600 under the folder's lock. An entry that does
//!   not hold together (an id that is not a ULID, a time that is not one) is left out, and a
//!   file that is not one purlis wrote reads as empty. Losing it only costs those chats their
//!   card: nothing starts, ends or allows anything by it.
//!
//! **Tier:** Clone state (ADR 0069), never committed; not rebuildable.

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

/// The most bytes of the file read: [`MOST_KEPT`] entries of a long title and a chat kept for
/// its Reopen (#6485) each.
pub const MOST_FILE_BYTES: u64 = 8 * 1024 * 1024;

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
    /// The numbers it answered to before a restart dealt it [`Self::number`], oldest first
    /// (#6486): each is looked up to it too. At most [`MOST_EARLIER`].
    pub earlier: Vec<u32>,
    /// **What its Reopen starts** (#6485): the chat as it closed, held to what a reopen needs
    /// ([`for_a_reopen`]), spelt as the record of open chats spells it. `None` in an entry
    /// written before it was kept, which does not reopen.
    pub chat: Option<crate::reopen::Chat>,
}

/// **Why a closed chat's Reopen does nothing** (#6485).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CannotReopen {
    /// It closed before purlis kept what a reopen needs.
    NotKept,
    /// Its harness named no conversation, so there is nothing to start it again on.
    NoConversation,
}

impl ChatRecord {
    /// **The chat its Reopen starts** (#6485): the chat as it closed, on its conversation, the
    /// same chat under its ULID in a new run, a new number to be dealt and the one it closed
    /// under kept with those it had ([`crate::reopen::Chat::renumbered`]), as Resume on an
    /// ended chat starts one. Refused where nothing was kept or its harness named no
    /// conversation.
    pub fn reopened(&self) -> Result<crate::reopen::Chat, CannotReopen> {
        let chat = self.chat.clone().ok_or(CannotReopen::NotKept)?;
        if chat.resume.is_none() {
            return Err(CannotReopen::NoConversation);
        }
        Ok(crate::reopen::Chat {
            number: Some(self.number),
            earlier: self.earlier.clone(),
            identity: crate::reopen::Identity {
                id: Some(self.id.clone()),
                ..chat.identity.clone()
            },
            ..for_a_reopen(&chat)
        }
        .renumbered())
    }

    /// Why its Reopen does nothing, or `None` where it reopens.
    pub fn cannot_reopen(&self) -> Option<CannotReopen> {
        self.reopened().err()
    }
}

/// **What a closed chat keeps for its Reopen** (#6485): `chat` as it closed, without what was
/// its run's or its place's alone. No process, run, tab or pane; no work left running; and no
/// dispatch it came from, so a handoff or a task reopens as an ordinary chat that owes nobody a
/// report, as a finished task's Reopen does. Its program, words, folder, conversation, persona,
/// profile, name and sandbox choices stay.
pub fn for_a_reopen(chat: &crate::reopen::Chat) -> crate::reopen::Chat {
    crate::reopen::Chat {
        active: false,
        from: None,
        reopened: None,
        held: None,
        tab_opened: false,
        unseen: false,
        held_from_clusters: false,
        unattended: false,
        shows: None,
        beside: None,
        pid: None,
        studio: None,
        own_work: None,
        own_pieces: Vec::new(),
        mid_turn: false,
        kept_tmp: None,
        identity: crate::reopen::Identity {
            run: None,
            ..chat.identity.clone()
        },
        ..chat.clone()
    }
}

/// The most earlier numbers a chat keeps, the oldest going first: a month of restarts.
pub const MOST_EARLIER: usize = 64;

/// **The numbers `earlier` keeps**: none zero, none twice, at most [`MOST_EARLIER`], the newest.
pub fn held_earlier(earlier: &[u32]) -> Vec<u32> {
    let mut kept: Vec<u32> = Vec::new();
    for number in earlier.iter().rev() {
        if *number != 0 && !kept.contains(number) && kept.len() < MOST_EARLIER {
            kept.push(*number);
        }
    }
    kept.reverse();
    kept
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
    /// [`ChatRecord::earlier`]: absent for a chat never restarted, and in every entry written
    /// before it was kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    earlier: Vec<u32>,
    /// [`ChatRecord::chat`], as `reopen::chat_to_json` spells it: absent in every entry written
    /// before it was kept. One that does not read as a chat reads as absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chat: Option<serde_json::Value>,
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
            earlier: held_earlier(&chat.earlier),
            chat: chat
                .chat
                .as_ref()
                .map(|kept| crate::reopen::chat_to_json(&for_a_reopen(kept))),
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
        earlier: held_earlier(&one.earlier),
        chat: one.chat.and_then(crate::reopen::chat_from_json),
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

/// **The chat that closed under `number`**, or had it before a restart (#6486), in `chats` as
/// [`every`] read them: the newest close, as a number is never dealt twice in a project
/// (`reopen::Record::dealt`).
pub fn of_number(chats: &[ChatRecord], number: u32) -> Option<&ChatRecord> {
    chats.iter().find(|chat| chat.answers_to(number))
}

impl ChatRecord {
    /// Whether this chat had `number`, as it closed or before a restart (#6486).
    pub fn answers_to(&self, number: u32) -> bool {
        self.number == number || self.earlier.contains(&number)
    }
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

/// `ended`, or now where it stands in the future: a clock ahead wrote it, and an entry kept with
/// it would never age ([`sweep`]).
fn no_later_than_now(ended: &str) -> String {
    let now = chrono::Utc::now();
    match chrono::DateTime::parse_from_rfc3339(ended) {
        Ok(at) if at > now => now.to_rfc3339(),
        _ => ended.to_owned(),
    }
}

/// **Keeps `chat`, which has just closed for good**, in the project at `root`: the newest entry,
/// replacing one of the same ULID (a chat closed again) or the same number, then held to
/// [`MOST_KEPT`]. Refused for an id that is not a ULID or a time that is not one.
pub fn keep(root: &Path, chat: &ChatRecord) -> std::io::Result<()> {
    keep_all(root, std::slice::from_ref(chat))
}

/// **Keeps every chat in `chats`, which have just closed for good together** (#6487): a fresh
/// start's, or what a launch leaves out of its put back. [`keep`] for each, under one lock and
/// one write, the first in `chats` the newest. Refused whole where any does not hold together.
pub fn keep_all(root: &Path, chats: &[ChatRecord]) -> std::io::Result<()> {
    if chats.is_empty() {
        return Ok(());
    }
    if !chats.iter().all(holds_together) {
        return Err(std::io::Error::other(
            "a closed chat is kept by its ULID, with when it started and ended",
        ));
    }
    let chats: Vec<ChatRecord> = chats
        .iter()
        .map(|chat| ChatRecord {
            title: chat.title.chars().take(MOST_TITLE_CHARS).collect(),
            ended: no_later_than_now(&chat.ended),
            ..chat.clone()
        })
        .collect();
    let entries = chats
        .iter()
        .map(|chat| serde_json::to_value(OnDisk::of(chat)).map_err(std::io::Error::other))
        .collect::<std::io::Result<Vec<_>>>()?;
    let _held = locked(root)?;
    let others = read_values(&path(root)).into_iter().filter(|one| {
        let id = one.get("id").and_then(serde_json::Value::as_str);
        let number = one.get("number").and_then(serde_json::Value::as_u64);
        !chats
            .iter()
            .any(|chat| id == Some(chat.id.as_str()) || number == Some(u64::from(chat.number)))
    });
    write_all(root, entries.into_iter().chain(others))
}

/// **Takes away every chat that ended [`crate::retention::KEEP_FOR`] or more before `now`**,
/// and every entry that does not hold together, in the project at `root`: what a dispatch
/// record is swept at. An end that stands in the future is due at once: [`keep`] writes none,
/// so it was written by a clock ahead of this one, and when it ended cannot be told. Answers
/// how many went.
pub fn sweep(root: &Path, now: SystemTime) -> usize {
    if !std::fs::symlink_metadata(dir(root)).is_ok_and(|meta| meta.is_dir()) {
        return 0;
    }
    let Ok(_held) = locked(root) else {
        return 0;
    };
    // Under the lock no write is under way, so an old temp file here is a killed write's.
    remove_left_behind(root);
    let file = path(root);
    if !std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file()) {
        return 0;
    }
    let values = read_values(&file);
    let before = values.len();
    let kept: Vec<serde_json::Value> = values
        .into_iter()
        .filter(|value| sound(value.clone()).is_some_and(|chat| !due(&chat, now)))
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

/// Removes what a write of [`FILE_NAME`] killed before its rename left in the folder: the temp
/// file [`crate::rewrite::replace`] writes, `.purlis-generated.closed.json.<pid>.<tag>.tmp` (or
/// an older build's prefix), which holds chats' titles and nothing else ever removes. Called
/// under the folder's lock, which every write holds; as that lock may not have been taken, only
/// a temp file last written more than [`crate::rewrite::MOST_WAIT`] ago goes. Nothing else in
/// the folder is touched.
fn remove_left_behind(root: &Path) {
    let Ok(names) = std::fs::read_dir(dir(root)) else {
        return;
    };
    let now = SystemTime::now();
    for one in names.flatten() {
        // A lock that could not be taken (`Lock(None)`) lets a write run beside the sweep: a
        // temp file written within the longest wait for the lock may still be one.
        let stale = one
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|written| {
                now.duration_since(written)
                    .is_ok_and(|age| age > crate::rewrite::MOST_WAIT)
            });
        if stale
            && left_behind(&one.file_name().to_string_lossy())
            && let Err(why) = std::fs::remove_file(one.path())
        {
            tracing::warn!(error = %why, "a closed chats' temp file was not removed");
        }
    }
}

/// Whether `name` is a temp file a write of [`FILE_NAME`] leaves when it is cut short.
fn left_behind(name: &str) -> bool {
    crate::rewrite::temp_of(name).and_then(|temp| temp.of) == Some(FILE_NAME)
}

/// Whether `chat` is [`crate::retention::KEEP_FOR`] or more old at `now`, or ended after `now`.
fn due(chat: &ChatRecord, now: SystemTime) -> bool {
    chrono::DateTime::parse_from_rfc3339(&chat.ended).map_or(true, |ended| {
        now.duration_since(SystemTime::from(ended))
            .map_or(true, |age| age >= crate::retention::KEEP_FOR)
    })
}

#[cfg(test)]
mod tests;
