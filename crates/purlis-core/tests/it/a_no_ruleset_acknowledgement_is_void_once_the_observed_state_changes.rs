// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the person was shown of a merge is what their press is bound to** (T1-40; R-150,
//! Q248). The core keeps the ruleset state it read with the merge ask, and a press says the
//! state the person saw: it merges only while that is the state a fresh read gives. A re-read
//! refreshes what a waiting ask shows, with the time it was read, and never raises an ask that
//! is gone. A press that fails puts its ask back only where no newer ask for the request has
//! been raised meanwhile, so a press at an old head racing the chat's try again never leaves two
//! rows for one request (pending-corrections, T1-11's final re-review).

use std::sync::Mutex;

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    At, Facts, Holds, MergeForge, MergeKey, Merged, NotMerged, Refreshed, Reread, Ruleset, Seen,
};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const MOVED: &str = "89abcdef0123456789abcdef0123456789abcdef";

fn key(head: &str) -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number: 12,
        head: head.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

fn facts(ruleset: Ruleset, read_at: u64) -> Facts {
    Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: Checks {
            total: Some(1),
            ci: Ci::Passed,
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

/// A forge that merges what it is asked to and counts it, or refuses; `meanwhile` runs inside
/// its merge, as a chat's try again would land while the press waits on the forge.
#[derive(Default)]
struct Forge<'a> {
    merged: Mutex<Vec<MergeKey>>,
    refuses: Option<&'static str>,
    meanwhile: Option<Box<dyn Fn() + 'a>>,
}

impl MergeForge for Forge<'_> {
    fn at(&self, _key: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: "main".try_into().unwrap(),
            squash: false,
        })
    }

    fn merge_at(&self, key: &MergeKey, _squash: bool) -> Result<Merged, String> {
        if let Some(meanwhile) = &self.meanwhile {
            meanwhile();
        }
        if let Some(why) = self.refuses {
            return Err(why.to_owned());
        }
        self.merged.lock().unwrap().push(key.clone());
        Ok(Merged::Now)
    }
}

/// R-150: acknowledged with no ruleset on the base, then a fresh read finds one: the press is
/// refused, nothing is merged, and the ask asks again, showing the new state. Unchanged, the
/// press proceeds.
#[test]
fn a_no_ruleset_acknowledgement_is_void_once_the_observed_state_changes() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(Ruleset::None, 1_000), 1_000);
    // The fresh read before the person's once-more press finds a ruleset on main now.
    assert_eq!(
        holds.refresh(
            4,
            &key(HEAD),
            key(HEAD),
            facts(Ruleset::Protects, 1_060),
            Reread::AsksOnceMore
        ),
        Ok(Refreshed::Same)
    );
    let forge = Forge::default();

    assert_eq!(
        holds.press(4, &key(HEAD), Seen::None, the_window(), &forge),
        Err(NotMerged::RulesetChanged)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "the ask asks again");
    assert_eq!(waiting[0].facts.ruleset.seen(), Seen::Protects);

    // Shown the state the fresh read gives, the press goes through.
    assert!(
        holds
            .press(4, &key(HEAD), Seen::Protects, the_window(), &forge)
            .is_ok()
    );
    assert_eq!(*forge.merged.lock().unwrap(), [key(HEAD)]);
}

/// R-150, unchanged: a "No ruleset here" the fresh read still gives is accepted.
#[test]
fn a_no_ruleset_acknowledgement_holds_while_the_fresh_read_gives_the_same_state() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(Ruleset::None, 1_000), 1_000);
    assert_eq!(
        holds.refresh(
            4,
            &key(HEAD),
            key(HEAD),
            facts(Ruleset::None, 1_060),
            Reread::AsksOnceMore
        ),
        Ok(Refreshed::Same)
    );
    let forge = Forge::default();
    assert!(
        holds
            .press(4, &key(HEAD), Seen::None, the_window(), &forge)
            .is_ok()
    );
    assert_eq!(forge.merged.lock().unwrap().len(), 1);
}

/// Q248: a re-read replaces what the ask shows with the time it was read, waiting since it was
/// first raised; a re-read that finds the head moved retires the ask, which its head can no
/// longer merge, for one at the new head (Q242); and a re-read of an ask that waits no more
/// raises nothing.
#[test]
fn a_reread_refreshes_a_waiting_ask_and_never_raises_one_that_is_gone() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(Ruleset::None, 1_000), 1_000);

    assert_eq!(
        holds.refresh(
            4,
            &key(HEAD),
            key(HEAD),
            facts(Ruleset::None, 1_060),
            Reread::AsksOnceMore
        ),
        Ok(Refreshed::Same)
    );
    let waiting = holds.every();
    assert_eq!(waiting[0].facts.read_at, 1_060, "the read keeps its time");
    assert_eq!(waiting[0].since, 1_000);

    assert_eq!(
        holds.refresh(
            4,
            &key(HEAD),
            key(MOVED),
            facts(Ruleset::None, 1_120),
            Reread::AsksOnceMore
        ),
        Ok(Refreshed::Moved)
    );
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "the ask at the old head is retired");
    assert_eq!(waiting[0].key, key(MOVED));

    assert!(holds.dont_merge(4, &key(MOVED), the_window()));
    assert_eq!(
        holds.refresh(
            4,
            &key(MOVED),
            key(MOVED),
            facts(Ruleset::None, 1_180),
            Reread::AsksOnceMore
        ),
        Err(NotMerged::NotHeld)
    );
    assert!(holds.every().is_empty(), "nothing is raised again");
}

/// The stale duplicate row: the person presses at the old head while the chat tries again at
/// a new one, and the forge refuses the old head. The ask at the old head is not put back
/// beside the new one: its head can't be merged, and one row stands for the request.
#[test]
fn a_press_at_an_old_head_racing_a_retry_leaves_only_the_new_ask() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(HEAD), facts(Ruleset::None, 1_000), 1_000);
    let forge = Forge {
        refuses: Some("Head branch was modified"),
        meanwhile: Some(Box::new(|| {
            holds.raise(4, key(MOVED), facts(Ruleset::None, 1_010), 1_010);
        })),
        ..Forge::default()
    };

    holds
        .refresh(
            4,
            &key(HEAD),
            key(HEAD),
            facts(Ruleset::None, 1_000),
            Reread::AsksOnceMore,
        )
        .expect("asked once more");
    assert_eq!(
        holds.press(4, &key(HEAD), Seen::None, the_window(), &forge),
        Err(NotMerged::Forge("Head branch was modified".to_owned()))
    );
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].key, key(MOVED));
}
