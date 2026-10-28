// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `workspace remove` and the exclude block it leaves in a repository kept elsewhere (#998).
//!
//! Two workspaces each hold a checkout that is a linked worktree of one repository outside the
//! project, so both read that repository's one `info/exclude`. Removing the first takes out of
//! purlis's block only the lines no checkout purlis still wires there needs, and removing the
//! second takes the block out whole. The operator's own lines around it are never touched.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::guest;
use purlis_core::repocmd::Say;
use purlis_core::wscmd::remove::remove;

/// What purlis's block in `text` lists, its markers and notes left out.
fn block(text: &str) -> Vec<String> {
    let mut inside = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if line == guest::EXCLUDE_BEGIN {
            inside = true;
        } else if line == guest::EXCLUDE_END {
            inside = false;
        } else if inside && !line.starts_with('#') {
            out.push(line.to_owned());
        }
    }
    out
}

/// `workspace remove <ws>`, unforced: its exit code and what it said.
fn removed(plane: &Path, ws: &str) -> (u8, Vec<String>) {
    let mut said = Vec::new();
    let done = remove(plane, ws, false, &mut |line: Say| {
        said.push(line.to_string())
    });
    (done.code, said)
}

/// A linked worktree of `main` at `workspaces/<ws>/guest`, on its own branch.
fn worktree_in(plane: &Path, main: &Path, ws: &str) -> PathBuf {
    let at = plane.join("workspaces").join(ws).join("guest");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    support::git(
        main,
        &["worktree", "add", "-q", "-b", ws, &at.display().to_string()],
    );
    at
}

#[test]
fn removing_one_workspace_takes_only_the_lines_no_other_wired_checkout_still_needs() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("own");
    f.give_the_plane_a_layer();
    let elsewhere = tempfile::tempdir().unwrap();
    let main = std::fs::canonicalize(elsewhere.path())
        .unwrap()
        .join("main");
    std::fs::create_dir_all(&main).unwrap();
    support::git(&main, &["init", "-q", "-b", "main", "."]);
    std::fs::write(main.join("README.md"), "one\n").unwrap();
    support::git(&main, &["add", "README.md"]);
    support::git(&main, &["commit", "-q", "-m", "one"]);
    let exclude = main.join(".git").join("info").join("exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    let theirs = "# the operator's own\n*.log\n/notes.txt\n";
    std::fs::write(&exclude, theirs).unwrap();

    let beta = worktree_in(&f.plane, &main, "beta");
    let gamma = worktree_in(&f.plane, &main, "gamma");
    guest::wire(&f.plane, &beta);
    guest::wire(&f.plane, &gamma);
    let agent = "/.claude/agents/steward.md";
    let wired = block(&std::fs::read_to_string(&exclude).unwrap());
    assert!(wired.iter().any(|l| l == agent), "{wired:?}");
    assert!(
        wired.iter().any(|l| l == "/.claude/settings.json"),
        "{wired:?}"
    );
    // gamma no longer holds the agent, so only beta needs its line.
    std::fs::remove_file(gamma.join(&agent[1..])).unwrap();

    let (code, said) = removed(&f.plane, "beta");

    assert_eq!(code, 0, "{said:?}");
    assert!(!beta.exists());
    let text = std::fs::read_to_string(&exclude).unwrap();
    assert!(
        text.starts_with(theirs),
        "the operator's lines stay first, as they were: {text}"
    );
    let left = block(&text);
    assert!(
        !left.iter().any(|l| l == agent),
        "beta's own line goes: {left:?}"
    );
    let staying: Vec<String> = wired.iter().filter(|l| *l != agent).cloned().collect();
    assert_eq!(left, staying, "every line gamma still has a file for stays");
    assert_eq!(f.status(&gamma), "", "gamma shows nothing purlis wrote");
    assert!(
        said.iter()
            .any(|l| l.contains("lines stay for another checkout")),
        "{said:?}"
    );

    // git's record of beta's folder went with it: nobody prunes by hand, and gamma's stays.
    let listed = worktrees_of(&main);
    assert!(!listed.contains(&beta), "{listed:?}");
    assert!(listed.contains(&gamma), "{listed:?}");
    let (code, said) = removed(&f.plane, "gamma");

    assert_eq!(code, 0, "{said:?}");
    assert_eq!(
        std::fs::read_to_string(&exclude).unwrap(),
        theirs,
        "the last wired checkout gone, the block goes and nothing else moves"
    );
    assert!(
        said.iter()
            .any(|l| l.contains("Removed purlis's generated-layer block")),
        "{said:?}"
    );
    assert!(!worktrees_of(&main).contains(&gamma));
}

/// The folders git lists as `main`'s worktrees, as git recorded them.
fn worktrees_of(main: &Path) -> Vec<PathBuf> {
    let out = support::git(main, &["worktree", "list", "--porcelain"]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)))
        .collect()
}

#[test]
fn a_record_git_will_not_clear_is_named_with_the_prune_that_clears_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("own");
    let elsewhere = tempfile::tempdir().unwrap();
    let main = std::fs::canonicalize(elsewhere.path())
        .unwrap()
        .join("main");
    std::fs::create_dir_all(&main).unwrap();
    support::git(&main, &["init", "-q", "-b", "main", "."]);
    std::fs::write(main.join("README.md"), "one\n").unwrap();
    support::git(&main, &["add", "README.md"]);
    support::git(&main, &["commit", "-q", "-m", "one"]);
    let beta = worktree_in(&f.plane, &main, "beta");
    // A locked worktree is one git refuses to remove without a double force.
    support::git(&main, &["worktree", "lock", &beta.display().to_string()]);

    let (code, said) = removed(&f.plane, "beta");

    assert_eq!(code, 0, "{said:?}");
    assert!(!beta.exists());
    let prune = format!("git -C {} worktree prune", main.display());
    assert!(
        said.iter().any(|l| l.contains(&prune)),
        "one line names the prune that clears the record: {said:?}"
    );
    assert_eq!(
        said.iter().filter(|l| l.contains("worktree prune")).count(),
        1,
        "{said:?}"
    );
}
