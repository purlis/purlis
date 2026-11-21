// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's asks are rate limited, and an unattended queue is bounded and forgotten** (one login
//! F2, round 2): a chat that asks in a loop is refused with a sentence naming the limit before
//! anything is read from the forge, and a closed chat leaves nothing queued.

use purlis_core::heldact::{Asked, HeldActs, Refused, Requested, holds_global_limit, holds_limits};

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, repo, sha, tag, world};

fn tag_of(name: &str) -> Requested {
    Requested::Tag {
        repo: repo("acme/app"),
        name: tag(name),
        rev: "main".into(),
    }
}

fn forge() -> Fake {
    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
        s.merged.insert(sha('1'));
    });
    forge
}

#[test]
fn the_asks_past_the_limit_are_refused_naming_it_and_read_nothing() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let (limit, _) = holds_limits();
    for n in 0..limit {
        assert!(
            matches!(
                held.ask(&world, 1, true, &tag_of("v1.0.0"), 10 + n as u64),
                Asked::Held { .. }
            ),
            "ask {n}"
        );
    }
    let before = forge.log().len();
    let Asked::Refused(Refused::Text(said)) = held.ask(&world, 1, true, &tag_of("v1.0.0"), 100)
    else {
        panic!("refused")
    };
    assert!(said.contains("20 times"), "{said}");
    assert_eq!(forge.log().len(), before, "nothing was read from the forge");
    // Another chat is not limited by this one, and the window passes.
    assert!(matches!(
        held.ask(&world, 2, true, &tag_of("v1.0.0"), 100),
        Asked::Held { .. }
    ));
    assert!(matches!(
        held.ask(&world, 1, true, &tag_of("v1.0.0"), 100 + holds_limits().1),
        Asked::Held { .. }
    ));
}

#[test]
fn a_closed_chat_leaves_no_queued_request_for_a_desktop_to_take() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    held.ask(&world, 7, false, &tag_of("v1.0.0"), 10);
    held.ask(&world, 8, false, &tag_of("v1.0.0"), 10);
    held.forget(7);
    let taken = held.take_queued();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].0, 8);
}

#[test]
fn a_release_with_notes_longer_than_shown_is_not_queued() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let long = Requested::Release {
        repo: repo("acme/app"),
        tag: tag("v9.0.0"),
        rev: None,
        title: String::new(),
        notes: "x".repeat(100_000),
        draft: false,
        prerelease: false,
    };
    assert!(matches!(
        held.ask(&world, 1, false, &long, 10),
        Asked::Refused(Refused::Text(_))
    ));
    assert!(held.take_queued().is_empty());
}

#[test]
fn notes_of_invisible_characters_are_bounded_by_their_raw_size_not_what_is_left_of_them() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let zero_width = Requested::Release {
        repo: repo("acme/app"),
        tag: tag("v9.0.0"),
        rev: None,
        title: String::new(),
        notes: "\u{200B}".repeat(2_000_000),
        draft: false,
        prerelease: false,
    };
    assert!(matches!(
        held.ask(&world, 1, false, &zero_width, 10),
        Asked::Refused(Refused::Text(_))
    ));
    assert!(held.take_queued().is_empty());
}

#[test]
fn many_chats_each_under_their_own_limit_are_stopped_by_the_ceiling_across_them() {
    purlis_core::unsteered!();
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let ceiling = holds_global_limit();
    assert!(ceiling > holds_limits().0);
    for chat in 0..ceiling {
        assert!(
            matches!(
                held.ask(&world, chat as u32 + 100, true, &tag_of("v1.0.0"), 10),
                Asked::Held { .. }
            ),
            "chat {chat}"
        );
    }
    let Asked::Refused(Refused::Text(said)) = held.ask(&world, 9_999, true, &tag_of("v1.0.0"), 11)
    else {
        panic!("refused")
    };
    assert!(said.contains("all chats"), "{said}");
}
