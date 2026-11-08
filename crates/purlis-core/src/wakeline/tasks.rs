// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the wake line tells a chat of its tasks** (#5754, spec #5743; #1546): the chat that
//! asked is cut off by the restart as its tasks are, and is brought back knowing nothing of
//! them. Its wake line names each task the launch restored and where it stands, and each task
//! whose report waits for it, so it neither dispatches them again nor waits on one that has
//! ended.
//!
//! Made only of purlis's own words, chat numbers the app dealt and the words a finished task's
//! row is told with ([`crate::dispatchrecord::Finished::said_to_a_chat`]): no task's name, no
//! report's text. A report itself reaches the chat as it always does, quoted as data in the
//! context of the turn the wake line starts ([`crate::handback::context`]), and
//! `purlis dispatch wait` reads it again from the task's dispatch record.

/// The most tasks one list in the line names; the rest are counted.
pub const MOST_NAMED: usize = 20;

/// Where a task the launch restored stands as its asking chat is brought back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Brought back on its conversation and told to carry on.
    CarriesOn,
    /// The session host kept its program running across the restart (#3900).
    KeptRunning,
    /// purlis could not bring it back: it has ended without a report, and purlis's word on it
    /// is left for the chat that asked.
    Ended,
}

impl Standing {
    fn said(self) -> &'static str {
        match self {
            Self::CarriesOn => "brought back on its conversation, carrying on",
            Self::KeptRunning => "still running, as the restart did not stop it",
            Self::Ended => "ended without a report, as purlis could not bring it back",
        }
    }
}

/// One task the launch restored, by the number its chat has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restored {
    pub chat: u32,
    pub standing: Standing,
}

/// One task whose report waits for the chat that asked: by the number its chat had, and how
/// it ended, in the words a chat's list of tasks says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reported {
    pub chat: u32,
    pub ended: &'static str,
}

/// **What a chat brought back is told of its tasks.**
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct News {
    /// Its tasks the launch restored, in the record's order.
    pub restored: Vec<Restored>,
    /// Its tasks whose reports wait for it: pending at the quit, or kept for it while it was
    /// away.
    pub reported: Vec<Reported>,
}

impl News {
    /// Whether there is nothing to tell.
    pub fn is_empty(&self) -> bool {
        self.restored.is_empty() && self.reported.is_empty()
    }
}

/// What a chat told nothing else is told first: the wake line's own opening.
const RESTARTED: &str =
    "purlis: purlis was restarted, and this is this chat's conversation brought back.";

/// **`line` with the news of the chat's tasks added**, or `line` as it was where there is no
/// news. A chat with no other wake line is given one for its tasks alone.
pub fn with(line: Option<String>, news: &News) -> Option<String> {
    if news.is_empty() {
        return line;
    }
    let mut said = line.unwrap_or_else(|| RESTARTED.to_owned());
    if !news.restored.is_empty() {
        let listed = listed(
            news.restored
                .iter()
                .map(|task| format!("chat {}: {}", task.chat, task.standing.said())),
        );
        said.push_str(&format!(
            " The tasks this chat dispatched before the restart: {listed}. They are still this \
             chat's tasks, and each report reaches it as it lands: dispatch none of them again \
             while they work. `purlis dispatch list` shows where each stands."
        ));
    }
    if !news.reported.is_empty() {
        let listed = listed(
            news.reported
                .iter()
                .map(|task| format!("chat {} reported ({})", task.chat, task.ended)),
        );
        said.push_str(&format!(
            " Reports of its tasks waited for this chat across the restart: {listed}. Each is \
             handed to this turn as context, quoted as data; `purlis dispatch wait <chat>` \
             prints one again."
        ));
    }
    Some(said)
}

/// `items` joined with "; ", the first [`MOST_NAMED`] of them, and the rest counted.
fn listed(items: impl Iterator<Item = String>) -> String {
    let items: Vec<String> = items.collect();
    let more = items.len().saturating_sub(MOST_NAMED);
    let mut said = items
        .into_iter()
        .take(MOST_NAMED)
        .collect::<Vec<_>>()
        .join("; ");
    if more > 0 {
        said.push_str(&format!("; and {more} more"));
    }
    said
}

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
