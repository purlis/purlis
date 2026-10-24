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

#[test]
fn a_named_file_is_staged_from_the_descriptor_that_was_checked() {
    let (_dir, root) = staged();
    std::fs::write(root.join("c.md"), "checked\n").unwrap();
    let OnDisk::File(file, runs) = opened(&root, "c.md") else {
        panic!("c.md is a regular file of one name");
    };
    // Another file takes the name once the descriptor is open.
    std::fs::write(root.join("c.next"), "swapped in\n").unwrap();
    std::fs::rename(root.join("c.next"), root.join("c.md")).unwrap();

    stage_opened(&root, vec![("c.md".to_owned(), file, runs)]).unwrap();
    run_ok(&root, &["update-index", "-q", "--refresh"]).unwrap();

    assert_eq!(run(&root, &["show", ":c.md"]), "checked\n");
    // What is on disk now is not what was staged, and the save refuses it.
    assert!(whys(&root).contains(&("c.md".to_owned(), Why::Changed)));
}

#[cfg(unix)]
#[test]
fn a_save_of_named_paths_stages_what_git_add_would_and_holds_a_second_name_back() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, root) = staged();
    run(&root, &["commit", "-q", "-m", "two"]);
    let dir = root.join("d");
    std::fs::create_dir_all(dir.join("local")).unwrap();
    std::fs::write(dir.join("new.md"), "new\n").unwrap();
    std::fs::write(dir.join("run.sh"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink("new.md", dir.join("link")).unwrap();
    std::fs::write(dir.join("local/kept.md"), "kept\n").unwrap();
    std::fs::write(root.join("outside"), "outside\n").unwrap();
    std::fs::hard_link(root.join("outside"), dir.join("twice.md")).unwrap();
    std::fs::rename(root.join("a.md"), dir.join("moved.md")).unwrap();

    let held = stage_named(&root, &[":(literal)d", ":(literal)a.md"], |path| {
        path.starts_with("d/local")
    })
    .unwrap();

    assert_eq!(
        held,
        [Held {
            path: "d/twice.md".to_owned(),
            why: Why::SecondName
        }]
    );
    let rows = run(&root, &["ls-files", "-s", "--", "d", "a.md"]);
    let rows: Vec<(&str, &str)> = rows
        .lines()
        .filter_map(|row| {
            let (meta, path) = row.split_once('\t')?;
            Some((meta.split(' ').next()?, path))
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("120000", "d/link"),
            ("100644", "d/moved.md"),
            ("100644", "d/new.md"),
            ("100755", "d/run.sh"),
        ]
    );
    assert_eq!(run(&root, &["show", ":d/moved.md"]), "staged\n");
    assert_eq!(whys(&root), []);
}

#[test]
fn only_a_loose_object_nothing_reaches_is_taken_out_of_the_store() {
    let (_dir, root) = staged();
    let committed = run(&root, &["rev-parse", "HEAD:README.md"]);
    let in_index = run(&root, &["rev-parse", ":a.md"]);
    std::fs::write(root.join("loose"), "nothing reaches this\n").unwrap();
    let unreached = run(&root, &["hash-object", "-w", "loose"]);
    let [committed, in_index, unreached] =
        [committed, in_index, unreached].map(|sha| sha.trim().to_owned());

    forget(
        &root,
        &[committed.clone(), in_index.clone(), unreached.clone()],
    );

    let there = |sha: &str| crate::testgit::run(&root, &["cat-file", "-e", sha]).ok();
    assert!(there(&committed), "a committed blob was removed");
    assert!(there(&in_index), "a staged blob was removed");
    assert!(!there(&unreached), "an unreached loose blob is still there");
}
