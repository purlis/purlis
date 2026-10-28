// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The first-task script (FR-28, #621): the guided task FR-1 measures.
//!
//! W10 puts a new user's first ten minutes on **one task, run twice**: the same task given to
//! two chats on their own repo — two harnesses, or one harness and two models — each on a
//! branch of its own, with both diffs openable. And it asks for one "month-two in minute five"
//! moment. The moment this script has is **a lesson carried across harnesses**: the first run
//! records what it learned about the repo, and the second run, on whichever harness, starts
//! with it in its briefing. That needs nothing the first run did not already set up — no
//! vault, no second machine, no forge — and it is the thing a harness on its own cannot do,
//! because the lesson is kept in the project and every harness is briefed from there
//! ([`crate::briefing`]).
//!
//! This module is the script's one copy, so the CI run (`purlis-cli`'s
//! `tests/it/first_task_script.rs`) and the guide a partner follows in the window
//! (`FirstTaskTab.tsx`, through the app's `first_task_run` command) cannot drift apart:
//!
//! - [`prompt`]: the task, typed into each run's chat and never sent (ADR 0061's rule for a
//!   prompt charter types);
//! - [`label`]: what each run's chat is called, which names its branch;
//! - [`cut_from`] and [`diff_command`]: the commit a chat's branch was cut from, and what shows
//!   the chat's diff against it, in a shell tab in its branch's folder;
//! - [`branch`] and [`cut_from_recorded`]: the same, read back from the clone for a run started
//!   before this launch (#945).
//!
//! **The task writes only in the run's own branch.** Each run's chat starts in a piece cut off
//! the workspace's clone (GL-1), the clone is charter's copy and not the operator's repo
//! (FR-4), and the prompt asks for nothing to be pushed or published.

use std::path::Path;

use crate::worktree::git;

/// How many runs the script has: the same task, twice.
pub const RUNS: u8 = 2;

/// What run `run` (1 or 2) is called: its chat's label, and so its branch's name
/// (`first-task-1`), by the rule every labelled chat's branch is named by.
pub fn label(run: u8) -> String {
    format!("first task {run}")
}

/// The branch run `run` works on: [`label`] as every labelled chat's branch is named
/// (`first-task-1`). What a relaunched window looks for in the clone to know the run started.
pub fn branch(run: u8) -> String {
    crate::worktree::name::slug(&label(run)).unwrap_or_else(|| format!("first-task-{run}"))
}

/// The task, as it is typed into each run's chat.
///
/// Small enough for any repo and any harness, and real: it reads the repo, changes one file,
/// and leaves a fact the next chat can use. The last step is the one the month-two moment
/// rests on, so it names the command a chat records a lesson with, and the persona is the
/// chat's own (`charter persona remember` with one argument resolves it).
///
/// **One line, and short**, because it is typed as one paste and a harness draws a long or
/// many-line paste as a placeholder the operator cannot read before pressing Enter (ADR 0061,
/// amended 2026-09-27; [`crate::harness::Harness::why_drawn_as_a_placeholder`]).
pub fn prompt() -> String {
    "purlis's first task, on this chat's own branch: check the memory in your briefing for \
     what an earlier chat learned about this repo, find out how its tests or checks are run, \
     and write that down in a short \"How to check a change\" section of the README (make one \
     if there is none). Change nothing else, and push or publish nothing. Then record the one \
     thing the next chat here should know with `purlis persona remember \"<the fact>\"`."
        .to_owned()
}

/// The commit the branch in `tree` was cut from: its `HEAD`, asked right after the cut, before
/// the chat has written anything. A commit and not the branch it came from, so a diff against
/// it shows only what the chat did, however far that branch has moved since.
pub fn cut_from(tree: &Path) -> Result<String, String> {
    let said =
        git::run(tree, &["rev-parse", "--verify", "HEAD^{commit}"], git::READ).map_err(|why| {
            format!(
                "purlis could not ask git where {} starts ({why}).",
                tree.display()
            )
        })?;
    let sha = said.line();
    if !said.ok() || sha.is_empty() || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "git did not say which commit {} starts at: {}",
            tree.display(),
            said.err.trim()
        ));
    }
    Ok(sha.to_owned())
}

/// [`cut_from`] for a run started before this launch (#945): where `branch` meets the branch it
/// was cut from, as the clone recorded that at the cut (ADR 0027), asked in `tree`, any folder
/// of the clone. The same commit [`cut_from`] answered at the start, unless the base branch was
/// rewritten or has merged `branch` since; a run's diff is then shorter than what it did, never
/// another branch's.
pub fn cut_from_recorded(tree: &Path, branch: &str) -> Result<String, String> {
    let base = crate::worktree::recorded_base(tree, branch).ok_or_else(|| {
        format!("purlis has no record of which branch {branch} was cut from, so its diff cannot be shown.")
    })?;
    let said = git::run(tree, &["merge-base", branch, &base], git::READ).map_err(|why| {
        format!("purlis could not ask git where {branch} was cut from {base} ({why}).")
    })?;
    let sha = said.line();
    if !said.ok() || sha.is_empty() || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "git did not say where {branch} was cut from {base}: {}",
            said.err.trim()
        ));
    }
    Ok(sha.to_owned())
}

/// The command that shows a chat's diff: everything its branch changed since `commit`, the one
/// [`cut_from`] answered — committed or not, and **files it added that git does not track
/// yet**, which `git diff` alone leaves out. `git add -N` records only that those files exist
/// (intent to add), so the diff shows them whole; nothing is staged, and nothing is
/// committed. Run in the chat's own folder.
///
/// `commit` is quoted as one shell word all the same: the command is typed into a shell, and
/// the quoting costs nothing for a commit id.
pub fn diff_command(commit: &str) -> String {
    format!(
        "git add -N -A && git diff {} --",
        crate::shellseg::shell_quote(commit)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_run_is_labelled_by_its_number() {
        assert_eq!(label(1), "first task 1");
        assert_eq!(label(2), "first task 2");
    }

    #[test]
    fn the_task_asks_for_the_lesson_and_for_nothing_to_leave_the_machine() {
        let task = prompt();
        assert!(
            task.contains("`purlis persona remember \"<the fact>\"`"),
            "{task}"
        );
        assert!(task.contains("push or publish nothing"), "{task}");
    }

    #[test]
    fn the_task_is_drawn_whole_by_every_harness_charter_types_into() {
        let pasted = crate::curation::pasted(&prompt());
        for harness in crate::harness::Harness::ALL {
            if harness.ready_to_type().is_none() {
                continue;
            }
            assert_eq!(
                harness.why_drawn_as_a_placeholder(&pasted),
                None,
                "{harness:?}"
            );
        }
    }

    #[test]
    fn each_run_works_on_the_branch_its_label_names() {
        assert_eq!(branch(1), "first-task-1");
        assert_eq!(branch(2), "first-task-2");
    }

    #[test]
    fn a_run_started_before_this_launch_diffs_against_the_commit_it_was_cut_from() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let git = |args: &[&str]| {
            let said = git::run(repo, args, git::READ).unwrap();
            assert!(said.ok(), "git {args:?}: {}", said.err);
            said.line().to_owned()
        };
        git(&["init", "-q", "-b", "main"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "one",
        ]);
        let cut = git(&["rev-parse", "HEAD"]);
        git(&["branch", "first-task-1"]);
        git(&["config", "branch.first-task-1.charterBase", "main"]);
        // The run commits on its branch, and the base moves on without it.
        git(&["checkout", "-q", "first-task-1"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "run",
        ]);
        git(&["checkout", "-q", "main"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "later",
        ]);
        assert_eq!(cut_from_recorded(repo, "first-task-1"), Ok(cut));
    }

    #[test]
    fn a_branch_with_no_recorded_base_has_no_diff_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let why = cut_from_recorded(dir.path(), "first-task-2").unwrap_err();
        assert!(
            why.contains("no record of which branch first-task-2 was cut from"),
            "{why}"
        );
    }

    #[test]
    fn a_chats_diff_is_against_the_commit_it_was_cut_from_and_shows_new_files() {
        assert_eq!(
            diff_command("1a2b3c4d"),
            "git add -N -A && git diff 1a2b3c4d --"
        );
    }

    #[test]
    fn anything_the_shell_would_read_is_one_quoted_word() {
        assert_eq!(
            diff_command("x$(rm -rf ~);y"),
            "git add -N -A && git diff 'x$(rm -rf ~);y' --"
        );
    }
}
