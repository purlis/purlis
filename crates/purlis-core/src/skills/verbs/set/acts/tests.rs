// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Pin, archive and move as one change each (#5332), on the same plane as a set's delete and turn
//! (`set::tests::plane`), read back through its commits and its files.

use super::super::tests::{commits, did, output, plane, project, subject, there};
use super::*;
use crate::testgit::run;

fn ops() -> PinnedBy {
    PinnedBy::Workspace("ops".to_owned())
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

/// A save that signs with a signer that refuses: every change is made, and no commit lands.
fn commits_refused(root: &Path) {
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\nsign = true\n",
    )
    .unwrap();
    run(root, &["add", "charter.toml"]);
    run(root, &["commit", "-q", "-m", "signs"]);
    run(root, &["config", "gpg.program", "false"]);
}

#[test]
fn pinning_a_set_is_one_commit_and_its_undo_leaves_one_pinned_before_as_it_was() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::verbs::pin(cx, &ops(), "review", true)
    })
    .unwrap();
    let before = commits(root);
    let changed = did(root, |cx| {
        pin_set(cx, &ops(), &names(&["grilling", "tdd", "review"]), true)
    })
    .unwrap();
    assert_eq!(changed, ["grilling", "tdd"], "review was pinned already");
    assert_eq!(commits(root), before + 1, "one commit, one History entry");
    assert!(
        subject(root).contains("skill pin 2 for ops's chats"),
        "{}",
        subject(root)
    );
    let mut pins = routing::pins_written(root, &ops());
    pins.sort();
    assert_eq!(pins, ["grilling", "review", "tdd"]);
    did(root, |cx| pin_set(cx, &ops(), &changed, false)).unwrap();
    assert_eq!(routing::pins_written(root, &ops()), ["review"]);
}

#[test]
fn a_pin_set_one_of_which_names_no_skill_changes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let before = commits(root);
    let refused = did(root, |cx| {
        pin_set(cx, &ops(), &names(&["grilling", "no-such"]), true)
    })
    .unwrap_err();
    assert!(refused.contains("no-such"), "{refused}");
    assert_eq!(commits(root), before);
    assert!(routing::pins_written(root, &ops()).is_empty());
}

#[test]
fn a_pin_set_past_the_most_a_chat_reads_is_refused_whole() {
    let tmp = plane();
    let root = tmp.path();
    let mut many = Vec::new();
    for at in 0..routing::MOST_PINS {
        let name = format!("many-{at}");
        super::super::tests::write(
            root,
            "skills",
            &name,
            &super::super::tests::skill_text(&name),
        );
        many.push(name);
    }
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "many"]);
    did(root, |cx| {
        pin_set(cx, &ops(), &many[..routing::MOST_PINS - 1], true)
    })
    .unwrap();
    let refused = did(root, |cx| {
        pin_set(cx, &ops(), &names(&["grilling", "tdd"]), true)
    })
    .unwrap_err();
    assert!(refused.contains("none of these was pinned"), "{refused}");
    assert_eq!(
        routing::pins_written(root, &ops()).len(),
        routing::MOST_PINS - 1
    );
}

#[test]
fn a_pin_set_whose_commit_cannot_land_unpins_what_it_pinned() {
    let tmp = plane();
    let root = tmp.path();
    commits_refused(root);
    let refused = did(root, |cx| pin_set(cx, &ops(), &names(&["grilling"]), true)).unwrap_err();
    assert!(refused.contains("no skill was pinned"), "{refused}");
    assert!(routing::pins_written(root, &ops()).is_empty());
    assert_eq!(output(root, &["status", "--porcelain"]).trim(), "");
}

fn lifecycle_of(root: &Path, name: &str) -> Lifecycle {
    purlis_skills::about_of(&root.join("skills").join(name))
        .unwrap()
        .lifecycle
}

#[test]
fn archiving_a_set_is_one_commit_and_its_undo_sets_back_each_state_it_had() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        crate::skills::lifecycle::set(cx, &Level::project(), "tdd", &Lifecycle::Archived)
    })
    .unwrap();
    let before = commits(root);
    let changed = did(root, |cx| {
        lifecycle_set(
            cx,
            &[
                (project("grilling"), Lifecycle::Archived),
                (project("tdd"), Lifecycle::Archived),
            ],
        )
    })
    .unwrap();
    assert_eq!(
        changed,
        [(project("grilling"), Lifecycle::Active)],
        "tdd was archived already"
    );
    assert_eq!(commits(root), before + 1);
    assert!(
        subject(root).contains("skill archive 1"),
        "{}",
        subject(root)
    );
    assert_eq!(lifecycle_of(root, "grilling"), Lifecycle::Archived);
    did(root, |cx| lifecycle_set(cx, &changed)).unwrap();
    assert_eq!(lifecycle_of(root, "grilling"), Lifecycle::Active);
    assert_eq!(
        lifecycle_of(root, "tdd"),
        Lifecycle::Archived,
        "left as it was"
    );
}

#[test]
fn an_archive_set_one_of_which_is_not_there_changes_nothing() {
    let tmp = plane();
    let root = tmp.path();
    let before = commits(root);
    assert!(
        did(root, |cx| {
            lifecycle_set(
                cx,
                &[
                    (project("grilling"), Lifecycle::Archived),
                    (project("no-such"), Lifecycle::Archived),
                ],
            )
        })
        .is_err()
    );
    assert_eq!(commits(root), before);
    assert_eq!(lifecycle_of(root, "grilling"), Lifecycle::Active);
}

#[test]
fn an_archive_set_whose_commit_cannot_land_sets_every_state_back() {
    let tmp = plane();
    let root = tmp.path();
    commits_refused(root);
    let refused = did(root, |cx| {
        lifecycle_set(
            cx,
            &[
                (project("grilling"), Lifecycle::Archived),
                (project("tdd"), Lifecycle::Archived),
            ],
        )
    })
    .unwrap_err();
    assert!(refused.contains("nothing changed"), "{refused}");
    assert_eq!(lifecycle_of(root, "grilling"), Lifecycle::Active);
    assert_eq!(lifecycle_of(root, "tdd"), Lifecycle::Active);
    assert_eq!(output(root, &["status", "--porcelain"]).trim(), "");
}

fn to_ops(name: &str) -> Moving {
    Moving {
        member: project(name),
        to: Level::workspace("ops"),
        grant: None,
    }
}

#[test]
fn moving_a_set_is_one_commit_and_its_undo_brings_each_back_with_its_grant() {
    let tmp = plane();
    let root = tmp.path();
    crate::skills::grant(root, "tdd", &Grant::All).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "granted"]);
    let before = commits(root);
    let back = did(root, |cx| {
        move_set(cx, &[to_ops("grilling"), to_ops("tdd")])
    })
    .unwrap();
    assert_eq!(commits(root), before + 1, "one commit, one History entry");
    assert!(subject(root).contains("skill mv 2"), "{}", subject(root));
    assert!(there(root, "workspaces/ops/skills", "grilling"));
    assert!(there(root, "workspaces/ops/skills", "tdd"));
    assert!(!there(root, "skills", "grilling") && !there(root, "skills", "tdd"));
    let status = output(root, &["status", "--porcelain"]);
    assert_eq!(status.trim(), "", "all of it is in the commit");
    assert_eq!(back.len(), 2);
    assert_eq!(back[1].grant, Some(Grant::All), "tdd had every workspace");
    did(root, |cx| move_set(cx, &back)).unwrap();
    assert!(there(root, "skills", "grilling") && there(root, "skills", "tdd"));
    assert_eq!(grant_of(&root.join("skills/tdd")).unwrap(), Grant::All);
}

#[test]
fn a_move_set_one_of_which_cannot_go_moves_nothing() {
    let tmp = plane();
    let root = tmp.path();
    super::super::tests::write(
        root,
        "workspaces/ops/skills",
        "tdd",
        &super::super::tests::skill_text("tdd"),
    );
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "ops has its own"]);
    let before = commits(root);
    let refused = did(root, |cx| {
        move_set(cx, &[to_ops("grilling"), to_ops("tdd")])
    })
    .unwrap_err();
    assert!(
        refused.contains("tdd cannot move") && refused.contains("nothing was moved"),
        "{refused}"
    );
    assert_eq!(commits(root), before);
    assert!(there(root, "skills", "grilling"), "nothing was written");
}

#[test]
fn a_move_set_whose_commit_cannot_land_puts_every_skill_back() {
    let tmp = plane();
    let root = tmp.path();
    commits_refused(root);
    let refused = did(root, |cx| {
        move_set(cx, &[to_ops("grilling"), to_ops("tdd")])
    })
    .unwrap_err();
    assert!(refused.contains("nothing was moved"), "{refused}");
    assert!(there(root, "skills", "grilling") && there(root, "skills", "tdd"));
    assert!(!there(root, "workspaces/ops/skills", "grilling"));
    let status = output(root, &["status", "--porcelain"]);
    assert_eq!(status.trim(), "", "{status}");
}
