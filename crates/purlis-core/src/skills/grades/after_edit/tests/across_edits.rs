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
    let planned = grades::planned(root, &Level::project(), "deploy", Some("work")).unwrap();
    let terms = Terms {
        profile: Some("work"),
        model: None,
        token_cap: planned.cap.tokens,
        limit_usd: 1.0,
    };
    let previous = planned.previous.as_ref().map(|one| one.commit.clone());
    (Now::of(&planned, &terms).unwrap(), previous)
}

/// What turning grading after each edit on from that plan keeps, as the window keeps it.
fn turned_on(now: &Now, previous: Option<String>) -> Accepted {
    Accepted {
        profile: now.profile.clone(),
        model: None,
        chats: now.chats,
        token_cap: now.token_cap,
        limit_usd: now.limit_usd,
        tests: now.tests.clone(),
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
    let (shown, previous) = now_of(root);
    assert_eq!(previous.as_deref(), Some(first.as_str()));
    let accepted = turned_on(&shown, previous.clone());

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
fn an_edit_that_brings_a_previous_versions_arm_still_waits_for_the_person_4016() {
    // A skill with one version is graded in two arms; its first edit commits a version before
    // it, and the third arm runs more chats than the person accepted.
    let plane = plane();
    let root = plane.path();
    let (shown, previous) = now_of(root);
    assert_eq!(previous, None);
    let accepted = turned_on(&shown, previous);

    saved(root, "Step one.", "Step two.");

    let (now, _) = now_of(root);
    assert!(now.chats > accepted.chats, "{} chats", now.chats);
    assert!(
        matches!(next(&accepted, &now), Next::Waits(why) if why.contains("more than the")),
        "the cost the person saw grew"
    );
}
