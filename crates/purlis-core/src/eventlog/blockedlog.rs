// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Blocked log** (R-221, R-222; Q69, Q264): a project's blocks of the last 30 days, a
//! view of the event log. In phase 1 Settings' Network page draws it where "Blocked lately" was,
//! and Blocked lately is folded into it; it moves into the Sandbox view in phase 2 (R-244).
//!
//! **Nothing beside the log is read.** [`listed`] takes the log's events and the caller's
//! filter alone, and keeps nothing of its own: a block the log gains is listed, and one it no
//! longer holds is not. Every sandbox refusal is written as a block (T1-10), so the rows the
//! network record's Blocked lately drew are here, by the target summary their block names
//! (R-220, Q206).
//!
//! **Only what ended as a block** (Q264): a refusal that is still an open ask is in Asks, and
//! one the person allowed is an answer (`ask.answered`), never a block, so neither is listed.
//!
//! **Thirty days, as the log keeps them** ([`KEPT`] is [`super::Retention::keep`]). The log may
//! hold a sealed segment longer for a consumer that has not taken it (#940), so the window is
//! the query's own, never left to the pruning.
//!
//! **Which project is the caller's to say** (Q264): the log is this machine's, and a block names
//! its chat and not its project, so the caller passes the chats it knows as the project's. The
//! app knows a project's open chats, those closed since it launched, and those its reopen record
//! holds; a chat that ended before a restart and is in none of those drops off the list until
//! a block names its project.

use std::io;
use std::path::Path;
use std::time::Duration;

use super::Event;
use super::stops::{self, Block};

/// **How far back the Blocked log reaches**: the 30 days the event log keeps every event.
pub const KEPT: Duration = super::Retention::DEFAULT.keep;

/// **One block the Blocked log lists**: when, its place in its device's log, its chat by id, and
/// the block as its event says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// When it was written, in milliseconds since 1970, from its event's id.
    pub at_ms: u64,
    pub seq: u64,
    pub chat: String,
    pub block: Block,
}

/// **The Blocked log of `events` at `now_ms`**: each `stop.block` of the last [`KEPT`] whose chat
/// `is_ours` keeps, newest first. A block of no chat, one whose id holds no time, and one whose
/// body does not read back as a [`Block`] (nothing says what it was aimed at) are not listed.
pub fn listed(events: &[Event], now_ms: u64, is_ours: &dyn Fn(&str) -> bool) -> Vec<Listed> {
    let window = u64::try_from(KEPT.as_millis()).unwrap_or(u64::MAX);
    let since = now_ms.saturating_sub(window);
    let mut listed: Vec<Listed> = events
        .iter()
        .filter(|event| event.kind == stops::BLOCK)
        .filter_map(|event| {
            let chat = event.chat.as_deref().filter(|chat| is_ours(chat))?;
            let at_ms = super::written_at_ms(&event.ulid).filter(|at| *at >= since)?;
            let block = serde_json::from_value(event.body.clone()).ok()?;
            Some(Listed {
                at_ms,
                seq: event.seq,
                chat: chat.to_owned(),
                block,
            })
        })
        .collect();
    // Newest first; two written in one millisecond, in the order the log numbered them.
    listed.sort_by(|one, other| (other.at_ms, other.seq).cmp(&(one.at_ms, one.seq)));
    listed
}

/// [`listed`] of the log in `dir`, read through [`super::read`].
pub fn listed_in(
    dir: &Path,
    now_ms: u64,
    is_ours: &dyn Fn(&str) -> bool,
) -> io::Result<Vec<Listed>> {
    Ok(listed(&super::read(dir)?, now_ms, is_ours))
}

#[cfg(test)]
#[path = "blockedlog/tests.rs"]
mod tests;
