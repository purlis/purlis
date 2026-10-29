// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the arm answers: first from a command's words alone, with git's answers stood in for,
//! then through the walk on a folder, and last on a real repository.
//!
//! A test never writes a name the sandbox denies (a manifest, `.claude/skills`): a chat that
//! runs this suite is itself denied writing them. A stand-in name is handed to the arm instead.

use super::*;

/// git's answers, stood in for: the names HEAD is at, the operands git tracks as paths, and
/// the paths that are plain files.
#[derive(Default)]
struct Fake {
    at: &'static [&'static str],
    tracked: &'static [&'static str],
    files: &'static [&'static str],
}

impl Asks for Fake {
    fn head_is_at(&self, target: &str) -> bool {
        self.at.contains(&target)
    }

    fn operand_kind(&self, operand: &str) -> OperandKind {
        if self.tracked.contains(&operand) {
            OperandKind::Path
        } else {
            OperandKind::Rev
        }
    }

    fn is_a_file(&self, path: &str) -> bool {
        self.files.contains(&path)
    }
}

const ROOT: &str = "/p";

fn held() -> Vec<String> {
    vec!["purlis.toml".to_string(), ".claude/skills".to_string()]
}

/// The arm's answer for `git <words>` in the root, with `fake` for git.
fn judge_with(words: &str, fake: &Fake) -> Option<String> {
    let mut words = words.split_whitespace().map(str::to_string);
    let sub = words.next().expect("a subcommand");
    let post: Vec<String> = words.collect();
    judged(&sub, &post, &held(), ROOT, fake)
}

fn judge(words: &str) -> Option<String> {
    judge_with(words, &Fake::default())
}

#[test]
fn a_command_that_rewrites_the_tree_from_another_commit_is_refused_before_it_starts() {
    for update in [
        "rebase origin/main",
        "rebase",
        "rebase -i HEAD~3",
        "rebase --onto main feature topic",
        "rebase --continue",
        "rebase --skip",
        "pull",
        "pull --rebase",
        "pull --ff-only origin main",
        "pull --no-rebase",
        "merge origin/main",
        "merge --ff-only origin/main",
        "merge --no-ff feature",
        "reset --hard origin/main",
        "reset --hard HEAD~1",
        "reset --keep HEAD~1",
        "reset --merge ORIG_HEAD",
        "reset -q --hard origin/main --",
        "stash pop",
        "stash apply stash@{1}",
        "stash -q pop",
        "stash branch topic",
        "cherry-pick abc123",
        "cherry-pick --continue",
        "am fix.patch",
        "am --continue",
        "revert HEAD",
        "bisect start",
        "bisect good",
        "bisect reset",
        "read-tree -u -m origin/main",
        "read-tree -mu origin/main",
        "switch feature",
        "switch -q main",
        "checkout main",
    ] {
        let said = judge(update).unwrap_or_else(|| panic!("git {update}: allowed"));
        let sub = update.split(' ').next().unwrap();
        assert!(
            said.starts_with(&format!("`git {sub}` would rewrite")),
            "{update}: {said}"
        );
        // Why, in the words the person will read.
        assert!(
            said.contains(
                "the sandbox stops this partway and would leave the project half-updated"
            ),
            "{update}: {said}"
        );
        assert!(
            said.contains("(`purlis.toml`, `.claude/skills`)"),
            "{update}: {said}"
        );
        assert!(said.contains("Nothing was run."), "{update}: {said}");
        // What to do instead, and the way back.
        assert!(
            said.contains("Run `purlis worktree update` here instead"),
            "{update}: {said}"
        );
        assert!(
            said.contains("`git restore --source=HEAD --worktree -- <paths>`"),
            "{update}: {said}"
        );
    }
}

#[test]
fn the_way_back_and_what_writes_no_file_from_another_commit_are_never_refused() {
    let at_main = Fake {
        at: &["main", "HEAD"],
        tracked: &["a", "b"],
        ..Fake::default()
    };
    for kept in [
        // An operation git stopped in the middle of, ended.
        "rebase --abort",
        "rebase --quit",
        "rebase --show-current-patch",
        "merge --abort",
        "merge --continue",
        "cherry-pick --abort",
        "revert --quit",
        "am --abort",
        // The tree put back as the current commit has it.
        "reset --hard",
        "reset --hard HEAD",
        "reset --hard @",
        "reset -q --hard HEAD --",
        "restore --source=HEAD --worktree -- a b",
        "restore --source=HEAD --worktree --pathspec-from-file=list.z --pathspec-file-nul",
        "restore --source HEAD --worktree a",
        "restore -s HEAD -W a",
        "restore -sHEAD a",
        "restore -Ws @ a",
        "restore a",
        "restore .",
        "restore --worktree --staged a",
        "checkout -- a",
        "checkout HEAD -- a b",
        "checkout @ -- .",
        "checkout -f -- a",
        "checkout a",
        "checkout a b",
        "checkout HEAD a b",
        "checkout",
        // The root already on the branch named: no file is written.
        "checkout main",
        "switch main",
        // A reset that leaves every file as it is.
        "reset --soft origin/main",
        "reset origin/main",
        "reset --mixed HEAD~1",
        "reset origin/main -- a",
        // The index only.
        "restore --staged --source=origin/main a",
        "restore -S -s origin/main a",
        "read-tree origin/main",
        // Stashes made, listed and dropped; a bisect read.
        "stash",
        "stash push -u",
        "stash list",
        "stash show -p",
        "stash drop",
        "bisect log",
        "bisect",
        // And everything this arm does not judge.
        "fetch origin",
        "status",
        "log --oneline",
        "commit -m rebase",
        "add -A",
        "push",
    ] {
        assert_eq!(judge_with(kept, &at_main), None, "git {kept}");
    }
}

#[test]
fn a_restore_that_names_a_file_the_sandbox_denies_is_refused_and_says_which() {
    let said = judge("restore --source=HEAD --worktree -- a purlis.toml .claude/skills/x/SKILL.md")
        .expect("refused");
    assert!(
        said.starts_with(
            "`git restore` would put back `purlis.toml`, `.claude/skills/x/SKILL.md`, which"
        ),
        "{said}"
    );
    assert!(said.contains("Nothing was run."), "{said}");
    assert!(
        said.contains("Restore the other paths without those"),
        "{said}"
    );
    // The exact command, for the person's terminal, for those files and no other.
    assert!(
        said.ends_with(
            "in their own terminal: `git -C /p restore --source=HEAD --worktree -- purlis.toml \
             .claude/skills/x/SKILL.md`."
        ),
        "{said}"
    );

    // Out of the index, as the chat asked it.
    let tracked = Fake {
        tracked: &["purlis.local.toml"],
        ..Fake::default()
    };
    for (words, command) in [
        (
            "checkout -- purlis.local.toml",
            "`git -C /p restore --worktree -- purlis.local.toml`.",
        ),
        (
            "checkout purlis.local.toml",
            "`git -C /p restore --worktree -- purlis.local.toml`.",
        ),
        (
            "restore purlis.local.toml",
            "`git -C /p restore --worktree -- purlis.local.toml`.",
        ),
        (
            "checkout HEAD -- .claude/settings.json",
            "`git -C /p restore --source=HEAD --worktree -- .claude/settings.json`.",
        ),
        (
            "restore --source=origin/main -- a 'it s.toml' .mcp.json",
            "`git -C /p restore --source=origin/main --worktree -- .mcp.json`.",
        ),
    ] {
        let said = judge_with(words, &tracked).unwrap_or_else(|| panic!("git {words}: allowed"));
        assert!(said.ends_with(command), "git {words}: {said}");
        assert!(said.contains("fail on that one"), "git {words}: {said}");
    }
}

#[test]
fn a_file_is_denied_by_where_its_name_puts_it() {
    let root = Path::new(ROOT);
    for denied in [
        "purlis.toml",
        "purlis.local.toml",
        "./purlis.toml",
        "../purlis.toml",
        "docs/../purlis.toml",
        ":/purlis.toml",
        "/p/purlis.toml",
        ".claude/skills/x/SKILL.md",
        "personas/dev/.claude/settings.json",
        ".mcp.json",
        "workspaces/a/.mcp.json",
        ".vscode/tasks.json",
        ".envrc",
        ".githooks/pre-commit",
        "/p/.claude/commands/x.md",
    ] {
        assert!(path_is_denied(denied, root), "{denied}");
    }
    // Every name the project's manifest has had, the older ones included.
    for manifest in sandbox::MANIFESTS {
        assert!(path_is_denied(manifest, root), "{manifest}");
        assert!(denied_names().contains(&manifest), "{manifest}");
    }
    for free in [
        "README.md",
        "docs/purlis.toml.md",
        // A manifest lower down changes no chat's sandbox, and is an ordinary file.
        "tests/fixtures/purlis.toml",
        "/elsewhere/purlis.toml",
        // Below `.claude` itself, only the names that load code are held.
        ".claude/rules/x.md",
        "personas/dev/memory/the-skills.md",
        ".",
        "",
    ] {
        assert!(!path_is_denied(free, root), "{free}");
    }
}

#[test]
fn a_restore_from_another_commit_runs_only_where_every_path_is_a_file_in_sight() {
    let fake = Fake {
        files: &["a.md", "docs/b.md"],
        tracked: &["a.md"],
        ..Fake::default()
    };
    for seen in [
        "checkout origin/main -- a.md docs/b.md",
        "restore --source=origin/main a.md",
        "restore -s origin/main -- a.md",
        "restore --source origin/main --worktree a.md docs/b.md",
        "restore -W -sorigin/main a.md",
        "checkout feature docs/b.md",
    ] {
        assert_eq!(judge_with(seen, &fake), None, "git {seen}");
    }
    for (unseen, commit) in [
        ("checkout origin/main -- docs", "origin/main"),
        ("checkout origin/main -- .", "origin/main"),
        ("checkout origin/main -- a.md new.md", "origin/main"),
        ("checkout origin/main -- a*.md", "origin/main"),
        ("checkout origin/main -- :/", "origin/main"),
        ("checkout origin/main -- $FILES", "origin/main"),
        (
            "restore --source=origin/main --pathspec-from-file=list",
            "origin/main",
        ),
        ("restore --source=HEAD~1 --worktree", "HEAD~1"),
        ("restore -s HEAD~1 docs", "HEAD~1"),
        ("restore --staged --worktree -s v1 .", "v1"),
        ("checkout feature docs", "feature"),
    ] {
        let said = judge_with(unseen, &fake).unwrap_or_else(|| panic!("git {unseen}: allowed"));
        assert!(
            said.contains(&format!(
                "would write files from another commit ('{commit}')"
            )),
            "git {unseen}: {said}"
        );
        assert!(
            said.contains("Name the files one by one"),
            "git {unseen}: {said}"
        );
        assert!(said.contains("Nothing was run."), "git {unseen}: {said}");
    }
}

#[test]
fn the_names_looked_for_in_the_root_are_the_ones_a_checkout_would_write() {
    let names = denied_names();
    for name in [
        "purlis.toml",
        "purlis.local.toml",
        ".claude/skills",
        ".claude/settings.json",
        ".mcp.json",
        ".vscode",
        ".envrc",
    ] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }
    // git's own folder is no part of the working tree, and every repository has one.
    assert!(
        names
            .iter()
            .all(|name| *name != ".git" && !name.starts_with(".git/")),
        "{names:?}"
    );
    assert!(names.iter().all(|name| !name.contains("**")), "{names:?}");

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("conf/held")).unwrap();
    std::fs::write(dir.path().join("held.toml"), "").unwrap();
    assert_eq!(
        found_in(dir.path(), &["held.toml", "gone.toml", "conf/held"]),
        ["held.toml", "conf/held"]
    );
}

#[test]
fn only_a_chat_that_may_write_the_root_folder_is_answered() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    let below = root.join("workspaces/alpha");
    let beside = dir.path().join("elsewhere");
    std::fs::create_dir_all(&below).unwrap();
    std::fs::create_dir_all(&beside).unwrap();
    let root_str = root.display().to_string();
    assert!(writes_the_root(&root_str, None), "a folder nobody named");
    assert!(writes_the_root(&root_str, Some(&root)));
    assert!(
        writes_the_root(&root_str, Some(dir.path())),
        "a folder holding the root"
    );
    assert!(!writes_the_root(&root_str, Some(&below)));
    assert!(!writes_the_root(&root_str, Some(&beside)));
    assert!(
        writes_the_root(&root_str, Some(&root.join("gone"))),
        "a folder that does not resolve"
    );
}

/// A folder standing for the project's root, with one name in it the sandbox is said to deny.
struct Root {
    _dir: tempfile::TempDir,
    root: String,
    elsewhere: String,
}

fn a_root() -> Root {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::pypath::realpath(dir.path().to_str().unwrap());
    let root = format!("{base}/project");
    let elsewhere = format!("{base}/elsewhere");
    std::fs::create_dir_all(format!("{root}/docs")).unwrap();
    std::fs::create_dir_all(format!("{root}/workspaces/alpha")).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(format!("{root}/held.toml"), "").unwrap();
    std::fs::write(format!("{root}/README.md"), "").unwrap();
    Root {
        _dir: dir,
        root,
        elsewhere,
    }
}

fn stand_in(root: &Path) -> Vec<String> {
    found_in(root, &["held.toml"])
}

/// The arm's answer for `cmd`, run in the root by a chat started there.
fn asked(r: &Root, cmd: &str) -> Option<(String, String)> {
    refusal_where(cmd, &r.root, &r.root, Some(Path::new(&r.root)), &stand_in)
}

#[test]
fn an_update_in_the_root_is_refused_however_the_line_reaches_it() {
    let r = a_root();
    let root = &r.root;
    for (cmd, sub) in [
        ("git rebase origin/main".to_string(), "rebase"),
        ("git pull".to_string(), "pull"),
        ("git pull --rebase".to_string(), "pull"),
        ("git merge origin/main".to_string(), "merge"),
        ("git reset --hard origin/main".to_string(), "reset"),
        ("git stash pop".to_string(), "stash"),
        ("git stash apply".to_string(), "stash"),
        ("git cherry-pick abc123".to_string(), "cherry-pick"),
        ("git am fix.patch".to_string(), "am"),
        ("git revert HEAD".to_string(), "revert"),
        (
            "git fetch origin && git rebase origin/main".to_string(),
            "rebase",
        ),
        (format!("git -C {root} pull"), "pull"),
        (
            format!("cd {} && git -C {root} rebase origin/main", r.elsewhere),
            "rebase",
        ),
        ("GIT pull".to_string(), "pull"),
        ("sh -c 'git rebase origin/main'".to_string(), "rebase"),
        ("echo 'git pull' | bash".to_string(), "pull"),
        ("git -c alias.up='pull --rebase' up".to_string(), "pull"),
        (
            "git -c rebase.autoStash=true rebase origin/main".to_string(),
            "rebase",
        ),
    ] {
        let (said_sub, said) = asked(&r, &cmd).unwrap_or_else(|| panic!("{cmd}: allowed"));
        assert_eq!(said_sub, sub, "{cmd}");
        assert!(said.contains("half-updated"), "{cmd}: {said}");
        assert!(said.contains("(`held.toml`)"), "{cmd}: {said}");
    }
}

#[test]
fn git_somewhere_else_and_the_way_back_in_the_root_are_left_alone() {
    let r = a_root();
    let (root, elsewhere) = (&r.root, &r.elsewhere);
    for cmd in [
        format!("git -C {elsewhere} pull"),
        format!("cd {elsewhere} && git rebase origin/main"),
        "git status".to_string(),
        "git fetch origin".to_string(),
        "git rebase --abort".to_string(),
        "git reset --hard HEAD".to_string(),
        "git restore --source=HEAD --worktree -- README.md docs".to_string(),
        "git restore --source=HEAD --worktree --pathspec-from-file=/tmp/list.z --pathspec-file-nul"
            .to_string(),
        "git checkout HEAD -- README.md".to_string(),
        "git checkout origin/main -- README.md".to_string(),
        "echo git pull".to_string(),
        "grep -r 'git rebase' docs".to_string(),
        format!("ls {root}"),
    ] {
        assert_eq!(asked(&r, &cmd), None, "{cmd}");
    }
}

#[test]
fn a_restore_in_the_root_that_names_the_denied_file_or_reaches_past_sight_is_refused() {
    let r = a_root();
    // `held.toml` stands for a later-code name here, so the path's own name does not deny it;
    // a manifest's does.
    let (sub, said) = asked(
        &r,
        "git restore --source=HEAD --worktree -- README.md purlis.toml",
    )
    .expect("refused");
    assert_eq!(sub, "restore");
    assert!(
        said.ends_with(&format!(
            "`git -C {} restore --source=HEAD --worktree -- purlis.toml`.",
            r.root
        )),
        "{said}"
    );
    let (sub, said) = asked(&r, "git checkout origin/main -- docs").expect("refused");
    assert_eq!(sub, "checkout");
    assert!(
        said.contains("from another commit ('origin/main')"),
        "{said}"
    );
    // A hook told no folder reads a path from the root alone, never from its own.
    let from_nowhere = |cmd: &str| refusal_where(cmd, "", &r.root, None, &stand_in);
    let git = format!("git -C {}", r.root);
    assert_eq!(
        from_nowhere(&format!("{git} checkout origin/main -- README.md")),
        None
    );
    assert!(from_nowhere(&format!("{git} checkout origin/main -- Cargo.toml")).is_some());
}

#[test]
fn a_root_holding_nothing_the_sandbox_denies_and_a_chat_below_it_are_not_answered() {
    let r = a_root();
    let nothing = |_: &Path| Vec::new();
    assert_eq!(
        refusal_where("git rebase origin/main", &r.root, &r.root, None, &nothing),
        None
    );
    // A chat in a workspace stops on git's lock file before a file is written: the
    // project-git arm's to explain.
    let below = format!("{}/workspaces/alpha", r.root);
    assert_eq!(
        refusal_where(
            "git -C ../.. rebase origin/main",
            &below,
            &r.root,
            Some(Path::new(&below)),
            &stand_in
        ),
        None
    );
    // A folder nobody named is taken to be one that may write the root.
    assert!(refusal_where("git pull", &r.root, &r.root, None, &stand_in).is_some());
}

#[test]
fn a_directory_the_guard_cannot_name_may_be_the_root_and_a_loose_cd_is_told_to_join() {
    let r = a_root();
    let elsewhere = &r.elsewhere;
    for cmd in [
        "git -C \"$REPO\" pull".to_string(),
        format!("cd {elsewhere} && cd \"$REPO\" && git rebase origin/main"),
    ] {
        let (_, said) = refusal_where(&cmd, elsewhere, &r.root, None, &stand_in)
            .unwrap_or_else(|| panic!("{cmd}: allowed"));
        assert!(
            said.starts_with("purlis cannot tell which repository this command acts on"),
            "{cmd}: {said}"
        );
        assert!(
            said.contains("so it may be the project's root folder"),
            "{cmd}: {said}"
        );
        assert!(said.contains("half-updated"), "{cmd}: {said}");
    }
    // A `cd` that may have failed leaves the shell in the root.
    let (_, said) = asked(&r, &format!("cd {elsewhere}; git pull")).expect("refused");
    assert!(said.contains("join it with `&&`"), "{said}");
}

fn git(dir: &Path, args: &[&str]) {
    let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
    full.extend_from_slice(args);
    let r = crate::testgit::run(dir, &full);
    assert!(r.ok(), "git {args:?} failed: {}", r.err);
}

/// A root that is a repository: on `main`, with a branch `feature` one commit on, a tracked
/// `README.md`, and an alias for an update.
fn a_repository() -> Root {
    let r = a_root();
    let root = Path::new(&r.root);
    // Its git directory beside it, not in it: a chat's sandbox denies writing `.git/config`,
    // and this suite runs inside one too.
    let git_dir = format!("{}.git", r.root);
    git(
        root,
        &[
            "init",
            "-q",
            "-b",
            "main",
            "--separate-git-dir",
            &git_dir,
            ".",
        ],
    );
    git(root, &["add", "README.md", "held.toml"]);
    git(root, &["commit", "-q", "-m", "one"]);
    git(root, &["branch", "same"]);
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(root.join("README.md"), "two\n").unwrap();
    git(root, &["commit", "-q", "-a", "-m", "two"]);
    git(root, &["checkout", "-q", "main"]);
    git(root, &["config", "alias.zzup", "pull --rebase"]);
    git(
        root,
        &["config", "alias.zzback", "restore --source=HEAD --worktree"],
    );
    r
}

#[test]
fn on_a_repository_git_is_asked_what_only_it_can_say() {
    let r = a_repository();
    // An alias is followed to the command it runs.
    let (sub, _) = asked(&r, "git zzup").expect("refused");
    assert_eq!(sub, "pull");
    // A folder inside the root's repository is the root's.
    let (sub, _) = asked(&r, &format!("cd {}/docs && git merge feature", r.root)).expect("refused");
    assert_eq!(sub, "merge");
    assert_eq!(asked(&r, "git zzback -- README.md"), None);
    // The root put back on the branch it is on writes no file; the branch guard's remedy
    // stays runnable. A branch at the same commit writes none either.
    assert_eq!(asked(&r, "git checkout main"), None);
    assert_eq!(asked(&r, "git switch main"), None);
    assert_eq!(
        asked(&r, "git switch same"),
        None,
        "the branch guard's to refuse"
    );
    // A tracked path is a restore out of the index.
    assert_eq!(asked(&r, "git checkout README.md"), None);
    assert_eq!(asked(&r, "git checkout HEAD README.md"), None);
    // From another commit: a file in sight runs, a folder does not.
    assert_eq!(asked(&r, "git checkout feature README.md"), None);
    let (sub, said) = asked(&r, "git checkout feature docs").expect("refused");
    assert_eq!(sub, "checkout");
    assert!(said.contains("from another commit ('feature')"), "{said}");
}

/// The root on another branch than its default: putting it back rewrites the tree, so the
/// sandbox would stop that partway too, and it is refused here though the branch guard allows
/// it.
#[test]
fn putting_the_root_back_on_its_default_branch_is_refused_where_that_rewrites_the_tree() {
    let r = a_repository();
    git(Path::new(&r.root), &["checkout", "-q", "feature"]);
    assert_eq!(
        planeroot::plane_root_branch_reason("git checkout main", &r.root, &r.root),
        None
    );
    let (sub, said) = asked(&r, "git checkout main").expect("refused");
    assert_eq!(sub, "checkout");
    assert!(said.contains("half-updated"), "{said}");
    assert_eq!(
        asked(&r, "git checkout feature"),
        None,
        "the branch guard's to refuse"
    );
}

/// An earlier git command on the line that changes the repository leaves what HEAD is at, and
/// what a word names, unknown when a later command runs: the update is refused as one that may
/// rewrite the tree. Alone, it is measured as before.
#[test]
fn an_earlier_git_command_that_changes_the_repository_leaves_the_tree_unmeasured() {
    let r = a_repository();
    assert_eq!(asked(&r, "git checkout main"), None);
    assert_eq!(asked(&r, "git checkout README.md"), None);
    // Putting a file back changes nothing the next command is measured by.
    assert_eq!(asked(&r, "git add README.md && git checkout main"), None);
    for cmd in [
        "git commit -q -m x && git checkout main",
        "git commit -q -m x && git checkout README.md",
    ] {
        let (sub, said) = asked(&r, cmd).expect(cmd);
        assert_eq!(sub, "checkout");
        assert!(said.contains("half-updated"), "{cmd:?}: {said}");
    }
}
