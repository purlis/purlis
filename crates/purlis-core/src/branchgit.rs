// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Bash guard's arm for `git add` and `git commit` in a branch folder purlis made, from a
//! sandboxed chat (#3830): refused before it runs, with the command that commits there.
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
//! speaks only where it is sure:
//!
//! - the chat was given a sandbox (the caller asks);
//! - the one repository git will use, read as [`crate::projectgit`] reads it, is the record of a
//!   branch folder **purlis made**: git's own record of the folder (its `gitdir` file) names a
//!   folder at `workspaces/<ws>/.worktrees/<repo>/<piece>` in the project. A worktree the person
//!   made with `git worktree add` anywhere else is left to the sandbox, since `purlis worktree
//!   commit` would not commit there (the review of #3841);
//! - and the line can be placed: a `cd` to a literal folder is followed only on a line whose
//!   commands all run in one shell, one after another (`&&`, `;`). A `cd` in a subshell, a
//!   pipeline or a substitution, or any move it cannot place (an expansion, `cd -`, `popd`, a
//!   wrapper's own chdir), leaves the line to the sandbox.

use std::path::{Component, Path, PathBuf};

use crate::shellwrap::{self, base_lower};
use crate::{projectgit, shellseg};

/// The trace reason this arm is tallied under.
pub const REASON: &str = "branch-folder-git";

/// The folder, inside a workspace, that holds the branch folders purlis makes
/// (`workspaces/<ws>/.worktrees/<repo>/<piece>`), as [`crate::active::workspace_of_tree`]
/// reads it.
const WORKTREES: &str = ".worktrees";

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

/// The refusal for `cmd`, run in `cwd` by a chat the app gave a sandbox, in the project at
/// `root`: `(subcommand, sentence)`, or `None`.
pub fn refusal(cmd: &str, cwd: &str, root: &str) -> Option<(String, String)> {
    let ((sub, ()), _) = found(cmd, cwd, root, |sub, post| {
        writes_the_index(sub, post).then_some(())
    })?;
    let why = said(&sub);
    Some((sub, why))
}

/// **Plain git that writes a branch folder's branch** (#3931): the first `git commit`, `merge`,
/// `cherry-pick`, `revert` or `am` (a commit), `pull` or `rebase` (an update) or `push` on the
/// line `cmd`, run in `cwd`, whose repository is a branch folder purlis made in the project at
/// `root`, read and placed as [`refusal`] reads and places a line: `(subcommand, act, folder)`,
/// where `folder` is the folder's top. What an unsandboxed chat's guard holds to the folder's
/// one live writer ([`crate::piececlaim`]). A dry run, and a rebase, merge, cherry-pick, revert
/// or am that only stops or shows one under way (`--abort`, `--quit`,
/// `--show-current-patch`), writes nothing of its own and is let through.
pub fn branch_write(
    cmd: &str,
    cwd: &str,
    root: &str,
) -> Option<(String, crate::piececlaim::Act, PathBuf)> {
    let ((sub, act), folder) = found(cmd, cwd, root, writes_the_branch)?;
    Some((sub, act, folder))
}

/// What `git <sub> <post…>` does to the branch it runs on, as a branch folder's claim names it
/// ([`branch_write`]), or `None` for git that makes no commit and moves no branch.
fn writes_the_branch(sub: &str, post: &[String]) -> Option<crate::piececlaim::Act> {
    use crate::piececlaim::Act;
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    let only_stops = has("--abort") || has("--quit") || has("--show-current-patch");
    match sub {
        "commit" if !has("--dry-run") => Some(Act::Commit),
        "merge" | "cherry-pick" | "revert" | "am" if !only_stops => Some(Act::Commit),
        "rebase" if !only_stops => Some(Act::Update),
        "pull" if !has("--dry-run") => Some(Act::Update),
        "push" if !(has("-n") || has("--dry-run")) => Some(Act::Push),
        _ => None,
    }
}

/// The first `git` on the line `cmd`, run in `cwd`, that `wanted` names a `T` for and whose
/// repository is the record of a branch folder purlis made in the project at `root`:
/// `((subcommand, T), the folder's top)`, or `None`.
fn found<T>(
    cmd: &str,
    cwd: &str,
    root: &str,
    wanted: impl Fn(&str, &[String]) -> Option<T>,
) -> Option<((String, T), PathBuf)> {
    if !shellwrap::prefilter_text(cmd).contains("git") {
        return None;
    }
    let segments = shellseg::joined_argv(cmd)?;
    // A `cd` carries over to the next command only where both run in this shell, in turn.
    let in_turn = |op: &Option<String>| {
        op.as_deref()
            .is_none_or(|op| matches!(op, "&&" | ";" | "\n"))
    };
    let one_shell = segments
        .iter()
        .all(|segment| in_turn(&segment.before) && in_turn(&segment.after));
    let argvs: Vec<Vec<String>> = segments.iter().map(|s| s.argv.clone()).collect();
    let exported = shellwrap::exported_env(&argvs);
    let mut here = PathBuf::from(cwd);
    for (segment, before) in segments.iter().zip(exported.iter()) {
        let call = shellwrap::split_env_chdir(&segment.argv);
        let program = base_lower(&call.prog);
        match program.as_str() {
            // A move to a literal folder, on a line run in one shell, is followed; anything
            // else is out of sight.
            "cd" | "pushd" => match call.argv.get(1) {
                Some(to)
                    if one_shell
                        && !projectgit::unplaceable(to)
                        && to != "-"
                        && !to.starts_with('-') =>
                {
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
        let Some(what) = wanted(sub, post) else {
            continue;
        };
        let mut env = before.clone();
        env.extend(call.env.iter().cloned());
        if let Some(folder) = projectgit::repository(&here, &pre, &env)
            .and_then(|repo| purlis_made(&repo, Path::new(root)))
        {
            return Some(((sub.clone(), what), folder));
        }
    }
    None
}

/// Whether `git <sub> <post…>` writes the index: `add` and `commit`, but for their dry runs
/// (`add -n`, `add --dry-run`, `commit --dry-run`; a commit's `-n` is `--no-verify`, which
/// writes).
fn writes_the_index(sub: &str, post: &[String]) -> bool {
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    match sub {
        "add" => !(has("-n") || has("--dry-run")),
        "commit" => !has("--dry-run"),
        _ => false,
    }
}

/// Whether the git directory `repo` is the record of a branch folder purlis made in the
/// project at `root`: a linked worktree's record (`.git/worktrees/<name>`), whose `gitdir`
/// file (git's own pointer back to the folder's `.git`) names a folder at
/// `workspaces/<ws>/.worktrees/<repo>/<piece>`: that folder, or `None`.
fn purlis_made(repo: &Path, root: &Path) -> Option<PathBuf> {
    if !a_worktree_record(repo) {
        return None;
    }
    let pointer = std::fs::read_to_string(repo.join("gitdir")).ok()?;
    let folder = std::fs::canonicalize(Path::new(pointer.trim()).parent()?).ok()?;
    let workspaces = std::fs::canonicalize(root.join("workspaces")).ok()?;
    let within = folder.strip_prefix(&workspaces).ok()?;
    let parts: Vec<Component<'_>> = within.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(_), Component::Normal(worktrees), Component::Normal(_), Component::Normal(_)]
            if *worktrees == WORKTREES
    )
    .then_some(folder)
}

/// Whether the git directory `repo` is a linked worktree's record in its clone's git directory
/// (`.git/worktrees/<name>`).
fn a_worktree_record(repo: &Path) -> bool {
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
mod tests;
