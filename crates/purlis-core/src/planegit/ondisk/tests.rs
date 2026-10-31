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

    let row = Row {
        path: "c.md".to_owned(),
        from: Source::File {
            file,
            runs,
            checked: None,
        },
    };
    assert_eq!(stage_rows(&root, vec![row]).unwrap(), []);
    refresh(&root, false).unwrap();

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

/// A repository stopped in a merge with `both.txt` resolved and staged: the blob it is staged
/// as. `dir` holds it.
fn a_resolved_conflict(dir: &tempfile::TempDir) -> (PathBuf, String) {
    let root = dir.path().canonicalize().unwrap().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    run(&root, &["init", "-q", "-b", "main", "."]);
    run(&root, &["config", "user.name", "Fixture"]);
    run(&root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join("both.txt"), "a\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "one"]);
    run(&root, &["switch", "-q", "-c", "side"]);
    std::fs::write(root.join("both.txt"), "side\n").unwrap();
    run(&root, &["commit", "-q", "-am", "side"]);
    run(&root, &["switch", "-q", "main"]);
    std::fs::write(root.join("both.txt"), "main\n").unwrap();
    run(&root, &["commit", "-q", "-am", "main"]);
    assert!(!crate::testgit::run(&root, &["merge", "-q", "side"]).ok());
    std::fs::write(root.join("both.txt"), "resolved, then swapped\n").unwrap();
    run(&root, &["add", "both.txt"]);
    let blob = run(&root, &["rev-parse", ":both.txt"]).trim().to_owned();
    (root, blob)
}

#[test]
fn a_resolved_file_held_back_is_marked_as_conflicted_again_and_its_blob_taken_out() {
    let dir = tempfile::tempdir().unwrap();
    let (root, blob) = a_resolved_conflict(&dir);
    std::fs::rename(root.join("both.txt"), root.join("aside")).unwrap();
    std::fs::write(root.join("both.txt"), "other bytes\n").unwrap();

    let staged = vec!["both.txt".to_owned()];
    let refusal = refused_into(&root, &not_as_staged(&root), PutBack::Conflicted(&staged))
        .expect("held back");

    assert!(refusal.then.contains("conflicted again"), "{refusal:?}");
    assert!(
        !run(&root, &["ls-files", "-u", "--", "both.txt"]).is_empty(),
        "the conflict is not back"
    );
    assert!(!crate::testgit::run(&root, &["cat-file", "-e", &blob]).ok());
    assert_eq!(
        std::fs::read_to_string(root.join("both.txt")).unwrap(),
        "other bytes\n"
    );
}

#[test]
fn where_git_cannot_say_every_resolved_file_is_conflicted_again_and_its_blob_taken_out() {
    let dir = tempfile::tempdir().unwrap();
    let (root, blob) = a_resolved_conflict(&dir);

    let staged = vec!["both.txt".to_owned()];
    let found = Err("git said nothing".to_owned());
    let refusal = refused_into(&root, &found, PutBack::Conflicted(&staged)).expect("refused");

    assert!(refusal.named[0].contains("git said nothing"), "{refusal:?}");
    assert!(refusal.then.contains("conflicted again"), "{refusal:?}");
    assert!(
        !run(&root, &["ls-files", "-u", "--", "both.txt"]).is_empty(),
        "the conflict is not back"
    );
    assert!(!crate::testgit::run(&root, &["cat-file", "-e", &blob]).ok());
}

#[test]
fn only_a_row_git_does_not_look_at_waits_for_the_secret_guard() {
    let row = |path: &str, why| Held {
        path: path.to_owned(),
        why,
    };
    let (first, after) = split_for_guard(Ok(vec![
        row("a", Why::SecondName),
        row("b", Why::NotLooked),
        row("c", Why::Changed),
        row("d", Why::Gone),
    ]));
    let paths = |held: &[Held]| held.iter().map(|one| one.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&first.unwrap()), ["a", "c", "d"]);
    assert_eq!(paths(&after), ["b"]);
    let (first, after) = split_for_guard(Err("no".into()));
    assert_eq!(first, Err("no".to_owned()));
    assert!(after.is_empty());
}

/// A file given a second name after `add_checked` looked and before it staged: its descriptor
/// shows the second name, so its bytes are never hashed into the object store.
#[cfg(unix)]
#[test]
fn a_second_name_made_as_a_checked_add_stages_never_reaches_the_object_store() {
    use crate::planegit::tests::{AFTER_THE_ADD, BEFORE_THE_ADD, doing_at};

    let (_dir, root) = staged();
    run(&root, &["commit", "-q", "-m", "two"]);
    std::fs::write(root.join("c.md"), "two names by now: canary-b61a93\n").unwrap();
    let second = root.parent().unwrap().join("second-name");

    let stored_once_staged = std::rc::Rc::new(std::cell::Cell::new(None));
    let added = doing_at(
        &BEFORE_THE_ADD,
        {
            let second = second.clone();
            move |root: &Path| std::fs::hard_link(root.join("c.md"), &second).unwrap()
        },
        || {
            doing_at(
                &AFTER_THE_ADD,
                {
                    let stored = stored_once_staged.clone();
                    let second = second.clone();
                    move |root: &Path| {
                        stored.set(Some(crate::testgit::stores_file(root, &second)));
                    }
                },
                || add_checked(&root, &[], "Refused", "Failed"),
            )
        },
    );

    assert_eq!(
        stored_once_staged.get(),
        Some(false),
        "the file's bytes were hashed into the object store: {added:?}"
    );
    let said = added.expect_err("the add is refused");
    assert!(
        said.starts_with("Refused: c.md has a second name (a hard link)"),
        "{said}"
    );
    assert!(!crate::testgit::stores_file(&root, &second));
    assert_eq!(run(&root, &["diff", "--cached", "--name-only"]), "");
}

/// Each regular file as [`stage_rows`] takes it: looked at and opened once.
fn rows_of(root: &Path, paths: &[&str]) -> Vec<Row> {
    paths
        .iter()
        .map(|path| Row::of((*path).to_owned(), opened(root, path)).unwrap())
        .collect()
}

/// The mode and the bytes the index holds for `path`.
fn staged_as(root: &Path, path: &str) -> (String, String) {
    let row = run(root, &["ls-files", "-s", "--", path]);
    let mode = row.split(' ').next().unwrap_or_default().to_owned();
    (mode, run(root, &["cat-file", "-p", &format!(":{path}")]))
}

/// A file given a second name after it was opened and before its bytes are hashed: its
/// descriptor is asked again, so its bytes never reach the object store, and no row is written.
#[cfg(unix)]
#[test]
fn a_second_name_made_before_the_hashing_never_reaches_the_object_store_and_nothing_is_staged() {
    let (_dir, root) = staged();
    run(&root, &["commit", "-q", "-m", "two"]);
    std::fs::write(root.join("c.md"), "two names by now: canary-41d07e\n").unwrap();
    std::fs::write(root.join("d.md"), "one name\n").unwrap();
    let second = root.parent().unwrap().join("second-name");
    let rows = rows_of(&root, &["c.md", "d.md"]);

    let held = before_hashing_doing(
        {
            let second = second.clone();
            move |root: &Path| std::fs::hard_link(root.join("c.md"), &second).unwrap()
        },
        || stage_rows(&root, rows),
    )
    .unwrap();

    assert_eq!(
        held,
        [Held {
            path: "c.md".to_owned(),
            why: Why::SecondName
        }]
    );
    assert!(!crate::testgit::stores_file(&root, &second));
    // And the blob hashed for the other file, with no row written for it, is gone again.
    assert!(!crate::testgit::stores_file(&root, &root.join("d.md")));
    assert_eq!(run(&root, &["diff", "--cached", "--name-only"]), "");
}

/// The bytes a caller read from the descriptor and looked at are what is staged, whatever the
/// file holds a moment later.
#[test]
fn the_bytes_that_were_checked_are_what_is_staged() {
    use std::io::Read;

    let (_dir, root) = staged();
    std::fs::write(root.join("c.md"), "checked\n").unwrap();
    let OnDisk::File(file, runs) = opened(&root, "c.md") else {
        panic!("c.md is a regular file of one name");
    };
    let mut checked = Vec::new();
    (&file).read_to_end(&mut checked).unwrap();
    // Written over in place once it was read: the same file, other bytes.
    std::fs::write(root.join("c.md"), "written after the check\n").unwrap();

    let row = Row {
        path: "c.md".to_owned(),
        from: Source::File {
            file,
            runs,
            checked: Some(checked),
        },
    };
    assert_eq!(stage_rows(&root, vec![row]).unwrap(), []);
    refresh(&root, false).unwrap();

    assert_eq!(run(&root, &["show", ":c.md"]), "checked\n");
    assert!(whys(&root).contains(&("c.md".to_owned(), Why::Changed)));
}

/// A link, a deletion and a nested repository are staged by hand from what was looked at: what
/// the names hold by the time the rows are written is never read.
#[cfg(unix)]
#[test]
fn a_link_a_deletion_and_a_nested_repository_are_staged_as_they_were_looked_at() {
    let (_dir, root) = staged();
    run(&root, &["commit", "-q", "-m", "two"]);
    std::os::unix::fs::symlink("first-target", root.join("link")).unwrap();
    std::fs::remove_file(root.join("b.md")).unwrap();
    let inner = root.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    run(&inner, &["init", "-q", "-b", "main", "."]);
    run(&inner, &["config", "user.name", "Fixture"]);
    run(&inner, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(inner.join("x"), "x\n").unwrap();
    run(&inner, &["add", "x"]);
    run(&inner, &["commit", "-q", "-m", "inner"]);
    let inner_head = run(&inner, &["rev-parse", "HEAD"]).trim().to_owned();

    let held = before_hashing_doing(
        |root: &Path| {
            std::fs::remove_file(root.join("link")).unwrap();
            std::os::unix::fs::symlink("second-target", root.join("link")).unwrap();
            std::fs::write(root.join("b.md"), "back after the look\n").unwrap();
        },
        || stage_named(&root, &[], |_| false),
    )
    .unwrap();

    assert_eq!(held, []);
    assert_eq!(
        staged_as(&root, "link"),
        ("120000".to_owned(), "first-target".to_owned())
    );
    assert_eq!(run(&root, &["ls-files", "--", "b.md"]), "");
    let nested = run(&root, &["ls-files", "-s", "--", "inner"]);
    assert!(
        nested.starts_with(&format!("160000 {inner_head} 0\tinner")),
        "{nested}"
    );
}

/// Many files are hashed on threads of their own, each row staged with its own bytes and mode.
#[cfg(unix)]
#[test]
fn many_files_are_each_staged_with_their_own_bytes_and_mode() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, root) = staged();
    run(&root, &["commit", "-q", "-m", "two"]);
    // More than one part's worth, so a staging holds no more than a part's files open.
    let names: Vec<String> = (0..OPEN_AT_ONCE + HASHERS + 1)
        .map(|n| format!("many/{n}.md"))
        .collect();
    std::fs::create_dir_all(root.join("many")).unwrap();
    for (n, name) in names.iter().enumerate() {
        std::fs::write(root.join(name), format!("file {n}\n")).unwrap();
        if n % 2 == 1 {
            std::fs::set_permissions(root.join(name), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
    }

    assert_eq!(stage_named(&root, &[], |_| false).unwrap(), []);

    for (n, name) in names.iter().enumerate() {
        let mode = if n % 2 == 1 { "100755" } else { "100644" };
        assert_eq!(
            staged_as(&root, name),
            (mode.to_owned(), format!("file {n}\n")),
            "{name}"
        );
    }
    assert_eq!(whys(&root), []);
}

/// A folder the file system will not open is not read as a deletion of what it holds: the
/// staging refuses, and the index keeps every row.
#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_opened_refuses_the_staging_and_removes_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, root) = staged();
    std::fs::create_dir_all(root.join("shut")).unwrap();
    std::fs::write(root.join("shut/kept.md"), "kept\n").unwrap();
    run(&root, &["add", "-A"]);
    run(&root, &["commit", "-q", "-m", "two"]);
    let shut = |mode| {
        std::fs::set_permissions(root.join("shut"), std::fs::Permissions::from_mode(mode)).unwrap();
    };
    shut(0o000);
    // Run with every permission (as root), nothing is shut out, and there is nothing to ask.
    let open_anyway = std::fs::read_dir(root.join("shut")).is_ok();
    let found = opened(&root, "shut/kept.md");
    let staged = stage_named(&root, &[":(literal)shut"], |_| false);
    shut(0o755);
    if open_anyway {
        return;
    }

    assert!(matches!(found, OnDisk::Unread(_)), "read as gone or beyond");
    assert!(Row::of("shut/kept.md".to_owned(), found).is_err());
    assert!(staged.is_err(), "{staged:?}");
    assert_eq!(run(&root, &["ls-files", "--", "shut"]), "shut/kept.md\n");
}
