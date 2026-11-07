// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Delete now (#5332): a committed delete leaves this machine's list and stays in history; one
//! this machine kept is removed from it.

use super::super::tests::{deleted_named, did, plane, project};
use super::super::{Level, Member, delete_set, restore_set};
use super::*;

#[test]
fn delete_now_takes_a_committed_delete_off_the_list_and_history_still_holds_it() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        delete_set(cx, &[project("grilling"), project("tdd")], None, None)
    })
    .unwrap();
    let listed = deleted(root);
    let grilling: Vec<Deleted> = listed
        .iter()
        .filter(|one| one.name == "grilling")
        .cloned()
        .collect();
    let gone = delete_now(root, &grilling).unwrap();
    assert_eq!(gone.off_the_list, 1);
    assert_eq!(gone.removed, 0);
    assert!(gone.failed.is_empty(), "{:?}", gone.failed);
    assert_eq!(deleted_named(root), ["tdd"], "only grilling left the list");
    let log = super::super::tests::output(
        root,
        &["log", "--diff-filter=D", "--name-only", "--format="],
    );
    assert!(
        log.contains("skills/grilling/SKILL.md"),
        "history keeps it: {log}"
    );
}

#[test]
fn delete_now_removes_a_skill_this_machine_kept_for_good() {
    let tmp = plane();
    let root = tmp.path();
    let local = Member {
        level: Level::project().local(true),
        name: "grilling".to_owned(),
    };
    did(root, |cx| delete_set(cx, &[local], None, None)).unwrap();
    let kept = deleted(root);
    assert_eq!(kept.len(), 1);
    let gone = delete_now(root, &kept).unwrap();
    assert_eq!((gone.off_the_list, gone.removed), (0, 1));
    assert!(deleted(root).is_empty());
    let state = crate::plane::state_dir(root).join(super::super::super::DELETED_DIR);
    let left = std::fs::read_dir(&state)
        .map(|dir| dir.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "nothing of it is left on this machine");
    assert!(
        did(root, |cx| restore_set(cx, &kept, None)).is_err(),
        "it cannot come back"
    );
}

#[test]
fn delete_now_of_one_no_longer_listed_takes_nothing_off() {
    let tmp = plane();
    let root = tmp.path();
    did(root, |cx| {
        delete_set(cx, &[project("grilling"), project("tdd")], None, None)
    })
    .unwrap();
    let listed = deleted(root);
    let tdd: Vec<Deleted> = listed
        .iter()
        .filter(|one| one.name == "tdd")
        .cloned()
        .collect();
    did(root, |cx| restore_set(cx, &tdd, None)).unwrap();
    let refused = delete_now(root, &listed).unwrap_err();
    assert!(
        refused.contains("tdd") && refused.contains("nothing was deleted"),
        "{refused}"
    );
    assert_eq!(
        deleted_named(root),
        ["grilling"],
        "grilling is still listed"
    );
}
