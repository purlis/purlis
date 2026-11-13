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
use purlis_core::mergehold::{self, Held, Holds, MergeForge, MergeKey, NotMerged, Request};

/// A forge that merges what it is asked to, counting each merge, or refuses every one.
#[derive(Default)]
struct Forge {
    merged: Mutex<Vec<MergeKey>>,
    refuses: Option<&'static str>,
}

impl MergeForge for Forge {
    fn merge_at(&self, key: &MergeKey) -> Result<(), String> {
        if let Some(why) = self.refuses {
            return Err(why.to_owned());
        }
        self.merged.lock().unwrap().push(key.clone());
        Ok(())
    }
}

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn key(number: u64) -> MergeKey {
    MergeKey {
        repo: "o/r".try_into().unwrap(),
        number,
        head: HEAD.try_into().unwrap(),
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
    let (asked, raised) = mergehold::asked(&holds, 4, &request, &|_| Ok(key(12)), 1_000)
        .expect("the request is found");
    assert_eq!(raised, mergehold::Raised::New);
    // Holding it merged nothing.
    assert!(forge.merged.lock().unwrap().is_empty());

    let pressed = holds.press(4, &asked, the_window(), &forge);
    assert!(pressed.is_ok(), "{pressed:?}");
    assert_eq!(*forge.merged.lock().unwrap(), [key(12)]);
    assert!(holds.every().is_empty());
    // A second press, a double click say, merges nothing more.
    assert_eq!(
        holds.press(4, &asked, the_window(), &forge),
        Err(NotMerged::NotHeld)
    );
    assert_eq!(forge.merged.lock().unwrap().len(), 1);
}

#[test]
fn a_merge_the_forge_refuses_waits_again_and_dont_merge_merges_nothing() {
    let holds = Holds::default();
    holds.raise(4, key(12), 1_000);
    let refusing = Forge {
        refuses: Some("Head branch was modified"),
        ..Forge::default()
    };
    assert_eq!(
        holds.press(4, &key(12), the_window(), &refusing),
        Err(NotMerged::Forge("Head branch was modified".to_owned()))
    );
    assert_eq!(holds.every().len(), 1);

    let forge = Forge::default();
    assert!(holds.dont_merge(4, &key(12), the_window()));
    assert_eq!(
        holds.press(4, &key(12), the_window(), &forge),
        Err(NotMerged::NotHeld)
    );
    assert!(forge.merged.lock().unwrap().is_empty());
}
