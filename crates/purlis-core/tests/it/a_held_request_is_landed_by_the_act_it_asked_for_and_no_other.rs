// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A held request is landed by the act it asked for, and no other** (#7543). A chat's merge
//! or enqueue each raises one ask that carries the act. The person's press performs that act,
//! pinned to the head the ask read: a merge merges, an enqueue puts the request in the merge
//! queue and never merges it. (Auto-merge is not held: it would merge any head pushed later,
//! unseen.) A head that moved retires the ask for one at the new head, which keeps the act. A
//! forge that cannot do the act refuses it with a reason, the ask waits, and nothing else is
//! done in its place. The act is part of what a press is bound to ([`Facts::digest`]).

use std::sync::{Arc, Mutex};

use purlis_core::forge::backend::Caller;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::{Forge as ForgeOf, Kind};
use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    At, ByForge, Facts, Holds, LandAs, MergeForge, MergeKey, Merged, NotMerged, Raised, Refreshed,
    Reread, Ruleset, Seen,
};
use serde_json::json;

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const MOVED: &str = "89abcdef0123456789abcdef0123456789abcdef";

fn key_at(head: &str) -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number: 7,
        head: head.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

fn facts() -> Facts {
    Facts {
        title: "Add the parser".to_owned(),
        author: "octo".to_owned(),
        checks: purlis_core::forge::checks::Checks {
            total: Some(1),
            ci: purlis_core::forge::checks::Ci::Passed,
            why: None,
            failed: Vec::new(),
        },
        ruleset: Ruleset::None,
        approved_by: None,
        squash: false,
        read_at: 40,
    }
}

fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

/// Which backend call a press made, and at which head.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Called {
    Merge(String),
    Enqueue(String),
}

/// A forge that records the act each call performs; `refuses` makes the enqueue fail as a repo
/// with no queue would.
#[derive(Default)]
struct Forge {
    calls: Mutex<Vec<Called>>,
    refuses: Option<&'static str>,
}

impl MergeForge for Forge {
    fn at(&self, _: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: "main".try_into().unwrap(),
            squash: false,
        })
    }

    fn merge_at(&self, key: &MergeKey, _: bool) -> Result<Merged, String> {
        self.calls
            .lock()
            .unwrap()
            .push(Called::Merge(key.head.as_str().to_owned()));
        Ok(Merged::Now)
    }

    fn enqueue_at(&self, key: &MergeKey, _: bool) -> Result<Merged, String> {
        if let Some(why) = self.refuses {
            return Err(why.to_owned());
        }
        self.calls
            .lock()
            .unwrap()
            .push(Called::Enqueue(key.head.as_str().to_owned()));
        Ok(Merged::Queued)
    }
}

/// A forge that implements only a merge, as a backend with no queue would.
struct MergeOnly(Mutex<u32>);

impl MergeForge for MergeOnly {
    fn at(&self, _: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: "main".try_into().unwrap(),
            squash: false,
        })
    }

    fn merge_at(&self, _: &MergeKey, _: bool) -> Result<Merged, String> {
        *self.0.lock().unwrap() += 1;
        Ok(Merged::Now)
    }
}

fn raised_as(holds: &Holds, act: LandAs, head: &str) {
    assert_eq!(
        holds.raise_as(4, key_at(head), facts(), act, 1_000),
        Raised::New
    );
}

fn asked_once_more(holds: &Holds, key: &MergeKey) {
    holds
        .refresh(4, key, key.clone(), facts(), Reread::AsksOnceMore)
        .expect("asked once more");
}

#[test]
fn each_act_is_performed_by_its_own_call_pinned_to_the_head_the_ask_read() {
    purlis_core::unsteered!();
    for (act, called) in [
        (LandAs::Merge, Called::Merge(HEAD.to_owned())),
        (LandAs::Enqueue, Called::Enqueue(HEAD.to_owned())),
    ] {
        let holds = Holds::default();
        raised_as(&holds, act, HEAD);
        assert_eq!(holds.every()[0].act, act, "{act:?}: the ask names it");
        asked_once_more(&holds, &key_at(HEAD));
        let forge = Forge::default();
        holds
            .press(4, &key_at(HEAD), Seen::None, the_window(), &forge)
            .unwrap_or_else(|why| panic!("{act:?}: {why}"));
        assert_eq!(
            *forge.calls.lock().unwrap(),
            [called],
            "{act:?}: only its own call"
        );
        assert!(holds.every().is_empty(), "{act:?}: answered");
    }
}

#[test]
fn an_enqueue_the_forge_refuses_is_never_a_merge_and_its_ask_waits() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised_as(&holds, LandAs::Enqueue, HEAD);
    asked_once_more(&holds, &key_at(HEAD));
    let forge = Forge {
        refuses: Some("o/r has no merge queue"),
        ..Forge::default()
    };
    assert_eq!(
        holds.press(4, &key_at(HEAD), Seen::None, the_window(), &forge),
        Err(NotMerged::Forge("o/r has no merge queue".to_owned()))
    );
    assert!(
        forge.calls.lock().unwrap().is_empty(),
        "no merge in its place"
    );
    assert_eq!(
        holds.every()[0].act,
        LandAs::Enqueue,
        "still waiting, still an enqueue"
    );
}

#[test]
fn an_enqueue_on_a_forge_that_has_no_queue_is_refused_and_merges_nothing() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised_as(&holds, LandAs::Enqueue, HEAD);
    asked_once_more(&holds, &key_at(HEAD));
    let forge = MergeOnly(Mutex::new(0));
    let pressed = holds.press(4, &key_at(HEAD), Seen::None, the_window(), &forge);
    assert!(matches!(pressed, Err(NotMerged::Forge(_))), "{pressed:?}");
    assert_eq!(*forge.0.lock().unwrap(), 0, "never merged");
    assert_eq!(holds.every().len(), 1, "the ask waits");
}

#[test]
fn a_moved_head_retires_the_ask_and_the_one_at_the_new_head_keeps_the_act() {
    purlis_core::unsteered!();
    for act in [LandAs::Merge, LandAs::Enqueue] {
        let holds = Holds::default();
        raised_as(&holds, act, HEAD);
        asked_once_more(&holds, &key_at(HEAD));
        assert_eq!(
            holds.refresh(4, &key_at(HEAD), key_at(MOVED), facts(), Reread::Watches),
            Ok(Refreshed::Moved)
        );
        let forge = Forge::default();
        assert_eq!(
            holds.press(4, &key_at(HEAD), Seen::None, the_window(), &forge),
            Err(NotMerged::NotHeld),
            "{act:?}: the old head's ask is gone"
        );
        let now = holds.every();
        assert_eq!(
            (now.len(), now[0].act, now[0].asked),
            (1, act, false),
            "{act:?}"
        );
        assert_eq!(
            holds.press(4, &key_at(MOVED), Seen::None, the_window(), &forge),
            Err(NotMerged::NotAsked),
            "{act:?}: the person is asked again first"
        );
        assert!(
            forge.calls.lock().unwrap().is_empty(),
            "{act:?}: nothing was done"
        );
    }
}

#[test]
fn a_try_again_for_another_act_at_the_same_head_asks_the_person_once_more() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised_as(&holds, LandAs::Merge, HEAD);
    asked_once_more(&holds, &key_at(HEAD));
    assert_eq!(
        holds.raise_as(4, key_at(HEAD), facts(), LandAs::Enqueue, 2_000),
        Raised::Already
    );
    assert_eq!(holds.every().len(), 1, "still the one ask");
    let forge = Forge::default();
    assert_eq!(
        holds.press(4, &key_at(HEAD), Seen::None, the_window(), &forge),
        Err(NotMerged::NotAsked),
        "what the person was asked is not what a press would do"
    );
    asked_once_more(&holds, &key_at(HEAD));
    holds
        .press(4, &key_at(HEAD), Seen::None, the_window(), &forge)
        .expect("pressed");
    assert_eq!(
        *forge.calls.lock().unwrap(),
        [Called::Enqueue(HEAD.to_owned())],
        "the newest act, and no merge"
    );
}

/// What a press is bound to includes the act: a press made with the digest of a row that showed
/// another act is refused as changed, even though no other fact moved.
#[test]
fn a_press_made_against_a_row_showing_another_act_is_refused_as_changed() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    raised_as(&holds, LandAs::Enqueue, HEAD);
    asked_once_more(&holds, &key_at(HEAD));
    let old = facts().digest(LandAs::Enqueue);
    assert_ne!(
        old,
        facts().digest(LandAs::Merge),
        "the act is in the digest"
    );
    assert!(holds.shows(4, &key_at(HEAD), &old));
    // The chat tries again, as a merge: the one ask is now for a merge and asks once more.
    assert_eq!(
        holds.raise_as(4, key_at(HEAD), facts(), LandAs::Merge, 2_000),
        Raised::Already
    );
    asked_once_more(&holds, &key_at(HEAD));
    assert!(
        !holds.shows(4, &key_at(HEAD), &old),
        "the old digest is of an enqueue"
    );
    assert!(holds.shows(4, &key_at(HEAD), &facts().digest(LandAs::Merge)));
}

/// Through the real GitHub backend over a recorded transport: an enqueue of a repo whose branch
/// has no merge queue is refused after the one read of how it lands, and no merge call is made.
#[test]
fn an_enqueue_through_the_github_backend_on_a_repo_with_no_queue_makes_no_merge_call() {
    purlis_core::unsteered!();
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/github/lands_through_queue.json");
    let mut exchanges = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(at).unwrap(),
    )
    .unwrap()["exchanges"]
        .as_array()
        .unwrap()
        .clone();
    exchanges[0]["reply"]["out"] = json!(
        json!({"data": {"repository": {"pullRequest": {"id": "PR_kwDOAbc",
               "isMergeQueueEnabled": false}}}})
        .to_string()
    );
    let text = json!({"source": "the forge contract's recordings", "exchanges": exchanges});
    let transport = Arc::new(Recorded::parse(&text.to_string()).unwrap());
    let backend = ForgeOf::default_of(Kind::GitHub).backend_over(transport.clone());
    let forge = ByForge {
        backend: backend.as_ref(),
        caller: Caller::window(),
        path: "acme/api".to_owned(),
    };
    let key = MergeKey {
        repo: "acme/api".try_into().unwrap(),
        number: 12,
        ..key_at(HEAD)
    };
    let refused = forge.enqueue_at(&key, false);
    let Err(why) = refused else {
        panic!("an enqueue with no queue was done: {refused:?}");
    };
    assert!(why.contains("no merge queue"), "{why}");
    assert!(why.contains("not merged"), "{why}");
    assert!(
        transport.unspent().is_empty(),
        "the one read was all it asked"
    );
}

/// The recorded enqueue, read as the press asks it: how the request lands (once, by the press)
/// and then the enqueue's own read of its node id, so the probe is recorded twice.
fn recorded_enqueue() -> Arc<Recorded> {
    let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/github/enqueue_at.json");
    let mut exchanges = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(at).unwrap(),
    )
    .unwrap()["exchanges"]
        .as_array()
        .unwrap()
        .clone();
    exchanges.insert(0, exchanges[0].clone());
    let text =
        json!({"source": "the forge contract's recording of enqueue_at", "exchanges": exchanges});
    Arc::new(Recorded::parse(&text.to_string()).unwrap())
}

/// Through the real GitHub backend over the recorded enqueue: the press of an enqueue puts the
/// request in the queue with `expectedHeadOid` set to the head the ask read. The recording is
/// replayed only for a call that matches it field for field, so a call without that head, or at
/// another, would find no exchange and fail.
#[test]
fn an_enqueue_through_the_github_backend_sends_the_head_the_ask_read() {
    purlis_core::unsteered!();
    const ASKED_AT: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";
    let key = MergeKey {
        repo: "acme/api".try_into().unwrap(),
        number: 12,
        ..key_at(ASKED_AT)
    };
    let enqueue = |key: &MergeKey| {
        let transport = recorded_enqueue();
        let backend = ForgeOf::default_of(Kind::GitHub).backend_over(transport.clone());
        let forge = ByForge {
            backend: backend.as_ref(),
            caller: Caller::window(),
            path: "acme/api".to_owned(),
        };
        (forge.enqueue_at(key, false), transport.unspent())
    };
    let (queued, unspent) = enqueue(&key);
    assert_eq!(queued, Ok(Merged::Queued));
    assert!(unspent.is_empty(), "every exchange was asked: {unspent:?}");
    // At another head the same press finds no recorded exchange: the head is in the call.
    let other = MergeKey {
        head: HEAD.try_into().unwrap(),
        ..key
    };
    let (queued, _) = enqueue(&other);
    assert!(queued.is_err(), "{queued:?}");
}
