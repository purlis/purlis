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
//! - **What it knows is what the records know.** A chat the person opened and closed with no
//!   dispatch record leaves nothing here, and is "no chat".
//!
//! **One entry point** ([`by_number_in`]), the app's window command (`chat_by_number`) and the
//! pane's chat links (#6217) both call it. [`by_number`] is the same answer from records already
//! read.

use std::path::Path;

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
    /// No chat had it, or none that purlis kept a record of.
    NoChat,
}

/// **What `number` names in the project at `root`**, given the chats open now: one read of its
/// dispatch records.
pub fn by_number_in(root: &Path, number: u32, open: &[OpenChat]) -> Found {
    // An open chat is answered without reading a record.
    open_under(number, open).unwrap_or_else(|| not_open(number, open, &dispatchrecord::list(root)))
}

/// [`by_number_in`], from `records` read already, newest first as [`dispatchrecord::list`] lists
/// them.
pub fn by_number(number: u32, open: &[OpenChat], records: &[Record]) -> Found {
    open_under(number, open).unwrap_or_else(|| not_open(number, open, records))
}

/// What `number` names where no open chat answers to it.
fn not_open(number: u32, open: &[OpenChat], records: &[Record]) -> Found {
    let Some(record) = records.iter().find(|record| {
        record.worker.chat.chat == number
            && !dispatchrecord::never_a_chat(record)
            && dispatchrecord::sound(record)
    }) else {
        return Found::NoChat;
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
