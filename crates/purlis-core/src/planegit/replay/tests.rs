// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Replaying off to the side, against real git: two clones of one plane.

use super::*;
use crate::planegit::tests::twoclones::{Two, commit, run, write};

const LONG: Duration = Duration::from_secs(60);

#[test]
fn a_line_of_commits_is_replayed_onto_the_remotes_with_nothing_at_the_root_moved() {
    let two = Two::new(&[("shared.md", "one\ntwo\nthree\n")]);
    write(&two.mine, "mine-1.md", "one of mine\n");
    commit(&two.mine, "mine 1");
    write(&two.mine, "shared.md", "one, mine\ntwo\nthree\n");
    commit(&two.mine, "mine 2");
    write(&two.theirs, "shared.md", "one\ntwo\nthree, theirs\n");
    two.land_theirs("theirs");
    let before = two.head();
    write(&two.mine, "unsaved.md", "a chat's words\n");

    let replayed = replay(
        &two.mine,
        "HEAD",
        "refs/remotes/origin/release",
        false,
        LONG,
    );

    let Replayed::Onto(tip) = replayed else {
        panic!("{replayed:?}");
    };
    // Nothing at the root moved: HEAD, the index and the work tree are as they were.
    assert_eq!(two.head(), before);
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "?? unsaved.md");
    assert!(!two.mine.join(".git/rebase-merge").exists());
    // The line is on the remote's, in order, each with its author and message.
    assert_eq!(
        run(&two.mine, &["log", "--format=%s|%an", "-3", &tip]),
        "mine 2|Mine\nmine 1|Mine\ntheirs|Theirs"
    );
    assert_eq!(
        run(&two.mine, &["show", &format!("{tip}:shared.md")]),
        "one, mine\ntwo\nthree, theirs"
    );
    assert_eq!(
        run(
            &two.mine,
            &["log", "-1", "--format=%ad", "--date=raw", &tip]
        ),
        run(
            &two.mine,
            &["log", "-1", "--format=%ad", "--date=raw", &before]
        )
    );
}

#[test]
fn a_commit_the_remote_already_has_or_that_comes_to_nothing_is_dropped() {
    let two = Two::new(&[("shared.md", "one\n")]);
    write(&two.mine, "same.md", "both wrote this\n");
    commit(&two.mine, "same on both");
    write(&two.mine, "mine.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "same.md", "both wrote this\n");
    two.land_theirs("same on both");

    let Replayed::Onto(tip) = replay(
        &two.mine,
        "HEAD",
        "refs/remotes/origin/release",
        false,
        LONG,
    ) else {
        panic!("not replayed");
    };

    assert_eq!(
        run(&two.mine, &["log", "--format=%s", "-2", &tip]),
        "mine\nsame on both"
    );
    assert_eq!(
        run(&two.mine, &["rev-parse", &format!("{tip}^")]),
        run(&two.mine, &["rev-parse", "refs/remotes/origin/release"])
    );
}

#[test]
fn a_conflict_stops_the_replay_naming_the_files_and_nothing_moves() {
    let two = Two::new(&[("shared.md", "one\n")]);
    write(&two.mine, "shared.md", "mine\n");
    commit(&two.mine, "mine");
    write(&two.theirs, "shared.md", "theirs\n");
    two.land_theirs("theirs");
    let before = two.head();

    let replayed = replay(
        &two.mine,
        "HEAD",
        "refs/remotes/origin/release",
        false,
        LONG,
    );

    assert_eq!(replayed, Replayed::Conflict(vec!["shared.md".to_owned()]));
    assert_eq!(two.head(), before);
    assert_eq!(run(&two.mine, &["status", "--porcelain"]), "");
    assert!(crate::gitstate::stopped(&two.mine).is_none());
}

#[test]
fn nothing_of_its_own_replays_onto_the_remotes_own_commit() {
    let two = Two::new(&[("shared.md", "one\n")]);
    write(&two.theirs, "theirs.md", "theirs\n");
    two.land_theirs("theirs");

    let replayed = replay(
        &two.mine,
        "HEAD",
        "refs/remotes/origin/release",
        false,
        LONG,
    );

    assert_eq!(
        replayed,
        Replayed::Onto(run(
            &two.mine,
            &["rev-parse", "refs/remotes/origin/release"]
        ))
    );
}
