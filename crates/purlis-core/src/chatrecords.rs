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
//!   launch's Start fresh, left out of a launch's put back, or let go with *Forget this chat*
//!   after a launch could not start it (#6487, [`keep_all`]). Not at a
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
//! - **Read as the app wrote it**: one file of at most [`MOST_FILE_BYTES`], opened from the
//!   project's root following no link on the way or at the file, as [`crate::reopen`] opens the
//!   record of open chats, and written whole through a temp file at 0600 under the folder's
//!   lock. An entry that does not hold together (an id that is not a ULID, a time that is not
//!   one) is left out, and a file that is not one purlis wrote reads as empty and is never
//!   written over: the oldest closes go first, so purlis never writes one too large to read.
//! - **What starts from it**: a closed chat's *Reopen* starts the program its entry keeps
//!   ([`ChatRecord::reopened`]), on the person's press in the window alone (the app's
//!   `reopen_closed_chat`, which no chat may ask for), as the record of open chats starts what
//!   it keeps at a launch. That is why it is opened as that record is, and denied to every
//!   sandboxed chat. Nothing ends or allows anything by it. Lost, those chats lose their card
//!   and their Reopen.
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

/// The longest title, place or name of the chat that started one kept, in characters, as it is
/// written and as it is read: a tab's name is far shorter.
const MOST_TITLE_CHARS: usize = 200;

/// `words`, held to [`MOST_TITLE_CHARS`].
fn held_words(words: &str) -> String {
    words.chars().take(MOST_TITLE_CHARS).collect()
}

impl StartedBy {
    /// Who started it, the name of a chat held to [`MOST_TITLE_CHARS`].
    fn held(self) -> Self {
        match self {
            Self::Person => Self::Person,
            Self::Chat { name } => Self::Chat {
                name: held_words(&name),
            },
        }
    }
}

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
    /// (#6486): each is looked up to it too. At most [`crate::reopen::MOST_EARLIER`].
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

    /// **The number this chat was found by, where it is not the one it closed under**
    /// (#6685): `asked`, where it is one it had before a restart, so what shows it can say
    /// `(was #702)` beside `#731`. The one place this rule is written for the core and the app.
    pub fn found_by(&self, asked: u32) -> Option<u32> {
        (asked != self.number && self.earlier.contains(&asked)).then_some(asked)
    }

    /// Whether this chat had `number`, as it closed or before a restart (#6486).
    pub fn answers_to(&self, number: u32) -> bool {
        self.number == number || self.earlier.contains(&number)
    }
}

/// **What a closed chat keeps for its Reopen** (#6485): `chat` as it closed, without what was
/// its run's or its place's alone. No process, run, tab or pane; no work left running; and no
/// dispatch it came from, so a handoff or a task reopens as an ordinary chat that owes nobody a
/// report. Its program, words, folder, conversation, persona, profile, name and sandbox choices
/// stay.
///
/// **A chat a dispatch started is still held to what its dispatch was held to**: it keeps
/// `reopened` ([`crate::reopen::Chat::reopened`]), from the dispatch it came from or as it was
/// already kept, so its Reopen and every later start of it ask its profile again
/// ([`crate::dispatchprofiles::may_start_again`]), as a finished task's Reopen does.
pub fn for_a_reopen(chat: &crate::reopen::Chat) -> crate::reopen::Chat {
    // Every field named, so a field added to a chat is decided here: kept for its Reopen, or
    // its run's or its place's alone.
    let crate::reopen::Chat {
        program,
        args,
        cwd,
        name,
        resume,
        active: _,
        profile,
        persona,
        show_footer,
        pinned,
        number,
        earlier,
        label,
        from,
        reopened,
        held: _,
        tab_opened: _,
        unseen: _,
        graded,
        held_from_clusters: _,
        unattended: _,
        shows: _,
        beside: _,
        renamed_from,
        runs_on,
        unsandboxed,
        no_sandbox,
        identity,
        pid: _,
        model,
        plane_skills,
        routing,
        task_level,
        unrecorded,
        studio: _,
        own_work: _,
        own_pieces: _,
        mid_turn: _,
        kept_tmp: _,
    } = chat.clone();
    let crate::reopen::Identity {
        id,
        device,
        run: _,
        resumed_from,
    } = identity;
    crate::reopen::Chat {
        program,
        args,
        cwd,
        name,
        resume,
        profile,
        persona,
        show_footer,
        pinned,
        number,
        earlier,
        label,
        // A dispatch it came from is held as a reopened task's, which owes nobody a report.
        reopened: from
            .map(|from| crate::reopen::ReopenedTask {
                by_person: from.by_person,
            })
            .or(reopened),
        graded,
        renamed_from,
        runs_on,
        unsandboxed,
        no_sandbox,
        identity: crate::reopen::Identity {
            id,
            device,
            resumed_from,
            ..Default::default()
        },
        model,
        plane_skills,
        routing,
        task_level,
        unrecorded,
        ..Default::default()
    }
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
            earlier: crate::reopen::held_earlier(&chat.earlier),
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
    str::parse::<jiff::Timestamp>(time).is_ok()
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
        title: held_words(&one.title),
        started: one.started,
        ended: one.ended,
        started_by: one.started_by.held(),
        place: held_words(&one.place),
        earlier: crate::reopen::held_earlier(&one.earlier),
        chat: one.chat.and_then(crate::reopen::chat_from_json),
    };
    holds_together(&chat).then_some(chat)
}

/// **What the file in the project at `root` holds**, as it reads.
enum Read {
    /// No file: no chat has closed for good yet.
    Absent,
    /// Its entries, newest close first.
    Kept(Vec<serde_json::Value>),
    /// A file that is not one this purlis writes: reached through a link, not a plain file,
    /// larger than [`MOST_FILE_BYTES`], not JSON, or of another version. It reads as empty and
    /// is never written over, so what it holds is not lost to the next close.
    Foreign,
}

impl Read {
    fn entries(self) -> Vec<serde_json::Value> {
        match self {
            Self::Kept(entries) => entries,
            Self::Absent | Self::Foreign => Vec::new(),
        }
    }
}

/// The file in the project at `root`, as it reads. Opened as the record of open chats is
/// ([`crate::reopen`]): from the project's root, following no link on the way or at the file,
/// and what it is asked of the descriptor the read uses.
fn read_values(root: &Path) -> Read {
    crate::fence::hold(crate::fence::Act::Read, root);
    let file = path(root);
    let open = match crate::contain::open_no_link(root, &file) {
        Ok(open) => open,
        Err(gone) if gone.kind() == std::io::ErrorKind::NotFound => return Read::Absent,
        Err(_) => return Read::Foreign,
    };
    let usable = open
        .metadata()
        .and_then(|found| crate::reopen::refuse_unusable_over(&file, &found, MOST_FILE_BYTES));
    if usable.is_err() {
        return Read::Foreign;
    }
    let mut text = String::new();
    if open
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Read::Foreign;
    }
    match serde_json::from_str::<File>(&text) {
        Ok(kept) if kept.v == VERSION => Read::Kept(kept.chats),
        _ => Read::Foreign,
    }
}

/// **Every chat closed in the project at `root`** that this store keeps, newest close first.
pub fn every(root: &Path) -> Vec<ChatRecord> {
    read_values(root)
        .entries()
        .into_iter()
        .filter_map(sound)
        .take(MOST_KEPT)
        .collect()
}

/// **The highest number a closed chat in the project at `root` answers to** (#6740): the one it
/// closed under or one it had before a restart, or 0 where none closed. A launch holds the
/// counter of numbers dealt (`reopen::Record::dealt`) at or above it, so a number a closed
/// record still answers to is never dealt to a new chat, even where the record of open chats was
/// lost.
pub fn highest_number(root: &Path) -> u32 {
    every(root)
        .iter()
        .flat_map(|chat| std::iter::once(chat.number).chain(chat.earlier.iter().copied()))
        .max()
        .unwrap_or(0)
}

/// **The chat that closed under `number`**, or had it before a restart (#6486), in `chats` as
/// [`every`] read them: the newest close, as a number is never dealt twice in a project
/// (`reopen::Record::dealt`).
pub fn of_number(chats: &[ChatRecord], number: u32) -> Option<&ChatRecord> {
    chats.iter().find(|chat| chat.answers_to(number))
}

/// Writes `chats`, newest first, whole, under the folder's lock already held: at most
/// [`MOST_KEPT`] of them and [`MOST_FILE_BYTES`] in all, the oldest close going first, so the
/// file is never larger than it is read.
fn write_all(root: &Path, chats: impl Iterator<Item = serde_json::Value>) -> std::io::Result<()> {
    let chats: Vec<serde_json::Value> = chats.take(MOST_KEPT).collect();
    let empty = serde_json::to_string(&File {
        v: VERSION,
        chats: Vec::new(),
    })
    .map_err(std::io::Error::other)?;
    let mut bytes = empty.len();
    let mut fit = 0;
    for (n, one) in chats.iter().enumerate() {
        let comma = usize::from(n > 0);
        let this = serde_json::to_string(one)
            .map_err(std::io::Error::other)?
            .len()
            + comma;
        if u64::try_from(bytes + this).map_or(true, |all| all > MOST_FILE_BYTES) {
            break;
        }
        bytes += this;
        fit += 1;
    }
    let chats = chats.into_iter().take(fit).collect();
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
    let now = jiff::Timestamp::now();
    match str::parse::<jiff::Timestamp>(ended) {
        Ok(at) if at > now => crate::oldspelling::rfc3339(now),
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
/// one write, the first in `chats` the newest. Refused whole where any does not hold together,
/// and where the file there is not one this purlis writes ([`Read::Foreign`]).
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
            title: held_words(&chat.title),
            place: held_words(&chat.place),
            started_by: chat.started_by.clone().held(),
            ended: no_later_than_now(&chat.ended),
            ..chat.clone()
        })
        .collect();
    let entries = chats
        .iter()
        .map(|chat| serde_json::to_value(OnDisk::of(chat)).map_err(std::io::Error::other))
        .collect::<std::io::Result<Vec<_>>>()?;
    let _held = locked(root)?;
    let others = match read_values(root) {
        Read::Absent => Vec::new(),
        Read::Kept(entries) => entries,
        Read::Foreign => {
            return Err(std::io::Error::other(
                "the closed chats' records are not a file this purlis writes, and are left as \
                 they are",
            ));
        }
    };
    let others = others.into_iter().filter(|one| {
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
    if !std::fs::symlink_metadata(path(root)).is_ok_and(|meta| meta.is_file()) {
        return 0;
    }
    // A file this purlis did not write is never written over, so it is not swept either.
    let Read::Kept(values) = read_values(root) else {
        return 0;
    };
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
    str::parse::<jiff::Timestamp>(&chat.ended).map_or(true, |ended| {
        now.duration_since(SystemTime::from(ended))
            .map_or(true, |age| age >= crate::retention::KEEP_FOR)
    })
}

#[cfg(test)]
mod tests;
