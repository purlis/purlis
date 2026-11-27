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
        "cd {} && cd ./workspaces && cd ./ide && cd ./.worktrees && cd ./purlis && \
         cd ./guard-port && cd ./src && cd .. && cd ./src && cd .. && cd ./src && cd .. && \
         git commit -m x",
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
/// `../`) or a whole path is followed. Where the line sets none, a sandboxed chat's bare `cd`
/// is followed.
#[test]
fn a_relative_cd_that_cdpath_may_send_elsewhere_is_not_followed() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let ide = l.root.join("workspaces/ide/.worktrees/purlis");
    let along = "export CDPATH=/elsewhere; cd guard-port && git commit -m x";
    assert_eq!(l.writes(along, &ide), []);
    assert_eq!(l.unplaced(along, &ide).as_deref(), Some("commit"));
    assert_eq!(l.ask(&along.replace("commit -m x", "add -A"), &ide), None);
    assert!(
        l.ask("cd guard-port && git add -A", &ide).is_some(),
        "a sandboxed chat's bare cd"
    );
    for placed in [
        "export CDPATH=/elsewhere; cd ./guard-port && git commit -m x".to_owned(),
        format!(
            "export CDPATH=/elsewhere; cd {} && git commit -m x",
            l.folder.display()
        ),
    ] {
        assert_eq!(
            l.writes(&placed, &ide),
            [("commit".to_owned(), Act::Commit, l.folder.clone())],
            "{placed}"
        );
        assert_eq!(l.unplaced(&placed, &ide), None, "{placed}");
    }
}

/// A `cd` with more than one operand goes nowhere its words name on its own: zsh's `cd old new`
/// swaps `old` for `new` in the folder the shell is in, and bash refuses it. Only a `cd` to
/// exactly one folder is followed.
#[test]
fn a_cd_with_more_than_one_operand_is_not_followed() {
    let l = Layout::new();
    let ide = l.root.join("workspaces/ide/.worktrees/purlis");
    for cmd in [
        "cd guard-port other && git commit -m x".to_owned(),
        "cd ./guard-port other && git commit -m x".to_owned(),
        format!("cd {} other && git commit -m x", l.folder.display()),
        "pushd guard-port other && git commit -m x".to_owned(),
    ] {
        assert_eq!(l.writes(&cmd, &ide), [], "{cmd}");
        assert_eq!(l.unplaced(&cmd, &ide).as_deref(), Some("commit"), "{cmd}");
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
        self.read(cmd, cwd, false)
    }

    /// The first git on the line that may write a branch in a folder the line cannot be
    /// followed to, as an unsandboxed chat's guard reads it.
    fn unplaced(&self, cmd: &str, cwd: &Path) -> Option<String> {
        self.read_where(cmd, cwd, false).unplaced
    }

    /// The line as an unsandboxed chat's guard reads it, its shell having a `CDPATH` or not.
    fn read_where(&self, cmd: &str, cwd: &Path, cdpath: bool) -> BranchWrites {
        branch_write_where(
            cmd,
            &cwd.display().to_string(),
            &self.root.display().to_string(),
            &|| cdpath,
        )
    }

    /// [`Layout::writes`], its shell having a `CDPATH` or not.
    fn read(
        &self,
        cmd: &str,
        cwd: &Path,
        cdpath: bool,
    ) -> Vec<(String, crate::piececlaim::Act, PathBuf)> {
        self.read_where(cmd, cwd, cdpath)
            .found
            .into_iter()
            .map(|one| (one.sub, one.what, one.folder))
            .collect()
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
        ("git commit -am x", "commit", Act::Commit),
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
        "git fetch",
        "git log -1",
        "git commit --dry-run -m x",
        "git push --dry-run",
        "git push -n",
        "git cherry-pick --quit",
        "git am --show-current-patch",
        "purlis worktree push",
    ] {
        assert_eq!(l.write(cmd, &l.folder), None, "{cmd}");
    }
    assert_eq!(l.write("git commit -m x", &l.clone), None);
    assert_eq!(l.write("git push", &l.theirs), None);
}

/// A line the reader cannot tokenize may still hold a git that writes a branch: an unsandboxed
/// chat's guard names it rather than let it through.
#[test]
fn a_line_that_cannot_be_read_and_names_git_is_named_unplaced() {
    let l = Layout::new();
    let cmd = "git commit -m \"never closed";
    assert_eq!(l.writes(cmd, &l.folder), []);
    assert!(
        l.unplaced(cmd, &l.folder).is_some(),
        "{cmd} was let through"
    );
    assert_eq!(l.unplaced("ls \"never closed", &l.folder), None);
}

/// git handed to a shell as a string, fed to one on its standard input, or reached through a
/// wrapper, may run anywhere and with any words: one that may write a branch is named.
#[test]
fn git_in_a_nested_shell_or_behind_a_wrapper_is_named_unplaced() {
    let l = Layout::new();
    let folder = l.folder.display();
    for cmd in [
        "bash -c \"git commit -m x\"".to_owned(),
        "sh -c 'cd src && git push'".to_owned(),
        "zsh -lc 'git rebase main'".to_owned(),
        "eval git commit -m x".to_owned(),
        "eval 'git push'".to_owned(),
        "echo 'git commit -m x' | sh".to_owned(),
        "echo commit | xargs git".to_owned(),
        "sudo git commit -m x".to_owned(),
        "doas git push".to_owned(),
        "env -i git commit -m x".to_owned(),
        "env -u HOME git commit -m x".to_owned(),
        format!("builtin cd {folder} && git commit -m x"),
        format!("command cd {folder} && git push"),
    ] {
        assert_eq!(l.writes(&cmd, &l.clone), [], "{cmd}");
        assert!(
            l.unplaced(&cmd, &l.clone).is_some(),
            "{cmd} was let through"
        );
    }
    for cmd in [
        "bash -c 'git status'",
        "sh -c 'git log --oneline'",
        "timeout 5 cat .gitignore",
        "bash ./build.sh && git status",
    ] {
        assert_eq!(l.unplaced(cmd, &l.clone), None, "{cmd}");
    }
}

/// A bare relative `cd` is looked for along `CDPATH` first. Where an unsandboxed chat's shell
/// may have one, the `cd` may go elsewhere, and only one named from where the shell is (`./`,
/// `../`) or a whole path is followed; where it has none, the bare `cd` is followed too.
#[test]
fn an_unsandboxed_chats_bare_relative_cd_is_followed_unless_its_shell_may_have_a_cdpath() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let ide = l.root.join("workspaces/ide/.worktrees/purlis");
    let bare = "cd guard-port && git commit -m x";
    let placed = [("commit".to_owned(), Act::Commit, l.folder.clone())];
    assert_eq!(l.read(bare, &ide, false), placed);
    assert_eq!(l.read_where(bare, &ide, false).unplaced, None);
    assert_eq!(l.read(bare, &ide, true), []);
    assert_eq!(
        l.read_where(bare, &ide, true).unplaced.as_deref(),
        Some("commit")
    );
    assert_eq!(
        l.read("cd ./guard-port && git commit -m x", &ide, true),
        placed
    );
}

/// What says whether a chat's shell may have a `CDPATH`: its environment, or a start file one
/// of its shells reads, in the home, in `ZDOTDIR`, in the system's folder, or named by
/// `BASH_ENV` or `ENV`. A start file that names it anywhere, in any case, is taken to set it.
#[test]
fn a_shell_may_have_a_cdpath_where_its_environment_or_a_start_file_it_reads_sets_one() {
    use std::ffi::OsString;
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (home, etc, zdot) = (base.join("home"), base.join("etc"), base.join("zdot"));
    for folder in [&home, &etc.join("zsh"), &zdot] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let may = |more: &[(&str, OsString)]| {
        let mut vars: Vec<(String, OsString)> = vec![("HOME".to_owned(), home.clone().into())];
        vars.extend(more.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
        shell_may_have_cdpath(
            &|name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()),
            &etc,
        )
    };
    assert!(!may(&[]));
    assert!(!may(&[("CDPATH", "".into())]));
    assert!(may(&[("CDPATH", "/src".into())]));
    std::fs::write(home.join(".zshrc"), "alias ll='ls -l'\n").unwrap();
    assert!(!may(&[]));
    for (file, says) in [
        (home.join(".zshenv"), "cdpath=(~/src)\n"),
        (home.join(".bashrc"), "export CDPATH=.:~/src\n"),
        (home.join(".profile"), "CdPath=~/src\n"),
        (etc.join("zsh/zshenv"), "cdpath+=(/srv)\n"),
        (etc.join("bashrc"), "CDPATH=/srv\n"),
    ] {
        std::fs::write(&file, says).unwrap();
        assert!(may(&[]), "{}", file.display());
        std::fs::remove_file(&file).unwrap();
    }
    std::fs::write(zdot.join(".zprofile"), "cdpath=(~/src)\n").unwrap();
    assert!(!may(&[]));
    assert!(may(&[("ZDOTDIR", zdot.clone().into())]));
    let named = base.join("bash_env");
    std::fs::write(&named, "CDPATH=~/src\n").unwrap();
    assert!(may(&[("BASH_ENV", named.clone().into())]));
    assert!(may(&[("ENV", named.clone().into())]));
    // A start file kept elsewhere and linked to, as a dotfiles manager keeps it, is read.
    std::os::unix::fs::symlink(&named, home.join(".zshenv")).unwrap();
    assert!(may(&[]));
}

/// A start file that reads another may set a `CDPATH` in the one it reads, which is not
/// followed; and a shell with `cdable_vars` (bash's `shopt`, zsh's `setopt` in any of its
/// spellings, or bash's `BASHOPTS` in the environment) takes a relative `cd` to a variable's
/// folder. Either way a bare relative `cd` may go elsewhere.
#[test]
fn a_shell_whose_start_files_read_others_or_that_has_cdable_vars_may_send_a_cd_elsewhere() {
    use std::ffi::OsString;
    let dir = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (home, etc) = (base.join("home"), base.join("etc"));
    for folder in [&home, &etc] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let may = |more: &[(&str, OsString)]| {
        let mut vars: Vec<(String, OsString)> = vec![("HOME".to_owned(), home.clone().into())];
        vars.extend(more.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
        shell_may_have_cdpath(
            &|name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()),
            &etc,
        )
    };
    let zshrc = home.join(".zshrc");
    for quiet in ["alias ll='ls -l'\n", "# see e.g. the manual. Then go.\n"] {
        std::fs::write(&zshrc, quiet).unwrap();
        assert!(!may(&[]), "{quiet}");
    }
    for says in [
        "source ~/.zsh_aliases\n",
        ". ~/.aliases\n",
        "[ -f ~/.local ] && . ~/.local\n",
        "if true; then . \"$HOME/x\"; fi\n",
        "setopt cdable_vars\n",
        "setopt CDABLEVARS\n",
        "setopt cd_able_vars\n",
        "shopt -s cdable_vars\n",
    ] {
        std::fs::write(&zshrc, says).unwrap();
        assert!(may(&[]), "{says}");
    }
    std::fs::remove_file(&zshrc).unwrap();
    assert!(!may(&[("BASHOPTS", "checkwinsize:cmdhist".into())]));
    assert!(may(&[("BASHOPTS", "cdable_vars:cmdhist".into())]));
}

/// A wrapper that moves no folder and changes no configuration git reads runs git where the
/// line is and as it reads: the git behind it is placed. One that may (`sudo`, `doas`, `env`
/// emptying or unsetting variables) is not read through.
#[test]
fn git_behind_a_wrapper_that_moves_no_folder_and_changes_no_config_is_placed() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    for (cmd, sub, act) in [
        ("time git commit -m x", "commit", Act::Commit),
        ("time -p git commit -m x", "commit", Act::Commit),
        ("command git commit -m x", "commit", Act::Commit),
        ("nohup git push", "push", Act::Push),
        ("timeout 60 git push", "push", Act::Push),
        ("timeout -s KILL 5m git push", "push", Act::Push),
        ("gtimeout 60 git push", "push", Act::Push),
        ("nice git commit -m x", "commit", Act::Commit),
        ("nice -n 5 git commit -m x", "commit", Act::Commit),
        ("caffeinate -i git push", "push", Act::Push),
        ("stdbuf -oL git push", "push", Act::Push),
        ("nohup nice timeout 60 git push", "push", Act::Push),
        ("FOO=1 timeout 60 git push", "push", Act::Push),
    ] {
        assert_eq!(
            l.write(cmd, &l.folder),
            Some((sub.to_owned(), act)),
            "{cmd}"
        );
        assert_eq!(l.unplaced(cmd, &l.folder), None, "{cmd}");
    }
    for cmd in [
        "sudo git commit -m x",
        "doas git push",
        "env -i git commit -m x",
        "env -u HOME git push",
        "timeout 60 sudo git push",
        "exec git push",
    ] {
        assert_eq!(l.writes(cmd, &l.folder), [], "{cmd}");
        assert!(
            l.unplaced(cmd, &l.folder).is_some(),
            "{cmd} was let through"
        );
    }
}

/// The shell takes a redirection out of a command's words wherever it stands, so one among the
/// words of `env` or another wrapper is read past, and the git behind it is placed.
#[test]
fn a_redirection_among_a_wrappers_words_is_read_past() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    for (cmd, sub, act) in [
        (
            "env FOO=1 2>/dev/null git commit -m x",
            "commit",
            Act::Commit,
        ),
        ("env 2>&1 git push", "push", Act::Push),
        ("env FOO=1 >/tmp/out git push", "push", Act::Push),
        ("nice 2>/dev/null git commit -m x", "commit", Act::Commit),
        ("FOO=1 2>/dev/null git push", "push", Act::Push),
    ] {
        assert_eq!(
            l.write(cmd, &l.folder),
            Some((sub.to_owned(), act)),
            "{cmd}"
        );
        assert_eq!(l.unplaced(cmd, &l.folder), None, "{cmd}");
    }
}

/// `xargs` and `parallel` may add any flag, so the ones in sight do not say what the git behind
/// them does: one whose subcommand writes a branch with some flags is named, whatever flag the
/// line shows that would make it write none. git's own commands that never write one pass.
#[test]
fn git_behind_xargs_whose_subcommand_can_write_is_named_whatever_flags_are_in_sight() {
    let l = Layout::new();
    for cmd in [
        "echo x | xargs git push --dry-run",
        "echo x | xargs git push -n",
        "echo x | xargs git commit --dry-run",
        "echo x | xargs git pull --dry-run",
        "echo x | xargs git merge --abort",
        "echo x | xargs git rebase --quit",
        "ls | parallel git push --dry-run",
    ] {
        assert!(
            l.unplaced(cmd, &l.folder).is_some(),
            "{cmd} was let through"
        );
    }
    for cmd in [
        "git ls-files | xargs git blame",
        "echo HEAD | xargs git show",
        "echo HEAD | xargs git log -1",
        "git ls-files | xargs git diff --",
        "git ls-files | xargs git status --",
        "ls | parallel git ls-files",
    ] {
        assert_eq!(l.unplaced(cmd, &l.folder), None, "{cmd}");
    }
}

/// git that `xargs` or `parallel` adds words to, or git on a line that cannot be read, is named
/// only where it may write a branch: its subcommand is missing, writes, or is not one of git's
/// own and so may be an alias. git's own read-only commands are let through.
#[test]
fn git_with_words_out_of_sight_is_named_only_where_it_may_write_a_branch() {
    let l = Layout::new();
    for cmd in [
        "git ls-files | xargs git blame",
        "echo HEAD | xargs git log -1",
        "git ls-files | xargs -n1 git log -1 --format=%H --",
        "ls | parallel git log -1",
        "git log --format=\"%H",
        "git status; git diff \"never closed",
    ] {
        assert_eq!(l.unplaced(cmd, &l.folder), None, "{cmd}");
    }
    for cmd in [
        "echo commit | xargs git",
        "echo x | xargs git commit -m",
        "echo x | xargs git ci",
        "echo -C | xargs git",
        "ls | parallel git push",
        "git commit -m \"never closed",
        "git ci -m \"never closed",
        "git status; bash -c \"git push",
    ] {
        assert!(
            l.unplaced(cmd, &l.folder).is_some(),
            "{cmd} was let through"
        );
    }
}

/// git in a string a shell or a runner is handed runs where and how that shell says: any shell
/// that runs a string or reads its standard input, a runner that hands a shell its words
/// (`watch`, `flock -c`, `script -c`, `parallel`), and a shell whose words end in a redirection
/// or that is fed a here-string. One that may write a branch is named.
#[test]
fn git_in_a_string_any_shell_or_runner_is_handed_is_named_unplaced() {
    let l = Layout::new();
    for cmd in [
        "watch 'git commit -m x'",
        "flock /tmp/lock -c 'git push'",
        "flock -c 'git push' /tmp/lock",
        "script -qc 'git push' /dev/null",
        "script -c 'git push'",
        "parallel ::: 'git push'",
        "echo 'git push' | parallel",
        "rbash -c 'git push'",
        "csh -c 'git push'",
        "tcsh -c 'git push'",
        "hush -c 'git push'",
        "posh -c 'git push'",
        "lksh -c 'git push'",
        "oksh -c 'git push'",
        "pdksh -c 'git push'",
        "fish -c 'git push'",
        "fish --command='git push'",
        "fish --command 'git push'",
        "echo 'git push' | bash 2>&1",
        "echo 'git push' | sh > /tmp/out",
        "bash <<< 'git push'",
        "bash 2>/dev/null <<< 'git commit -m x'",
        "sh -c 'bash <<< \"git push\"'",
    ] {
        assert_eq!(l.writes(cmd, &l.clone), [], "{cmd}");
        assert!(l.unplaced(cmd, &l.clone).is_some(), "{cmd} was let through");
    }
    for cmd in [
        "watch 'git status'",
        "fish -c 'git log'",
        "tcsh -c 'git diff'",
        "echo 'git status' | bash 2>&1",
        "bash <<< 'git log -1'",
        "flock /tmp/lock -c 'git status'",
    ] {
        assert_eq!(l.unplaced(cmd, &l.clone), None, "{cmd}");
    }
}

/// An alias is read as the line's git reads it: where the line moves where git finds its
/// configuration, or defines or includes configuration itself in any spelling, the alias
/// cannot be read and is named.
#[test]
fn an_alias_the_lines_own_configuration_may_change_is_named_unplaced() {
    let (_dir, root, folder) = a_real_branch_folder();
    crate::testgit::run(&folder, &["config", "alias.ci", "commit"]);
    let (cwd, root) = (folder.display().to_string(), root.display().to_string());
    for cmd in [
        "HOME=/elsewhere git ci -m x",
        "XDG_CONFIG_HOME=/elsewhere git ci -m x",
        "GIT_CONFIG_GLOBAL=/elsewhere git ci -m x",
        "GIT_CONFIG_NOSYSTEM=1 git ci -m x",
        "export HOME=/elsewhere; git ci -m x",
        "git -c include.path=/elsewhere ci -m x",
        "git -c includeIf.onbranch:p1.path=/elsewhere ci -m x",
        "git --config-env=alias.ci=WHAT ci -m x",
        "git -c ALIAS.zz=commit zz -m x",
        "git -c Alias.Zz=commit zz -m x",
    ] {
        let read = branch_write(cmd, &cwd, &root);
        assert!(read.found.is_empty(), "{cmd}: {:?}", read.found);
        assert!(read.unplaced.is_some(), "{cmd} was let through");
    }
}

/// An alias whose words begin with a global option, or that stands for another alias, is not
/// read further: it is named.
#[test]
fn an_alias_with_a_global_option_or_standing_for_another_alias_is_named_unplaced() {
    let (_dir, root, folder) = a_real_branch_folder();
    crate::testgit::run(&folder, &["config", "alias.pc", "-p commit"]);
    crate::testgit::run(&folder, &["config", "alias.ci", "commit"]);
    crate::testgit::run(&folder, &["config", "alias.c2", "ci"]);
    let (cwd, root) = (folder.display().to_string(), root.display().to_string());
    for cmd in ["git pc -m x", "git c2 -m x"] {
        let read = branch_write(cmd, &cwd, &root);
        assert!(read.found.is_empty(), "{cmd}: {:?}", read.found);
        assert!(read.unplaced.is_some(), "{cmd} was let through");
    }
}

/// A line of many shells fed on their standard input is read in a moment: the line's own words
/// are read for them once, not once for each.
#[test]
fn a_line_of_many_shells_fed_on_standard_input_is_read_in_a_moment() {
    let l = Layout::new();
    let line = "echo git status | sh; ".repeat(4000);
    let started = std::time::Instant::now();
    assert_eq!(l.unplaced(&line, &l.clone), None);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// A subcommand that is an expansion names no alias git can be asked for until the shell has
/// expanded it: it is named, never read as no alias at all.
#[test]
fn a_subcommand_that_is_an_expansion_is_named_unplaced() {
    let (_dir, root, folder) = a_real_branch_folder();
    let (cwd, root) = (folder.display().to_string(), root.display().to_string());
    for cmd in [
        "git $SUB -m x",
        "git ${SUB} -m x",
        "git \"$SUB\" -m x",
        "git \"$(echo commit)\" -m x",
        "git `echo commit` -m x",
        "git c* -m x",
    ] {
        let read = branch_write_where(cmd, &cwd, &root, &|| false);
        assert!(read.found.is_empty(), "{cmd}: {:?}", read.found);
        assert!(read.unplaced.is_some(), "{cmd} was let through");
    }
}

/// Where the child below finds its branch folder and project, and what it is told.
const ALIAS_FROM_THE_ENVIRONMENT: &str = "BRANCHGIT_TEST_ALIAS_FROM_THE_ENVIRONMENT";

/// An alias the chat's own git reads from its environment (`GIT_CONFIG_GLOBAL`, or
/// `GIT_CONFIG_COUNT` and its pairs) is read as that git reads it.
#[test]
fn an_alias_from_the_chats_own_git_config_environment_is_read() {
    let (_dir, root, folder) = a_real_branch_folder();
    let told = format!("{}\n{}", root.display(), folder.display());
    let child = ["branchgit::tests::an_alias_the_guard_is_given_in_its_environment_is_read"];
    crate::testrun::rerun(
        &child,
        &[
            (ALIAS_FROM_THE_ENVIRONMENT, told.as_ref()),
            ("GIT_CONFIG_COUNT", "1".as_ref()),
            ("GIT_CONFIG_KEY_0", "alias.zz".as_ref()),
            ("GIT_CONFIG_VALUE_0", "commit".as_ref()),
        ],
    );
    let global = root.join("global.gitconfig");
    std::fs::write(&global, "[alias]\n\tzz = commit\n").unwrap();
    crate::testrun::rerun(
        &child,
        &[
            (ALIAS_FROM_THE_ENVIRONMENT, told.as_ref()),
            ("GIT_CONFIG_GLOBAL", global.as_os_str()),
        ],
    );
}

/// The child of [`an_alias_from_the_chats_own_git_config_environment_is_read`]: nothing unless
/// it was started with [`ALIAS_FROM_THE_ENVIRONMENT`].
#[test]
fn an_alias_the_guard_is_given_in_its_environment_is_read() {
    let Some(told) = std::env::var_os(ALIAS_FROM_THE_ENVIRONMENT) else {
        return;
    };
    let told = told.to_string_lossy().into_owned();
    let (root, folder) = told.split_once('\n').unwrap();
    let read = branch_write_where("git zz -m x", folder, root, &|| false);
    assert_eq!(
        read.found
            .iter()
            .map(|one| (one.sub.as_str(), one.what))
            .collect::<Vec<_>>(),
        [("zz", crate::piececlaim::Act::Commit)]
    );
    assert_eq!(read.unplaced, None);
}

/// Every git that may move a branch or HEAD, or write the index or the working tree of a
/// branch folder, is judged as the porcelain that commits is: plumbing, a forced or renamed
/// branch, a reset, a worktree's own commands, and what rewrites refs or the tree.
#[test]
fn git_that_moves_a_branch_or_head_or_writes_the_index_or_the_tree_is_named() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    for (cmd, sub) in [
        ("git update-ref refs/heads/p1 HEAD~1", "update-ref"),
        ("git update-ref -d refs/heads/other", "update-ref"),
        ("git symbolic-ref HEAD refs/heads/other", "symbolic-ref"),
        ("git symbolic-ref -d HEAD", "symbolic-ref"),
        ("git branch -f p1 HEAD~2", "branch"),
        ("git branch -m p1 renamed", "branch"),
        ("git branch -M renamed", "branch"),
        ("git branch -D other", "branch"),
        ("git branch new-branch", "branch"),
        ("git branch --set-upstream-to=origin/main", "branch"),
        ("git checkout -B p1 origin/main", "checkout"),
        ("git checkout main", "checkout"),
        ("git checkout -- src/a.rs", "checkout"),
        ("git switch -C p1 origin/main", "switch"),
        ("git switch other", "switch"),
        ("git reset --hard HEAD~1", "reset"),
        ("git reset HEAD~1", "reset"),
        ("git reset src/a.rs", "reset"),
        ("git restore src/a.rs", "restore"),
        ("git restore --staged src/a.rs", "restore"),
        ("git add src/a.rs", "add"),
        ("git rm src/a.rs", "rm"),
        ("git mv src/a.rs src/b.rs", "mv"),
        ("git clean -fd", "clean"),
        ("git stash", "stash"),
        ("git stash pop", "stash"),
        ("git worktree add ../other", "worktree"),
        ("git worktree move . ../elsewhere", "worktree"),
        ("git worktree remove .", "worktree"),
        ("git worktree prune", "worktree"),
        ("git fast-import < stream", "fast-import"),
        ("git read-tree -u -m HEAD", "read-tree"),
        ("git checkout-index -a -f", "checkout-index"),
        (
            "git update-index --assume-unchanged src/a.rs",
            "update-index",
        ),
        ("git notes add -m x", "notes"),
        ("git replace abc123 def456", "replace"),
        ("git gc", "gc"),
        ("git prune", "prune"),
        ("git reflog expire --expire=now --all", "reflog"),
        ("git reflog delete HEAD@{1}", "reflog"),
        ("git tag v1", "tag"),
        ("git tag -d v1", "tag"),
        ("git rebase --abort", "rebase"),
        ("git merge --abort", "merge"),
        ("git fetch origin main:main", "fetch"),
        ("git fetch . HEAD:refs/heads/p1", "fetch"),
        ("git config user.name someone", "config"),
        ("git config --unset core.hooksPath", "config"),
        ("git config set core.bare true", "config"),
        ("git remote add up https://example.com/x.git", "remote"),
        ("git apply fix.patch", "apply"),
        ("git submodule update --init", "submodule"),
        ("git sparse-checkout set src", "sparse-checkout"),
        ("git bisect start", "bisect"),
        ("git filter-branch --force HEAD", "filter-branch"),
        ("git pack-refs --all", "pack-refs"),
        ("git maintenance run", "maintenance"),
    ] {
        assert_eq!(
            l.write(cmd, &l.folder),
            Some((sub.to_owned(), Act::Update)),
            "{cmd}"
        );
    }
}

/// A subcommand that is neither git's own nor an alias, such as an extension's `git-<name>`
/// program, may do anything, so it counts as a write.
#[test]
fn an_unknown_subcommand_counts_as_a_write() {
    use crate::piececlaim::Act;
    let (_dir, root, folder) = a_real_branch_folder();
    crate::testgit::run(&folder, &["config", "alias.st", "status"]);
    crate::testgit::run(&folder, &["config", "alias.up", "update-ref"]);
    let (cwd, root) = (folder.display().to_string(), root.display().to_string());
    let folder = folder.canonicalize().unwrap();
    for (cmd, sub) in [
        ("git frobnicate --all", "frobnicate"),
        ("git lfs migrate import", "lfs"),
        ("git up refs/heads/p1 HEAD", "up"),
    ] {
        let read = branch_write(cmd, &cwd, &root);
        assert_eq!(
            read.found
                .iter()
                .map(|one| (one.sub.as_str(), one.what, one.folder.clone()))
                .collect::<Vec<_>>(),
            [(sub, Act::Update, folder.clone())],
            "{cmd}"
        );
        assert_eq!(read.unplaced, None, "{cmd}");
    }
    assert_eq!(
        branch_write("git st --short", &cwd, &root),
        BranchWrites {
            found: Vec::new(),
            unplaced: None,
        }
    );
}

/// The everyday git a chat runs to look, in a branch folder: none of it is judged, and none is
/// named unplaced. The measure of the guard's false positives.
#[test]
fn everyday_read_only_git_in_a_branch_folder_is_let_through() {
    let l = Layout::new();
    let folder = l.folder.display();
    let corpus = [
        "git status".to_owned(),
        "git status --porcelain".to_owned(),
        "git status -sb".to_owned(),
        "git diff".to_owned(),
        "git diff --cached".to_owned(),
        "git diff --stat origin/main...HEAD".to_owned(),
        "git diff HEAD~1 -- src/a.rs | cat".to_owned(),
        "git diff-index --quiet HEAD --".to_owned(),
        "git diff-tree --no-commit-id --name-only -r HEAD".to_owned(),
        "git log -1".to_owned(),
        "git log --oneline -20".to_owned(),
        "git log --graph --decorate --all".to_owned(),
        "git log -p -- src/a.rs | head -50".to_owned(),
        "git log -g".to_owned(),
        "git --no-pager log --oneline".to_owned(),
        "git -P show HEAD".to_owned(),
        "git -c color.ui=never diff".to_owned(),
        "git show HEAD:src/a.rs".to_owned(),
        "git show --stat".to_owned(),
        "git shortlog -sn".to_owned(),
        "git whatchanged -1".to_owned(),
        "git blame src/a.rs".to_owned(),
        "git annotate src/a.rs".to_owned(),
        "git grep -n needle".to_owned(),
        "git ls-files".to_owned(),
        "git ls-files -m".to_owned(),
        "git ls-tree -r HEAD".to_owned(),
        "git ls-remote origin".to_owned(),
        "git rev-parse HEAD".to_owned(),
        "git rev-parse --show-toplevel".to_owned(),
        "git rev-parse --abbrev-ref HEAD".to_owned(),
        "git rev-list --count HEAD".to_owned(),
        "git cat-file -p HEAD".to_owned(),
        "git cat-file -t abc123".to_owned(),
        "git for-each-ref --format='%(refname)' refs/heads".to_owned(),
        "git show-ref --heads".to_owned(),
        "git show-branch".to_owned(),
        "git describe --tags".to_owned(),
        "git name-rev HEAD".to_owned(),
        "git merge-base HEAD origin/main".to_owned(),
        "git merge-base --is-ancestor HEAD origin/main".to_owned(),
        "git cherry -v origin/main".to_owned(),
        "git range-diff main...HEAD".to_owned(),
        "git check-ignore -v target".to_owned(),
        "git check-attr -a src/a.rs".to_owned(),
        "git count-objects -v".to_owned(),
        "git fsck".to_owned(),
        "git verify-commit HEAD".to_owned(),
        "git format-patch -1 --stdout".to_owned(),
        "git archive HEAD | tar t".to_owned(),
        "git apply --check fix.patch".to_owned(),
        "git apply --stat fix.patch".to_owned(),
        "git fetch".to_owned(),
        "git fetch origin".to_owned(),
        "git fetch --prune origin main".to_owned(),
        "git branch".to_owned(),
        "git branch -a".to_owned(),
        "git branch -vv".to_owned(),
        "git branch -r".to_owned(),
        "git branch --show-current".to_owned(),
        "git branch --list 'feat*'".to_owned(),
        "git branch --contains HEAD".to_owned(),
        "git branch --merged main".to_owned(),
        "git tag".to_owned(),
        "git tag -l 'v*'".to_owned(),
        "git tag --contains HEAD".to_owned(),
        "git stash list".to_owned(),
        "git stash show -p".to_owned(),
        "git worktree list".to_owned(),
        "git worktree list --porcelain".to_owned(),
        "git remote".to_owned(),
        "git remote -v".to_owned(),
        "git remote get-url origin".to_owned(),
        "git remote show origin".to_owned(),
        "git config user.email".to_owned(),
        "git config --get remote.origin.url".to_owned(),
        "git config --get-regexp '^alias'".to_owned(),
        "git config --list".to_owned(),
        "git config -l --show-origin".to_owned(),
        "git config get user.name".to_owned(),
        "git config --file .gitmodules --get-regexp path".to_owned(),
        "git reflog".to_owned(),
        "git reflog show p1".to_owned(),
        "git symbolic-ref HEAD".to_owned(),
        "git symbolic-ref --short HEAD".to_owned(),
        "git submodule status".to_owned(),
        "git sparse-checkout list".to_owned(),
        "git notes show HEAD".to_owned(),
        "git bisect log".to_owned(),
        "git help commit".to_owned(),
        "git version".to_owned(),
        "git --version".to_owned(),
        "git var GIT_EDITOR".to_owned(),
        "git hash-object src/a.rs".to_owned(),
        "git -C src log -1".to_owned(),
        "git rebase --show-current-patch".to_owned(),
        "git cherry-pick --quit".to_owned(),
        format!("cd {folder} && git status && git diff --stat && git log -3"),
        "git status; git log -1; git diff".to_owned(),
        "git -C $SOMEWHERE log".to_owned(),
        "GIT_DIR=$SOMEWHERE git log".to_owned(),
        "git --work-tree=$SOMEWHERE status".to_owned(),
    ];
    let mut judged = Vec::new();
    for cmd in &corpus {
        let read = branch_write(
            cmd,
            &l.folder.display().to_string(),
            &l.root.display().to_string(),
        );
        if !read.found.is_empty() || read.unplaced.is_some() {
            judged.push((cmd.clone(), read));
        }
    }
    assert!(
        judged.is_empty(),
        "{} of {} read-only lines were judged: {judged:#?}",
        judged.len(),
        corpus.len()
    );
}

/// `-C`, `--git-dir`, `--work-tree`, `GIT_DIR` and `GIT_WORK_TREE` place a git where they
/// point: a branch folder written from elsewhere is judged as if from inside it, and so is a
/// branch folder's working tree written through another repository.
#[test]
fn a_git_pointed_at_a_branch_folder_from_elsewhere_is_judged_there() {
    use crate::piececlaim::Act;
    let l = Layout::new();
    let folder = l.folder.display();
    let record = l.clone.join(".git/worktrees/guard-port");
    let record = record.display();
    let clone_git = l.clone.join(".git");
    let clone_git = clone_git.display();
    for (cmd, at) in [
        (format!("git -C {folder} reset --hard"), &l.clone),
        (
            format!("git -C {folder} update-ref refs/heads/p1 HEAD~1"),
            &l.clone,
        ),
        (
            format!("git --git-dir={record} symbolic-ref HEAD refs/heads/x"),
            &l.clone,
        ),
        (
            format!("git --git-dir {record} update-ref HEAD HEAD~1"),
            &l.clone,
        ),
        (
            format!("GIT_DIR={record} git branch -f p1 HEAD~1"),
            &l.clone,
        ),
        (
            format!("export GIT_DIR={record}; git reset --hard"),
            &l.clone,
        ),
        (
            format!("git --git-dir={clone_git} --work-tree={folder} checkout main -- ."),
            &l.clone,
        ),
        (
            format!("GIT_WORK_TREE={folder} git checkout -f main"),
            &l.clone,
        ),
        (format!("git --work-tree {folder} reset --hard"), &l.clone),
        // A git directory given without a work tree writes the tree it runs in.
        (
            format!("GIT_DIR={clone_git} git checkout -f main"),
            &l.folder,
        ),
    ] {
        assert!(
            l.writes(&cmd, at)
                .iter()
                .any(|(_, act, placed)| *act == Act::Update && *placed == l.folder),
            "{cmd}: {:?}",
            l.writes(&cmd, at)
        );
    }
    // Pointed away from every branch folder, a write is not this guard's.
    let cmd = format!("git -C {} reset --hard", l.clone.display());
    assert_eq!(l.writes(&cmd, &l.folder), []);
    assert_eq!(l.unplaced(&cmd, &l.folder), None);
}

/// Where `-C`, `--git-dir`, `--work-tree`, `GIT_DIR` or `GIT_WORK_TREE` names a place the
/// line does not show, a git that may write is named unplaced.
#[test]
fn a_git_pointed_where_the_line_does_not_show_is_named_unplaced() {
    let l = Layout::new();
    for (cmd, sub) in [
        ("git -C $PIECE reset --hard", "reset"),
        (
            "git -C ~/elsewhere update-ref refs/heads/p1 HEAD",
            "update-ref",
        ),
        ("git --git-dir=$X/.git branch -f p1", "branch"),
        (
            "git --git-dir $X symbolic-ref HEAD refs/heads/x",
            "symbolic-ref",
        ),
        ("git --work-tree=$X checkout -f main", "checkout"),
        ("git --work-tree ~/x reset --hard", "reset"),
        ("GIT_DIR=$X git update-ref HEAD HEAD~1", "update-ref"),
        ("GIT_WORK_TREE=$X git checkout .", "checkout"),
        ("export GIT_DIR=$X; git reset --hard", "reset"),
        ("git -C $PIECE commit -m x", "commit"),
    ] {
        let read = branch_write(
            cmd,
            &l.clone.display().to_string(),
            &l.root.display().to_string(),
        );
        assert!(read.found.is_empty(), "{cmd}: {:?}", read.found);
        assert_eq!(read.unplaced.as_deref(), Some(sub), "{cmd}");
    }
}

/// Judging a git reads its words a bounded number of times: a line of very many words is read
/// in a moment.
#[test]
fn a_git_of_very_many_words_is_judged_in_a_moment() {
    let l = Layout::new();
    let many = " x".repeat(200_000);
    for sub in [
        "branch",
        "config",
        "tag",
        "fetch origin",
        "symbolic-ref",
        "apply",
    ] {
        let cmd = format!("git {sub}{many}");
        let started = std::time::Instant::now();
        let _ = l.writes(&cmd, &l.folder);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{sub}: {:?}",
            started.elapsed()
        );
    }
}

/// A heredoc body no shell runs is data (#6293): a ledger entry or a script written to a file
/// that names git is not a git the line runs, whether its delimiter is quoted or not, indented
/// (`<<-`) or one of several.
#[test]
fn a_heredoc_body_no_shell_runs_is_never_read_as_git() {
    let l = Layout::new();
    for cmd in [
        // An append to a ledger whose text names `git push`.
        "cat >> ledger.md <<'EOF'\n- pushed with `cd \"$WT\" && git push`\n$GIT push origin main\nEOF",
        // A patch script written to a scratch file.
        "cat > \"$TMPDIR/p.sh\" <<\"EOF\"\n#!/bin/sh\ncd \"$1\" && git commit -am x\n$GIT push\nEOF\nchmod +x \"$TMPDIR/p.sh\"",
        // Indented, its tab-led terminator ends it.
        "cat > p.sh <<-'EOF'\n\tcd \"$1\" && git push\n\tEOF",
        // Two on one line, and a body that spells a heredoc of its own.
        "cat <<'A' > a.sh; cat <<'B' > b.sh\n$GIT push\nA\ncd \"$1\" && git rebase main\nB",
        "cat > outer.sh <<'OUT'\ncat > inner.sh <<'IN'\n$GIT push\nIN\ncd \"$1\" && git push\nOUT",
        // Unquoted: expanded, but the text around a substitution is data still.
        "cat > notes.md <<EOF\nrun cd \"$WT\" && git push when $USER is back\nEOF",
    ] {
        assert_eq!(l.unplaced(cmd, &l.root), None, "{cmd}");
        assert_eq!(l.writes(cmd, &l.root), [], "{cmd}");
    }
    // In a branch folder, a sandboxed chat's guard reads the same.
    let cmd = "cat > p.sh <<'EOF'\ngit add -A\ngit commit -m x\nEOF";
    assert_eq!(l.ask(cmd, &l.folder), None);
    assert_eq!(l.writes(cmd, &l.folder), []);
}

/// What a shell does run is still read (#6293): the command after a body, a body fed to a
/// shell, and a substitution in a body whose delimiter is unquoted.
#[test]
fn git_after_a_heredoc_or_run_from_one_is_still_read() {
    let l = Layout::new();
    for (cmd, sub) in [
        (
            "cat > f <<'EOF'\ngit push\nEOF\ncd \"$DIR\" && git push",
            "push",
        ),
        (
            "cat > f <<-'EOF'\n\tgit push\n\tEOF\ncd \"$DIR\" && git commit -m x",
            "commit",
        ),
        ("bash <<'EOF'\ncd \"$DIR\" && git push\nEOF", "push"),
        (
            "cat > f <<EOF\nsha: $(cd \"$DIR\" && git commit -m x)\nEOF",
            "commit",
        ),
        ("cat > f <<EOF\n'`cd \"$DIR\" && git push`'\nEOF", "push"),
    ] {
        assert_eq!(l.unplaced(cmd, &l.root).as_deref(), Some(sub), "{cmd}");
    }
    let cmd = "cat > f <<'EOF'\nnotes\nEOF\ngit add -A";
    assert_eq!(
        l.ask(cmd, &l.folder).map(|(sub, _)| sub).as_deref(),
        Some("add")
    );
}
