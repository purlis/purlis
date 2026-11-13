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
//!   dispatch record names the number. It is matched to the open chats by ULID too.
//! - **What it knows is what the records know.** A number neither names, or one swept after 30
//!   days, is "no chat".
//!
//! **One entry point** ([`by_number_in`]), the app's window command (`chat_by_number`) and the
//! pane's chat links (#6217) both call it. [`by_number`] is the same answer from records already
//! read ([`Known`]).

use std::path::Path;

use crate::chatrecords::{self, ChatRecord};
use crate::dispatchrecord::{self, ChatRef, Record};

/// A chat the app has open now: the number it answers to, and its ULID where it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenChat {
    pub number: u32,
    pub id: Option<String>,
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

/// What `number` names where no open chat answers to it.
fn not_open(number: u32, open: &[OpenChat], known: &Known) -> Found {
    let Some(record) = known.dispatches.iter().find(|record| {
        record.worker.chat.chat == number
            && !dispatchrecord::never_a_chat(record)
            && dispatchrecord::sound(record)
    }) else {
        return closed(number, open, &known.chats);
    };
    if let Some(now) = open.iter().find(|chat| is(&record.worker.chat, chat)) {
        return Found::Restarted { now: now.number };
    }
    if record.running() {
        // Its record is open and no chat has it: a start still waiting, nothing ended to show.
        return Found::NoChat;
    }
    Found::Ended(Box::new(record.clone()))
}

/// What `number` names where no dispatch record does: a closed chat's record, or the chat open
/// again under another number, matched by its ULID.
fn closed(number: u32, open: &[OpenChat], chats: &[ChatRecord]) -> Found {
    let Some(chat) = chatrecords::of_number(chats, number) else {
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
