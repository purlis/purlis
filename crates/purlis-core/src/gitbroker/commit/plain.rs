// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis worktree commit` where nothing holds the chat** (#4534): a chat with no sandbox,
//! or a terminal, commits by itself, in its own process.
//!
//! A commit the app makes runs no hooks, signs nothing and reads none of the person's git
//! config, because a sandboxed chat must not reach any of them through the app. None of that
//! protects anything here: this process can already run them. So git runs as `git add` and
//! `git commit` would in this chat, with the person's config, the repository's hooks and
//! purlis's own (the scan, the trailers), its signer and its filters, and with no deadline
//! but the hooks' own. Git says why it stopped, on this process's own output.
//!
//! It keeps the shape of the command: one branch folder purlis cut, on a branch, a message of
//! its own that claims no provenance. It never amends, resets, rebases, merges or pushes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{FolderName, NOT_PLACED, Say, Stage, named_folder, standing, through_no_link};
use crate::counts::counted;
use crate::worktree::{self, git};

/// The branch folder a plain commit is made in, and what is staged there.
#[derive(Debug)]
pub struct Placed {
    /// The folder by its names, as the one-writer claim takes it.
    pub at: worktree::Located,
    top: PathBuf,
    stage: Stage,
}

/// Where a commit asked of a process standing at `cwd` is made: the branch folder `folder`
/// names, or the one it stands in, or the one below it its paths name, as for a brokered one.
pub fn placed(
    root: &Path,
    cwd: &Path,
    folder: Option<&FolderName>,
    stage: &Stage,
) -> Result<Placed, String> {
    let (top, stage) = match folder {
        Some(named) => named_folder(root, cwd, named, stage)?,
        None => {
            if let Stage::Paths(paths) = stage {
                for path in paths {
                    through_no_link(cwd, path)?;
                }
            }
            standing(root, cwd, stage)?
        }
    };
    let at = worktree::locate(root, &top).ok_or(NOT_PLACED)?;
    Ok(Placed { at, top, stage })
}

/// Stages what `placed` holds and commits it with `message`, and answers the new commit, or
/// why not. Named paths are committed alone, and anything else already staged stays staged;
/// `--all` commits the whole index. What was staged stays staged where the commit stops, as
/// with git itself.
pub fn commit(placed: &Placed, message: &str) -> Result<Vec<Say>, String> {
    made(&placed.top, &placed.stage, message)
}

/// [`commit`], of `stage` in the branch folder at `top`.
fn made(top: &Path, stage: &Stage, message: &str) -> Result<Vec<Say>, String> {
    super::trailers_ok(message)?;
    // Before any git is chosen: on a Mac without developer tools `/usr/bin/git` is a stub that
    // opens the system's install dialog (#1813).
    git::presence::require().map_err(|why| why.to_string())?;
    let branch = read(top, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok_or(
        "this folder is not on a branch (a detached HEAD), so there is no branch to commit on",
    )?;
    let after = match stage {
        Stage::Tracked => tracked(top, message)?,
        Stage::Paths(paths) => alone(top, paths, message)?,
    };
    let id = read(top, &["rev-parse", "HEAD"]).unwrap_or_default();
    let files = read(top, &["show", "--format=", "--name-only", "HEAD"])
        .unwrap_or_default()
        .lines()
        .count();
    let done = Say::Done(
        format!(
            "committed {id} on {branch}: {}",
            counted(files, "file", "files")
        )
        .into(),
    );
    Ok(std::iter::once(done).chain(after).collect())
}

/// Where git reads the named paths: its standard input, each literal and ended by a NUL, so
/// that no number of them meets the system's limit on a command's arguments.
const NAMES_ON_STDIN: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];

const NOTHING: &str =
    "there is nothing to commit: no named path differs from the branch's last commit";

const NOT_MADE: &str = "git did not make the commit, as it says above: what was named stays staged";

/// `--all`: every change to a tracked file is staged, and the whole index is committed.
fn tracked(top: &Path, message: &str) -> Result<Option<Say>, String> {
    if !ran(top, &["add", "--update"], None, Stdio::inherit())? {
        return Err("git did not stage that, as it says above: nothing was committed".to_owned());
    }
    if ran(
        top,
        &["diff", "--cached", "--quiet"],
        None,
        Stdio::inherit(),
    )? {
        return Err(NOTHING.to_owned());
    }
    let commit = ["commit", "--quiet", "--cleanup=whitespace", "--file=-"];
    if !ran(
        top,
        &commit,
        Some(message.as_bytes().to_vec()),
        Stdio::inherit(),
    )? {
        return Err(NOT_MADE.to_owned());
    }
    Ok(None)
}

/// Named paths are committed alone, as `git commit -- <paths>` does: what else was staged
/// before stays staged and out of the commit (#7928).
fn alone(top: &Path, paths: &[String], message: &str) -> Result<Option<Say>, String> {
    // Git refuses a commit of some paths while it is concluding something, and only once they
    // are staged: say so first, and how to conclude it.
    concludes_nothing(top)?;
    let names = paths.join("\0").into_bytes();
    let with_names = |args: &[&'static str]| -> Vec<&'static str> {
        [&["--literal-pathspecs"][..], args, &NAMES_ON_STDIN].concat()
    };
    let staging = with_names(&["add", "--all"]);
    if !ran(top, &staging, Some(names.clone()), Stdio::inherit())? {
        return Err("git did not stage that, as it says above: nothing was committed".to_owned());
    }
    // A dry run of the same commit says whether there is one to make. It runs no hook and
    // leaves the index as it was.
    let dry = with_names(&["commit", "--dry-run", "--short"]);
    if !ran(top, &dry, Some(names.clone()), Stdio::null())? {
        return Err(NOTHING.to_owned());
    }
    // The names are on standard input, so the message is read from a file of its own.
    let mut file = tempfile::NamedTempFile::new().map_err(|e| {
        format!("the message could not be written for git ({e}): nothing was committed")
    })?;
    file.write_all(message.as_bytes()).map_err(|e| {
        format!("the message could not be written for git ({e}): nothing was committed")
    })?;
    let message_at = format!("--file={}", file.path().display());
    let commit = [
        &with_names(&["commit", "--quiet", "--cleanup=whitespace"])[..],
        &[message_at.as_str()],
    ]
    .concat();
    if !ran(top, &commit, Some(names.clone()), Stdio::inherit())? {
        return Err(NOT_MADE.to_owned());
    }
    // A hook that restages a named path puts its content in the commit but leaves the folder's
    // index as it was before the hook: the named paths are staged again as they were committed.
    let restage = with_names(&["reset", "--quiet"]);
    if ran(top, &restage, Some(names), Stdio::inherit()).unwrap_or(false) {
        return Ok(None);
    }
    Ok(Some(Say::Warn(
        "git could not stage the named paths as they were committed, as it says above: \
         `git status` may show them as changed"
            .into(),
    )))
}

/// Why a commit of named paths cannot be made in the folder at `top` now: it is in the middle
/// of a merge, a cherry-pick or a revert, which only a commit of the whole index concludes.
fn concludes_nothing(top: &Path) -> Result<(), String> {
    let git_dir = read(top, &["rev-parse", "--absolute-git-dir"])
        .ok_or("git could not say where this folder's repository is: nothing was committed")?;
    for (entry, what) in [
        ("MERGE_HEAD", "a merge"),
        ("CHERRY_PICK_HEAD", "a cherry-pick"),
        ("REVERT_HEAD", "a revert"),
    ] {
        if std::fs::symlink_metadata(Path::new(&git_dir).join(entry)).is_ok() {
            return Err(format!(
                "this folder is in the middle of {what}, which a commit of named paths cannot \
                 conclude: commit with --all to conclude it, or finish or abort it first. \
                 Nothing was committed"
            ));
        }
    }
    Ok(())
}

/// Git in the branch folder at `top`, in this process's own environment.
fn at(top: &Path) -> Command {
    let (binary, _) = git::git_binary();
    let mut git = Command::new(binary);
    git.current_dir(top);
    git
}

/// What git printed for `args`, trimmed, where it succeeded with something to say.
fn read(top: &Path, args: &[&str]) -> Option<String> {
    let out = crate::forklock::output(at(top).args(args).stderr(Stdio::null())).ok()?;
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !said.is_empty()).then_some(said)
}

/// Whether `args` succeeded, its standard output `stdout`, its errors this process's own and
/// `input` on its standard input.
fn ran(top: &Path, args: &[&str], input: Option<Vec<u8>>, stdout: Stdio) -> Result<bool, String> {
    let mut git = at(top);
    git.args(args).stdout(stdout).stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let could_not = |e: std::io::Error| format!("git could not run: {e}");
    let mut child = crate::forklock::spawn(&mut git).map_err(could_not)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(&input).map_err(could_not)?;
    }
    Ok(child.wait().map_err(could_not)?.success())
}

#[cfg(test)]
mod tests;
