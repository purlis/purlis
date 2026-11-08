// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A root git step purlis left part-way, recovered** (#5824 F3, D3).
//!
//! # The marker
//!
//! Before a git step at a plane root that could be cut off part-way, purlis writes
//! [`MARKER`] in the repository's git directory: the step, the branch, the commit the root was
//! at, where it was going and from what base. It removes the marker when the step returns,
//! however the step ended: a step that stopped on a conflict for the person to settle is still
//! a step that ended. **Only a step that never returned leaves it**, killed by a quit, a crash
//! or sleep, and its process's hold on the save lock went with it ([`super::Claim`]).
//!
//! # Recovery, off to the side
//!
//! A look or a save of the window that holds the plane's claim and finds a stop with the marker
//! recovers it without touching anything a person or a chat wrote since:
//!
//! 1. What the root holds now, uncommitted, is snapshot into a commit of its own, through an
//!    index of its own (`add -A`, as a save stages it, so `.gitignore` holds).
//! 2. Where the step was going is worked out off to the side: the root's own commits since the
//!    base carried onto the target as one commit ([`super::bringin::plan`], so what purlis
//!    generates is rebuilt, never stitched).
//! 3. What was written since the stop is merged onto that, with the stop's HEAD as the base: a
//!    3-way merge whose base is the stop, ours where the step was going, theirs the snapshot.
//! 4. The stop is put away (`rebase --quit`, or the merge's files), the root's HEAD is set to
//!    the snapshot with a mixed reset (nothing in the work tree moves), then onto the result with
//!    `reset --keep`, which writes only the files the snapshot does not already have as the
//!    result has them, and the branch is pointed at it.
//!
//! A conflict in what a person wrote is said, naming the files, and nothing moves. A stop
//! without the marker is the person's: it is said, and nothing changes. Under a chat's sandbox
//! the recovery is not started ([`super::sandbox_would_stop`]): the window's own look makes it.
//!
//! A stop from a purlis older than the marker is recognised by its shape: a rebase onto a
//! commit of origin's target, of the plane's branch, cut off between two picks (no `patch`,
//! `stopped-sha` or `message`: it did not stop for a person), and left for a minute or more.

use std::path::{Path, PathBuf};

use super::{git, git_dir_of, is_object_name, tail};

/// The file in a plane's git directory that says a root git step is under way.
pub const MARKER: &str = "purlis-op.json";

/// How long a rebase left with no marker has to have been still before it is read as one purlis
/// started and was cut off: a rebase still running is never touched.
const STILL_FOR: std::time::Duration = std::time::Duration::from_secs(60);

/// A step under way at a root, for as long as it is held: the marker is removed when it drops.
#[derive(Debug)]
pub(super) struct Marked {
    path: PathBuf,
}

impl Drop for Marked {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Write the marker for `op` at `root`, going `onto` from `base`. `None` where the root is on
/// no branch or the marker could not be written: the step then runs unmarked, as it did before.
pub(super) fn mark(root: &Path, op: &str, onto: &str, base: &str) -> Option<Marked> {
    let branch = rev(root, &["symbolic-ref", "-q", "HEAD"])?;
    let orig_head = rev(root, &["rev-parse", "--verify", "-q", "HEAD^{commit}"])?;
    let marker = serde_json::json!({
        "op": op,
        "branch": branch,
        "orig_head": orig_head,
        "onto": onto,
        "base": base,
        "pid": std::process::id(),
    });
    let path = git_dir_of(root).join(MARKER);
    write(&path, &marker)?;
    Some(Marked { path })
}

fn write(path: &Path, value: &serde_json::Value) -> Option<()> {
    let held = path.with_extension("json.tmp");
    std::fs::write(&held, format!("{value}\n")).ok()?;
    std::fs::rename(&held, path).ok()
}

/// A git answer's one line, where git answered.
fn rev(root: &Path, args: &[&str]) -> Option<String> {
    git::run(root, args, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|line| !line.is_empty())
}

/// A stop purlis left, and what the step was.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stop {
    /// `refs/heads/<branch>`.
    branch: String,
    /// Where the branch was before the step.
    orig_head: String,
    /// Where it was going.
    onto: String,
    /// Where the root's own commits start: what is carried is `base..orig_head`.
    base: String,
    /// HEAD as the step left it.
    stop_head: String,
    /// The step was a merge: the result has `orig_head` and `onto` as its parents.
    merge: bool,
}

/// What [`recover`] came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// Nothing is stopped, or nothing purlis started.
    Nothing,
    /// A stop nobody marked: the person's own, left as it is.
    Persons(String),
    /// The root is on its branch again, at this commit, with everything in it.
    Recovered(String),
    /// What was written conflicts with where the step was going, in these files a person wrote.
    /// Nothing moved.
    Conflict(Vec<String>),
    /// Not recovered from here, and why: the window's own look does it.
    Waiting(String),
    /// git could not do it, in its words. What it did not finish is marked still, so the next
    /// look tries again.
    Failed(String),
}

/// Whether the root at `root` has a stop purlis marked, or one of its stops from before the
/// marker: what [`recover`] would recover.
pub fn is_ours(root: &Path) -> bool {
    stop_of(root).is_some()
}

/// The stop at `root` purlis left, if there is one.
fn stop_of(root: &Path) -> Option<Stop> {
    let git_dir = git_dir_of(root);
    let rebase = ["rebase-merge", "rebase-apply"]
        .into_iter()
        .map(|dir| git_dir.join(dir))
        .find(|dir| dir.is_dir());
    let merge_head = std::fs::read_to_string(git_dir.join("MERGE_HEAD"))
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
        .filter(|id| is_object_name(id));
    let detached = rev(root, &["symbolic-ref", "-q", "HEAD"]).is_none();
    let stop_head = rev(root, &["rev-parse", "--verify", "-q", "HEAD^{commit}"])?;
    let marker: Option<serde_json::Value> = std::fs::read_to_string(git_dir.join(MARKER))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    if let Some(marker) = marker {
        let field = |name: &str| {
            marker
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let named = |name: &str| field(name).filter(|id| is_object_name(id));
        let branch = field("branch").filter(|b| b.starts_with("refs/heads/"))?;
        let stopped = rebase.is_some() || merge_head.is_some() || detached;
        if !stopped {
            // The step went through, and only the marker was left behind.
            let _ = std::fs::remove_file(git_dir.join(MARKER));
            return None;
        }
        return Some(Stop {
            branch,
            orig_head: named("orig_head")?,
            onto: merge_head.clone().or_else(|| named("onto"))?,
            base: named("base")?,
            // Where a recovery that did not finish moved HEAD from: the stop it began at.
            stop_head: named("stop_head").unwrap_or(stop_head),
            merge: merge_head.is_some() || field("op").as_deref() == Some("merge"),
        });
    }
    pre_marker_stop(root, rebase.as_deref()?, stop_head)
}

/// A rebase a purlis from before the marker left, cut off between two picks.
fn pre_marker_stop(root: &Path, dir: &Path, stop_head: String) -> Option<Stop> {
    // It stopped for a person: theirs to settle.
    if ["patch", "stopped-sha", "message"]
        .iter()
        .any(|file| dir.join(file).exists())
    {
        return None;
    }
    let still = std::fs::metadata(dir)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|since| since >= STILL_FOR);
    if !still {
        return None;
    }
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file))
            .ok()
            .map(|text| text.trim().to_owned())
    };
    let branch = read("head-name").filter(|b| b.starts_with("refs/heads/"))?;
    let name = branch.trim_start_matches("refs/heads/");
    let plane = crate::planesave::Settings::read(root).plane;
    if plane
        .branch
        .value
        .as_deref()
        .is_some_and(|target| target != name)
    {
        return None;
    }
    let onto = read("onto").filter(|id| is_object_name(id))?;
    let tracking = format!("refs/remotes/origin/{name}");
    let on_origin = git::run(
        root,
        &["merge-base", "--is-ancestor", &onto, &tracking],
        git::READ,
    )
    .is_ok_and(|run| run.ok());
    if !on_origin {
        return None;
    }
    let orig_head = read("orig-head").filter(|id| is_object_name(id))?;
    // The first commit it picked has the base as its parent.
    let first = read("done")
        .and_then(|done| {
            done.lines()
                .find_map(|line| line.strip_prefix("pick ").map(str::to_owned))
        })
        .and_then(|rest| rest.split_whitespace().next().map(str::to_owned));
    let base = first
        .and_then(|pick| rev(root, &["rev-parse", "--verify", "-q", &format!("{pick}^")]))
        .or_else(|| rev(root, &["merge-base", &orig_head, &onto]))?;
    Some(Stop {
        branch,
        orig_head,
        onto,
        base,
        stop_head,
        merge: false,
    })
}

/// **Recover the stop purlis left at `root`**, off to the side. The caller holds the plane's
/// claim, so no step of purlis's is running: the stop is one that was cut off.
pub(crate) fn recover(root: &Path, sign: bool) -> Recovered {
    let Some(stop) = stop_of(root) else {
        return match crate::gitstate::stopped(root) {
            Some(stopped) => Recovered::Persons(format!(
                "{}. purlis did not start it, so it is left as it is",
                stopped.why()
            )),
            None => Recovered::Nothing,
        };
    };
    match recovered(root, &stop, sign) {
        Ok(Ok(commit)) => {
            let _ = std::fs::remove_file(git_dir_of(root).join(MARKER));
            Recovered::Recovered(commit)
        }
        Ok(Err(early)) => early,
        Err(why) => Recovered::Failed(why),
    }
}

/// The steps of [`recover`]: `Ok(Ok(commit))` once the root is on its branch at `commit`,
/// `Ok(Err(..))` where it stops before moving anything, `Err` where git failed.
fn recovered(root: &Path, stop: &Stop, sign: bool) -> Result<Result<String, Recovered>, String> {
    let target = stop.branch.trim_start_matches("refs/heads/");
    // 1. What the root holds now.
    let snapshot = snapshot(root, &stop.stop_head)?;
    // 2. Where the step was going.
    let going = if stop.merge {
        super::bringin::plan_between(root, &stop.orig_head, &stop.onto, None)?
    } else {
        super::bringin::plan_between(root, &stop.orig_head, &stop.onto, Some(&stop.base))?
    };
    if !going.person.is_empty() {
        return Ok(Err(Recovered::Conflict(going.person)));
    }
    let parents: Vec<&str> = if stop.merge {
        vec![&stop.orig_head, &stop.onto]
    } else {
        vec![&stop.onto]
    };
    let tree = super::bringin::tree_of(root, &going)?;
    let carried = super::bringin::commit_tree(
        root,
        &tree,
        &parents,
        &format!("Carry this machine's saves onto {target}"),
        sign,
    )?;
    // 3. What was written since the stop, onto that.
    let merged = super::bringin::plan_between(root, &carried, &snapshot, Some(&stop.stop_head))?;
    if !merged.person.is_empty() {
        return Ok(Err(Recovered::Conflict(merged.person)));
    }
    let tree = super::bringin::tree_of(root, &merged)?;
    let result = super::bringin::commit_tree(
        root,
        &tree,
        &parents,
        &format!(
            "Carry this machine's saves onto {target}, recovered from a step purlis left \
             part-way"
        ),
        sign,
    )?;
    let stops = super::sandbox_would_stop(root, &result);
    if !stops.is_empty() {
        return Ok(Err(Recovered::Waiting(format!(
            "a step purlis left part-way is recovered from the window, not from this chat: {}",
            super::said_sandbox_stops(&stops)
        ))));
    }
    // From here the root moves: the stop's HEAD is kept in the marker, so a recovery cut off
    // in turn starts again from the same stop.
    let git_dir = git_dir_of(root);
    let marker = serde_json::json!({
        "op": if stop.merge { "merge" } else { "rebase" },
        "branch": stop.branch,
        "orig_head": stop.orig_head,
        "onto": stop.onto,
        "base": stop.base,
        "stop_head": stop.stop_head,
        "pid": std::process::id(),
    });
    write(&git_dir.join(MARKER), &marker).ok_or("the marker could not be written")?;
    // 4. The stop put away, nothing in the work tree touched.
    if ["rebase-merge", "rebase-apply"]
        .iter()
        .any(|dir| git_dir.join(dir).is_dir())
    {
        step(root, &["rebase", "--quit"])?;
    }
    for file in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "AUTO_MERGE"] {
        let _ = std::fs::remove_file(git_dir.join(file));
    }
    step(root, &["reset", "-q", "--mixed", &snapshot])?;
    // Only what the snapshot does not already hold as the result has it is written.
    step(root, &["reset", "-q", "--keep", &result])?;
    step(root, &["update-ref", &stop.branch, &result])?;
    step(root, &["symbolic-ref", "HEAD", &stop.branch])?;
    Ok(Ok(result))
}

/// One git step at the root, untimed: a step that checks out a tree and is killed leaves it
/// half-written.
fn step(root: &Path, args: &[&str]) -> Result<(), String> {
    match git::run_untimed(root, args) {
        Ok(run) if run.ok() => Ok(()),
        Ok(run) => Err(tail(&run)),
        Err(unavailable) => Err(unavailable.to_string()),
    }
}

/// A commit of what the work tree at `root` holds, on `parent`, staged as a save stages it in an
/// index of its own: the root's own index, with whatever a stop left in it, is never read or
/// written.
fn snapshot(root: &Path, parent: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let index = dir.path().join("index");
    let in_index = |args: &[&str]| -> Result<git::Run, String> {
        let run = git::run_with_index(root, &index, args)
            .map_err(|unavailable| unavailable.to_string())?;
        if run.ok() { Ok(run) } else { Err(tail(&run)) }
    };
    in_index(&["read-tree", parent])?;
    let stages = super::save_stages();
    let stages: Vec<&str> = stages.iter().map(String::as_str).collect();
    in_index(&stages)?;
    let tree = in_index(&["write-tree"])?.line().trim().to_owned();
    super::bringin::commit_tree(
        root,
        &tree,
        &[parent],
        "What the plane held when purlis recovered it",
        false,
    )
}

#[cfg(test)]
mod tests;
