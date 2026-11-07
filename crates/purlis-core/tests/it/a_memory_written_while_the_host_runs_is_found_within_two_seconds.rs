// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! While the host runs, its [`Keeper`] keeps the project index up to date (ADR 0079 §4, #5498): a
//! memory an agent writes, one edited by hand and one a pull brings are each found by recall
//! within two seconds, answered from the index and not by a scan. The watcher tells the keeper
//! the paths that moved; a pull is told as the commit having moved, and the keeper asks git
//! which paths the range changed.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::projectindex::Keeper;
use purlis_core::recall::{self, Ask, Workspaces};

const WITHIN: Duration = Duration::from_secs(2);

fn git(root: &Path, args: &[&str]) {
    let done = purlis_core::forklock::status(
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(root),
    )
    .unwrap();
    assert!(done.success(), "git {args:?}");
}

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha/memory")).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    write(
        &root,
        "workspaces/alpha/memory/20260301-091400-first.md",
        "First",
        "the first memory",
    );
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "start"]);
    (dir, root)
}

fn write(root: &Path, at: &str, title: &str, body: &str) -> PathBuf {
    let path = root.join(at);
    std::fs::write(
        &path,
        format!("# {title}\n\n_2026-03-02 09:14 · persistent_\n\n{body}\n"),
    )
    .unwrap();
    path
}

/// Whether recall, answered from the index, finds `word` in a memory titled `title`.
fn found(root: &Path, word: &str, title: &str) -> bool {
    let got = recall::recall(
        root,
        &Ask {
            scopes: vec!["workspace".into()],
            workspaces: Some(Workspaces::One("alpha".into())),
            persona: None,
            session: "s1".into(),
            query: Some(word.into()),
            limit: 0,
            since: None,
        },
    );
    got.index.is_some() && got.hits.iter().any(|h| h.title == title)
}

/// How long it took recall to find it, or `None` when it did not within [`WITHIN`].
fn found_within(root: &Path, word: &str, title: &str) -> Option<Duration> {
    let started = Instant::now();
    while started.elapsed() < WITHIN {
        if found(root, word, title) {
            return Some(started.elapsed());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn keeper(root: &Path) -> Keeper {
    let keeper = Keeper::start(root.to_path_buf(), Arc::new(|_: &str| {}));
    assert!(
        keeper.settle(Duration::from_secs(10)),
        "the first build ended"
    );
    assert!(
        found(root, "first", "First"),
        "the first build indexed the project"
    );
    keeper
}

#[test]
fn a_memory_an_agent_writes_is_found_within_two_seconds() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let keeper = keeper(&root);

    let path = write(
        &root,
        "workspaces/alpha/memory/20260302-091400-agent.md",
        "Agent wrote this",
        "zebrafish appear",
    );
    keeper.touched(Some(vec![path]));

    assert!(found_within(&root, "zebrafish", "Agent wrote this").is_some());
}

#[test]
fn a_memory_edited_by_hand_is_found_by_its_new_words_within_two_seconds() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let keeper = keeper(&root);

    let path = write(
        &root,
        "workspaces/alpha/memory/20260301-091400-first.md",
        "First",
        "now about narwhals",
    );
    // The watcher could not say what moved: every source is swept.
    keeper.touched(None);

    assert!(found_within(&root, "narwhals", "First").is_some());
    assert!(path.exists());
    assert!(
        !found(&root, "memory", "First"),
        "the words the edit removed no longer answer"
    );
}

#[test]
fn a_memory_a_pull_brings_is_found_within_two_seconds() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let keeper = keeper(&root);

    // Another clone's commit, arriving as a pull: the tree and the checked-out commit move
    // together, and the watcher is told nothing.
    write(
        &root,
        "workspaces/alpha/memory/20260303-091400-pulled.md",
        "Pulled in",
        "axolotls arrive",
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "a teammate's memory"]);
    keeper.pulled();

    assert!(found_within(&root, "axolotls", "Pulled in").is_some());
}

#[test]
fn a_memory_deleted_stops_answering() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let keeper = keeper(&root);

    let path = root.join("workspaces/alpha/memory/20260301-091400-first.md");
    std::fs::remove_file(&path).unwrap();
    keeper.touched(Some(vec![path]));
    assert!(keeper.settle(WITHIN));

    assert!(!found(&root, "first", "First"));
}
