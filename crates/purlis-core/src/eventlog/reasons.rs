// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The reasons a harness's own check gave, held in memory for the blocks' detail** (R-340,
//! Q185, T1-29): what the app hears on a [`HarnessReason`] line, kept by the project, the chat's
//! number and the call's id, and handed to the window with the block's Chat log row
//! ([`super::chatlog::Row::Blocked`]'s `call`), a harness check's block's alone (Q259).
//!
//! **Never written, anywhere.** The event log keeps a block's source and its arguments' digest,
//! never the reason, which can quote a command; this is no store, and a restarted app holds none,
//! so a block's detail then has no reason. Nothing reads the reason but the window, which shows
//! it as written and never parses it. A `Debug` print names the keys alone, never a reason.
//!
//! **Untrusted, so bounded per chat** (Q259): the reason is the chat's or its harness's words.
//! The first reason held for a call is kept, and a later one for it dropped. A chat holds at
//! most [`MOST_PER_CHAT`], its own oldest let go first, so a chat refused without end evicts no
//! other chat's; a reason past [`LONGEST`] bytes is cut, ending in [`CUT`]. Across chats at
//! most [`MOST_HELD`] are held, the oldest let go first, so the app's memory stays fixed.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::hookwire::HarnessReason;

/// The most reasons held at once, across chats: past it, the oldest is let go.
pub const MOST_HELD: usize = 4096;

/// The most reasons one chat holds at once: past it, that chat's oldest is let go.
pub const MOST_PER_CHAT: usize = 256;

/// The longest a reason is held, in bytes, its [`CUT`] included.
pub const LONGEST: usize = 16 * 1024;

/// What ends a reason cut at [`LONGEST`], so the window shows that it was.
pub const CUT: &str = "… (cut)";

/// What a reason is held under: the project, the chat's number in it, and the call's id.
type Key = (PathBuf, u32, String);

/// **The reasons the app holds, for the app's life** ([`Reasons::hold`], [`Reasons::of`]).
#[derive(Default)]
pub struct Reasons {
    held: Mutex<Held>,
}

/// Keys only: a reason is the chat's words, and is never printed.
impl fmt::Debug for Reasons {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_struct("Reasons")
            .field("held", &held.oldest_first)
            .finish()
    }
}

#[derive(Default)]
struct Held {
    by_call: HashMap<Key, String>,
    oldest_first: VecDeque<Key>,
    /// How many reasons each chat (project and number) holds.
    per_chat: HashMap<(PathBuf, u32), usize>,
}

impl Held {
    /// Lets go of the reason at `at` in [`Held::oldest_first`].
    fn let_go(&mut self, at: usize) {
        let Some((plane, chat, call)) = self.oldest_first.remove(at) else {
            return;
        };
        let chat_key = (plane, chat);
        if let Some(count) = self.per_chat.get_mut(&chat_key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.per_chat.remove(&chat_key);
            }
        }
        let (plane, chat) = chat_key;
        self.by_call.remove(&(plane, chat, call));
    }
}

/// `reason`, or its first bytes ending in [`CUT`] where it is past [`LONGEST`].
fn bounded(mut reason: String) -> String {
    if reason.len() <= LONGEST {
        return reason;
    }
    let mut end = LONGEST - CUT.len();
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason.truncate(end);
    reason.push_str(CUT);
    reason
}

impl Reasons {
    /// Holds `said` for the block of its call in `plane`; a line that names no call is of no
    /// block, and is let go. **The first reason held for a call wins** (Q259): a later one for
    /// the same call is dropped, so a chat cannot put its words over its harness's.
    pub fn hold(&self, plane: &Path, said: HarnessReason) {
        let Some(call) = said.call else { return };
        let key = (plane.to_path_buf(), said.chat, call);
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if held.by_call.contains_key(&key) {
            return;
        }
        let chat_key = (key.0.clone(), key.1);
        if held.per_chat.get(&chat_key).copied().unwrap_or(0) >= MOST_PER_CHAT {
            let oldest = held
                .oldest_first
                .iter()
                .position(|(plane, chat, _)| (plane, *chat) == (&chat_key.0, chat_key.1));
            if let Some(at) = oldest {
                held.let_go(at);
            }
        }
        held.by_call
            .insert(key.clone(), bounded(said.harness_reason));
        held.oldest_first.push_back(key);
        *held.per_chat.entry(chat_key).or_insert(0) += 1;
        while held.oldest_first.len() > MOST_HELD {
            held.let_go(0);
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
    fn the_first_reason_held_for_a_call_wins() {
        let reasons = Reasons::default();
        reasons.hold(Path::new("/one"), said(7, Some("c1"), "first"));
        reasons.hold(Path::new("/one"), said(7, Some("c1"), "second"));
        assert_eq!(
            reasons.of(Path::new("/one"), 7, "c1").as_deref(),
            Some("first")
        );
        assert_eq!(reasons.held.lock().unwrap().oldest_first.len(), 1);
    }

    #[test]
    fn one_chat_flooding_leaves_another_chats_reason_held() {
        let reasons = Reasons::default();
        reasons.hold(Path::new("/one"), said(2, Some("kept"), "chat two's"));
        for n in 0..=MOST_PER_CHAT * 4 {
            reasons.hold(Path::new("/one"), said(1, Some(&n.to_string()), "flood"));
        }
        assert_eq!(
            reasons.of(Path::new("/one"), 2, "kept").as_deref(),
            Some("chat two's"),
            "chat 1's flood evicts only chat 1's own"
        );
        assert_eq!(
            reasons.of(Path::new("/one"), 1, "0"),
            None,
            "its oldest went"
        );
        let last = (MOST_PER_CHAT * 4).to_string();
        assert!(reasons.of(Path::new("/one"), 1, &last).is_some());
        let held = reasons.held.lock().unwrap();
        assert_eq!(held.by_call.len(), MOST_PER_CHAT + 1);
    }

    #[test]
    fn a_reason_past_its_length_is_cut_with_a_marker() {
        let reasons = Reasons::default();
        let long = "é".repeat(LONGEST);
        reasons.hold(Path::new("/one"), said(1, Some("c1"), &long));
        let kept = reasons.of(Path::new("/one"), 1, "c1").expect("held");
        assert!(kept.len() <= LONGEST, "{} bytes", kept.len());
        assert!(kept.ends_with(CUT), "it says it was cut");
        assert!(kept.starts_with("éé"));
        let short = "a".repeat(LONGEST);
        reasons.hold(Path::new("/one"), said(1, Some("c2"), &short));
        assert_eq!(
            reasons.of(Path::new("/one"), 1, "c2"),
            Some(short),
            "one at the bound is whole"
        );
    }

    #[test]
    fn a_debug_print_names_keys_never_the_reason() {
        let reasons = Reasons::default();
        let secret = "the reason's own words";
        let line = said(7, Some("c1"), secret);
        assert!(!format!("{line:?}").contains(secret));
        assert!(format!("{line:?}").contains("c1"));
        reasons.hold(Path::new("/one"), line);
        let printed = format!("{reasons:?}");
        assert!(!printed.contains(secret), "{printed}");
        assert!(printed.contains("c1"), "{printed}");
    }

    #[test]
    fn past_the_bound_across_chats_the_oldest_reason_is_let_go() {
        let reasons = Reasons::default();
        let chat_of = |n: usize| u32::try_from(n / MOST_PER_CHAT).unwrap();
        for n in 0..=MOST_HELD {
            reasons.hold(
                Path::new("/one"),
                said(chat_of(n), Some(&n.to_string()), "why"),
            );
        }
        assert_eq!(
            reasons.of(Path::new("/one"), 0, "0"),
            None,
            "the oldest went"
        );
        assert!(reasons.of(Path::new("/one"), 0, "1").is_some());
        assert!(
            reasons
                .of(
                    Path::new("/one"),
                    chat_of(MOST_HELD),
                    &MOST_HELD.to_string()
                )
                .is_some()
        );
        let held = reasons.held.lock().unwrap();
        assert_eq!(held.by_call.len(), MOST_HELD);
        assert_eq!(held.oldest_first.len(), MOST_HELD);
    }
}
