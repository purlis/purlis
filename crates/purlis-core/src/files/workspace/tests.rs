// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{Kind, Opened};
use super::{Entry, Files, NotHere, View, managed, view_of};
use crate::repofolder::Layout;

/// A project with workspace `alpha`: a repo `api` in `repos/`, an old clone `svc` at the root,
/// the branches' folders, what purlis keeps there and the person's own notes and script.
pub(super) fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    let ws = plane.join("workspaces/alpha");
    for folder in [
        "repos/api/.git",
        "svc/.git",
        ".worktrees/api/fix",
        "memory",
        "skills.local/mine",
        ".claude",
        "notes/drafts",
    ] {
        std::fs::create_dir_all(ws.join(folder)).unwrap();
    }
    for (path, text) in [
        ("repos/api/main.rs", "fn main() {}\n"),
        ("svc/lib.rs", "pub fn lib() {}\n"),
        (".worktrees/api/fix/main.rs", "fn main() {}\n"),
        ("workspace.md", "# alpha\n"),
        ("memory/one.md", "a memory\n"),
        (".claude/settings.json", "{}\n"),
        (".purlis-structure", "6\n"),
        (".env", "TOKEN=secret\n"),
        ("notes/plan.md", "the plan\n"),
        ("notes/drafts/2-first.md", "first\n"),
        ("run.sh", "#!/bin/sh\n"),
    ] {
        std::fs::write(ws.join(path), text).unwrap();
    }
    (dir, plane)
}

pub(super) fn files(plane: &Path) -> Files {
    Files::of(plane, "alpha").unwrap()
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|one| one.name.as_str()).collect()
}

fn entry<'a>(entries: &'a [Entry], name: &str) -> &'a Entry {
    entries.iter().find(|one| one.name == name).unwrap()
}

#[test]
fn the_listing_leaves_out_the_repos_in_both_layouts_and_the_branches_folders() {
    let (_dir, plane) = project();
    let top = files(&plane).tree("", &[]).unwrap();
    assert_eq!(
        names(&top.entries),
        vec![
            ".claude",
            "memory",
            "notes",
            "skills.local",
            ".env",
            ".purlis-structure",
            "run.sh",
            "workspace.md",
        ],
        "folders first, each kind in reading order, and no repos/, svc/ or .worktrees/"
    );
    assert_eq!(
        top.repos
            .iter()
            .map(|one| (one.name.as_str(), one.layout))
            .collect::<Vec<_>>(),
        vec![("api", Layout::Repos), ("svc", Layout::Root)],
        "the repos are said apart, each with where it is"
    );
}

#[test]
fn a_folder_below_the_top_lists_no_repos_of_its_own() {
    let (_dir, plane) = project();
    let notes = files(&plane).tree("notes", &[]).unwrap();
    assert_eq!(names(&notes.entries), vec!["drafts", "plan.md"]);
    assert!(notes.repos.is_empty());
}

#[test]
fn what_purlis_keeps_is_marked_managed_and_a_dotfile_hidden() {
    let (_dir, plane) = project();
    let top = files(&plane).tree("", &["tracker".to_string()]).unwrap();
    for kept in [
        "memory",
        "skills.local",
        ".claude",
        ".purlis-structure",
        "workspace.md",
    ] {
        assert!(entry(&top.entries, kept).managed, "{kept} is purlis's");
    }
    for own in ["notes", "run.sh", ".env"] {
        assert!(!entry(&top.entries, own).managed, "{own} is the person's");
    }
    for hidden in [".claude", ".env", ".purlis-structure"] {
        assert!(entry(&top.entries, hidden).hidden, "{hidden} is a dotfile");
    }
    for shown in ["memory", "notes", "run.sh", "workspace.md"] {
        assert!(!entry(&top.entries, shown).hidden, "{shown} is no dotfile");
    }
    let memory = files(&plane).tree("memory", &[]).unwrap();
    assert!(
        entry(&memory.entries, "one.md").managed,
        "what a managed folder holds is managed too"
    );
}

#[test]
fn a_folder_an_extension_keeps_is_managed_however_it_is_cased() {
    assert!(managed("tracker/board.json", &["tracker".to_string()]));
    assert!(managed("Tracker", &["tracker".to_string()]));
    assert!(!managed("tracker", &[]));
    assert!(managed("Memory/one.md", &[]));
    assert!(managed(".purlis-generated", &[]));
    assert!(!managed("notes/memory", &[]));
}

#[test]
fn a_managed_file_says_which_view_is_its_own() {
    assert_eq!(view_of("workspace.md"), Some(View::Vision));
    assert_eq!(view_of("memory/one.md"), Some(View::Memory));
    assert_eq!(view_of("todos/todos.json"), Some(View::Todos));
    assert_eq!(view_of("sessions/2026-11-05.md"), Some(View::Sessions));
    assert_eq!(view_of("workspace.json"), None);
    assert_eq!(view_of("notes/memory/one.md"), None);
}

#[test]
fn the_persons_file_opens_as_text() {
    let (_dir, plane) = project();
    assert_eq!(
        files(&plane).open("notes/plan.md").unwrap(),
        Opened::Text {
            text: "the plan\n".to_string()
        }
    );
}

#[test]
fn a_repos_file_or_a_branch_folders_is_never_one_of_the_workspace_files() {
    let (_dir, plane) = project();
    let files = files(&plane);
    for path in [
        "repos/api/main.rs",
        "svc/lib.rs",
        "SVC/lib.rs",
        ".worktrees/api/fix/main.rs",
        ".Worktrees/api/fix/main.rs",
    ] {
        assert!(files.open(path).is_err(), "{path} opened");
        assert!(files.place(path).is_err(), "{path} placed");
        assert!(files.named(path).is_err(), "{path} named");
    }
    for folder in ["repos", "repos/api", "svc", ".worktrees"] {
        assert!(files.tree(folder, &[]).is_err(), "{folder} listed");
        assert!(files.folders(&[folder])[0].is_err(), "{folder} watched");
    }
}

#[test]
fn a_path_that_walks_up_or_is_absolute_is_refused_by_its_spelling() {
    let (_dir, plane) = project();
    let files = files(&plane);
    let outside = plane.join("workspaces/alpha/run.sh");
    for path in [
        "../beta/run.sh",
        "notes/../../beta",
        outside.to_str().unwrap(),
        "",
    ] {
        assert!(
            matches!(files.open(path), Err(NotHere::Files(_))),
            "{path:?} opened"
        );
    }
    assert!(files.tree("notes/..", &[]).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_is_listed_and_never_followed_out_of_the_workspace_folder() {
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::create_dir_all(plane.join("elsewhere")).unwrap();
    std::fs::write(plane.join("elsewhere/secret.txt"), "not yours\n").unwrap();
    std::os::unix::fs::symlink(plane.join("elsewhere"), ws.join("away")).unwrap();
    std::os::unix::fs::symlink(plane.join("elsewhere/secret.txt"), ws.join("secret.txt")).unwrap();
    std::os::unix::fs::symlink(ws.join("notes/plan.md"), ws.join("plan.md")).unwrap();
    let files = files(&plane);

    let top = files.tree("", &[]).unwrap();
    for link in ["away", "secret.txt", "plan.md"] {
        let one = entry(&top.entries, link);
        assert_eq!(one.kind, Kind::Link, "{link}");
        assert!(one.refused.is_some(), "{link} would open");
    }
    for path in ["away/secret.txt", "secret.txt", "plan.md"] {
        assert!(files.open(path).is_err(), "{path} opened");
        assert!(files.place(path).is_err(), "{path} placed");
        assert!(
            files
                .in_your_editor(path, 1, crate::youreditor::Editor::VsCode, &|_| None)
                .is_err(),
            "{path} handed to an editor"
        );
    }
    assert!(
        files.tree("away", &[]).is_err(),
        "a linked folder is listed"
    );
    assert!(
        files.folders(&["away"])[0].is_err(),
        "a linked folder is watched"
    );
    // Its own path is copied, followed nowhere.
    assert_eq!(files.named("secret.txt").unwrap(), "secret.txt");
}

#[cfg(unix)]
#[test]
fn a_workspace_folder_that_is_a_link_is_refused_whole() {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(plane.join("workspaces")).unwrap();
    std::fs::create_dir_all(plane.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(plane.join("elsewhere"), plane.join("workspaces/alpha")).unwrap();
    assert!(Files::of(&plane, "alpha").is_err());
}

#[test]
fn the_folder_itself_is_placed_for_the_empty_path_and_a_file_by_its_path() {
    let (_dir, plane) = project();
    let files = files(&plane);
    let ws = plane.join("workspaces/alpha");
    let top = files.place("").unwrap();
    assert_eq!((top.absolute, top.folder), (ws.clone(), true));
    let plan = files.place("notes/plan.md").unwrap();
    assert_eq!(plan.absolute, ws.join("notes/plan.md"));
    assert_eq!(plan.relative, "notes/plan.md");
    assert!(!plan.folder);
    assert_eq!(
        files.folders(&["", "notes"])[1].as_ref().unwrap(),
        &ws.join("notes")
    );
}

#[test]
fn the_files_found_by_name_are_the_shown_files_and_no_dotfile() {
    let (_dir, plane) = project();
    let (paths, more) = files(&plane).list(100);
    assert_eq!(
        paths,
        vec![
            "memory/one.md",
            "notes/drafts/2-first.md",
            "notes/plan.md",
            "run.sh",
            "workspace.md",
        ]
    );
    assert_eq!(more, 0);
    let (cut, more) = files(&plane).list(2);
    assert_eq!(cut.len(), 2);
    assert_eq!(more, 3);
}

#[test]
fn a_find_by_name_in_the_workspace_files_finds_the_persons_file_and_no_repos() {
    let (_dir, plane) = project();
    let scope = [super::super::Place {
        plane: &plane,
        branch: super::super::Branch::workspace_files("alpha"),
    }];
    let found = super::super::find(&scope, "main", 10);
    assert!(found.hits.is_empty(), "a repo's file was found: {found:?}");
    let found = super::super::find(&scope, "plan", 10);
    assert_eq!(
        found
            .hits
            .iter()
            .map(|hit| hit.path.as_str())
            .collect::<Vec<_>>(),
        vec!["notes/plan.md"]
    );
    assert!(found.refused.is_empty());
}

#[test]
fn a_branch_read_never_reads_the_workspace_files() {
    let (_dir, plane) = project();
    let files = super::super::Branch::workspace_files("alpha");
    assert!(super::super::tree(&plane, files, "").is_err());
    assert!(super::super::open(&plane, files, "run.sh").is_err());
    assert!(super::super::place(&plane, files, "run.sh").is_err());
}

#[test]
fn a_workspace_file_is_handed_to_a_chat_as_its_path_and_a_repos_never_is() {
    let (_dir, plane) = project();
    let files = super::super::Branch::workspace_files("alpha");
    let ws = plane.join("workspaces/alpha");
    let made = crate::reference::of(&plane, files, "notes/plan.md", None, Some(&ws)).unwrap();
    assert_eq!(made.plain(), "notes/plan.md");
    assert!(crate::reference::of(&plane, files, "svc/lib.rs", None, Some(&ws)).is_err());
    assert!(crate::reference::of(&plane, files, "", None, Some(&ws)).is_err());
}

/// #3185: a workspace file handed to your editor is held as it was checked, so a folder on its
/// way swapped for a link before the launch hands the editor nothing.
#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_after_the_check_is_not_handed_to_your_editor() {
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::create_dir_all(plane.join("elsewhere")).unwrap();
    std::fs::write(plane.join("elsewhere/plan.md"), "not yours\n").unwrap();
    let files = files(&plane);

    let checked = files
        .in_your_editor(
            "notes/plan.md",
            1,
            crate::youreditor::Editor::VsCode,
            &|_| None,
        )
        .unwrap();
    std::fs::rename(ws.join("notes"), ws.join("notes-moved")).unwrap();
    std::os::unix::fs::symlink(plane.join("elsewhere"), ws.join("notes")).unwrap();

    let refused = checked.launch().unwrap_err();
    assert!(
        matches!(refused, crate::youreditor::NotLaunched::NotHeld(_)),
        "{refused:?}"
    );
}
