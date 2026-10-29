// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Bash guard's arm for `git add` and `git commit` in a branch folder, from a sandboxed chat
//! (#3830): refused before it runs, with the command that commits there.
//!
//! A branch folder is a linked worktree, and git keeps its index in the clone's git directory,
//! under `.git/worktrees/<name>`. Every sandboxed chat's sandbox denies writing there, at any
//! depth ([`crate::sandbox::PLANTED`]), so `git add` and `git commit` in a branch folder stop
//! on `Operation not permitted`, and the block raised a sandbox Notice for the person, though
//! the fix is the chat's own: `purlis worktree commit`, which the app runs outside the sandbox
//! on the folder's branch (#1476). **When an agent can fix its own call, it is refused with the
//! shape to retry, and never stops on the person** (the operator's ruling of 2026-10-29).
//!
//! **A mistake guard, not the boundary**: the sandbox refuses the write either way. So it
//! speaks only where it is sure: the chat was given a sandbox (the caller asks), and the one
//! repository git will use is a branch folder's record, read as [`crate::projectgit`] reads it.
//! A `cd` to a literal folder on the line is followed; any other move it cannot place (an
//! expansion, `cd -`, `popd`, a wrapper's own chdir) leaves the line to the sandbox.

use std::path::{Component, Path, PathBuf};

use crate::shellwrap::{self, base_lower};
use crate::{projectgit, shellseg};

/// The trace reason this arm is tallied under.
pub const REASON: &str = "branch-folder-git";

/// What the chat is told, for `git <sub>`: the shape to retry, in a call of its own.
pub fn said(sub: &str) -> String {
    format!(
        "`git {sub}` in a branch folder writes the folder's git record in its clone's .git \
         (`.git/worktrees/<name>`), which this chat's sandbox does not let it write, so it \
         would stop on Operation not permitted. Commit through the app, from inside the \
         folder and in a call of its own: `purlis worktree commit -m \"<message>\" <path>…`, \
         naming the files to commit, or `--all` in place of the paths for every change to a \
         file git already tracks. The app makes the commit on the folder's branch, outside the \
         sandbox, and asks nobody. Read-only git (`git status`, `git diff`, `git log`) works \
         here."
    )
}

/// The refusal for `cmd`, run in `cwd` by a chat the app gave a sandbox: `(subcommand,
/// sentence)`, or `None`.
pub fn refusal(cmd: &str, cwd: &str) -> Option<(String, String)> {
    if !shellwrap::prefilter_text(cmd).contains("git") {
        return None;
    }
    let segments = shellseg::joined_argv(cmd)?;
    let argvs: Vec<Vec<String>> = segments.iter().map(|s| s.argv.clone()).collect();
    let exported = shellwrap::exported_env(&argvs);
    let mut here = PathBuf::from(cwd);
    for (segment, before) in segments.iter().zip(exported.iter()) {
        let call = shellwrap::split_env_chdir(&segment.argv);
        let program = base_lower(&call.prog);
        match program.as_str() {
            // A move to a literal folder is followed; anything else is out of sight.
            "cd" | "pushd" => match call.argv.get(1) {
                Some(to) if !projectgit::unplaceable(to) && to != "-" && !to.starts_with('-') => {
                    here = here.join(to);
                    continue;
                }
                _ => return None,
            },
            "popd" => return None,
            _ => {}
        }
        if program != "git" || !call.chdir.is_empty() {
            continue;
        }
        let (pre, rest) = shellwrap::git_globals(&call.argv);
        let Some((sub, post)) = rest.split_first() else {
            continue;
        };
        if !writes_the_index(sub, post) {
            continue;
        }
        let mut env = before.clone();
        env.extend(call.env.iter().cloned());
        if projectgit::repository(&here, &pre, &env).is_some_and(|repo| a_branch_folders(&repo)) {
            return Some((sub.clone(), said(sub)));
        }
    }
    None
}

/// Whether `git <sub> <post…>` writes the index: `add`, and `commit` but for its dry run.
fn writes_the_index(sub: &str, post: &[String]) -> bool {
    match sub {
        "add" => true,
        "commit" => !post.iter().any(|arg| arg == "--dry-run"),
        _ => false,
    }
}

/// Whether the git directory `repo` is a linked worktree's record in its clone's git directory
/// (`.git/worktrees/<name>`).
fn a_branch_folders(repo: &Path) -> bool {
    let parts: Vec<&std::ffi::OsStr> = repo
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    parts
        .windows(3)
        .any(|three| three[0] == ".git" && three[1] == "worktrees")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clone with one branch folder beside it, as `purlis worktree add` lays one out: the
    /// folder's `.git` is a file naming its record in the clone's `.git/worktrees`.
    struct Layout {
        _dir: tempfile::TempDir,
        clone: PathBuf,
        folder: PathBuf,
    }

    impl Layout {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(dir.path()).unwrap();
            let clone = base.join("workspaces/ide/purlis");
            let record = clone.join(".git/worktrees/guard-port");
            std::fs::create_dir_all(&record).unwrap();
            let folder = base.join("workspaces/ide/pieces/guard-port");
            std::fs::create_dir_all(folder.join("src")).unwrap();
            std::fs::write(
                folder.join(".git"),
                format!("gitdir: {}\n", record.display()),
            )
            .unwrap();
            Self {
                _dir: dir,
                clone,
                folder,
            }
        }

        fn folder(&self) -> String {
            self.folder.display().to_string()
        }

        fn clone(&self) -> String {
            self.clone.display().to_string()
        }
    }

    #[test]
    fn git_add_and_commit_in_a_branch_folder_are_refused_with_the_command_that_commits_there() {
        let l = Layout::new();
        for (cmd, sub) in [
            ("git add src/a.rs", "add"),
            ("git add -A", "add"),
            ("git commit -m 'port the guard'", "commit"),
            ("git add . && git commit -m x", "add"),
            ("git -C src commit -am x", "commit"),
            ("GIT_PAGER=cat git commit -m x", "commit"),
        ] {
            let (said_sub, why) =
                refusal(cmd, &l.folder()).unwrap_or_else(|| panic!("{cmd:?} was let through"));
            assert_eq!(said_sub, sub, "{cmd:?}");
            assert!(why.contains(&format!("`git {sub}`")), "{why}");
            assert!(
                why.contains("`purlis worktree commit -m \"<message>\" <path>…`"),
                "{why}"
            );
            assert!(why.contains("--all"), "{why}");
            assert!(why.contains("in a call of its own"), "{why}");
        }
    }

    #[test]
    fn a_literal_cd_into_the_branch_folder_is_followed() {
        let l = Layout::new();
        let cmd = format!("cd {} && git commit -m x", l.folder());
        assert!(refusal(&cmd, &l.clone()).is_some(), "{cmd}");
        let named = format!("git -C {} add src/a.rs", l.folder());
        assert!(refusal(&named, "/").is_some(), "{named}");
        // And out of it again: the clone's own index is not a branch folder's record.
        let back = format!("cd {} && git commit -m x", l.clone());
        assert_eq!(refusal(&back, &l.folder()), None, "{back}");
    }

    #[test]
    fn read_only_git_and_git_in_the_clone_itself_are_let_through() {
        let l = Layout::new();
        for cmd in [
            "git status",
            "git diff",
            "git log -1",
            "git commit --dry-run -m x",
            "ls",
            "purlis worktree commit -m x --all",
        ] {
            assert_eq!(refusal(cmd, &l.folder()), None, "{cmd}");
        }
        assert_eq!(refusal("git commit -m x", &l.clone()), None);
    }

    #[test]
    fn a_move_it_cannot_place_is_left_to_the_sandbox() {
        let l = Layout::new();
        for cmd in [
            "cd $PIECE && git commit -m x",
            "cd - && git commit -m x",
            "popd && git commit -m x",
            "git -C $PIECE commit -m x",
        ] {
            assert_eq!(refusal(cmd, &l.clone()), None, "{cmd}");
        }
    }
}
