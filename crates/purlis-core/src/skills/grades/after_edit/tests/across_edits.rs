// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for grading after each edit across the person's edits (#4016), on a temporary plane
//! that is a real git repository: each saved edit is committed (`verbs::edit`), so the version a
//! grade compares with is the one before that edit, and it moves with every edit.

use std::path::Path;

use crate::repocmd::Say;
use crate::skills::grades::tests::{plane, plane_of_two_versions};
use crate::skills::grades::{self, Terms};
use crate::skills::verbs::{self, By, Ctx};

use super::super::{Accepted, Level, Next, Now, next};

/// What a grade of the project's `deploy` would run now, on the profile `work` and its own
/// model, held to a dollar.
fn now_of(root: &Path) -> (Now, Option<String>) {
    let (now, previous, _) = shown_of(root);
    (now, previous)
}

/// That, and the chats the plan that turns grading after each edit on shows and keeps.
fn shown_of(root: &Path) -> (Now, Option<String>, u32) {
    let planned = grades::planned(root, &Level::project(), "deploy", Some("work")).unwrap();
    let terms = Terms {
        profile: Some("work"),
        model: None,
        token_cap: planned.cap.tokens,
        limit_usd: 1.0,
        grader: None,
    };
    let previous = planned.previous.as_ref().map(|one| one.commit.clone());
    let accepts = planned.after_edit_chats.unwrap();
    (Now::of(&planned, &terms).unwrap(), previous, accepts)
}

/// What turning grading after each edit on from that plan keeps, as the window keeps it, with
/// `chats` the chats it shows.
fn turned_on(now: &Now, previous: Option<String>, chats: u32) -> Accepted {
    Accepted {
        profile: now.profile.clone(),
        model: None,
        chats,
        token_cap: now.token_cap,
        limit_usd: now.limit_usd,
        tests: now.tests.clone(),
        outcomes: None,
        evals: Vec::new(),
        previous,
        runs_on: now.runs_on.clone(),
        at: "2026-10-30T10:00:00+00:00".into(),
        graded: None,
        paused: None,
    }
}

/// The window's save of `deploy`'s editor: `from` replaced by `to` in its `SKILL.md`.
fn saved(root: &Path, from: &str, to: &str) {
    let level = Level::project();
    let (_, was) = verbs::text_of(root, &level, "deploy").unwrap();
    let mut say = |_: Say| {};
    verbs::edit(
        &mut Ctx {
            plane: root,
            built_in: None,
            by: By::Window,
            say: &mut say,
        },
        &level,
        "deploy",
        &was,
        &was.replace(from, to),
    )
    .unwrap();
}

#[test]
fn each_edit_of_a_skill_with_a_previous_version_is_graded_without_asking_again_4016() {
    let (plane, first) = plane_of_two_versions();
    let root = plane.path();
    let (shown, previous, accepts) = shown_of(root);
    assert_eq!(previous.as_deref(), Some(first.as_str()));
    assert_eq!(
        accepts, shown.chats,
        "its previous version's arm is in the plan already"
    );
    let accepted = turned_on(&shown, previous.clone(), accepts);

    saved(root, "Step two.", "Step three.");
    let (now, moved) = now_of(root);
    assert_ne!(
        moved, previous,
        "the edit is committed, so the version before it is the one it compares with"
    );
    assert_eq!(next(&accepted, &now), Next::Grade, "the first edit grades");

    // As the window keeps what it graded once that grade starts.
    let graded = Accepted {
        graded: Some(now.consent.clone()),
        ..accepted
    };
    assert_eq!(next(&graded, &now), Next::Same, "the same copy never twice");
    saved(root, "Step three.", "Step four.");
    assert_eq!(next(&graded, &now_of(root).0), Next::Grade, "nor the next");
}

#[test]
fn a_skill_with_one_version_accepts_its_previous_versions_arm_up_front_4035() {
    // A skill with one version is graded in two arms; its first edit commits a version before
    // it, and a third arm comes. The plan that turns grading after each edit on shows and keeps
    // the chats with that arm counted, so the first edit grades without asking again.
    let plane = plane();
    let root = plane.path();
    let (shown, previous, accepts) = shown_of(root);
    assert_eq!(previous, None);
    assert_eq!(accepts, shown.chats / 2 * 3, "the third arm is counted");
    let accepted = turned_on(&shown, previous, accepts);

    saved(root, "Step one.", "Step two.");

    let (now, _) = now_of(root);
    assert_eq!(now.chats, accepted.chats, "no more than the plan showed");
    assert_eq!(now.limit_usd, accepted.limit_usd, "nor may it spend more");
    assert_eq!(next(&accepted, &now), Next::Grade);
}

#[test]
fn an_opt_in_kept_on_two_arms_waits_at_the_first_edit_and_says_why_4035() {
    // An opt-in kept before the third arm was counted (or with fewer chats) still never runs
    // more chats than it accepted: it asks again, and names the previous version as the reason.
    let plane = plane();
    let root = plane.path();
    let (shown, previous) = now_of(root);
    let accepted = turned_on(&shown, previous, shown.chats);

    saved(root, "Step one.", "Step two.");

    let (now, _) = now_of(root);
    assert!(now.chats > accepted.chats, "{} chats", now.chats);
    let Next::Waits(why) = next(&accepted, &now) else {
        panic!("it graded past the chats accepted");
    };
    assert!(why.contains("its previous version"), "{why}");
    assert!(why.contains("more than the"), "{why}");
}
