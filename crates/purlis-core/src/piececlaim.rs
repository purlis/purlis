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
//! The claim and the check are made under one lock, so of two chats writing at once in a free
//! folder one takes it and the other is refused. The log lives in the workspace, which chats
//! may write: this keeps two chats from writing one folder by accident, which is what happened,
//! and is not a wall against a chat set on going through it.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::pieces::{self, Event, Who};

/// A chat the app has, as a claim's holder is looked for among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    /// Its stable id (ADR 0066).
    pub id: String,
    /// The app's number for it now: what the refusal names it by.
    pub chat: u32,
    /// Whether its program runs here now. Not yet: it is restoring after a restart, or waiting
    /// to start again, and whether it still works cannot be told.
    pub back: bool,
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

/// Who holds `(repo, piece)` by workspace `ws`'s piece log, read in order: the latest `claimed`
/// line names a chat, or none; a `done` or `abandoned` by that chat, or by the person, lets it go.
fn holder(plane: &Path, ws: &str, repo: &str, piece: &str) -> Option<Holder> {
    let text = |e: &Value, key: &str| {
        e.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let mut held: Option<Holder> = None;
    for e in pieces::events(plane, ws) {
        if text(&e, "piece").as_deref() != Some(piece)
            || text(&e, "repo").unwrap_or_default() != repo
        {
            continue;
        }
        match text(&e, "event").as_deref() {
            Some("claimed") => {
                held = text(&e, "chat").map(|id| Holder {
                    id,
                    number: text(&e, "session").and_then(|n| n.parse().ok()),
                    persona: text(&e, "persona"),
                });
            }
            Some("done" | "abandoned") => {
                let by = text(&e, "chat");
                if by.is_none() || by.as_deref() == held.as_ref().map(|h| h.id.as_str()) {
                    held = None;
                }
            }
            _ => {}
        }
    }
    held
}

/// Whether the chat `me` (its stable id, where it has one) may `act` (`commit`, `update`,
/// `push`) in `repo`'s branch folder `piece` of workspace `ws`, by the piece log and `live`:
/// the chats the app has, or `None` where no app said.
pub fn writer(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    me: Option<&str>,
    live: Option<&[Live]>,
    act: &str,
) -> Writer {
    let Some(holder) = holder(plane, ws, repo, piece) else {
        return Writer::Free;
    };
    if me == Some(holder.id.as_str()) {
        return Writer::Yours;
    }
    let found = live.map(|live| live.iter().find(|one| one.id == holder.id));
    let named = |number: Option<u32>| {
        let chat = match number.or(holder.number) {
            Some(number) => format!("chat {number}"),
            None => "another chat".to_owned(),
        };
        match &holder.persona {
            Some(persona) => format!("{chat} ({})", crate::personas::one_line(persona)),
            None => chat,
        }
    };
    let folder = format!("{repo}'s branch folder '{piece}'");
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
            let who = named(Some(one.chat));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which is still at work there, so purlis will not \
                 {act} in it for a second chat: two chats writing one folder overwrite each \
                 other's work. {}",
                hand_over(&who)
            ))
        }
        Some(Some(one)) => {
            let who = named(Some(one.chat));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which has not come back since purlis restarted, \
                 so whether it still works there cannot be told, and purlis will not {act} in \
                 it for a second chat. {}",
                hand_over(&who)
            ))
        }
        None => {
            let who = named(None);
            Writer::Refused(format!(
                "{folder} is claimed by {who}, and whether that chat still works there cannot \
                 be told without the purlis app, which did not answer, so purlis will not {act} \
                 in it for a second chat. {}",
                hand_over(&who)
            ))
        }
    }
}

/// One check-and-claim at a time, so two chats writing at once in a free folder do not both
/// take it. One app, one process: a lock here is every brokered writer's.
static CLAIMING: Mutex<()> = Mutex::new(());

/// **The check every brokered write of a branch folder makes first** (#3901): `Ok` where `who`
/// may `act` in `repo`'s folder `piece` of workspace `ws`, having taken it where no live chat
/// held it; else the refusal. A chat with no stable id cannot be named as a holder, so it
/// takes nothing, and writes only where no live chat holds the folder. A claim that cannot be
/// written is a refusal: a write nobody holds the folder for is what this guards against.
#[allow(clippy::too_many_arguments)]
pub fn take(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: &str,
    who: &Who,
    live: Option<&[Live]>,
    act: &str,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let _one_at_a_time = CLAIMING.lock().unwrap_or_else(PoisonError::into_inner);
    match writer(plane, ws, repo, piece, who.chat.as_deref(), live, act) {
        Writer::Yours => Ok(()),
        Writer::Refused(why) => Err(why),
        Writer::Free if who.chat.is_none() => Ok(()),
        Writer::Free => pieces::record(plane, ws, Event::Claimed, repo, piece, None, who, now)
            .map(|_| ())
            .ok_or_else(|| {
                format!(
                    "purlis could not record that this chat works in {repo}'s branch folder \
                     '{piece}', so a second chat could not be kept out of it, and it will not \
                     {act} there. Nothing was done"
                )
            }),
    }
}

#[cfg(test)]
#[path = "piececlaim_tests.rs"]
mod tests;
