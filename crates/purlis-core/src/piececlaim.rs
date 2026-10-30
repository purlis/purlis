// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch folder has one live writer** (#3901, from #3897 concern 2).
//!
//! After an app restart a coordinator took a live task for a dead one and dispatched a resume
//! chat; both rebased, committed and pushed in one branch folder before either noticed. So the
//! brokered writes of a branch folder (`purlis worktree commit`, `update` and `push`) are held
//! to **purlis's piece claim** ([`crate::pieces`]): the latest `claimed` line for the folder
//! names the chat that holds it, by its stable id ([`crate::pieces::Who::chat`]).
//!
//! - **A chat that writes in a folder no live chat holds takes it**: its claim is logged, and
//!   from then it is the holder. A folder cut for the person, or by the window, or claimed
//!   before chats were named in the log, is held by no chat.
//! - **A second chat's write in a folder a live chat holds is refused**, naming the holder and
//!   how to hand it over: the holder's `purlis worktree done` or `abandon`, or the person.
//! - **A claim whose chat has ended is released**: the chat is not open in this app, or its
//!   program has ended. So is one its holder, or the person, declared `done` or `abandoned`; a
//!   declaration by some other chat releases nothing.
//! - **Where the holder's liveness cannot be read, the write is refused** (fail closed): its
//!   chat has not come back since a restart (restoring, or waiting to start again), or no app
//!   answered for this chat at all.
//!
//! The claim and the check are made under one lock, an advisory lock on a file beside the
//! workspace's piece log taken by the app and by the command line alike, so of two chats
//! writing at once in a free folder one takes it and the other is refused, whichever process
//! each runs in. The log lives in the workspace, which chats
//! may write: this keeps two chats from writing one folder by accident, which is what happened,
//! and is not a wall against a chat set on going through it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::pieces::{self, Event, Who};
use crate::worktree::Located;

/// A chat the app has, as a claim's holder is looked for among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    /// Its stable id (ADR 0066).
    pub id: String,
    /// The app's number for it now: what the refusal names it by. None for a chat purlis is
    /// still bringing back that has no number yet.
    pub chat: Option<u32>,
    /// Whether its program runs here now. Not yet: it is restoring after a restart, or waiting
    /// to start again, and whether it still works cannot be told.
    pub back: bool,
}

/// What a chat would do in a branch folder, as a refusal names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Commit,
    Update,
    Push,
    /// Any of the three: what a dispatch into a held folder is warned of.
    Write,
}

impl Act {
    fn word(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Update => "update",
            Self::Push => "push",
            Self::Write => "commit, update or push",
        }
    }
}

/// Whether a chat may write in a branch folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Writer {
    /// No live chat holds it: the chat that writes takes it.
    Free,
    /// The chat that writes holds it.
    Yours,
    /// Another chat holds it, or may: the refusal, naming the holder and how to hand it over.
    Refused(String),
}

/// The chat a folder's claim names, where one does.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Holder {
    id: String,
    /// The number its claim was logged under, where it reads as one.
    number: Option<u32>,
    persona: Option<String>,
}

/// Who holds the branch folder `at` by its workspace's piece log, read in order:
///
/// - a `claimed` line with a `chat` makes that chat the holder;
/// - a `claimed` line with no `chat` (the window's, the person's, or one from before chats
///   were named) changes nothing: it holds nothing, and it lets no holder go;
/// - a `done` or `abandoned` line releases the holder only where it is the holder's own (its
///   `chat` is the holder's) or the person's (it has neither `chat` nor `session`). One from
///   any other chat, or with a `session` and no `chat`, releases nothing.
fn holder(plane: &Path, at: &Located) -> Option<Holder> {
    let text = |e: &Value, key: &str| {
        e.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let mut held: Option<Holder> = None;
    for e in pieces::events(plane, &at.workspace) {
        if text(&e, "piece").as_deref() != Some(at.piece.as_str())
            || text(&e, "repo").unwrap_or_default() != at.repo
        {
            continue;
        }
        let chat = text(&e, "chat");
        match text(&e, "event").as_deref() {
            Some("claimed") => {
                if let Some(id) = chat {
                    held = Some(Holder {
                        id,
                        number: text(&e, "session").and_then(|n| n.parse().ok()),
                        persona: text(&e, "persona"),
                    });
                }
            }
            Some("done" | "abandoned") => {
                let the_persons = chat.is_none() && text(&e, "session").is_none();
                let the_holders =
                    chat.is_some() && chat.as_deref() == held.as_ref().map(|h| h.id.as_str());
                if the_persons || the_holders {
                    held = None;
                }
            }
            _ => {}
        }
    }
    held
}

/// Whether the chat `me` (its stable id, where it has one) may `act` in the branch folder `at`,
/// by the piece log and `live`: the chats the app has, or `None` where no app said.
pub fn writer(
    plane: &Path,
    at: &Located,
    me: Option<&str>,
    live: Option<&[Live]>,
    act: Act,
) -> Writer {
    let Some(holder) = holder(plane, at) else {
        return Writer::Free;
    };
    if me == Some(holder.id.as_str()) {
        return Writer::Yours;
    }
    let act = act.word();
    let found = live.map(|live| live.iter().find(|one| one.id == holder.id));
    let named = |chat: String| match &holder.persona {
        Some(persona) => format!("{chat} ({})", crate::personas::one_line(persona)),
        None => chat,
    };
    let by_number = |number: Option<u32>| match number {
        Some(number) => format!("chat {number}"),
        None => "another chat".to_owned(),
    };
    let folder = format!("{}'s branch folder '{}'", at.repo, at.piece);
    let hand_over = |who: &str| {
        format!(
            "To hand it over, {who} runs `purlis worktree done`, or `purlis worktree abandon \
             \"<reason>\"`, in it; or the person closes {who}, or runs one of those in the \
             folder from their own terminal. Until then, work in a branch folder of your own \
             (`purlis worktree add`). Nothing was done"
        )
    };
    match found {
        // Not open here: it has ended, and its claim with it.
        Some(None) => Writer::Free,
        Some(Some(one)) if one.back => {
            let who = named(by_number(one.chat.or(holder.number)));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which is still at work there, so purlis will not \
                 {act} in it for a second chat: two chats writing one folder overwrite each \
                 other's work. {}",
                hand_over(&who)
            ))
        }
        Some(Some(one)) => {
            // A chat still coming back has no number of its own yet: its old one may be dealt.
            let who = named(match one.chat {
                Some(number) => format!("chat {number}"),
                None => "a chat purlis is still bringing back".to_owned(),
            });
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which has not come back since purlis restarted, \
                 so whether it still works there cannot be told, and purlis will not {act} in \
                 it for a second chat. {}",
                hand_over(&who)
            ))
        }
        None => {
            let who = named(by_number(holder.number));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, and whether that chat still works there cannot \
                 be told without the purlis app, which did not answer, so purlis will not {act} \
                 in it for a second chat. {}",
                hand_over(&who)
            ))
        }
    }
}

/// The file whose advisory lock every check-and-claim in a workspace holds: beside its piece
/// log, so the app and a command line in a chat take the same one.
fn lock_path(plane: &Path, ws: &str) -> PathBuf {
    pieces::dir_for(plane, ws).join(".claims.lock")
}

/// How long a check-and-claim waits on another's: a check and one appended line.
const A_CLAIM_WAITS_AT_MOST: Duration = Duration::from_secs(5);

/// Takes workspace `ws`'s claim lock, or says why not. The file is opened through no link.
fn locked(plane: &Path, ws: &str) -> Result<crate::filelock::Held, String> {
    let path = lock_path(plane, ws);
    let opened = (|| -> std::io::Result<std::fs::File> {
        crate::contain::writable(plane, &path).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(pieces::dir_for(plane, ws))?;
        let mut open = std::fs::OpenOptions::new();
        open.create(true).truncate(false).read(true).write(true);
        // Never waits to open: a FIFO a chat planted at the path opens at once, rather than
        // holding the broker until a reader comes, and is refused below. `flock` takes no
        // notice of the flag, so it stays.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
        }
        let file = open.open(&path)?;
        if !file.metadata()?.file_type().is_file() {
            return Err(std::io::Error::other(
                "what is at its lock's place is not a plain file",
            ));
        }
        Ok(file)
    })();
    opened
        .and_then(|file| {
            crate::filelock::lock_within(file, A_CLAIM_WAITS_AT_MOST, "the branch folder claims")
        })
        .map_err(|why| format!("purlis could not hold the branch folder claims ({why})"))
}

/// **The check every brokered write of a branch folder makes first** (#3901): `Ok` where `who`
/// may `act` in the branch folder `at`, having taken it where no live chat held it; else the
/// refusal. A chat with no stable id cannot be named as a holder, so it takes nothing, and
/// writes only where no live chat holds the folder. A claim that cannot be written, or a lock
/// that cannot be held, is a refusal: a write nobody holds the folder for is what this guards
/// against.
pub fn take(
    plane: &Path,
    at: &Located,
    who: &Who,
    live: Option<&[Live]>,
    act: Act,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let refused = |why: String| {
        format!(
            "{why}, so a second chat could not be kept out of {}'s branch folder '{}', and \
             purlis will not {} there. Nothing was done",
            at.repo,
            at.piece,
            act.word()
        )
    };
    let _one_at_a_time = locked(plane, &at.workspace).map_err(refused)?;
    match writer(plane, at, who.chat.as_deref(), live, act) {
        Writer::Yours => Ok(()),
        Writer::Refused(why) => Err(why),
        Writer::Free if who.chat.is_none() => Ok(()),
        Writer::Free => pieces::record(
            plane,
            &at.workspace,
            Event::Claimed,
            &at.repo,
            &at.piece,
            None,
            who,
            now,
        )
        .map(|_| ())
        .ok_or_else(|| refused("purlis could not record that this chat works there".to_owned())),
    }
}

#[cfg(test)]
mod tests;
