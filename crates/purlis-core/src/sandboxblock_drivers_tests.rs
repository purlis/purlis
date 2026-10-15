// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A git reader in a repository whose config names a program it runs is a program (#2212).

use super::*;
use serde_json::json;

/// A repository at `<dir>/repo` whose `.git/config` holds `config`, made by hand: a sandboxed
/// test cannot `git init`, and only the files git reads are needed.
fn repo(config: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let git = dir.path().join("repo/.git");
    std::fs::create_dir_all(git.join("hooks")).expect("the git dir");
    std::fs::write(
        git.join("config"),
        format!("[core]\n\trepositoryformatversion = 0\n\tbare = false\n{config}"),
    )
    .expect("the config");
    dir
}

fn place_in(cwd: &Path) -> Place<'_> {
    Place {
        root: cwd,
        chat: cwd,
        cwd,
        home: None,
    }
}

const SOCKET: &str = "/Users/dev/.docker/run/docker.sock";

fn refused() -> String {
    format!("dial unix {SOCKET}: connect: operation not permitted\n")
}

fn unnamed() -> Vec<(Block, Option<String>)> {
    vec![(
        Block {
            operation: Operation::Connect,
            kind: Kind::LocalSocket,
            ours: false,
        },
        None,
    )]
}

/// Claude Code's result that did not fail, and Codex's one string.
fn results(command: &str) -> [serde_json::Value; 2] {
    [
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": command},
            "tool_response": {"stdout": refused(), "stderr": "", "interrupted": false},
        }),
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": command},
            "tool_response": refused(),
        }),
    ]
}

fn raises(command: &str, cwd: &Path) -> bool {
    results(command).iter().all(|payload| {
        let found = detect_with_targets(payload, &place_in(cwd));
        assert!(
            found.is_empty() || found == unnamed(),
            "{command}: {found:?}"
        );
        found == unnamed()
    })
}

/// The ticket's case: a reading git command, in a repository whose own config names an
/// fsmonitor hook, a diff or textconv driver, a filter, or a signature program, may have run
/// that program, so a refusal it handed back raises its block. Read from the repository's
/// folder, a folder under it, after a `cd` on the line, or with `git -C`.
#[test]
fn a_git_reader_whose_repository_names_a_driver_raises_its_block() {
    for config in [
        "\tfsmonitor = ./ping\n",
        "[diff]\n\texternal = ./ping\n",
        "[diff \"pdf\"]\n\ttextconv = ./ping\n",
        "[diff \"pdf\"]\n\tcommand = ./ping\n",
        "[filter \"crypt\"]\n\tclean = ./ping\n",
        "[filter \"crypt\"]\n\tprocess = ./ping\n",
        "[gpg]\n\tprogram = ./ping\n",
        "[log]\n\tshowSignature = true\n",
        "[hook \"ping\"]\n\tcommand = ./ping\n\tevent = post-index-change\n",
        "[extensions]\n\tpartialClone = origin\n",
        "[remote \"origin\"]\n\tpromisor = true\n",
        // git's older spelling of a subsection, and a key on the header's line.
        "[diff.pdf]\n\ttextconv = ./ping\n",
        "[diff \"pdf\"] textconv = ./ping\n",
    ] {
        let dir = repo(config);
        let at = dir.path().join("repo");
        std::fs::create_dir_all(at.join("src")).expect("a folder under it");
        for (command, cwd) in [
            ("git status", at.clone()),
            ("git log -p | tail -40", at.clone()),
            ("git diff", at.join("src")),
            ("cd repo && git show HEAD", dir.path().to_path_buf()),
            ("git -C repo blame src/x.rs", dir.path().to_path_buf()),
            ("sh -c 'cd repo; git grep needle'", dir.path().to_path_buf()),
            ("cd -P -- repo && git status", dir.path().to_path_buf()),
        ] {
            assert!(raises(command, &cwd), "{config:?}: {command}");
        }
    }
}

/// A driver named in a file the config includes, and the config of a linked work tree's
/// common git directory, count the same; so does a hook git runs when a reader refreshes the
/// index.
#[test]
fn a_driver_an_include_a_linked_work_tree_or_a_hook_names_raises_its_block() {
    let dir = repo("[include]\n\tpath = ../drivers.inc\n");
    std::fs::write(
        dir.path().join("repo/drivers.inc"),
        "[diff]\n\texternal = ./ping\n",
    )
    .expect("the included file");
    assert!(raises("git diff", &dir.path().join("repo")));

    let dir = repo("[filter \"crypt\"]\n\tsmudge = ./ping\n");
    let linked = dir.path().join("repo/.git/worktrees/side");
    std::fs::create_dir_all(&linked).expect("the linked git dir");
    std::fs::write(linked.join("commondir"), "../..\n").expect("its commondir");
    let side = dir.path().join("side");
    std::fs::create_dir_all(&side).expect("the linked work tree");
    std::fs::write(side.join(".git"), format!("gitdir: {}\n", linked.display()))
        .expect("its .git file");
    assert!(raises("git status", &side));

    let dir = repo("");
    std::fs::write(
        dir.path().join("repo/.git/hooks/post-index-change"),
        "#!/bin/sh\n./ping\n",
    )
    .expect("the hook");
    assert!(raises("git status", &dir.path().join("repo")));

    let dir = repo("\thooksPath = .husky/_\n");
    let husky = dir.path().join("repo/.husky/_");
    std::fs::create_dir_all(&husky).expect("the hooks folder");
    std::fs::write(husky.join("post-index-change"), "#!/bin/sh\n./ping\n").expect("the hook");
    assert!(raises("git status", &dir.path().join("repo")));

    // A relative hooks folder in a work tree whose `.git` is a file (a submodule's, a separate
    // git folder's) is read against that work tree.
    let dir = repo("\thooksPath = hooks-here\n");
    let module = dir.path().join("module");
    std::fs::create_dir_all(module.join("hooks-here")).expect("the work tree");
    std::fs::write(
        module.join(".git"),
        format!("gitdir: {}\n", dir.path().join("repo/.git").display()),
    )
    .expect("its .git file");
    assert!(!raises("git status", &module));
    std::fs::write(module.join("hooks-here/post-index-change"), "").expect("the hook");
    assert!(raises("git status", &module));
}

/// A `.git` file that names no folder, or one that cannot be read, is a repository that
/// could not be read: its reader runs a program. No `.git` up to the root is no repository.
#[test]
fn a_git_file_that_cannot_be_followed_counts_as_naming_a_program() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join(".git"), "not a pointer\n").expect("a .git file");
    assert!(raises("git log", dir.path()));
    std::fs::write(dir.path().join(".git"), "gitdir: \n").expect("an empty pointer");
    assert!(raises("git log", dir.path()));
}

/// A repository whose config names no program still only reads, as do the builtin fsmonitor,
/// a sample hook, and a hooks folder with no hook a reader runs.
#[test]
fn a_git_reader_in_a_repository_that_names_none_still_only_reads() {
    for config in [
        "",
        "\tfsmonitor = true\n",
        "\tfsmonitor = false\n",
        "\tpager = ./ping\n",
        "[remote \"origin\"]\n\turl = https://example.com/x.git\n[branch \"main\"]\n\tremote = origin\n",
        "[diff]\n\tcolorMoved = zebra\n",
        "[log]\n\tshowSignature = false\n",
        "[include]\n\tpath = ../missing.inc\n",
        // A hooks folder of its own (husky's) with no hook a reader runs.
        "\thooksPath = .husky/_\n",
    ] {
        let dir = repo(config);
        std::fs::write(
            dir.path().join("repo/.git/hooks/post-index-change.sample"),
            "#!/bin/sh\n",
        )
        .expect("a sample hook");
        let at = dir.path().join("repo");
        for (command, cwd) in [
            ("git status", at.clone()),
            ("git log -p | tail -40", at.clone()),
            ("cd repo && git show HEAD", dir.path().to_path_buf()),
            ("git -C repo blame src/x.rs", dir.path().to_path_buf()),
        ] {
            assert!(!raises(command, &cwd), "{config:?}: {command}");
        }
    }
}

/// Outside any repository a git reader only reads; a folder the line names in a way that is
/// not read (a variable, `cd -`), or a repository or config named past its folder, may be any
/// repository, so its reader runs a program.
#[test]
fn a_git_reader_whose_repository_is_not_read_runs_a_program() {
    let outside = tempfile::tempdir().expect("a temp dir");
    assert!(!raises("cd sub && git -C there log | tail", outside.path()));
    assert!(!raises("cd -P -- sub && git log", outside.path()));
    assert!(raises("cd - && git log", outside.path()));
    assert!(raises("git -C \"$X\" log", outside.path()));
    assert!(!raises("git log", outside.path()));
    assert!(raises("cd \"$REPO\" && git log", outside.path()));
    assert!(raises("git --git-dir=elsewhere/.git log", outside.path()));
    assert!(raises("GIT_DIR=elsewhere/.git git log", outside.path()));
    assert!(raises("HOME=elsewhere git log", outside.path()));
}

/// The config entries read, at the git folder: a folder named other than `.git` holds them
/// here, so this runs where a test may not make a `.git` folder.
#[test]
fn a_git_folders_config_is_read_for_the_programs_it_names() {
    let names = |config: &str| {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("config"), config).expect("the config");
        drivers::git_dir_names_a_program(dir.path(), None, None)
    };
    for config in [
        "[core]\n\tfsmonitor = ./ping\n",
        "[diff]\n\texternal = ./ping\n",
        "[diff \"pdf\"]\n\ttextconv = ./ping\n",
        "[diff.pdf]\n\tcommand = ./ping\n",
        "[diff \"pdf\"] textconv = ./ping\n",
        "[filter \"crypt\"]\n\tsmudge = ./ping\n",
        "[gpg \"ssh\"]\n\tprogram = ./ping\n",
        "[log]\n\tshowSignature = yes\n",
        "[hook \"ping\"]\n\tevent = post-index-change\n",
        // Relative to a work tree whose top is not known.
        "[core]\n\thooksPath = rel\n",
        "[extensions]\n\tpartialClone = origin\n",
        "[remote \"origin\"]\n\tpromisor = true\n",
    ] {
        assert!(names(config), "{config:?}");
    }
    for config in [
        "",
        "[core]\n\tfsmonitor = true\n\tpager = ./ping\n",
        "[diff]\n\tcolorMoved = zebra\n",
        "[log]\n\tshowSignature = false\n",
        "[remote \"origin\"]\n\turl = https://example.com/x.git\n\tpromisor = false\n",
        "[include]\n\tpath = missing.inc\n",
        "[core]\n\thooksPath = /nonexistent/hooks\n",
        "[hook \"lint\"]\n\tcommand = ./ping\n\tevent = pre-commit\n",
    ] {
        assert!(!names(config), "{config:?}");
    }
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("config"),
        "[includeIf \"gitdir:x/\"]\n\tpath = a.inc\n",
    )
    .expect("the config");
    std::fs::write(dir.path().join("a.inc"), "[include]\n\tpath = b.inc\n").expect("a.inc");
    std::fs::write(dir.path().join("b.inc"), "[diff]\n\texternal = ./ping\n").expect("b.inc");
    assert!(drivers::git_dir_names_a_program(dir.path(), None, None));
    let linked = dir.path().join("worktrees/side");
    std::fs::create_dir_all(&linked).expect("a linked git folder");
    std::fs::write(linked.join("commondir"), "../..\n").expect("its commondir");
    assert!(drivers::git_dir_names_a_program(&linked, None, None));
    let hooked = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir_all(hooked.path().join("hooks")).expect("hooks");
    std::fs::write(hooked.path().join("hooks/post-index-change.sample"), "").expect("sample");
    assert!(!drivers::git_dir_names_a_program(hooked.path(), None, None));
    std::fs::write(hooked.path().join("hooks/post-index-change"), "").expect("hook");
    assert!(drivers::git_dir_names_a_program(hooked.path(), None, None));
}

/// What cannot be read counts as naming a program, since the answer decides whether a refusal
/// is raised: a config that is not a regular file, one that cannot be opened, one past the
/// bound, an include past git's depth, a `~/` include with no home, and a reading past its
/// budget of files. A file that is not there names none.
#[test]
fn a_config_that_cannot_be_read_counts_as_naming_a_program() {
    let names = |dir: &Path| drivers::git_dir_names_a_program(dir, None, None);

    let dir = tempfile::tempdir().expect("a temp dir");
    assert!(!names(dir.path()), "no config at all");
    std::fs::create_dir(dir.path().join("config")).expect("a folder named config");
    assert!(names(dir.path()), "a config that is a folder");

    let dir = tempfile::tempdir().expect("a temp dir");
    let big = format!(
        "[core]\n\tbare = false\n#{}\n",
        "x".repeat(crate::gitconfig::MAX_CONFIG_BYTES as usize)
    );
    std::fs::write(dir.path().join("config"), big).expect("a big config");
    assert!(names(dir.path()), "a config past the bound");

    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("config"), "[core]\n\tbare = false\n").expect("the config");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let config = dir.path().join("config");
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000))
            .expect("unreadable");
        // Root reads it anyway: only where it cannot be opened is that the case asked.
        if std::fs::File::open(&config).is_err() {
            assert!(names(dir.path()), "a config that cannot be opened");
        }
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600))
            .expect("readable again");
    }

    // A chain of includes one past git's depth.
    let dir = tempfile::tempdir().expect("a temp dir");
    let deepest = drivers::MOST_INCLUDES_DEEP + 1;
    std::fs::write(dir.path().join("config"), "[include]\n\tpath = 1.inc\n").expect("config");
    for n in 1..=deepest {
        std::fs::write(
            dir.path().join(format!("{n}.inc")),
            format!("[include]\n\tpath = {}.inc\n", n + 1),
        )
        .expect("an include");
    }
    assert!(names(dir.path()), "an include past git's depth");
    // The same chain within it, ending in a file that is not there, names none.
    std::fs::write(
        dir.path()
            .join(format!("{}.inc", drivers::MOST_INCLUDES_DEEP - 1)),
        "[core]\n\tbare = false\n",
    )
    .expect("the chain's end");
    assert!(!names(dir.path()), "a chain within git's depth");

    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("config"), "[include]\n\tpath = ~/x.inc\n").expect("config");
    assert!(names(dir.path()), "a ~/ include with no home");
}

/// A config that includes itself twice fans out past any budget git's depth would allow; the
/// reading stops at [`drivers::MOST_FILES`] and counts it as naming a program.
#[test]
fn a_self_including_config_spends_the_budget_and_counts() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("config"),
        "[include]\n\tpath = config\n\tpath = config\n",
    )
    .expect("config");
    let started = std::time::Instant::now();
    assert!(drivers::git_dir_names_a_program(dir.path(), None, None));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));

    // Many includes of files that are not there spend it too.
    let dir = tempfile::tempdir().expect("a temp dir");
    let many: String = (0..drivers::MOST_FILES)
        .map(|n| format!("\tpath = missing-{n}.inc\n"))
        .collect();
    std::fs::write(dir.path().join("config"), format!("[include]\n{many}")).expect("config");
    assert!(drivers::git_dir_names_a_program(dir.path(), None, None));
}
