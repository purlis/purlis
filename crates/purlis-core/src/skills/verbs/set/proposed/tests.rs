// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A change to several skills a chat proposes (#5357): what the person is shown, when it reads as
//! changed since, and what Apply makes, on the plane a set's own tests use (`set::tests::plane`).

use super::super::tests::{commits, deleted_named, did, plane, project, there, write};
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
fn a_skills_other_file_changed_since_it_was_proposed_makes_the_proposal_read_as_changed() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Delete {
        members: vec![project("tdd")],
    };
    let then = change.shown(root);
    std::fs::create_dir_all(root.join("skills/tdd/scripts")).unwrap();
    std::fs::write(root.join("skills/tdd/scripts/run.sh"), "rm -rf ~\n").unwrap();
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

/// The plane with `tdd` and `review` deleted as one set.
fn with_two_deleted() -> tempfile::TempDir {
    let tmp = plane();
    did(tmp.path(), |cx| {
        super::super::delete_set(cx, &[project("tdd"), project("review")], None, None)
    })
    .unwrap();
    tmp
}

#[test]
fn a_proposed_restore_shows_each_deleted_skill_and_brings_them_back_as_one_commit() {
    let tmp = with_two_deleted();
    let root = tmp.path();
    let change = SetChange::Restore {
        members: vec![project("tdd"), project("review")],
    };
    assert_eq!(change.title(), "Bring back 2 skills");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("tdd · project: deleted"),
        "{}",
        shown.before
    );
    assert!(
        shown.after.contains("tdd · project: back"),
        "{}",
        shown.after
    );
    assert!(!there(root, "skills", "tdd"), "shown, nothing written");
    let before = commits(root);
    did(root, |cx| change.apply(cx)).unwrap();
    assert!(there(root, "skills", "tdd") && there(root, "skills", "review"));
    assert_eq!(commits(root), before + 1);
}

#[test]
fn a_skill_not_in_deleted_stays_out_of_a_proposed_restore() {
    let tmp = with_two_deleted();
    let root = tmp.path();
    let change = SetChange::Restore {
        members: vec![project("tdd"), project("grilling")],
    };
    let shown = change.shown(root);
    assert!(
        shown
            .after
            .contains("grilling · project: stays (It is not in Deleted.)"),
        "{}",
        shown.after
    );
    did(root, |cx| change.apply(cx)).unwrap();
    assert!(there(root, "skills", "tdd"));
    assert!(!there(root, "skills", "review"), "only what was named");
}

#[test]
fn a_proposed_delete_now_says_it_has_no_undo_and_takes_them_off_deleted() {
    let tmp = with_two_deleted();
    let root = tmp.path();
    let change = SetChange::DeleteNow {
        members: vec![project("tdd")],
    };
    assert_eq!(change.title(), "Delete 1 skill now, with no undo");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("tdd · project: deleted"),
        "{}",
        shown.before
    );
    assert!(
        shown.after.contains("tdd · project: gone for good"),
        "{}",
        shown.after
    );
    assert_eq!(deleted_named(root), ["review", "tdd"]);
    did(root, |cx| change.apply(cx)).unwrap();
    assert_eq!(deleted_named(root), ["review"]);
}

#[test]
fn a_proposed_move_shows_where_each_goes_and_moves_them_as_one_commit() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Move {
        members: vec![project("tdd"), project("review")],
        to: Level::workspace("ops"),
        grant: Vec::new(),
    };
    assert_eq!(change.title(), "Move 2 skills to the workspace ops skills");
    let shown = change.shown(root);
    assert!(
        shown.before.contains("tdd · project: there"),
        "{}",
        shown.before
    );
    assert!(
        shown
            .after
            .contains("tdd · project: moved to the workspace ops skills"),
        "{}",
        shown.after
    );
    assert!(there(root, "skills", "tdd"), "shown, nothing moved");
    let before = commits(root);
    did(root, |cx| change.apply(cx)).unwrap();
    assert!(!there(root, "skills", "tdd") && !there(root, "skills", "review"));
    assert!(there(root, "workspaces/ops/skills", "tdd"));
    assert_eq!(commits(root), before + 1);
}

#[test]
fn a_proposed_move_into_the_projects_skills_with_no_grant_moves_nothing() {
    let tmp = plane();
    let root = tmp.path();
    write(
        root,
        "workspaces/ops/skills",
        "lint",
        &super::super::tests::skill_text("lint"),
    );
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "lint"]);
    let lint = Member {
        level: Level::workspace("ops"),
        name: "lint".to_owned(),
    };
    let change = SetChange::Move {
        members: vec![lint.clone()],
        to: Level::project(),
        grant: Vec::new(),
    };
    assert!(
        change
            .shown(root)
            .after
            .contains("lint · workspace ops: stays")
    );
    assert!(did(root, |cx| change.apply(cx)).is_err());
    assert!(there(root, "workspaces/ops/skills", "lint"));
    let granted = SetChange::Move {
        members: vec![lint],
        to: Level::project(),
        grant: vec!["all".to_owned()],
    };
    did(root, |cx| granted.apply(cx)).unwrap();
    assert!(there(root, "skills", "lint"));
}

/// **A proposed export carries no place** (#5492, the person's choice of 2026-11-06): it names the
/// skills and folder or zip, and is written only where the person picks at Apply.
#[test]
fn a_proposed_export_names_no_place_and_writes_only_where_it_is_given_one() {
    let tmp = plane();
    let root = tmp.path();
    let change = SetChange::Export {
        members: vec![project("tdd"), project("review")],
        zip: false,
    };
    assert_eq!(change.title(), "Export 2 skills to a folder");
    let travels = serde_json::to_value(&change).unwrap();
    let mut keys: Vec<&String> = travels.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["act", "members", "zip"], "no place travels");
    assert!(
        change.shown(root).after.contains("tdd · project: exported"),
        "{}",
        change.shown(root).after
    );
    assert!(
        did(root, |cx| change.apply(cx)).is_err(),
        "nothing is written with no place picked"
    );
    let out = tempfile::tempdir().unwrap();
    let to = out.path().join("skills");
    change.export_to(root, None, &to).unwrap();
    assert!(to.join("tdd/SKILL.md").exists() && to.join("review/SKILL.md").exists());
    assert!(!to.join("grilling").exists(), "only what was named");
}
