// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! qw387: a branch folder is cut from the remote's default branch, brought up to its base and
//! pushed, by `purlis worktree add`, `update` and `push`, the verbs the app runs for a
//! sandboxed chat, which may not write a linked worktree's git directory.
//!
//! A real forge (a bare repository on this machine, by `file://` address), a clone of it in a
//! workspace, and every git call of the code under test made under a brokered isolation: no
//! global or system config, and an identity for the commits a rebase writes.

mod support;

use std::path::{Path, PathBuf};

use purlis_core::hookwire::UpdateStep;
use purlis_core::pieces::Who;
use purlis_core::pieceupdate::{self, Piece, Reach};
use purlis_core::repocmd::Say;
use purlis_core::worktree::git;

use support::git;

struct World {
    _dir: tempfile::TempDir,
    plane: PathBuf,
    forge: PathBuf,
    /// A checkout of the forge that is not the plane's: "someone else" pushing to it.
    theirs: PathBuf,
    clone: PathBuf,
}

const WS: &str = "alpha";
const REPO: &str = "widget";

fn out(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&git(dir, args).stdout)
        .trim()
        .to_owned()
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let plane = base.join("plane");
    std::fs::create_dir_all(plane.join("workspaces").join(WS)).unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let forge = base.join("forge.git");
    git(
        &base,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            forge.to_str().unwrap(),
        ],
    );
    let theirs = base.join("theirs");
    let url = format!("file://{}", forge.display());
    git(&base, &["clone", "-q", &url, theirs.to_str().unwrap()]);
    write(&theirs, "README.md", "one\n");
    write(&theirs, "shared.txt", "a\nb\nc\n");
    git(&theirs, &["add", "README.md", "shared.txt"]);
    git(&theirs, &["commit", "-q", "-m", "one"]);
    git(&theirs, &["push", "-q", "origin", "HEAD:main"]);
    let clone = plane.join("workspaces").join(WS).join(REPO);
    git(&base, &["clone", "-q", &url, clone.to_str().unwrap()]);
    World {
        _dir: dir,
        plane,
        forge,
        theirs,
        clone,
    }
}

impl World {
    /// Someone else's commit on the forge's `main`.
    fn they_commit(&self, file: &str, text: &str) {
        git(&self.theirs, &["pull", "-q", "--ff-only", "origin", "main"]);
        write(&self.theirs, file, text);
        git(&self.theirs, &["add", file]);
        git(
            &self.theirs,
            &["commit", "-q", "-m", &format!("theirs: {file}")],
        );
        git(&self.theirs, &["push", "-q", "origin", "HEAD:main"]);
    }

    fn piece(&self, name: &str) -> PathBuf {
        self.plane
            .join("workspaces")
            .join(WS)
            .join(".worktrees")
            .join(REPO)
            .join(name)
    }

    fn at<'a>(&'a self, piece: &'a str) -> Piece<'a> {
        Piece {
            root: &self.plane,
            ws: WS,
            repo: REPO,
            piece,
        }
    }
}

/// `then`, with every git call reading no global or system config and committing as a fixed
/// identity, as the app runs them for a chat.
fn brokered<T>(then: impl FnOnce() -> T) -> T {
    git::isolated(
        &git::Isolated::identity(Some("Tester"), Some("t@e.invalid")),
        then,
    )
}

fn said(lines: &[Say]) -> String {
    lines
        .iter()
        .map(|line| format!("{line:?}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn add(w: &World, piece: &str, base: Option<&str>) -> (u8, String) {
    let mut lines = Vec::new();
    let code = brokered(|| {
        purlis_core::piececmd::add(
            &w.plane,
            WS,
            REPO,
            piece,
            None,
            base,
            &Who::here(None, Some("3".to_owned()), None),
            chrono::Utc::now(),
            &mut |line| lines.push(line),
        )
    });
    (code, said(&lines))
}

fn update(w: &World, piece: &str, step: &UpdateStep, reach: &Reach) -> (u8, String) {
    let mut lines = Vec::new();
    let code = brokered(|| pieceupdate::update(&w.at(piece), step, reach, &mut |l| lines.push(l)));
    (code, said(&lines))
}

fn push(w: &World, piece: &str, lease: bool) -> (u8, String) {
    let mut lines = Vec::new();
    let code = brokered(|| {
        pieceupdate::push(&w.at(piece), lease, &Reach::default(), &mut |l| {
            lines.push(l);
        })
    });
    (code, said(&lines))
}

fn start(base: Option<&str>) -> UpdateStep {
    UpdateStep::Start {
        base: base.map(str::to_owned),
    }
}

#[test]
fn a_piece_is_cut_from_the_remotes_default_branch_whatever_the_clone_is_on() {
    purlis_core::unsteered!();
    let w = world();
    // Another chat put the clone on a branch of its own, with a commit.
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    write(&w.clone, "theirs.txt", "x\n");
    git(&w.clone, &["add", "theirs.txt"]);
    git(&w.clone, &["commit", "-q", "-m", "theirs"]);
    let main = out(&w.clone, &["rev-parse", "main"]);

    let (code, said) = add(&w, "p1", None);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("base:   main"), "{said}");
    assert_eq!(out(&w.piece("p1"), &["rev-parse", "HEAD"]), main);
    assert_eq!(
        out(&w.clone, &["config", "--get", "branch.p1.charterBase"]),
        "main"
    );

    // `--base HEAD` keeps what the clone is on.
    let (code, said) = add(&w, "p2", Some("HEAD"));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("base:   ci/theirs"), "{said}");
    assert_eq!(
        out(&w.clone, &["config", "--get", "branch.p2.charterBase"]),
        "ci/theirs"
    );

    // A branch the clone does not have is refused, and nothing is cut.
    let (code, said) = add(&w, "p3", Some("nope"));
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("does not have it"), "{said}");
    assert!(!w.piece("p3").exists());
}

#[test]
fn a_base_behind_its_remote_is_brought_up_where_nothing_has_it_checked_out() {
    purlis_core::unsteered!();
    let w = world();
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    w.they_commit("new.txt", "new\n");
    git(&w.clone, &["fetch", "-q", "origin"]);
    let upstream = out(&w.clone, &["rev-parse", "origin/main"]);

    let (code, said) = add(&w, "p1", Some("main"));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("brought up to origin/main"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "main"]), upstream);
    assert_eq!(out(&w.piece("p1"), &["rev-parse", "HEAD"]), upstream);
}

#[test]
fn an_update_fetches_and_rebases_the_piece_onto_its_base() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "mine.txt", "mine\n");
    git(&piece, &["add", "mine.txt"]);
    git(&piece, &["commit", "-q", "-m", "mine"]);
    w.they_commit("new.txt", "new\n");
    let theirs = out(&w.theirs, &["rev-parse", "HEAD"]);

    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("rebased onto main"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD~1"]), theirs);
    assert!(piece.join("new.txt").is_file());
    assert!(piece.join("mine.txt").is_file());

    // Again: nothing to do.
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("already on main"), "{said}");

    // Uncommitted changes refuse it, and nothing moves.
    w.they_commit("other.txt", "o\n");
    write(&piece, "mine.txt", "changed\n");
    let before = out(&piece, &["rev-parse", "HEAD"]);
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("uncommitted changes"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_conflict_stops_the_update_naming_the_files_and_continue_or_abort_finishes_it() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    let mine = out(&piece, &["rev-parse", "HEAD"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");

    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict in shared.txt"), "{said}");
    assert!(said.contains("--continue"), "{said}");

    // A marker left in is refused, and nothing is staged.
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict marker"), "{said}");

    // Abort puts the branch back.
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), mine);

    // Again, resolved this time.
    let (code, _) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1);
    write(&piece, "shared.txt", "a\nboth\nc\n");
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(
        std::fs::read_to_string(piece.join("shared.txt")).unwrap(),
        "a\nboth\nc\n"
    );
    assert_eq!(
        out(&piece, &["rev-parse", "HEAD~1"]),
        out(&w.theirs, &["rev-parse", "HEAD"])
    );

    // Nothing is stopped now.
    let (code, said) = update(&w, "p1", &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("no rebase is stopped"), "{said}");
}

#[test]
fn a_piece_pushes_its_own_branch_and_forces_only_with_a_lease_after_an_update() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "mine.txt", "mine\n");
    git(&piece, &["add", "mine.txt"]);
    git(&piece, &["commit", "-q", "-m", "mine"]);

    let (code, said) = push(&w, "p1", false);
    assert_eq!(code, 0, "{said}");
    assert_eq!(
        out(&w.forge, &["rev-parse", "refs/heads/p1"]),
        out(&piece, &["rev-parse", "HEAD"])
    );

    // A lease with no update before it is refused.
    let (code, said) = push(&w, "p1", true);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("only right after"), "{said}");

    // After an update rewrote it, a plain push is refused by git and the lease goes through.
    w.they_commit("new.txt", "new\n");
    assert_eq!(update(&w, "p1", &start(None), &Reach::default()).0, 0);
    let (code, _) = push(&w, "p1", false);
    assert_eq!(code, 1);
    let (code, said) = push(&w, "p1", true);
    assert_eq!(code, 0, "{said}");
    assert_eq!(
        out(&w.forge, &["rev-parse", "refs/heads/p1"]),
        out(&piece, &["rev-parse", "HEAD"])
    );
    // The forge's main is never touched.
    assert_eq!(
        out(&w.forge, &["rev-parse", "refs/heads/main"]),
        out(&w.theirs, &["rev-parse", "HEAD"])
    );
}

#[test]
fn an_update_for_a_sandboxed_chat_that_would_write_a_denied_file_does_not_start() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "mine.txt", "mine\n");
    git(&piece, &["add", "mine.txt"]);
    git(&piece, &["commit", "-q", "-m", "mine"]);
    // Upstream adds an editor's tasks file, which a chat's sandbox never lets it write.
    std::fs::create_dir_all(w.theirs.join(".vscode")).unwrap();
    w.they_commit(".vscode/tasks.json", "{}\n");
    let before = out(&piece, &["rev-parse", "HEAD"]);
    let chat = w.plane.join("workspaces").join(WS);
    let reach = Reach {
        hosts: None,
        sandboxed_at: Some(&chat),
    };

    let (code, said) = update(&w, "p1", &start(None), &reach);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), before);
    assert!(!piece.join(".vscode").exists());

    // From a terminal, the same update runs.
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
}
