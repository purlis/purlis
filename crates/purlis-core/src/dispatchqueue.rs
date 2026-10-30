// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The messages a chat sent while the app did not answer**, kept until it does (#3902).
//!
//! A `tell`, `note`, `ask` or `answer` (the command or its tool) is written here first, durable
//! before the sender is answered, and then the app is asked to deliver what the chat queued
//! ([`crate::dispatched::What::Deliver`]). Where the app answers, it delivers each message and
//! removes it; where it does not, the message waits, and the app delivers it the next time it
//! hears from the chat (a hook line, any ask), when a chat waits on it (a coordinator's `wait`, a
//! task's wait for an answer), and when it opens the project, once the tasks are back. So a
//! message is delivered once (the app removes it as it delivers it, one drain of a chat at a
//! time, and never takes a number at or below the last one it delivered: [`floor`]) and in the
//! order its chat queued it (by number). A message the app finds it cannot deliver (the task
//! has finished, it is not the sender's task, it does not check) is removed and recorded
//! undeliverable, which the sender reads in `purlis dispatch list`.
//!
//! # Where, and who may write it
//!
//! In the hook spool's directory, `spool/messages/<chat>/` ([`dir_for`]), under the spool's
//! own integrity denial: a sandboxed chat neither reads nor writes it (ADR 0067 §5), so one
//! chat cannot remove or reorder another's. Each message is signed with the key the chat's
//! spool lines are signed with, derived from its token ([`crate::hookwire::spool::sign_as`]),
//! and the app checks it against the key it issued that chat
//! ([`crate::hookwire::spool::checks_as`]): nothing that does not hold the chat's token queues
//! a message as that chat. What may write here is what may write the spool: purlis's MCP
//! server, which a harness runs outside its sandbox, and the command where the chat is not
//! sandboxed. A command a sandbox confines cannot queue, and says so ([`Unqueued`]).
//!
//! # A file per message
//!
//! `<seq>.json`, its number (twenty digits) taken by making `<seq>.part` exclusively, written and
//! synced there, then renamed: a reader never sees half of one, and two senders of one chat never
//! take one number. A `.part` a sender left more than an hour ago is a dead sender's, and is
//! removed. Every name counts against [`MOST_QUEUED`]; a name of any other shape is not read.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The folder of the spool directory messages wait in.
pub const DIR: &str = "messages";

/// The most messages a chat may have waiting: one past it is refused, not queued.
pub const MOST_QUEUED: usize = 256;

/// The version of a queued message's file.
const VERSION: u32 = 1;

/// The most of a message's file that is read: a message's text is at most 4,096 bytes.
const A_FILE_IS_READ_UP_TO: u64 = 64 * 1024;

/// How long a `.part` is a sender's still writing it; past that it is a dead sender's.
const A_PART_IS_DEAD_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The digits a message's number is written with.
const DIGITS: usize = 20;

/// Where the app keeps what became of each chat's messages, and its instruction numbers
/// ([`crate::dispatchtalk::Talk::keep_at`]), beside the queues and under their denial.
pub const RECORDS: &str = "records.json";

/// What a queued message's MAC is over, ahead of its body: so no spool line's MAC is ever a
/// message's, or the other way round.
const LABEL: &str = "purlis dispatch message v1\n";

/// What a chat queued: the ask it would have made of the app, as the app will make it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "what")]
pub enum Queued {
    /// A follow-up to task `to`.
    Tell { to: u32, text: String },
    /// A follow-up to every live task of the sender (#3904).
    TellAll { text: String },
    /// A progress note to the sender's asking chat, and the instruction it acknowledges.
    Note {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ack: Option<u32>,
    },
    /// A question to the sender's asking chat.
    Question { text: String },
    /// An answer to task `to`'s question.
    Answer { to: u32, text: String },
}

/// One message, as its file holds it.
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    v: u32,
    seq: u64,
    key: String,
    body: String,
    mac: String,
}

/// What a file's `body` is: the MAC is over these exact bytes.
#[derive(serde::Serialize, serde::Deserialize)]
struct Body {
    id: String,
    at: i64,
    #[serde(flatten)]
    what: Queued,
}

/// A message waiting to be delivered, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// Its number in its chat's queue: the order it is delivered in.
    pub seq: u64,
    /// Its id, which the sender was told and its delivery is recorded under.
    pub id: String,
    /// When it was queued, in seconds since the epoch.
    pub at: i64,
    pub what: Queued,
    /// The id of the key it was signed under: what its floor is kept by.
    key: String,
    path: PathBuf,
}

/// What a look at a chat's queue found, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A message that checked.
    Message(Waiting),
    /// A file that is not a message this chat queued: it does not read, or its MAC does not
    /// check under a key the app issued the chat. It is removed, never delivered.
    Unchecked(PathBuf),
    /// A message that checks, numbered at or below the last one the app delivered: a copy of one
    /// delivered already, or one an app that stopped as it delivered it left behind. It is
    /// removed, never delivered again.
    Delivered(PathBuf),
}

/// Why a message was not queued.
#[derive(Debug)]
pub enum Unqueued {
    /// This process may not write the queue: a sandbox keeps it from it, or the hook socket is
    /// not in a project's state folder.
    Unreachable(io::Error),
    /// [`MOST_QUEUED`] are waiting already.
    Full,
}

impl Unqueued {
    /// What the sender is told.
    pub fn say(&self) -> String {
        match self {
            Self::Unreachable(_) => "this chat cannot reach purlis's message queue (its sandbox \
                                     keeps it out), so nothing was queued"
                .to_owned(),
            Self::Full => format!(
                "this chat already has {MOST_QUEUED} messages waiting for the app, so this one \
                 was not queued"
            ),
        }
    }
}

/// **Whether purlis's `dispatch_*` tools queue a message in a chat of `harness`** where its
/// sandbox keeps the `purlis dispatch` command from the queue (#3914), by the harness the chat's
/// environment names ([`crate::hookwire::chat_harness`]; none is Claude Code's).
///
/// The ruling on #3914 grants a sandboxed command no way into the queue: the tools are what
/// queue, because a harness that runs its MCP servers outside the sandbox it puts a command in
/// runs purlis's there too. Claude Code does. Codex and opencode do not: purlis's wrap is around
/// the whole harness, its MCP servers with it
/// ([`crate::harness::HarnessAdapter::sandbox_holds_what_it_starts`]), so there the tools are
/// kept from the queue as the command is, and a sender is never pointed to them for it. A
/// harness purlis ships no adapter for is promised nothing.
pub fn tools_queue(harness: Option<&str>) -> bool {
    crate::harness::Harness::of_chat(harness)
        .is_some_and(|harness| !harness.adapter().sandbox_holds_what_it_starts())
}

/// Where chat `chat`'s queue is, beside the hook socket `socket`.
pub fn dir_for(socket: &Path, chat: u32) -> PathBuf {
    queues_for(socket).join(chat.to_string())
}

/// Where the app keeps what became of the messages ([`RECORDS`]), beside `socket`.
pub fn records_for(socket: &Path) -> PathBuf {
    queues_for(socket).join(RECORDS)
}

/// Where every chat's queue is.
fn queues_for(socket: &Path) -> PathBuf {
    spool_dir(socket).join(DIR)
}

fn spool_dir(socket: &Path) -> PathBuf {
    socket
        .parent()
        .map_or_else(|| PathBuf::from("spool"), |parent| parent.join("spool"))
}

/// **Queues `what` from chat `chat`**, signed with `token`'s spool key, durable before it
/// returns: the message's id.
pub fn queue(
    socket: &Path,
    chat: u32,
    token: &crate::hookwire::ChatToken,
    what: Queued,
    at: i64,
) -> Result<String, Unqueued> {
    queued(socket, chat, token, what, at).map_err(|why| match why.kind() {
        io::ErrorKind::StorageFull => Unqueued::Full,
        _ => Unqueued::Unreachable(why),
    })
}

fn queued(
    socket: &Path,
    chat: u32,
    token: &crate::hookwire::ChatToken,
    what: Queued,
    at: i64,
) -> io::Result<String> {
    let spool = spool_dir(socket);
    if !covered(&spool) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the hook socket is not in a project's state folder",
        ));
    }
    let dir = dir_for(socket, chat);
    crate::secrets::make_private_dir(&queues_for(socket))?;
    crate::secrets::make_private_dir(&dir)?;
    crate::contain::no_link_on_the_way(&spool, &dir)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let body = serde_json::to_string(&Body {
        id: id.clone(),
        at,
        what,
    })
    .map_err(io::Error::other)?;
    let numbers = numbers(&dir)?;
    if numbers.len() >= MOST_QUEUED {
        return Err(io::Error::from(io::ErrorKind::StorageFull));
    }
    // Past every number in the folder, and past the last one the app delivered: a number
    // never goes back.
    let start = numbers.iter().map(|(seq, _)| *seq).max().unwrap_or(0);
    let (key, _) = sign(token, chat, 0, "");
    let (seq, part) = claim(&dir, start, &|| floor_of(socket, chat, &key))?;
    let (key, mac) = sign(token, chat, seq, &body);
    written(
        &dir,
        seq,
        part,
        &File {
            v: VERSION,
            seq,
            key,
            body,
            mac,
        },
    )?;
    Ok(id)
}

/// **Takes the next free number past `start` and past the floor** in `dir`, by making its `.part`
/// exclusively: the number and the part. `floor` is read again once the part is made, so a
/// number a drain delivered and freed meanwhile (the floor raised to it, its file removed) is
/// never taken again: a message under it would be read as delivered already, and dropped.
fn claim(dir: &Path, start: u64, floor: &dyn Fn() -> u64) -> io::Result<(u64, std::fs::File)> {
    let mut seq = start.max(floor());
    loop {
        seq += 1;
        let part = dir.join(name(seq, "part"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)
        {
            Ok(file) => {
                // Taken already by a sender that finished, or at or below what a drain has
                // delivered since this sender looked: let go of it, and go on past both.
                let delivered = floor();
                if dir.join(name(seq, "json")).exists() || seq <= delivered {
                    let _ = std::fs::remove_file(&part);
                    seq = seq.max(delivered);
                    continue;
                }
                return Ok((seq, file));
            }
            Err(why) if why.kind() == io::ErrorKind::AlreadyExists => {}
            Err(why) => return Err(why),
        }
    }
}

/// **Writes `file` into the part numbered `seq` and links it whole as `<seq>.json`**. A part that
/// could not be written is removed, so it holds up no later message of the chat's.
fn written(dir: &Path, seq: u64, mut part: std::fs::File, file: &File) -> io::Result<()> {
    let part_path = dir.join(name(seq, "part"));
    let done = (|| {
        part.write_all(&serde_json::to_vec(file).map_err(io::Error::other)?)?;
        part.sync_all()?;
        // Linked, never renamed over: a name that is there already is never replaced.
        std::fs::hard_link(&part_path, dir.join(name(seq, "json")))
    })();
    let _ = std::fs::remove_file(&part_path);
    done?;
    if let Ok(folder) = std::fs::File::open(dir) {
        let _ = folder.sync_all();
    }
    Ok(())
}

/// **Moves what chat `old` has queued to chat `new`** (#3902): the same chat, started again
/// under a new number in its own place. Each message that checks is written again under `new`,
/// in its order, signed with the key the app keeps for `new`, and removed from `old`; what does
/// not check is left for `old`'s end to clear. Answers how many moved.
pub fn moved(socket: &Path, old: u32, new: u32) -> usize {
    if old == new || !holds(socket, old) {
        return 0;
    }
    let dir = dir_for(socket, new);
    if crate::secrets::make_private_dir(&dir).is_err() {
        return 0;
    }
    let mut count = 0;
    for found in waiting(socket, old) {
        let Found::Message(message) = found else {
            continue;
        };
        let Ok(body) = serde_json::to_string(&Body {
            id: message.id.clone(),
            at: message.at,
            what: message.what.clone(),
        }) else {
            continue;
        };
        let Some(key) = kept_key(socket, new) else {
            break;
        };
        let start = numbers(&dir)
            .map(|numbers| numbers.iter().map(|(seq, _)| *seq).max().unwrap_or(0))
            .unwrap_or(0);
        let Ok((seq, part)) = claim(&dir, start, &|| floor_of(socket, new, &key)) else {
            break;
        };
        let Some((key, mac)) = signed_kept(socket, new, seq, &body) else {
            break;
        };
        let file = File {
            v: VERSION,
            seq,
            key,
            body,
            mac,
        };
        if written(&dir, seq, part, &file).is_err() {
            break;
        }
        let _ = std::fs::remove_file(&message.path);
        count += 1;
    }
    count
}

/// The last number chat `chat`'s messages under key `key` had delivered.
#[cfg(unix)]
fn floor_of(socket: &Path, chat: u32, key: &str) -> u64 {
    crate::hookwire::spool::message_floor(&spool_dir(socket), chat, key)
}

#[cfg(not(unix))]
fn floor_of(_: &Path, _: u32, _: &str) -> u64 {
    0
}

/// The id of the key the app keeps for chat `chat` now: the last one it issued it.
#[cfg(unix)]
fn kept_key(socket: &Path, chat: u32) -> Option<String> {
    crate::hookwire::spool::kept_key_of(&spool_dir(socket), chat)
}

#[cfg(not(unix))]
fn kept_key(_: &Path, _: u32) -> Option<String> {
    None
}

/// `body`, number `seq` of chat `chat`, signed under the key the app keeps for it.
#[cfg(unix)]
fn signed_kept(socket: &Path, chat: u32, seq: u64, body: &str) -> Option<(String, String)> {
    crate::hookwire::spool::sign_kept(&spool_dir(socket), chat, seq, &format!("{LABEL}{body}"))
}

#[cfg(not(unix))]
fn signed_kept(_: &Path, _: u32, _: u64, _: &str) -> Option<(String, String)> {
    None
}

/// A message's file name: its number in [`DIGITS`] digits, and `ext`.
fn name(seq: u64, ext: &str) -> String {
    format!("{seq:0width$}.{ext}", width = DIGITS)
}

/// The number a file name is a message's under, and whether it is a `.part` still: only the
/// names [`name`] writes.
fn named(name: &str) -> Option<(u64, bool)> {
    let (seq, part) = match name.strip_suffix(".json") {
        Some(seq) => (seq, false),
        None => (name.strip_suffix(".part")?, true),
    };
    if seq.len() != DIGITS || !seq.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((seq.parse().ok()?, part))
}

/// The numbers in a queue's folder, each with whether it is a `.part` still. A `.part` older
/// than [`A_PART_IS_DEAD_AFTER`] is a dead sender's, and is removed, not listed.
fn numbers(dir: &Path) -> io::Result<Vec<(u64, bool)>> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let (seq, part) = named(entry.file_name().to_str()?)?;
            let dead = part
                && entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|at| at.elapsed().ok())
                    .is_some_and(|age| age > A_PART_IS_DEAD_AFTER);
            if dead {
                let _ = std::fs::remove_file(entry.path());
                return None;
            }
            Some((seq, part))
        })
        .collect())
}

/// **Every message chat `chat` has waiting, oldest first**, each checked against the keys the
/// app issued that chat. Nothing is removed: [`delivered`] and [`drop_unchecked`] do that.
pub fn waiting(socket: &Path, chat: u32) -> Vec<Found> {
    waiting_in(&spool_dir(socket), chat)
}

/// [`waiting`], in the spool directory `spool`.
fn waiting_in(spool: &Path, chat: u32) -> Vec<Found> {
    let dir = spool.join(DIR).join(chat.to_string());
    if crate::contain::no_link_on_the_way(spool, &dir).is_err() {
        return Vec::new();
    }
    let Ok(mut numbers) = numbers(&dir) else {
        return Vec::new();
    };
    // **Never past a message still being written**: a sender that took a lower number than
    // another is still writing it, and one delivered after it would raise the floor over it.
    // What comes after the lowest `.part` waits for the next look.
    let writing = numbers
        .iter()
        .filter(|(_, part)| *part)
        .map(|(seq, _)| *seq)
        .min();
    numbers.retain(|(seq, part)| !part && writing.is_none_or(|first| *seq < first));
    numbers.sort_unstable();
    numbers
        .into_iter()
        .map(|(seq, _)| {
            let path = dir.join(name(seq, "json"));
            match read(spool, chat, seq, &path) {
                Some(Looked::Fresh(message)) => Found::Message(message),
                Some(Looked::Delivered) => Found::Delivered(path),
                None => Found::Unchecked(path),
            }
        })
        .collect()
}

/// What a message's file read as, where it checked.
enum Looked {
    Fresh(Waiting),
    /// At or below the floor: delivered already.
    Delivered,
}

fn read(spool: &Path, chat: u32, seq: u64, path: &Path) -> Option<Looked> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(A_FILE_IS_READ_UP_TO + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > A_FILE_IS_READ_UP_TO {
        return None;
    }
    let file: File = serde_json::from_slice(&bytes).ok()?;
    if file.v != VERSION
        || file.seq != seq
        || !checks(spool, chat, seq, &file.key, &file.body, &file.mac)
    {
        return None;
    }
    if seq <= delivered_floor(spool, chat, &file.key) {
        return Some(Looked::Delivered);
    }
    let body: Body = serde_json::from_str(&file.body).ok()?;
    Some(Looked::Fresh(Waiting {
        seq,
        id: body.id,
        at: body.at,
        what: body.what,
        key: file.key,
        path: path.to_path_buf(),
    }))
}

#[cfg(unix)]
fn delivered_floor(spool: &Path, chat: u32, key: &str) -> u64 {
    crate::hookwire::spool::message_floor(spool, chat, key)
}

#[cfg(not(unix))]
fn delivered_floor(_: &Path, _: u32, _: &str) -> u64 {
    0
}

/// Whether chat `chat` has a message waiting: one look at its folder, for a caller that drains
/// only where there is something to drain.
pub fn holds(socket: &Path, chat: u32) -> bool {
    std::fs::read_dir(dir_for(socket, chat)).is_ok_and(|mut entries| {
        entries.any(|entry| {
            entry.is_ok_and(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .and_then(named)
                    .is_some_and(|(_, part)| !part)
            })
        })
    })
}

/// The chats that have a folder of waiting messages: what the app drains as a project opens.
pub fn chats(socket: &Path) -> Vec<u32> {
    chats_in(&spool_dir(socket))
}

/// [`chats`], in the spool directory `spool`.
pub(crate) fn chats_in(spool: &Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(spool.join(DIR)) else {
        return Vec::new();
    };
    let mut chats: Vec<u32> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.parse::<u32>().ok().filter(|n| n.to_string() == name))
        .collect();
    chats.sort_unstable();
    chats
}

/// **The message has been delivered, or found undeliverable**: the floor of chat `chat`'s
/// numbers is raised to it, and then it is removed, so neither it nor a copy of it is ever
/// delivered again.
pub fn delivered(socket: &Path, chat: u32, message: &Waiting) {
    raise(socket, chat, message);
    let _ = std::fs::remove_file(&message.path);
}

#[cfg(unix)]
fn raise(socket: &Path, chat: u32, message: &Waiting) {
    if let Err(why) = crate::hookwire::spool::raise_message_floor(
        &spool_dir(socket),
        chat,
        &message.key,
        message.seq,
    ) {
        tracing::warn!(
            "purlis: the floor of chat {chat}'s queued messages was not raised ({why}); the \
             message is removed all the same"
        );
    }
}

#[cfg(not(unix))]
fn raise(_: &Path, _: u32, _: &Waiting) {}

/// The folder of the messages folder where what an ended chat left queued is reported (#3958).
pub const LEFT: &str = "left";

/// The most of a report's file that is read: [`MOST_QUEUED`] messages of at most 4,096 bytes'
/// text, with room.
const A_REPORT_IS_READ_UP_TO: u64 = 4 * 1024 * 1024;

/// **What a chat left queued as it ended without the app delivering it** (#3958): its messages,
/// read from their own files while its key still checked them, kept until the app records each
/// where the person reads it ([`left`], [`reported`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Left {
    v: u32,
    /// The chat that queued them, by the number it had then.
    pub sender: u32,
    /// Its name, where the app found one for it ([`unplaced`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// When its queue was cleared, in seconds since the epoch.
    pub at: i64,
    /// What it queued, oldest first.
    pub messages: Vec<LeftMessage>,
}

/// One message of a [`Left`] report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeftMessage {
    /// Its id, which its sender was told.
    pub id: String,
    /// When it was queued, in seconds since the epoch.
    pub at: i64,
    #[serde(flatten)]
    pub what: Queued,
}

/// **Sets aside what chat `chat` left queued in the spool directory `spool`**, as its keys are
/// about to be dropped (#3958, [`crate::hookwire::spool::forget_all_but`]): every message that
/// checks under them, and is above what the app delivered, is written to one report under
/// [`LEFT`] before the queue is cleared. Answers how many were reported. What does not check is
/// no message of the chat's, and is not reported.
pub(crate) fn set_aside(spool: &Path, chat: u32, at: i64) -> io::Result<usize> {
    let messages: Vec<LeftMessage> = waiting_in(spool, chat)
        .into_iter()
        .filter_map(|found| match found {
            Found::Message(message) => Some(LeftMessage {
                id: message.id,
                at: message.at,
                what: message.what,
            }),
            Found::Unchecked(_) | Found::Delivered(_) => None,
        })
        .collect();
    if messages.is_empty() {
        return Ok(0);
    }
    let count = messages.len();
    let folder = spool.join(DIR).join(LEFT);
    crate::secrets::make_private_dir(&folder)?;
    crate::contain::no_link_on_the_way(spool, &folder)?;
    let report = Left {
        v: VERSION,
        sender: chat,
        name: None,
        at,
        messages,
    };
    let name = format!("{chat}.{}.json", uuid::Uuid::new_v4().simple());
    whole(
        &folder.join(name),
        &serde_json::to_vec(&report).map_err(io::Error::other)?,
    )
    .map(|()| count)
}

/// **Writes `bytes` to `path` whole**: into a `.part` beside it, synced, then renamed over it,
/// so a reader never sees half of one. A part that could not be written is removed.
fn whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let part = path.with_extension("part");
    let done = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&part, path)
    })();
    if done.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    done
}

/// **Every report of what an ended chat left queued**, beside `socket` (#3958): each with its
/// file, for [`reported`] once it is recorded. A file that does not read is left alone.
pub fn left(socket: &Path) -> Vec<(PathBuf, Left)> {
    reports(socket, true)
}

/// The reports under [`LEFT`]: those not looked at yet (`fresh`), or those [`unplaced`] kept.
fn reports(socket: &Path, fresh: bool) -> Vec<(PathBuf, Left)> {
    use std::io::Read;
    let spool = spool_dir(socket);
    let folder = queues_for(socket).join(LEFT);
    if crate::contain::no_link_on_the_way(&spool, &folder).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut reports: Vec<(PathBuf, Left)> =
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_str().is_some_and(|name| {
                    name.ends_with(".json") && name.ends_with(UNPLACED) != fresh
                }) && entry.file_type().is_ok_and(|kind| kind.is_file())
            })
            .filter_map(|entry| {
                let mut bytes = Vec::new();
                std::fs::File::open(entry.path())
                    .ok()?
                    .take(A_REPORT_IS_READ_UP_TO + 1)
                    .read_to_end(&mut bytes)
                    .ok()?;
                if bytes.len() as u64 > A_REPORT_IS_READ_UP_TO {
                    return None;
                }
                let report: Left = serde_json::from_slice(&bytes).ok()?;
                (report.v == VERSION).then(|| (entry.path(), report))
            })
            .collect();
    reports.sort_by_key(|(path, report)| (report.at, path.clone()));
    reports
}

/// A report has been recorded where the person reads it: it is removed.
pub fn reported(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// How a report's file ends once the app found no dispatch record for some of its messages.
const UNPLACED: &str = ".unplaced.json";

/// **The messages of report `path` the app found no running dispatch record for** (#3958), kept
/// for the person as one notice ([`unplaced_reports`]) until they put it away
/// ([`put_away`]), with the name the app found for the sender. The report itself goes.
pub fn unplaced(path: &Path, report: &Left, messages: Vec<LeftMessage>, name: Option<String>) {
    let Some(stem) = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".json"))
    else {
        return;
    };
    let kept = Left {
        v: VERSION,
        sender: report.sender,
        name,
        at: report.at,
        messages,
    };
    let to = path.with_file_name(format!("{stem}{UNPLACED}"));
    let written = serde_json::to_vec(&kept)
        .map_err(io::Error::other)
        .and_then(|bytes| whole(&to, &bytes));
    match written {
        Ok(()) => reported(path),
        Err(why) => tracing::warn!(
            "purlis: what chat {} left queued is reported at the next open ({why})",
            report.sender
        ),
    }
}

/// **The reports the person is told of as a notice** ([`unplaced`]), beside `socket`: each by
/// the name [`put_away`] takes.
pub fn unplaced_reports(socket: &Path) -> Vec<(String, Left)> {
    reports(socket, false)
        .into_iter()
        .filter_map(|(path, report)| Some((path.file_name()?.to_str()?.to_owned(), report)))
        .collect()
}

/// **The person put the notice of report `name` away** ([`unplaced_reports`]): it is removed. A
/// name that is not one of those reports' removes nothing.
pub fn put_away(socket: &Path, name: &str) -> io::Result<()> {
    if !unplaced_reports(socket).iter().any(|(one, _)| one == name) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "there is no such notice of undelivered messages",
        ));
    }
    std::fs::remove_file(queues_for(socket).join(LEFT).join(name))
}

/// A file that did not check is removed.
pub fn drop_unchecked(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(unix)]
fn covered(spool: &Path) -> bool {
    crate::hookwire::spool::covered(spool)
}

#[cfg(not(unix))]
fn covered(_: &Path) -> bool {
    false
}

#[cfg(unix)]
fn sign(token: &crate::hookwire::ChatToken, chat: u32, seq: u64, body: &str) -> (String, String) {
    crate::hookwire::spool::sign_as(token, chat, seq, &format!("{LABEL}{body}"))
}

#[cfg(not(unix))]
fn sign(_: &crate::hookwire::ChatToken, _: u32, _: u64, _: &str) -> (String, String) {
    (String::new(), String::new())
}

#[cfg(unix)]
fn checks(spool: &Path, chat: u32, seq: u64, key: &str, body: &str, mac: &str) -> bool {
    crate::hookwire::spool::checks_as(spool, chat, seq, key, &format!("{LABEL}{body}"), mac)
}

#[cfg(not(unix))]
fn checks(_: &Path, _: u32, _: u64, _: &str, _: &str, _: &str) -> bool {
    false
}

#[cfg(all(test, unix))]
mod tests;
