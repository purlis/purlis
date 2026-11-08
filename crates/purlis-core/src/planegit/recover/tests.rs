// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Recovering a root left part-way, against real git: two clones of one plane, a rebase cut off
//! between two picks, and what a chat wrote after it.

use std::path::Path;

use super::*;
use crate::planegit::tests::twoclones::{Two, commit, read, run, write};

const SHARED: &str = "one\ntwo\nthree\nfour\nfive\n";

/// A plane with four commits of its own since the seed, and a commit of the remote's it has
/// fetched and not brought in.
fn ahead_and_behind() -> (Two, String) {
    let two = Two::new(&[
        ("shared.md", SHARED),
        ("old.md", "old\n"),
        ("gone-later.md", "a chat deletes this\n"),
    ]);
    let base = two.head();
    write(&two.mine, "mine-1.md", "one of mine\n");
    commit(&two.mine, "mine 1");
    write(
        &two.mine,
        "shared.md",
        &SHARED.replace("one\n", "one, mine\n"),
    );
    commit(&two.mine, "mine 2");
    write(&two.mine, "mine-3.md", "three of mine\n");
    std::fs::remove_file(two.mine.join("old.md")).unwrap();
    commit(&two.mine, "mine 3");
    write(&two.mine, "mine-4.md", "four of mine\n");
    commit(&two.mine, "mine 4");
    write(&two.theirs, "theirs.md", "the remote's\n");
    write(
        &two.theirs,
        "shared.md",
        &SHARED.replace("five\n", "five, theirs\n"),
    );
    two.land_theirs("theirs");
    (two, base)
}

/// `git rebase --onto origin/release <base>` stopped after its second pick, as a quit cuts one
/// off: the rebase's folder, a detached HEAD, and nothing to say it stopped for a person.
fn rebase_cut_off_after_two_picks(root: &Path, base: &str) {
    let out = crate::forklock::output(
        std::process::Command::new("git")
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "sequence.editor=perl -pi -e 'print qq(break\\n) if $. == 3'",
                "rebase",
                "-q",
                "-i",
                "--onto",
                "refs/remotes/origin/release",
                base,
            ]),
    )
    .unwrap();
    assert!(
        root.join(".git/rebase-merge").is_dir(),
        "the rebase stopped part-way: {out:?}"
    );
}

/// What a chat wrote after the stop: a file of its own, a line of a file, and a file deleted.
fn a_chat_writes(root: &Path) {
    write(root, "chat.md", "a chat's words\n");
    let shared = read(root, "shared.md").replace("three\n", "three, a chat\n");
    write(root, "shared.md", &shared);
    std::fs::remove_file(root.join("gone-later.md")).unwrap();
}

fn tracked(root: &Path, file: &str) -> Option<String> {
    let done = crate::testgit::run(root, &["show", &format!("HEAD:{file}")]);
    done.ok().then_some(done.out)
}

#[test]
fn a_rebase_killed_part_way_with_chats_writing_since_is_recovered_with_nothing_lost() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    let remote = run(root, &["rev-parse", "refs/remotes/origin/release"]);
    // purlis marks the step before it starts it, and the quit kills it before the marker goes.
    let marked = mark(root, Op::Rebase, &remote, &base).expect("the marker");
    rebase_cut_off_after_two_picks(root, &base);
    std::mem::forget(marked);
    a_chat_writes(root);

    let got = recover(root, false);

    let Recovered::Recovered { commit, .. } = got else {
        panic!("not recovered: {got:?}");
    };
    let git_dir = root.join(".git");
    for left in ["rebase-merge", "rebase-apply", "MERGE_HEAD", MARKER] {
        assert!(!git_dir.join(left).exists(), "{left} is left behind");
    }
    assert_eq!(run(root, &["symbolic-ref", "HEAD"]), "refs/heads/release");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), commit);
    assert!(
        crate::testgit::run(root, &["merge-base", "--is-ancestor", &remote, "HEAD"]).ok(),
        "the remote's commits are in it"
    );
    // Every local commit's content, every chat write and the remote's, committed.
    for (file, text) in [
        ("mine-1.md", "one of mine\n"),
        ("mine-3.md", "three of mine\n"),
        ("mine-4.md", "four of mine\n"),
        ("theirs.md", "the remote's\n"),
        ("chat.md", "a chat's words\n"),
        (
            "shared.md",
            "one, mine\ntwo\nthree, a chat\nfour\nfive, theirs\n",
        ),
    ] {
        assert_eq!(tracked(root, file).as_deref(), Some(text), "{file}");
        assert_eq!(read(root, file), text, "{file} in the work tree");
    }
    // Deleted by a commit of its own the rebase never reached, and by the chat: neither back.
    for gone in ["old.md", "gone-later.md"] {
        assert_eq!(tracked(root, gone), None, "{gone} came back");
        assert!(!root.join(gone).exists(), "{gone} came back on disk");
    }
    assert_eq!(run(root, &["status", "--porcelain"]), "");
}

#[test]
fn a_rebase_nobody_marked_is_the_persons_and_nothing_changes() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    rebase_cut_off_after_two_picks(root, &base);
    a_chat_writes(root);
    let head = run(root, &["rev-parse", "HEAD"]);
    let status = run(root, &["status", "--porcelain"]);

    let got = recover(root, false);

    assert!(matches!(got, Recovered::Persons(_)), "{got:?}");
    assert!(root.join(".git/rebase-merge").is_dir());
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(run(root, &["status", "--porcelain"]), status);
}

#[test]
fn a_rebase_a_purlis_before_the_marker_left_is_recovered_once_it_has_been_still() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    rebase_cut_off_after_two_picks(root, &base);
    a_chat_writes(root);
    // Still for a minute and more, as one cut off by a quit is.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    std::fs::File::open(root.join(".git/rebase-merge"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let got = recover(root, false);

    assert!(matches!(got, Recovered::Recovered { .. }), "{got:?}");
    assert_eq!(run(root, &["symbolic-ref", "HEAD"]), "refs/heads/release");
    for file in [
        "mine-1.md",
        "mine-3.md",
        "mine-4.md",
        "theirs.md",
        "chat.md",
    ] {
        assert!(tracked(root, file).is_some(), "{file}");
    }
    assert_eq!(tracked(root, "old.md"), None);
}

#[test]
fn a_chats_write_that_conflicts_with_where_the_step_was_going_is_said_and_nothing_moves() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    let remote = run(root, &["rev-parse", "refs/remotes/origin/release"]);
    let marked = mark(root, Op::Rebase, &remote, &base).expect("the marker");
    rebase_cut_off_after_two_picks(root, &base);
    std::mem::forget(marked);
    // The chat wrote, after the stop, a file of the name a commit not yet replayed adds.
    write(root, "mine-4.md", "a chat's four\n");
    let head = run(root, &["rev-parse", "HEAD"]);

    let got = recover(root, false);

    assert_eq!(got, Recovered::Conflict(vec!["mine-4.md".to_owned()]));
    assert!(root.join(".git/rebase-merge").is_dir(), "nothing moved");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
    assert_eq!(read(root, "mine-4.md"), "a chat's four\n");
}

/// A rebase marked by purlis and cut off after two picks, as a quit leaves one.
fn marked_and_cut_off(two: &Two, base: &str) -> String {
    let root = &two.mine;
    let remote = run(root, &["rev-parse", "refs/remotes/origin/release"]);
    let marked = mark(root, Op::Rebase, &remote, base).expect("the marker");
    rebase_cut_off_after_two_picks(root, base);
    std::mem::forget(marked);
    remote
}

#[test]
fn a_commit_made_on_the_detached_head_during_the_stop_survives_recovery() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    marked_and_cut_off(&two, &base);
    // A person, or a chat, commits on the detached HEAD the stop left.
    write(root, "made-in-the-stop.md", "committed during the stop\n");
    commit(root, "made in the stop");
    a_chat_writes(root);

    let got = recover(root, false);

    assert!(matches!(got, Recovered::Recovered { .. }), "{got:?}");
    assert_eq!(
        tracked(root, "made-in-the-stop.md").as_deref(),
        Some("committed during the stop\n"),
        "the commit made during the stop is in the result"
    );
    assert_eq!(
        read(root, "made-in-the-stop.md"),
        "committed during the stop\n"
    );
    assert_eq!(
        tracked(root, "chat.md").as_deref(),
        Some("a chat's words\n")
    );
    assert_eq!(run(root, &["status", "--porcelain"]), "");
}

#[test]
fn a_branch_moved_since_the_step_began_is_not_overwritten() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    marked_and_cut_off(&two, &base);
    // Someone points the branch somewhere else while the stop sits there.
    run(root, &["update-ref", "refs/heads/release", &base]);
    let head = run(root, &["rev-parse", "HEAD"]);

    let got = recover(root, false);

    assert!(matches!(got, Recovered::Failed(_)), "{got:?}");
    assert_eq!(run(root, &["rev-parse", "refs/heads/release"]), base);
    assert!(root.join(".git/rebase-merge").is_dir(), "nothing moved");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
}

/// `rebase-merge` set as still for ten minutes, as a pre-marker stop cut off by a quit is.
fn left_still(root: &Path) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    std::fs::File::open(root.join(".git/rebase-merge"))
        .unwrap()
        .set_modified(old)
        .unwrap();
}

#[test]
fn a_persons_rebase_onto_an_older_commit_of_the_target_is_never_taken_over() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    // The person's own `git rebase -i <base>`: onto a commit of origin's target, not its tip.
    let out = crate::forklock::output(
        std::process::Command::new("git")
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "sequence.editor=perl -pi -e 'print qq(break\\n) if $. == 3'",
                "rebase",
                "-q",
                "-i",
                &base,
            ]),
    )
    .unwrap();
    assert!(root.join(".git/rebase-merge").is_dir(), "{out:?}");
    left_still(root);
    let head = run(root, &["rev-parse", "HEAD"]);

    let got = recover(root, false);

    assert!(matches!(got, Recovered::Persons(_)), "{got:?}");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
    assert!(root.join(".git/rebase-merge").is_dir());
}

#[test]
fn a_pre_marker_rebase_whose_branch_moved_since_is_the_persons() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    rebase_cut_off_after_two_picks(root, &base);
    left_still(root);
    // The branch is no longer where the rebase found it: not the stop purlis left.
    run(root, &["update-ref", "refs/heads/release", &base]);

    let got = recover(root, false);

    assert!(matches!(got, Recovered::Persons(_)), "{got:?}");
    assert!(root.join(".git/rebase-merge").is_dir());
}

/// A plane whose own commit and the remote's change one line, and a marked rebase of it that
/// stopped on that conflict, then was cut off: `stopped-sha`, an unmerged path, markers on disk.
fn marked_and_stopped_on_a_conflict() -> Two {
    let two = Two::new(&[("shared.md", SHARED)]);
    let base = two.head();
    let root = &two.mine;
    write(root, "shared.md", &SHARED.replace("one\n", "one, mine\n"));
    commit(root, "mine");
    write(
        &two.theirs,
        "shared.md",
        &SHARED.replace("one\n", "one, theirs\n"),
    );
    two.land_theirs("theirs");
    let remote = run(root, &["rev-parse", "refs/remotes/origin/release"]);
    let marked = mark(root, Op::Rebase, &remote, &base).expect("the marker");
    let stopped = crate::testgit::run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "rebase",
            "-q",
            "refs/remotes/origin/release",
        ],
    );
    assert!(
        !stopped.ok(),
        "the rebase stopped on the conflict: {stopped:?}"
    );
    std::mem::forget(marked);
    assert!(read(root, "shared.md").contains("<<<<<<<"));
    two
}

#[test]
fn a_marked_rebase_cut_off_on_a_conflict_is_refused_and_its_markers_never_committed() {
    let two = marked_and_stopped_on_a_conflict();
    let root = &two.mine;
    let head = run(root, &["rev-parse", "HEAD"]);

    let got = recover(root, false);

    assert_eq!(got, Recovered::Unsettled(vec!["shared.md".to_owned()]));
    assert!(root.join(".git/rebase-merge").is_dir(), "nothing moved");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);

    // Staged with its markers still in it, the file is no longer unmerged: still refused.
    run(root, &["add", "shared.md"]);
    let got = recover(root, false);
    assert_eq!(got, Recovered::Unsettled(vec!["shared.md".to_owned()]));
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
    let log = run(root, &["log", "--all", "-p", "--format=%H"]);
    assert!(
        !log.contains("+<<<<<<<"),
        "a commit holds the markers:\n{log}"
    );
}

/// `index.lock` in the root's git directory, last written `ago` back.
fn a_lock_left(root: &Path, ago: std::time::Duration) -> std::path::PathBuf {
    let lock = root.join(".git/index.lock");
    std::fs::write(&lock, b"").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&lock)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - ago)
        .unwrap();
    lock
}

#[test]
fn an_index_lock_a_cut_off_step_left_is_removed_once_old_and_said() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    marked_and_cut_off(&two, &base);
    a_chat_writes(root);
    let lock = a_lock_left(root, std::time::Duration::from_secs(3600));

    let got = recover(root, false);

    let Recovered::Recovered { lock_removed, .. } = got else {
        panic!("not recovered: {got:?}");
    };
    assert!(lock_removed.is_some(), "the removal is said");
    assert!(!lock.exists());
    assert_eq!(run(root, &["symbolic-ref", "HEAD"]), "refs/heads/release");
}

#[test]
fn a_young_index_lock_is_left_and_named() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    marked_and_cut_off(&two, &base);
    let lock = a_lock_left(root, std::time::Duration::from_secs(5));
    let head = run(root, &["rev-parse", "HEAD"]);

    let got = recover(root, false);

    let Recovered::Failed(why) = &got else {
        panic!("not refused: {got:?}");
    };
    assert!(why.contains("index.lock"), "the lock is named: {why}");
    assert!(lock.exists(), "a young lock is never removed");
    assert_eq!(run(root, &["rev-parse", "HEAD"]), head);
}

#[test]
fn a_recovery_refused_on_a_look_reaches_the_standing() {
    let (two, base) = ahead_and_behind();
    let root = &two.mine;
    marked_and_cut_off(&two, &base);
    write(root, "mine-4.md", "a chat's four\n");

    // The window's look recovers with nobody to tell: what it found is kept for the standing.
    let got = recover(root, false);
    assert!(matches!(got, Recovered::Conflict(_)), "{got:?}");

    let standing = crate::planegit::standing(root);
    assert_eq!(standing.stage, crate::planegit::Stage::Blocked);
    let blocked = standing.blocked.unwrap_or_default();
    assert!(blocked.contains("mine-4.md"), "{blocked}");
}
