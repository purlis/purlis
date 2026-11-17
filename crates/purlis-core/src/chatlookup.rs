// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat, looked up by its number** (#6214, parent #5820): number → ULID → the chat's
//! record. It answers whether the chat is open, was started again under another number, or has
//! ended, and then with the dispatch record its card is drawn from. It answers "no chat" for a
//! number nothing names.
//!
//! - **The number is only the way in.** A number is never dealt twice in a project
//!   (`reopen::Record::dealt`), but a chat started again is dealt a new one and keeps its ULID
//!   (ADR 0066). So a number is resolved to the ULID that held it, and the ULID to what the chat
//!   is now. A dispatch record ([`dispatchrecord::ChatRef`]) holds both, and the open chats are
//!   matched to it by ULID ([`dispatchrecord::same_chat`]), never by number.
//! - **The newest record answers.** A number was one chat's, so every record that names it names
//!   that chat, and the newest is what it did last. A task that never had a chat
//!   ([`dispatchrecord::never_a_chat`]) names nobody, and a record that does not read as one the
//!   app wrote ([`dispatchrecord::sound`]) is passed over.
//! - **A chat no dispatch started has its own record** (#6304): the app keeps number → ULID →
//!   title, times and who started it as any chat closes for good ([`crate::chatrecords`]).
//!   A dispatch record is read first, as it is the fuller card; a chat record answers where no
//!   dispatch record names the number, or where the one that does draws no card: a handoff's,
//!   a task's that ended owing no report, or one still running. It is matched to the open
//!   chats by ULID too.
//! - **Every number a chat had answers** (#6486): a restart keeps the ULID and deals a new
//!   number, and the chat keeps the ones it had ([`OpenChat::earlier`], `reopen::Chat::earlier`,
//!   [`ChatRecord::earlier`]). An open chat that had the number is the chat it names; a closed
//!   one's record answers to its earlier numbers as to its last.
//! - **What it knows is what the records know.** A number neither names, or one swept after 30
//!   days, is "no chat".
//!
//! **One entry point** ([`by_number_in`]), the app's window command (`chat_by_number`) and the
//! pane's chat links (#6217) both call it. [`by_number`] is the same answer from records already
//! read ([`Known`]).

use std::path::Path;

use crate::chatrecords::{self, ChatRecord};
use crate::dispatchrecord::{self, ChatRef, Record};

/// A chat the app has open now: the number it answers to, its ULID where it has one, and the
/// numbers it had before a restart (#6486, `reopen::Chat::earlier`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenChat {
    pub number: u32,
    pub id: Option<String>,
    pub earlier: Vec<u32>,
}

/// What a number names.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// An open chat answers to it.
    Open { number: u32 },
    /// The chat it was is open now under another number: it was started again, or reopened.
    Restarted { now: u32 },
    /// The chat has ended: its newest dispatch record, which its card is drawn from.
    Ended(Box<Record>),
    /// A chat no dispatch started has closed: the app's record of it, which its card is drawn
    /// from. It kept no screen.
    Closed(Box<ChatRecord>),
    /// No chat had it, or none that purlis kept a record of.
    NoChat,
}

/// **What a project's records say of its chats**, read once: its dispatch records, newest first
/// as [`dispatchrecord::list`] lists them, and its closed chats ([`chatrecords::every`]).
#[derive(Debug, Clone, Default)]
pub struct Known {
    pub dispatches: Vec<Record>,
    pub chats: Vec<ChatRecord>,
}

impl Known {
    /// Both read from the project at `root`.
    pub fn read(root: &Path) -> Self {
        Self {
            dispatches: dispatchrecord::list(root),
            chats: chatrecords::every(root),
        }
    }
}

/// **What `number` names in the project at `root`**, given the chats open now: one read of its
/// records.
pub fn by_number_in(root: &Path, number: u32, open: &[OpenChat]) -> Found {
    // An open chat is answered without reading a record.
    open_under(number, open).unwrap_or_else(|| not_open(number, open, &Known::read(root)))
}

/// [`by_number_in`], from records read already.
pub fn by_number(number: u32, open: &[OpenChat], known: &Known) -> Found {
    open_under(number, open).unwrap_or_else(|| not_open(number, open, known))
}

/// **What the chat whose ULID is `id` is now** in the project at `root`, given the chats open
/// now (#6685's review): open again under a number, ended with the dispatch record that draws
/// its card, closed with its own record, or no chat. A row of the Chats list's filter opens its
/// chat by this, never by the number typed: more than one record may answer to a number where
/// it was dealt twice (#6740), and only the ULID is the chat's own.
pub fn by_id_in(root: &Path, id: &str, open: &[OpenChat]) -> Found {
    if let Some(now) = open.iter().find(|chat| chat.id.as_deref() == Some(id)) {
        return Found::Restarted { now: now.number };
    }
    let known = Known::read(root);
    if let Some(record) = known.dispatches.iter().find(|record| {
        record.worker.chat.id.as_deref() == Some(id)
            && dispatchrecord::sound(record)
            && makes_a_card(record)
    }) {
        return Found::Ended(Box::new(record.clone()));
    }
    known
        .chats
        .iter()
        .find(|chat| chat.id == id)
        .map_or(Found::NoChat, |chat| Found::Closed(Box::new(chat.clone())))
}

/// What `number` names where no open chat answers to it.
fn not_open(number: u32, open: &[OpenChat], known: &Known) -> Found {
    // A chat open under a later number had it before a restart (#6486): a number is never
    // dealt twice, so the chat that had it is that one.
    if let Some(now) = open.iter().find(|chat| chat.earlier.contains(&number)) {
        return Found::Restarted { now: now.number };
    }
    let Some(record) = known.dispatches.iter().find(|record| {
        record.worker.chat.chat == number
            && !dispatchrecord::never_a_chat(record)
            && dispatchrecord::sound(record)
    }) else {
        return closed(chatrecords::of_number(&known.chats, number), open);
    };
    if let Some(now) = open.iter().find(|chat| is(&record.worker.chat, chat)) {
        return Found::Restarted { now: now.number };
    }
    if !makes_a_card(record) {
        // A handoff's record, a task's that ended owing no report, or one still running draws
        // no card: the chat's own record, kept as it closed, is what answers for it. By the
        // record's ULID where it names one, so a number dealt again never shows another chat.
        let chat = match record.worker.chat.id.as_deref() {
            Some(id) => known.chats.iter().find(|chat| chat.id == id),
            None => chatrecords::of_number(&known.chats, number),
        };
        return closed(chat, open);
    }
    Found::Ended(Box::new(record.clone()))
}

/// **Whether a dispatch record makes a closed chat's card**: a finished task's
/// ([`dispatchrecord::Finished::of`]) whose chat had a ULID. The window's card is drawn from
/// such a record alone (`chatcard::card_of`).
pub fn makes_a_card(record: &Record) -> bool {
    record.worker.chat.id.is_some() && dispatchrecord::Finished::of(record).is_some()
}

/// What a closed chat's record `chat` answers where no dispatch record draws a card: its card,
/// or the chat open again under another number, matched by its ULID. No chat where there is no
/// record.
fn closed(chat: Option<&ChatRecord>, open: &[OpenChat]) -> Found {
    let Some(chat) = chat else {
        return Found::NoChat;
    };
    match open
        .iter()
        .find(|one| one.id.as_deref() == Some(chat.id.as_str()))
    {
        Some(now) => Found::Restarted { now: now.number },
        None => Found::Closed(Box::new(chat.clone())),
    }
}

/// **The chats that closed and are not open again** (#6484), in `chats` as
/// [`chatrecords::every`] read them, newest close first: what the Chats list's filter finds a
/// closed chat among. One open again under another number is that open chat's row, matched by
/// its ULID.
pub fn closed_now<'c>(chats: &'c [ChatRecord], open: &[OpenChat]) -> Vec<&'c ChatRecord> {
    chats
        .iter()
        .filter(|chat| {
            !open
                .iter()
                .any(|one| one.id.as_deref() == Some(chat.id.as_str()))
        })
        .collect()
}

fn open_under(number: u32, open: &[OpenChat]) -> Option<Found> {
    open.iter()
        .any(|chat| chat.number == number)
        .then_some(Found::Open { number })
}

/// **Whether the chat a record names is the open chat `chat`**: by its ULID alone, which a
/// restart and a relaunch keep. A record that names no ULID is no open chat's: a number is
/// only the way in ([`by_number_in`]).
pub fn is(recorded: &ChatRef, chat: &OpenChat) -> bool {
    recorded.id.is_some() && recorded.id == chat.id
}

#[cfg(test)]
mod tests;
