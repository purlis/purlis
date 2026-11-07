// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5551: a write holds the folders it checked through to the write, so a link a chat swaps in
//! between the two sends nothing elsewhere; and import's and rename's edge cases.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::super::NotWritten;
use super::super::tests::{files, project};
use super::{AFTER_THE_CHECK, Trash};

/// A Trash of the test's own, which only notes what it is handed.
#[derive(Default)]
struct Bin {
    took: RefCell<Vec<PathBuf>>,
}

impl Trash for Bin {
    fn put(&self, path: &Path) -> Result<(), String> {
        self.took.borrow_mut().push(path.to_path_buf());
        Ok(())
    }
}

/// The workspace `alpha` of [`project`], with a `plan.md` in repo `api` too.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let (dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::write(ws.join("repos/api/plan.md"), "the repo's plan\n").unwrap();
    (dir, plane)
}

/// What repo `api` holds, by name.
fn in_the_repo(ws: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(ws.join("repos/api"))
        .unwrap()
        .map(|one| one.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Between the check and the write, `notes/` is moved to `notes.held/` and a link to repo `api`
/// put in its place: what a chat in the workspace's folder can do.
#[cfg(unix)]
fn notes_swapped_for_a_link_after_the_check(ws: &Path) {
    let ws = ws.to_path_buf();
    AFTER_THE_CHECK.with(|fault| {
        *fault.borrow_mut() = Some(Box::new(move || {
            std::fs::rename(ws.join("notes"), ws.join("notes.held")).unwrap();
            std::os::unix::fs::symlink(ws.join("repos/api"), ws.join("notes")).unwrap();
        }));
    });
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_after_the_check_gets_nothing_new_through_it() {
    crate::unsteered!();
    for write in ["new file", "new folder", "import"] {
        let (_dir, plane) = workspace();
        let ws = plane.join("workspaces/alpha");
        let files = files(&plane);
        let away = tempfile::tempdir().unwrap();
        let picked = away.path().join("new.md");
        std::fs::write(&picked, "picked\n").unwrap();
        let before = in_the_repo(&ws);

        notes_swapped_for_a_link_after_the_check(&ws);
        let answer = match write {
            "new file" => files.new_file("notes", "new.md", &[]).map(|_| ()),
            "new folder" => files.new_folder("notes", "new.md", &[]).map(|_| ()),
            _ => files.import("notes", &[picked], &[]).map(|_| ()),
        };

        assert!(answer.is_ok(), "{write}: {answer:?}");
        assert_eq!(in_the_repo(&ws), before, "{write} went through the link");
        assert!(
            ws.join("notes.held/new.md").exists(),
            "{write} is made in the very folder that was checked"
        );
    }
}

#[cfg(unix)]
#[test]
fn an_entry_whose_folder_is_swapped_for_a_link_after_the_check_is_renamed_where_it_was() {
    crate::unsteered!();
    let (_dir, plane) = workspace();
    let ws = plane.join("workspaces/alpha");
    let files = files(&plane);

    notes_swapped_for_a_link_after_the_check(&ws);
    files.rename("notes/plan.md", "renamed.md", &[]).unwrap();

    assert!(ws.join("repos/api/plan.md").is_file(), "the repo's is kept");
    assert!(!ws.join("repos/api/renamed.md").exists());
    assert_eq!(
        std::fs::read_to_string(ws.join("notes.held/renamed.md")).unwrap(),
        "the plan\n"
    );
}

#[cfg(unix)]
#[test]
fn a_delete_whose_folder_is_swapped_for_a_link_after_the_check_is_refused() {
    crate::unsteered!();
    let (_dir, plane) = workspace();
    let ws = plane.join("workspaces/alpha");
    let files = files(&plane);
    let bin = Bin::default();

    notes_swapped_for_a_link_after_the_check(&ws);
    let refused = files.trash("notes/plan.md", &[], &bin).unwrap_err();

    assert!(matches!(refused, NotWritten::Moved(_)), "{refused}");
    assert!(bin.took.borrow().is_empty(), "{:?}", bin.took.borrow());
    assert!(ws.join("repos/api/plan.md").is_file());
    assert!(ws.join("notes.held/plan.md").is_file());
}

#[cfg(unix)]
#[test]
fn a_rename_onto_another_hard_link_of_the_same_file_is_refused() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::hard_link(ws.join("notes/plan.md"), ws.join("notes/also.md")).unwrap();

    let refused = files(&plane)
        .rename("notes/plan.md", "also.md", &[])
        .unwrap_err();

    assert!(matches!(refused, NotWritten::Taken(_)), "{refused}");
    assert!(ws.join("notes/plan.md").is_file());
    assert!(ws.join("notes/also.md").is_file());
}

#[cfg(unix)]
#[test]
fn an_import_of_a_fifo_is_refused_without_waiting_for_a_writer() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let away = tempfile::tempdir().unwrap();
    let fifo = away.path().join("pipe");
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&fifo)).unwrap();
    assert!(made.success());

    let (said, answer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let refused = files(&plane).import("notes", &[fifo], &[]).unwrap_err();
        let _ = said.send(refused.to_string());
    });
    let refused = answer
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the import waited on the FIFO");

    assert!(refused.contains("is not one"), "{refused}");
}

#[test]
fn an_import_that_stops_partway_says_which_files_were_copied() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    let away = tempfile::tempdir().unwrap();
    let first = away.path().join("first.md");
    let last = away.path().join("last.md");
    std::fs::write(&first, "first\n").unwrap();
    std::fs::write(&last, "last\n").unwrap();
    let folder = away.path().join("folder");
    std::fs::create_dir(&folder).unwrap();

    let refused = files(&plane)
        .import("notes", &[first, folder, last], &[])
        .unwrap_err();

    match &refused {
        NotWritten::PartlyImported { copied, why } => {
            assert_eq!(copied, &["notes/first.md"]);
            assert!(matches!(**why, NotWritten::NotAFileToImport(_)), "{why}");
        }
        other => panic!("{other}"),
    }
    assert!(
        refused.to_string().contains("'notes/first.md'"),
        "{refused}"
    );
    assert!(ws.join("notes/first.md").is_file());
    assert!(!ws.join("notes/last.md").exists());
}
