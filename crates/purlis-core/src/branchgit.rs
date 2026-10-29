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
        if !writes_the_index(sub, post) {
            continue;
        }
        let mut env = before.clone();
        env.extend(call.env.iter().cloned());
        if projectgit::repository(&here, &pre, &env)
            .is_some_and(|repo| purlis_made(&repo, Path::new(root)))
        {
            return Some((sub.clone(), said(sub)));
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
/// `workspaces/<ws>/.worktrees/<repo>/<piece>`.
fn purlis_made(repo: &Path, root: &Path) -> bool {
    if !a_worktree_record(repo) {
        return false;
    }
    let Ok(pointer) = std::fs::read_to_string(repo.join("gitdir")) else {
        return false;
    };
    let Some(folder) = Path::new(pointer.trim()).parent() else {
        return false;
    };
    let (Ok(folder), Ok(workspaces)) = (
        std::fs::canonicalize(folder),
        std::fs::canonicalize(root.join("workspaces")),
    ) else {
        return false;
    };
    let Ok(within) = folder.strip_prefix(&workspaces) else {
        return false;
    };
    let parts: Vec<Component<'_>> = within.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(_), Component::Normal(worktrees), Component::Normal(_), Component::Normal(_)]
            if *worktrees == WORKTREES
    )
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
mod tests {
    use super::*;

    /// A project with a clone in a workspace and two linked worktrees of it: a branch folder
    /// purlis made (`workspaces/ide/.worktrees/purlis/guard-port`) and one the person made with
    /// `git worktree add` somewhere else. Each folder's `.git` is a file naming its record in
    /// the clone's `.git/worktrees`, and each record's `gitdir` names the folder's `.git` back.
    struct Layout {
        _dir: tempfile::TempDir,
        root: PathBuf,
        clone: PathBuf,
        folder: PathBuf,
        theirs: PathBuf,
    }

    impl Layout {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(dir.path()).unwrap();
            let clone = root.join("workspaces/ide/purlis");
            let folder = root.join("workspaces/ide/.worktrees/purlis/guard-port");
            let theirs = root.join("workspaces/ide/scratch/side");
            for (name, at) in [("guard-port", &folder), ("side", &theirs)] {
                let record = clone.join(".git/worktrees").join(name);
                std::fs::create_dir_all(&record).unwrap();
                std::fs::create_dir_all(at.join("src")).unwrap();
                std::fs::write(at.join(".git"), format!("gitdir: {}\n", record.display())).unwrap();
                std::fs::write(
                    record.join("gitdir"),
                    format!("{}\n", at.join(".git").display()),
                )
                .unwrap();
            }
            Self {
                _dir: dir,
                root,
                clone,
                folder,
                theirs,
            }
        }

        fn ask(&self, cmd: &str, cwd: &Path) -> Option<(String, String)> {
            refusal(
                cmd,
                &cwd.display().to_string(),
                &self.root.display().to_string(),
            )
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
            ("git commit -n -m x", "commit"),
        ] {
            let (said_sub, why) = l
                .ask(cmd, &l.folder)
                .unwrap_or_else(|| panic!("{cmd:?} was let through"));
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
        let cmd = format!("cd {} && git commit -m x", l.folder.display());
        assert!(l.ask(&cmd, &l.clone).is_some(), "{cmd}");
        let named = format!("git -C {} add src/a.rs", l.folder.display());
        assert!(l.ask(&named, Path::new("/")).is_some(), "{named}");
        // And out of it again: the clone's own index is not a branch folder's record.
        let back = format!("cd {} && git commit -m x", l.clone.display());
        assert_eq!(l.ask(&back, &l.folder), None, "{back}");
    }

    /// The review of #3841: a `cd` in a subshell or a pipeline does not move the shell the next
    /// command runs in, so it is never carried over to it.
    #[test]
    fn a_cd_in_a_subshell_or_a_pipeline_is_not_carried_over() {
        let l = Layout::new();
        let folder = l.folder.display();
        for cmd in [
            format!("(cd {folder} && ls); git commit -m x"),
            format!("cd {folder} | cat; git commit -m x"),
            format!("echo $(cd {folder}); git commit -m x"),
        ] {
            assert_eq!(l.ask(&cmd, &l.clone), None, "{cmd}");
        }
    }

    #[test]
    fn read_only_git_dry_runs_and_git_in_the_clone_itself_are_let_through() {
        let l = Layout::new();
        for cmd in [
            "git status",
            "git diff",
            "git log -1",
            "git commit --dry-run -m x",
            "git add -n .",
            "git add --dry-run src/a.rs",
            "ls",
            "purlis worktree commit -m x --all",
        ] {
            assert_eq!(l.ask(cmd, &l.folder), None, "{cmd}");
        }
        assert_eq!(l.ask("git commit -m x", &l.clone), None);
    }

    /// The review of #3841: a worktree the person made with `git worktree add` is not a branch
    /// folder purlis made, and `purlis worktree commit` would not commit there.
    #[test]
    fn a_worktree_the_person_made_is_left_to_the_sandbox() {
        let l = Layout::new();
        assert_eq!(l.ask("git commit -m x", &l.theirs), None);
        assert_eq!(l.ask("git add src/a.rs", &l.theirs), None);
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
            assert_eq!(l.ask(cmd, &l.clone), None, "{cmd}");
        }
    }
}
