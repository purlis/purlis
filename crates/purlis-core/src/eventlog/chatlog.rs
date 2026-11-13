// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's timeline and the fleet counts are queries of the event log** (R-223, Q129, Q216):
//! the timeline the Chat log tab draws, and the counts of each kind of stop, per chat and across
//! chats, that the marker and the finish rows read.
//!
//! **No second write path.** Both are read from the one log through its read API
//! ([`super::read`]), and neither keeps anything of its own: an event the log gains shows in
//! both, and one it no longer holds shows in neither. Nothing is migrated: the shipped Activity
//! view ([`crate::activity`]) is still read from the dispatch records, with the chats' own words,
//! and the event log stays metadata-only.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use super::Event;
use super::stops::{self, Block};

/// **Whether an event of `kind` is part of a chat's story**: its runs starting and ending, its
/// stops ([`stops`]: blocks, holds, loop pauses and answers to asks), and the grants and
/// protection changes the log writes under it (`trust.*`). A tool hook's line and the log's own
/// housekeeping are not: the Chat log is the story, never the trace.
pub fn is_story(kind: &str) -> bool {
    matches!(kind, "run.started" | "run.ended") || is_stop(kind) || kind.starts_with("trust.")
}

/// **Chat `chat`'s timeline among `events`**: the events of its story ([`is_story`]), in the
/// order of the log, oldest first.
pub fn timeline(events: &[Event], chat: &str) -> Vec<Event> {
    events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat) && is_story(&event.kind))
        .cloned()
        .collect()
}

/// [`timeline`] of the log in `dir`, read through [`super::read`].
pub fn timeline_in(dir: &Path, chat: &str) -> io::Result<Vec<Event>> {
    Ok(timeline(&super::read(dir)?, chat))
}

/// **How many of each kind of stop.** A block whose report is qualified, `unverified` or
/// `cant_tell` (Q165, Q235, Q237), is counted apart from a plain one, since such a report never
/// counts on the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    /// Blocks with no qualifier.
    pub blocks: u32,
    /// Blocks reported `unverified` or `cant_tell`.
    pub unverified_blocks: u32,
    pub holds: u32,
    pub loop_pauses: u32,
    pub asks_answered: u32,
}

impl Tally {
    /// Counts `event` where it is a stop.
    fn add(&mut self, event: &Event) {
        let count = match event.kind.as_str() {
            stops::BLOCK => match serde_json::from_value::<Block>(event.body.clone()) {
                Ok(Block {
                    qualifier: None, ..
                }) => &mut self.blocks,
                // A block that does not read back is not trusted to be a plain one.
                _ => &mut self.unverified_blocks,
            },
            stops::HOLD => &mut self.holds,
            stops::LOOP_PAUSE => &mut self.loop_pauses,
            stops::ASK_ANSWERED => &mut self.asks_answered,
            _ => return,
        };
        *count = count.saturating_add(1);
    }
}

/// **The fleet counts**: each kind of stop across chats, and per chat by its id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counts {
    /// Every stop in the log, a chat's or of no chat.
    pub all: Tally,
    /// Each chat's, by its id; a chat with no stop is not listed.
    pub per_chat: BTreeMap<String, Tally>,
}

/// The fleet counts of `events`.
pub fn counts(events: &[Event]) -> Counts {
    let mut counts = Counts::default();
    for event in events {
        counts.all.add(event);
        if let Some(chat) = &event.chat
            && is_stop(&event.kind)
        {
            counts.per_chat.entry(chat.clone()).or_default().add(event);
        }
    }
    counts
}

fn is_stop(kind: &str) -> bool {
    matches!(
        kind,
        stops::BLOCK | stops::HOLD | stops::LOOP_PAUSE | stops::ASK_ANSWERED
    )
}

/// [`counts`] of the log in `dir`, read through [`super::read`].
pub fn counts_in(dir: &Path) -> io::Result<Counts> {
    Ok(counts(&super::read(dir)?))
}

#[cfg(test)]
#[path = "chatlog/tests.rs"]
mod tests;
