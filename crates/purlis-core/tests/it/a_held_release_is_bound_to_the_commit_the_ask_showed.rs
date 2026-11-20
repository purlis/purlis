// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A release is made against the commit the person was shown, never a branch that moves**
//! (ADR 0128 D8.2): a `target_commitish` naming a branch is rewritten to the commit it was at when
//! the ask was made, the tag is made at that commit first, and the release is made against the tag
//! and names no `target_commitish`.

use purlis_core::heldact::{Act, Asked, HeldActs, NotDone, Pressed, Refused, Requested};

use crate::support::held_acts::{Fake, NoTrees, Rec, Second, repo, sha, tag, the_person, world};

fn release(rev: Option<&str>) -> Requested {
    Requested::Release {
        repo: repo("acme/app"),
        tag: tag("v2.0.0"),
        rev: rev.map(str::to_owned),
        title: "Two".into(),
        notes: "What changed:\n- everything".into(),
        draft: false,
        prerelease: true,
    }
}

fn forge() -> Fake {
    let forge = Fake::github();
    forge.with(|s| {
        s.revs.insert("main".into(), sha('1'));
        s.revs.insert("side".into(), sha('5'));
        s.merged.insert(sha('1'));
    });
    forge
}

fn press_twice(
    held: &HeldActs,
    world: &purlis_core::heldact::World<'_>,
    id: u64,
) -> Result<Pressed, NotDone> {
    let (_, digest) = held.shown(id).unwrap();
    held.press(world, id, &digest, the_person())?;
    held.press(world, id, &digest, the_person())
}

#[test]
fn a_branch_that_moves_after_the_ask_changes_nothing_about_what_is_released() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release(Some("main")), 10) else {
        panic!("asked");
    };
    let (shown, _) = held.shown(id).unwrap();
    assert!(
        shown
            .rows
            .iter()
            .any(|r| r.label == "Commit" && r.value == sha('1').as_str())
    );
    assert_eq!(
        shown.body.as_ref().unwrap().text,
        "What changed:\n- everything"
    );

    // `main` moves on while the ask waits.
    forge.with(|s| {
        s.revs.insert("main".into(), sha('9'));
        s.merged.insert(sha('9'));
    });
    let Ok(Pressed::Done(done)) = press_twice(&held, &world, id) else {
        panic!("the release stands as asked");
    };
    let Act::Release {
        target,
        creates_tag,
        ..
    } = &done.act
    else {
        panic!()
    };
    assert_eq!((target, creates_tag), (&sha('1'), &true));
    // The tag first, at the commit the person saw; then the release against that tag.
    assert_eq!(
        forge.log(),
        vec![
            format!("create_tag acme/app v2.0.0 {}", sha('1').as_str()),
            format!(
                "create_release acme/app v2.0.0 at {} title=\"Two\" notes=\"What changed:\\n- everything\" \
                 draft=false prerelease=true",
                sha('1').as_str()
            ),
        ]
    );
}

#[test]
fn a_release_with_no_target_is_bound_to_the_default_branch_as_it_was_at_the_ask() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release(None), 10) else {
        panic!()
    };
    forge.with(|s| s.default = Some((crate::support::held_acts::base("main"), sha('8'))));
    let Ok(Pressed::Done(_)) = press_twice(&held, &world, id) else {
        panic!()
    };
    assert!(
        forge.log()[0].ends_with(sha('1').as_str()),
        "{:?}",
        forge.log()
    );
}

#[test]
fn a_release_against_a_tag_that_exists_is_made_at_the_tags_commit_and_makes_no_tag() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    forge.with(|s| {
        s.tags.insert("v2.0.0".into(), sha('1'));
    });
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release(None), 10) else {
        panic!()
    };
    let (shown, _) = held.shown(id).unwrap();
    assert!(
        shown
            .rows
            .iter()
            .any(|r| r.label == "Makes the tag" && r.value.starts_with("No"))
    );
    let Ok(Pressed::Done(_)) = press_twice(&held, &world, id) else {
        panic!()
    };
    assert_eq!(forge.log().len(), 1);
    assert!(forge.log()[0].starts_with("create_release acme/app v2.0.0 at "));
}

#[test]
fn a_release_that_would_move_an_existing_tag_is_refused() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    forge.with(|s| {
        s.tags.insert("v2.0.0".into(), sha('3'));
    });
    assert_eq!(
        held.ask(&world, 1, true, &release(Some("main")), 10),
        Asked::Refused(Refused::TagExists {
            name: tag("v2.0.0"),
            at: sha('3')
        })
    );
}

#[test]
fn an_existing_tag_that_moves_after_the_ask_refuses_the_press() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    forge.with(|s| {
        s.tags.insert("v2.0.0".into(), sha('1'));
    });
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release(None), 10) else {
        panic!()
    };
    forge.with(|s| {
        s.tags.insert("v2.0.0".into(), sha('4'));
    });
    let refused = press_twice(&held, &world, id).unwrap_err();
    assert_eq!(
        refused,
        NotDone::Refused(Refused::TagExists {
            name: tag("v2.0.0"),
            at: sha('4')
        })
    );
    assert!(forge.log().is_empty());
}

#[test]
fn a_commit_the_forge_no_longer_has_refuses_the_press() {
    let (forge, held) = (forge(), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let world = world(&forge, &NoTrees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &release(Some("side")), 10) else {
        panic!()
    };
    // A force-push removed the commit the ask was bound to.
    forge.with(|s| {
        s.revs.clear();
        s.known.clear();
    });
    let (_, digest) = held.shown(id).unwrap();
    assert!(matches!(
        held.press(&world, id, &digest, the_person()),
        Err(NotDone::Refused(Refused::NoSuchCommit { .. }))
    ));
    assert!(forge.log().is_empty());
}
