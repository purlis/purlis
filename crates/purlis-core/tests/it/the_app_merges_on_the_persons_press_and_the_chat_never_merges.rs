// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app performs merges itself; a chat never merges** (R-115, Q9, Q97). A chat's merge
//! command is held whatever its mode, and the merge it names waits as one ask. The person's
//! press asks the forge to merge exactly the request and head the ask is keyed on, once: a
//! second press merges nothing. Here the forge is a fake that counts its merges.
//!
//! The hook's refusal is in `crates/purlis-cli/tests/it/a_held_action_is_refused_to_the_chat_and_raises_one_ask.rs`.

use std::sync::Mutex;

use purlis_core::harness::asks::{Admitted, Answerer};
use purlis_core::mergehold::{
    self, At, Held, Holds, MergeForge, MergeKey, Merged, NotMerged, Raised, Request, Reread, Seen,
};

/// A forge that merges what it is asked to, counting each merge and the method, or refuses
/// every one; its request merges into `base` (main where none is said).
#[derive(Default)]
struct Forge {
    merged: Mutex<Vec<(MergeKey, bool)>>,
    refuses: Option<&'static str>,
    base: Option<&'static str>,
    squash: bool,
}

impl MergeForge for Forge {
    fn at(&self, _key: &MergeKey) -> Result<At, String> {
        Ok(At {
            base: self.base.unwrap_or("main").try_into().unwrap(),
            squash: self.squash,
        })
    }

    fn merge_at(&self, key: &MergeKey, squash: bool) -> Result<Merged, String> {
        if let Some(why) = self.refuses {
            return Err(why.to_owned());
        }
        self.merged.lock().unwrap().push((key.clone(), squash));
        Ok(Merged::Now)
    }
}

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn key(number: u64) -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number,
        head: HEAD.try_into().unwrap(),
        base: "main".try_into().unwrap(),
    }
}

fn the_window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

#[test]
fn the_app_merges_on_the_persons_press_and_the_chat_never_merges() {
    purlis_core::unsteered!();
    // The chat's own merge, in every spelling a forge CLI has, is held: never run.
    for cmd in [
        "gh pr merge 12",
        "gh pr merge 12 --squash --delete-branch",
        "gh pr -R o/r merge 12",
        "glab mr merge 12",
        "glab mr accept 12",
    ] {
        assert!(
            matches!(
                mergehold::held(cmd),
                Some(Held::Request(Request { number: 12, .. }))
            ),
            "{cmd}: {:?}",
            mergehold::held(cmd)
        );
    }
    // And one whose request cannot be read is still held, with nobody asked.
    for cmd in [
        "gh api -X PUT repos/o/r/pulls/12/merge",
        "gh pr merge",
        "gh pr merge 12 && glab mr merge 3",
    ] {
        assert_eq!(mergehold::held(cmd), Some(Held::Unreadable), "{cmd}");
    }
    // What is no merge is not held.
    for cmd in [
        "gh pr view 12",
        "gh pr create --fill",
        "git merge main",
        "git tag v1",
    ] {
        assert_eq!(mergehold::held(cmd), None, "{cmd}");
    }

    let holds = Holds::default();
    let forge = Forge::default();
    let request = Request {
        repo: None,
        number: 12,
    };
    let (asked, raised) = mergehold::asked(&holds, 4, &request, &|_| Ok((key(12), facts())), 1_000)
        .expect("the request is found");
    assert_eq!(raised, mergehold::Raised::New);
    // Holding it merged nothing.
    assert!(forge.merged.lock().unwrap().is_empty());

    asked_once_more(&holds, &asked);
    let pressed = holds.press(4, &asked, Seen::None, the_window(), &forge);
    assert!(pressed.is_ok(), "{pressed:?}");
    assert_eq!(*forge.merged.lock().unwrap(), [(key(12), false)]);
    assert!(holds.every().is_empty());
    // A second press, a double click say, merges nothing more.
    assert_eq!(
        holds.press(4, &asked, Seen::None, the_window(), &forge),
        Err(NotMerged::NotHeld)
    );
    assert_eq!(forge.merged.lock().unwrap().len(), 1);
}

#[test]
fn a_merge_the_forge_refuses_waits_again_and_dont_merge_merges_nothing() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(12), facts(), 1_000);
    let refusing = Forge {
        refuses: Some("Head branch was modified"),
        ..Forge::default()
    };
    asked_once_more(&holds, &key(12));
    assert_eq!(
        holds.press(4, &key(12), Seen::None, the_window(), &refusing),
        Err(NotMerged::Forge("Head branch was modified".to_owned()))
    );
    assert_eq!(holds.every().len(), 1);

    let forge = Forge::default();
    assert!(holds.dont_merge(4, &key(12), the_window()));
    assert_eq!(
        holds.press(4, &key(12), Seen::None, the_window(), &forge),
        Err(NotMerged::NotHeld)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
}

/// Q242: a try again of a request whose branch moved is a new ask at the head the forge says
/// now. The ask at the old head is gone, so nothing the person pressed or confirmed on it
/// carries over, and a press naming the old head merges nothing.
#[test]
fn a_retried_merge_at_a_new_head_is_a_new_ask_and_the_old_one_is_gone() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    assert_eq!(holds.raise(4, key(12), facts(), 1_000), Raised::New);
    let moved = MergeKey {
        head: "89abcdef0123456789abcdef0123456789abcdef"
            .try_into()
            .unwrap(),
        ..key(12)
    };
    assert_eq!(holds.raise(4, moved.clone(), facts(), 2_000), Raised::New);
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1, "the ask at the old head waits no more");
    assert_eq!(waiting[0].key, moved);
    assert_eq!(
        waiting[0].since, 2_000,
        "it is a new ask, waiting since now"
    );
    let forge = Forge::default();
    assert_eq!(
        holds.press(4, &key(12), Seen::None, the_window(), &forge),
        Err(NotMerged::NotHeld)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
    asked_once_more(&holds, &moved);
    assert!(
        holds
            .press(4, &moved, Seen::None, the_window(), &forge)
            .is_ok()
    );
}

/// A try again at the same head and base is still the one ask, waiting since the first try.
#[test]
fn a_retried_merge_at_the_same_head_is_still_one_ask() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    assert_eq!(holds.raise(4, key(12), facts(), 1_000), Raised::New);
    assert_eq!(holds.raise(4, key(12), facts(), 2_000), Raised::Already);
    let waiting = holds.every();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].since, 1_000);
}

/// A try again at a new head is a new ask in the old one's place, so a chat already at its
/// most merge asks can still refresh one.
#[test]
fn a_retried_merge_at_a_new_head_takes_the_old_asks_place_under_the_limit() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    for number in 1..=mergehold::AT_MOST_PER_CHAT as u64 {
        assert_eq!(holds.raise(4, key(number), facts(), 1_000), Raised::New);
    }
    let moved = MergeKey {
        head: "89abcdef0123456789abcdef0123456789abcdef"
            .try_into()
            .unwrap(),
        ..key(1)
    };
    assert_eq!(holds.raise(4, moved, facts(), 2_000), Raised::New);
    assert_eq!(holds.every().len(), mergehold::AT_MOST_PER_CHAT);
}

/// Q239 (S7): the base is pinned in the key, and a press on a request retargeted since the ask
/// merges nothing and says so; the ask waits again.
#[test]
fn a_press_on_a_request_whose_base_moved_merges_nothing() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(12), facts(), 1_000);
    let retargeted = Forge {
        base: Some("release"),
        ..Forge::default()
    };
    asked_once_more(&holds, &key(12));
    assert_eq!(
        holds.press(4, &key(12), Seen::None, the_window(), &retargeted),
        Err(NotMerged::BaseMoved {
            was: "main".to_owned(),
            now: "release".to_owned()
        })
    );
    assert!(retargeted.merged.lock().unwrap().is_empty());
    assert_eq!(holds.every().len(), 1);
}

/// Q239 (S5): the press merges by the repo's own method, as the forge says it at the press.
#[test]
fn a_press_merges_by_the_repos_own_method() {
    purlis_core::unsteered!();
    let holds = Holds::default();
    holds.raise(4, key(12), facts(), 1_000);
    let squashes = Forge {
        squash: true,
        ..Forge::default()
    };
    asked_once_more(&holds, &key(12));
    assert!(
        holds
            .press(4, &key(12), Seen::None, the_window(), &squashes)
            .is_ok()
    );
    assert_eq!(*squashes.merged.lock().unwrap(), [(key(12), true)]);
}

/// The person pressed Merge once on chat 4's ask for `key`: the window read it again and asked
/// once more (Q269).
fn asked_once_more(holds: &Holds, key: &MergeKey) {
    holds
        .refresh(4, key, key.clone(), facts(), Reread::AsksOnceMore)
        .expect("asked once more");
}

/// What the forge said of the request, as a merge ask shows it.
fn facts() -> purlis_core::mergehold::Facts {
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
        approved: false,
        read_at: 40,
    }
}
