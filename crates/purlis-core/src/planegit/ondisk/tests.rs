// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a save holds back, asked of a repository whose index and disk were made to disagree
//! after the `git add`, which a save's own add does not undo.

use std::path::{Path, PathBuf};

use super::*;

fn run(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out
}

/// A repository with one commit, and `a.md` and `b.md` staged after it.
fn staged() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    run(&root, &["init", "-q", "-b", "main", "."]);
    run(&root, &["config", "user.name", "Fixture"]);
    run(&root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join("README.md"), "one\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "one"]);
    std::fs::write(root.join("a.md"), "staged\n").unwrap();
    std::fs::write(root.join("b.md"), "staged\n").unwrap();
    run(&root, &["add", "a.md", "b.md"]);
    (dir, root)
}

fn whys(root: &Path) -> Vec<(String, Why)> {
    not_as_staged(root)
        .unwrap()
        .into_iter()
        .map(|held| (held.path, held.why))
        .collect()
}

#[test]
fn files_staged_as_they_are_on_disk_are_not_held_back() {
    let (_dir, root) = staged();
    run(&root, &["rm", "-q", "README.md"]);

    assert_eq!(whys(&root), []);
    assert_eq!(refused(&root, &not_as_staged(&root)), None);
}

#[test]
fn a_file_swapped_or_removed_after_it_was_staged_is_held_back_and_unstaged() {
    let (_dir, root) = staged();
    // The same bytes under a new file: no name or time of the one staged.
    std::fs::rename(root.join("a.md"), root.join("aside")).unwrap();
    std::fs::write(root.join("a.md"), "staged\n").unwrap();
    std::fs::remove_file(root.join("b.md")).unwrap();

    let found = not_as_staged(&root);
    assert_eq!(
        whys(&root),
        [
            ("a.md".to_owned(), Why::Changed),
            ("b.md".to_owned(), Why::Gone)
        ]
    );
    let refusal = refused(&root, &found).expect("a refusal");
    assert_eq!(
        refusal.named,
        [
            "a.md changed on disk after it was staged, so purlis does not save it",
            "b.md is no longer on disk as it was staged, so purlis does not save it",
        ]
    );
    assert!(refusal.then.contains("no longer staged"), "{refusal:?}");
    assert_eq!(run(&root, &["diff", "--cached", "--name-only"]), "");
    assert_eq!(
        std::fs::read_to_string(root.join("a.md")).unwrap(),
        "staged\n"
    );
}

#[test]
fn a_file_given_a_second_name_after_it_was_staged_is_held_back() {
    let (_dir, root) = staged();
    std::fs::hard_link(root.join("a.md"), root.join("a.again")).unwrap();

    assert_eq!(whys(&root), [("a.md".to_owned(), Why::SecondName)]);
}
