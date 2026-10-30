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
    let dir = dir_for(socket, chat);
    if crate::contain::no_link_on_the_way(&spool_dir(socket), &dir).is_err() {
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
            match read(socket, chat, seq, &path) {
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

fn read(socket: &Path, chat: u32, seq: u64, path: &Path) -> Option<Looked> {
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
        || !checks(socket, chat, seq, &file.key, &file.body, &file.mac)
    {
        return None;
    }
    if seq <= delivered_floor(socket, chat, &file.key) {
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
fn delivered_floor(socket: &Path, chat: u32, key: &str) -> u64 {
    crate::hookwire::spool::message_floor(&spool_dir(socket), chat, key)
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
    let Ok(entries) = std::fs::read_dir(queues_for(socket)) else {
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
fn checks(socket: &Path, chat: u32, seq: u64, key: &str, body: &str, mac: &str) -> bool {
    crate::hookwire::spool::checks_as(
        &spool_dir(socket),
        chat,
        seq,
        key,
        &format!("{LABEL}{body}"),
        mac,
    )
}

#[cfg(not(unix))]
fn checks(_: &Path, _: u32, _: u64, _: &str, _: &str, _: &str) -> bool {
    false
}

#[cfg(all(test, unix))]
mod tests;
