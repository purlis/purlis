// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A change to several skills a chat proposes (#5357): what the person is shown, when it reads as
//! changed since, and what Apply makes, on the plane a set's own tests use (`set::tests::plane`).

use super::super::tests::{commits, did, plane, project, there, write};
use super::*;
use crate::testgit::run;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

fn grilling_everywhere() -> Vec<Member> {
    vec![
        project("grilling"),
        Member {
            level: Level::project().local(true),
            name: "grilling".to_owned(),
        },
        project("tdd"),
    ]
}

#[test]
fn a_proposed_delete_shows_each_skill_and_the_skill_left_naming_one() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Delete {
        members: grilling_everywhere(),
    };
    let shown = change.shown(root);
    assert_eq!(change.title(), "Delete 3 skills");
    assert!(
        shown.before.contains("grilling · project: there"),
        "{}",
        shown.before
    );
    assert!(
        shown.after.contains("grilling · project: deleted"),
        "{}",
        shown.after
    );
    assert!(
        shown.after.contains("tdd · project: deleted"),
        "{}",
        shown.after
    );
    assert!(
        shown.after.contains("to-spec still names grilling"),
        "{}",
        shown.after
    );
    // Shown, and nothing written.
    assert!(there(root, "skills", "grilling"));
}

#[test]
fn applying_a_proposed_delete_is_one_commit_of_every_skill_it_showed() {
    let tmp = plane();
    let root = tmp.path();
    let before = commits(root);
    let change = SetChange::Delete {
        members: vec![project("tdd"), project("review")],
    };
    did(root, |cx| change.apply(cx)).unwrap();
    assert!(!there(root, "skills", "tdd"));
    assert!(!there(root, "skills", "review"));
    assert_eq!(commits(root), before + 1);
}

#[test]
fn a_skill_that_is_not_there_stays_out_and_the_rest_are_applied() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Delete {
        members: vec![project("tdd"), project("nothing")],
    };
    let shown = change.shown(root);
    assert!(
        shown
            .after
            .contains("nothing · project: stays (It is not there any more.)"),
        "{}",
        shown.after
    );
    did(root, |cx| change.apply(cx)).unwrap();
    assert!(!there(root, "skills", "tdd"));
}

#[test]
fn a_skill_edited_since_it_was_proposed_makes_the_proposal_read_as_changed() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Delete {
        members: vec![project("tdd")],
    };
    let then = change.shown(root);
    assert_eq!(then.state, change.shown(root).state, "nothing moved yet");
    write(
        root,
        "skills",
        "tdd",
        "---\nname: tdd\ndescription: New.\n---\n\nnew\n",
    );
    assert_ne!(then.state, change.shown(root).state);
}

#[test]
fn a_proposed_turn_off_shows_and_turns_each_skill_in_the_workspace() {
    let tmp = plane();
    let root = tmp.path();
    for name in ["grilling", "tdd"] {
        crate::skills::grant(root, name, &crate::skills::Grant::All).unwrap();
    }
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "granted"]);
    let change = SetChange::Turn {
        workspace: "ops".to_owned(),
        names: names(&["grilling", "tdd"]),
        on: false,
    };
    assert_eq!(change.title(), "Turn 2 skills off in ops");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("tdd · project: on in ops"),
        "{}",
        shown.before
    );
    assert!(
        shown.after.contains("tdd · project: off in ops"),
        "{}",
        shown.after
    );
    let before = commits(root);
    did(root, |cx| change.apply(cx)).unwrap();
    assert_eq!(commits(root), before + 1);
    assert!(crate::skills::turned_off(root, "ops").contains(&"tdd".to_owned()));
}

#[test]
fn a_proposed_pin_shows_what_is_pinned_and_applies_as_one_commit() {
    let tmp = plane();
    let root = tmp.path();
    let by = crate::skills::routing::PinnedBy::Workspace("ops".to_owned());
    let change = SetChange::Pin {
        by: by.clone(),
        names: names(&["grilling", "tdd"]),
        on: true,
    };
    assert_eq!(change.title(), "Pin 2 skills for ops's chats");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("grilling: not pinned"),
        "{}",
        shown.before
    );
    assert!(shown.after.contains("grilling: pinned"), "{}", shown.after);
    let before = commits(root);
    did(root, |cx| change.apply(cx)).unwrap();
    assert_eq!(commits(root), before + 1);
    let mut pins = crate::skills::routing::pins_written(root, &by);
    pins.sort();
    assert_eq!(pins, ["grilling", "tdd"]);
}

#[test]
fn a_proposed_archive_shows_each_state_and_archives_them() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Archive {
        members: vec![project("tdd"), project("review")],
        archive: true,
    };
    assert_eq!(change.title(), "Archive 2 skills");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("tdd · project: in use"),
        "{}",
        shown.before
    );
    assert!(
        shown.after.contains("tdd · project: archived"),
        "{}",
        shown.after
    );
    did(root, |cx| change.apply(cx)).unwrap();
    let folder = root.join("skills/tdd");
    assert_eq!(
        purlis_skills::about_of(&folder).unwrap().lifecycle,
        Lifecycle::Archived
    );
}

#[test]
fn a_proposal_travels_whole_as_json() {
    let change = SetChange::Pin {
        by: crate::skills::routing::PinnedBy::Persona("steward".to_owned()),
        names: names(&["grilling"]),
        on: false,
    };
    let line = serde_json::to_string(&change).unwrap();
    assert_eq!(serde_json::from_str::<SetChange>(&line).unwrap(), change);
}

#[test]
fn a_proposal_of_no_skill_or_too_many_is_refused() {
    let none = SetChange::Delete { members: vec![] };
    assert!(none.checked().is_err());
    let many = SetChange::Turn {
        workspace: "ops".to_owned(),
        names: (0..=MOST_SKILLS).map(|n| format!("s{n}")).collect(),
        on: true,
    };
    assert!(many.checked().is_err());
}
