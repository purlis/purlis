// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's number in a pane** (#6217): `chat 641`, the word and a number, and nothing else.
//! A title never links, and a bare `#641` is an issue's number, never a chat.
//!
//! - **The number is looked up in the pane's own project** ([`chatlookup::by_number_in`]), the
//!   lookup the window's `chat_by_number` asks: an open chat, one started again under another
//!   number (reached through its ULID), or one that ended, with the record its card is drawn
//!   from. A number no chat of this project had is not a link.
//! - **Its own number is this chat** (the ruling on #6211's Q3): a link whose hover says so, and
//!   opening it does nothing more. So is a number this chat had before it was started again.
//! - **Opening never asks**: it shows a chat or a card, and runs nothing.

use std::path::Path;

use crate::chatlookup::{self, Found, OpenChat};
use crate::dispatchrecord::Record;

use super::{Kind, LinkKind, Opening, Pane, Resolved, Target};

/// **Where a chat link goes**: the number it names now, and what that chat is.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatAt {
    /// The number the chat answers to now: the one written, or the one a restart dealt it.
    pub number: u32,
    pub goes: Goes,
    /// How it is shown and copied: `chat 641`, so a copy links again in a pane (D6).
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
    /// Open, under `now`: opening it shows it in its tab or workspace.
    Open { now: u32 },
    /// Ended: its newest dispatch record, which its card is drawn from.
    Ended(Box<Record>),
}

/// **The chat kind**, given the chats of the pane's project open now.
pub struct ChatNumber {
    open: Vec<OpenChat>,
}

impl ChatNumber {
    /// The kind, with the chats open now in the project the panes it reads are in.
    pub fn with_open(open: Vec<OpenChat>) -> Self {
        Self { open }
    }
}

/// The most digits a chat's number is read from: a `u32` has ten.
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
        let Some(project) = pane.project.as_deref() else {
            return Resolved::NotALink("this pane is in no project to find a chat in".into());
        };
        match self.found(project, number, pane.session) {
            Some(at) => Resolved::Link(Target::Chat(Box::new(at))),
            None => Resolved::NotALink(format!("no chat {number} in this project")),
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
                ChatAt::new(now, Goes::Open { now })
            }
        };
        if Some(number) == mine {
            return Some(open_as(number));
        }
        match chatlookup::by_number_in(project, number, &self.open) {
            Found::Open { number } => Some(open_as(number)),
            Found::Restarted { now } => Some(open_as(now)),
            Found::Ended(record) => Some(ChatAt::new(number, Goes::Ended(record))),
            Found::NoChat => None,
        }
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
