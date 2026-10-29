// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The messages a chat sent while the app did not answer**, kept until it does (#3902).
//!
//! A `tell`, `note`, `ask` or `answer` (the command or its tool) is written here first, durable
//! before the sender is answered, and then the app is asked to deliver what the chat queued
//! ([`crate::dispatched::What::Deliver`]). Where the app answers, it delivers each message and
//! removes it; where it does not, the message waits, and the app delivers it the next time it
//! hears from the chat (a hook line, any ask) or opens the project. So a message is delivered
//! once (the app removes it as it delivers it, one drain at a time) and in the order its chat
//! queued it (by number). A message the app finds it cannot deliver (the task has ended, the
//! chat is not the sender's task) is removed and recorded undeliverable, which the sender reads
//! in `purlis dispatch list`.
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
//! `<seq>.json`, its number taken by making `<seq>.part` exclusively, written and synced there,
//! then renamed: a reader never sees half of one, and two senders of one chat never take one
//! number.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The folder of the spool directory messages wait in.
pub const DIR: &str = "messages";

/// The most messages a chat may have waiting: one past it is refused, not queued.
pub const MOST_QUEUED: usize = 256;

/// The version of a queued message's file.
const VERSION: u32 = 1;

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
    if numbers.iter().filter(|(_, part)| !part).count() >= MOST_QUEUED {
        return Err(io::Error::from(io::ErrorKind::StorageFull));
    }
    let mut seq = numbers.iter().map(|(seq, _)| *seq).max().unwrap_or(0);
    // A number taken by another sender since the listing is passed over.
    let (seq, mut part) = loop {
        seq += 1;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(format!("{seq:020}.part")))
        {
            Ok(file) => break (seq, file),
            Err(why) if why.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(why) => return Err(why),
        }
    };
    let (key, mac) = sign(token, chat, seq, &body);
    let file = File {
        v: VERSION,
        seq,
        key,
        body,
        mac,
    };
    part.write_all(&serde_json::to_vec(&file).map_err(io::Error::other)?)?;
    part.sync_all()?;
    std::fs::rename(
        dir.join(format!("{seq:020}.part")),
        dir.join(format!("{seq:020}.json")),
    )?;
    if let Ok(folder) = std::fs::File::open(&dir) {
        let _ = folder.sync_all();
    }
    Ok(id)
}

/// The numbers in a queue's folder, each with whether it is a `.part` still.
fn numbers(dir: &Path) -> io::Result<Vec<(u64, bool)>> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let (seq, part) = match name.strip_suffix(".json") {
                Some(seq) => (seq, false),
                None => (name.strip_suffix(".part")?, true),
            };
            Some((seq.parse().ok()?, part))
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
    numbers.retain(|(_, part)| !part);
    numbers.sort_unstable();
    numbers
        .into_iter()
        .map(|(seq, _)| {
            let path = dir.join(format!("{seq:020}.json"));
            read(socket, chat, seq, &path).map_or(Found::Unchecked(path), Found::Message)
        })
        .collect()
}

fn read(socket: &Path, chat: u32, seq: u64, path: &Path) -> Option<Waiting> {
    let bytes = std::fs::read(path).ok()?;
    let file: File = serde_json::from_slice(&bytes).ok()?;
    if file.v != VERSION
        || file.seq != seq
        || !checks(socket, chat, seq, &file.key, &file.body, &file.mac)
    {
        return None;
    }
    let body: Body = serde_json::from_str(&file.body).ok()?;
    Some(Waiting {
        seq,
        id: body.id,
        at: body.at,
        what: body.what,
        path: path.to_path_buf(),
    })
}

/// Whether chat `chat` has a message waiting: one look at its folder, for a caller that drains
/// only where there is something to drain.
pub fn holds(socket: &Path, chat: u32) -> bool {
    numbers(&dir_for(socket, chat)).is_ok_and(|numbers| numbers.iter().any(|(_, part)| !part))
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

/// The message has been delivered, or found undeliverable: it is removed, so it is never
/// delivered again.
pub fn delivered(message: &Waiting) {
    let _ = std::fs::remove_file(&message.path);
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
