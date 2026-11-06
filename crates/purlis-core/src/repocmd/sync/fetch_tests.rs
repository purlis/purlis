// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

/// A project folder, a repository standing in for the remote, and a clone of it whose
/// remote-tracking branch has a reflog.
fn a_clone() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (root, upstream, clone) = (base.join("plane"), base.join("up"), base.join("clone"));
    for folder in [&root, &upstream] {
        std::fs::create_dir_all(folder).unwrap();
    }
    // In the repository's own config: CI runners have no git identity.
    for args in [
        &["init", "-q", "-b", "main", "."][..],
        &["config", "user.name", "Fixture"],
        &["config", "user.email", "fixture@example.invalid"],
        &["commit", "-q", "--allow-empty", "-m", "one"],
    ] {
        let ran = crate::testgit::run(&upstream, args);
        assert!(ran.ok(), "{args:?}: {}", ran.err);
    }
    let from = upstream.display().to_string();
    let to = clone.display().to_string();
    for (dir, args) in [
        (&base, &["clone", "-q", from.as_str(), to.as_str()][..]),
        (&upstream, &["commit", "-q", "--allow-empty", "-m", "two"]),
    ] {
        let ran = crate::testgit::run(dir, args);
        assert!(ran.ok(), "{args:?}: {}", ran.err);
    }
    (dir, root, upstream, clone)
}

#[cfg(unix)]
#[test]
fn a_fetch_never_writes_through_a_reflog_that_links_elsewhere() {
    let (dir, root, _upstream, clone) = a_clone();
    let fixed = dir.path().join("fixed");
    std::fs::write(&fixed, "fixed\n").unwrap();
    let reflog = clone.join(".git/logs/refs/remotes/origin/main");
    let _ = std::fs::remove_file(&reflog);
    std::fs::create_dir_all(reflog.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&fixed, &reflog).unwrap();

    let refused = fetch_remote(&root, &clone, "origin", None).unwrap_err();

    assert!(refused.contains("is a link"), "{refused}");
    assert_eq!(std::fs::read_to_string(&fixed).unwrap(), "fixed\n");
}

#[test]
fn a_fetch_with_plain_reflogs_moves_the_remote_tracking_branch() {
    let (_dir, root, upstream, clone) = a_clone();
    fetch_remote(&root, &clone, "origin", None).expect("fetched");
    let theirs = crate::testgit::run(&upstream, &["rev-parse", "--verify", "HEAD"]);
    let tracking = crate::testgit::run(
        &clone,
        &["rev-parse", "--verify", "refs/remotes/origin/main"],
    );
    assert!(
        theirs.ok() && tracking.ok(),
        "{} {}",
        theirs.err,
        tracking.err
    );
    assert_eq!(tracking.out.trim(), theirs.out.trim());
}
