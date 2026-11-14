// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A merge press is bound to every fact it was shown** (T1-41; the follow-up Q269 named).
//! Q269 held the press to the ruleset state alone, so checks or a title that changed after the
//! question still merged. Now the press's own read keeps the asked mark only while every fact
//! shows as it did, and the digest the window was shown is the one the ask holds.

use std::sync::Mutex;

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    At, Facts, Holds, MergeForge, MergeKey, Merged, NotMerged, Reread, Ruleset, Seen,
};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn key() -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number: 12,
        head: HEAD.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

fn facts(ci: Ci, title: &str, read_at: u64) -> Facts {
    Facts {
        title: title.to_owned(),
        author: "octo".to_owned(),
        checks: Checks {
            total: Some(1),
            ci,
            why: None,
            failed: Vec::new(),
        },
        ruleset: Ruleset::None,
        approved: false,
        read_at,
    }
}

fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

#[derive(Default)]
struct Forge {
    merged: Mutex<Vec<MergeKey>>,
}

impl MergeForge for Forge {
    fn at(&self, _key: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: "main".try_into().unwrap(),
            squash: false,
        })
    }

    fn merge_at(&self, key: &MergeKey, _squash: bool) -> Result<Merged, String> {
        self.merged.lock().unwrap().push(key.clone());
        Ok(Merged::Now)
    }
}

/// Asked once more of passing checks, a press whose read finds them failed merges nothing.
#[test]
fn checks_that_change_after_the_question_ask_again() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Passed, "Add the parser", 1_000), 1_000);
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, "Add the parser", 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");

    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Failed, "Add the parser", 1_120),
            Reread::Presses,
        )
        .expect("the press's own read");

    let forge = Forge::default();
    assert_eq!(
        holds.press(4, &key(), Seen::None, the_window(), &forge),
        Err(NotMerged::NotAsked)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
}

/// Asked once more of one title, a press whose read finds another merges nothing.
#[test]
fn a_title_that_changes_after_the_question_asks_again() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Passed, "Add the parser", 1_000), 1_000);
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, "Add the parser", 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");

    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, "Add the parser and delete the tests", 1_120),
            Reread::Presses,
        )
        .expect("the press's own read");

    assert!(!holds.every()[0].asked, "a new title is asked of again");
}

/// The digest is of what the person is shown, not of when it was read; and the ask holds the
/// window to the digest of what it shows now.
#[test]
fn the_digest_is_of_what_is_shown_and_the_ask_holds_the_window_to_it() {
    purlis_core::unsteered!();
    let shown = facts(Ci::Passed, "Add the parser", 1_000);
    assert_eq!(
        shown.digest(),
        facts(Ci::Passed, "Add the parser", 2_000).digest(),
        "the time of a read is no part of it"
    );
    assert_ne!(
        shown.digest(),
        facts(Ci::Failed, "Add the parser", 1_000).digest()
    );
    assert_ne!(
        shown.digest(),
        facts(Ci::Passed, "Add the parse", 1_000).digest()
    );
    assert_ne!(
        shown.digest(),
        Facts {
            approved: true,
            ..shown.clone()
        }
        .digest()
    );

    let holds = Holds::default();
    holds.raise(4, key(), shown.clone(), 1_000);
    assert!(holds.shows(4, &key(), &shown.digest()));
    assert!(!holds.shows(
        4,
        &key(),
        &facts(Ci::Failed, "Add the parser", 1_000).digest()
    ));
    assert!(
        !holds.shows(5, &key(), &shown.digest()),
        "another chat's ask"
    );
}
