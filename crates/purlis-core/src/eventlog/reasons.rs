// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The reasons a harness's own check gave, held in memory for the blocks' detail** (R-340,
//! Q185, T1-29): what the app hears on a [`HarnessReason`] line, kept by the project, the chat's
//! number and the call's id, and handed to the window with the block's Chat log row
//! ([`super::chatlog::Row::Blocked`]'s `call`).
//!
//! **Never written, anywhere.** The event log keeps a block's source and its arguments' digest,
//! never the reason, which can quote a command; this is no store, and a restarted app holds none,
//! so a block's detail then has no reason. Nothing reads the reason but the window, which shows
//! it as written and never parses it.
//!
//! **Bounded**: at most [`MOST_HELD`] reasons, the oldest let go first, so a chat that is refused
//! without end costs the app a fixed amount of memory.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::hookwire::HarnessReason;

/// The most reasons held at once, across chats: past it, the oldest is let go.
pub const MOST_HELD: usize = 4096;

/// What a reason is held under: the project, the chat's number in it, and the call's id.
type Key = (PathBuf, u32, String);

/// **The reasons the app holds, for the app's life** ([`Reasons::hold`], [`Reasons::of`]).
#[derive(Debug, Default)]
pub struct Reasons {
    held: Mutex<Held>,
}

#[derive(Debug, Default)]
struct Held {
    by_call: HashMap<Key, String>,
    oldest_first: VecDeque<Key>,
}

impl Reasons {
    /// Holds `said` for the block of its call in `plane`; a line that names no call is of no
    /// block, and is let go. A second reason for the same call takes the first's place.
    pub fn hold(&self, plane: &Path, said: HarnessReason) {
        let Some(call) = said.call else { return };
        let key = (plane.to_path_buf(), said.chat, call);
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held
            .by_call
            .insert(key.clone(), said.harness_reason)
            .is_none()
        {
            held.oldest_first.push_back(key);
        }
        while held.oldest_first.len() > MOST_HELD {
            if let Some(oldest) = held.oldest_first.pop_front() {
                held.by_call.remove(&oldest);
            }
        }
    }

    /// The reason held for call `call` of chat `chat` in `plane`, as the harness wrote it.
    pub fn of(&self, plane: &Path, chat: u32, call: &str) -> Option<String> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.by_call
            .get(&(plane.to_path_buf(), chat, call.to_owned()))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(chat: u32, call: Option<&str>, reason: &str) -> HarnessReason {
        HarnessReason {
            chat,
            call: call.map(str::to_owned),
            harness_reason: reason.to_owned(),
        }
    }

    #[test]
    fn a_reason_is_found_by_its_project_chat_and_call_only() {
        let reasons = Reasons::default();
        reasons.hold(
            Path::new("/one"),
            said(7, Some("c1"), "  as <i>written</i>\n"),
        );
        assert_eq!(
            reasons.of(Path::new("/one"), 7, "c1").as_deref(),
            Some("  as <i>written</i>\n")
        );
        assert_eq!(reasons.of(Path::new("/two"), 7, "c1"), None);
        assert_eq!(reasons.of(Path::new("/one"), 8, "c1"), None);
        assert_eq!(reasons.of(Path::new("/one"), 7, "c2"), None);
    }

    #[test]
    fn a_reason_of_no_call_is_let_go() {
        let reasons = Reasons::default();
        reasons.hold(Path::new("/one"), said(7, None, "why"));
        assert!(reasons.held.lock().unwrap().by_call.is_empty());
    }

    #[test]
    fn past_the_bound_the_oldest_reason_is_let_go() {
        let reasons = Reasons::default();
        for n in 0..=MOST_HELD {
            reasons.hold(Path::new("/one"), said(1, Some(&n.to_string()), "why"));
        }
        assert_eq!(
            reasons.of(Path::new("/one"), 1, "0"),
            None,
            "the oldest went"
        );
        assert!(reasons.of(Path::new("/one"), 1, "1").is_some());
        assert!(
            reasons
                .of(Path::new("/one"), 1, &MOST_HELD.to_string())
                .is_some()
        );
        let held = reasons.held.lock().unwrap();
        assert_eq!(held.by_call.len(), MOST_HELD);
        assert_eq!(held.oldest_first.len(), MOST_HELD);
    }
}
