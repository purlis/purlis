// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The store's summary of the dispatches that have ended** (#5502, spec #5486):
//! `app/dispatches/ended.json`, one small file, so a caller that needs only the running tasks
//! reads the running records and this, not every record in full. The sidebar's task status
//! and each turn's roster are two such callers; a project that has run thousands of tasks
//! pays for the few still running.
//!
//! **It holds only what never changes once a dispatch has ended**, so it needs no keeping in
//! step with the records' later writes. A dispatch ends once and never runs again
//! ([`super::close`]), and what an entry says (its id, the chat that worked it, the chat that
//! asked for it, and whether that was never a chat, [`super::never_a_chat`]) is written before
//! or at its end and never after. Everything else, its report included, is read from the record itself where a
//! caller needs it ([`Ledger::latest_for`]). Nothing a chat wrote is in it: no brief, no
//! report, no talk.
//!
//! **Derived, and rebuilt by reading.** The record files stay the truth. A read lists the
//! store's file names (it does that to find the running records anyway), takes an entry's word
//! for each name the summary holds, reads every other record in full, and writes the summary
//! again where it found a dispatch ended that it did not hold or holds one that is gone. So a
//! summary deleted, torn or written by an older build costs one full read, and a record
//! collected by [`crate::retention`] leaves it at once ([`in_step`]). Two readers racing to
//! write it can each leave out what the other found; the next read finds it again.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{ChatRef, Record};

/// The summary's file name in the store's folder: not a ULID, so never read as a record.
pub const SUMMARY_NAME: &str = "ended.json";

/// The summary's version: one of another is read as no summary, and rebuilt.
/// Version 2 added [`Entry::asker_id`]: one of version 1 is rebuilt once.
const SUMMARY_VERSION: u32 = 2;

/// One ended dispatch, as the summary keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    id: String,
    /// The worker chat's number when the dispatch started.
    chat: u32,
    /// The worker chat's ULID, absent for a chat given none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chat_id: Option<String>,
    /// The ULID of the chat that asked for it, absent for one given none: what a chat's list
    /// of its finished tasks is filtered by before a record is read ([`super::asked_by`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    asker_id: Option<String>,
    /// [`super::never_a_chat`]: a task that never started and was never a chat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    never_a_chat: bool,
}

impl Entry {
    fn of(record: &Record) -> Self {
        Self {
            id: record.id.clone(),
            chat: record.worker.chat.chat,
            chat_id: record.worker.chat.id.clone(),
            asker_id: record.asker.chat.id.clone(),
            never_a_chat: super::never_a_chat(record),
        }
    }

    /// Whether this dispatch was worked by `chat`, as [`super::same_chat`] matches.
    fn worked_by(&self, chat: &ChatRef) -> bool {
        !self.never_a_chat
            && super::named(
                &ChatRef {
                    chat: self.chat,
                    id: self.chat_id.clone(),
                    ..ChatRef::default()
                },
                chat.id.as_deref(),
                Some(chat.chat),
            )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Summary {
    v: u32,
    /// Newest first, as the store lists them.
    ended: Vec<Entry>,
}

/// **The store as a caller that needs only the running tasks reads it** (#5502): the running
/// records in full, and the ended ones by the summary's entry. Newest first, as
/// [`super::list`] lists them.
#[derive(Debug, Default)]
pub struct Ledger {
    running: Vec<Record>,
    ended: Vec<Entry>,
}

/// [`Ledger`] of the project at `root`, writing the summary again where the store has moved
/// past it. A summary that cannot be written is no error: the next read tries again.
pub fn ledger(root: &Path) -> Ledger {
    let ids = super::ids(root);
    let mut kept = held(root);
    let mut ledger = Ledger::default();
    let mut found = false;
    for id in &ids {
        if let Some(entry) = kept.remove(id) {
            ledger.ended.push(entry);
            continue;
        }
        let Some(record) = super::read(root, id) else {
            continue;
        };
        if record.running() {
            ledger.running.push(record);
        } else {
            ledger.ended.push(Entry::of(&record));
            found = true;
        }
    }
    // What is left in `kept` names a record that is no longer there.
    if found || !kept.is_empty() {
        let _ = write(root, &ledger.ended);
    }
    ledger
}

/// The running records of the project at `root`, newest first, as stored ([`ledger`]).
pub fn running(root: &Path) -> Vec<Record> {
    ledger(root).running
}

impl Ledger {
    /// The running records, newest first, as stored.
    pub fn running(&self) -> &[Record] {
        &self.running
    }

    /// The ids of the dispatches that have ended, newest first.
    pub fn ended(&self) -> impl Iterator<Item = &str> {
        self.ended.iter().map(|entry| entry.id.as_str())
    }

    /// **The newest dispatch whose persona chat is `chat`, running or ended**, as
    /// [`super::latest_for`] answers: an ended one is read whole, and only that one.
    pub fn latest_for(&self, root: &Path, chat: &ChatRef) -> Option<Record> {
        let mut running = self
            .running
            .iter()
            .filter(|record| {
                !super::never_a_chat(record) && super::same_chat(&record.worker.chat, chat)
            })
            .peekable();
        let mut ended = self
            .ended
            .iter()
            .filter(|entry| entry.worked_by(chat))
            .peekable();
        loop {
            // Both are newest first, and an id sorts by the time it was minted.
            let newer_runs = match (running.peek(), ended.peek()) {
                (None, None) => return None,
                (Some(record), Some(entry)) => record.id > entry.id,
                (some, _) => some.is_some(),
            };
            if newer_runs {
                return running.next().cloned();
            }
            let entry = ended.next()?;
            // One whose file no longer reads is passed over, as [`super::list`] passes it.
            if let Some(record) = super::read(root, &entry.id) {
                return Some(record);
            }
        }
    }
}

impl Ledger {
    /// **[`super::finished_for`], from this one reading of the store** (felt speed round 2):
    /// the running records it holds, and of the ended ones only those whose entry says `asker`
    /// asked for them are read. Oldest first, as [`super::finished_for`] answers.
    pub fn finished_for(
        &self,
        root: &Path,
        asker: &ChatRef,
        open: impl Fn(&ChatRef) -> bool,
    ) -> Vec<Record> {
        if asker.id.is_none() {
            // Listed under nobody ([`super::asked_by`]).
            return Vec::new();
        }
        let ended = self
            .ended
            .iter()
            .filter(|entry| entry.asker_id == asker.id)
            .filter_map(|entry| super::read(root, &entry.id));
        let mut finished: Vec<Record> = self
            .running
            .iter()
            .cloned()
            .chain(ended)
            .filter(|record| super::asked_by(record, asker) && super::finished_now(record, &open))
            .collect();
        finished.sort_by(|a, b| a.id.cmp(&b.id));
        finished
    }
}

/// The ids the summary of the project at `root` holds, where it reads.
pub(super) fn ended_ids(root: &Path) -> impl Iterator<Item = String> {
    held(root).into_keys()
}

/// **Keeps the summary of the project at `root` in step with its records** after
/// [`crate::retention`] collected some: an entry whose record is gone is dropped.
pub(crate) fn in_step(root: &Path) {
    let Some(summary) = summary(root) else {
        return;
    };
    let ids: std::collections::HashSet<String> = super::ids(root).into_iter().collect();
    let kept: Vec<Entry> = summary
        .ended
        .iter()
        .filter(|entry| ids.contains(&entry.id))
        .cloned()
        .collect();
    if kept.len() != summary.ended.len() {
        let _ = write(root, &kept);
    }
}

fn path(root: &Path) -> std::path::PathBuf {
    super::dir(root).join(SUMMARY_NAME)
}

/// The summary as stored, where it reads as this version's: read as a record is, through no
/// link and at most [`crate::reopen::MAX_BYTES`] of it. A summary past that is no summary.
fn summary(root: &Path) -> Option<Summary> {
    use std::io::Read;
    let path = path(root);
    let file = crate::contain::open_no_link(root, &path).ok()?;
    if !file.metadata().ok()?.file_type().is_file() {
        return None;
    }
    let mut text = String::new();
    file.take(SUMMARY_MOST_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    serde_json::from_str::<Summary>(&text)
        .ok()
        .filter(|summary| summary.v == SUMMARY_VERSION)
}

/// The most of the summary read: room for about a million entries.
const SUMMARY_MOST_BYTES: u64 = 128 * 1024 * 1024;

/// The summary's entries by id; none where it does not read.
fn held(root: &Path) -> HashMap<String, Entry> {
    summary(root)
        .map(|summary| {
            summary
                .ended
                .into_iter()
                .filter(|entry| super::an_id(&entry.id))
                .map(|entry| (entry.id.clone(), entry))
                .collect()
        })
        .unwrap_or_default()
}

/// Writes the summary whole, replacing it.
fn write(root: &Path, ended: &[Entry]) -> std::io::Result<()> {
    let text = serde_json::to_string(&SummaryRef {
        v: SUMMARY_VERSION,
        ended,
    })
    .map_err(std::io::Error::other)?;
    crate::rewrite::replace(
        root,
        &path(root),
        format!("{text}\n").as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

#[derive(Serialize)]
struct SummaryRef<'a> {
    v: u32,
    ended: &'a [Entry],
}

#[cfg(test)]
mod tests;
