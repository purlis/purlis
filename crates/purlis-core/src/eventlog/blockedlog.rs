// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Blocked log** (R-221, R-222; Q69, Q264): a project's blocks of the last 30 days, a
//! view of the event log. Stub: the tests come first (T1-32).

use std::io;
use std::path::Path;
use std::time::Duration;

use super::Event;
use super::stops::Block;

/// How far back the Blocked log reaches.
pub const KEPT: Duration = Duration::ZERO;

/// One block the Blocked log lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub at_ms: u64,
    pub seq: u64,
    pub chat: String,
    pub block: Block,
}

/// The Blocked log of `events` at `now_ms`, of the chats `is_ours` keeps.
pub fn listed(_events: &[Event], _now_ms: u64, _is_ours: &dyn Fn(&str) -> bool) -> Vec<Listed> {
    Vec::new()
}

/// [`listed`] of the log in `dir`.
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
