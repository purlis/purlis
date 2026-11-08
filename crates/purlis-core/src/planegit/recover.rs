// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A root git step purlis left part-way, recovered** (#5824 F3, D3).
//!
//! # The marker
//!
//! Before a git step at a plane root that could be cut off part-way, purlis writes
//! [`MARKER`] in the repository's git directory ([`Marker`]): the step, the branch, the commit
//! the root was at, where it was going and from what base. It removes the marker when the step
//! returns, however the step ended: a step that stopped on a conflict for the person to settle
//! is still a step that ended. **Only a step that never returned leaves it**, killed by a quit, a
//! crash or sleep, and its process's hold on the save lock went with it ([`super::Claim`]).
//!
//! # Recovery, off to the side
//!
//! A look or a save of the window that holds the plane's claim and finds a stop with the marker
//! recovers it without touching anything a person or a chat wrote since:
//!
//! 1. What the root holds now, uncommitted, is snapshot into a commit of its own, through an
//!    index of its own (`add -A`, as a save stages it, so `.gitignore` holds). A file git
//!    calls unmerged, or one holding conflict markers, is never snapshot: the recovery is
//!    refused, naming it ([`Recovered::Unsettled`]).
//! 2. Where the step was going is worked out off to the side: the root's own commits since the
//!    base carried onto the target as one commit ([`super::bringin::plan`], so what purlis
//!    generates is rebuilt, never stitched).
//! 3. What was written since the stop is merged onto that, with the stop's HEAD as the base: a
//!    3-way merge whose base is the stop, ours where the step was going, theirs the snapshot.
//!    The stop's HEAD is where the rebase last moved HEAD, read from HEAD's reflog, so a commit
//!    made on the detached HEAD since is carried with the snapshot, never taken for the base.
//! 4. The stop is put away (`rebase --quit`, or the merge's files), the root's HEAD is set to
//!    the snapshot with a mixed reset (nothing in the work tree moves), then onto the result with
//!    `reset --keep`, which writes only the files the snapshot does not already have as the
//!    result has them, and the branch is pointed at it, only from where the step found it.
//!
//! A conflict in what a person wrote is said, naming the files, and nothing moves. A stop
//! without the marker is the person's: it is said, and nothing changes. Under a chat's sandbox
//! the recovery is not started ([`super::sandbox_would_stop`]): the window's own look makes it.
//! A refusal is kept beside the marker ([`SAID`]), so the standing the window shows says it
//! whichever look found it ([`said`]).
//!
//! # The one index lock purlis removes
//!
//! purlis never removes an index lock ([`crate::gitstate`]), but for one: a step purlis marked
//! was cut off inside a `reset`, the lock is older than [`crate::gitstate::STALE_AFTER`], and no
//! git process works in the root. The marker says purlis was the writer, the claim says no step
//! of purlis's runs, and the process list says nothing else does. Any other lock is named, and
//! left.
//!
//! # A stop from before the marker
//!
//! A stop from a purlis older than the marker (#5824 D5's one-off cover, for the plane a quit
//! left stuck on 2026-11-07) is recognised by its whole shape, so a person's own `git rebase -i`
//! is never taken over: a rebase on a detached HEAD, of the plane's branch, still where it
//! started (`orig-head` is the branch's tip), onto origin's target branch exactly, cut off
//! between two picks (no `patch`, `stopped-sha` or `message`: it did not stop for a person), and
//! left for a minute or more.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{git, git_dir_of, is_object_name, line_of, rebase_in_progress, tail};

/// The file in a plane's git directory that says a root git step is under way.
pub const MARKER: &str = "purlis-op.json";

/// The file beside [`MARKER`] that says why the last recovery of its stop was refused.
pub const SAID: &str = "purlis-op-said.txt";

/// How long a rebase left with no marker has to have been still before it is read as one purlis
/// started and was cut off: a rebase still running is never touched.
const STILL_FOR: Duration = Duration::from_secs(60);

/// A step purlis runs at a root, as the marker names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    /// The root's own commits replayed onto a target.
    Rebase,
    /// A target merged into the root.
    Merge,
}

/// What [`MARKER`] holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Marker {
    op: Op,
    /// `refs/heads/<branch>`.
    branch: String,
    /// Where the branch was before the step.
    orig_head: String,
    /// Where it was going.
    onto: String,
    /// Where the root's own commits start.
    base: String,
    /// HEAD as the stop left it, written once a recovery starts moving the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stop_head: Option<String>,
    /// The commit that recovery points the branch at, written with `stop_head`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    pid: u32,
}

impl Marker {
    fn read(git_dir: &Path) -> Option<Marker> {
        let text = std::fs::read_to_string(git_dir.join(MARKER)).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write(&self, git_dir: &Path) -> Option<()> {
        let path = git_dir.join(MARKER);
        let held = path.with_extension("json.tmp");
        let text = serde_json::to_string(self).ok()?;
        std::fs::write(&held, format!("{text}\n")).ok()?;
        std::fs::rename(&held, path).ok()
    }
}

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
pub(super) fn mark(root: &Path, op: Op, onto: &str, base: &str) -> Option<Marked> {
    let marker = Marker {
        op,
        branch: line_of(root, &["symbolic-ref", "-q", "HEAD"])?,
        orig_head: head(root)?,
        onto: onto.to_owned(),
        base: base.to_owned(),
        stop_head: None,
        result: None,
        pid: std::process::id(),
    };
    let git_dir = git_dir_of(root);
    marker.write(&git_dir)?;
    Some(Marked {
        path: git_dir.join(MARKER),
    })
}

fn head(root: &Path) -> Option<String> {
    line_of(root, &["rev-parse", "--verify", "-q", "HEAD^{commit}"])
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
    /// Read from the marker, not from the stop's shape alone.
    marked: bool,
    /// Where a recovery cut off before this one pointed the branch, if it got that far.
    result: Option<String>,
}

/// What [`recover`] came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovered {
    /// Nothing is stopped, or nothing purlis started.
    Nothing,
    /// A stop nobody marked: the person's own, left as it is.
    Persons(String),
    /// The root is on its branch again, at `commit`, with everything in it. `lock_removed` is
    /// the index lock the cut-off step left, where this removed it.
    Recovered {
        commit: String,
        lock_removed: Option<PathBuf>,
    },
    /// What was written conflicts with where the step was going, in these files a person wrote.
    /// Nothing moved.
    Conflict(Vec<String>),
    /// These files hold a conflict git stopped on: unmerged, or with its markers in them. A
    /// snapshot would commit the markers, so nothing moved.
    Unsettled(Vec<String>),
    /// Not recovered from here, and why: the window's own look does it.
    Waiting(String),
    /// git could not do it, in its words. What it did not finish is marked still, so the next
    /// look tries again.
    Failed(String),
}

impl Recovered {
    /// The refusal as the standing says it, one claim: `None` where nothing was refused.
    fn refusal(&self) -> Option<String> {
        match self {
            Self::Conflict(files) => Some(format!(
                "a step purlis left part-way conflicts with what was written since, in {}",
                files.join(", ")
            )),
            Self::Unsettled(files) => Some(format!(
                "a step purlis left part-way stopped on a conflict still unsettled in {}",
                files.join(", ")
            )),
            Self::Failed(why) => Some(format!("a step purlis left part-way: {why}")),
            Self::Nothing | Self::Persons(_) | Self::Recovered { .. } | Self::Waiting(_) => None,
        }
    }
}

/// Whether the root at `root` has a stop purlis marked, or one of its stops from before the
/// marker: what [`recover`] would recover. Reads only.
pub(super) fn is_ours(root: &Path) -> bool {
    stop_of(root).is_some()
}

/// Remove a marker its step left behind once it went through: nothing is stopped, so it marks
/// nothing. With it goes what was said about it.
pub(super) fn drop_spent_marker(root: &Path) {
    let git_dir = git_dir_of(root);
    if git_dir.join(MARKER).exists() && !stopped(root, &git_dir) {
        forget(&git_dir);
    }
}

/// Why the last recovery of the stop at `root` was refused, while that stop is still there.
pub fn said(root: &Path) -> Option<String> {
    let git_dir = git_dir_of(root);
    if !stopped(root, &git_dir) {
        return None;
    }
    std::fs::read_to_string(git_dir.join(SAID))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn forget(git_dir: &Path) {
    let _ = std::fs::remove_file(git_dir.join(MARKER));
    let _ = std::fs::remove_file(git_dir.join(SAID));
}

/// Whether git is stopped at `root`: a rebase, a merge, or a detached HEAD.
fn stopped(root: &Path, git_dir: &Path) -> bool {
    rebase_in_progress(root)
        || merge_head(git_dir).is_some()
        || line_of(root, &["symbolic-ref", "-q", "HEAD"]).is_none()
}

fn merge_head(git_dir: &Path) -> Option<String> {
    std::fs::read_to_string(git_dir.join("MERGE_HEAD"))
        .ok()
        .and_then(|text| text.lines().next().map(str::to_owned))
        .filter(|id| is_object_name(id))
}

fn rebase_dir(git_dir: &Path) -> Option<PathBuf> {
    ["rebase-merge", "rebase-apply"]
        .into_iter()
        .map(|dir| git_dir.join(dir))
        .find(|dir| dir.is_dir())
}

/// The stop at `root` purlis left, if there is one. Reads only.
fn stop_of(root: &Path) -> Option<Stop> {
    let git_dir = git_dir_of(root);
    let head_now = head(root)?;
    let Some(marker) = Marker::read(&git_dir) else {
        return pre_marker_stop(root, &rebase_dir(&git_dir)?, head_now);
    };
    if !stopped(root, &git_dir) {
        return None;
    }
    let named = |id: &str| is_object_name(id).then(|| id.to_owned());
    let merge_head = merge_head(&git_dir);
    let merge = merge_head.is_some() || marker.op == Op::Merge;
    let onto = merge_head.or_else(|| named(&marker.onto))?;
    // Where a recovery that did not finish moved HEAD from: the stop it began at. Otherwise
    // where the rebase last moved HEAD, never a commit made on it since.
    let stop_head = match marker.stop_head.as_deref().and_then(named) {
        Some(kept) => kept,
        None if !merge => where_the_rebase_left_head(root, &onto).unwrap_or(head_now),
        None => head_now,
    };
    Some(Stop {
        branch: Some(marker.branch).filter(|b| b.starts_with("refs/heads/"))?,
        orig_head: named(&marker.orig_head)?,
        onto,
        base: named(&marker.base)?,
        stop_head,
        merge,
        marked: true,
        result: marker.result.as_deref().and_then(named),
    })
}

/// The newest commit HEAD's reflog says a rebase moved it to, on top of `onto`: where the stop
/// left HEAD, whatever was committed on it since. `None` where the reflog does not say.
fn where_the_rebase_left_head(root: &Path, onto: &str) -> Option<String> {
    let log = git::run(
        root,
        &["reflog", "show", "-n", "200", "--format=%H %gs", "HEAD"],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)?;
    log.out
        .lines()
        .filter_map(|line| line.split_once(' '))
        // `rebase (pick): …`, or `rebase -i (pick): …` from a git before 2.26.
        .find(|(_, said)| said.starts_with("rebase (") || said.starts_with("rebase -i ("))
        .map(|(id, _)| id.to_owned())
        .filter(|id| {
            git::run(root, &["merge-base", "--is-ancestor", onto, id], git::READ)
                .is_ok_and(|run| run.ok())
        })
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
    // A rebase always detaches HEAD; one on a branch is not a rebase purlis ran.
    if line_of(root, &["symbolic-ref", "-q", "HEAD"]).is_some() {
        return None;
    }
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file))
            .ok()
            .map(|text| text.trim().to_owned())
    };
    let target = crate::planesave::Settings::read(root)
        .plane
        .branch
        .value
        .or_else(|| {
            line_of(
                root,
                &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"],
            )
            .and_then(|name| name.strip_prefix("origin/").map(str::to_owned))
        })?;
    let branch = format!("refs/heads/{target}");
    if read("head-name").as_deref() != Some(branch.as_str()) {
        return None;
    }
    // Onto origin's target exactly, as purlis's save rebased: a person's rebase onto an older
    // commit of it is theirs.
    let onto = read("onto").filter(|id| is_object_name(id))?;
    let tracking = format!("refs/remotes/origin/{target}");
    if line_of(root, &["rev-parse", "--verify", "-q", &tracking]).as_deref() != Some(&*onto) {
        return None;
    }
    // The branch still where the rebase found it.
    let orig_head = read("orig-head").filter(|id| is_object_name(id))?;
    if line_of(root, &["rev-parse", "--verify", "-q", &branch]).as_deref() != Some(&*orig_head) {
        return None;
    }
    // The first commit it picked has the base as its parent.
    let first = read("done")
        .and_then(|done| {
            done.lines()
                .find_map(|line| line.strip_prefix("pick ").map(str::to_owned))
        })
        .and_then(|rest| rest.split_whitespace().next().map(str::to_owned));
    let base = first
        .and_then(|pick| line_of(root, &["rev-parse", "--verify", "-q", &format!("{pick}^")]))
        .or_else(|| line_of(root, &["merge-base", &orig_head, &onto]))?;
    let stop_head = where_the_rebase_left_head(root, &onto).unwrap_or(stop_head);
    Some(Stop {
        branch,
        orig_head,
        onto,
        base,
        stop_head,
        merge: false,
        marked: false,
        result: None,
    })
}

/// **Recover the stop purlis left at `root`**, off to the side. The caller holds the plane's
/// claim, so no step of purlis's is running: the stop is one that was cut off. A refusal is
/// kept for the standing ([`said`]).
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
    let git_dir = git_dir_of(root);
    let got = match recovered(root, &git_dir, &stop, sign) {
        Ok(Ok((commit, lock_removed))) => Recovered::Recovered {
            commit,
            lock_removed,
        },
        Ok(Err(early)) => early,
        Err(why) => Recovered::Failed(why),
    };
    match (&got, got.refusal()) {
        (Recovered::Recovered { .. }, _) => forget(&git_dir),
        (_, Some(said)) => {
            let _ = std::fs::write(git_dir.join(SAID), format!("{said}\n"));
        }
        (_, None) => {}
    }
    got
}

/// What [`recovered`] answers once the root is on its branch: the commit, and the lock it
/// removed.
type Done = (String, Option<PathBuf>);

/// The steps of [`recover`]: `Ok(Ok(..))` once the root is on its branch, `Ok(Err(..))` where it
/// stops before moving anything, `Err` where git failed.
fn recovered(
    root: &Path,
    git_dir: &Path,
    stop: &Stop,
    sign: bool,
) -> Result<Result<Done, Recovered>, String> {
    let target = stop.branch.trim_start_matches("refs/heads/");
    // 0. A lock a step cut off inside left: removed only where it is purlis's, else named.
    let lock_removed = match crate::gitstate::find(git_dir) {
        None => None,
        Some(lock) if stop.marked && lock.age >= crate::gitstate::STALE_AFTER => {
            match git_works_in(root) {
                Ok(false) => {
                    std::fs::remove_file(&lock.path).map_err(|e| {
                        format!("{} could not be removed: {e}", lock.path.display())
                    })?;
                    Some(lock.path)
                }
                Ok(true) => return Err(format!("a git process holds {}", lock.path.display())),
                Err(why) => {
                    return Err(format!("{} is left, since {why}", lock.path.display()));
                }
            }
        }
        Some(lock) => return Err(format!("git's index is locked: {}", lock.describe())),
    };
    // 1. What the root holds now, never a conflict git stopped on.
    let unmerged = crate::gitstate::unmerged(root);
    if !unmerged.is_empty() {
        return Ok(Err(Recovered::Unsettled(unmerged)));
    }
    let parent = head(root).ok_or("HEAD names no commit")?;
    let snapshot = snapshot(root, &parent)?;
    let marked = with_markers(root, &stop.stop_head, &snapshot)?;
    if !marked.is_empty() {
        return Ok(Err(Recovered::Unsettled(marked)));
    }
    // 2. Where the step was going.
    let base = (!stop.merge).then_some(stop.base.as_str());
    let going = super::bringin::plan_between(root, &stop.orig_head, &stop.onto, base)?;
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
            "a chat's sandbox stops it: {}",
            super::said_sandbox_stops(&stops)
        ))));
    }
    // The branch only from where the step found it, or where a recovery cut off pointed it.
    let tip = line_of(root, &["rev-parse", "--verify", "-q", &stop.branch])
        .ok_or_else(|| format!("{} names no commit", stop.branch))?;
    if tip != stop.orig_head && Some(&tip) != stop.result.as_ref() {
        return Err(format!(
            "{} moved to {} since the step began, so it is left there",
            stop.branch,
            &tip[..tip.len().min(7)]
        ));
    }
    // From here the root moves: the stop's HEAD is kept in the marker, so a recovery cut off
    // in turn starts again from the same stop.
    Marker {
        op: if stop.merge { Op::Merge } else { Op::Rebase },
        branch: stop.branch.clone(),
        orig_head: stop.orig_head.clone(),
        onto: stop.onto.clone(),
        base: stop.base.clone(),
        stop_head: Some(stop.stop_head.clone()),
        result: Some(result.clone()),
        pid: std::process::id(),
    }
    .write(git_dir)
    .ok_or("the marker could not be written")?;
    // 4. The stop put away, nothing in the work tree touched.
    if rebase_in_progress(root) {
        step(root, &["rebase", "--quit"])?;
    }
    for file in ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "AUTO_MERGE"] {
        let _ = std::fs::remove_file(git_dir.join(file));
    }
    step(root, &["reset", "-q", "--mixed", &snapshot])?;
    // Only what the snapshot does not already hold as the result has it is written.
    step(root, &["reset", "-q", "--keep", &result])?;
    step(root, &["update-ref", &stop.branch, &result, &tip])?;
    step(root, &["symbolic-ref", "HEAD", &stop.branch])?;
    Ok(Ok((result, lock_removed)))
}

/// The files `snapshot` changes from `from` that hold a conflict marker.
fn with_markers(root: &Path, from: &str, snapshot: &str) -> Result<Vec<String>, String> {
    let changed = git::run(
        root,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--name-only",
            "--diff-filter=d",
            from,
            snapshot,
        ],
        git::READ,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if !changed.ok() {
        return Err(tail(&changed));
    }
    Ok(changed
        .out
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter(|path| {
            std::fs::read(root.join(path)).is_ok_and(|text| super::resolve::markers_in(&text))
        })
        .map(str::to_owned)
        .collect())
}

/// Whether a git process works in `root` (its folder is at or under it), by the process list.
/// `Err` where the list could not be read: then nobody can say no git holds the lock.
#[cfg(target_os = "linux")]
fn git_works_in(root: &Path) -> Result<bool, String> {
    let real = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let entries =
        std::fs::read_dir("/proc").map_err(|e| format!("/proc could not be read: {e}"))?;
    Ok(entries.flatten().any(|entry| {
        let is_git = std::fs::read_to_string(entry.path().join("comm"))
            .is_ok_and(|comm| comm.trim() == "git" || comm.trim().starts_with("git-"));
        is_git
            && std::fs::read_link(entry.path().join("cwd"))
                .is_ok_and(|cwd| cwd.starts_with(&real) || cwd.starts_with(root))
    }))
}

/// Whether a git process works in `root`, by `lsof`'s list of the folders git processes work in.
#[cfg(target_os = "macos")]
fn git_works_in(root: &Path) -> Result<bool, String> {
    let real = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut lsof = std::process::Command::new("/usr/sbin/lsof");
    // `-b` and `-w`: no kernel call that can block, and no warnings on its output.
    lsof.args(["-b", "-w", "-nP", "-a", "-c", "git", "-d", "cwd", "-F", "n"]);
    let (status, listing) = crate::restartends::within(&mut lsof, Duration::from_secs(5))
        .ok_or("the process list could not be read in time")?;
    match status {
        // 1 with nothing listed: no process matched.
        Some(1) if listing.trim().is_empty() => Ok(false),
        Some(0) => Ok(listing
            .lines()
            .filter_map(|line| line.strip_prefix('n'))
            .any(|cwd| Path::new(cwd).starts_with(&real) || Path::new(cwd).starts_with(root))),
        _ => Err("the process list could not be read".to_owned()),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn git_works_in(_root: &Path) -> Result<bool, String> {
    Err("this system's process list is not read".to_owned())
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
