// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's number in a pane** (#6217): `chat 641`, the word and a number, and nothing else.
//! A title never links, and a bare `#641` is an issue's number, never a chat.
//!
//! - **The number is looked up in the pane's own project** ([`chatlookup::by_number_in`]), the
//!   lookup the window's `chat_by_number` asks: an open chat, one started again under another
//!   number (reached through its ULID), or one that ended, with the record its card is drawn
//!   from. A number no chat of this project had is not a link, and says nothing on a hover
//!   ([`Resolved::Quiet`]): a harness that prints `chat 12` of another project is not wrong.
//! - **Its own number is this chat** (the ruling on #6211's Q3): a link whose hover says so, and
//!   opening it does nothing more. So is a number this chat had before it was started again.
//! - **Opening never asks**: it shows a chat or a card, and runs nothing.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::chatlookup::{self, Found, Known, OpenChat};
use crate::chatrecords::ChatRecord;
use crate::dispatchrecord::Record;

use super::{Kind, LinkKind, Opening, Pane, Resolved, Target};

/// **Where a chat link goes**: the number it names now, and what that chat is.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatAt {
    /// The number the chat answers to now: the one written, or the one a restart dealt it.
    pub number: u32,
    pub goes: Goes,
    /// How it is shown and copied: `chat 641`, so a copy links again in a pane (the
    /// prototype's D6).
    shown: String,
}

impl ChatAt {
    fn new(number: u32, goes: Goes) -> Self {
        Self {
            number,
            goes,
            shown: format!("chat {number}"),
        }
    }

    /// `chat 641`.
    pub fn shown(&self) -> &str {
        &self.shown
    }
}

/// **What the chat a link names is.**
#[derive(Debug, Clone, PartialEq)]
pub enum Goes {
    /// The chat the pane draws: opening it does nothing more.
    ThisChat,
    /// Open, under [`ChatAt::number`]: opening it shows it in its tab or workspace.
    Open,
    /// Ended: its newest dispatch record, which its card is drawn from.
    Ended(Box<Record>),
    /// A chat no dispatch started has closed (#6304): the app's record of it, which its card is
    /// drawn from.
    Closed(Box<ChatRecord>),
}

/// **The chat kind**, given the chats of the pane's project open now. The project's dispatch
/// records and closed chats' records ([`Known`]) are read once, the first time a number no open chat answers to is looked up, and
/// kept for the rest of the batch: one resolver is made for each line or open it answers.
pub struct ChatNumber {
    open: Vec<OpenChat>,
    records: OnceLock<(PathBuf, Known)>,
}

impl ChatNumber {
    /// The kind, with the chats open now in the project the panes it reads are in.
    pub fn with_open(open: Vec<OpenChat>) -> Self {
        Self {
            open,
            records: OnceLock::new(),
        }
    }

    /// What `number` names in `project`: the records read once, where they are that project's.
    fn lookup(&self, project: &Path, number: u32) -> Found {
        let (read_in, records) = self
            .records
            .get_or_init(|| (project.to_owned(), Known::read(project)));
        if read_in == project {
            chatlookup::by_number(number, &self.open, records)
        } else {
            chatlookup::by_number_in(project, number, &self.open)
        }
    }
}

/// The most digits a chat's number is read from: a `u32` has ten. The pane's pattern
/// (`app/src/paneLinks.ts`, `CHAT`: `[1-9]\d{0,8}`) reads no more, and no leading zero either.
const MOST_DIGITS: usize = 9;

/// The number `text` names, where it reads `chat 641` (or `Chat 641`) and nothing more.
pub fn number_in(text: &str) -> Option<u32> {
    let digits = text
        .strip_prefix("chat ")
        .or_else(|| text.strip_prefix("Chat "))?;
    if digits.is_empty()
        || digits.len() > MOST_DIGITS
        || digits.starts_with('0')
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    digits.parse().ok()
}

impl LinkKind for ChatNumber {
    fn kind(&self) -> Kind {
        Kind::Chat
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        let Some(number) = number_in(text) else {
            return Resolved::NotALink("that is not a chat's number".into());
        };
        // A pane in no project (a saved chat's screen) has no chats to find.
        let Some(project) = pane.project.as_deref() else {
            return Resolved::Quiet;
        };
        match self.found(project, number, pane.session) {
            Some(at) => Resolved::Link(Target::Chat(Box::new(at))),
            None => Resolved::Quiet,
        }
    }

    fn opening(&self, target: Target, _text: &str, _pane: &Pane) -> Opening {
        Opening::Open(target)
    }
}

impl ChatNumber {
    /// What `number` names in `project`, seen from the pane drawing chat `mine`.
    fn found(&self, project: &Path, number: u32, mine: Option<u32>) -> Option<ChatAt> {
        let open_as = |now: u32| {
            if Some(now) == mine {
                ChatAt::new(now, Goes::ThisChat)
            } else {
                ChatAt::new(now, Goes::Open)
            }
        };
        if Some(number) == mine {
            return Some(open_as(number));
        }
        // An open chat is answered without reading a record.
        if self.open.iter().any(|chat| chat.number == number) {
            return Some(open_as(number));
        }
        match self.lookup(project, number) {
            Found::Open { number } => Some(open_as(number)),
            Found::Restarted { now } => Some(open_as(now)),
            Found::Ended(record) => Some(ChatAt::new(number, Goes::Ended(record))),
            Found::Closed(chat) => Some(ChatAt::new(number, Goes::Closed(chat))),
            Found::NoChat => None,
        }
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
