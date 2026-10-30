// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What A3 and A3b answer on a real repository, and what they answer where the Python raises.
//!
//! The recorded differential (`fixtures/corpora/planeroot-*`) is the arbiter of fidelity; these are
//! the rules stated once each, on a fixture small enough to read, plus the three inputs the
//! differential cannot arbitrate because the oracle raises on them (charter#1178).
//!
//! Every repository here is made through [`crate::testgit`]: the runner keeps `HOME`, and a signer
//! in the operator's global config would otherwise be asked to sign fixture commits.

use super::*;

fn git(dir: &std::path::Path, args: &[&str]) {
    let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
    full.extend_from_slice(args);
    let r = crate::testgit::run(dir, &full);
    assert!(r.ok(), "git {args:?} failed: {}", r.err);
}

/// A plane root with an upstream, one unpushed commit, a tracked `README`, a branch `feature`,
/// a name that is both a tracked path and a branch (`both`), and repo aliases that move HEAD.
struct Fixture {
    _dir: tempfile::TempDir,
    base: String,
    root: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::pypath::realpath(dir.path().to_str().unwrap());
    let up = format!("{base}/up.git");
    let root = format!("{base}/plane");
    std::fs::create_dir_all(&root).unwrap();
    let b = std::path::Path::new(&base);
    let r = std::path::Path::new(&root);
    git(b, &["init", "-q", "--bare", "-b", "main", &up]);
    git(r, &["init", "-q", "-b", "main", "."]);
    std::fs::write(r.join("README"), "r\n").unwrap();
    std::fs::write(r.join("both"), "b\n").unwrap();
    git(r, &["add", "README", "both"]);
    git(r, &["commit", "-q", "-m", "one"]);
    git(r, &["branch", "feature"]);
    git(r, &["branch", "both"]);
    git(r, &["remote", "add", "origin", &up]);
    git(r, &["push", "-q", "-u", "origin", "main"]);
    git(r, &["remote", "set-head", "origin", "main"]);
    git(r, &["commit", "-q", "--allow-empty", "-m", "unpushed"]);
    git(r, &["config", "alias.zzco", "checkout"]);
    git(r, &["config", "alias.zzck", "zzco"]);
    git(r, &["config", "alias.zzwipe", "reset --hard"]);
    git(r, &["config", "alias.zzsh", "!sh -c 'echo x'"]);
    git(r, &["config", "alias.zzgco", "!git checkout"]);
    Fixture {
        _dir: dir,
        base,
        root,
    }
}

fn branch(f: &Fixture, cmd: &str) -> Option<String> {
    plane_root_branch_reason(cmd, &f.root, &f.root)
}

fn reset(f: &Fixture, cmd: &str) -> Option<String> {
    plane_root_reset_reason(cmd, &f.root, &f.root)
}

#[test]
fn a_branch_switch_in_the_root_is_refused_and_the_remedy_is_not() {
    let f = fixture();
    let said = branch(&f, "git checkout feature").unwrap();
    assert!(
        said.starts_with("would switch to 'feature' in the PLANE ROOT. "),
        "{said}"
    );
    assert!(said.ends_with(
        "`git checkout main` — putting the root back on its default branch — is always allowed."
    ));
    assert_eq!(branch(&f, "git checkout main"), None);
    assert_eq!(branch(&f, "git switch -q main"), None);
    // A detach that names the default branch is not the remedy.
    assert!(
        branch(&f, "git checkout --detach main")
            .unwrap()
            .starts_with("would detach HEAD at 'main'")
    );
}

/// #3798: a chat refused a branch move in the root is sent to a branch folder of the clone it
/// already has, `purlis worktree add`, before a new workspace, which is for a new task: a chat
/// told only `workspace create` made a second workspace the app then refused it. The same tail
/// ends a refusal of a `git` call the guard cannot read.
#[test]
fn a_refused_branch_move_names_purlis_worktree_add_before_a_new_workspace() {
    let f = fixture();
    for cmd in ["git checkout feature", "git checkout --track README"] {
        let said = branch(&f, cmd).unwrap();
        let piece = said
            .find("`purlis worktree add <repo> <piece>`")
            .unwrap_or_else(|| panic!("{cmd}: {said}"));
        let workspace = said
            .find("`purlis workspace create <task>`")
            .unwrap_or_else(|| panic!("{cmd}: {said}"));
        assert!(piece < workspace, "{cmd}: {said}");
        for then in ["`purlis worktree commit`", "`purlis worktree push`"] {
            assert!(said.contains(then), "{cmd}: {then}: {said}");
        }
        // #3865: the commit through purlis is a sandboxed chat's; any other chat uses git.
        let sandboxed = said.find("in a sandboxed chat with `purlis worktree commit`");
        let plain = said.find("in any other chat with plain `git commit` and `git push`");
        assert!(sandboxed.is_some() && plain.is_some(), "{cmd}: {said}");
        assert!(sandboxed < plain, "{cmd}: {said}");
    }
}

#[test]
fn a_file_restore_is_not_a_branch_move_and_an_ambiguous_name_is_refused() {
    let f = fixture();
    assert_eq!(branch(&f, "git checkout README"), None);
    assert_eq!(branch(&f, "git checkout -- anything"), None);
    assert_eq!(branch(&f, "git checkout feature README"), None);
    assert!(
        branch(&f, "git checkout both")
            .unwrap()
            .contains("AMBIGUOUS")
    );
    assert!(
        branch(&f, "git checkout nosuch")
            .unwrap()
            .contains("is not a path this tree tracks")
    );
    // `--orphan README` reads like a restore and creates a branch.
    assert!(
        branch(&f, "git checkout --orphan README")
            .unwrap()
            .starts_with("would create 'README'")
    );
    assert!(
        branch(&f, "git checkout -bREADME")
            .unwrap()
            .starts_with("would create 'README'")
    );
    assert!(
        branch(&f, "git checkout --track README")
            .unwrap()
            .contains("'--track'")
    );
}

#[test]
fn an_alias_is_followed_to_the_command_it_runs() {
    let f = fixture();
    assert!(branch(&f, "git zzco feature").is_some());
    assert!(branch(&f, "git zzck feature").is_some());
    assert!(branch(&f, "git zzgco feature").is_some());
    assert!(branch(&f, "git -c alias.zzin=checkout zzin feature").is_some());
    assert_eq!(branch(&f, "git zzsh feature"), None);
    assert_eq!(branch(&f, "git zznotanalias feature"), None);
}

#[test]
fn only_the_root_is_guarded_however_it_is_reached() {
    let f = fixture();
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    assert_eq!(
        plane_root_branch_reason("git checkout feature", &elsewhere, &f.root),
        None
    );
    let via_c = format!("git -C {} checkout feature", f.root);
    assert!(plane_root_branch_reason(&via_c, &elsewhere, &f.root).is_some());
    let via_gd = format!("git --git-dir={}/.git/refs/.. checkout feature", f.root);
    assert!(plane_root_branch_reason(&via_gd, &elsewhere, &f.root).is_some());
    let via_env = format!("export GIT_DIR={}/.git && git checkout feature", f.root);
    assert!(plane_root_branch_reason(&via_env, &elsewhere, &f.root).is_some());
    // #183: the workflow the denial recommends.
    let cd = format!("cd {elsewhere} && git checkout -b x");
    assert_eq!(branch(&f, &cd), None);
}

/// A `cd` that fails leaves the shell where it was (#345). Only `&&` stops the list when it
/// fails, so only a `cd` joined by `&&` may be taken to have moved the commands after it — and
/// only until the `&&` chain ends.
#[test]
fn a_cd_that_fails_leaves_the_later_commands_in_the_root() {
    let f = fixture();
    let nowhere = format!("{}/nowhere", f.base);
    for sep in [";", "||", "&", "\n"] {
        let cmd = format!("cd {nowhere} {sep} git checkout feature");
        assert!(branch(&f, &cmd).is_some(), "{cmd:?}");
        let cmd = format!("cd {nowhere} {sep} git reset --hard origin/main");
        assert!(reset(&f, &cmd).is_some(), "{cmd:?}");
    }
    // `&&` stops the list: the one spelling that moves the later command for certain.
    assert_eq!(
        branch(&f, &format!("cd {nowhere} && git checkout feature")),
        None
    );
    // ...but only as far as the `&&` chain runs: a failed `cd` skips `true` and not what
    // follows the `;` or the `||`.
    for tail in ["; git checkout feature", "|| git checkout feature"] {
        let cmd = format!("cd {nowhere} && true {tail}");
        assert!(branch(&f, &cmd).is_some(), "{cmd:?}");
    }
}

/// Only the SHELL's own `cd` moves the shell: one run in a subshell, a pipeline, or as a
/// program (`env cd`, `/usr/bin/cd`, and `CD`, which a case-insensitive filesystem finds as
/// `/usr/bin/cd`) leaves the next command in the root.
#[test]
fn a_cd_that_does_not_move_the_shell_does_not_move_the_guard() {
    let f = fixture();
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    for cmd in [
        format!("(cd {elsewhere} && true); git checkout feature"),
        format!("(cd {elsewhere}) && git checkout feature"),
        format!("true | cd {elsewhere} && git checkout feature"),
        format!("env cd {elsewhere} && git checkout feature"),
        format!("/usr/bin/cd {elsewhere} && git checkout feature"),
        format!("CD {elsewhere} && git checkout feature"),
        format!("! cd {elsewhere} && git checkout feature"),
    ] {
        assert!(branch(&f, &cmd).is_some(), "{cmd:?}");
    }
    // The shell's own `cd`, however it is spelled, still moves it.
    for cmd in [
        format!("builtin cd {elsewhere} && git checkout feature"),
        format!("command cd {elsewhere} && git checkout feature"),
        format!("FOO=1 cd {elsewhere} && git checkout feature"),
        format!("pushd {elsewhere} && git checkout feature"),
    ] {
        assert_eq!(branch(&f, &cmd), None, "{cmd:?}");
    }
}

/// Where a `cd` goes is read the way the shell reads it: `pushd` is a `cd`, `~` is `$HOME`, a
/// `..` after a symlink is taken logically as well as physically, a wrapper's chdir flag moves
/// git, and a destination the guard cannot read (`$VAR`, a glob, `cd -`, `popd`) may be the root.
#[test]
fn a_cd_is_followed_to_where_the_shell_goes() {
    let f = fixture();
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    let from_elsewhere = |cmd: &str| plane_root_branch_reason(cmd, &elsewhere, &f.root);
    for cmd in [
        format!("pushd {} && git checkout feature", f.root),
        "cd $PLANE && git checkout feature".to_string(),
        format!("cd {}/pla* && git checkout feature", f.base),
        format!("cd {} && cd - && git checkout feature", f.root),
        "popd && git checkout feature".to_string(),
        format!("env -C {} git checkout feature", f.root),
        format!("sudo --chdir={} git checkout feature", f.root),
    ] {
        assert!(from_elsewhere(&cmd).is_some(), "{cmd:?}");
    }
    // `cd` reads `a/link/..` logically: back in `a`, wherever the link points.
    #[cfg(unix)]
    {
        let link = format!("{}/away", f.root);
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(from_elsewhere(&format!("cd {link}/.. && git checkout feature")).is_some());
    }
    // `~` is the home directory: reach the root from it through `..`.
    if let Ok(home) = std::env::var("HOME")
        && home.starts_with('/')
    {
        let ups = "../".repeat(home.trim_end_matches('/').matches('/').count());
        let cmd = format!("cd ~/{ups}{} && git checkout feature", &f.root[1..]);
        assert!(from_elsewhere(&cmd).is_some(), "{cmd:?}");
    }
}

/// `coproc` runs the command after it, named or not, and busybox the applet it is handed: a
/// branch move behind either is the branch move it is (#1325).
#[test]
fn a_branch_move_behind_coproc_or_busybox_is_read() {
    let f = fixture();
    for cmd in [
        "coproc git checkout feature",
        "coproc G { git checkout feature; }",
        "coproc env git checkout feature",
        "busybox env git checkout feature",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &f.root, &f.root).is_some(),
            "{cmd:?}"
        );
    }
    assert_eq!(
        plane_root_branch_reason("coproc git status", &f.root, &f.root),
        None
    );
}

/// A root and a directory beside it, with no repository in either: what the walk reads off the
/// line needs no git to ask.
fn bare_root() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::pypath::realpath(dir.path().to_str().unwrap());
    let root = format!("{base}/plane");
    let elsewhere = format!("{base}/elsewhere");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    (dir, root, elsewhere)
}

/// A git a shell is handed in a string runs where that shell runs, with what the line exported
/// to it: `sh -c`, any shell that runs a string, `eval`, a here-string, one inside another. The
/// walk reads it as a line of its own (#1325). A string that moves no branch stands aside, and a
/// `cd` inside `sh -c` moves only that shell.
#[test]
fn a_branch_move_in_a_string_a_shell_runs_is_read() {
    let (_dir, root, elsewhere) = bare_root();
    let moves = |cmd: &str, cwd: &str| {
        plane_root_git(cmd, cwd, &root)
            .iter()
            .any(|inv| BRANCH_MOVERS.contains(&inv.sub.as_str()))
    };
    for cmd in [
        "sh -c 'git checkout feature'",
        "bash -c \"git switch -c x\"",
        "env bash -lc 'git checkout -b x'",
        "mksh -c 'git checkout feature'",
        "fish -c 'true; and git checkout feature'",
        "eval git checkout feature",
        "eval 'git switch -c x'",
        "bash <<< 'git checkout feature'",
        "sh -c \"sh -c 'git checkout feature'\"",
        "sh -c 'cd /; cd - && git checkout feature'",
        "true && sh -c 'git status; git checkout feature' | cat",
    ] {
        assert!(moves(cmd, &root), "from the root: {cmd:?}");
        assert!(
            plane_root_branch_reason(cmd, &root, &root).is_some(),
            "from the root: {cmd:?}"
        );
    }
    // From beside the root, the string still reaches it wherever the line sends the shell.
    for cmd in [
        format!("cd {root} && sh -c 'git checkout feature'"),
        format!("env -C {root} sh -c 'git checkout feature'"),
        format!("sh -c 'cd {root} && git checkout feature'"),
        format!("sh -c 'git -C {root} checkout feature'"),
        format!("eval cd {root}; git checkout feature"),
        format!("ln -s {root} here && sh -c 'git -C here checkout feature'"),
        format!("sh -c 'ln -s {root} here' && git -C here checkout feature"),
    ] {
        assert!(moves(&cmd, &elsewhere), "from beside it: {cmd:?}");
    }
    for cmd in [
        "sh -c 'git status'",
        "sh -c 'echo git checkout feature'",
        "eval git log",
    ] {
        assert!(!moves(cmd, &root), "{cmd:?}");
    }
    for cmd in [
        "sh -c 'git checkout feature'".to_owned(),
        format!("sh -c 'cd {root}'; git checkout feature"),
    ] {
        assert!(!moves(&cmd, &elsewhere), "from beside it: {cmd:?}");
    }
}

/// A string handed on deeper than the guards read is refused unread where it may name git, and
/// stands aside where it cannot (#1325).
#[test]
fn a_string_handed_on_past_the_cap_is_refused_where_it_may_be_git() {
    let (_dir, root, _elsewhere) = bare_root();
    let deep = |inner: &str| {
        (0..=crate::guardcaps::MAX_NESTING).fold(inner.to_owned(), |inner, _| {
            format!("eval {}", shellwrap_quote(&inner))
        })
    };
    let said = plane_root_branch_reason(&deep("git log"), &root, &root).unwrap();
    assert!(said.contains("too deeply nested"), "{said}");
    assert_eq!(plane_root_branch_reason(&deep("ls"), &root, &root), None);
}

/// A git a shell is fed on its stdin (a pipe, a heredoc), or a `find -exec` runs, is read as a
/// line of its own, from where that shell or that `find` runs (#1325). fish's keywords are read
/// past in what fish is fed. A pipe into a program that runs nothing, and a git that moves no
/// branch, stand aside.
#[test]
fn a_branch_move_fed_to_a_shell_or_run_by_find_is_read() {
    let (_dir, root, elsewhere) = bare_root();
    let moves = |cmd: &str, cwd: &str| {
        plane_root_git(cmd, cwd, &root)
            .iter()
            .any(|inv| BRANCH_MOVERS.contains(&inv.sub.as_str()))
    };
    for cmd in [
        "echo 'git checkout feature' | sh",
        "printf 'git switch -c x\\n' | bash",
        "echo 'git checkout feature' | bash -s",
        "echo 'git checkout -b x' | env sh",
        "echo 'git checkout feature' | . /dev/stdin",
        "{ echo 'git status'; echo 'git checkout feature'; } | sh",
        "echo 'true; and git checkout feature' | fish",
        "printf 'not git switch -c x' | fish",
        "sh <<'EOF'\ngit checkout feature\nEOF",
        "bash <<EOF\ngit switch -c x\nEOF",
        "fish <<'EOF'\ntrue; and git checkout feature\nEOF",
        "find . -maxdepth 0 -exec git checkout feature \\;",
        "find . -maxdepth 0 -exec sh -c 'git checkout feature' \\;",
        "find . -maxdepth 0 -exec env git switch -c x +",
    ] {
        assert!(moves(cmd, &root), "from the root: {cmd:?}");
    }
    // From beside the root, what the shell or `find` is fed reaches it wherever they run.
    for cmd in [
        format!("cd {root} && echo 'git checkout feature' | sh"),
        format!("echo 'git checkout feature' | env -C {root} sh"),
        format!("echo 'cd {root} && git checkout feature' | sh"),
        format!("sh <<'EOF'\ncd {root} && git checkout feature\nEOF"),
        format!("find {root} -maxdepth 0 -exec git -C {{}} checkout feature \\;"),
        format!("cd {root} && find . -exec git checkout feature \\;"),
        "find / -execdir git checkout feature \\;".to_owned(),
    ] {
        assert!(moves(&cmd, &elsewhere), "from beside it: {cmd:?}");
    }
    for cmd in [
        "echo 'git checkout feature' | cat",
        "echo 'git status' | sh",
        "echo 'echo git checkout feature' | sh",
        "find . -exec git status \\;",
        "find . -name 'git checkout feature'",
    ] {
        assert!(!moves(cmd, &root), "{cmd:?}");
    }
    assert!(!moves("echo 'git checkout feature' | sh", &elsewhere));
}

/// What pipes feed shells is read once for each way it may be printed, and those readings
/// nest: a line that would multiply them level on level is read up to a budget and refused
/// unread past it where it may name git, quickly (#1325).
#[test]
fn readings_that_multiply_level_on_level_are_bounded() {
    let (_dir, root, _elsewhere) = bare_root();
    let nested = |inner: &str| {
        (0..crate::guardcaps::MAX_NESTING).fold(inner.to_owned(), |inner, _| {
            format!("printf %s {} | fish | sh", shellwrap_quote(&inner))
        })
    };
    let began = std::time::Instant::now();
    assert!(plane_root_branch_reason(&nested("git checkout feature"), &root, &root).is_some());
    assert_eq!(plane_root_branch_reason(&nested("ls"), &root, &root), None);
    assert!(
        began.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        began.elapsed()
    );
}

/// An `export` an `eval` runs is made in this shell, so it reaches every segment after it, and
/// every string those hand on, as one the line made itself (#1325). One a `sh -c` makes does
/// not outlive that shell. An export the line makes after it still counts.
#[test]
fn an_export_made_inside_eval_carries_to_the_rest_of_the_line() {
    let (_dir, root, elsewhere) = bare_root();
    std::fs::create_dir_all(format!("{root}/.git")).unwrap();
    let moves = |cmd: &str| {
        plane_root_git(cmd, &elsewhere, &root)
            .iter()
            .any(|inv| BRANCH_MOVERS.contains(&inv.sub.as_str()))
    };
    // The bare root reaches git through `GIT_DIR` as the fixture's does.
    assert!(moves(&format!(
        "export GIT_DIR={root}/.git; git checkout feature"
    )));
    for cmd in [
        format!("eval export GIT_DIR={root}/.git; git checkout feature"),
        format!("eval 'export GIT_WORK_TREE={root}' && git switch -c x"),
        format!("eval \"declare -x GIT_DIR={root}/.git\"; git checkout feature"),
        format!("eval export GIT_DIR={root}/.git; sh -c 'git checkout feature'"),
        format!("eval \"eval export GIT_DIR={root}/.git\"; git checkout feature"),
        format!("eval 'set -a; GIT_DIR={root}/.git'; git checkout feature"),
        format!("eval GIT_DIR={root}/.git; export GIT_DIR; git checkout feature"),
        format!(
            "eval export GIT_DIR={elsewhere}/.git; export GIT_DIR={root}/.git; \
             git checkout feature"
        ),
    ] {
        assert!(moves(&cmd), "{cmd:?}");
    }
    for cmd in [
        format!("sh -c 'export GIT_DIR={root}/.git'; git checkout feature"),
        format!("eval export GIT_DIR={root}/.git; git status"),
        "eval export GIT_DIR=/nowhere/.git; git checkout feature".to_owned(),
    ] {
        assert!(!moves(&cmd), "{cmd:?}");
    }
}

/// Under `set -a` every bare assignment is exported, one an `eval` makes as well: the `eval` runs
/// in this shell, under its options. A `set -a` an `eval` runs is this shell's from there on.
/// A shell handed a string does not take the option from the line, nor give it back (#1325).
#[test]
fn an_assignment_an_eval_makes_under_set_a_is_exported() {
    let (_dir, root, elsewhere) = bare_root();
    std::fs::create_dir_all(format!("{root}/.git")).unwrap();
    let moves = |cmd: &str| {
        plane_root_git(cmd, &elsewhere, &root)
            .iter()
            .any(|inv| BRANCH_MOVERS.contains(&inv.sub.as_str()))
    };
    for cmd in [
        format!("set -a; eval GIT_DIR={root}/.git; git checkout feature"),
        format!("set -o allexport; eval 'GIT_DIR={root}/.git'; git checkout feature"),
        format!("set -ax && eval \"GIT_DIR={root}/.git\" && git switch -c x"),
        format!("set -a; eval 'GIT_DIR={root}/.git; git checkout feature'"),
        format!("set -a; eval \"eval GIT_DIR={root}/.git\"; git checkout feature"),
        format!("set -a; eval GIT_DIR={root}/.git; sh -c 'git checkout feature'"),
        format!("eval 'set -a'; GIT_DIR={root}/.git; git checkout feature"),
        format!("eval set -a; eval GIT_DIR={root}/.git; git checkout feature"),
        format!("eval 'eval set -a'; GIT_DIR={root}/.git; git checkout feature"),
    ] {
        assert!(moves(&cmd), "{cmd:?}");
    }
    for cmd in [
        format!("eval GIT_DIR={root}/.git; git checkout feature"),
        format!("set -a; sh -c 'GIT_DIR={root}/.git'; git checkout feature"),
        format!("sh -c 'set -a'; GIT_DIR={root}/.git; git checkout feature"),
        format!("set -a; eval GIT_DIR={root}/.git; git status"),
        "set -a; eval GIT_DIR=/nowhere/.git; git checkout feature".to_owned(),
    ] {
        assert!(!moves(&cmd), "{cmd:?}");
    }
}

/// What a shell is fed, or a `find -exec` runs, in the root is answered as the same command
/// typed bare, and so is a git after an `eval` that exported where it acts (#1325).
#[test]
fn what_a_shell_is_fed_in_the_root_gets_the_bare_commands_answer() {
    let f = fixture();
    assert!(
        branch(&f, "echo 'git checkout feature' | sh")
            .unwrap()
            .starts_with("would switch to 'feature' in the PLANE ROOT. ")
    );
    assert_eq!(branch(&f, "echo 'git checkout main' | sh"), None);
    assert!(reset(&f, "echo 'git reset --hard origin/main' | bash").is_some());
    assert!(reset(&f, "sh <<'EOF'\ngit reset --hard origin/main\nEOF").is_some());
    assert!(
        reset(
            &f,
            "find . -maxdepth 0 -exec git reset --hard origin/main \\;"
        )
        .is_some()
    );
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    let exported = format!("eval export GIT_DIR={}/.git; git checkout feature", f.root);
    assert!(plane_root_branch_reason(&exported, &elsewhere, &f.root).is_some());
}

/// `inner` as one single-quoted shell word.
fn shellwrap_quote(inner: &str) -> String {
    format!("'{}'", inner.replace('\'', "'\\''"))
}

/// What a string a shell runs does in the root is answered as the same command typed bare: a
/// branch move refused, the remedy allowed, a reset that loses commits refused (#1325).
#[test]
fn a_string_a_shell_runs_in_the_root_gets_the_bare_commands_answer() {
    let f = fixture();
    assert!(
        branch(&f, "sh -c 'git checkout feature'")
            .unwrap()
            .starts_with("would switch to 'feature' in the PLANE ROOT. ")
    );
    assert_eq!(branch(&f, "sh -c 'git checkout main'"), None);
    assert_eq!(branch(&f, "bash -c 'git checkout README'"), None);
    assert!(reset(&f, "sh -c 'git reset --hard origin/main'").is_some());
    assert!(reset(&f, "eval git zzwipe origin/main").is_some());
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    let via_env = format!("GIT_DIR={}/.git sh -c 'git checkout feature'", f.root);
    assert!(plane_root_branch_reason(&via_env, &elsewhere, &f.root).is_some());
    let exported = format!(
        "export GIT_DIR={}/.git; bash -c 'git checkout feature'",
        f.root
    );
    assert!(plane_root_branch_reason(&exported, &elsewhere, &f.root).is_some());
    assert_eq!(
        plane_root_branch_reason("sh -c 'git checkout feature'", &elsewhere, &f.root),
        None
    );
}

/// A `cd` the line itself can redirect is not taken on trust: a `pushd -n` that moves nothing, a
/// `~` or `CDPATH` the line sets, a `cd` the line redefines or disables, and zsh's two-operand
/// `cd old new` all leave the later command possibly in the root.
#[test]
fn a_cd_the_line_can_redirect_is_not_taken_on_trust() {
    let f = fixture();
    let clone = format!("{}/workspaces/w/clone", f.root);
    std::fs::create_dir_all(&clone).unwrap();
    git(
        std::path::Path::new(&clone),
        &["init", "-q", "-b", "main", "."],
    );
    for cmd in [
        "pushd -n workspaces/w/clone && git checkout feature".to_string(),
        format!("HOME={}; cd ~ && git checkout feature", f.root),
        format!("HOME={} cd && git checkout feature", f.root),
        format!(
            "CDPATH={}; cd workspaces/w/clone && git checkout feature",
            f.base
        ),
        "cd(){ :;}; cd workspaces/w/clone && git checkout feature".to_string(),
        "function cd { :; }; cd workspaces/w/clone && git checkout feature".to_string(),
        "enable -n cd; cd workspaces/w/clone && git checkout feature".to_string(),
        format!("cd {0} {0} && git checkout feature", f.base),
    ] {
        assert!(branch(&f, &cmd).is_some(), "{cmd:?}");
    }
    // The workflow the denial recommends still runs.
    assert_eq!(
        branch(&f, "cd workspaces/w/clone && git checkout -b x"),
        None
    );
}

/// A `cd` joined by `;`, `||`, `&` or a newline leaves the root in play, as before (#345), and
/// the denial now says to join it with `&&` (#1326). A line with no such `cd`, and one whose
/// `cd` `&&` would not make certain (the line redefines it, or it runs in a pipeline), do not
/// get the hint.
#[test]
fn a_cd_not_joined_by_and_and_is_told_to_join_it() {
    let f = fixture();
    for cmd in [
        "cd /nonexistent-dir; git checkout feature",
        "cd /nonexistent-dir || git checkout feature",
        "cd /nonexistent-dir\ngit checkout -b x",
    ] {
        let said = branch(&f, cmd).expect("still refused");
        assert!(said.contains(LOOSE_CD_HINT), "{cmd:?}: {said}");
    }
    let said = reset(&f, "cd /nonexistent-dir; git reset --hard origin/main").expect("refused");
    assert!(said.ends_with(LOOSE_CD_HINT.trim_end()), "{said}");
    for cmd in [
        "git checkout feature",
        "cd(){ :;}; cd /nonexistent-dir; git checkout feature",
        "cd /nonexistent-dir | cat; git checkout feature",
    ] {
        let said = branch(&f, cmd).expect("refused");
        assert!(!said.contains(LOOSE_CD_HINT), "{cmd:?}: {said}");
    }
    assert!(
        !reset(&f, "git reset --hard origin/main")
            .expect("refused")
            .contains(LOOSE_CD_HINT)
    );
}

/// An alias chain too deep to follow is refused with the same hint for a loose `cd` (#1093),
/// and the hint says what `&` does: a backgrounded `cd` never moves the shell.
#[test]
fn an_alias_chain_too_deep_is_told_to_join_a_loose_cd_too() {
    let f = fixture();
    let r = std::path::Path::new(&f.root);
    // zzd0 -> zzd1 -> ... -> zzd5 -> checkout: past MAX_ALIAS_HOPS.
    for hop in 0..=MAX_ALIAS_HOPS {
        git(
            r,
            &[
                "config",
                &format!("alias.zzd{hop}"),
                &format!("zzd{}", hop + 1),
            ],
        );
    }
    git(
        r,
        &[
            "config",
            &format!("alias.zzd{}", MAX_ALIAS_HOPS + 1),
            "checkout",
        ],
    );
    for cmd in [
        "cd /nonexistent-dir; git zzd0 feature",
        "cd /nonexistent-dir & git zzd0 feature",
    ] {
        let said = branch(&f, cmd).expect("refused");
        assert!(
            said.contains("past where purlis follows them"),
            "{cmd:?}: {said}"
        );
        assert!(said.contains(LOOSE_CD_HINT), "{cmd:?}: {said}");
    }
    let said = branch(&f, "git zzd0 feature").expect("refused");
    assert!(said.contains("past where purlis follows them"), "{said}");
    assert!(!said.contains(LOOSE_CD_HINT), "{said}");
    assert!(
        LOOSE_CD_HINT.contains("one sent to the background by `&` never moves the shell"),
        "{LOOSE_CD_HINT}"
    );
}

/// git is recognised by what runs, not how it is spelled: on APFS and NTFS `GIT` runs git (#346).
#[test]
fn a_git_spelled_in_capitals_is_git() {
    let f = fixture();
    for cmd in [
        "GIT checkout feature",
        "Git checkout feature",
        "/usr/bin/GIT checkout feature",
    ] {
        assert!(branch(&f, cmd).is_some(), "{cmd}");
    }
    assert!(reset(&f, "GIT reset --hard origin/main").is_some());
    assert!(reset(&f, "Git zzwipe origin/main").is_some());
    // The shell takes the quotes off before it runs the word, and the walk reads the word it runs.
    assert!(branch(&f, "g''it checkout feature").is_some());
    assert!(reset(&f, "g''it reset --hard origin/main").is_some());
    assert!(reset(&f, "G\\IT reset --hard origin/main").is_some());
}

/// `git` written with ANSI-C escapes, as a locale string, or split by a backslash-newline is
/// the `git` the shell runs, and the reset guard's `git` filter lets each of them through to the
/// walk that decides.
#[test]
fn a_git_the_shell_spells_out_of_quoting_is_git() {
    let f = fixture();
    for cmd in [
        "$'\\x67it' checkout feature",
        "$'\\147\\151\\164' checkout feature",
        "$\"git\" checkout feature",
        "g\\\nit checkout feature",
    ] {
        assert!(branch(&f, cmd).is_some(), "{cmd:?}");
    }
    assert!(reset(&f, "$'\\x67it' reset --hard origin/main").is_some());
    assert!(reset(&f, "$\\\n'\\x67it' reset --hard origin/main").is_some());
    // A `!` alias is handed to `sh -c`, which decodes the same quoting.
    assert!(branch(&f, "git -c \"alias.zzq=!$'\\x67it' checkout\" zzq feature").is_some());
}

/// An alias defined in one case and used in another is the same alias: git folds the key.
#[test]
fn an_inline_alias_is_found_in_any_case() {
    let f = fixture();
    for cmd in [
        "git -c alias.ZZIN=checkout zzin feature",
        "git -c alias.zzin=checkout ZZIN feature",
        "git -c alias.ZzIn=checkout zZiN feature",
        "git -c 'alias.zzb=!GIT checkout' zzb feature",
    ] {
        assert!(branch(&f, cmd).is_some(), "{cmd}");
    }
}

/// The root is recognised by WHAT it is, not how its path is spelled (#346): a re-cased path on
/// a case-insensitive filesystem and macOS's `/System/Volumes/Data` firmlink both name it, and so
/// does any directory git would discover the root's repository from — a subdirectory of it, or
/// one not made yet — while a repository of its own inside it is not the root.
#[test]
fn the_root_is_recognised_by_what_it_is_not_how_it_is_spelled() {
    let f = fixture();
    let elsewhere = format!("{}/elsewhere", f.base);
    std::fs::create_dir_all(&elsewhere).unwrap();
    let from_elsewhere = |cmd: &str| plane_root_branch_reason(cmd, &elsewhere, &f.root);

    let recased = format!("{}/PLANE", f.base);
    if std::fs::metadata(&recased).is_ok() {
        assert!(from_elsewhere(&format!("git -C {recased} checkout feature")).is_some());
        assert!(from_elsewhere(&format!("cd {recased} && git checkout feature")).is_some());
    }
    let firmlinked = format!("/System/Volumes/Data{}", f.root);
    if std::fs::metadata(&firmlinked).is_ok() {
        assert!(from_elsewhere(&format!("git -C {firmlinked} checkout feature")).is_some());
    }

    std::fs::create_dir_all(format!("{}/docs", f.root)).unwrap();
    for cmd in [
        format!("git -C {}/docs checkout feature", f.root),
        format!("cd {}/docs && git checkout feature", f.root),
        format!(
            "mkdir {0}/new && cd {0}/new && git checkout feature",
            f.root
        ),
        format!("git -C {}/docs reset --hard origin/main", f.root),
    ] {
        let said = if cmd.contains("reset") {
            plane_root_reset_reason(&cmd, &elsewhere, &f.root)
        } else {
            from_elsewhere(&cmd)
        };
        assert!(said.is_some(), "{cmd:?}");
    }

    let clone = format!("{}/workspaces/w/clone", f.root);
    std::fs::create_dir_all(&clone).unwrap();
    git(
        std::path::Path::new(&clone),
        &["init", "-q", "-b", "main", "."],
    );
    assert_eq!(
        branch(&f, "cd workspaces/w/clone && git checkout -b x"),
        None
    );
    assert_eq!(
        from_elsewhere(&format!("git -C {clone} checkout -b x")),
        None
    );
}

#[test]
fn a_clone_whose_config_names_the_root_as_its_work_tree_is_the_root() {
    let f = fixture();
    let clone = format!("{}/ws/clone", f.base);
    std::fs::create_dir_all(format!("{clone}/.git")).unwrap();
    std::fs::write(
        format!("{clone}/.git/config"),
        format!("[core]\n\tworktree = {}\n", f.root),
    )
    .unwrap();
    assert!(plane_root_branch_reason("git checkout feature", &clone, &f.root).is_some());
}

#[test]
fn a_reset_that_destroys_an_unpushed_commit_is_refused_and_one_that_does_not_is_not() {
    let f = fixture();
    let said = reset(&f, "git reset --hard origin/main").unwrap();
    assert!(
        said.starts_with("would delete 1 commit from the PLANE ROOT that is not on origin/main,"),
        "{said}"
    );
    assert!(said.contains(&format!(
        "`git -C {} log --oneline '@{{upstream}}..HEAD'`",
        f.root
    )));
    assert!(reset(&f, "git zzwipe origin/main").is_some());
    assert!(reset(&f, "git reset --soft origin/main").is_some());
    assert_eq!(reset(&f, "git reset --hard HEAD"), None);
    assert_eq!(reset(&f, "git reset --hard"), None);
    assert_eq!(reset(&f, "git reset --hard origin/main -- README"), None);
    assert_eq!(reset(&f, "echo reset --hard origin/main"), None);
}

#[test]
fn the_default_branch_is_the_remotes_answer_first() {
    let f = fixture();
    assert_eq!(default_branch(&f.root).as_deref(), Some("main"));
    let bare = format!("{}/bare", f.base);
    std::fs::create_dir_all(&bare).unwrap();
    git(
        std::path::Path::new(&bare),
        &["init", "-q", "-b", "dev", "."],
    );
    assert_eq!(default_branch(&bare), None);
}

#[test]
fn the_subject_list_only_grows_and_resolves_against_the_shell_directory() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let got = git_target(
        "/nowhere/a",
        &s(&["-C", "..", "-C", "b", "--work-tree", "w"]),
        &[],
    );
    assert_eq!(got, ["/nowhere/a/../b", "/nowhere/a/../b/w"]);
    let got = git_target("/nowhere", &s(&["--git-dir=g"]), &s(&["GIT_WORK_TREE=/t"]));
    assert_eq!(got, ["/nowhere", "/t", "/nowhere/g", "/nowhere/g/.."]);
    assert_eq!(git_target("", &[], &[]), ["."]);
}

#[test]
fn an_option_is_placed_by_its_name_not_its_spelling() {
    assert_eq!(checkout_opt_kind("-bREADME"), OptKind::Create);
    assert_eq!(checkout_opt_kind("-qbREADME"), OptKind::Create);
    assert_eq!(checkout_opt_kind("-fq"), OptKind::Restore);
    assert_eq!(checkout_opt_kind("-dq"), OptKind::Detach);
    assert_eq!(checkout_opt_kind("--orphan=x"), OptKind::Create);
    assert_eq!(checkout_opt_kind("--no-orphan"), OptKind::Create);
    assert_eq!(checkout_opt_kind("--no-quiet"), OptKind::Restore);
    assert_eq!(checkout_opt_kind("--track"), OptKind::Unknown);
    assert_eq!(checkout_opt_kind("-t"), OptKind::Unknown);
}

// ---- where the Python RAISES (charter#1178), pinned here because the differential cannot
// arbitrate an input one side has no answer for.

/// A symlink loop among the subjects: CPython 3.11/3.12 raises `RuntimeError` out of the walk,
/// which is an ALLOW for the whole line. This resolves it as 3.14 does — not the root — and
/// goes on to the later segment, which it refuses.
#[cfg(unix)]
#[test]
fn a_symlink_loop_among_the_subjects_does_not_stop_the_walk() {
    let f = fixture();
    let lp = format!("{}/loop", f.base);
    std::os::unix::fs::symlink(&lp, &lp).unwrap();
    let cmd = format!("git -C {lp} status; git checkout feature");
    assert!(branch(&f, &cmd).is_some());
    let cmd = format!("git --work-tree={lp} status; git reset --hard origin/main");
    assert!(reset(&f, &cmd).is_some());
}

/// A NUL in `-C`, `--work-tree` or `--git-dir`: the Python raises `ValueError`. Here it is a
/// path that is not the root, and the later segment is still judged.
#[test]
fn a_nul_in_a_subject_does_not_stop_the_walk() {
    let f = fixture();
    for opt in ["-C a\u{0}b", "--work-tree=a\u{0}b", "--git-dir=a\u{0}b"] {
        let cmd = format!("git {opt} status; git checkout feature");
        assert!(branch(&f, &cmd).is_some(), "{opt}");
        let cmd = format!("git {opt} status; git reset --hard origin/main");
        assert!(reset(&f, &cmd).is_some(), "{opt}");
    }
}

/// An operand holding a NUL cannot be put to git: `Unknown`, which keeps the refusal.
#[test]
fn an_operand_git_cannot_be_asked_about_keeps_the_refusal() {
    let f = fixture();
    assert_eq!(
        checkout_operand_kind(&f.root, "a\u{0}b"),
        OperandKind::Unknown
    );
    assert!(
        branch(&f, "git checkout 'a\u{0}b'")
            .unwrap()
            .contains("could not ask git")
    );
}

/// An alias body that is not UTF-8: the Python's strict decode raises `ValueError`, which
/// `_resolve_git_alias` answers by standing aside — so the lossy reading that WOULD resolve it
/// to `checkout` must not happen here either, or the two part company.
#[test]
fn an_alias_body_that_is_not_utf8_is_not_read() {
    let f = fixture();
    let config = format!("{}/.git/config", f.root);
    let mut text = std::fs::read(&config).unwrap();
    text.extend_from_slice(b"[alias]\n\tzznu = checkout \xff\n");
    std::fs::write(&config, text).unwrap();
    assert_eq!(resolve_git_alias(&f.root, "zznu", &[], &[]).0, "zznu");
    assert_eq!(branch(&f, "git zznu feature"), None);
}

#[test]
fn python_int_reads_what_rev_list_writes_and_nothing_else() {
    assert_eq!(py_int("12"), Some(12));
    assert_eq!(py_int("1_0"), Some(10));
    assert_eq!(py_int("-3"), Some(-3));
    assert_eq!(py_int("_1"), None);
    assert_eq!(py_int("1__0"), None);
    assert_eq!(py_int("x"), None);
    assert_eq!(py_int(""), None);
}

/// A plane root, a session directory inside it, and a scratch clone of the root OUTSIDE it — the
/// layout of #1323. With `linked`, each repository keeps its git directory beside it and has a
/// `.git` FILE naming it (`git clone --separate-git-dir`), the layout a scratch clone often has.
struct Scratch {
    _dir: tempfile::TempDir,
    root: String,
    session: String,
    clone: String,
}

fn scratch(linked: bool) -> Scratch {
    scratch_with(linked, linked)
}

/// [`scratch`] with the root's and the clone's layouts chosen apart.
fn scratch_with(root_linked: bool, clone_linked: bool) -> Scratch {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::pypath::realpath(dir.path().to_str().unwrap());
    let root = format!("{base}/plane");
    let clone = format!("{base}/scratch/clone");
    let session = format!("{root}/workspaces/w");
    std::fs::create_dir_all(format!("{base}/scratch")).unwrap();
    let b = std::path::Path::new(&base);
    let r = std::path::Path::new(&root);
    let beside = |repo: &str| format!("--separate-git-dir={repo}.gitdir");
    let mut init = vec!["init", "-q", "-b", "main"];
    let root_gd = beside(&root);
    if root_linked {
        init.push(&root_gd);
    }
    init.push(&root);
    git(b, &init);
    std::fs::write(r.join("README"), "r\n").unwrap();
    git(r, &["add", "README"]);
    git(r, &["commit", "-q", "-m", "one"]);
    git(r, &["branch", "feature"]);
    std::fs::create_dir_all(&session).unwrap();
    let mut cl = vec!["clone", "-q"];
    let clone_gd = beside(&clone);
    if clone_linked {
        cl.push(&clone_gd);
    }
    cl.extend([root.as_str(), clone.as_str()]);
    git(b, &cl);
    Scratch {
        _dir: dir,
        root,
        session,
        clone,
    }
}

/// `~/` followed by the climb from `$HOME` to `/` and then `abs` — the path `abs` spelled from
/// the home directory — or `None` where the test's `HOME` is not absolute.
fn from_home(abs: &str) -> Option<String> {
    let home = std::env::var("HOME").ok().filter(|h| h.starts_with('/'))?;
    let ups = "../".repeat(home.trim_end_matches('/').matches('/').count());
    Some(format!("~/{ups}{}", &abs[1..]))
}

/// #1323: the guard follows a command to the repository it really acts on. `git -C <clone>` and
/// a `cd <clone> &&` earlier in the line act in the clone, from a session standing in the root;
/// the same two spellings aimed at the root are refused from a session standing in the clone.
fn a_command_aimed_at_another_clone_acts_there(linked: bool) {
    let s = scratch(linked);
    let (root, clone) = (&s.root, &s.clone);
    for cmd in [
        format!("git -C {clone} switch -c x"),
        format!("git -C {clone} checkout -b x"),
        format!("git -C {clone} checkout --detach"),
        format!("cd {clone} && git checkout -b x"),
        format!("cd {clone} && git switch -c x"),
        format!("cd {clone} && git fetch -q && git checkout -q -b x 2>&1 | tail -3"),
    ] {
        assert_eq!(
            plane_root_branch_reason(&cmd, &s.session, root),
            None,
            "{cmd:?}"
        );
    }
    for cmd in [
        format!("git -C {root} switch -c x"),
        format!("cd {root} && git checkout -b x"),
        format!("git -C {clone} status && git -C {root} checkout -b x"),
        format!("cd {root}/workspaces/w && git checkout -b x"),
    ] {
        assert!(
            plane_root_branch_reason(&cmd, clone, root).is_some(),
            "{cmd:?}"
        );
    }
}

#[test]
fn a_command_aimed_at_another_clone_acts_there_with_a_git_directory() {
    a_command_aimed_at_another_clone_acts_there(false);
}

#[test]
fn a_command_aimed_at_another_clone_acts_there_with_a_git_file() {
    a_command_aimed_at_another_clone_acts_there(true);
}

/// A `~` in a directory git is pointed at is read the way the shell reads it, as a `cd`'s is:
/// `$HOME`, and the literal name as well, since a quoted `~` stays one.
fn a_home_relative_directory_is_where_the_shell_sends_git(linked: bool) {
    let s = scratch(linked);
    let (Some(to_clone), Some(to_root)) = (from_home(&s.clone), from_home(&s.root)) else {
        return;
    };
    for cmd in [
        format!("git -C {to_clone} switch -c x"),
        format!("cd {to_clone} && git checkout -b x"),
    ] {
        assert_eq!(
            plane_root_branch_reason(&cmd, &s.session, &s.root),
            None,
            "{cmd:?}"
        );
    }
    for cmd in [
        format!("git -C {to_root} switch -c x"),
        format!("GIT_WORK_TREE={to_root} git checkout -b x"),
        format!("git --git-dir {to_root}/.git checkout -b x"),
        format!("env -C {to_root} git checkout -b x"),
        format!("HOME=/elsewhere git -C {to_clone} switch -c x"),
    ] {
        assert!(
            plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some(),
            "{cmd:?}"
        );
    }
}

#[test]
fn a_home_relative_directory_is_where_the_shell_sends_git_with_a_git_directory() {
    a_home_relative_directory_is_where_the_shell_sends_git(false);
}

#[test]
fn a_home_relative_directory_is_where_the_shell_sends_git_with_a_git_file() {
    a_home_relative_directory_is_where_the_shell_sends_git(true);
}

/// A directory git is pointed at that the guard cannot name — the shell expands it later — may
/// be the root, so it is refused from anywhere, a clone of the guard's own included. Fail
/// closed: the guard cannot tell, so it answers as if it were the root.
fn a_directory_the_guard_cannot_name_may_be_the_root(linked: bool) {
    let s = scratch(linked);
    for cmd in [
        "git -C \"$PLANE\" switch -c x".to_string(),
        "git -C $PLANE/ checkout -b x".to_string(),
        "git -C ~operator/plane checkout -b x".to_string(),
        format!("git -C {}/pla* checkout -b x", &s.root[..s.root.len() - 6]),
        "git -C {a,b} checkout -b x".to_string(),
        "git --git-dir=$P/.git checkout -b x".to_string(),
        "git --work-tree \"$P\" checkout -b x".to_string(),
        "GIT_DIR=$P/.git git checkout -b x".to_string(),
        "export GIT_WORK_TREE=$P && git checkout -b x".to_string(),
        "env -C \"$P\" git checkout -b x".to_string(),
    ] {
        assert!(
            plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some(),
            "{cmd:?}"
        );
    }
    // What it cannot name only matters where it names a directory.
    assert_eq!(
        plane_root_branch_reason("git -c x.y=$V switch -c x", &s.clone, &s.root),
        None
    );
}

#[test]
fn a_directory_the_guard_cannot_name_may_be_the_root_with_a_git_directory() {
    a_directory_the_guard_cannot_name_may_be_the_root(false);
}

#[test]
fn a_directory_the_guard_cannot_name_may_be_the_root_with_a_git_file() {
    a_directory_the_guard_cannot_name_may_be_the_root(true);
}

/// A `~` the shell leaves alone names a directory called `~`: quoted (`'~'`, `\~`), after an
/// assignment's `=` under `env` (zsh), and in an attached wrapper flag. The reader takes the
/// quoting off, so such a `~` arrives looking like `$HOME`. From a clone holding a `~` that
/// links to the root, each of these acts on the root and is refused (#1323).
#[cfg(unix)]
fn a_tilde_the_shell_leaves_alone_is_a_directory_named_tilde(linked: bool) {
    let s = scratch(linked);
    std::os::unix::fs::symlink(&s.root, format!("{}/~", s.clone)).unwrap();
    for cmd in [
        "git -C '~' checkout -b x",
        "git -C \\~ switch -c x",
        "env GIT_DIR=~/.git git checkout -b x",
        "env --chdir=~ git checkout -b x",
        "env -C '~' git checkout -b x",
        "sudo -D~ git checkout -b x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &s.clone, &s.root).is_some(),
            "{cmd:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_tilde_the_shell_leaves_alone_is_a_directory_named_tilde_with_a_git_directory() {
    a_tilde_the_shell_leaves_alone_is_a_directory_named_tilde(false);
}

#[cfg(unix)]
#[test]
fn a_tilde_the_shell_leaves_alone_is_a_directory_named_tilde_with_a_git_file() {
    a_tilde_the_shell_leaves_alone_is_a_directory_named_tilde(true);
}

/// A root whose git directory lives beside it (a `.git` file) is reached by naming that
/// directory: `--git-dir=<it>` moves the root's HEAD from anywhere.
#[test]
fn the_roots_own_git_directory_is_the_root_wherever_it_lives() {
    let s = scratch(true);
    let gd = format!("{}.gitdir", s.root);
    for cmd in [
        format!("git --git-dir={gd} checkout -b x"),
        format!("git --git-dir {gd} --work-tree {} switch -c x", s.clone),
        format!("GIT_DIR={gd} git checkout -b x"),
    ] {
        assert!(
            plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some(),
            "{cmd:?}"
        );
    }
}

/// A command refused only because a directory it is sent to cannot be named says so, rather
/// than claiming what it would do in the root; one that reaches the root by name keeps its own
/// sentence.
fn a_directory_the_guard_cannot_name_is_refused_as_unnamed(linked: bool) {
    let s = scratch(linked);
    for cmd in [
        "git -C \"$WT\" switch -c x",
        "cd \"$X\" && git checkout -b x",
        "cd - && git checkout feature",
    ] {
        let said = plane_root_branch_reason(cmd, &s.clone, &s.root).expect(cmd);
        assert!(
            said.starts_with("cannot tell which repository this `git "),
            "{cmd:?}: {said}"
        );
        assert!(said.contains("Spell the path out"), "{said}");
    }
    let said =
        plane_root_branch_reason(&format!("git -C {} switch -c x", s.root), &s.clone, &s.root)
            .unwrap();
    assert!(
        said.starts_with("would create 'x' in the PLANE ROOT"),
        "{said}"
    );
}

#[test]
fn a_directory_the_guard_cannot_name_is_refused_as_unnamed_with_a_git_directory() {
    a_directory_the_guard_cannot_name_is_refused_as_unnamed(false);
}

#[test]
fn a_directory_the_guard_cannot_name_is_refused_as_unnamed_with_a_git_file() {
    a_directory_the_guard_cannot_name_is_refused_as_unnamed(true);
}

/// The layout the live refusals came from: the root has a `.git` directory, and the scratch clone
/// outside it has a `.git` FILE naming a git directory outside both. Pointed at by name, the clone
/// is the clone; pointed at through a variable, it is refused as a directory the guard cannot
/// name, and the denial asks for the path spelled out.
fn a_clone_whose_git_is_a_file_elsewhere(root_linked: bool) {
    let s = scratch_with(root_linked, true);
    let clone = &s.clone;
    for cmd in [
        format!("git -C {clone} switch -c x"),
        format!("git -C {clone} checkout -b x"),
        format!("cd {clone} && git switch -c x"),
    ] {
        assert_eq!(
            plane_root_branch_reason(&cmd, &s.session, &s.root),
            None,
            "{cmd:?}"
        );
    }
    let said = plane_root_branch_reason("git -C \"$C\" switch -c x", &s.session, &s.root).unwrap();
    assert!(said.contains("Spell the path out"), "{said}");
}

#[test]
fn a_clone_whose_git_is_a_file_elsewhere_beside_a_root_with_a_git_directory() {
    a_clone_whose_git_is_a_file_elsewhere(false);
}

#[test]
fn a_clone_whose_git_is_a_file_elsewhere_beside_a_root_with_a_git_file() {
    a_clone_whose_git_is_a_file_elsewhere(true);
}

/// Whether a separated `~` is a directory named `~` is read from the line's quoting, never from
/// the filesystem before the line runs: a `~` the same line makes, quoted or escaped, is the
/// directory git is sent to, and from inside the root that directory is in the root.
fn a_quoted_tilde_the_same_line_makes_is_read_literally(linked: bool) {
    let s = scratch(linked);
    for cmd in [
        "mkdir \\~ && git -C \\~ switch -c x",
        "mkdir -p '~/a' && git -C '~/a' switch -c x",
        "git -C \"~/clone\" checkout -b x",
        "git -C ~'/a' checkout -b x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &s.session, &s.root).is_some(),
            "{cmd:?}"
        );
    }
}

#[test]
fn a_quoted_tilde_the_same_line_makes_is_read_literally_with_a_git_directory() {
    a_quoted_tilde_the_same_line_makes_is_read_literally(false);
}

#[test]
fn a_quoted_tilde_the_same_line_makes_is_read_literally_with_a_git_file() {
    a_quoted_tilde_the_same_line_makes_is_read_literally(true);
}

/// Reading a long run of `-C`s costs what the run is long, not twice as much per `-C`: the hook
/// answers on a deadline, and a guard that timed out would let the command through.
#[test]
fn a_long_run_of_directory_options_is_read_in_time() {
    let s = scratch(true);
    let cmd = format!("git -C '~' {}-C {} switch -c x", "-C . ".repeat(64), s.root);
    let started = std::time::Instant::now();
    assert!(plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some());
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "{took:?}");
}

/// ANSI-C quoting spells a `~` without one (`$'\x7e'`, `$'\176'`), and a `cd` to a quoted `~`
/// goes to the directory named `~` as well as `$HOME`. From a clone holding a `~` that links to
/// the root, and from inside the root after the line makes one, each of these is refused.
#[cfg(unix)]
fn a_tilde_spelled_without_one_or_reached_by_cd_is_read_literally(linked: bool) {
    let s = scratch(linked);
    std::os::unix::fs::symlink(&s.root, format!("{}/~", s.clone)).unwrap();
    for cmd in [
        "git -C $'\\x7e' checkout -b x",
        "git -C $'\\176' switch -c x",
        "cd \\~ && git switch -c x",
        "cd $'\\x7e' && git checkout -b x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &s.clone, &s.root).is_some(),
            "{cmd:?}"
        );
    }
    for cmd in [
        "mkdir $'\\x7e' && git -C $'\\x7e' switch -c x",
        "mkdir \\~ && cd \\~ && git switch -c x",
        "mkdir $'\\x7e' && cd $'\\x7e' && git switch -c x",
        "mkdir '~' && cd '~' && git switch -c x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &s.session, &s.root).is_some(),
            "{cmd:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_tilde_spelled_without_one_or_reached_by_cd_is_read_literally_with_a_git_directory() {
    a_tilde_spelled_without_one_or_reached_by_cd_is_read_literally(false);
}

#[cfg(unix)]
#[test]
fn a_tilde_spelled_without_one_or_reached_by_cd_is_read_literally_with_a_git_file() {
    a_tilde_spelled_without_one_or_reached_by_cd_is_read_literally(true);
}

/// An unquoted command substitution among git's own options — `$(…)` or backticks — names a
/// directory only the shell knows, and the reader spreads its words through git's argv (#1354).
/// From a clone, a branch move or a reset after one is refused as unnamed: neither the directory
/// nor the options after it can be read, and the words do not move the subcommand out of sight.
fn a_substitution_among_gits_options_is_unnamed(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    let parent = &root[..root.rfind('/').unwrap()];
    let r = std::path::Path::new(root);
    // An upstream the root is a commit ahead of, so a reset there would lose one.
    git(
        std::path::Path::new(parent),
        &["init", "-q", "--bare", "-b", "main", "up.git"],
    );
    git(r, &["remote", "add", "origin", &format!("{parent}/up.git")]);
    git(r, &["push", "-q", "-u", "origin", "main"]);
    git(r, &["commit", "-q", "--allow-empty", "-m", "unpushed"]);
    let unnamed = "cannot tell which repository this `git ";
    for (open, close) in [("$(", ")"), ("`", "`")] {
        let sub = |inner: &str| format!("{open}{inner}{close}");
        let echo = sub(&format!("echo {root}"));
        for cmd in [
            format!("git -C {echo} checkout -b x"),
            format!("git -C {echo}/ switch -c x"),
            format!(
                "git -C {} checkout -b x",
                sub("git rev-parse --show-toplevel")
            ),
            format!("git --git-dir={echo}/.git checkout -b x"),
            format!("git --git-dir {echo}/.git switch -c x"),
            format!("git --work-tree={echo} checkout -b x"),
            format!("git --work-tree {echo} checkout --detach"),
            format!("git -c x.y={} -C {root} checkout -b x", sub("echo 1")),
            format!("GIT_DIR={echo}/.git git checkout -b x"),
            format!("env -C {echo} git switch -c x"),
        ] {
            let said = plane_root_branch_reason(&cmd, &s.clone, root).expect(&cmd);
            assert!(said.starts_with(unnamed), "{cmd:?}: {said}");
        }
        for cmd in [
            format!("git -C {echo} reset --hard HEAD~1"),
            format!("git --git-dir={echo}/.git reset --hard HEAD~1"),
            format!("git --work-tree {echo} reset --hard HEAD~1"),
        ] {
            let said = plane_root_reset_reason(&cmd, &s.clone, root).expect(&cmd);
            assert!(said.starts_with(unnamed), "{cmd:?}: {said}");
        }
        // A substitution after the subcommand is an argument, and the clone stays the clone.
        let after = format!(
            "git -C {} checkout -b x {}",
            s.clone,
            sub("git rev-parse HEAD")
        );
        assert_eq!(plane_root_branch_reason(&after, &s.session, root), None);
    }
}

#[test]
fn a_substitution_among_gits_options_is_unnamed_with_a_git_directory() {
    a_substitution_among_gits_options_is_unnamed(false);
}

#[test]
fn a_substitution_among_gits_options_is_unnamed_with_a_git_file() {
    a_substitution_among_gits_options_is_unnamed(true);
}

/// An alias after a substitution among git's options is followed as one in its place would be:
/// inline (`-c alias.co=checkout`, before or after the substitution) or in the root's config.
/// And a program only the shell can name (`$G`, `$(which git)`) may be git: a branch move or
/// reset after it is refused as unnamed (#1354).
fn an_alias_or_a_variable_git_after_a_substitution_is_followed(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    git(
        std::path::Path::new(root),
        &["config", "alias.zzsw", "switch"],
    );
    let unnamed = "cannot tell which repository this `git ";
    for (open, close) in [("$(", ")"), ("`", "`")] {
        let echo = format!("{open}echo {root}{close}");
        for cmd in [
            format!("git -c alias.co=checkout -C {echo} co -b x"),
            format!("git -C {echo} -c alias.co=checkout co -b x"),
            format!("git -C {echo} zzsw -c x"),
            format!("git --git-dir={echo}/.git zzsw -c x"),
            format!("{open}which git{close} -C {root} switch -c x"),
        ] {
            let said = plane_root_branch_reason(&cmd, &s.clone, root).expect(&cmd);
            assert!(said.starts_with(unnamed), "{cmd:?}: {said}");
        }
        // An alias that resolves to no branch move is still refused: the substitution is not
        // quoted.
        let st = format!("git -c alias.st=status -C {echo} st");
        let said = plane_root_branch_reason(&st, &s.clone, root).expect(&st);
        assert!(said.contains("is not quoted"), "{said}");
    }
    for cmd in [
        "G=git; $G checkout -b x",
        "$GIT switch -c x",
        "\"$G\" checkout --detach",
    ] {
        let said = plane_root_branch_reason(cmd, &s.session, root).expect(cmd);
        assert!(said.starts_with(unnamed), "{cmd:?}: {said}");
    }
    assert_eq!(
        plane_root_branch_reason("G=git; $G status", &s.session, root),
        None
    );
}

/// An alias after a program only the shell can name (`G=git; $G co -b x`) is followed as one
/// after `git` would be: in the root's config or defined on the line, through the line's one
/// alias read. A word that is no alias of a branch move stays allowed (#1326).
fn an_alias_after_a_variable_program_is_followed(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    let r = std::path::Path::new(root);
    git(r, &["config", "alias.zzsw", "switch"]);
    git(r, &["config", "alias.zzco", "checkout"]);
    git(r, &["config", "alias.zzck", "zzco"]);
    let unnamed = "cannot tell which repository this `git ";
    for cmd in [
        "G=git; $G zzsw -c x",
        "G=git; $G zzco -b x",
        "G=git; $G zzck feature",
        "$GIT zzsw -c x",
        "\"$G\" -c alias.co=checkout co -b x",
    ] {
        let said = plane_root_branch_reason(cmd, &s.session, root).expect(cmd);
        assert!(said.starts_with(unnamed), "{cmd:?}: {said}");
    }
    for cmd in [
        "G=git; $G status",
        "G=git; $G -c alias.st=status st",
        "$EDITOR notes.txt",
    ] {
        assert_eq!(
            plane_root_branch_reason(cmd, &s.session, root),
            None,
            "{cmd:?}"
        );
    }
}

#[test]
fn an_alias_after_a_variable_program_is_followed_with_a_git_directory() {
    an_alias_after_a_variable_program_is_followed(false);
}

#[test]
fn an_alias_after_a_variable_program_is_followed_with_a_git_file() {
    an_alias_after_a_variable_program_is_followed(true);
}

/// An alias the line itself defines after a program only the shell can name is followed with no
/// repository's config to read (#1326).
#[test]
fn an_inline_alias_after_a_variable_program_is_followed() {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::pypath::realpath(dir.path().to_str().unwrap());
    for cmd in [
        "G=git; $G -c alias.co=checkout co -b x",
        "\"$G\" -c alias.co=switch co -c x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &root, &root).is_some(),
            "{cmd:?}"
        );
    }
}

/// A line of many programs only the shell can name, each with words to follow, is read in
/// time: the aliases are read once for the line, not once per segment (#1326).
#[test]
fn many_variable_programs_are_read_in_time() {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::pypath::realpath(dir.path().to_str().unwrap());
    let cmd = "$G -c alias.st=status st notes; $EDITOR notes.txt; ".repeat(1000);
    let started = std::time::Instant::now();
    assert_eq!(plane_root_branch_reason(&cmd, &root, &root), None);
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(3), "{took:?}");
}

/// Where git cannot be asked which words are aliases, what a program only the shell can name
/// runs is unread, and refused (#1326, as after a substitution, #1354).
#[test]
fn an_alias_book_git_cannot_read_keeps_a_variable_program_refused() {
    use std::io::Write;
    let s = scratch(false);
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(format!("{}/.git/config", s.root))
        .unwrap();
    f.write_all(b"[alias]\n\tzzbad = checkout \xff\n").unwrap();
    let said = plane_root_branch_reason("G=git; $G zzco -b x", &s.session, &s.root).unwrap();
    assert!(
        said.starts_with("cannot tell what this command does: its program is named only"),
        "{said}"
    );
}

/// What a program only the shell can name runs, unread, is told as that: its program, not a
/// substitution among git's options, is what the guard could not read (#1326).
#[test]
fn an_unread_variable_program_is_told_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::pypath::realpath(dir.path().to_str().unwrap());
    let said = plane_root_branch_reason("\"$G\" -c alias.co=$Y co -b x", &root, &root).unwrap();
    assert!(
        said.starts_with("cannot tell what this command does: its program is named only"),
        "{said}"
    );
    assert!(!said.contains("substitution among its options"), "{said}");
    // After git itself the substitution is still what hides the command.
    let said =
        plane_root_branch_reason("git -C $(pwd) -c alias.co=$Y co -b x", &root, &root).unwrap();
    assert!(
        said.starts_with("cannot tell what this `git` command does"),
        "{said}"
    );
}

/// A line that points git at a config file the guard does not read refuses a word after a
/// program only the shell can name only when an alias book it reads names that word: the
/// program may be no git at all (#1326).
#[test]
fn a_config_file_the_line_names_does_not_refuse_every_word_after_a_variable_program() {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::pypath::realpath(dir.path().to_str().unwrap());
    for cmd in [
        "env -i HOME=$HOME PATH=$PATH \"$BIN\" persona list",
        "HOME=/tmp/h; $EDITOR notes.txt",
        "export XDG_CONFIG_HOME=/tmp/c; \"$PY\" script.py run",
        "GIT_CONFIG_GLOBAL=/tmp/g $CARGO build",
    ] {
        assert_eq!(plane_root_branch_reason(cmd, &root, &root), None, "{cmd:?}");
    }
    for cmd in [
        "HOME=/tmp/h; $G -c alias.co=checkout co -b x",
        "HOME=/tmp/h; $G checkout -b x",
    ] {
        assert!(
            plane_root_branch_reason(cmd, &root, &root).is_some(),
            "{cmd:?}"
        );
    }
}

#[test]
fn an_alias_or_a_variable_git_after_a_substitution_is_followed_with_a_git_directory() {
    an_alias_or_a_variable_git_after_a_substitution_is_followed(false);
}

#[test]
fn an_alias_or_a_variable_git_after_a_substitution_is_followed_with_a_git_file() {
    an_alias_or_a_variable_git_after_a_substitution_is_followed(true);
}

/// Following aliases after a substitution costs one git question per command line, however
/// many segments and alias words it holds: the hook answers on a deadline (#1354).
#[test]
fn aliases_after_substitutions_are_read_once_per_line() {
    let s = scratch(true);
    let root = &s.root;
    git(
        std::path::Path::new(root),
        &["config", "alias.st", "status"],
    );
    let one = format!("git -C $(echo {root}) {}", "st ".repeat(40));
    // Unquoted, the substitution is refused, and the line is answered in time all the same.
    let cmd = vec![one; 200].join("; ");
    let started = std::time::Instant::now();
    assert!(plane_root_branch_reason(&cmd, &s.clone, root).is_some());
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(3), "{took:?}");
    // Past the budget of different aliases to follow, what git runs is unread, and refused.
    let mut words = String::new();
    for n in 0..=MAX_CHECKOUT_OPERANDS {
        let name = format!("alias.st{n}");
        git(std::path::Path::new(root), &["config", &name, "status"]);
        words.push_str(&format!("st{n} "));
    }
    let many = format!("git -C $(echo {root}) {words}");
    let said = plane_root_branch_reason(&many, &s.clone, root).unwrap();
    assert!(
        said.starts_with("cannot tell what this `git` command does"),
        "{said}"
    );
}

/// Where git cannot be asked which words are aliases — here, an alias body that is not UTF-8 —
/// a branch move hidden behind a substitution is not taken to be absent: it is refused (#1354,
/// as an operand git cannot be asked about is, #438).
fn an_alias_book_git_cannot_read_keeps_the_refusal(linked: bool) {
    use std::io::Write;
    let s = scratch(linked);
    let config = if linked {
        format!("{}.gitdir/config", s.root)
    } else {
        format!("{}/.git/config", s.root)
    };
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(config)
        .unwrap();
    f.write_all(b"[alias]\n\tzzbad = checkout \xff\n").unwrap();
    let cmd = format!("git -C $(echo {}) zzco -b x", s.root);
    let said = plane_root_branch_reason(&cmd, &s.clone, &s.root).unwrap();
    assert!(
        said.starts_with("cannot tell what this `git` command does"),
        "{said}"
    );
}

#[test]
fn an_alias_book_git_cannot_read_keeps_the_refusal_with_a_git_directory() {
    an_alias_book_git_cannot_read_keeps_the_refusal(false);
}

#[test]
fn an_alias_book_git_cannot_read_keeps_the_refusal_with_a_git_file() {
    an_alias_book_git_cannot_read_keeps_the_refusal(true);
}

/// An alias redefined on the line after a word that named it is a new alias: what the word
/// after the redefinition runs is followed again, not answered from the first (#1354).
#[test]
fn an_alias_redefined_after_a_substitution_is_followed_again() {
    let s = scratch(true);
    let cmd = format!(
        "git -c alias.zz=status -C $(echo {}; : zz) -c alias.zz=checkout zz -b x",
        s.root
    );
    assert!(plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some());
}

/// Thousands of inline alias definitions after a substitution are read in one pass, not once
/// per word: the hook answers on a deadline (#1354).
#[test]
fn many_inline_aliases_after_a_substitution_are_read_in_time() {
    let s = scratch(true);
    let cmd = format!(
        "git -C $(echo {}) {}",
        s.root,
        "-c alias.zz=status zz ".repeat(6000)
    );
    let started = std::time::Instant::now();
    assert!(plane_root_branch_reason(&cmd, &s.clone, &s.root).is_some());
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(3), "{took:?}");
}

/// A chain of aliases longer than the guard follows is refused, not taken to end where the
/// guard stopped looking: written out, and after a substitution (#1354).
fn an_alias_chain_past_the_hop_limit_is_refused(linked: bool) {
    let s = scratch(linked);
    let r = std::path::Path::new(&s.root);
    for (name, body) in [
        ("e1", "e2"),
        ("e2", "e3"),
        ("e3", "e4"),
        ("e4", "e5"),
        ("e5", "checkout"),
    ] {
        git(r, &["config", &format!("alias.{name}"), body]);
    }
    let said =
        plane_root_branch_reason(&format!("git -C {} e1 -b x", s.root), &s.clone, &s.root).unwrap();
    assert!(said.starts_with("cannot tell what `git e1` does"), "{said}");
    let said = plane_root_branch_reason(
        &format!("git -C $(echo {}) e1 -b x", s.root),
        &s.clone,
        &s.root,
    )
    .unwrap();
    assert!(
        said.starts_with("cannot tell what this `git` command does"),
        "{said}"
    );
    // Four hops still reach the end.
    let said =
        plane_root_branch_reason(&format!("git -C {} e2 -b x", s.root), &s.clone, &s.root).unwrap();
    assert!(said.starts_with("would create 'x'"), "{said}");
}

#[test]
fn an_alias_chain_past_the_hop_limit_is_refused_with_a_git_directory() {
    an_alias_chain_past_the_hop_limit_is_refused(false);
}

#[test]
fn an_alias_chain_past_the_hop_limit_is_refused_with_a_git_file() {
    an_alias_chain_past_the_hop_limit_is_refused(true);
}

/// An alias the command defines for itself, from wherever git takes configuration, is followed
/// as one in the root's config is (#1358): the numbered `GIT_CONFIG_*` pairs (set on the
/// invocation, exported earlier, or assigned bare), `--config-env`, and
/// `GIT_CONFIG_PARAMETERS`. From a clone aimed at the root and from a session in the root, a
/// branch move or reset behind one is refused; one that leads nowhere it judges is allowed.
fn an_alias_the_command_defines_is_followed(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    let parent = &root[..root.rfind('/').unwrap()];
    let r = std::path::Path::new(root);
    git(
        std::path::Path::new(parent),
        &["init", "-q", "--bare", "-b", "main", "up.git"],
    );
    git(r, &["remote", "add", "origin", &format!("{parent}/up.git")]);
    git(r, &["push", "-q", "-u", "origin", "main"]);
    git(r, &["commit", "-q", "--allow-empty", "-m", "unpushed"]);
    let count = |name: &str, body: &str| {
        format!("GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.{name} GIT_CONFIG_VALUE_0='{body}'")
    };
    for (cwd, to) in [
        (&s.clone, format!("-C {root} ")),
        (&s.session, String::new()),
    ] {
        for cmd in [
            format!("{} git {to}yy -b x", count("yy", "checkout")),
            format!("V=checkout git --config-env=alias.yy=V {to}yy -b x"),
            format!("V=switch git --config-env alias.yy=V {to}yy -c x"),
            format!("GIT_CONFIG_PARAMETERS=\"'alias.yy'='checkout'\" git {to}yy -b x"),
            format!("GIT_CONFIG_PARAMETERS=\"'alias.yy=switch'\" git {to}yy -c x"),
        ] {
            let said = plane_root_branch_reason(&cmd, cwd, root).expect(&cmd);
            assert!(said.starts_with("would "), "{cmd:?}: {said}");
        }
        // Defined by an earlier segment, which may not reach git: unread, and refused (D-1358f).
        for cmd in [
            format!("export {} && git {to}yy -b x", count("yy", "checkout")),
            format!("{}; git {to}yy -b x", count("yy", "checkout")),
            format!("export {} && git {to}yy", count("yy", "status")),
        ] {
            let said = plane_root_branch_reason(&cmd, cwd, root).expect(&cmd);
            assert!(
                said.starts_with("cannot tell what `git yy` does"),
                "{cmd:?}: {said}"
            );
        }
        for cmd in [
            format!("{} git {to}ww HEAD~1", count("ww", "reset --hard")),
            format!("V='reset --hard' git --config-env=alias.ww=V {to}ww HEAD~1"),
        ] {
            assert!(
                plane_root_reset_reason(&cmd, cwd, root).is_some(),
                "{cmd:?}"
            );
        }
        for cmd in [
            format!("{} git {to}yy", count("yy", "status")),
            format!("V=status git --config-env=alias.yy=V {to}yy"),
        ] {
            assert_eq!(plane_root_branch_reason(&cmd, cwd, root), None, "{cmd:?}");
        }
    }
}

#[test]
fn an_alias_the_command_defines_is_followed_with_a_git_directory() {
    an_alias_the_command_defines_is_followed(false);
}

#[test]
fn an_alias_the_command_defines_is_followed_with_a_git_file() {
    an_alias_the_command_defines_is_followed(true);
}

/// Configuration the guard does not read — a config file the command points git at, an include,
/// or a value only the shell fills in — may define any alias, so a possible alias after it is
/// refused with a sentence that says why; git's own commands are not aliases and run (#1358).
fn configuration_the_guard_does_not_read_keeps_the_refusal(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    let unread = "cannot tell what `git yy` does: it may be an alias";
    for (cwd, to) in [
        (&s.clone, format!("-C {root} ")),
        (&s.session, String::new()),
    ] {
        for cmd in [
            format!("GIT_CONFIG_GLOBAL=/tmp/c git {to}yy -b x"),
            format!("GIT_CONFIG_SYSTEM=/tmp/c git {to}yy -b x"),
            format!("GIT_CONFIG=/tmp/c git {to}yy -b x"),
            format!("HOME=/tmp/h git {to}yy -b x"),
            format!("HOME=/tmp/h; git {to}yy -b x"),
            format!("XDG_CONFIG_HOME=/tmp/x git {to}yy -b x"),
            format!("git -c include.path=/tmp/c {to}yy -b x"),
            format!("git -c includeIf.onbranch:main.path=/tmp/c {to}yy -b x"),
            format!(
                "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=include.path GIT_CONFIG_VALUE_0=/tmp/c git {to}yy -b x"
            ),
            format!(
                "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.yy GIT_CONFIG_VALUE_0=$V git {to}yy -b x"
            ),
            format!("GIT_CONFIG_COUNT=$N git {to}yy -b x"),
            format!(
                "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=$K GIT_CONFIG_VALUE_0=checkout git {to}yy -b x"
            ),
            format!("git --config-env=alias.yy=NOT_SET_ANYWHERE_1358 {to}yy -b x"),
            format!("GIT_CONFIG_PARAMETERS=\"$P\" git {to}yy -b x"),
            format!("git -c alias.yy=$B {to}yy -b x"),
        ] {
            let said = plane_root_branch_reason(&cmd, cwd, root).expect(&cmd);
            assert!(said.starts_with(unread), "{cmd:?}: {said}");
        }
        for cmd in [
            format!("HOME=/tmp/h git {to}status"),
            format!("GIT_CONFIG_GLOBAL=/tmp/c git {to}log --oneline"),
        ] {
            assert_eq!(plane_root_branch_reason(&cmd, cwd, root), None, "{cmd:?}");
        }
    }
    // After a substitution among git's options, the same: followed where it is read, refused
    // where it is not.
    let echo = format!("$(echo {root})");
    for cmd in [
        format!(
            "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.yy GIT_CONFIG_VALUE_0=checkout git -C {echo} yy -b x"
        ),
        format!("V=checkout git --config-env=alias.yy=V -C {echo} yy -b x"),
        format!("HOME=/tmp/h git -C {echo} yy -b x"),
        format!("git -c include.path=/tmp/c -C {echo} yy -b x"),
        format!("V=$(echo checkout) git --config-env=alias.yy=V -C {root} yy -b x"),
    ] {
        let said = plane_root_branch_reason(&cmd, &s.clone, root).expect(&cmd);
        assert!(said.starts_with("cannot tell "), "{cmd:?}: {said}");
    }
}

#[test]
fn configuration_the_guard_does_not_read_keeps_the_refusal_with_a_git_directory() {
    configuration_the_guard_does_not_read_keeps_the_refusal(false);
}

#[test]
fn configuration_the_guard_does_not_read_keeps_the_refusal_with_a_git_file() {
    configuration_the_guard_does_not_read_keeps_the_refusal(true);
}

/// A body the line gives an alias that git may never see — set on another command, exported in
/// a subshell, after `false &&`, then unset or dropped by `env` — cannot hide the config's own
/// alias: both readings are followed, and the one that moves HEAD is refused (#1358). The
/// pairs on git itself are what git sees, and decide alone.
fn a_line_value_git_may_not_see_only_adds_refusals(linked: bool) {
    let s = scratch(linked);
    let root = &s.root;
    git(
        std::path::Path::new(root),
        &["config", "alias.co", "checkout"],
    );
    let triple = "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.co GIT_CONFIG_VALUE_0=status";
    let zz = "GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.zz GIT_CONFIG_VALUE_0=co";
    let params = "GIT_CONFIG_PARAMETERS=\"'alias.co'='status'\"";
    for cmd in [
        format!("{triple} true; git -C {root} co -b x"),
        format!("export {triple}; env -u GIT_CONFIG_COUNT git -C {root} co -b x"),
        format!("false && export {triple}; git -C {root} co -b x"),
        format!("(export {triple}); git -C {root} co -b x"),
        format!("export {triple}; unset GIT_CONFIG_COUNT; git -C {root} co -b x"),
        format!("export {triple}; env -i PATH=/usr/bin git -C {root} co -b x"),
        format!("{triple}; git -C {root} co -b x"),
        format!("{triple} true; git -C $(echo {root}) co -b x"),
        // A SUBSET of the line's values reaching git: `zz = co` does, `co = status` does not.
        format!("export {zz} {params}; env -u GIT_CONFIG_PARAMETERS git -C {root} zz -b x"),
        format!("export {zz} {params}; unset GIT_CONFIG_PARAMETERS; git -C {root} zz -b x"),
        format!("export {zz}; false && export {params}; git -C {root} zz -b x"),
        format!("export {zz}; (export {params}); git -C {root} zz -b x"),
        format!("export {zz} {params}; false && export GIT_CONFIG_COUNT=0; git -C {root} zz -b x"),
        format!("export {zz} {params}; env -u GIT_CONFIG_PARAMETERS git -C $(echo {root}) zz -b x"),
    ] {
        assert!(
            plane_root_branch_reason(&cmd, &s.clone, root).is_some(),
            "{cmd:?}"
        );
    }
    assert_eq!(
        plane_root_branch_reason(&format!("{triple} git -C {root} co -b x"), &s.clone, root),
        None
    );
    // A pair whose value is set nowhere the guard can see is unread, not empty.
    let mut pairs: Vec<String> = (0..64)
        .map(|n| format!("GIT_CONFIG_KEY_{n}=x.y{n} GIT_CONFIG_VALUE_{n}=1"))
        .collect();
    pairs.push("GIT_CONFIG_KEY_64=alias.yy".to_string());
    let cmd = format!(
        "GIT_CONFIG_COUNT=65 {} git -C {root} yy -b x",
        pairs.join(" ")
    );
    let said = plane_root_branch_reason(&cmd, &s.clone, root).unwrap();
    assert!(said.starts_with("cannot tell what `git yy` does"), "{said}");
}

#[test]
fn a_line_value_git_may_not_see_only_adds_refusals_with_a_git_directory() {
    a_line_value_git_may_not_see_only_adds_refusals(false);
}

#[test]
fn a_line_value_git_may_not_see_only_adds_refusals_with_a_git_file() {
    a_line_value_git_may_not_see_only_adds_refusals(true);
}

/// A long line of bare assignments before a git command costs what it is long (#1358).
#[test]
fn a_long_line_of_assignments_is_read_in_time() {
    let s = scratch(true);
    let cmd = format!("{}git -C {} status", "A=1; ".repeat(20_000), s.root);
    let started = std::time::Instant::now();
    assert_eq!(plane_root_branch_reason(&cmd, &s.clone, &s.root), None);
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "{took:?}");
}

/// A long line of exports costs what it is long, in every guard that reads them (#1358).
#[test]
fn a_long_line_of_exports_is_read_in_time() {
    let s = scratch(true);
    let cmd = format!("{}git -C {} status", "export A=1; ".repeat(20_000), s.root);
    let started = std::time::Instant::now();
    assert_eq!(plane_root_branch_reason(&cmd, &s.clone, &s.root), None);
    assert_eq!(plane_root_reset_reason(&cmd, &s.clone, &s.root), None);
    assert!(crate::credguard::single_credential_hit(&cmd, &[]).is_none());
    assert!(crate::commitguard::hook_skip_hit(&cmd, &s.clone).is_none());
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(2), "{took:?}");
}

/// A directory word the same line makes or re-points before git runs — the target of an `ln`,
/// `mv` or `cp`, or a path a redirection writes into — is read by the guard before the line
/// runs, when it does not exist yet or is still what it was. So it is one the guard cannot name,
/// and git sent there may be on the root (#1326). Read off the line, never the filesystem.
#[test]
fn a_directory_the_same_line_links_or_moves_is_one_the_guard_cannot_name() {
    let dir = tempfile::tempdir().unwrap();
    let base = crate::pypath::realpath(dir.path().to_str().unwrap());
    let root = format!("{base}/plane");
    let away = format!("{base}/away");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&away).unwrap();
    let unnamed = |cmd: &str| plane_root_git(cmd, &away, &root).iter().any(|i| i.unnamed);
    for cmd in [
        format!("ln -s {root} foo && git -C foo checkout -b x"),
        format!("ln -sfn {root} foo; git -C foo/ switch -c x"),
        format!("ln -s {root} && git -C plane checkout -b x"),
        format!("sudo ln -s {root} {away}/foo && git -C ./foo checkout -b x"),
        format!("ln -s {root} foo && cd foo && git checkout -b x"),
        format!("mv clone clone.old && ln -s {root} clone && git --git-dir=clone/.git switch -c x"),
        "mv foo bar && git -C bar checkout -b x".to_owned(),
        "cp -R elsewhere foo && GIT_DIR=foo/.git git checkout -b x".to_owned(),
        "ln -t foo -s /x && git -C foo checkout -b x".to_owned(),
        format!("mkdir foo && echo 'gitdir: {root}/.git' > foo/.git && git -C foo checkout -b x"),
        format!(
            "mkdir foo && printf 'gitdir: {root}/.git' >foo/.git && env -C foo git checkout -b x"
        ),
        // Train 77 review: coreutils under its GNU name, busybox's applet, and what an
        // archive or a copy tool unpacks, which the line cannot spell.
        format!("gln -s {root} foo && git -C foo checkout -b x"),
        format!("busybox ln -s {root} foo && git -C foo checkout -b x"),
        "tar xf links.tar && git -C foo checkout -b x".to_owned(),
        "unzip -o links.zip && git -C foo checkout -b x".to_owned(),
        "rsync -a elsewhere/ . && git -C foo checkout -b x".to_owned(),
    ] {
        assert!(unnamed(&cmd), "{cmd:?}");
    }
    // A directory nothing on the line makes is read as it stands; a link made AFTER git ran, or
    // one somewhere else, changes nothing.
    for cmd in [
        "git -C foo checkout -b x".to_owned(),
        "git -C foo checkout -b x && ln -s /x foo".to_owned(),
        "ln -s /x bar && git -C foo checkout -b x".to_owned(),
        "echo hi > notes.txt && git -C foo checkout -b x".to_owned(),
        "cp a b && git -C foo status".to_owned(),
    ] {
        assert!(!unnamed(&cmd), "{cmd:?}");
    }
}

/// A line of loose `cd`s to relative names is read in time: past the directories the walk
/// spells, the shell may be anywhere, and a branch move there is refused (#1325).
/// #1325: past the most directories the walk spells, what a later segment makes where the shell
/// may be is still one the guard cannot name, as it was before the cap: a path made by a
/// relative name there may be where a later `cd` lands.
#[test]
fn what_is_made_past_the_most_directories_spelt_is_still_unnameable() {
    let (_dir, root, elsewhere) = bare_root();
    let cds: String = (0..70).map(|i| format!("cd d{i}; ")).collect();
    let deep: String = (0..70).map(|i| format!("/d{i}")).collect();
    let line =
        format!("{cds}ln -s {root} foo; cd {elsewhere}{deep} && git -C foo checkout feature");
    assert!(
        plane_root_git(&line, &elsewhere, &root)
            .iter()
            .any(|inv| inv.unnamed),
        "{line}"
    );
}

/// After a `cd` the guard cannot name, the shell may be in a directory no word spells, so a path
/// made there by a relative name is one no later directory word can be told apart from: a later
/// `cd` to a named directory may land where it was made. It stays unnameable rather than lost.
#[test]
fn what_is_made_by_a_relative_name_after_an_unnameable_cd_is_still_unnameable() {
    let (_dir, root, elsewhere) = bare_root();
    let unnamed = |cmd: &str| {
        plane_root_git(cmd, &elsewhere, &root)
            .iter()
            .any(|inv| inv.unnamed)
    };
    for cmd in [
        format!("cd \"$D\" && ln -s {root} foo && cd {elsewhere} && git -C foo checkout feature"),
        format!("cd - && ln -s {root} foo; cd {elsewhere} && git -C foo switch -c x"),
        format!("popd && mv clone foo && cd {elsewhere} && GIT_DIR=foo/.git git checkout -b x"),
        format!(
            "cd ~other && echo 'gitdir: {root}/.git' > .git && cd {elsewhere} && git checkout -b x"
        ),
        format!(
            "cd \"$D\" && sh -c 'ln -s {root} foo' && cd {elsewhere} && git -C foo checkout -b x"
        ),
    ] {
        assert!(unnamed(&cmd), "{cmd:?}");
    }
    // A path made by an absolute name is spelt wherever the shell is, and one made before the
    // unnameable `cd` was spelt where the shell then was: both are read as before.
    for cmd in [
        format!("cd \"$D\" && ln -s {root} /x/foo && cd {elsewhere} && git -C foo checkout -b x"),
        format!("ln -s /x bar && cd \"$D\" && cd {elsewhere} && git -C foo checkout -b x"),
    ] {
        assert!(!unnamed(&cmd), "{cmd:?}");
    }
}

#[test]
fn many_loose_cds_are_read_in_time_and_fail_closed() {
    let (_dir, root, elsewhere) = bare_root();
    let cds: String = (0..40).map(|i| format!("cd d{i}; ")).collect();
    let began = std::time::Instant::now();
    assert!(
        plane_root_git(&format!("{cds}git checkout feature"), &elsewhere, &root)
            .iter()
            .any(|inv| inv.unnamed)
    );
    assert!(plane_root_git(&format!("{cds}ls"), &elsewhere, &root).is_empty());
    assert!(
        began.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
}

/// What these two guards answer for an update of the root from another commit, and for the
/// way back from one that stopped partway (qw346). Neither guard judges an update: a rebase, a
/// pull or a merge moves no branch and destroys no commit, so they stand aside, and the arm
/// for a sandboxed chat ([`crate::rootupdate`]) is the one that speaks. A restore to the
/// current commit is no branch move wherever its paths are on the line.
#[test]
fn an_update_from_another_commit_is_not_these_guards_to_judge_and_a_restore_is_allowed() {
    let f = fixture();
    for update in [
        "git rebase origin/main",
        "git pull",
        "git pull --rebase",
        "git pull --rebase origin main",
        "git merge origin/main",
        "git merge --ff-only origin/main",
        "git stash pop",
        "git stash apply",
        "git cherry-pick feature",
        "git am patch.mbox",
        "git revert HEAD",
    ] {
        assert_eq!(branch(&f, update), None, "{update}");
        assert_eq!(reset(&f, update), None, "{update}");
    }
    // A reset to another commit is refused only where it would lose a commit no remote has.
    assert_eq!(branch(&f, "git reset --hard feature"), None);
    assert!(
        reset(&f, "git reset --hard origin/main")
            .unwrap()
            .starts_with("would delete 1 commit from the PLANE ROOT")
    );
    assert_eq!(reset(&f, "git reset --hard HEAD"), None);
    // A branch move is refused, as it always was.
    for moving in ["git checkout feature", "git switch feature"] {
        assert!(
            branch(&f, moving)
                .unwrap()
                .starts_with("would switch to 'feature' in the PLANE ROOT. "),
            "{moving}"
        );
    }
    // The way back: files put back as the current commit has them.
    for restore in [
        "git restore --source=HEAD --worktree -- README both",
        "git restore --source=HEAD --worktree --pathspec-from-file=list.z --pathspec-file-nul",
        "git restore README",
        "git checkout HEAD -- README both",
        "git checkout -- README",
        "git checkout HEAD README",
    ] {
        assert_eq!(branch(&f, restore), None, "{restore}");
        assert_eq!(reset(&f, restore), None, "{restore}");
    }
}

/// `git checkout HEAD` with no path on the line after a `--` reads as a move to `HEAD`: the
/// paths a file or `xargs` hands git are out of the guard's sight. It stays refused, and the
/// refusal names the spelling that cannot move HEAD, which is allowed (qw346).
#[test]
fn a_checkout_of_head_with_its_paths_out_of_sight_is_refused_with_the_restore_that_is_allowed() {
    let f = fixture();
    for unseen in [
        "git checkout HEAD --pathspec-from-file=list.z --pathspec-file-nul",
        "git checkout HEAD",
        "git checkout HEAD --",
        "git checkout @ --pathspec-from-file=list.z",
        "git zzco HEAD",
    ] {
        let said = branch(&f, unseen).unwrap_or_else(|| panic!("{unseen}: allowed"));
        assert!(
            said.contains("`git restore --source=HEAD --worktree -- <paths>`"),
            "{unseen}: {said}"
        );
        assert!(
            said.contains("--pathspec-from-file=<file>"),
            "{unseen}: {said}"
        );
        assert!(
            said.ends_with("is always allowed."),
            "{unseen}: the tail every denial ends with: {said}"
        );
    }
    // Only that: a detach, another commit and `git switch` keep their own sentences.
    assert!(
        branch(&f, "git checkout --detach HEAD")
            .unwrap()
            .starts_with("would detach HEAD at 'HEAD'")
    );
    assert!(
        branch(&f, "git switch HEAD")
            .unwrap()
            .starts_with("would switch to 'HEAD' in the PLANE ROOT. ")
    );
    assert!(
        branch(&f, "git checkout HEAD~1")
            .unwrap()
            .starts_with("would switch to 'HEAD~1' in the PLANE ROOT. ")
    );
}

/// The directories a line leaves the shell in are spelt up to a number and a size, and past
/// either none is: every git is read against each, so what is spelt is what one git costs
/// (qw367). Only a `cd` to an absolute directory that cannot fail unnoticed names one again.
#[test]
fn the_directories_in_play_are_spelt_up_to_a_cap_and_then_none_is() {
    let line = CdContext::of("");
    let mut here = Whereabouts::at("/r");
    for n in 1..MAX_WHEREABOUTS {
        here.merge(Whereabouts::at(&format!("/r/{n}")));
    }
    assert_eq!(here.dirs.len(), MAX_WHEREABOUTS);
    assert!(!here.anywhere && !here.lost);
    // One already spelt is no more to spell.
    here.merge(Whereabouts::at("/r/1"));
    assert!(!here.lost);
    here.merge(Whereabouts::at("/r/more"));
    assert!(
        here.lost && here.anywhere && here.dirs.is_empty(),
        "{here:?}"
    );
    // It stays so whatever is taken in, and a relative `cd` from it spells nothing.
    here.merge(Whereabouts::at("/elsewhere"));
    assert!(here.lost && here.dirs.is_empty());
    let moved = cd_to("sub".to_string(), &here, &line);
    assert!(
        moved.lost && moved.anywhere && moved.dirs.is_empty(),
        "{moved:?}"
    );
    // An absolute one names where the shell is, if it ran.
    let moved = cd_to("/abs".to_string(), &here, &line);
    assert_eq!(moved.dirs, ["/abs"]);
    assert!(!moved.lost && !moved.anywhere);
    // The names' size is capped as their number is: a `cd` to a relative name makes each
    // longer.
    let mut deep = Whereabouts::at("/r");
    let long = format!("/r/{}", "a/".repeat(MAX_SPELT_BYTES / 2));
    deep.merge(Whereabouts::at(&long));
    assert!(deep.lost && deep.dirs.is_empty());
}

/// A git behind more directories than are spelt is one the guard cannot name, and is listed
/// for the root as any such git is; behind a few it is read where it runs (qw367).
#[test]
fn a_git_behind_more_directories_than_are_spelt_is_one_the_guard_cannot_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::pypath::realpath(dir.path().to_str().unwrap());
    std::fs::create_dir_all(format!("{root}/.git")).unwrap();
    std::fs::create_dir_all(format!("{root}/clone/.git")).unwrap();
    let few = "cd clone && true; git checkout feature";
    let read = line_git(few, &root, &root);
    assert_eq!(read.root.len(), 1, "{read:?}");
    assert!(
        !read.root[0].unnamed,
        "the root is one of two it may run in"
    );
    // Each `cd <name> && …;` leaves one more directory in play, a level deeper.
    let many = format!(
        "{}git checkout feature",
        "cd clone && true; ".repeat(MAX_WHEREABOUTS + 1)
    );
    let read = line_git(&many, &root, &root);
    assert_eq!(read.root.len(), 1, "{read:?}");
    assert!(read.root[0].unnamed);
    assert!(read.elsewhere.is_empty());
    assert_eq!(plane_root_git(&many, &root, &root), read.root);
    // An absolute `cd` joined by `&&` names where git runs again.
    let named = format!("{many}; cd {root}/clone && git checkout feature");
    let read = line_git(&named, &root, &root);
    assert_eq!(read.elsewhere.len(), 1, "{read:?}");
    assert_eq!(read.elsewhere[0].dirs, [format!("{root}/clone")]);
}

/// [`fixture`] with its one unpushed commit pushed: every reset there measures nothing lost.
fn pushed_fixture() -> Fixture {
    let f = fixture();
    git(std::path::Path::new(&f.root), &["push", "-q"]);
    f
}

/// A git command word, or one of git's own options, that the shell fills in — a variable, a
/// quoted substitution, a function's arguments — is a word the guard cannot read: the command is
/// refused as unknown, and a branch move or reset among the words after it is judged as written.
/// A function's body is read where the shell may call it from: anywhere.
#[test]
fn a_git_command_or_option_the_shell_fills_in_is_read_as_unknown() {
    let f = fixture();
    for cmd in [
        "A=checkout; git $A feature",
        "git $UNSET checkout feature",
        "git ${SUB} feature",
        "git \"$SUB\" feature",
        "f(){ git \"$@\"; }; f checkout feature",
        "f(){ git checkout feature; }; f",
        "function f { git checkout feature; }; f",
        "git \"`echo checkout`\" feature",
        "M=reset; git $M --hard HEAD~1",
        "git --$OPT checkout feature",
    ] {
        assert!(branch(&f, cmd).is_some(), "{cmd:?}");
    }
    // What the shell fills in after the command is its arguments, read as before.
    for cmd in [
        "git status",
        "git log $REV",
        "git show HEAD:$FILE",
        "git -C \"$PWD\" status",
    ] {
        assert_eq!(branch(&f, cmd), None, "{cmd:?}");
    }
}

/// An alias used after an earlier command on the line that may change git's configuration is
/// read as unknown: the configuration the guard reads is the one from before the line ran.
#[test]
fn an_alias_after_a_command_that_may_change_configuration_is_read_as_unknown() {
    let f = fixture();
    for cmd in [
        "git config alias.zzlater 'reset --hard HEAD~1'; git zzlater",
        "git config alias.zzlater checkout && git zzlater feature",
        "printf x >> notes && git zzlater feature",
        "git config alias.zzlater checkout && git -C $(pwd) zzlater feature",
        "git config alias.zzlater checkout && G=git && $G zzlater feature",
    ] {
        let said = branch(&f, cmd).expect(cmd);
        assert!(said.starts_with("cannot tell what"), "{cmd:?}: {said}");
    }
    for cmd in [
        "git zzlater feature",
        "git config user.name x && git status",
        "git add README && git log --oneline",
    ] {
        assert_eq!(branch(&f, cmd), None, "{cmd:?}");
    }
}

/// An earlier git command on the line that changes the repository makes the guards judge the
/// commands after it as unknown where their answer rests on what the repository holds: whether
/// a word is a file or a branch, and how many commits a reset would take off the branch. Run on
/// its own, the command is measured as before.
#[test]
fn an_earlier_git_command_that_changes_the_repository_makes_later_ones_unknown() {
    let f = fixture();
    assert_eq!(branch(&f, "git checkout README"), None);
    assert_eq!(branch(&f, "git status && git checkout README"), None);
    assert_eq!(branch(&f, "git add README && git checkout -- README"), None);
    let said = branch(&f, "git branch README HEAD~1 && git checkout README").expect("refused");
    assert!(said.contains("as a command of its own"), "{said}");

    let p = pushed_fixture();
    assert_eq!(reset(&p, "git reset --hard HEAD~1"), None);
    for cmd in [
        "git commit --allow-empty -m x && git reset --hard HEAD~1",
        "git checkout main && git reset --hard origin/main",
        "git fetch && git reset --soft HEAD~1",
        "git commit --allow-empty -m x && git update-ref refs/heads/main HEAD~1",
    ] {
        let said = reset(&p, cmd).expect(cmd);
        assert!(said.contains("as a command of its own"), "{cmd:?}: {said}");
    }
    for cmd in [
        "git status && git reset --hard HEAD~1",
        "git commit --allow-empty -m x && git reset --hard",
        "git add README && git reset --hard HEAD",
    ] {
        assert_eq!(reset(&p, cmd), None, "{cmd:?}");
    }
}

/// A reset in any mode, or a ref update of the branch the root is on, that would take commits
/// no upstream has off that branch is refused: the working tree may keep their files, but the
/// commits are left with the reflog as their only copy.
#[test]
fn taking_unpushed_commits_off_the_roots_branch_is_refused_in_any_mode() {
    let f = fixture();
    for cmd in [
        "git reset --soft origin/main",
        "git reset --mixed origin/main",
        "git reset origin/main",
        "git reset -q @{u}",
        "git update-ref refs/heads/main origin/main",
        "git update-ref HEAD origin/main",
        "git update-ref -m why refs/heads/main origin/main",
        "git update-ref -d refs/heads/main",
        "git update-ref --stdin",
    ] {
        assert!(reset(&f, cmd).is_some(), "{cmd:?}");
    }
    for cmd in [
        "git reset",
        "git reset --soft HEAD",
        "git reset README",
        "git reset HEAD README",
        "git reset origin/main README",
        "git reset origin/main -- README",
        "git reset -p origin/main",
        "git update-ref refs/heads/main HEAD",
        "git update-ref refs/heads/feature origin/main",
        "git update-ref refs/heads/zznew HEAD",
    ] {
        assert_eq!(reset(&f, cmd), None, "{cmd:?}");
    }
}

/// An unquoted command substitution among git's own options is split into words git reads as
/// options and as its command: refused, saying to quote it. Quoted, it is one word, and the
/// command after it is read as written.
#[test]
fn an_unquoted_substitution_among_gits_options_is_refused_and_a_quoted_one_is_read() {
    let f = fixture();
    for cmd in [
        "git -C $(git rev-parse --show-toplevel) status",
        "git -C $(pwd)",
        "git -C `pwd` log --oneline",
        "git --git-dir=$(pwd)/.git log",
        "git -c x.y=$(echo 1) status",
        "git -C $(printf '%s\\x20che%s' \"$PWD\" ckout) status",
    ] {
        let said = branch(&f, cmd).expect(cmd);
        assert!(said.contains("`git -C \"$(…)\" …`"), "{cmd:?}: {said}");
    }
    for cmd in [
        "git -C \"$(git rev-parse --show-toplevel)\" status",
        "git -C \"$(pwd)\" log --oneline",
        "git --git-dir=\"$(pwd)/.git\" log",
    ] {
        assert_eq!(branch(&f, cmd), None, "{cmd:?}");
    }
}

/// `git symbolic-ref HEAD <ref>` moves the root to another branch as a checkout does, without
/// touching a file: refused. Reading a symbolic ref, or setting one other than HEAD, is not.
#[test]
fn pointing_head_at_another_branch_is_a_branch_move() {
    let f = fixture();
    for cmd in [
        "git symbolic-ref HEAD refs/heads/feature",
        "git symbolic-ref -m why HEAD refs/heads/feature",
        "git symbolic-ref $REF refs/heads/feature",
    ] {
        let said = branch(&f, cmd).expect(cmd);
        assert!(said.contains("PLANE ROOT"), "{cmd:?}: {said}");
    }
    for cmd in [
        "git symbolic-ref HEAD",
        "git symbolic-ref --short HEAD",
        "git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/main",
    ] {
        assert_eq!(branch(&f, cmd), None, "{cmd:?}");
    }
}

/// How many times git was asked exactly `question` in `root` since `before` questions there.
fn times_asked(root: &str, before: usize, question: &[&str]) -> usize {
    crate::worktree::git::tally::asked(std::path::Path::new(root))[before..]
        .iter()
        .filter(|asked| *asked == question)
        .count()
}

/// How many questions git was asked in `root` so far.
fn asked_so_far(root: &str) -> usize {
    crate::worktree::git::tally::asked(std::path::Path::new(root)).len()
}

/// Every git on a line puts the same few questions, and each is a git process: one guard call
/// asks each once, and answers every git what it would have been answered (#2619).
#[test]
fn a_question_every_git_on_a_line_repeats_is_asked_of_git_once() {
    let f = fixture();
    let unpushed = [
        "rev-list",
        "--count",
        "HEAD",
        "--not",
        "HEAD",
        "@{upstream}",
        "--",
    ];
    let before = asked_so_far(&f.root);
    assert_eq!(reset(&f, &"git reset --hard HEAD; ".repeat(50)), None);
    assert_eq!(times_asked(&f.root, before, &unpushed), 1);

    let before = asked_so_far(&f.root);
    assert_eq!(branch(&f, &"git checkout README; ".repeat(50)), None);
    for question in [
        &["rev-parse", "--verify", "--quiet", "README^{commit}"][..],
        &["ls-files", "--error-unmatch", "--", "README"],
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ] {
        assert_eq!(times_asked(&f.root, before, question), 1, "{question:?}");
    }
}

/// What one call remembers changes no verdict: a refusal behind any number of gits that pass is
/// still made, a different question is still asked, and the next call asks git again, so a push
/// between two commands is seen (#2619).
#[test]
fn what_git_answered_is_kept_for_one_call_and_refuses_as_it_did() {
    let f = fixture();
    let wipe = "git reset --hard HEAD~1";
    let alone = reset(&f, wipe).expect("the unpushed commit is at risk");
    let behind = format!("{}{wipe}", "git reset --hard HEAD; ".repeat(50));
    assert_eq!(reset(&f, &behind).as_deref(), Some(alone.as_str()));
    let twice = format!("{wipe}; {wipe}");
    assert_eq!(reset(&f, &twice).as_deref(), Some(alone.as_str()));
    // The branch guard: a restore repeated, then a move.
    let restores = "git checkout README; ".repeat(50);
    let moved = branch(&f, "git checkout feature").expect("a branch move");
    assert_eq!(
        branch(&f, &format!("{restores}git checkout feature")).as_deref(),
        Some(moved.as_str())
    );
    // Once the commit is pushed, the next call reads the repository as it is then.
    git(
        std::path::Path::new(&f.root),
        &["push", "-q", "origin", "main"],
    );
    assert_eq!(reset(&f, wipe), None);
    assert_eq!(reset(&f, &behind), None);
}

/// A question about the fixture's `HEAD`, and what git answers it.
const HEAD_COMMIT: [&str; 4] = ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"];

/// The memory keeps at most [`MAX_REMEMBERED_ANSWERS`] answers: one more is asked of git each
/// time and answered as git answers it, and the last one that fits is kept (#2619).
#[test]
fn past_the_most_answers_kept_git_is_asked_again_and_answers_the_same() {
    let f = fixture();
    let head = |a: Answer| (a.ok, a.out);
    let truth = head(git_asked(&f.root, &HEAD_COMMIT).expect("git answers"));
    assert!(truth.0 && !truth.1.is_empty());
    for (kept_before, asked) in [(MAX_REMEMBERED_ANSWERS - 1, 1), (MAX_REMEMBERED_ANSWERS, 2)] {
        remembering(|| {
            REMEMBERED.with(|r| {
                let mut r = r.borrow_mut();
                let m = r.as_mut().expect("inside a call");
                for n in 0..kept_before {
                    let answer = Answer {
                        ok: true,
                        out: String::new(),
                    };
                    m.insert(("elsewhere".to_string(), vec![n.to_string()]), answer);
                }
            });
            let before = asked_so_far(&f.root);
            for _ in 0..2 {
                let answer = git_in(&f.root, &HEAD_COMMIT).expect("git answers");
                assert_eq!(head(answer), truth);
            }
            assert_eq!(
                times_asked(&f.root, before, &HEAD_COMMIT),
                asked,
                "with {kept_before} answers kept"
            );
        });
    }
}

/// A guard call inside another shares the outer's memory and leaves it standing; the outer's
/// end clears it, and the next call asks git again (#2619).
#[test]
fn a_call_inside_another_shares_its_memory_and_only_the_outer_ends_it() {
    let f = fixture();
    let before = asked_so_far(&f.root);
    let ask = || git_in(&f.root, &HEAD_COMMIT).expect("git answers").out;
    remembering(|| {
        let first = ask();
        let inner = remembering(ask);
        assert_eq!(inner, first);
        assert!(
            REMEMBERED.with(|r| r.borrow().is_some()),
            "the inner call ended the memory"
        );
        assert_eq!(ask(), first);
    });
    assert_eq!(times_asked(&f.root, before, &HEAD_COMMIT), 1);
    assert!(
        REMEMBERED.with(|r| r.borrow().is_none()),
        "the outer call left its memory"
    );
    remembering(ask);
    assert_eq!(times_asked(&f.root, before, &HEAD_COMMIT), 2);
}

/// What a long line exported or assigned near its start still reaches a git near its end, past
/// many other exports and gits, as it does on a short line: the line's environment is shared
/// between its gits, not copied into each (#2648). Each case is judged at both lengths and must
/// answer the same.
#[test]
fn an_early_export_reaches_a_git_at_the_end_of_a_long_line() {
    let (_dir, root, elsewhere) = bare_root();
    std::fs::create_dir_all(format!("{root}/.git")).unwrap();
    let moves = |cmd: &str| {
        plane_root_git(cmd, &elsewhere, &root)
            .iter()
            .any(|inv| BRANCH_MOVERS.contains(&inv.sub.as_str()))
    };
    // Exports, bare assignments, `eval`s and strings handed to shells, each with a git.
    let filler = |n: usize| -> String {
        (0..n)
            .map(|i| {
                format!(
                    "export GIT_V{i}=x; A{i}=y; git status; eval export B{i}=z; \
                     sh -c 'git log'; eval GIT_W{i}=w; "
                )
            })
            .collect()
    };
    let cases = |gap: &str| {
        let moving = [
            format!("export GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("eval export GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("set -a; eval GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("eval set -a; GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("export GIT_WORK_TREE={root}; {gap}git switch -c x"),
            format!("export GIT_DIR={root}/.git; {gap}sh -c 'git checkout feature'"),
            format!(
                "export GIT_DIR=/nowhere/.git; {gap}export GIT_DIR={root}/.git; {gap}\
                 git checkout feature"
            ),
        ];
        let staying = [
            format!("GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("eval GIT_DIR={root}/.git; {gap}git checkout feature"),
            format!("sh -c 'export GIT_DIR={root}/.git'; {gap}git checkout feature"),
            format!(
                "export GIT_DIR={root}/.git; {gap}export GIT_DIR=/nowhere/.git; {gap}\
                 git checkout feature"
            ),
        ];
        (moving, staying)
    };
    let (short_moving, short_staying) = cases("");
    let gap = filler(60);
    let (long_moving, long_staying) = cases(&gap);
    for (short, long) in short_moving.iter().zip(&long_moving) {
        assert!(moves(short), "{short:?}");
        assert!(moves(long), "{short:?}, after a long run");
    }
    for (short, long) in short_staying.iter().zip(&long_staying) {
        assert!(!moves(short), "{short:?}");
        assert!(!moves(long), "{short:?}, after a long run");
    }
}

/// An alias an export near the start of a long line defines is still unread at its end, and
/// refused, as on a short line (#2648, D-1358f).
#[test]
fn an_alias_an_early_export_defines_is_still_unread_at_the_end_of_a_long_line() {
    let f = fixture();
    let gap: String = (0..60)
        .map(|i| format!("export GIT_V{i}=x; A{i}=y; git status; eval GIT_W{i}=w; "))
        .collect();
    for gap in ["", gap.as_str()] {
        for cmd in [
            format!(
                "export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=alias.yy GIT_CONFIG_VALUE_0=checkout; \
                 {gap}git yy -b x"
            ),
            format!("export GIT_CONFIG_PARAMETERS=\"'alias.yy=checkout'\"; {gap}git yy -b x"),
            format!("export V=checkout; {gap}git --config-env=alias.yy=V yy -b x"),
            format!("export HOME=/tmp/h; {gap}git yy -b x"),
        ] {
            let said = branch(&f, &cmd).expect(&cmd);
            assert!(
                said.starts_with("cannot tell what `git yy` does"),
                "{cmd:?}: {said}"
            );
        }
        // A word the line does not make an alias stays git's own.
        assert_eq!(
            branch(&f, &format!("export HOME=/tmp/h; {gap}git status")),
            None
        );
    }
}

/// When the shell fills in git's command word, a word after it that reads as a command may be
/// an option or an operand of another: the command is refused as unknown, not judged as that
/// word.
#[test]
fn a_word_after_a_command_word_the_shell_fills_in_is_not_taken_for_the_command() {
    let f = fixture();
    for cmd in [
        "S=checkout; git $S -b reset",
        "S=update-ref; git $S -m reset refs/heads/main HEAD~1",
        "G=git; S=checkout; $G $S -b reset",
    ] {
        let said = branch(&f, cmd).expect(cmd);
        assert!(said.starts_with("cannot tell what"), "{cmd:?}: {said}");
    }
    assert_eq!(branch(&f, "G=git; $G status"), None);
}

/// A git that sets configuration for itself, or after a line that points git at configuration,
/// is not taken to only put files back: configuration can run a program as it does.
#[test]
fn a_git_that_sets_configuration_is_not_taken_to_only_put_files_back() {
    let p = pushed_fixture();
    assert_eq!(reset(&p, "git add README && git reset --hard HEAD~1"), None);
    for cmd in [
        "git -c core.fsmonitor=x add README && git reset --hard HEAD~1",
        "git --config-env=core.fsmonitor=X add README && git reset --hard HEAD~1",
        "GIT_CONFIG_PARAMETERS=\"'core.fsmonitor'='x'\" git add README && git reset --hard HEAD~1",
        "export HOME=/tmp/elsewhere; git add README && git reset --hard HEAD~1",
    ] {
        let said = reset(&p, cmd).expect(cmd);
        assert!(said.contains("as a command of its own"), "{cmd:?}: {said}");
    }
}

/// A ref named in another case is read as the root's own where the volume folds case, as
/// macOS's does by default: `refs/heads/MAIN` is `main`, and `head` is `HEAD`.
#[test]
fn a_ref_named_in_another_case_is_the_roots_own() {
    let f = fixture();
    for cmd in [
        "git update-ref refs/heads/MAIN origin/main",
        "git update-ref head origin/main",
    ] {
        assert!(reset(&f, cmd).is_some(), "{cmd:?}");
    }
    assert!(branch(&f, "git symbolic-ref head refs/heads/feature").is_some());
}
