// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

/// The command as a caller that only cares about the verdict sees it. The list it refused
/// on has a test of its own below, where it is the claim rather than a detail.
fn run(root: &Path, name: &str, force: bool) -> (u8, Vec<String>) {
    let (done, said) = run_fully(root, name, force);
    (done.code, said)
}

fn run_fully(root: &Path, name: &str, force: bool) -> (Removal, Vec<String>) {
    let mut said = Vec::new();
    let done = remove(root, name, force, &mut |line: Say| {
        said.push(line.to_string())
    });
    (done, said)
}

/// A real repository, because the guard's whole job is to read one.
fn repo(at: &Path) -> PathBuf {
    std::fs::create_dir_all(at).unwrap();
    git(at, &["init", "-q", "-b", "main"]);
    git(at, &["config", "user.email", "t@example.com"]);
    git(at, &["config", "user.name", "T"]);
    std::fs::write(at.join("README.md"), "hi\n").unwrap();
    git(at, &["add", "-A"]);
    git(at, &["commit", "-qm", "first"]);
    at.to_path_buf()
}

fn git(at: &Path, argv: &[&str]) {
    let run = crate::testgit::run(at, argv);
    assert!(run.ok(), "git {argv:?} failed: {}", run.err);
}

#[test]
fn git_is_handed_only_a_folder_of_the_workspace_spelled_plainly() {
    let going = Path::new("/var/p/workspaces/beta");
    let inside = Path::new("/private/var/p/workspaces/beta");
    for ok in [
        "/var/p/workspaces/beta/guest",
        "/private/var/p/workspaces/beta/guest",
        "/var/p/workspaces/beta/a/b",
    ] {
        assert!(plainly_inside(Path::new(ok), going, inside), "{ok}");
    }
    for refused in [
        "/var/p/workspaces/beta",
        "/var/p/workspaces/betamax/guest",
        "/var/p/workspaces/beta/../gamma/guest",
        "/var/p/workspaces/beta/guest/..",
        "/elsewhere/guest",
        "guest",
    ] {
        assert!(
            !plainly_inside(Path::new(refused), going, inside),
            "{refused}"
        );
    }
}

#[test]
fn an_empty_workspace_is_removed() {
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0, "{said:?}");
    assert!(!dir.path().join("workspaces/beta").exists());
    assert_eq!(said, vec!["✓ Removed workspace 'beta' and its clones."]);
}

#[test]
fn a_workspace_a_chat_runs_in_is_refused_forced_or_not_and_kept() {
    // V27d (ADR 0076 §10): a removal must not leave a program running in a deleted folder.
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
    for force in [false, true] {
        let mut said = Vec::new();
        let done = remove_unless_running(
            dir.path(),
            "beta",
            force,
            &["steward 1".to_owned(), "codex 2".to_owned()],
            &mut |line: Say| said.push(line.to_string()),
        );
        assert_eq!(done.code, 1, "{said:?}");
        assert_eq!(
            said,
            vec![
                "✗ Refusing to remove 'beta' — chats are running in it: steward 1, codex 2. \
                 Removing it would leave them running in a deleted folder; close them first."
            ]
        );
        assert!(dir.path().join("workspaces/beta").is_dir());
    }

    // Once nothing runs in it, it goes.
    let mut said = Vec::new();
    let done = remove_unless_running(dir.path(), "beta", false, &[], &mut |line: Say| {
        said.push(line.to_string())
    });
    assert_eq!(done.code, 0, "{said:?}");
    assert!(!dir.path().join("workspaces/beta").exists());
}

#[test]
fn with_no_app_listening_no_chat_runs_in_any_workspace() {
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
    assert!(wscmd::rename::open_in_app(dir.path(), &["beta"]).is_empty());
    assert!(wscmd::rename::adopting_in_app(dir.path(), "steward").is_empty());
}

#[test]
fn removing_a_workspace_forgets_what_its_tasks_said_and_says_what_it_leaves() {
    // #1520: the records of work in it stay, each with its brief and report; the words its
    // tasks and their asking chats sent each other go with the workspace.
    use crate::dispatchrecord::{self, Asker, Mode, Opening, Place, Worker};
    let dir = plane();
    std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
    let now = chrono::Utc::now();
    let opened = dispatchrecord::open(
        dir.path(),
        Opening {
            mode: Mode::Task,
            asker: Asker::default(),
            persona: None,
            worker: Worker::default(),
            task: None,
            place: Place {
                workspace: Some("beta".to_owned()),
                ..Place::default()
            },
            brief: "Check prod.".to_owned(),
            report_owed: false,
        },
        now,
    )
    .unwrap();
    dispatchrecord::said(
        dir.path(),
        &opened.id,
        crate::dispatchtalk::Kind::Note,
        "Still up.",
        now,
    )
    .unwrap();
    dispatchrecord::close(
        dir.path(),
        &opened.id,
        dispatchrecord::Ending::default(),
        now,
    )
    .unwrap();

    let (code, said) = run(dir.path(), "beta", false);

    assert_eq!(code, 0, "{said:?}");
    assert!(
        said.iter().any(
            |line| line.contains("Kept 1 dispatch record of work in 'beta'")
                && line.contains("forgotten now")
        ),
        "{said:?}"
    );
    let kept = dispatchrecord::read(dir.path(), &opened.id).expect("the record stays");
    assert_eq!(kept.brief, "Check prod.");
    assert_eq!(kept.talk[0].text, "");
}

#[test]
fn what_a_removal_leaves_is_said_true_of_each_record() {
    use crate::dispatchrecord::LeftBehind;
    let said = |records, forgot, running| {
        left_behind_said(
            "beta",
            LeftBehind {
                records,
                forgot,
                running,
            },
        )
    };
    assert_eq!(
        said(1, 1, 0),
        "Kept 1 dispatch record of work in 'beta' in the project's own state, each with its \
         brief and report, until it is collected 30 days after its task ended, or after its \
         report became owed to a chat that asked, where it is still owed. What 1 \
         task that ended and the chats that asked said to each other is forgotten now."
    );
    // Some ended and some run on: the running ones are not said to be forgotten.
    let mixed = said(3, 1, 2);
    assert!(mixed.contains("What 1 task that ended"), "{mixed}");
    assert!(
        mixed.ends_with(
            "2 tasks still running keep what their chats say until they end, and for 30 \
             days after."
        ),
        "{mixed}"
    );
    // All running: nothing is said to be forgotten.
    let running = said(1, 0, 1);
    assert!(!running.contains("forgotten"), "{running}");
    assert!(running.contains("1 task still running keeps what its chats say"));
}

#[test]
fn a_workspace_this_plane_does_not_have_is_refused() {
    let dir = plane();
    let (code, said) = run(dir.path(), "nope", false);
    assert_eq!(code, 1);
    assert_eq!(said, vec!["✗ no workspace 'nope'"]);
}

#[test]
fn a_dirty_clone_stops_the_removal_and_nothing_is_deleted() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    std::fs::write(clone.join("README.md"), "changed\n").unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("svc: uncommitted changes"), "{said:?}");
    assert!(said[0].contains("or pass --force"), "{said:?}");
    assert!(clone.exists(), "the guard fired, so nothing was deleted");
}

/// **The refusal hands back the reading it was made on** (charter-app#182).
///
/// Not a convenience. The surface that asks for consent draws a preview from an earlier
/// reading of the same guard, and between the two the workspace can move; without this,
/// the only list a caller has to name what is being discarded is that older one. The claim
/// here is the identity: every sentence in the list is a sentence in the refusal, and
/// nothing is in one and not the other.
#[test]
fn the_refusal_carries_the_list_it_refused_on() {
    // Two, so the assertion is about a LIST and not about one sentence that happens to
    // match: the failure #182 is about is a button naming one clone while the refusal
    // above it names another.
    let dir = plane();
    for name in ["svc", "lib"] {
        let clone = repo(&dir.path().join("workspaces/beta").join(name));
        std::fs::write(clone.join("README.md"), "changed\n").unwrap();
    }

    let (done, said) = run_fully(dir.path(), "beta", false);

    assert_eq!(done.code, 2, "{said:?}");
    let named: Vec<&str> = done
        .refused_over
        .iter()
        .map(|risk| risk.what.as_str())
        .collect();
    assert_eq!(named, vec!["lib", "svc"], "{said:?}");
    for risk in &done.refused_over {
        assert!(
            said[0].contains(&risk.said),
            "the refusal says {:?} and the list does not: {said:?}",
            risk.said
        );
    }
}

/// **And nothing else hands one back.** A caller drawing "what this discarded" from a
/// non-empty list would be drawing it on a delete that went through, and on a name that
/// was never a workspace.
#[test]
fn every_outcome_but_the_guards_refusal_refuses_over_nothing() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    std::fs::write(clone.join("README.md"), "changed\n").unwrap();

    assert_eq!(run_fully(dir.path(), "nope", false).0.refused_over, vec![]);
    assert_eq!(
        run_fully(dir.path(), "../escape", false).0.refused_over,
        vec![]
    );
    // Forced, over work the guard did find: `--force` is the operator discarding it, so
    // what went is said in the lines and never as something that stopped the command.
    let (done, said) = run_fully(dir.path(), "beta", true);
    assert_eq!(done.code, 0, "{said:?}");
    assert_eq!(done.refused_over, vec![]);
}

#[test]
fn an_untracked_file_is_uncommitted_work_too() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    std::fs::write(clone.join("scratch.txt"), "notes\n").unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("svc: uncommitted changes"), "{said:?}");
}

#[test]
fn unpushed_commits_stop_the_removal() {
    let dir = plane();
    let origin = repo(&dir.path().join("origin"));
    git(&origin, &["config", "receive.denyCurrentBranch", "ignore"]);
    let clone = dir.path().join("workspaces/beta/svc");
    std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
    git(
        dir.path(),
        &[
            "clone",
            "-q",
            &origin.display().to_string(),
            &clone.display().to_string(),
        ],
    );
    git(&clone, &["config", "user.email", "t@example.com"]);
    git(&clone, &["config", "user.name", "T"]);
    std::fs::write(clone.join("second.md"), "x\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "second"]);

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("svc: 1 unpushed commit"), "{said:?}");
    assert!(clone.exists());

    // …and `--force` is the way past it, which is what makes the guard a guard rather
    // than a wall.
    let (code, said) = run(dir.path(), "beta", true);
    assert_eq!(code, 0, "{said:?}");
    assert!(!dir.path().join("workspaces/beta").exists());
}

#[test]
fn a_clone_with_no_upstream_at_all_is_not_work_at_risk() {
    // charter#104: a fresh local repository has no upstream from the moment it exists, so
    // "has no upstream" as the rule refuses over every workspace with nothing to lose —
    // and a guard that fires on the harmless common case teaches the `--force` habit.
    let dir = plane();
    repo(&dir.path().join("workspaces/beta/svc"));
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0, "{said:?}");
}

#[test]
fn a_detached_head_with_unpushed_work_still_stops_the_removal() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    std::fs::write(clone.join("wip.md"), "x\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "wip"]);
    git(&clone, &["checkout", "-q", "--detach", "HEAD"]);
    // Detached, clean, and its commit is still on `main`, so there is nothing unique:
    // charter does not refuse over it, and that is the same rule as the worktree one.
    let (code, _said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0);

    let clone = repo(&dir.path().join("workspaces/gamma/svc"));
    git(&clone, &["checkout", "-q", "--detach", "HEAD"]);
    std::fs::write(clone.join("only-here.md"), "x\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "only here"]);
    // Now the tree is dirty-free but holds a commit no branch reaches. A CLONE is judged
    // by `status`, which is clean, so this one is removable — and the same shape inside a
    // worktree is refused, because that is where the unique-commit rule applies.
    let (code, _said) = run(dir.path(), "gamma", false);
    assert_eq!(code, 0);
}

#[test]
fn a_worktree_holding_commits_that_exist_nowhere_else_stops_the_removal() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            &piece.display().to_string(),
        ],
    );
    std::fs::write(piece.join("work.md"), "x\n").unwrap();
    git(&piece, &["add", "-A"]);
    git(&piece, &["commit", "-qm", "unique"]);

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(
        said[0].contains("svc/task: 1 commit that exists nowhere else"),
        "{said:?}"
    );
    assert!(piece.exists());
}

#[test]
fn a_worktree_holding_git_state_of_its_own_stops_the_removal() {
    // Clean, and nothing on it that no branch reaches: but a ref only that folder has,
    // which git deletes with it, as `wt remove` refuses over too (#1472).
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            &piece.display().to_string(),
        ],
    );
    git(&piece, &["update-ref", "refs/worktree/kept", "HEAD"]);

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(
        said[0].contains("svc/task: refs/worktree/kept, a ref only this folder has"),
        "{said:?}"
    );
    assert!(piece.exists());
}

#[test]
fn a_dirty_worktree_stops_the_removal() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            &piece.display().to_string(),
        ],
    );
    std::fs::write(piece.join("README.md"), "changed\n").unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(
        said[0].contains("svc/task: uncommitted changes"),
        "{said:?}"
    );
}

#[test]
fn a_clean_worktree_whose_branch_is_reachable_elsewhere_does_not_stop_the_removal() {
    let dir = plane();
    let clone = repo(&dir.path().join("workspaces/beta/svc"));
    let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            &piece.display().to_string(),
        ],
    );
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 0, "{said:?}");
}

#[test]
fn a_clone_charter_cannot_read_is_at_risk_rather_than_empty() {
    // charter#917, and the sharpest instance of it in the plane: what the list is empty of
    // is what is handed to a recursive delete.
    let dir = plane();
    let clone = dir.path().join("workspaces/beta/svc");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    // A `.git` directory with nothing in it: git answers "not a repository", which is a
    // failed `status` and not a clean one.
    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("svc: could not be read"), "{said:?}");
    assert!(clone.exists());
}

#[test]
fn a_git_dot_git_that_is_a_symlink_is_refused_and_counts_as_at_risk() {
    let dir = plane();
    let outside = tempfile::tempdir().unwrap();
    let real = repo(&outside.path().join("real"));
    let clone = dir.path().join("workspaces/beta/svc");
    std::fs::create_dir_all(&clone).unwrap();
    std::os::unix::fs::symlink(real.join(".git"), clone.join(".git")).unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 2, "{said:?}");
    assert!(said[0].contains("svc"), "{said:?}");
    assert!(real.exists(), "nothing outside the plane was touched");
}

#[test]
fn open_todos_are_reported_and_never_guard_the_removal() {
    let dir = plane();
    let ws = crate::workspaces::Plane::open(dir.path())
        .workspace("beta")
        .unwrap();
    ws.add_todo("ship the thing", "2026-05-04T11:32:17".parse().unwrap())
        .unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(
        code, 0,
        "a todo is a note about the future, not work: {said:?}"
    );
    assert!(
        said.iter()
            .any(|l| l.contains("Discarding 1 open todo with 'beta'")),
        "{said:?}"
    );
    assert!(!dir.path().join("workspaces/beta").exists());
}

#[test]
fn a_workspace_that_is_a_link_out_of_the_plane_is_refused_and_its_target_survives() {
    let dir = plane();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("treasure")).unwrap();
    std::fs::write(outside.path().join("treasure").join("keep.txt"), "x").unwrap();
    std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("treasure"),
        dir.path().join("workspaces").join("beta"),
    )
    .unwrap();

    let (code, said) = run(dir.path(), "beta", false);
    assert_eq!(code, 1, "{said:?}");
    assert!(
        said[0].contains("does not resolve to a directory inside this plane"),
        "{said:?}"
    );
    assert!(outside.path().join("treasure").join("keep.txt").exists());
}

/// A workspace `beta` holding `guest`, a linked worktree of a repository outside the
/// plane whose exclude holds the operator's line and purlis's block listing one path; the
/// exclude file and the operator's text.
fn beta_reading_an_exclude_elsewhere(plane: &Path, outside: &Path) -> (PathBuf, &'static str) {
    let main = repo(&outside.join("main"));
    let piece = plane.join("workspaces/beta/guest");
    std::fs::create_dir_all(piece.parent().unwrap()).unwrap();
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "guest",
            &piece.display().to_string(),
        ],
    );
    let exclude = main.join(".git").join("info").join("exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    let theirs = "# mine\n/notes.txt\n";
    std::fs::write(
        &exclude,
        format!(
            "{theirs}{}\n/.claude/settings.json\n{}\n",
            crate::guest::EXCLUDE_BEGIN,
            crate::guest::EXCLUDE_END
        ),
    )
    .unwrap();
    (exclude, theirs)
}

#[test]
fn an_exclude_block_the_delete_cannot_reach_is_cleared_and_only_it() {
    // charter#870, #998: a checkout inside the workspace whose git directory is elsewhere
    // would keep purlis's managed block after the workspace is gone. No other checkout
    // purlis wires reads that exclude, so the whole block goes, and the operator's line
    // above it stays byte for byte.
    let dir = plane();
    let outside = tempfile::tempdir().unwrap();
    let (exclude, theirs) = beta_reading_an_exclude_elsewhere(dir.path(), outside.path());

    let (code, said) = run(dir.path(), "beta", true);

    assert_eq!(code, 0, "{said:?}");
    assert_eq!(std::fs::read_to_string(&exclude).unwrap(), theirs);
    assert!(
        said.iter()
            .any(|l| l.starts_with("• Removed purlis's generated-layer block from")),
        "{said:?}"
    );
}

#[test]
fn an_exclude_block_with_a_line_purlis_cannot_account_for_is_left_whole_and_named() {
    // Leaving the block is the safe direction: a worktree git cannot list (its record has
    // no `gitdir`) may be a checkout still needing every line.
    let dir = plane();
    let outside = tempfile::tempdir().unwrap();
    let (exclude, _) = beta_reading_an_exclude_elsewhere(dir.path(), outside.path());
    let before = std::fs::read_to_string(&exclude).unwrap();
    let stray = outside.path().join("main/.git/worktrees/stray");
    std::fs::create_dir_all(&stray).unwrap();

    let (code, said) = run(dir.path(), "beta", true);

    assert_eq!(code, 0, "{said:?}");
    assert_eq!(std::fs::read_to_string(&exclude).unwrap(), before);
    assert!(
        said.iter()
            .any(|l| l.contains("leaves purlis's generated-layer block")
                && l.contains("could not account for every line")
                && l.contains("worktree repair")),
        "{said:?}"
    );
}
