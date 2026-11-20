// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A merge binds its method as well as its head and base** (ADR 0128 D8.1): the press merges by
//! the method the row showed, and a repo whose method changed since refuses the press and asks
//! once more of the new one.

use std::sync::Mutex;

use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    At, Holds, MergeForge, MergeKey, Merged, NotMerged, Raised, Reread, Seen,
};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

struct Forge {
    squash: Mutex<bool>,
    merged: Mutex<Vec<bool>>,
}

impl MergeForge for Forge {
    fn at(&self, _: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: "main".try_into().unwrap(),
            squash: *self.squash.lock().unwrap(),
        })
    }

    fn merge_at(&self, _: &MergeKey, squash: bool) -> Result<Merged, String> {
        self.merged.lock().unwrap().push(squash);
        Ok(Merged::Now)
    }
}

fn key() -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number: 12,
        head: HEAD.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

fn facts(squash: bool) -> purlis_core::mergehold::Facts {
    purlis_core::mergehold::Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: purlis_core::forge::checks::Checks {
            total: Some(1),
            ci: purlis_core::forge::checks::Ci::Passed,
            why: None,
            failed: Vec::new(),
        },
        ruleset: purlis_core::mergehold::Ruleset::None,
        approved_by: None,
        squash,
        read_at: 40,
    }
}

fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).unwrap()
}

fn held(shown_squash: bool) -> Holds {
    let holds = Holds::default();
    assert_eq!(holds.raise(4, key(), facts(shown_squash), 40), Raised::New);
    holds
        .refresh(4, &key(), key(), facts(shown_squash), Reread::AsksOnceMore)
        .unwrap();
    holds
}

#[test]
fn the_press_merges_by_the_method_the_row_showed() {
    purlis_core::unsteered!();
    let holds = held(true);
    let forge = Forge {
        squash: Mutex::new(true),
        merged: Mutex::default(),
    };
    holds
        .press(4, &key(), Seen::None, the_window(), &forge)
        .unwrap();
    assert_eq!(*forge.merged.lock().unwrap(), vec![true]);
}

#[test]
fn a_repo_whose_method_changed_after_the_ask_refuses_the_press_and_asks_once_more() {
    purlis_core::unsteered!();
    let holds = held(false);
    let forge = Forge {
        squash: Mutex::new(true),
        merged: Mutex::default(),
    };
    let refused = holds
        .press(4, &key(), Seen::None, the_window(), &forge)
        .unwrap_err();
    assert_eq!(
        refused,
        NotMerged::MethodMoved {
            was: "a merge commit",
            now: "squashing"
        }
    );
    assert!(
        forge.merged.lock().unwrap().is_empty(),
        "nothing was merged"
    );
    // The ask waits, now showing the new method, and has not been asked once more of it.
    let waits = holds.every();
    assert_eq!(waits.len(), 1);
    assert!(waits[0].facts.squash);
    assert_eq!(
        holds
            .press(4, &key(), Seen::None, the_window(), &forge)
            .unwrap_err(),
        NotMerged::NotAsked
    );
    holds
        .refresh(4, &key(), key(), facts(true), Reread::AsksOnceMore)
        .unwrap();
    holds
        .press(4, &key(), Seen::None, the_window(), &forge)
        .unwrap();
    assert_eq!(*forge.merged.lock().unwrap(), vec![true]);
}

#[test]
fn a_new_read_that_shows_another_method_asks_once_more() {
    purlis_core::unsteered!();
    let holds = held(false);
    assert!(holds.every()[0].asked);
    // The same head, read again, with the repo now squashing: the row shows it and asks again.
    holds
        .refresh(4, &key(), key(), facts(true), Reread::Watches)
        .unwrap();
    assert!(!holds.every()[0].asked);
    assert!(holds.every()[0].facts.squash);
}
