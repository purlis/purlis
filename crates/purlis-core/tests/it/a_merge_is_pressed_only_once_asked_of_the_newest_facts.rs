// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A merge takes two presses, and the core holds it to them** (Q268, Q269; T1-40's security
//! review). A press merges only after the window read the request again and asked once more,
//! and only while what the ask shows has not changed since. What a waiting ask shows never goes
//! back to an older read, whether a raise or a re-read brings it.

use std::sync::Mutex;

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    At, Facts, Holds, MergeForge, MergeKey, Merged, NotMerged, Raised, Refreshed, Reread, Ruleset,
    Seen,
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

fn facts(ci: Ci, ruleset: Ruleset, read_at: u64) -> Facts {
    Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: Checks {
            total: Some(1),
            ci,
            why: None,
            failed: Vec::new(),
        },
        ruleset,
        approved: false,
        read_at,
    }
}

fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

/// A forge that merges what it is asked to, and counts it.
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

/// Q269: a press the window never asked once more for merges nothing, and the ask still waits.
#[test]
fn a_press_with_no_reread_merges_nothing() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Passed, Ruleset::None, 1_000), 1_000);
    let forge = Forge::default();

    assert_eq!(
        holds.press(4, &key(), Seen::None, the_window(), &forge),
        Err(NotMerged::NotAsked)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
    assert_eq!(holds.every().len(), 1, "the ask still waits");

    // Asked once more, the same press merges.
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, Ruleset::None, 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");
    assert!(
        holds
            .press(4, &key(), Seen::None, the_window(), &forge)
            .is_ok()
    );
    assert_eq!(*forge.merged.lock().unwrap(), [key()]);
}

/// Q269: asked once more, then the chat's try again brings facts the person was not asked of:
/// the press merges nothing, and the ask waits to be asked of them.
#[test]
fn a_reread_then_a_facts_change_then_a_press_merges_nothing() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Running, Ruleset::None, 1_000), 1_000);
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Running, Ruleset::None, 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");
    assert!(holds.every()[0].asked);

    assert_eq!(
        holds.raise(4, key(), facts(Ci::Failed, Ruleset::None, 1_120), 1_120),
        Raised::Already
    );
    assert!(!holds.every()[0].asked, "the new facts were never asked of");
    let forge = Forge::default();

    assert_eq!(
        holds.press(4, &key(), Seen::None, the_window(), &forge),
        Err(NotMerged::NotAsked)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
    assert_eq!(holds.every().len(), 1, "the ask still waits");
}

/// Q269: a raise or a re-read that read the forge before what the ask shows never puts the
/// older facts back, so a slow hook can't undo the ruleset the person was shown.
#[test]
fn an_older_raise_never_overwrites_newer_facts() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Passed, Ruleset::None, 1_000), 1_000);
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, Ruleset::Protects, 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");

    assert_eq!(
        holds.raise(4, key(), facts(Ci::Passed, Ruleset::None, 1_030), 1_070),
        Raised::Already
    );
    assert_eq!(
        holds.refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, Ruleset::None, 1_040),
            Reread::Presses,
        ),
        Ok(Refreshed::Same)
    );

    let waiting = holds.every();
    assert_eq!(waiting[0].facts.ruleset, Ruleset::Protects);
    assert_eq!(waiting[0].facts.read_at, 1_060);
    assert!(
        waiting[0].asked,
        "what the person was asked of still stands"
    );
    let forge = Forge::default();
    assert_eq!(
        holds.press(4, &key(), Seen::None, the_window(), &forge),
        Err(NotMerged::RulesetChanged)
    );
    assert!(
        holds
            .press(4, &key(), Seen::Protects, the_window(), &forge)
            .is_ok()
    );
}

/// Q269: the press's own read keeps the mark only while the ruleset is unchanged. Asked once
/// more of no ruleset, a press whose read finds one leaves the ask unasked, so a press of the
/// new state merges nothing until the person is asked of it.
#[test]
fn a_press_whose_read_finds_a_new_ruleset_leaves_the_ask_unasked() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(), facts(Ci::Passed, Ruleset::None, 1_000), 1_000);
    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, Ruleset::None, 1_060),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");
    assert!(holds.every()[0].asked);

    holds
        .refresh(
            4,
            &key(),
            key(),
            facts(Ci::Passed, Ruleset::Protects, 1_120),
            Reread::Presses,
        )
        .expect("the press's own read");
    assert!(!holds.every()[0].asked, "a new ruleset is asked of again");
    let forge = Forge::default();

    assert_eq!(
        holds.press(4, &key(), Seen::Protects, the_window(), &forge),
        Err(NotMerged::NotAsked)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
}
