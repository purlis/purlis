// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `doctor` lists a workspace's clones outside `repos/`, and **Move into repos/** moves them
//! only when the operator asks (ADR 0114, decisions 3 and 4; #5374).
//!
//! Driven as the doctor's callers drive it: run the doctor, read the row, apply the fix by its
//! id, look at the plane on disk and at what git answers in every branch folder afterwards.

use crate::support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use purlis_core::doctor::fix::{self, FixId, Fixed, move_into_repos};
use purlis_core::doctor::{Doctor, Row, Status};
use purlis_core::files::{self, Branch, Mark};
use purlis_core::worktree;

const OUTSIDE: &str = "repos outside repos/";
const BESIDE: &str = "repos beside repos/";
const NAMED_REPOS: &str = "repo named repos";

/// A plane with workspace `alpha` and its baseline folders, so the only findings are the ones a
/// test makes.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    for sub in ["personas", "inventory", "workspaces/alpha"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    (dir, root)
}

fn ws(root: &Path) -> PathBuf {
    root.join("workspaces/alpha")
}

/// A clone with one commit at `below` workspace `alpha`'s folder.
fn clone_at(root: &Path, below: &str) -> PathBuf {
    let clone = ws(root).join(below);
    std::fs::create_dir_all(&clone).unwrap();
    support::git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    support::git(&clone, &["add", "README.md"]);
    support::git(&clone, &["commit", "-q", "-m", "one"]);
    clone
}

fn rows(root: &Path) -> Vec<Row> {
    Doctor::at(root, root, false, true).run()
}

fn names(rows: &[Row]) -> Vec<&str> {
    rows.iter()
        .map(|r| r.name.as_str())
        .filter(|name| [OUTSIDE, BESIDE, NAMED_REPOS].contains(name))
        .collect()
}

fn find<'a>(rows: &'a [Row], name: &str) -> &'a Row {
    rows.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
}

fn said(applied: &Fixed) -> &[String] {
    match applied {
        Fixed::Ran { said, complete } => {
            assert!(complete, "it did not finish: {said:?}");
            said
        }
        Fixed::Refused(why) => panic!("refused: {why}"),
    }
}

fn refused(applied: &Fixed) -> &str {
    match applied {
        Fixed::Refused(why) => why,
        Fixed::Ran { said, .. } => panic!("it ran: {said:?}"),
    }
}

/// Every file, directory and link under `root` with its bytes or its target: what "exactly as
/// it was" is measured against.
fn tree(root: &Path) -> BTreeMap<PathBuf, String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                out.insert(rel, format!("-> {:?}", std::fs::read_link(&path).unwrap()));
            } else if meta.is_dir() {
                out.insert(rel, "dir".to_owned());
                walk(&path, root, out);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                out.insert(rel, String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// A branch folder that works: git reads its status and makes a commit in it.
fn works(piece: &Path) {
    let status = support::git(piece, &["status", "--porcelain"]);
    assert!(
        status.status.success(),
        "git status in {}: {}",
        piece.display(),
        String::from_utf8_lossy(&status.stderr)
    );
    std::fs::write(piece.join("after.txt"), "after the move\n").unwrap();
    support::git(piece, &["add", "after.txt"]);
    let commit = support::git(piece, &["commit", "-q", "-m", "after the move"]);
    assert!(
        commit.status.success(),
        "git commit in {}: {}",
        piece.display(),
        String::from_utf8_lossy(&commit.stderr)
    );
}

/// The bounded reader Changes reads with, as the app starts it, but this test binary run
/// again, picking [`reader_child`].
fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "clones_outside_repos_are_listed_and_moved_on_request::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: answers the one question it was asked and exits; in any other run it
/// does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

/// A session record in workspace `alpha`, of a Claude Code chat that ran in `cwd`.
fn record(root: &Path, file: &str, title: &str, cwd: &str) {
    let sessions = ws(root).join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(
        sessions.join(file),
        format!(
            "---\ntitle: {title}\ndate: 2026-11-06 10:00:00\nchat: 3\nchat-name: {title}\n\
             persona: none\nharness: claude\nprofile: unknown\n\
             conversation: 0b6f1c9e-58a8-4c55-9f3e-2f7f0e1a9c11\nworkspace: alpha\ncwd: {cwd}\n\
             chat-id: unknown\ndevice: unknown\nrun: unknown\nhanded-from: none\n\
             resumed-from: none\n---\n\n# {title}\n"
        ),
    )
    .unwrap();
}

// ---- the findings ---------------------------------------------------------------------------

#[test]
fn a_plane_whose_clones_are_all_in_repos_has_none_of_the_three_findings() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "repos/api");

    assert_eq!(names(&rows(&root)), Vec::<&str>::new());
}

#[test]
fn clones_at_a_workspaces_root_are_listed_and_offer_the_move() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "web");
    clone_at(&root, "repos/docs");

    let rows = rows(&root);

    assert_eq!(names(&rows), [OUTSIDE]);
    let row = find(&rows, OUTSIDE);
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(row.detail.starts_with("2 repos outside repos/"), "{row:?}");
    assert!(row.detail.contains("alpha/api"), "{row:?}");
    assert!(row.detail.contains("alpha/web"), "{row:?}");
    assert!(!row.detail.contains("docs"), "{row:?}");
    assert_eq!(row.fix, Some(FixId::MoveIntoRepos), "{row:?}");
}

#[test]
fn a_folder_beside_the_repo_of_its_name_in_repos_is_warned_about_and_not_offered_the_move() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "repos/api");
    clone_at(&root, "api");

    let rows = rows(&root);

    assert_eq!(names(&rows), [BESIDE]);
    let row = find(&rows, BESIDE);
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(row.detail.contains("alpha/api/"), "{row:?}");
    assert!(
        row.detail.contains("repos/api/ is the repo"),
        "it says which one is read: {row:?}"
    );
    assert_eq!(row.fix, None, "{row:?}");
}

#[test]
fn an_old_clone_named_repos_is_warned_about_with_how_to_rename_it() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "repos");

    let rows = rows(&root);

    assert_eq!(names(&rows), [NAMED_REPOS]);
    let row = find(&rows, NAMED_REPOS);
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(row.detail.contains("alpha"), "{row:?}");
    assert!(row.hint.contains("rename"), "it says how: {row:?}");
    assert!(row.hint.contains("git worktree repair"), "{row:?}");
    assert_eq!(row.fix, None, "{row:?}");
}

/// ADR 0114 decision 3: the old layout is read for ever, so it is not out of date, and the
/// *layout out of date* notice (#3650) and the doctor's `workspace layout` row stay quiet.
#[test]
fn clones_at_the_root_of_a_workspace_at_the_current_layout_raise_no_layout_notice() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let code = purlis_core::wscmd::reinit::reinit(
        &root,
        purlis_core::wscmd::reinit::Scope::One("alpha"),
        chrono::Utc::now(),
        &mut |_| {},
    );
    assert_eq!(code, 0, "the workspace is brought to the current layout");

    let rows = rows(&root);

    assert!(
        !rows.iter().any(|r| r.name == "workspace layout"),
        "{:?}",
        find(&rows, "workspace layout")
    );
    assert_eq!(names(&rows), [OUTSIDE], "the doctor still lists it");
}

// ---- the move -------------------------------------------------------------------------------

#[test]
fn the_move_runs_only_by_its_name() {
    assert_eq!(FixId::parse("move-into-repos"), Some(FixId::MoveIntoRepos));
    assert!(FixId::MoveIntoRepos.by_name_only());
}

#[test]
fn the_move_puts_root_clones_in_repos_and_every_branch_folder_cut_from_them_still_works() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "web");
    let api_piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    let web_piece = worktree::add(&root, "alpha", "web", "other", None)
        .expect("a branch folder of web")
        .path;

    let applied = fix::apply(&root, FixId::MoveIntoRepos);

    let said = said(&applied);
    assert!(
        said.iter().any(|line| line.contains("alpha")),
        "it says which workspace: {said:?}"
    );
    for repo in ["api", "web"] {
        assert!(
            !ws(&root).join(repo).exists(),
            "{repo} is still at the root"
        );
        assert!(ws(&root).join("repos").join(repo).join(".git").is_dir());
    }
    // Branch folders stay where they are, and find their clone again.
    assert_eq!(
        api_piece,
        worktree::path_for(&root, "alpha", "api", "piece").unwrap()
    );
    works(&api_piece);
    works(&web_piece);
    // Changes reads the branch folder, through the clone it now finds in repos/.
    std::fs::write(api_piece.join("new.txt"), "new\n").unwrap();
    let changes = files::status(&reader(), &root, Branch::piece("alpha", "api", "piece"))
        .expect("Changes reads the branch folder");
    assert!(
        changes
            .changes
            .iter()
            .any(|change| change.path == "new.txt" && change.mark == Mark::Added),
        "{:?}",
        changes.changes
    );
    // And the doctor has nothing left to list.
    assert_eq!(names(&rows(&root)), Vec::<&str>::new());
}

#[test]
fn the_move_is_refused_while_a_chat_is_open_in_the_workspace_and_nothing_moves() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let clone = clone_at(&root, "api");
    worktree::add(&root, "alpha", "api", "piece", None).expect("a branch folder");
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "claude".to_owned(),
            name: "steward".to_owned(),
            cwd: Some(clone.clone()),
            number: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    purlis_core::reopen::write(&root, &record).unwrap();
    let before = tree(&ws(&root));

    let applied = fix::apply(&root, FixId::MoveIntoRepos);

    let why = refused(&applied);
    assert!(why.contains("alpha"), "{why}");
    assert!(why.contains("steward"), "it names the chat: {why}");
    assert_eq!(tree(&ws(&root)), before, "something moved");
}

#[test]
fn a_failure_partway_through_the_move_leaves_the_workspace_exactly_as_it_was() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "web");
    let api_piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    let web_piece = worktree::add(&root, "alpha", "web", "other", None)
        .expect("a branch folder of web")
        .path;
    let before = tree(&ws(&root));

    // The first clone moves, and is repaired; the second will not.
    let mut moves = 0;
    let moved = move_into_repos::move_workspace(&root, "alpha", &[], &mut |from, to| {
        moves += 1;
        if moves == 2 {
            return Err(std::io::Error::other("the disk said no"));
        }
        std::fs::rename(from, to)
    });

    let why = moved.expect_err("the move failed");
    assert!(why.contains("the disk said no"), "{why}");
    assert!(why.contains("put back"), "it says nothing changed: {why}");
    assert_eq!(moves, 2, "it got partway");
    assert_eq!(tree(&ws(&root)), before);
    works(&api_piece);
    works(&web_piece);
}

#[test]
fn the_confirmation_lists_chats_started_in_a_clone_and_not_those_started_in_a_branch_folder() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "repos/docs");
    record(
        &root,
        "20261106-100000-in-the-clone.md",
        "In the clone",
        "workspaces/alpha/api",
    );
    record(
        &root,
        "20261106-100100-deep-in-the-clone.md",
        "Deep in the clone",
        "workspaces/alpha/api/src",
    );
    record(
        &root,
        "20261106-100200-in-a-branch.md",
        "In a branch",
        "workspaces/alpha/.worktrees/api/piece",
    );
    record(
        &root,
        "20261106-100300-in-the-workspace.md",
        "In the workspace",
        "workspaces/alpha",
    );
    record(
        &root,
        "20261106-100400-in-repos.md",
        "In a clone already in repos",
        "workspaces/alpha/repos/docs",
    );

    let planned = move_into_repos::plan(&root);

    assert_eq!(planned.len(), 1, "{planned:?}");
    assert_eq!(planned[0].workspace, "alpha");
    assert_eq!(planned[0].repos, ["api"]);
    let chats: Vec<&str> = planned[0]
        .unresumable
        .iter()
        .map(|chat| chat.title.as_str())
        .collect();
    // Newest first, as the Sessions panel lists them.
    assert_eq!(chats, ["Deep in the clone", "In the clone"]);
}
