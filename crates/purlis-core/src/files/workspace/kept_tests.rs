// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a LIVE workspace's files are kept, and Save with the project (#5377).

use std::path::{Path, PathBuf};

use super::{Files, Kept, NotHere};
use crate::wscmd::saved::NotSaved;

/// A project whose `.gitignore` has `init`'s workspace lines, with workspace `alpha` holding a
/// repo, its memory, its local skills, a note folder and a script.
fn project(live: bool) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(plane.join("charter.toml"), "").unwrap();
    std::fs::write(
        plane.join(".gitignore"),
        "/.charter/\n/workspaces/*/*\n!/workspaces/.gitkeep\n",
    )
    .unwrap();
    if live {
        crate::wscmd::set_live(&plane, "alpha", true).unwrap();
    }
    let ws = plane.join("workspaces/alpha");
    for folder in ["repos/api/.git", "memory", "skills.local/mine", "notes"] {
        std::fs::create_dir_all(ws.join(folder)).unwrap();
    }
    for (path, text) in [
        ("memory/one.md", "a memory\n"),
        ("notes/plan.md", "the plan\n"),
        ("run.sh", "#!/bin/sh\n"),
    ] {
        std::fs::write(ws.join(path), text).unwrap();
    }
    (dir, plane)
}

fn kept_at(plane: &Path, folder: &str) -> Vec<(String, Option<Kept>)> {
    Files::of(plane, "alpha")
        .unwrap()
        .tree(folder, &[])
        .unwrap()
        .entries
        .into_iter()
        .map(|one| (one.name, one.kept))
        .collect()
}

fn gitignore(plane: &Path) -> String {
    std::fs::read_to_string(plane.join(".gitignore")).unwrap()
}

#[test]
fn a_live_workspaces_files_each_say_where_they_are_kept() {
    let (_dir, plane) = project(true);
    let this = Some(Kept::ThisMachineOnly);
    let shared = Some(Kept::WithTheProject);
    assert_eq!(
        kept_at(&plane, ""),
        [
            ("memory".to_string(), shared),
            ("notes".to_string(), this),
            ("skills.local".to_string(), this),
            ("run.sh".to_string(), this),
        ]
    );
    assert_eq!(kept_at(&plane, "notes"), [("plan.md".to_string(), this)]);
}

#[test]
fn save_with_the_project_marks_that_entry_and_what_it_holds_and_nothing_else() {
    let (_dir, plane) = project(true);
    assert!(
        Files::of(&plane, "alpha")
            .unwrap()
            .save_with_the_project("notes", &[])
            .unwrap()
    );
    let top = kept_at(&plane, "");
    assert!(top.contains(&("notes".to_string(), Some(Kept::WithTheProject))));
    assert!(top.contains(&("run.sh".to_string(), Some(Kept::ThisMachineOnly))));
    assert_eq!(
        kept_at(&plane, "notes"),
        [("plan.md".to_string(), Some(Kept::WithTheProject))]
    );
}

#[test]
fn a_workspace_that_is_not_live_marks_nothing_and_saves_nothing() {
    let (_dir, plane) = project(false);
    assert!(kept_at(&plane, "").iter().all(|(_, kept)| kept.is_none()));
    let before = gitignore(&plane);
    let refused = Files::of(&plane, "alpha")
        .unwrap()
        .save_with_the_project("notes", &[])
        .unwrap_err();
    assert!(
        matches!(refused, NotHere::NotSaved(NotSaved::NotLive(_))),
        "{refused}"
    );
    assert_eq!(gitignore(&plane), before);
}

#[test]
fn what_purlis_keeps_a_repo_a_file_below_the_top_and_one_not_there_are_never_saved() {
    let (_dir, plane) = project(true);
    let files = Files::of(&plane, "alpha").unwrap();
    let before = gitignore(&plane);
    for (path, refused) in [
        ("memory", "KeptByPurlis"),
        ("skills.local", "KeptByPurlis"),
        ("notes/plan.md", "NotOneEntry"),
        ("repos", "OfARepo"),
        ("gone.md", "Files"),
    ] {
        let why = files.save_with_the_project(path, &[]).unwrap_err();
        let kind = match why {
            NotHere::KeptByPurlis(_) => "KeptByPurlis",
            NotHere::NotSaved(NotSaved::NotOneEntry(_)) => "NotOneEntry",
            NotHere::OfARepo(_) => "OfARepo",
            NotHere::Files(_) => "Files",
            _ => "other",
        };
        assert_eq!(kind, refused, "{path}: {why}");
    }
    assert_eq!(gitignore(&plane), before);
}

#[test]
fn a_repo_whose_name_is_no_repos_is_still_a_repo_and_never_saved() {
    let (_dir, plane) = project(true);
    std::fs::create_dir_all(plane.join("workspaces/alpha/My Tool/.git")).unwrap();
    let before = gitignore(&plane);
    let files = Files::of(&plane, "alpha").unwrap();
    let why = files.save_with_the_project("My Tool", &[]).unwrap_err();
    assert!(matches!(why, NotHere::OfARepo(_)), "{why}");
    assert_eq!(gitignore(&plane), before);
    assert!(
        !kept_at(&plane, "")
            .iter()
            .any(|(name, _)| name == "My Tool"),
        "a repo is not listed as the person's own"
    );
}
