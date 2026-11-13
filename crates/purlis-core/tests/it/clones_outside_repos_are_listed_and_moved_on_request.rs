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
use purlis_core::forge::gitcredential::vouch;
use purlis_core::forge::route::HostScope;
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
    purlis_core::unsteered!();
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
    let moved =
        move_into_repos::move_workspace(&root, "alpha", &Vec::new, None, &mut |from, to| {
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

// ---- submodules, and a repair that fails ----------------------------------------------------

/// A repo with one commit outside the plane, in `dir`, for a clone to take as its submodule.
fn library(dir: &Path) -> PathBuf {
    let lib = dir.join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    support::git(&lib, &["init", "-q", "-b", "main", "."]);
    std::fs::write(lib.join("lib.txt"), "lib\n").unwrap();
    support::git(&lib, &["add", "lib.txt"]);
    support::git(&lib, &["commit", "-q", "-m", "lib"]);
    lib
}

/// `lib` added to `clone` as its submodule `sub`, committed, and checked out in the clone.
fn with_submodule(clone: &Path, lib: &Path) {
    let lib = lib.to_str().unwrap();
    support::git(
        clone,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            lib,
            "sub",
        ],
    );
    support::git(clone, &["commit", "-q", "-m", "sub"]);
}

/// A submodule git reads: its status, without writing its index.
fn reads(sub: &Path) -> bool {
    support::git_tried(sub, &["--no-optional-locks", "status", "--porcelain"])
        .status
        .success()
}

/// A branch folder's submodule keeps a relative `gitdir` into the clone's `.git/worktrees/<id>/
/// modules/`, and that module a relative `core.worktree` back: moving the clone puts both a
/// level off, and `git worktree repair` mends neither. So such a workspace is refused, by name.
#[test]
fn a_workspace_whose_branch_folders_hold_submodules_is_refused_naming_them_and_nothing_moves() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let outside = tempfile::tempdir().unwrap();
    let lib = library(outside.path());
    let clone = clone_at(&root, "api");
    with_submodule(&clone, &lib);
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    support::git(
        &piece,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
            "-q",
        ],
    );
    assert!(reads(&piece.join("sub")), "the submodule works before");
    let before = tree(&ws(&root));

    let applied = fix::apply(&root, FixId::MoveIntoRepos);

    let why = refused(&applied);
    assert!(why.contains("alpha"), "{why}");
    assert!(
        why.contains(&piece.display().to_string()),
        "it names the branch folder: {why}"
    );
    assert!(why.contains("sub"), "it names the submodule: {why}");
    assert!(why.contains("nothing was moved"), "{why}");
    assert_eq!(tree(&ws(&root)), before, "something moved");
    assert!(reads(&piece.join("sub")));
}

#[test]
fn a_clones_own_submodule_moves_with_it_and_still_works() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let outside = tempfile::tempdir().unwrap();
    let lib = library(outside.path());
    let clone = clone_at(&root, "api");
    with_submodule(&clone, &lib);
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;

    said(&fix::apply(&root, FixId::MoveIntoRepos));

    assert!(reads(&ws(&root).join("repos/api/sub")));
    works(&piece);
}

/// After the move every submodule is asked for its status, and one git cannot read puts the
/// workspace back. Here, a submodule whose `.git` names its module by an absolute path, as git
/// wrote them before 1.7.10: right before the move, and nowhere after it.
#[test]
fn a_submodule_git_cannot_read_after_the_move_puts_the_workspace_back_as_it_was() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let outside = tempfile::tempdir().unwrap();
    let lib = library(outside.path());
    let clone = clone_at(&root, "api");
    with_submodule(&clone, &lib);
    std::fs::write(
        clone.join("sub/.git"),
        format!("gitdir: {}\n", clone.join(".git/modules/sub").display()),
    )
    .unwrap();
    assert!(reads(&clone.join("sub")), "the submodule works before");
    let before = tree(&ws(&root));

    let applied = fix::apply(&root, FixId::MoveIntoRepos);

    let why = refused(&applied);
    assert!(why.contains("sub"), "it names the submodule: {why}");
    assert!(why.contains("put back"), "{why}");
    assert_eq!(tree(&ws(&root)), before);
    assert!(reads(&clone.join("sub")));
}

/// `git worktree repair` reports an error, and exits so, for a branch folder whose `.git` is a
/// folder: here one someone made a repo of its own. The clone has already moved by then.
#[test]
fn a_repair_that_fails_after_the_clone_moved_leaves_the_workspace_exactly_as_it_was() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    let odd = worktree::add(&root, "alpha", "api", "odd", None)
        .expect("a second branch folder of api")
        .path;
    std::fs::remove_file(odd.join(".git")).unwrap();
    support::git(&odd, &["init", "-q", "-b", "main", "."]);
    let before = tree(&ws(&root));

    let mut moves = 0;
    let moved =
        move_into_repos::move_workspace(&root, "alpha", &Vec::new, None, &mut |from, to| {
            moves += 1;
            std::fs::rename(from, to)
        });

    let why = moved.expect_err("the repair failed");
    assert_eq!(moves, 1, "the clone moved before the repair ran");
    assert!(why.contains("git worktree repair failed"), "{why}");
    assert!(why.contains("put back"), "{why}");
    assert_eq!(tree(&ws(&root)), before);
    works(&piece);
}

// ---- a move cut short, open chats, case, and what was shown (#5569) -------------------------

const CUT_SHORT: &str = "move into repos/ cut short";

/// `move_workspace` killed at its `kill`th `mv`, after that move: what a power cut or a killed
/// process leaves, since nothing after the panic runs (no rollback, no journal removed).
fn killed_at(root: &Path, kill: usize) {
    let mut moves = 0;
    let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        move_into_repos::move_workspace(root, "alpha", &Vec::new, None, &mut |from, to| {
            moves += 1;
            if moves > kill {
                panic!("killed before move {moves}");
            }
            std::fs::rename(from, to)?;
            if moves == kill {
                panic!("killed after move {moves}");
            }
            Ok(())
        })
    }));
    assert!(killed.is_err(), "the move was not killed");
}

/// Whether git reads the branch folder at `piece`.
fn git_reads(piece: &Path) -> bool {
    support::git_tried(piece, &["status", "--porcelain"])
        .status
        .success()
}

#[test]
fn a_move_killed_midway_is_flagged_and_the_next_move_finishes_it() {
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

    killed_at(&root, 1);
    assert!(ws(&root).join("repos/api/.git").is_dir(), "api moved");
    assert!(ws(&root).join("web/.git").is_dir(), "web did not");
    assert!(
        !git_reads(&api_piece),
        "the kill left api's branch folder unrepaired"
    );

    let flagged = rows(&root);
    let row = find(&flagged, CUT_SHORT);
    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(row.detail.contains("alpha"), "{row:?}");
    assert_eq!(row.fix, Some(FixId::MoveIntoRepos), "{row:?}");

    let applied = fix::apply(&root, FixId::MoveIntoRepos);

    let said = said(&applied);
    assert!(
        said.iter().any(|line| line.contains("cut short")),
        "it says it finished the move cut short: {said:?}"
    );
    assert!(ws(&root).join("repos/web/.git").is_dir());
    works(&api_piece);
    works(&web_piece);
    let after = rows(&root);
    assert!(!after.iter().any(|r| r.name == CUT_SHORT), "{after:?}");
    assert_eq!(names(&after), Vec::<&str>::new());
}

#[test]
fn a_move_killed_after_its_last_clone_moved_is_still_offered_and_finished() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;

    killed_at(&root, 1);
    assert!(!git_reads(&piece));

    let planned = move_into_repos::plan(&root);
    assert_eq!(planned.len(), 1, "{planned:?}");
    assert_eq!(planned[0].repos, Vec::<String>::new());
    assert_eq!(planned[0].finishing, ["api"]);

    said(&fix::apply(&root, FixId::MoveIntoRepos));

    works(&piece);
    assert!(move_into_repos::plan(&root).is_empty());
}

#[test]
fn an_open_chat_the_record_gives_no_folder_still_holds_the_move_back() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "claude".to_owned(),
            name: "nowhere".to_owned(),
            cwd: None,
            number: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    purlis_core::reopen::write(&root, &record).unwrap();
    let before = tree(&ws(&root));

    let why = refused(&fix::apply(&root, FixId::MoveIntoRepos)).to_owned();

    assert!(why.contains("nowhere"), "it names the chat: {why}");
    assert_eq!(tree(&ws(&root)), before, "something moved");
}

#[test]
fn a_chat_opened_while_the_move_runs_puts_the_workspace_back() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    let before = tree(&ws(&root));
    let asked = std::cell::Cell::new(0);
    let open = || {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            Vec::new()
        } else {
            vec!["late".to_owned()]
        }
    };

    let moved = move_into_repos::move_workspace(&root, "alpha", &open, None, &mut |from, to| {
        std::fs::rename(from, to)
    });

    let why = moved.expect_err("a chat opened during the move");
    assert!(why.contains("late"), "it names the chat: {why}");
    assert!(why.contains("put back"), "{why}");
    assert_eq!(tree(&ws(&root)), before);
    works(&piece);
}

/// Whether the disk under `root` reads a name in any case as the same name, as APFS and NTFS do
/// by default.
fn ignores_case(root: &Path) -> bool {
    std::fs::write(root.join("Probe"), "").unwrap();
    let ignores = root.join("probe").exists();
    std::fs::remove_file(root.join("Probe")).unwrap();
    ignores
}

#[test]
fn a_root_folder_named_as_one_in_repos_in_another_case_is_reported_beside_it() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    if !ignores_case(&root) {
        // A disk that tells `Api` from `api` has two repos here, and nothing to report.
        return;
    }
    clone_at(&root, "repos/api");
    clone_at(&root, "Api");

    let rows = rows(&root);

    assert_eq!(names(&rows), [BESIDE]);
    let row = find(&rows, BESIDE);
    assert!(row.detail.contains("alpha/Api/"), "{row:?}");
    assert!(row.detail.contains("repos/api/ is the repo"), "{row:?}");
    assert!(move_into_repos::plan(&root).is_empty());
}

fn shown(repos: &[&str]) -> Vec<move_into_repos::Shown> {
    vec![move_into_repos::Shown {
        workspace: "alpha".to_owned(),
        repos: repos.iter().map(|r| (*r).to_owned()).collect(),
    }]
}

#[test]
fn a_clone_added_after_the_confirmation_was_shown_refuses_the_move() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let confirmed = shown(&["api"]);
    clone_at(&root, "web");
    let before = tree(&ws(&root));

    let applied = fix::apply_moving_into_repos(&root, &|_, _| Vec::new(), Some(&confirmed));

    let why = refused(&applied);
    assert!(why.contains("web"), "it names what was not shown: {why}");
    assert_eq!(tree(&ws(&root)), before, "something moved");
}

#[test]
fn the_move_takes_what_the_confirmation_showed() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "web");

    let applied =
        fix::apply_moving_into_repos(&root, &|_, _| Vec::new(), Some(&shown(&["api", "web"])));

    said(&applied);
    assert!(ws(&root).join("repos/api/.git").is_dir());
    assert!(ws(&root).join("repos/web/.git").is_dir());
}

/// A rollback that cannot move a clone back keeps its journal, and no other workspace's move
/// may write over it: the clone left in `repos/` is still flagged, and the next move mends it.
#[test]
fn a_rollback_that_leaves_a_clone_in_repos_keeps_its_journal_from_the_next_workspace() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    let beta = root.join("workspaces/beta/web");
    std::fs::create_dir_all(&beta).unwrap();
    support::git(&beta, &["init", "-q", "-b", "main", "."]);
    std::fs::write(beta.join("README.md"), "one\n").unwrap();
    support::git(&beta, &["add", "README.md"]);
    support::git(&beta, &["commit", "-q", "-m", "one"]);
    // api moves; then something takes its old place, and a chat opens, so the rollback runs and
    // cannot move api back.
    let asked = std::cell::Cell::new(0);
    let open = || {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            Vec::new()
        } else {
            vec!["late".to_owned()]
        }
    };
    let moved = move_into_repos::move_workspace(&root, "alpha", &open, None, &mut |from, to| {
        std::fs::rename(from, to)?;
        std::fs::write(from, "in the way\n")
    });
    let why = moved.expect_err("a chat opened during the move");
    assert!(why.contains("not everything could be put back"), "{why}");

    let next = move_into_repos::move_workspace(&root, "beta", &Vec::new, None, &mut |from, to| {
        std::fs::rename(from, to)
    });

    let why = next.expect_err("a move cut short is not finished yet");
    assert!(why.contains("nothing was moved"), "{why}");
    assert!(beta.join(".git").is_dir(), "beta's clone moved");
    let flagged = rows(&root);
    let row = find(&flagged, CUT_SHORT);
    assert!(row.detail.contains("alpha"), "{row:?}");
    assert!(row.detail.contains("api"), "{row:?}");

    std::fs::remove_file(ws(&root).join("api")).unwrap();
    fix::apply(&root, FixId::MoveIntoRepos);
    works(&piece);
    assert!(!rows(&root).iter().any(|r| r.name == CUT_SHORT));
}

/// A rollback killed after it moved a clone back and before it wrote its branch folders'
/// pointers back leaves the clone at the root with them pointing into `repos/`: the next move
/// mends them even when it moves nothing.
#[test]
fn a_rollback_killed_before_its_pointers_were_written_back_is_mended_by_the_next_move() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    clone_at(&root, "api");
    clone_at(&root, "web");
    let piece = worktree::add(&root, "alpha", "api", "piece", None)
        .expect("a branch folder of api")
        .path;
    // api moves and is repaired, then the run is killed before web moves; the rollback's
    // first step, api moved back, is all that happened before the kill.
    let mut moves = 0;
    let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        move_into_repos::move_workspace(&root, "alpha", &Vec::new, None, &mut |from, to| {
            moves += 1;
            if moves == 2 {
                panic!("killed before move 2");
            }
            std::fs::rename(from, to)
        })
    }));
    assert!(killed.is_err(), "the move was not killed");
    assert!(
        git_reads(&piece),
        "api's branch folder was repaired in repos/"
    );
    std::fs::rename(ws(&root).join("repos/api"), ws(&root).join("api")).unwrap();
    std::fs::remove_dir(ws(&root).join("repos")).unwrap();
    assert!(!git_reads(&piece), "its pointers still lead into repos/");
    // A chat is open, so the move itself is refused.
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "claude".to_owned(),
            name: "steward".to_owned(),
            cwd: None,
            number: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    };
    purlis_core::reopen::write(&root, &record).unwrap();

    fix::apply(&root, FixId::MoveIntoRepos);

    works(&piece);
}

/// A clone at workspace `alpha`'s root, as [`clone_at`] makes it, its `origin` on github.com.
fn on_github_at(root: &Path, name: &str) -> PathBuf {
    let clone = clone_at(root, name);
    support::git(
        &clone,
        &[
            "remote",
            "add",
            "origin",
            &format!("https://github.com/acme/{name}.git"),
        ],
    );
    clone
}

/// **Move into repos/ offers the trust in the same step** (R-339, Q184): the clones it moves
/// that purlis has no record of are offered before the move, and the person's yes records each
/// as trusted where it now is.
#[test]
fn doctor_move_into_repos_offers_the_trust_in_the_same_step() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let home = tempfile::tempdir().unwrap();
    let api = on_github_at(&root, "api");

    let offered = move_into_repos::offered(&root, home.path());
    assert_eq!(
        offered
            .iter()
            .map(|clone| (clone.folder.as_str(), clone.place.as_deref()))
            .collect::<Vec<_>>(),
        [("workspaces/alpha/api", Some("github.com/acme/api"))],
        "the move offers to trust the clone purlis has no record of"
    );

    let scope = HostScope::for_a_test();
    let applied = move_into_repos::apply_with(
        &root,
        &move_into_repos::open_in_record,
        None,
        Some(&move_into_repos::Records {
            config_root: home.path(),
            trust: Some(&scope),
        }),
    );

    let moved = ws(&root).join("repos/api");
    let said = said(&applied);
    assert!(!api.exists(), "api is still at the root: {said:?}");
    assert!(moved.join(".git").is_dir(), "{said:?}");
    assert!(
        vouch::trusted(home.path(), &moved),
        "the yes records the moved clone as trusted: {said:?}"
    );
    assert!(
        said.iter()
            .any(|line| line.contains("workspaces/alpha/repos/api")
                && line.contains("github.com/acme/api")),
        "it says what it trusted, where: {said:?}"
    );
}

/// Without the person's yes the move trusts nothing, and a clone purlis has a record of keeps
/// it where it moved to (R-336: nothing is adopted automatically, and nothing trusted is lost).
#[test]
fn answered_no_the_move_trusts_nothing_new_and_a_recorded_clone_keeps_its_record() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let home = tempfile::tempdir().unwrap();
    let api = on_github_at(&root, "api");
    on_github_at(&root, "web");
    vouch::vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        &root,
        &api,
        "github.com/acme/api",
    )
    .expect("api is trusted before the move");

    let applied = move_into_repos::apply_with(
        &root,
        &move_into_repos::open_in_record,
        None,
        Some(&move_into_repos::Records {
            config_root: home.path(),
            trust: None,
        }),
    );

    let said = said(&applied);
    assert!(
        vouch::trusted(home.path(), &ws(&root).join("repos/api")),
        "api keeps its record through the move: {said:?}"
    );
    assert!(
        !vouch::trusted(home.path(), &ws(&root).join("repos/web")),
        "web is not trusted without the person's yes: {said:?}"
    );
}

/// **The move re-records a trusted clone only while its remote is the one it was trusted at**
/// (Q257): a clone whose origin changed after the person trusted it is not trusted again where
/// it moved to, and the move says so.
#[test]
fn a_trusted_clone_whose_remote_changed_is_not_trusted_again_where_it_moved_to() {
    purlis_core::unsteered!();
    let (_d, root) = plane();
    let home = tempfile::tempdir().unwrap();
    let api = on_github_at(&root, "api");
    vouch::vouch_clone(
        &HostScope::for_a_test(),
        home.path(),
        &root,
        &api,
        "github.com/acme/api",
    )
    .expect("api is trusted before the move");
    support::git(
        &api,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/other/api.git",
        ],
    );

    let applied = move_into_repos::apply_with(
        &root,
        &move_into_repos::open_in_record,
        None,
        Some(&move_into_repos::Records {
            config_root: home.path(),
            trust: None,
        }),
    );

    let said = said(&applied);
    let moved = ws(&root).join("repos/api");
    assert!(moved.join(".git").is_dir(), "{said:?}");
    assert!(
        !vouch::trusted(home.path(), &moved),
        "a remote changed since the trust is not trusted again: {said:?}"
    );
    assert!(
        said.iter()
            .any(|line| line.contains("workspaces/alpha/repos/api")
                && line.contains("github.com/acme/api")),
        "it says which clone it did not trust again, and at what: {said:?}"
    );
}
