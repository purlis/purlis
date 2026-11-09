// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A line of commits replayed off to the side** (#5855, #5824 D2): what `git rebase` does, made
//! without touching the work tree, the index or any ref.
//!
//! Each commit is merged onto the one before with `git merge-tree --write-tree`, whose merge base
//! is the commit's own parent, and written with `git commit-tree`, keeping its author, its date
//! and its message, and signed exactly as the plane's saves are. Nothing moves until the whole
//! line is made: the caller then moves the root onto the last commit with one `reset --keep`. A
//! quit, a crash or sleep in the middle leaves objects nothing names, and the root as it was.
//!
//! The commits replayed are the ones `git rebase <onto>` would pick: not merges, and not one whose
//! change `onto` already has (`rev-list --cherry-pick`). A commit with nothing left to change once
//! replayed is dropped, as git's own rebase drops it.
//!
//! Each commit is merged as a save's merge is ([`bringin::plan_between`], #5864): what purlis
//! generates is rebuilt from its sources, never merged by lines, so a replay needs no `merge=union`
//! rule for a memory index. Only a conflict in what a person wrote stops the replay, with nothing
//! made: the caller says it, and the window's Resolve settles it in a resolve folder
//! ([`super::resolvefolder`]), never in a stopped rebase at the root.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{bringin, git, is_object_name, line_of, tail};

/// Why a replay made no line to move onto. Nothing moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Unreplayed {
    /// A commit's change conflicts with what it is replayed onto, in these files.
    Conflict(Vec<String>),
    /// Asked to sign, git could not write a commit: the signer's words.
    Unsigned(String),
    /// Still running at the deadline.
    OutOfTime,
    /// git could not do it, in its words.
    Failed(String),
}

/// **Replay `head`'s own commits onto `onto`**, off to the side, giving up at `deadline`. `head`
/// and `onto` are anything git reads as a commit. `Ok` is the last commit made, to move onto
/// (`onto` itself where none was left).
pub(super) fn replay(
    root: &Path,
    head: &str,
    onto: &str,
    sign: bool,
    deadline: Duration,
) -> Result<String, Unreplayed> {
    let started = Instant::now();
    let Some(onto_id) = commit_id(root, onto) else {
        return Err(Unreplayed::Failed(format!("{onto} is not a commit here")));
    };
    let picks = picked(root, head, &onto_id).map_err(Unreplayed::Failed)?;
    let mut at = onto_id;
    for pick in picks {
        if started.elapsed() >= deadline {
            return Err(Unreplayed::OutOfTime);
        }
        let parent = match line_of(root, &["rev-parse", "--verify", "-q", &format!("{pick}^")]) {
            Some(parent) => parent,
            None => {
                return Err(Unreplayed::Failed(format!(
                    "{pick} has no parent to replay it from"
                )));
            }
        };
        let tree = match merged(root, &parent, &at, &pick).map_err(Unreplayed::Failed)? {
            Merged::Tree(tree) => tree,
            Merged::Conflict(files) => return Err(Unreplayed::Conflict(files)),
        };
        // Nothing left to change: dropped, as a rebase drops it.
        if line_of(root, &["rev-parse", &format!("{at}^{{tree}}")]).as_deref() == Some(&tree) {
            continue;
        }
        at = written(root, &pick, &tree, &at, sign, deadline)?;
    }
    Ok(at)
}

/// The commit `rev` names, or `None`.
fn commit_id(root: &Path, rev: &str) -> Option<String> {
    line_of(
        root,
        &["rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")],
    )
    .filter(|id| is_object_name(id))
}

/// The commits of `head` a rebase onto `onto` picks, oldest first.
fn picked(root: &Path, head: &str, onto: &str) -> Result<Vec<String>, String> {
    let run = git::run(
        root,
        &[
            "rev-list",
            "--reverse",
            "--no-merges",
            "--right-only",
            "--cherry-pick",
            &format!("{onto}...{head}"),
            "--",
        ],
        git::READ,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if !run.ok() {
        return Err(tail(&run));
    }
    Ok(run
        .out
        .lines()
        .map(str::trim)
        .filter(|id| is_object_name(id))
        .map(str::to_owned)
        .collect())
}

/// What one commit's merge came to.
enum Merged {
    Tree(String),
    Conflict(Vec<String>),
}

/// `pick`'s change from `parent`, merged onto `at`, as a save's merge makes it
/// ([`bringin::plan_between`], #5864): a file purlis generates (a memory store's `MEMORY.md`, a
/// `sessions/index.md`, a workspace's `todos/MEMORY.md`) is rebuilt from the sources the merged
/// tree holds, and the settings file is merged key by key, so only what a person wrote can stop
/// the replay.
fn merged(root: &Path, parent: &str, at: &str, pick: &str) -> Result<Merged, String> {
    let plan = bringin::plan_between(root, at, pick, Some(parent))?;
    if !plan.person.is_empty() {
        return Ok(Merged::Conflict(plan.person));
    }
    bringin::tree_of(root, &plan).map(Merged::Tree)
}

/// `pick` written again as a commit of `tree` on `parent`: its author, date and message kept, and
/// signed exactly when the plane's saves are ([`gpgsign`]).
fn written(
    root: &Path,
    pick: &str,
    tree: &str,
    parent: &str,
    sign: bool,
    deadline: Duration,
) -> Result<String, Unreplayed> {
    let read = git::run(
        root,
        &[
            "log",
            "-1",
            "--date=raw",
            "--format=%an%x00%ae%x00%ad%x00%B",
            pick,
            "--",
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .ok_or_else(|| Unreplayed::Failed(format!("{pick} could not be read")))?;
    let mut fields = read.out.splitn(4, '\0');
    let (Some(name), Some(email), Some(date), Some(message)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(Unreplayed::Failed(format!("{pick} could not be read")));
    };
    let author = bringin::Author { name, email, date };
    bringin::commit_tree_as(
        root,
        tree,
        &[parent],
        message.trim_end(),
        sign,
        Some(&author),
        deadline,
    )
    .map_err(|unmade| match unmade {
        bringin::Unmade::Unsigned(said) => Unreplayed::Unsigned(said),
        bringin::Unmade::OutOfTime => Unreplayed::OutOfTime,
        bringin::Unmade::Failed(said) => Unreplayed::Failed(said),
    })
}

#[cfg(test)]
mod tests;
