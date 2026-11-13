// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's finish rows are a query of the event log** (R-241, R-178, R-179; Q131, Q164): each
//! turn of the chat's that ended, and each run of its own that ended failed, with the blocks of
//! that turn still unresolved at its end. Updates › Mine draws one row for each.

use std::io;
use std::path::Path;

use super::Event;

/// How a finish ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// The chat's turn ended (`hook.stop` of its own run).
    Finished,
    /// The chat's own run ended `failed`.
    Failed,
}

/// **One finish of a chat's**: the event that ended it, and its turn's unresolved blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finish {
    pub seq: u64,
    pub ulid: String,
    pub at_ms: Option<u64>,
    pub how: How,
    pub blocked: u32,
}

/// The finishes of chat `chat` among `events`, oldest first.
pub fn finishes(_events: &[Event], _chat: &str) -> Vec<Finish> {
    Vec::new()
}

/// [`finishes`] of the log in `dir`, read through [`super::read`].
pub fn finishes_in(dir: &Path, chat: &str) -> io::Result<Vec<Finish>> {
    Ok(finishes(&super::read(dir)?, chat))
}

#[cfg(test)]
#[path = "finishes/tests.rs"]
mod tests;
