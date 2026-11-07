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
    /// A second branch folder purlis made, `workspaces/ide/.worktrees/purlis/second`.
    second: PathBuf,
    theirs: PathBuf,
}

impl Layout {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let clone = root.join("workspaces/ide/purlis");
        let folder = root.join("workspaces/ide/.worktrees/purlis/guard-port");
        let second = root.join("workspaces/ide/.worktrees/purlis/second");
        let theirs = root.join("workspaces/ide/scratch/side");
        for (name, at) in [
            ("guard-port", &folder),
            ("second", &second),
            ("side", &theirs),
        ] {
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
            second,
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

/// A `cd` that fails leaves the shell where it was, and a command after a `;` or a new line runs
/// all the same: so git there is judged in both folders, the one the `cd` names and the one it
/// left. Only `&&` all the way from the `cd` to the git says the `cd` was made.
#[test]
fn git_after_a_cd_that_may_have_failed_is_judged_in_the_folder_the_shell_was_in_too() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let elsewhere = l.root.join("not-there");
    let elsewhere = elsewhere.display();
    for cmd in [
        format!("cd {elsewhere}; git add -A"),
        format!("cd {elsewhere}\ngit commit -m x"),
        format!("cd {elsewhere} && ls; git commit -m x"),
        format!("cd {elsewhere}; cd {}; git add -A", l.clone.display()),
    ] {
        assert!(l.ask(&cmd, &l.folder).is_some(), "{cmd:?} was let through");
    }
    assert_eq!(
        l.write(&format!("cd {elsewhere}; git push"), &l.folder),
        Some(("push".to_owned(), Act::Push))
    );
    // Run only once the `cd` is made: the git is where the `cd` went.
    let made = format!("cd {elsewhere} && git add -A");
    assert_eq!(l.ask(&made, &l.folder), None, "{made}");
}

/// After a `cd` that may have failed, the git may run in either folder: each is judged, and a
/// branch folder's claim is asked of every one it may be.
#[test]
fn git_that_may_run_in_either_of_two_branch_folders_is_judged_in_both() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let cmd = format!("cd {}; git commit -m x", l.second.display());

    let writes = l.writes(&cmd, &l.folder);

    assert_eq!(
        writes,
        [
            ("commit".to_owned(), Act::Commit, l.second.clone()),
            ("commit".to_owned(), Act::Commit, l.folder.clone()),
        ]
    );
}

/// A long line is read whole where its moves leave the shell in few folders: a chain joined by
/// `&&` alone, which is certain, and the same move made again and again.
#[test]
fn a_long_line_of_moves_that_leave_few_folders_is_read_whole() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let chain = format!(
        "cd {} && cd workspaces && cd ide && cd .worktrees && cd purlis && cd guard-port && \
         cd src && cd .. && cd src && cd .. && cd src && cd .. && git commit -m x",
        l.root.display()
    );
    assert_eq!(
        l.write(&chain, &l.theirs),
        Some(("commit".to_owned(), Act::Commit)),
        "{chain}"
    );
    let again = format!("cd {}; ", l.folder.display()).repeat(12) + "git commit -m x";
    assert_eq!(
        l.write(&again, &l.root),
        Some(("commit".to_owned(), Act::Commit)),
        "{again}"
    );
}

/// A line of many moves that may each have failed leaves the shell in too many folders to
/// follow: a sandboxed chat's is left to the sandbox, and an unsandboxed chat's git after them
/// is named as one that cannot be placed. Either is read in a moment, however long the line.
#[test]
fn a_line_of_many_moves_that_may_each_fail_is_left_to_the_sandbox() {
    let l = Layout::new();
    let many: String = (0..20).map(|n| format!("cd d{n}; ")).collect();
    let cmd = format!("{many}cd {}; git commit -m x", l.folder.display());

    assert_eq!(l.ask(&cmd, &l.root), None);
    assert_eq!(l.writes(&cmd, &l.root), []);
    assert_eq!(l.unplaced(&cmd, &l.root).as_deref(), Some("commit"));
    let long = "cd d && ".repeat(5000) + "git commit -m x";
    let started = std::time::Instant::now();
    assert_eq!(l.writes(&long, &l.root), []);
    let long_and_then = long.replace("git commit", "true; git commit");
    assert_eq!(l.writes(&long_and_then, &l.root), []);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

/// A chat with no sandbox has nothing to leave a line to: a git that may write a branch after a
/// move the reader cannot place is named, so its guard refuses it rather than let it through.
#[test]
fn an_unsandboxed_chats_git_after_a_move_that_cannot_be_placed_is_named_unplaced() {
    let l = Layout::new();
    for cmd in [
        "cd \"$DIR\" && git commit -m x".to_owned(),
        "cd - && git push".to_owned(),
        "pushd +1 && git commit -m x".to_owned(),
        "pushd -1; git rebase main".to_owned(),
        "popd; git pull".to_owned(),
        "env -C \"$DIR\" git commit -m x".to_owned(),
        format!("cd \"$DIR\"; cd {}; git merge main", l.folder.display()),
    ] {
        assert_eq!(l.writes(&cmd, &l.folder), [], "{cmd}");
        assert!(
            l.unplaced(&cmd, &l.folder).is_some(),
            "{cmd} was let through"
        );
        // A sandboxed chat's line is left to its sandbox, as it was.
        assert_eq!(
            l.ask(&cmd.replace("commit -m x", "add -A"), &l.folder),
            None,
            "{cmd}"
        );
    }
    // Git that writes no branch, after the same move, is nothing to hold.
    assert_eq!(l.unplaced("cd \"$DIR\" && git status", &l.folder), None);
    assert_eq!(
        l.unplaced("cd \"$DIR\" && git commit --dry-run", &l.folder),
        None
    );
}

/// `pushd +1` turns the stack, and names no folder `+1`: it is never read as a move into one.
#[test]
fn pushd_with_a_place_in_the_stack_is_never_a_move_to_a_folder_of_that_name() {
    let l = Layout::new();
    std::fs::create_dir_all(l.root.join("+1")).unwrap();
    let cmd = "pushd +1 && git commit -m x";
    assert_eq!(l.writes(cmd, &l.root), []);
    assert_eq!(l.unplaced(cmd, &l.root).as_deref(), Some("commit"));
}

/// A relative `cd` is looked for along `CDPATH` first, so where the line sets one, the folder
/// it names may be another: only a `cd` that names a folder from where the shell is (`./`,
/// `../`) or a whole path is followed.
#[test]
fn a_relative_cd_that_cdpath_may_send_elsewhere_is_not_followed() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let ide = l.root.join("workspaces/ide/.worktrees/purlis");
    let along = "export CDPATH=/elsewhere; cd guard-port && git commit -m x";
    assert_eq!(l.writes(along, &ide), []);
    assert_eq!(l.unplaced(along, &ide).as_deref(), Some("commit"));
    for placed in [
        "export CDPATH=/elsewhere; cd ./guard-port && git commit -m x".to_owned(),
        format!(
            "export CDPATH=/elsewhere; cd {} && git commit -m x",
            l.folder.display()
        ),
        "cd guard-port && git commit -m x".to_owned(),
    ] {
        assert_eq!(
            l.writes(&placed, &ide),
            [("commit".to_owned(), Act::Commit, l.folder.clone())],
            "{placed}"
        );
        assert_eq!(l.unplaced(&placed, &ide), None, "{placed}");
    }
}

/// A project with one real clone and a branch folder purlis cut from it with git, where an
/// alias can be read as git reads it.
fn a_real_branch_folder() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let clone = root.join("workspaces/ide/purlis");
    std::fs::create_dir_all(&clone).unwrap();
    crate::testgit::run(&clone, &["init", "-q", "-b", "main"]);
    crate::testgit::run(&clone, &["commit", "-q", "--allow-empty", "-m", "one"]);
    let folder = root.join("workspaces/ide/.worktrees/purlis/p1");
    crate::testgit::run(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "p1",
            &folder.display().to_string(),
        ],
    );
    (dir, root, folder)
}

/// An alias is read as git reads it in the folder it runs in: one that stands for a git
/// command that writes the branch is that write, and one that runs a shell command, or that
/// the line defines itself, cannot be read and is named unplaced.
#[test]
fn an_alias_is_read_as_the_command_it_stands_for_or_named_where_it_cannot_be() {
    use crate::piececlaim::Act;
    let (_dir, root, folder) = a_real_branch_folder();
    crate::testgit::run(&folder, &["config", "alias.ci", "commit -v"]);
    crate::testgit::run(&folder, &["config", "alias.st", "status"]);
    crate::testgit::run(&folder, &["config", "alias.save", "!git commit -am wip"]);
    let (cwd, root) = (folder.display().to_string(), root.display().to_string());

    let ci = branch_write("git ci -m x", &cwd, &root);
    assert_eq!(
        ci.found
            .iter()
            .map(|one| (one.sub.as_str(), one.what, one.folder.clone()))
            .collect::<Vec<_>>(),
        [("ci", Act::Commit, folder.canonicalize().unwrap())]
    );
    assert_eq!(ci.unplaced, None);
    assert_eq!(
        branch_write("git st", &cwd, &root),
        BranchWrites {
            found: Vec::new(),
            unplaced: None,
        }
    );
    for cmd in [
        "git save",
        "git -c alias.zz=commit zz -m x",
        "GIT_CONFIG_COUNT=1 git zz",
    ] {
        let read = branch_write(cmd, &cwd, &root);
        assert!(read.found.is_empty(), "{cmd}");
        assert!(read.unplaced.is_some(), "{cmd} was let through");
    }
}

/// Where the project has a branch folder, its guard has one a git could be in.
#[test]
fn a_project_has_a_branch_folder_once_one_is_cut() {
    let l = Layout::new();
    assert!(any_branch_folder(&l.root));
    let empty = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(empty.path().join("workspaces/ide/.worktrees/purlis")).unwrap();
    assert!(!any_branch_folder(empty.path()));
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
    /// Every write on the line, as `(subcommand, act, folder)`.
    fn writes(&self, cmd: &str, cwd: &Path) -> Vec<(String, crate::piececlaim::Act, PathBuf)> {
        branch_write(
            cmd,
            &cwd.display().to_string(),
            &self.root.display().to_string(),
        )
        .found
        .into_iter()
        .map(|one| (one.sub, one.what, one.folder))
        .collect()
    }

    /// The first git on the line that may write a branch in a folder the line cannot be
    /// followed to, as an unsandboxed chat's guard reads it.
    fn unplaced(&self, cmd: &str, cwd: &Path) -> Option<String> {
        branch_write(
            cmd,
            &cwd.display().to_string(),
            &self.root.display().to_string(),
        )
        .unplaced
    }

    /// The line's one write, which must be in [`Layout::folder`], or `None` for none.
    fn write(&self, cmd: &str, cwd: &Path) -> Option<(String, crate::piececlaim::Act)> {
        let mut writes = self.writes(cmd, cwd);
        assert!(writes.len() <= 1, "{cmd}: {writes:?}");
        let (sub, act, folder) = writes.pop()?;
        assert_eq!(folder, self.folder, "{cmd}");
        Some((sub, act))
    }
}

/// The review of #4129: every write on the line is judged, each in its own folder, not only
/// the first.
#[test]
fn every_write_on_a_line_is_named_each_with_its_own_folder() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let cmd = format!(
        "git -C {} commit -m x && git -C {} push",
        l.folder.display(),
        l.second.display()
    );
    assert_eq!(
        l.writes(&cmd, &l.clone),
        vec![
            ("commit".to_owned(), Act::Commit, l.folder.clone()),
            ("push".to_owned(), Act::Push, l.second.clone()),
        ]
    );
    // What was placed before a move that cannot be is kept; nothing after it is read.
    let cmd = format!(
        "git -C {} commit -m x && cd $ELSEWHERE && git push",
        l.folder.display()
    );
    assert_eq!(
        l.writes(&cmd, &l.clone),
        vec![("commit".to_owned(), Act::Commit, l.folder.clone())]
    );
}

/// The review of #4129: `env -C <folder> git …` moves that one command, so it is placed there,
/// and the shell after it stays where it was.
#[test]
fn a_wrappers_own_move_to_a_literal_folder_places_that_one_command() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let cmd = format!("env -C {} git push", l.second.display());
    assert_eq!(
        l.writes(&cmd, &l.clone),
        vec![("push".to_owned(), Act::Push, l.second.clone())]
    );
    let cmd = format!("env -C {} git commit -m x; git push", l.folder.display());
    assert_eq!(
        l.writes(&cmd, &l.clone),
        vec![("commit".to_owned(), Act::Commit, l.folder.clone())],
        "the push runs in the clone"
    );
    // A move it cannot place leaves that command alone.
    assert_eq!(l.writes("env -C $PIECE git push", &l.clone), Vec::new());
    // And the sandboxed chat's arm places it the same way.
    let cmd = format!("env -C {} git commit -m x", l.folder.display());
    assert!(l.ask(&cmd, &l.clone).is_some(), "{cmd}");
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
