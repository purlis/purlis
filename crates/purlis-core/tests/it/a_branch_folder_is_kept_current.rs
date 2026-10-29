// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! qw387: a branch folder is cut from the remote's default branch, brought up to its base and
//! pushed, by `purlis worktree add`, `update` and `push`, the verbs the app runs for a
//! sandboxed chat, which may not write a linked worktree's git directory.
//!
//! A real forge (a bare repository on this machine, by `file://` address), a clone of it in a
//! workspace, and every git call of the code under test made under a brokered isolation: no
//! global or system config, and an identity for the commits a rebase writes.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::hookwire::UpdateStep;
use purlis_core::pieces::Who;
use purlis_core::pieceupdate::{self, CloneAt, Piece, Reach};
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
    add_judged(w, piece, base, false)
}

/// [`add`], judged as the app judges a cut for a sandboxed chat where `judged` is set.
fn add_judged(w: &World, piece: &str, base: Option<&str>, judged: bool) -> (u8, String) {
    add_judged_then(w, piece, base, judged, &|| {})
}

/// [`add_judged`], with `after` run once the start has been judged and before git cuts: what
/// a chat moving a ref in between would do.
fn add_judged_then(
    w: &World,
    piece: &str,
    base: Option<&str>,
    judged: bool,
    after: &dyn Fn(),
) -> (u8, String) {
    let chat = w.plane.join("workspaces").join(WS);
    let judge = |clone: &Path, start: &str, folder: &Path| {
        let said =
            purlis_core::cloneupdate::judged_cut(clone, start, folder, &w.plane, Some(&chat))
                .map_or(Ok(()), Err);
        after();
        said
    };
    let vet: Option<purlis_core::worktree::Vet<'_>> = judged.then_some(&judge);
    let mut lines = Vec::new();
    let code = brokered(|| {
        purlis_core::piececmd::add(
            &w.plane,
            WS,
            REPO,
            piece,
            None,
            base,
            vet,
            None,
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

#[test]
fn a_cut_for_a_sandboxed_chat_is_judged_as_its_own_checkout_would_be() {
    purlis_core::unsteered!();
    let w = world();
    // A commit that adds an editor's tasks file, which the clone does not have checked out,
    // put under the default branch's name while the clone is on another branch.
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    std::fs::create_dir_all(w.clone.join(".vscode")).unwrap();
    write(&w.clone, ".vscode/tasks.json", "{}\n");
    git(&w.clone, &["add", ".vscode/tasks.json"]);
    git(&w.clone, &["commit", "-q", "-m", "tasks"]);
    let crafted = out(&w.clone, &["rev-parse", "HEAD"]);
    git(&w.clone, &["reset", "-q", "--hard", "HEAD~1"]);
    std::fs::remove_dir_all(w.clone.join(".vscode")).ok();
    git(&w.clone, &["update-ref", "refs/heads/main", &crafted]);

    let (code, said) = add_judged(&w, "p1", None, true);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
    assert!(!w.piece("p1").exists());

    // A cut whose start writes nothing of that kind runs, judged or not.
    let (code, said) = add_judged(&w, "p2", Some("HEAD"), true);
    assert_eq!(code, 0, "{said}");
}

#[test]
fn the_apps_own_fetch_puts_back_a_remote_branch_moved_in_the_clone() {
    purlis_core::unsteered!();
    let w = world();
    let theirs = out(&w.clone, &["rev-parse", "origin/main"]);
    git(&w.clone, &["checkout", "-q", "-b", "side"]);
    write(&w.clone, "side.txt", "s\n");
    git(&w.clone, &["add", "side.txt"]);
    git(&w.clone, &["commit", "-q", "-m", "side"]);
    let side = out(&w.clone, &["rev-parse", "HEAD"]);
    git(&w.clone, &["update-ref", "refs/remotes/origin/main", &side]);

    brokered(|| purlis_core::repocmd::sync::fetch_remote(&w.plane, &w.clone, "origin", None))
        .expect("fetched");
    assert_eq!(out(&w.clone, &["rev-parse", "origin/main"]), theirs);
}

/// A commit on its own, in the clone, that adds an editor's tasks file with `mode` and is
/// not checked out: what a chat could write into the clone's objects and point a ref at.
fn crafted(w: &World, mode: &str) -> String {
    let file = w.clone.with_extension("crafted-blob");
    std::fs::write(&file, "{}\n").unwrap();
    let blob = out(&w.clone, &["hash-object", "-w", file.to_str().unwrap()]);
    let index = w.clone.with_extension("crafted-index");
    with_index(&w.clone, &index, &["read-tree", "main"]);
    with_index(
        &w.clone,
        &index,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("{mode},{blob},.vscode/tasks.json"),
        ],
    );
    let tree = with_index(&w.clone, &index, &["write-tree"]);
    with_index(
        &w.clone,
        &index,
        &["commit-tree", &tree, "-p", "main", "-m", "crafted"],
    )
}

/// git in `dir` with its index at `index`: plumbing that writes no file of the folder.
fn with_index(dir: &Path, index: &Path, args: &[&str]) -> String {
    let mut cmd = support::unsigned();
    cmd.arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_INDEX_FILE", index)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t");
    let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_cut_checks_out_the_commit_it_judged_whatever_the_ref_names_after() {
    purlis_core::unsteered!();
    let w = world();
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    let main = out(&w.clone, &["rev-parse", "main"]);
    let crafted = crafted(&w, "100644");
    let clone = w.clone.clone();
    let move_main = || {
        git(&clone, &["update-ref", "refs/heads/main", &crafted]);
    };

    let (code, said) = add_judged_then(&w, "p1", Some("main"), true, &move_main);
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&w.piece("p1"), &["rev-parse", "HEAD"]), main);
    assert!(!w.piece("p1").join(".vscode").exists());
}

#[test]
fn a_cut_is_judged_by_mode_as_well_as_content() {
    purlis_core::unsteered!();
    let w = world();
    // The clone has the tasks file checked out, as a plain file.
    std::fs::create_dir_all(w.clone.join(".vscode")).unwrap();
    write(&w.clone, ".vscode/tasks.json", "{}\n");
    git(&w.clone, &["add", ".vscode/tasks.json"]);
    git(&w.clone, &["commit", "-q", "-m", "tasks"]);
    let main = out(&w.clone, &["rev-parse", "main"]);
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    // Main moves to the same file, made executable.
    let index = w.clone.with_extension("mode-index");
    let with = |args: &[&str]| with_index(&w.clone, &index, args);
    with(&["read-tree", &main]);
    with(&["update-index", "--chmod=+x", ".vscode/tasks.json"]);
    let tree = with(&["write-tree"]);
    let exec = with(&["commit-tree", &tree, "-p", &main, "-m", "exec"]);
    git(&w.clone, &["update-ref", "refs/heads/main", &exec]);

    let (code, said) = add_judged(&w, "p1", Some("main"), true);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
}

#[test]
fn the_default_base_is_asked_of_the_remote_and_never_read_from_the_clone() {
    purlis_core::unsteered!();
    let w = world();
    git(&w.clone, &["checkout", "-q", "-b", "ci/theirs"]);
    // The clone's own record of the remote's default, pointed elsewhere.
    git(&w.clone, &["branch", "evil", "main"]);
    git(
        &w.clone,
        &["update-ref", "refs/remotes/origin/evil", "main"],
    );
    git(
        &w.clone,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/evil",
        ],
    );

    let (code, said) = add(&w, "p1", None);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("base:   main"), "{said}");
}

#[test]
fn a_lease_is_asked_of_purlis_s_own_record_of_its_update_never_of_the_reflog() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "mine.txt", "mine\n");
    git(&piece, &["add", "mine.txt"]);
    git(&piece, &["commit", "-q", "-m", "mine"]);
    assert_eq!(push(&w, "p1", false).0, 0);
    // A reflog line that reads like a rebase, written by anyone who may write the clone's refs.
    git(
        &w.clone,
        &[
            "update-ref",
            "-m",
            "rebase (finish): refs/heads/p1 onto x",
            "refs/heads/p1",
            "p1",
        ],
    );
    let (code, said) = push(&w, "p1", true);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("only right after"), "{said}");
}

#[test]
fn an_update_whose_branch_moved_while_it_was_stopped_leaves_the_branch_where_it_was_moved() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    let (code, _) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1);
    // The branch is moved while the update is stopped.
    let elsewhere = out(&w.clone, &["rev-parse", "main"]);
    git(&w.clone, &["update-ref", "refs/heads/p1", &elsewhere]);
    write(&piece, "shared.txt", "a\nboth\nc\n");

    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("moved while purlis worked on it"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "refs/heads/p1"]), elsewhere);
}

/// Whether the object store of the repository `dir` is in holds `sha`.
fn holds(dir: &Path, sha: &str) -> bool {
    git::run(dir, &["cat-file", "-e", sha], git::READ).is_ok_and(|run| run.ok())
}

#[cfg(unix)]
#[test]
fn a_resolved_file_with_a_second_name_is_not_staged_and_the_rebase_stays_stopped() {
    purlis_core::unsteered!();
    let w = world();
    let (code, cut) = add(&w, "p1", None);
    assert_eq!(code, 0, "{cut}");
    let piece = w.piece("p1");
    write(&piece, "shared.txt", "a\nmine\nc\n");
    git(&piece, &["commit", "-q", "-am", "mine"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    let (code, said) = update(&w, "p1", &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");

    write(&piece, "shared.txt", "a\nresolved under two names\nc\n");
    let second = w.plane.join("second-name");
    std::fs::hard_link(piece.join("shared.txt"), &second).unwrap();
    let blob = out(&piece, &["hash-object", "shared.txt"]);
    let head = out(&piece, &["rev-parse", "HEAD"]);

    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("has a second name"), "{said}");
    assert!(said.contains("Nothing was staged"), "{said}");
    assert_eq!(out(&piece, &["rev-parse", "HEAD"]), head);
    assert!(
        !out(&piece, &["ls-files", "-u", "--", "shared.txt"]).is_empty(),
        "the resolved file was staged"
    );
    assert!(
        !holds(&piece, &blob),
        "the file's bytes reached the object store"
    );

    // With one name again, the rebase goes on.
    std::fs::remove_file(&second).unwrap();
    let (code, said) = update(&w, "p1", &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
}

/// #3831: `purlis worktree update` standing in a workspace's clone, which the app runs for a
/// sandboxed chat as it runs a branch folder's.
fn update_clone(w: &World, step: &UpdateStep, reach: &Reach) -> (u8, String) {
    let mut lines = Vec::new();
    let at = CloneAt {
        root: &w.plane,
        ws: WS,
        repo: REPO,
    };
    let code = brokered(|| pieceupdate::update_clone(&at, step, reach, &mut |l| lines.push(l)));
    (code, said(&lines))
}

#[test]
fn a_clone_behind_its_remote_is_fetched_and_fast_forwarded() {
    purlis_core::unsteered!();
    let w = world();
    w.they_commit("new.txt", "new\n");
    let theirs = out(&w.theirs, &["rev-parse", "HEAD"]);

    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("fast-forwarded"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), theirs);
    assert_eq!(out(&w.clone, &["branch", "--show-current"]), "main");
    assert_eq!(
        std::fs::read_to_string(w.clone.join("new.txt")).unwrap(),
        "new\n"
    );

    // Nothing more to bring in.
    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("already"), "{said}");
}

#[test]
fn a_clone_s_own_commits_are_rebased_onto_its_remote_and_a_conflict_goes_on_or_back() {
    purlis_core::unsteered!();
    let w = world();
    write(&w.clone, "mine.txt", "mine\n");
    git(&w.clone, &["add", "mine.txt"]);
    git(&w.clone, &["commit", "-q", "-m", "mine"]);
    w.they_commit("new.txt", "new\n");

    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("rebased onto origin/main"), "{said}");
    assert_eq!(
        out(&w.clone, &["rev-parse", "HEAD~1"]),
        out(&w.theirs, &["rev-parse", "HEAD"])
    );
    assert_eq!(out(&w.clone, &["branch", "--show-current"]), "main");

    // A conflict stops it and names the file; abort puts the branch back.
    write(&w.clone, "shared.txt", "a\nmine\nc\n");
    git(&w.clone, &["commit", "-q", "-am", "mine again"]);
    let mine = out(&w.clone, &["rev-parse", "HEAD"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("conflict in shared.txt"), "{said}");
    let (code, said) = update_clone(&w, &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), mine);
    assert_eq!(out(&w.clone, &["branch", "--show-current"]), "main");

    // Resolved, it goes on.
    let (code, _) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 1);
    write(&w.clone, "shared.txt", "a\nboth\nc\n");
    let (code, said) = update_clone(&w, &UpdateStep::Continue, &Reach::default());
    assert_eq!(code, 0, "{said}");
    // Both of the clone's own commits, on top of theirs.
    assert_eq!(
        out(&w.clone, &["rev-parse", "HEAD~2"]),
        out(&w.theirs, &["rev-parse", "HEAD"])
    );
    assert_eq!(out(&w.clone, &["branch", "--show-current"]), "main");
}

#[test]
fn a_clone_update_never_moves_uncommitted_work_or_a_file_the_commit_does_not_track() {
    purlis_core::unsteered!();
    let w = world();
    w.they_commit("new.txt", "new\n");
    let before = out(&w.clone, &["rev-parse", "HEAD"]);

    // A change to a tracked file: refused, and nothing moved.
    write(&w.clone, "README.md", "edited\n");
    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("uncommitted changes"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), before);
    git(&w.clone, &["checkout", "-q", "--", "README.md"]);

    // An ignored file the update would write over: refused, named, and kept.
    write(&w.clone, ".gitignore", "new.txt\n");
    write(&w.clone, "new.txt", "my local copy\n");
    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("would overwrite new.txt"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(w.clone.join("new.txt")).unwrap(),
        "my local copy\n"
    );

    // Moved out of the way, the update runs.
    std::fs::remove_file(w.clone.join("new.txt")).unwrap();
    let (code, said) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 0, "{said}");

    // `--base` is a branch folder's.
    let (code, said) = update_clone(&w, &start(Some("main")), &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("`--base` is for a branch folder"), "{said}");
}

#[test]
fn a_clone_update_for_a_sandboxed_chat_that_would_write_a_denied_file_does_not_start() {
    purlis_core::unsteered!();
    let w = world();
    std::fs::create_dir_all(w.theirs.join(".vscode")).unwrap();
    w.they_commit(".vscode/tasks.json", "{}\n");
    let before = out(&w.clone, &["rev-parse", "HEAD"]);
    let chat = w.plane.join("workspaces").join(WS);
    let reach = Reach {
        hosts: None,
        sandboxed_at: Some(&chat),
    };

    let (code, said) = update_clone(&w, &start(None), &reach);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains(".vscode/tasks.json"), "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), before);
    assert!(!w.clone.join(".vscode").exists());
}

#[test]
fn an_abort_is_only_of_a_rebase_purlis_started_never_of_the_persons_own() {
    purlis_core::unsteered!();
    // #3831: `--abort` undoes the update purlis started for the chat; a rebase the person
    // started in their own terminal is theirs to finish.
    let w = world();
    write(&w.clone, "shared.txt", "a\nmine\nc\n");
    git(&w.clone, &["commit", "-q", "-am", "mine"]);
    w.they_commit("shared.txt", "a\ntheirs\nc\n");
    git(&w.clone, &["fetch", "-q", "origin"]);
    // The person's own rebase, stopped on the conflict.
    let mut person = support::unsigned();
    person
        .args(["-c", "user.name=t", "-c", "user.email=t@e.invalid", "-C"])
        .arg(&w.clone)
        .args(["rebase", "origin/main"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    let theirs = purlis_core::forklock::output(&mut person).expect("git runs");
    assert!(
        !theirs.status.success(),
        "the person's rebase stops on the conflict"
    );
    let git_dir = PathBuf::from(out(&w.clone, &["rev-parse", "--absolute-git-dir"]));
    assert!(git_dir.join("rebase-merge").is_dir());

    let (code, said) = update_clone(&w, &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("not one purlis started"), "{said}");
    assert!(
        git_dir.join("rebase-merge").is_dir(),
        "the person's rebase was aborted"
    );

    // Theirs to abort; then purlis's own update, stopped, is purlis's to abort.
    git(&w.clone, &["rebase", "--abort"]);
    let mine = out(&w.clone, &["rev-parse", "HEAD"]);
    let (code, _) = update_clone(&w, &start(None), &Reach::default());
    assert_eq!(code, 1);
    let (code, said) = update_clone(&w, &UpdateStep::Abort, &Reach::default());
    assert_eq!(code, 0, "{said}");
    assert_eq!(out(&w.clone, &["rev-parse", "HEAD"]), mine);
}
