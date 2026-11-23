// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A push is scanned the same where the clone's object directory cannot be written.**
//!
//! A task chat's sandbox may write its branch folder and not the shared clone's `.git/objects`.
//! `git log -p --diff-merges=remerge` makes a temporary object directory there even when no merge
//! is in range, so the scan failed and every push was refused ("unable to create temporary object
//! directory"). The scan now gives git a scratch object directory that borrows the real one.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::support::{git, git_tried};

fn write(root: &Path, file: &str, text: &str) {
    std::fs::write(root.join(file), text).unwrap();
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

fn head(root: &Path) -> String {
    let out = git(root, &["rev-parse", "HEAD"]);
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn a_token() -> String {
    format!("ghp_{}", "aB3dE5gH7".repeat(4))
}

/// A repository with a plain commit, and a merge whose conflict resolution holds a token that only
/// the merge's remerge diff shows. The push sends everything past the first commit.
fn with_a_merge(root: &Path) -> (String, String) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.test"]);
    git(root, &["config", "user.name", "T"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    write(root, "f.txt", "one\n");
    commit(root, "base");
    let base = head(root);
    git(root, &["checkout", "-q", "-b", "side"]);
    write(root, "f.txt", "side\n");
    commit(root, "side");
    git(root, &["checkout", "-q", "main"]);
    write(root, "f.txt", "main\n");
    commit(root, "main");
    // Conflicts, left for us to resolve.
    git_tried(root, &["merge", "-q", "--no-commit", "side"]);
    write(root, "f.txt", &format!("TOKEN={}\n", a_token()));
    // The commit hook would refuse the token; the push scan is what is under test.
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "--no-verify", "-m", "merge"]);
    (base, head(root))
}

fn updates(base: &str, tip: &str) -> String {
    let _ = base;
    format!("refs/heads/main {tip} refs/heads/main {}\n", "0".repeat(40))
}

fn scan(root: &Path, tip: &str) -> Result<Vec<String>, String> {
    purlis_core::diffscan::pushed_without_asking(root, &updates("", tip))
        .map(|scan| scan.refused.iter().map(|f| format!("{f:?}")).collect())
}

#[test]
fn a_push_is_scanned_the_same_with_the_clones_objects_read_only() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let (_, tip) = with_a_merge(dir.path());

    let writable = scan(dir.path(), &tip).expect("scanned with writable objects");

    let objects = dir.path().join(".git/objects");
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o555)).unwrap();
    let read_only = scan(dir.path(), &tip);
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(read_only.expect("scanned with read-only objects"), writable);
}

#[test]
fn a_push_containing_a_merge_is_still_read_as_its_remerge_diff() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let (_, tip) = with_a_merge(dir.path());
    let objects = dir.path().join(".git/objects");
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o555)).unwrap();
    let read = scan(dir.path(), &tip);
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o755)).unwrap();

    let found = read.expect("scanned");
    assert!(
        !found.is_empty(),
        "the token a merge's resolution added was not read: {found:?}"
    );
}

#[test]
fn a_push_from_a_repository_whose_path_holds_a_colon_is_still_scanned() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("a:b");
    std::fs::create_dir(&dir).unwrap();
    let (_, tip) = with_a_merge(&dir);
    let objects = dir.join(".git/objects");
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o555)).unwrap();
    let read = scan(&dir, &tip);
    std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(0o755)).unwrap();

    let found = read.expect("scanned from a path with a colon");
    assert!(
        !found.is_empty(),
        "the merge's token was not read: {found:?}"
    );
}
