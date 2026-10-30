// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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

impl Layout {
    fn write(&self, cmd: &str, cwd: &Path) -> Option<(String, crate::piececlaim::Act)> {
        let (sub, act, folder) = branch_write(
            cmd,
            &cwd.display().to_string(),
            &self.root.display().to_string(),
        )?;
        assert_eq!(folder, self.folder, "{cmd}");
        Some((sub, act))
    }
}

/// #3931: what an unsandboxed chat's guard holds to the folder's one live writer.
#[test]
fn plain_git_that_commits_updates_or_pushes_in_a_branch_folder_is_named_with_its_folder() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    for (cmd, sub, act) in [
        ("git commit -m x", "commit", Act::Commit),
        ("git add . && git commit -am x", "commit", Act::Commit),
        ("git merge origin/main", "merge", Act::Commit),
        ("git cherry-pick abc123", "cherry-pick", Act::Commit),
        ("git revert HEAD", "revert", Act::Commit),
        ("git am < fix.patch", "am", Act::Commit),
        ("git pull --rebase", "pull", Act::Update),
        ("git rebase origin/main", "rebase", Act::Update),
        ("git rebase --continue", "rebase", Act::Update),
        ("git push", "push", Act::Push),
        ("git push -u origin HEAD", "push", Act::Push),
        ("git -C src push", "push", Act::Push),
    ] {
        assert_eq!(
            l.write(cmd, &l.folder),
            Some((sub.to_owned(), act)),
            "{cmd}"
        );
    }
    let cd = format!("cd {} && git push", l.folder.display());
    assert_eq!(l.write(&cd, &l.clone), Some(("push".to_owned(), Act::Push)));
}

#[test]
fn git_that_writes_no_branch_or_runs_outside_a_branch_folder_is_not_named() {
    let l = Layout::new();
    for cmd in [
        "git status",
        "git add src/a.rs",
        "git fetch",
        "git log -1",
        "git commit --dry-run -m x",
        "git push --dry-run",
        "git push -n",
        "git rebase --abort",
        "git merge --abort",
        "git cherry-pick --quit",
        "git am --show-current-patch",
        "purlis worktree push",
    ] {
        assert_eq!(l.write(cmd, &l.folder), None, "{cmd}");
    }
    assert_eq!(l.write("git commit -m x", &l.clone), None);
    assert_eq!(l.write("git push", &l.theirs), None);
}
