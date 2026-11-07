// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The briefing's unshared-memory line runs no `git status` (#5503): it reads what the host's
//! [`Keeper`] last recorded in the project index of the project's git standing. The keeper
//! records it when it starts, after each batch of changes the watch tells it, and when a save
//! has moved the standing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use purlis_core::projectindex::{Keeper, Reader};

const WITHIN: Duration = Duration::from_secs(5);

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
    std::fs::create_dir_all(root.join("personas/ops/memory")).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    std::fs::write(
        root.join("personas/ops/persona.md"),
        "---\nrole: Ops\n---\n",
    )
    .unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "start"]);
    (dir, root)
}

fn unshared(root: &Path) -> Option<(usize, usize)> {
    Reader::open(root).expect("an index").unshared()
}

#[test]
fn the_keeper_records_what_memory_git_has_not_committed_and_records_it_again_after_a_save() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    std::fs::write(root.join("workspaces/alpha/memory/a.md"), "# A\n").unwrap();
    let keeper = Keeper::start(root.clone(), Arc::new(|_: &str| {}));
    assert!(keeper.settle(WITHIN));
    assert_eq!(unshared(&root), Some((0, 1)), "recorded at start");

    let written = root.join("personas/ops/memory/b.md");
    std::fs::write(&written, "# B\n").unwrap();
    // As the watch tells it: the shared standing is touched first, then the keeper hears it.
    purlis_core::planegit::touch(&root);
    keeper.touched(Some(vec![written]));
    assert!(keeper.settle(WITHIN));
    assert_eq!(
        unshared(&root),
        Some((1, 1)),
        "recorded after the watch's batch"
    );

    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "saved"]);
    keeper.git_moved();
    assert!(keeper.settle(WITHIN));
    assert_eq!(unshared(&root), Some((0, 0)), "recorded after the save");
}
