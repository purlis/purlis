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
//! A conflict stops the replay with nothing made: the caller says it, and the window's Resolve
//! settles it in a resolve folder ([`super::resolvefolder`]), never in a stopped rebase at the root.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{UNWRITTEN, git, gpgsign, is_object_name, line_of, signer_said, tail};

/// How a replay ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Replayed {
    /// Every commit is made: the last one, to move onto (`onto` itself where none was left).
    Onto(String),
    /// A commit's change conflicts with what it is replayed onto, in these files. Nothing moved.
    Conflict(Vec<String>),
    /// Asked to sign, git could not write a commit: the signer's words.
    Unsigned(String),
    /// Still running at the deadline.
    OutOfTime,
    /// git could not do it, in its words. Nothing moved.
    Failed(String),
}

/// **Replay `head`'s own commits onto `onto`**, off to the side, giving up at `deadline`. `head`
/// and `onto` are anything git reads as a commit.
pub(super) fn replay(
    root: &Path,
    head: &str,
    onto: &str,
    sign: bool,
    deadline: Duration,
) -> Replayed {
    let started = Instant::now();
    let Some(onto_id) = commit_id(root, onto) else {
        return Replayed::Failed(format!("{onto} is not a commit here"));
    };
    let picks = match picked(root, head, &onto_id) {
        Ok(picks) => picks,
        Err(said) => return Replayed::Failed(said),
    };
    let mut at = onto_id;
    for pick in picks {
        if started.elapsed() >= deadline {
            return Replayed::OutOfTime;
        }
        let parent = match line_of(root, &["rev-parse", "--verify", "-q", &format!("{pick}^")]) {
            Some(parent) => parent,
            None => return Replayed::Failed(format!("{pick} has no parent to replay it from")),
        };
        let tree = match merged(root, &parent, &at, &pick) {
            Ok(Merged::Tree(tree)) => tree,
            Ok(Merged::Conflict(files)) => return Replayed::Conflict(files),
            Err(said) => return Replayed::Failed(said),
        };
        // Nothing left to change: dropped, as a rebase drops it.
        if line_of(root, &["rev-parse", &format!("{at}^{{tree}}")]).as_deref() == Some(&tree) {
            continue;
        }
        at = match written(root, &pick, &tree, &at, sign, deadline) {
            Ok(commit) => commit,
            Err(ended) => return ended,
        };
    }
    Replayed::Onto(at)
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

/// `pick`'s change from `parent`, merged onto `at`.
fn merged(root: &Path, parent: &str, at: &str, pick: &str) -> Result<Merged, String> {
    let base = format!("--merge-base={parent}");
    let run = git::run(
        root,
        &[
            "merge-tree",
            "--write-tree",
            "-z",
            "--name-only",
            "--no-messages",
            &base,
            at,
            pick,
        ],
        super::WRITE,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    // 0 is clean and 1 is conflicts: anything else is git refusing, with no tree.
    let mut fields = run.out.split('\0').filter(|field| !field.is_empty());
    match run.code {
        Some(0) => fields
            .next()
            .filter(|tree| is_object_name(tree))
            .map(|tree| Merged::Tree(tree.to_owned()))
            .ok_or_else(|| tail(&run)),
        Some(1) => {
            let _tree = fields.next();
            let mut files: Vec<String> = fields.map(str::to_owned).collect();
            files.sort();
            files.dedup();
            Ok(Merged::Conflict(files))
        }
        _ => Err(tail(&run)),
    }
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
) -> Result<String, Replayed> {
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
    .ok_or_else(|| Replayed::Failed(format!("{pick} could not be read")))?;
    let mut fields = read.out.splitn(4, '\0');
    let (Some(name), Some(email), Some(date), Some(message)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(Replayed::Failed(format!("{pick} could not be read")));
    };
    let mut args = vec!["-c", gpgsign(sign), "commit-tree", tree, "-p", parent];
    if sign {
        args.push("-S");
    }
    let message = message.trim_end();
    args.extend(["-m", message]);
    let made = git::run_with_vars(
        root,
        &[
            ("GIT_AUTHOR_NAME", name),
            ("GIT_AUTHOR_EMAIL", email),
            ("GIT_AUTHOR_DATE", date),
        ],
        &args,
        deadline,
    );
    match made {
        Ok(run) if run.ok() => {
            let id = run.line().trim().to_owned();
            if is_object_name(&id) {
                Ok(id)
            } else {
                Err(Replayed::Failed(tail(&run)))
            }
        }
        Ok(run) if run.code.is_none() => Err(Replayed::OutOfTime),
        Ok(run) if sign && run.err.contains(UNWRITTEN) => {
            Err(Replayed::Unsigned(signer_said(&run.err)))
        }
        Ok(run) => Err(Replayed::Failed(tail(&run))),
        Err(unavailable) => Err(Replayed::Failed(unavailable.to_string())),
    }
}

#[cfg(test)]
mod tests;
