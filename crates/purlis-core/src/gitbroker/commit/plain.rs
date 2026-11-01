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
/// why not. What was staged stays staged where the commit stops, as with git itself.
pub fn commit(placed: &Placed, message: &str) -> Result<Vec<Say>, String> {
    let top = &placed.top;
    super::trailers_ok(message)?;
    let branch = read(top, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok_or(
        "this folder is not on a branch (a detached HEAD), so there is no branch to commit on",
    )?;
    let staging = match &placed.stage {
        Stage::Tracked => ran(top, &["add", "--update"], None),
        Stage::Paths(paths) => ran(
            top,
            &[
                "--literal-pathspecs",
                "add",
                "--all",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(paths.join("\0").into_bytes()),
        ),
    };
    if !staging? {
        return Err("git did not stage that, as it says above: nothing was committed".to_owned());
    }
    if ran(top, &["diff", "--cached", "--quiet"], None)? {
        return Err(
            "there is nothing to commit: no named path differs from the branch's last commit"
                .to_owned(),
        );
    }
    let made = ran(
        top,
        &["commit", "--quiet", "--cleanup=whitespace", "--file=-"],
        Some(message.as_bytes().to_vec()),
    )?;
    if !made {
        return Err(
            "git did not make the commit, as it says above: what was named stays staged".to_owned(),
        );
    }
    let id = read(top, &["rev-parse", "HEAD"]).unwrap_or_default();
    let files = read(top, &["show", "--format=", "--name-only", "HEAD"])
        .unwrap_or_default()
        .lines()
        .count();
    Ok(vec![Say::Done(
        format!(
            "committed {id} on {branch}: {}",
            counted(files, "file", "files")
        )
        .into(),
    )])
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
    let out = at(top).args(args).stderr(Stdio::null()).output().ok()?;
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !said.is_empty()).then_some(said)
}

/// Whether `args` succeeded, its output this process's own and `input` on its standard input.
fn ran(top: &Path, args: &[&str], input: Option<Vec<u8>>) -> Result<bool, String> {
    let mut git = at(top);
    git.args(args).stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let could_not = |e: std::io::Error| format!("git could not run: {e}");
    let mut child = git.spawn().map_err(could_not)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(&input).map_err(could_not)?;
    }
    Ok(child.wait().map_err(could_not)?.success())
}
