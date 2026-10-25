// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A fix's commit of its own paths, with real git (CI: the sandbox refuses `git init`).

use std::path::{Path, PathBuf};

use super::*;

fn run(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out
}

/// A project with one commit holding `.claude/skills/s/SKILL.md`.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().join("project");
    std::fs::create_dir_all(root.join(".claude/skills/s")).unwrap();
    run(&root, &["init", "-q", "-b", "main", "."]);
    run(&root, &["config", "user.name", "Fixture"]);
    run(&root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join(".claude/skills/s/SKILL.md"), "# s\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "one"]);
    (dir, root)
}

/// **A folder a fix moves can hold a file the fix did not write**: one with a second name
/// refuses the commit before anything is staged, and its bytes reach no commit and no object
/// store.
#[test]
fn a_moved_folder_holding_a_file_with_a_second_name_is_not_committed() {
    let (dir, root) = a_project();
    let outside = dir.path().join("outside.md");
    std::fs::write(&outside, "from outside: canary-91c0d4\n").unwrap();
    std::fs::hard_link(&outside, root.join(".claude/skills/s/notes.md")).unwrap();
    let sha = run(&root, &["hash-object", outside.to_str().unwrap()]);
    let before = run(&root, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(root.join("skills")).unwrap();
    run(&root, &["mv", ".claude/skills/s", "skills/s"]);

    let said = commit_paths(&root, &[".claude/skills/s", "skills/s"], "move").unwrap_err();

    let named = "skills/s/notes.md has a second name (a hard link)";
    assert!(said.contains(named), "{said}");
    assert!(said.contains("Nothing was staged or committed."), "{said}");
    assert_eq!(run(&root, &["rev-parse", "HEAD"]), before, "{said}");
    assert!(
        !crate::testgit::run(&root, &["cat-file", "-e", sha.trim()]).ok(),
        "the object store holds the file: {said}"
    );
}

/// A fix's own paths, each one name on disk, are committed.
#[test]
fn a_fixs_own_paths_are_committed() {
    let (_dir, root) = a_project();
    std::fs::create_dir_all(root.join("skills")).unwrap();
    run(&root, &["mv", ".claude/skills/s", "skills/s"]);
    std::fs::write(root.join("skills/s/notes.md"), "new\n").unwrap();

    let sha = commit_paths(&root, &[".claude/skills/s", "skills/s"], "move").unwrap();

    assert!(sha.is_some());
    let files = run(&root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    let files: Vec<&str> = files.lines().collect();
    assert_eq!(files, ["skills/s/SKILL.md", "skills/s/notes.md"]);
}
