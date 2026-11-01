// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How soon a chat's refused tool asks again** (#4332): each refusal waits twice as long as the
//! last, from [`FIRST`] to [`MOST`], until something clears it. Kept per project, chat and tool, in
//! memory, so a chat cannot reach it: it runs in the host, on the far side of the chat's sandbox.
//!
//! **Runs started at once take [`Turns`]** (#4374): a pace holds for runs one after another, and
//! several started together would each ask before the first refusal is recorded. So those that come
//! while one is under way wait for what comes of it, and then go by the pace it left.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, PoisonError};
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

    /// Chat `chat` went: what is started again under its number starts with no pace (#4374).
    pub fn chat_went(&self, chat: u32) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|whose, _| whose.1 != chat);
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

/// Whose runs are asking now, one at a time for each [`Whose`].
#[derive(Default)]
pub struct Turns {
    under_way: Mutex<HashSet<Whose>>,
    one_ended: Condvar,
    /// How many of each chat's runs wait for a turn now.
    waiting: Mutex<HashMap<u32, usize>>,
}

/// One chat's runs waiting for a turn at once, past which the next is refused: a chat's own runs do
/// not fill the host's connections, which other chats and their hooks share.
const MOST_WAITING_FOR_A_TURN: usize = 8;

/// One run counted as waiting for a turn, until dropped.
pub struct WaitsForATurn<'a> {
    turns: &'a Turns,
    chat: u32,
}

impl Drop for WaitsForATurn<'_> {
    fn drop(&mut self) {
        let mut waiting = self
            .turns
            .waiting
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = waiting.get_mut(&self.chat) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                waiting.remove(&self.chat);
            }
        }
    }
}

/// One run's turn to ask, until dropped.
pub struct Turn<'a> {
    turns: &'a Turns,
    whose: Whose,
}

impl Turns {
    /// Counts one more of chat `chat`'s runs as waiting; `None` where too many do already.
    pub fn waits(&self, chat: u32) -> Option<WaitsForATurn<'_>> {
        let mut waiting = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        let count = waiting.entry(chat).or_default();
        if *count >= MOST_WAITING_FOR_A_TURN {
            return None;
        }
        *count += 1;
        Some(WaitsForATurn { turns: self, chat })
    }

    /// `whose`'s turn once it is free, looking no longer than `within`; `None` past that. A turn let
    /// go of wakes what waits at once, so a tool that asks nothing costs its parallel runs nothing.
    pub fn taken_within(&self, whose: &Whose, within: Duration) -> Option<Turn<'_>> {
        let until = Instant::now() + within;
        let mut under_way = self
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while under_way.contains(whose) {
            let left = until.checked_duration_since(Instant::now())?;
            under_way = self
                .one_ended
                .wait_timeout(under_way, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        under_way.insert(whose.clone());
        Some(Turn {
            turns: self,
            whose: whose.clone(),
        })
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.turns
            .under_way
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.whose);
        self.turns.one_ended.notify_all();
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

    #[test]
    fn a_chat_gone_takes_its_paces_with_it_and_no_other() {
        let paced = Paced::default();
        let now = Instant::now();
        let another_tool = (PathBuf::from("/project"), 1, "glab".to_owned());
        paced.refused_at(&a_chat(1), "no", now);
        paced.refused_at(&another_tool, "no", now);
        paced.refused_at(&a_chat(2), "no", now);
        paced.chat_went(1);
        assert_eq!(paced.waits_at(&a_chat(1), now), None);
        assert_eq!(paced.waits_at(&another_tool, now), None);
        assert!(paced.waits_at(&a_chat(2), now).is_some());
    }

    #[test]
    fn a_turn_is_one_at_a_time_and_only_for_its_own() {
        let turns = Turns::default();
        let held = turns
            .taken_within(&a_chat(1), Duration::ZERO)
            .expect("free");
        assert!(turns.taken_within(&a_chat(1), Duration::ZERO).is_none());
        assert!(turns.taken_within(&a_chat(2), Duration::ZERO).is_some());
        drop(held);
        assert!(turns.taken_within(&a_chat(1), Duration::ZERO).is_some());
    }

    #[test]
    fn a_chats_runs_waiting_for_a_turn_are_bounded_and_its_own() {
        let turns = Turns::default();
        let held: Vec<_> = (0..MOST_WAITING_FOR_A_TURN)
            .map(|_| turns.waits(1).expect("room"))
            .collect();
        assert!(turns.waits(1).is_none());
        assert!(turns.waits(2).is_some(), "another chat has its own");
        drop(held);
        assert!(turns.waits(1).is_some());
    }

    #[test]
    fn a_turn_let_go_of_wakes_what_waits_for_it() {
        let turns = Turns::default();
        let held = turns
            .taken_within(&a_chat(1), Duration::ZERO)
            .expect("free");
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| {
                let since = Instant::now();
                turns
                    .taken_within(&a_chat(1), Duration::from_secs(60))
                    .is_some()
                    && since.elapsed() < Duration::from_secs(30)
            });
            std::thread::sleep(Duration::from_millis(50));
            drop(held);
            assert!(waiting.join().unwrap(), "woken, well before its time");
        });
    }
}
