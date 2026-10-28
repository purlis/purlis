// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `git worktree repair` across a workspace rename, through the hardened runner (#1476).
//!
//! The runner reads a folder's `.git` link itself before git runs there, and refuses a linked
//! worktree whose repository's record names another folder: one moved by hand, most often. A
//! rename moves every folder of the workspace, so it repairs them: a branch folder purlis cut
//! from its clone's side, and a checkout of a repository kept elsewhere from its own side. That
//! one repair is the only call the runner lets through a link whose record names another
//! folder. These pin, against real git, that each folder belongs to its own branch afterwards,
//! and that the exemption reaches nothing else.

use crate::support;

use std::path::Path;

use purlis_core::worktree::{self, git, link};

/// What the runner's git says in `dir`, where it answers.
fn said(dir: &Path, args: &[&str]) -> String {
    let run = git::run(dir, args, git::READ).expect("git runs");
    assert!(run.ok(), "{args:?} in {}: {run:?}", dir.display());
    run.line().to_owned()
}

/// What the runner answers in `dir`: git's status and its first words.
fn answered(dir: &Path, args: &[&str]) -> (Option<i32>, String) {
    let run = git::run(dir, args, git::READ).expect("git runs");
    (run.code, run.err.trim().to_owned())
}

#[test]
fn a_rename_repairs_each_folder_to_its_own_branch_and_the_exemption_reaches_nothing_else() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    let piece_branch = said(&piece, &["branch", "--show-current"]);
    // A checkout directly in the workspace that is a worktree of a repository kept outside the
    // project: repaired from its own side, which is what the exemption is for.
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = std::fs::canonicalize(elsewhere.path())
        .unwrap()
        .join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    support::git(&outside, &["init", "-q", "-b", "main", "."]);
    std::fs::write(outside.join("a.txt"), "a\n").unwrap();
    support::git(&outside, &["add", "a.txt"]);
    support::git(&outside, &["commit", "-q", "-m", "a"]);
    let loose = f.workspace().join("loose");
    support::git(
        &outside,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "loose-branch",
            &loose.display().to_string(),
        ],
    );
    assert_eq!(said(&loose, &["branch", "--show-current"]), "loose-branch");

    let mut lines = Vec::new();
    purlis_core::wscmd::rename::rename(
        &purlis_core::wscmd::rename::Request {
            root: &f.plane,
            old: &f.ws,
            new: "beta",
            running: &[],
            config_root: None,
        },
        &mut |line| lines.push(line.to_string()),
    );
    let ws = f.plane.join("workspaces/beta");
    assert!(ws.is_dir(), "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.contains("not repaired")),
        "{lines:?}"
    );

    // Each folder reads as its own worktree again, on its own branch, through the runner's
    // whole check, and each repository's record names it where it is now.
    let piece = ws.join(".worktrees/thing/piece");
    let loose = ws.join("loose");
    assert_eq!(said(&piece, &["branch", "--show-current"]), piece_branch);
    assert_eq!(said(&loose, &["branch", "--show-current"]), "loose-branch");
    said(&piece, &["status", "--porcelain"]);
    said(&loose, &["status", "--porcelain"]);
    let listed = said(&outside, &["worktree", "list", "--porcelain"]);
    assert!(
        listed.contains(&format!("worktree {}", loose.display())),
        "{listed}"
    );
    let listed = said(&ws.join("thing"), &["worktree", "list", "--porcelain"]);
    assert!(
        listed.contains(&format!("worktree {}", piece.display())),
        "{listed}"
    );

    // The exemption reaches nothing else. A folder purlis cut is repaired from its clone's
    // side only: moved by hand, its own repair is refused and its record is left as it is.
    let record = ws.join("thing/.git/worktrees/piece/gitdir");
    let away = format!("{}\n", ws.join("was-here/.git").display());
    std::fs::write(&record, &away).unwrap();
    assert_eq!(
        answered(&piece, &["worktree", "repair"]),
        (Some(128), link::MOVED.to_owned())
    );
    assert_eq!(std::fs::read_to_string(&record).unwrap(), away);
    std::fs::write(&record, format!("{}\n", piece.join(".git").display())).unwrap();

    // A folder pointed at the entry the loose checkout still holds was not moved: its repair
    // is refused, and the loose checkout keeps its record and its branch.
    let copy = ws.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::copy(loose.join(".git"), copy.join(".git")).unwrap();
    assert_eq!(
        answered(&copy, &["worktree", "repair"]),
        (Some(128), link::CHANGED.to_owned())
    );
    assert_eq!(said(&loose, &["branch", "--show-current"]), "loose-branch");

    // In a folder that was moved, the path form and every other verb are held to the whole
    // check: only the bare repair writes the record back.
    let moved = ws.join("moved");
    std::fs::rename(&loose, &moved).unwrap();
    for args in [
        &["status", "--porcelain"][..],
        &["worktree", "repair", "../thing"][..],
        &["rev-parse", "worktree", "repair"][..],
    ] {
        assert_eq!(
            answered(&moved, args),
            (Some(128), link::MOVED.to_owned()),
            "{args:?}"
        );
    }
    said(&moved, &["worktree", "repair"]);
    assert_eq!(said(&moved, &["branch", "--show-current"]), "loose-branch");
}
