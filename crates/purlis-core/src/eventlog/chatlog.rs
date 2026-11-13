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
//!
//! **A new kind of story event needs a line in [`is_story`].** Only `trust.*` is taken as a
//! family; `run.started`, `run.ended` and the four kinds of stop ([`stops`]: `stop.block`,
//! `stop.hold`, `stop.loop_pause` and `ask.answered`) are taken by name. Any other kind, a new
//! `stop.*` or `run.*` one included, and one outside those families such as an ask being raised,
//! is left out of the timeline until [`is_story`] names it, and a new kind of stop is counted
//! only once [`Tally`] has a bucket for it.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use super::Event;
use super::stops::{self, Block, Qualifier};
use crate::sandbox::protection::InForce;

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

/// **A row the Chat log draws** of a chat's timeline (R-227, R-230, R-233). Phase 1 draws a
/// start, an end and each block (T1-29); the other rows are T1-28's, and a kind of story event
/// with no row here is drawn by none yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A start of the chat's own run that resolved the protection it runs under.
    Started {
        /// When it was written, in milliseconds since 1970, where its id says.
        at_ms: Option<u64>,
        /// Why the run began, as `run.started` says it (`start`, `reopen`, `fresh`, ...).
        cause: String,
        /// The protection it recorded; `None` where the event has none, says `unknown`, or
        /// has one of a shape this version cannot read (T1-06's three shapes).
        protection: Option<InForce>,
    },
    /// The end of the chat's own run, and its result.
    Ended {
        at_ms: Option<u64>,
        /// `completed`, `failed` or `stopped`.
        state: String,
        /// Why: `exited`, `host-crash`, or the stop's word.
        cause: String,
        /// The exit code, where the program left one.
        code: Option<i64>,
    },
    /// **A block** (R-230, T1-29): a call refused, by whom, how much its report is worth and at
    /// what, as its `stop.block` says; a sub-agent's included, since its stops are the chat's.
    Blocked {
        at_ms: Option<u64>,
        /// The block, or `None` for a body that does not read back as one ([`Tally`]'s
        /// `unreadable_blocks`): nothing says who refused it.
        block: Option<Block>,
        /// The harness's id for the refused call, from the tool event the block was written
        /// just after (`Recorder::tool_spooled`): what the app finds a reason it holds by
        /// ([`super::reasons::Reasons`]). None where no tool event of a call came just before.
        call: Option<String>,
        /// The chat's harness, by the word its latest `run.started` gave (`claude`, `codex`,
        /// `opencode`), or `unknown`: whose own check a `harness_check` block was.
        harness: String,
    },
}

/// The call id of `event`, a tool hook's event (`hook.*`) with a `call`.
fn call_of(event: &Event) -> Option<String> {
    if !event.kind.starts_with("hook.") {
        return None;
    }
    event.body.get("call")?.as_str().map(str::to_owned)
}

/// **The rows of chat `chat`'s Chat log** among `events`, oldest first: read from its
/// [`timeline`]'s events, so the log holds nothing a row does not.
///
/// **Only the chat's own runs start and end rows** (T1-27's ruling on T1-03's review). A run
/// with a `parent_run` is a sub-agent's: its start and end are the trace of the chat's work,
/// not its story, and its stops are still the chat's rows. A `/clear`'s run (and a sub-agent's)
/// carries a copy of what the run before it started under, so it is no start of its own; a
/// relaunch (`reopen`, `fresh`) resolved its protection anew, and is.
///
/// **A block finds its call by the event before it**: the writer writes a block just after its
/// call's tool event, under the same run and with nothing between (one line's events, written
/// together), so the chat's event before a block, under its run, is that call's.
pub fn rows(events: &[Event], chat: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut harness = super::UNKNOWN.to_owned();
    let mut before: Option<&Event> = None;
    for event in events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
    {
        let previous = before.replace(event);
        if !is_story(&event.kind) {
            continue;
        }
        let word = |field: &str| {
            event
                .body
                .get(field)
                .and_then(serde_json::Value::as_str)
                .unwrap_or(super::UNKNOWN)
                .to_owned()
        };
        let at_ms = super::written_at_ms(&event.ulid);
        if event.kind == "run.started" {
            harness = word("harness");
        }
        let own = event.parent_run.is_none();
        let row = match event.kind.as_str() {
            "run.started" if own => {
                let cause = word("cause");
                (!matches!(cause.as_str(), "clear" | "child")).then(|| Row::Started {
                    at_ms,
                    cause,
                    protection: event
                        .body
                        .get("protection")
                        .and_then(|said| serde_json::from_value(said.clone()).ok()),
                })
            }
            "run.ended" if own => Some(Row::Ended {
                at_ms,
                state: word("state"),
                cause: word("cause"),
                code: event.body.get("code").and_then(serde_json::Value::as_i64),
            }),
            stops::BLOCK => Some(Row::Blocked {
                at_ms,
                block: serde_json::from_value(event.body.clone()).ok(),
                call: previous
                    .filter(|previous| previous.run == event.run)
                    .and_then(call_of),
                harness: harness.clone(),
            }),
            _ => None,
        };
        rows.extend(row);
    }
    rows
}

/// **How many of each kind of stop.** A block whose report is qualified, `unverified` or
/// `cant_tell` (Q165, Q235, Q237), is counted apart from a plain one, since such a report never
/// counts on the sidebar, and each qualifier apart from the other. A `stop.block` whose body does
/// not read back as a [`Block`] is counted in a bucket of its own, `unreadable_blocks`: nothing
/// says which kind of block it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    /// Blocks with no qualifier.
    pub blocks: u32,
    /// Blocks reported `unverified`.
    pub unverified_blocks: u32,
    /// Blocks reported `cant_tell`.
    pub cant_tell_blocks: u32,
    /// Blocks whose body does not read back as a [`Block`].
    pub unreadable_blocks: u32,
    pub holds: u32,
    pub loop_pauses: u32,
    pub asks_answered: u32,
}

impl Tally {
    /// Counts `event` where it is a stop.
    fn add(&mut self, event: &Event) {
        let count = match event.kind.as_str() {
            stops::BLOCK => match serde_json::from_value::<Block>(event.body.clone()) {
                Ok(Block { qualifier, .. }) => match qualifier {
                    None => &mut self.blocks,
                    Some(Qualifier::Unverified) => &mut self.unverified_blocks,
                    Some(Qualifier::CantTell) => &mut self.cant_tell_blocks,
                },
                // A block that does not read back is not trusted to be any of those.
                Err(_) => &mut self.unreadable_blocks,
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
