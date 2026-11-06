// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5376 (ADR 0081, amended 2026-11-05): **the person's own workspace files are written from
//! Explorer, and nothing else is.** New file, new folder, rename, delete to the system's Trash and
//! import, each on a real plane with a repo in `repos/`, an old clone at the top and a branch's
//! folder; and each refused on what purlis keeps, inside a repo or a branch's folder, and outside
//! the workspace's folder through `..` or a link. Delete is refused, the file kept, where there is
//! no Trash; an import whose name is taken gets a ` (2)` suffix.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::support;

use purlis_core::files::workspace::{Files, NotWritten, Trash};

/// A plane whose workspace `alpha` has repo `api` in `repos/`, an old clone `svc` at its top, a
/// branch's folder, what purlis keeps (`workspace.md`, `memory/`, `.purlis-structure`) and the
/// person's `notes/plan.md`.
fn plane() -> (support::Fixture, PathBuf) {
    let f = support::plane_with_clone_in_repos("api");
    let ws = f.clone.parent().unwrap().parent().unwrap().to_path_buf();
    let svc = ws.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    support::git(&svc, &["init", "-q", "-b", "main", "."]);
    for folder in [".worktrees/api/fix", "memory", "notes"] {
        std::fs::create_dir_all(ws.join(folder)).unwrap();
    }
    for (path, text) in [
        ("workspace.md", "# alpha\n"),
        ("memory/one.md", "a memory\n"),
        (".purlis-structure", "6\n"),
        ("notes/plan.md", "the plan\n"),
        (".worktrees/api/fix/main.rs", "fn main() {}\n"),
    ] {
        std::fs::write(ws.join(path), text).unwrap();
    }
    (f, ws)
}

fn files(f: &support::Fixture) -> Files {
    Files::of(&f.plane, &f.ws).unwrap()
}

/// A Trash of the test's own, which keeps what it is handed in a folder of its own.
struct Bin {
    folder: tempfile::TempDir,
    took: RefCell<Vec<PathBuf>>,
}

impl Bin {
    fn new() -> Self {
        Self {
            folder: tempfile::tempdir().unwrap(),
            took: RefCell::new(Vec::new()),
        }
    }
}

impl Trash for Bin {
    fn put(&self, path: &Path) -> Result<(), String> {
        let name = path.file_name().unwrap();
        std::fs::rename(path, self.folder.path().join(name)).map_err(|e| e.to_string())?;
        self.took.borrow_mut().push(path.to_path_buf());
        Ok(())
    }
}

/// A system with no Trash for the file.
struct NoTrash;

impl Trash for NoTrash {
    fn put(&self, _path: &Path) -> Result<(), String> {
        Err("there is no Trash on this volume".to_string())
    }
}

/// Every write, at `path` (a folder for the new ones), and what each answered.
fn every_write(files: &Files, path: &str) -> Vec<(&'static str, Result<(), String>)> {
    let source = tempfile::tempdir().unwrap();
    let picked = source.path().join("picked.txt");
    std::fs::write(&picked, "picked\n").unwrap();
    let done = |result: Result<String, NotWritten>| result.map(|_| ()).map_err(|e| e.to_string());
    vec![
        ("new file", done(files.new_file(path, "new.md", &[]))),
        ("new folder", done(files.new_folder(path, "new", &[]))),
        ("rename", done(files.rename(path, "renamed", &[]))),
        (
            "delete",
            files
                .trash(path, &[], &Bin::new())
                .map_err(|e| e.to_string()),
        ),
        (
            "import",
            files
                .import(path, &[picked], &[])
                .map(|_| ())
                .map_err(|e| e.to_string()),
        ),
    ]
}

fn all_refused(answers: &[(&str, Result<(), String>)], said: &str) {
    for (what, answer) in answers {
        match answer {
            Ok(()) => panic!("{what} was made"),
            Err(why) => assert!(why.contains(said), "{what}: {why}"),
        }
    }
}

#[test]
fn a_new_file_and_a_new_folder_are_made_empty_in_the_persons_folder() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    assert_eq!(
        files.new_file("notes", "today.md", &[]).unwrap(),
        "notes/today.md"
    );
    assert_eq!(files.new_folder("", "scripts", &[]).unwrap(), "scripts");

    assert_eq!(std::fs::read(ws.join("notes/today.md")).unwrap(), b"");
    assert!(ws.join("scripts").is_dir());
}

#[test]
fn a_new_name_already_taken_is_refused_and_nothing_is_overwritten() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    let file = files.new_file("notes", "plan.md", &[]).unwrap_err();
    let folder = files.new_folder("", "notes", &[]).unwrap_err();

    assert!(matches!(file, NotWritten::Taken(_)), "{file}");
    assert!(matches!(folder, NotWritten::Taken(_)), "{folder}");
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/plan.md")).unwrap(),
        "the plan\n"
    );
}

#[test]
fn a_name_that_is_no_files_name_is_refused() {
    purlis_core::unsteered!();
    let (f, _ws) = plane();
    let files = files(&f);

    for name in ["", "  ", "a/b", "..", ".", ".git", "a\\b"] {
        let refused = files.new_file("notes", name, &[]).unwrap_err();
        assert!(
            matches!(refused, NotWritten::BadName(..)),
            "{name:?}: {refused}"
        );
    }
}

#[test]
fn a_rename_renames_in_its_folder_and_never_over_another() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    std::fs::write(ws.join("notes/old.md"), "old\n").unwrap();

    assert_eq!(
        files.rename("notes/old.md", "new.md", &[]).unwrap(),
        "notes/new.md"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/new.md")).unwrap(),
        "old\n"
    );
    assert!(!ws.join("notes/old.md").exists());

    let over = files.rename("notes/new.md", "plan.md", &[]).unwrap_err();
    assert!(matches!(over, NotWritten::Taken(_)), "{over}");
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/plan.md")).unwrap(),
        "the plan\n"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/new.md")).unwrap(),
        "old\n"
    );
}

#[test]
fn a_rename_that_changes_only_the_case_is_made() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    assert_eq!(
        files.rename("notes/plan.md", "Plan.md", &[]).unwrap(),
        "notes/Plan.md"
    );

    let names: Vec<String> = std::fs::read_dir(ws.join("notes"))
        .unwrap()
        .map(|one| one.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["Plan.md"]);
}

#[test]
fn a_delete_hands_the_file_or_folder_to_the_trash() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    let bin = Bin::new();

    files.trash("notes/plan.md", &[], &bin).unwrap();
    files.trash("notes", &[], &bin).unwrap();

    assert_eq!(
        *bin.took.borrow(),
        [ws.join("notes/plan.md"), ws.join("notes")]
    );
    assert!(!ws.join("notes").exists());
    assert!(bin.folder.path().join("plan.md").is_file());
}

#[test]
fn a_delete_is_refused_and_the_file_kept_where_there_is_no_trash() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    let refused = files.trash("notes/plan.md", &[], &NoTrash).unwrap_err();

    assert!(matches!(refused, NotWritten::NoTrash { .. }), "{refused}");
    assert!(
        refused
            .to_string()
            .contains("never deletes a file for good"),
        "{refused}"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/plan.md")).unwrap(),
        "the plan\n"
    );
}

#[test]
fn an_import_copies_and_a_name_already_taken_gets_a_2_suffix() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    let away = tempfile::tempdir().unwrap();
    let plan = away.path().join("plan.md");
    let makefile = away.path().join("Makefile");
    std::fs::write(&plan, "another plan\n").unwrap();
    std::fs::write(&makefile, "all:\n").unwrap();

    let first = files
        .import("notes", &[plan.clone(), makefile.clone()], &[])
        .unwrap();
    let again = files
        .import("notes", &[plan.clone(), makefile], &[])
        .unwrap();
    let third = files
        .import("notes", std::slice::from_ref(&plan), &[])
        .unwrap();

    assert_eq!(first, ["notes/plan (2).md", "notes/Makefile"]);
    assert_eq!(again, ["notes/plan (3).md", "notes/Makefile (2)"]);
    assert_eq!(third, ["notes/plan (4).md"]);
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/plan.md")).unwrap(),
        "the plan\n"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("notes/plan (2).md")).unwrap(),
        "another plan\n"
    );
    assert!(
        plan.is_file(),
        "an import copies, and leaves the picked file where it was"
    );
}

#[test]
fn an_import_never_takes_a_name_purlis_keeps_at_the_top() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    let away = tempfile::tempdir().unwrap();
    let picked: Vec<PathBuf> = ["workspace.md", "todos", "repos"]
        .iter()
        .map(|name| {
            let path = away.path().join(name);
            std::fs::write(&path, "mine\n").unwrap();
            path
        })
        .collect();

    let imported = files.import("", &picked, &[]).unwrap();

    assert_eq!(imported, ["workspace (2).md", "todos (2)", "repos (2)"]);
    assert_eq!(
        std::fs::read_to_string(ws.join("workspace.md")).unwrap(),
        "# alpha\n"
    );
    assert!(!ws.join("todos").exists());
}

#[test]
fn an_import_of_a_folder_is_refused() {
    purlis_core::unsteered!();
    let (f, _ws) = plane();
    let away = tempfile::tempdir().unwrap();

    let refused = files(&f)
        .import("", &[away.path().to_path_buf()], &[])
        .unwrap_err();

    assert!(
        matches!(refused, NotWritten::NotAFileToImport(_)),
        "{refused}"
    );
}

#[test]
fn every_write_is_refused_on_what_purlis_keeps() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    std::fs::create_dir_all(ws.join("ext-data")).unwrap();

    all_refused(&every_write(&files, "memory"), "kept by purlis");
    for kept in ["workspace.md", ".purlis-structure", "memory/one.md"] {
        for (what, answer) in &every_write(&files, kept)[2..4] {
            assert!(
                answer
                    .as_ref()
                    .is_err_and(|why| why.contains("kept by purlis")),
                "{what} {kept}: {answer:?}"
            );
        }
    }
    // A folder an extension keeps in the workspace is purlis's too.
    let extension = ["ext-data".to_string()];
    assert!(matches!(
        files.new_file("ext-data", "x", &extension),
        Err(NotWritten::Managed(_))
    ));
    // And no new entry, rename or import takes a name purlis keeps.
    for name in [
        "memory",
        "workspace.md",
        "todos",
        "repos",
        ".worktrees",
        ".purlis-x",
    ] {
        let made = files.new_folder("", name, &[]).unwrap_err();
        assert!(
            matches!(made, NotWritten::Reserved(_) | NotWritten::Taken(_)),
            "{name}: {made}"
        );
        let renamed = files.rename("notes", name, &[]).unwrap_err();
        assert!(
            matches!(renamed, NotWritten::Reserved(_)),
            "{name}: {renamed}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(ws.join("workspace.md")).unwrap(),
        "# alpha\n"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join("memory/one.md")).unwrap(),
        "a memory\n"
    );
    assert!(ws.join("notes/plan.md").is_file());
}

#[test]
fn every_write_is_refused_inside_a_repo_in_either_layout() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    for path in [
        "repos",
        "repos/api",
        "repos/api/README.md",
        "svc",
        "svc/README.md",
    ] {
        all_refused(&every_write(&files, path), "it is in a repo");
    }
    assert_eq!(
        std::fs::read_to_string(f.clone.join("README.md")).unwrap(),
        "one\n"
    );
    assert!(!ws.join("svc/new.md").exists());
    assert!(!f.clone.join("new").exists());
}

#[test]
fn every_write_is_refused_inside_a_branchs_folder() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    for path in [
        ".worktrees",
        ".worktrees/api/fix",
        ".worktrees/api/fix/main.rs",
    ] {
        all_refused(&every_write(&files, path), "it is in a branch's folder");
    }
    assert!(ws.join(".worktrees/api/fix/main.rs").is_file());
}

#[test]
fn every_write_is_refused_outside_the_workspace_through_walking_up() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    std::fs::create_dir_all(ws.join("../beta")).unwrap();

    for path in ["..", "../beta", "notes/../../beta", "/tmp"] {
        for (what, answer) in every_write(&files, path) {
            assert!(answer.is_err(), "{what} at {path} was made");
        }
    }
    assert_eq!(std::fs::read_dir(ws.join("../beta")).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn every_write_is_refused_through_a_link_and_a_link_is_never_followed() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let away = tempfile::tempdir().unwrap();
    std::fs::write(away.path().join("theirs.md"), "theirs\n").unwrap();
    std::os::unix::fs::symlink(away.path(), ws.join("out")).unwrap();
    std::os::unix::fs::symlink(away.path().join("theirs.md"), ws.join("notes/link.md")).unwrap();
    let files = files(&f);

    // Into the folder a link leads to: nothing is made there.
    all_refused(&every_write(&files, "out")[..2], "reached through a link");
    all_refused(
        &every_write(&files, "out/theirs.md")[2..4],
        "reached through a link",
    );
    // The link itself: neither renamed nor deleted, and what it leads to never trashed.
    let bin = Bin::new();
    assert!(matches!(
        files.trash("notes/link.md", &[], &bin),
        Err(NotWritten::ALink(_))
    ));
    assert!(matches!(
        files.rename("out", "other", &[]),
        Err(NotWritten::ALink(_))
    ));
    assert!(bin.took.borrow().is_empty());
    assert_eq!(std::fs::read_dir(away.path()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_to_string(away.path().join("theirs.md")).unwrap(),
        "theirs\n"
    );
}

#[test]
fn the_workspaces_folder_itself_is_never_renamed_or_deleted() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);

    assert!(matches!(
        files.rename("", "beta", &[]),
        Err(NotWritten::TheFolderItself)
    ));
    assert!(matches!(
        files.trash("", &[], &Bin::new()),
        Err(NotWritten::TheFolderItself)
    ));
    assert!(ws.is_dir());
}

/// #5513's review: a name the disk folds onto a repo's, the branches' folders' or a name purlis
/// keeps is that name. APFS folds `ſ` (U+017F) to `s` and the Kelvin sign (U+212A) to `k`, which
/// Rust's lowercase does not; each such spelling is refused as the name it folds to, whatever
/// the disk below does.
#[test]
fn a_name_the_disk_folds_onto_a_kept_one_is_refused_as_that_name() {
    purlis_core::unsteered!();
    let (f, ws) = plane();
    let files = files(&f);
    let bin = Bin::new();

    for (path, said) in [
        ("repoſ", "it is in a repo"),
        ("repoſ/api", "it is in a repo"),
        (".wor\u{212A}trees", "it is in a branch's folder"),
        (".worKtrees", "it is in a branch's folder"),
    ] {
        let trashed = files.trash(path, &[], &bin).unwrap_err().to_string();
        assert!(trashed.contains(said), "trash {path}: {trashed}");
        let renamed = files
            .rename(path, "elsewhere", &[])
            .unwrap_err()
            .to_string();
        assert!(renamed.contains(said), "rename {path}: {renamed}");
    }
    // A new name, there or not yet: refused as the name it folds to, on any disk.
    for name in ["workſpace.md", "todoſ", "repoſ", ".wor\u{212A}trees"] {
        let made = files.new_file("", name, &[]).unwrap_err();
        assert!(matches!(made, NotWritten::Reserved(_)), "{name}: {made}");
    }
    // A rename onto one: of a file at the top, so the new name lands beside workspace.md.
    std::fs::write(ws.join("draft.md"), "draft\n").unwrap();
    for name in ["workſpace.md", "todoſ", "repoſ"] {
        let renamed = files.rename("draft.md", name, &[]).unwrap_err();
        assert!(
            matches!(renamed, NotWritten::Reserved(_)),
            "{name}: {renamed}"
        );
    }
    assert!(bin.took.borrow().is_empty());
    assert!(ws.join("repos/api/README.md").is_file());
    assert!(ws.join(".worktrees/api/fix/main.rs").is_file());
    assert_eq!(
        std::fs::read_to_string(ws.join("workspace.md")).unwrap(),
        "# alpha\n"
    );
}
