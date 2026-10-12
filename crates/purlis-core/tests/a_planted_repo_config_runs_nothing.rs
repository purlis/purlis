// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A repository whose own config names programs for git to run: no git purlis runs there
//! starts any of them (#810, ADR 0084's rule for every git call).
//!
//! A chat can write the repository it works in, its config included where no sandbox holds it,
//! and purlis runs git there from outside any sandbox. So the fixture plants, in the
//! repository's own config, a program for every way config makes git start one on the verbs
//! purlis runs: a hooks folder (`core.hooksPath`) holding every hook those verbs fire, an
//! fsmonitor (`core.fsmonitor`), a pager (`core.pager`, `pager.<verb>`), and aliases under the
//! verbs' own names. Each program only appends its name to a mark file. Then every verb is run
//! through the runner (`purlis_core::worktree::git`), each entry point of it, and the mark file
//! must not exist.
//!
//! **The control** runs plain git on the same fixture and sees the hooks and the fsmonitor
//! run, so a plant that had stopped working cannot pass for a runner that holds. A pager and a
//! shadowing alias have no control: git starts a pager only for a terminal, which the runner
//! never gives it, and never takes an alias under a verb of its own. They are planted so that
//! a runner that one day did either is caught.
//!
//! Two layouts, because the runner reads them differently: a clone, whose `.git` is a folder,
//! and a work tree whose git directory is kept beside it (`--separate-git-dir`), which the
//! runner names to git itself (#1055). The linked worktree the run adds is a third, read the
//! second way.
//!
//! Unix only: the planted programs are shell scripts.
#![cfg(unix)]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use purlis_core::worktree::git;

/// Every hook a verb the runner is asked here fires: `worktree add` (post-checkout), `merge`
/// (post-merge), any ref it moves (reference-transaction), an index it writes
/// (post-index-change), and the rest a writer's verb would reach, so that a verb added to the
/// run below is covered without touching this list.
const HOOKS: [&str; 12] = [
    "post-checkout",
    "post-merge",
    "reference-transaction",
    "post-index-change",
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-merge-commit",
    "pre-rebase",
    "post-rewrite",
    "pre-push",
];

/// The verbs the runner is asked below, each planted as an alias of the same name.
const VERBS: [&str; 7] = [
    "status",
    "log",
    "diff",
    "worktree",
    "merge",
    "fetch",
    "check-attr",
];

/// A repository with config that names programs, and where they say they ran.
struct Planted {
    _dir: tempfile::TempDir,
    case: PathBuf,
    /// The work tree the runner is asked in.
    work: PathBuf,
    /// A bare repository with `main` in it, to fetch from.
    upstream: PathBuf,
    /// The file each planted program appends its name to.
    ran: PathBuf,
}

#[derive(Clone, Copy)]
enum Layout {
    /// `.git` is a folder in the work tree.
    Clone,
    /// `.git` is a file naming a git directory kept beside the work tree.
    KeptBeside,
}

/// A program at `case/<name>` that appends `name` to the mark file and exits 0.
fn mark(case: &Path, ran: &Path, name: &str) -> PathBuf {
    let program = case.join(name);
    std::fs::write(
        &program,
        format!("#!/bin/sh\necho '{name}' >> '{}'\nexit 0\n", ran.display()),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

fn planted(layout: Layout) -> Planted {
    let dir = tempfile::tempdir().unwrap();
    let case = std::fs::canonicalize(dir.path()).unwrap();
    let work = case.join("work");
    match layout {
        Layout::Clone => {
            std::fs::create_dir_all(&work).unwrap();
            support::git(&work, &["init", "-q", "-b", "main", "."]);
        }
        Layout::KeptBeside => {
            let store = case.join("store");
            support::git(
                &case,
                &[
                    "init",
                    "-q",
                    "-b",
                    "main",
                    &format!("--separate-git-dir={}", store.display()),
                    "work",
                ],
            );
        }
    }
    std::fs::write(work.join("a.txt"), "one\n").unwrap();
    support::git(&work, &["add", "a.txt"]);
    support::git(&work, &["commit", "-q", "-m", "one"]);
    support::git(&work, &["checkout", "-q", "-b", "ahead"]);
    std::fs::write(work.join("a.txt"), "two\n").unwrap();
    support::git(&work, &["commit", "-q", "-am", "two"]);
    support::git(&work, &["checkout", "-q", "main"]);
    let upstream = case.join("upstream");
    support::git(&case, &["init", "-q", "--bare", "upstream"]);
    support::git(
        &work,
        &["push", "-q", &upstream.display().to_string(), "ahead"],
    );
    std::fs::write(
        work.join("a.txt"),
        "changed, so status and diff have work to do\n",
    )
    .unwrap();

    // Planted last, so the fixture's own git above ran none of it.
    let ran = case.join("ran");
    let hooks = case.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for hook in HOOKS {
        mark(&hooks, &ran, hook);
    }
    let fsmonitor = mark(&case, &ran, "fsmonitor");
    let pager = mark(&case, &ran, "pager");
    let alias = mark(&case, &ran, "alias");
    let mut config = vec![
        ("core.hooksPath".to_owned(), hooks.display().to_string()),
        ("core.fsmonitor".to_owned(), fsmonitor.display().to_string()),
        ("core.pager".to_owned(), pager.display().to_string()),
    ];
    for verb in VERBS {
        config.push((format!("pager.{verb}"), pager.display().to_string()));
        config.push((format!("alias.{verb}"), format!("!{}", alias.display())));
    }
    for (key, value) in &config {
        support::git(&work, &["config", key, value]);
    }
    assert!(!ran.exists(), "planting ran something");
    Planted {
        _dir: dir,
        case,
        work,
        upstream,
        ran,
    }
}

/// What the planted programs said they ran, in order.
fn ran(planted: &Planted) -> Vec<String> {
    std::fs::read_to_string(&planted.ran)
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Every verb, through every entry point of the runner a test can reach: what purlis does in
/// a repository, from a read to a linked worktree it adds and a merge it makes.
fn run_everything(planted: &Planted) {
    let work = planted.work.as_path();
    let piece = planted.case.join("piece");
    let piece_arg = piece.display().to_string();
    let upstream = planted.upstream.display().to_string();
    let answered = |what: &str, run: Result<git::Run, git::GitUnavailable>| {
        let run = run.unwrap_or_else(|e| panic!("{what}: git did not start: {e}"));
        assert!(run.ok(), "{what}: {run:?}");
    };

    answered(
        "status",
        git::run(work, &["status", "--porcelain"], git::READ),
    );
    answered(
        "log",
        git::run(work, &["log", "-1", "--oneline"], git::READ),
    );
    answered("diff", git::run(work, &["diff"], git::READ));
    answered(
        "worktree add",
        git::run_untimed(work, &["worktree", "add", "-q", "-b", "piece", &piece_arg]),
    );
    answered(
        "status in the added worktree",
        git::run(&piece, &["status", "--porcelain"], git::READ),
    );
    answered(
        "merge",
        git::run_untimed(&piece, &["merge", "-q", "--ff-only", "ahead"]),
    );
    answered(
        "fetch",
        git::run_network(
            work,
            None,
            &[
                "fetch",
                "-q",
                &upstream,
                "ahead:refs/remotes/upstream/ahead",
            ],
        ),
    );
    let attrs = git::run_with_input(
        work,
        &["check-attr", "-z", "--stdin", "--all"],
        b"a.txt\0".to_vec(),
        git::READ,
    )
    .expect("git starts");
    assert_eq!(
        attrs.code,
        Some(0),
        "check-attr: {}",
        String::from_utf8_lossy(&attrs.err)
    );
    for (what, raw) in [
        (
            "status as the session",
            git::run_as_session(work, &["status", "--porcelain"], git::READ),
        ),
        (
            "status from a hook",
            git::run_in_hook(work, &["status", "--porcelain"], git::READ),
        ),
    ] {
        let raw = raw.unwrap_or_else(|e| panic!("{what}: git did not start: {e}"));
        assert_eq!(
            raw.code,
            Some(0),
            "{what}: {}",
            String::from_utf8_lossy(&raw.err)
        );
    }
}

fn nothing_runs_in(layout: Layout) {
    let planted = planted(layout);

    run_everything(&planted);

    assert_eq!(
        ran(&planted),
        Vec::<String>::new(),
        "a program the repository's config names was run by purlis's git"
    );
}

#[test]
fn nothing_a_clones_config_names_runs_on_any_git_purlis_runs_there() {
    purlis_core::unsteered!();
    nothing_runs_in(Layout::Clone);
}

#[test]
fn nothing_the_config_of_a_git_directory_kept_beside_its_work_tree_names_runs() {
    purlis_core::unsteered!();
    nothing_runs_in(Layout::KeptBeside);
}

/// The control: git that is not the runner, on the same fixture, runs the fsmonitor and the
/// hooks. Without it a plant git had stopped reading would pass for a runner that holds.
fn the_plant_is_live_in(layout: Layout) {
    let planted = planted(layout);
    let plain = |dir: &Path, args: &[&str]| {
        let mut cmd = support::unsigned();
        cmd.arg("-C").arg(dir).args(args);
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        purlis_core::forklock::output(&mut cmd).expect("git runs")
    };
    let piece = planted.case.join("piece").display().to_string();

    plain(&planted.work, &["status", "--porcelain"]);
    plain(
        &planted.work,
        &["worktree", "add", "-q", "-b", "piece", &piece],
    );

    let said = ran(&planted);
    for expected in ["fsmonitor", "post-checkout"] {
        assert!(
            said.iter().any(|name| name == expected),
            "plain git did not run the planted {expected}: {said:?}"
        );
    }
}

#[test]
fn the_control_plain_git_runs_what_a_clones_config_names() {
    purlis_core::unsteered!();
    the_plant_is_live_in(Layout::Clone);
}

#[test]
fn the_control_plain_git_runs_what_a_kept_beside_git_directorys_config_names() {
    purlis_core::unsteered!();
    the_plant_is_live_in(Layout::KeptBeside);
}
