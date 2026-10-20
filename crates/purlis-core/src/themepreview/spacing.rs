// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How often one chat is drawn a specimen** (#2536): at most once every [`GAP`], counted from
//! the end of its last turn that began a picture (#2751). Each chat is spaced on its own: one
//! chat's asks never hold back another's, and one chat asking again and again cannot take every
//! turn from the chats asking beside it. A turn that drew nothing, a theme refused by its id
//! for one, is not counted, so the chat may ask again at once.
//!
//! The gallery's share of the painter is not this module's: after a chat's turn the gallery
//! asked during, it is owed one picture ([`super::turns`]), however many chats ask.
//!
//! What is kept is when each chat's last counted turn ended, and only while that is within the
//! gap.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// **The least time between the end of one chat's turn that drew and its next**: one specimen's
/// budget.
pub const GAP: Duration = super::BUDGET;

/// When each chat's last turn ended, by a key the app gives each chat.
#[derive(Debug, Default)]
pub struct Spacing {
    ended: Mutex<HashMap<String, Instant>>,
}

impl Spacing {
    pub fn new() -> Self {
        Self::default()
    }

    /// **Whether the chat `chat` may take a turn at `now`**: `Err` with how long it is to wait
    /// when its last turn ended less than [`GAP`] before.
    pub fn may(&self, chat: &str, now: Instant) -> Result<(), Duration> {
        let mut ended = self.ended.lock().unwrap_or_else(PoisonError::into_inner);
        ended.retain(|_, at| now.saturating_duration_since(*at) < GAP);
        match ended.get(chat) {
            Some(at) => Err(GAP.saturating_sub(now.saturating_duration_since(*at))),
            None => Ok(()),
        }
    }

    /// The chat `chat`'s turn, which began a picture, ended at `now`.
    pub fn ended(&self, chat: &str, now: Instant) {
        self.ended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(chat.to_owned(), now);
    }

    /// What a chat asking within its gap is told, `left` before it may ask again.
    pub fn said(left: Duration) -> String {
        format!(
            "this chat was drawn a theme specimen a moment ago, and one chat is drawn one at most \
             every {} s so other chats are drawn between: ask again in {} s",
            GAP.as_secs(),
            left.as_secs().max(1)
        )
    }
}

#[cfg(test)]
mod tests;
