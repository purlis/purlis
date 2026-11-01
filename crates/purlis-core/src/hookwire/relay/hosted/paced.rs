// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How soon a chat's refused tool asks again** (#4332): each refusal waits twice as long as the
//! last, from [`FIRST`] to [`MOST`], until something clears it. Kept per project, chat and tool, in
//! memory, so a chat cannot reach it: it runs in the host, on the far side of the chat's sandbox.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The wait after the first refusal.
const FIRST: Duration = Duration::from_secs(5);

/// The longest wait.
const MOST: Duration = Duration::from_secs(300);

/// Whose pace it is: the project, the chat and its tool.
pub type Whose = (PathBuf, u32, String);

/// What is still to wait, and the words of the refusal it follows.
#[derive(Debug, PartialEq, Eq)]
pub struct Waiting {
    pub left: Duration,
    pub words: String,
}

struct Pace {
    refusals: u32,
    until: Instant,
    words: String,
}

/// One kind of asking, paced ([`Whose`]).
#[derive(Default)]
pub struct Paced {
    held: Mutex<HashMap<Whose, Pace>>,
}

impl Paced {
    /// What `whose` has still to wait, if anything.
    pub fn waits(&self, whose: &Whose) -> Option<Waiting> {
        self.waits_at(whose, Instant::now())
    }

    /// `whose` was refused, saying `words`.
    pub fn refused(&self, whose: &Whose, words: &str) {
        self.refused_at(whose, words, Instant::now());
    }

    /// `whose` got what it asked for: it waits for nothing now.
    pub fn cleared(&self, whose: &Whose) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(whose);
    }

    fn waits_at(&self, whose: &Whose, now: Instant) -> Option<Waiting> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let pace = held.get(whose)?;
        let left = pace.until.checked_duration_since(now)?;
        (!left.is_zero()).then(|| Waiting {
            left,
            words: pace.words.clone(),
        })
    }

    fn refused_at(&self, whose: &Whose, words: &str, now: Instant) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        // What has waited out its time long since is forgotten, so this does not grow with chats.
        held.retain(|_, pace| now.saturating_duration_since(pace.until) < MOST * 2);
        let refusals = held.get(whose).map_or(0, |pace| pace.refusals) + 1;
        let wait = FIRST
            .checked_mul(1 << (refusals - 1).min(16))
            .map_or(MOST, |wait| wait.min(MOST));
        held.insert(
            whose.clone(),
            Pace {
                refusals,
                until: now + wait,
                words: words.to_owned(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_chat(chat: u32) -> Whose {
        (PathBuf::from("/project"), chat, "gh".to_owned())
    }

    #[test]
    fn each_refusal_waits_twice_as_long_up_to_the_most() {
        let paced = Paced::default();
        let (chat, start) = (a_chat(1), Instant::now());
        let mut now = start;
        for seconds in [5, 10, 20, 40, 80, 160, 300, 300] {
            paced.refused_at(&chat, "no", now);
            let waiting = paced.waits_at(&chat, now).expect("it waits");
            assert_eq!(waiting.left, Duration::from_secs(seconds));
            assert_eq!(waiting.words, "no");
            now += Duration::from_secs(seconds);
            assert_eq!(paced.waits_at(&chat, now), None, "its time is up");
        }
    }

    #[test]
    fn a_pace_is_one_chats_and_a_clearing_ends_it() {
        let paced = Paced::default();
        let now = Instant::now();
        paced.refused_at(&a_chat(1), "no", now);
        assert_eq!(paced.waits_at(&a_chat(2), now), None);
        paced.cleared(&a_chat(1));
        assert_eq!(paced.waits_at(&a_chat(1), now), None);
        paced.refused_at(&a_chat(1), "no", now);
        assert_eq!(
            paced.waits_at(&a_chat(1), now).unwrap().left,
            FIRST,
            "cleared, it starts again from the first wait"
        );
    }

    #[test]
    fn a_pace_long_past_is_forgotten() {
        let paced = Paced::default();
        let now = Instant::now();
        paced.refused_at(&a_chat(1), "no", now);
        paced.refused_at(&a_chat(2), "no", now + MOST * 3);
        assert_eq!(paced.held.lock().unwrap().len(), 1);
    }
}
