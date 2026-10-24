// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_brokered_call_reads_no_global_or_system_config_and_only_the_identity_it_is_given() {
    let dir = tempfile::tempdir().expect("a directory");
    let listed = |dir: &Path| {
        run(dir, &["config", "--list", "--show-scope"], READ)
            .expect("git runs")
            .out
    };
    let held = Isolated::identity(Some("Op Erator"), Some("op@e.invalid"));
    let seen = isolated(&held, || listed(dir.path()));
    assert!(
        seen.lines()
            .all(|line| !line.starts_with("global") && !line.starts_with("system")),
        "{seen}"
    );
    assert!(seen.contains("command\tuser.name=Op Erator"), "{seen}");
    assert!(seen.contains("command\tuser.email=op@e.invalid"), "{seen}");
    assert_eq!(isolation(), None, "the isolation outlived its call");
    assert!(
        !listed(dir.path()).contains("user.name=Op Erator"),
        "an ordinary call read the brokered identity"
    );
}

/// #1415: every call, brokered or not, uses a bare repository only where it is named.
#[test]
fn no_call_finds_a_bare_repository_it_was_not_given() {
    let dir = tempfile::tempdir().expect("a directory");
    let bare = dir.path().join("planted");
    std::fs::create_dir(&bare).expect("a folder");
    assert!(
        crate::testgit::run(&bare, &["init", "-q", "--bare", "."]).ok(),
        "a bare repository"
    );
    let asked = |held: Option<&Isolated>| {
        within(held, || {
            run(&bare, &["rev-parse", "--absolute-git-dir"], READ).expect("git runs")
        })
    };
    let plain = asked(None);
    assert!(
        !plain.ok(),
        "an ordinary call found the bare repository: {plain:?}"
    );
    let brokered = asked(Some(&Isolated::default()));
    assert!(!brokered.ok(), "a brokered call found it: {brokered:?}");
    // A test's own fixture git may, as it reads its bare remotes.
    assert!(crate::testgit::run(&bare, &["rev-parse", "--absolute-git-dir"]).ok());
}

/// #1415: a pin is held to the directory that was checked, not to whatever has its name.
#[test]
fn a_pin_holds_only_while_its_git_directory_is_the_one_checked() {
    let dir = tempfile::tempdir().expect("a directory");
    let tree = dir.path().join("clone");
    let git_dir = tree.join(".git");
    std::fs::create_dir_all(&git_dir).expect("a git directory");
    let pinned = Isolated::default().pinned(&tree, &git_dir);
    assert!(pinned.holds());

    // Renamed away, and another put in its place under the same name.
    std::fs::rename(&git_dir, tree.join("checked.git")).expect("renamed");
    std::fs::create_dir(&git_dir).expect("another");
    assert!(
        !pinned.holds(),
        "another directory passed as the one checked"
    );

    // Every call in the tree is refused before git runs, and names why.
    let refused = isolated(&pinned, || run(&tree, &["status"], READ).expect("answered"));
    assert_eq!(refused.code, Some(128), "{refused:?}");
    assert!(refused.err.contains("was replaced"), "{refused:?}");
    // Pinned to what is there now, it holds again.
    assert!(Isolated::default().pinned(&tree, &git_dir).holds());
}

#[test]
fn a_pin_to_an_identity_taken_before_is_refused_once_the_directory_changed() {
    let dir = tempfile::tempdir().expect("a directory");
    let git_dir = dir.path().join("clone/.git");
    std::fs::create_dir_all(&git_dir).expect("a git directory");
    let checked = DirId::of(&git_dir);
    // Kept elsewhere, so its inode is not the new one's.
    std::fs::rename(&git_dir, dir.path().join("kept.git")).expect("moved");
    std::fs::create_dir(&git_dir).expect("back, as another");
    let pinned = Isolated::default().pinned_as(&dir.path().join("clone"), &git_dir, checked);
    assert!(!pinned.holds());
}

#[test]
fn an_identity_that_would_break_a_line_is_left_out() {
    assert_eq!(
        Isolated::identity(Some("a\nb"), Some("")),
        Isolated::default()
    );
}

fn lookup_in<'a>(
    vars: &'a [(&'a str, &'a str)],
) -> impl Fn(&str) -> Option<std::ffi::OsString> + 'a {
    move |name| {
        vars.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| std::ffi::OsString::from(*v))
    }
}

#[test]
fn a_session_git_sees_the_config_the_session_relocated_and_nothing_else() {
    let vars = [
        ("XDG_CONFIG_HOME", "/x"),
        ("GIT_CONFIG_GLOBAL", "/g"),
        ("GIT_DIR", "/elsewhere/.git"),
        ("GIT_EXEC_PATH", "/evil"),
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "alias.co"),
        ("GIT_CONFIG_VALUE_0", "checkout"),
    ];
    let got: Vec<String> = config_location_env(lookup_in(&vars))
        .into_iter()
        .map(|(k, v)| format!("{k}={}", v.to_string_lossy()))
        .collect();
    assert_eq!(
        got,
        [
            "XDG_CONFIG_HOME=/x",
            "GIT_CONFIG_GLOBAL=/g",
            "GIT_CONFIG_COUNT=1",
            "GIT_CONFIG_KEY_0=alias.co",
            "GIT_CONFIG_VALUE_0=checkout",
        ]
    );
}

#[test]
fn a_config_count_naming_a_pair_that_is_not_there_is_withheld_whole() {
    let vars = [
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "a.b"),
        ("GIT_CONFIG_VALUE_0", "c"),
    ];
    assert!(config_location_env(lookup_in(&vars)).is_empty());
    let vars = [("GIT_CONFIG_COUNT", "65")];
    assert!(config_location_env(lookup_in(&vars)).is_empty());
    let vars = [("GIT_CONFIG_COUNT", "x")];
    assert!(config_location_env(lookup_in(&vars)).is_empty());
}

#[test]
fn a_session_git_keeps_bytes_that_are_not_utf8() {
    let dir = repo();
    let config = dir.path().join(".git").join("config");
    let mut text = std::fs::read(&config).unwrap();
    text.extend_from_slice(b"[alias]\n\tco = checkout \xff\n");
    std::fs::write(&config, text).unwrap();
    let raw = run_as_session(dir.path(), &["config", "--get", "alias.co"], READ).unwrap();
    assert_eq!(raw.code, Some(0));
    assert_eq!(raw.out, b"checkout \xff\n");
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    crate::testgit::run(dir.path(), &["init", "-q", "-b", "main", "."]);
    dir
}

/// The marker that tells a re-executed copy of this test binary it is the child.
const CHILD: &str = "CHARTER_TEST_HOSTILE_CHILD";

#[test]
fn a_hostile_environment_does_not_reach_git_through_the_runner() {
    // `env_clear` is the load-bearing line, and asserting on `child_env` alone cannot see
    // it — a mutation that deleted `env_clear` passed every test in this module. So the
    // hostile environment is set on a CHILD PROCESS: this test re-executes the test
    // binary with it, which needs no `unsafe` (the workspace forbids it) and is sound
    // against the concurrent `getenv` that `std::env::set_var` is not.
    if std::env::var_os(CHILD).is_some() {
        // The child. Its environment is hostile in every way the parent could make it,
        // including HOME — which is where git reads a config that names programs, and is
        // the one variable the runner cannot simply drop.
        let dir = repo();
        std::fs::write(dir.path().join("f"), "x").unwrap();
        run(dir.path(), &["add", "f"], READ).unwrap();
        run(
            dir.path(),
            &[
                "-c",
                "user.email=t@e.invalid",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "one",
            ],
            READ,
        )
        .unwrap();

        let ran = std::env::var_os("CHARTER_TEST_HOOK_MARKER").expect("the marker path");
        let ran = std::path::PathBuf::from(ran);
        let wt = dir.path().join("wt");
        let cut = run_untimed(
            dir.path(),
            &["worktree", "add", "-b", "probe", &wt.display().to_string()],
        )
        .unwrap();

        assert!(cut.ok(), "the call still worked: {cut:?}");
        assert!(
            !ran.exists(),
            "a hook named by the hostile HOME's gitconfig ran"
        );
        // And the fsmonitor, which `status` would run on every dirt check.
        let _ = run(dir.path(), &["status", "--porcelain"], READ).unwrap();
        assert!(!ran.exists(), "the fsmonitor named by that config ran");
        return;
    }

    // The parent. It builds a hostile HOME with a gitconfig that names programs, and a
    // hostile environment, then re-executes this test inside both.
    let home = tempfile::tempdir().expect("a home");
    let hooks = home.path().join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let marker = home.path().join("RAN");
    let script = format!("#!/bin/sh\ntouch {}\n", marker.display());
    // Through `stand_in::program`: these are run the moment they are written, and a
    // program this process wrote through its own descriptor can lose to `ETXTBSY`
    // (charter-app#81).
    for hook in ["post-checkout", "query-watchman"] {
        stand_in::program(&hooks, hook, &script);
    }
    std::fs::write(
        home.path().join(".gitconfig"),
        format!(
            "[core]\n\thooksPath = {}\n\tfsmonitor = {}\n",
            hooks.display(),
            hooks.join("query-watchman").display()
        ),
    )
    .unwrap();

    let trace = std::env::temp_dir().join(format!(
        "charter-hostile-trace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    let me = std::env::current_exe().expect("the test binary");
    let out = crate::forklock::output(
        Command::new(me)
            .args([
                "--exact",
                "worktree::git::tests::a_hostile_environment_does_not_reach_git_through_the_runner",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("CHARTER_TEST_HOOK_MARKER", &marker)
            .env("HOME", home.path())
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "core.hooksPath")
            .env("GIT_CONFIG_VALUE_0", &hooks)
            .env("GIT_EXEC_PATH", "/tmp/charter-test-evil-exec")
            .env("GIT_TRACE", &trace),
    )
    .expect("the test binary re-runs");

    assert!(
        out.status.success(),
        "the hostile environment reached git:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !trace.exists(),
        "GIT_TRACE reached git and it wrote {} outside the plane",
        trace.display()
    );
}

#[test]
fn every_variable_git_calls_repository_local_is_absent_from_the_child() {
    // Held to GIT's own answer rather than to a list someone maintains. This is the check
    // on the allowlist, not the mechanism: the mechanism is `env_clear`.
    let dir = repo();
    let printed = run(dir.path(), &["rev-parse", "--local-env-vars"], READ).unwrap();
    assert!(!printed.out.trim().is_empty(), "git named its variables");

    for name in printed.out.split_whitespace() {
        let seen = run(dir.path(), &["config", "--get", "nothing.here"], READ);
        assert!(seen.is_ok(), "{name}");
    }
    // The real assertion: the child's whole environment, read back through git itself.
    let env = child_env("/usr/bin");
    for name in printed.out.split_whitespace() {
        assert!(
            !env.iter().any(|(k, _)| *k == name),
            "{name} is repository-local and must not be given to the child"
        );
    }
}

/// A `git` planted where only a relative `PATH` entry reaches it, with no git in the
/// fixed directories — the one arrangement in which the fallback search decides.
///
/// **Reached through a relative path to a temporary directory, not through `.`.** `.`
/// and an empty entry are both the working directory, and a test cannot move the
/// process's working directory without moving it under every other test running beside
/// it. A relative entry is a relative entry to the search — it is resolved against the
/// same working directory — so this plants where one reaches and puts `.` and an empty
/// entry in front of it as well.
#[cfg(unix)]
#[test]
fn a_git_that_only_a_relative_path_entry_reaches_is_never_the_one_charter_runs() {
    let fixed = tempfile::tempdir().unwrap();
    let planted = tempfile::tempdir().unwrap();
    stand_in::program(planted.path(), "git", "#!/bin/sh\nexit 0\n");
    let here = std::env::current_dir().unwrap().canonicalize().unwrap();
    let up = "../".repeat(here.components().count() - 1);
    let relative = format!(
        "{up}{}",
        planted.path().display().to_string().trim_start_matches('/')
    );
    assert!(
        Path::new(&relative).join("git").is_file(),
        "the planted git is reachable through {relative}"
    );

    let fixed_dir = fixed.path().display().to_string();
    let path = format!(".::{relative}");
    let (binary, child_path) =
        git_binary_in(&[fixed_dir.as_str()], Some(std::ffi::OsStr::new(&path)));
    assert_eq!(
        binary,
        PathBuf::from("git"),
        "a relative PATH entry chose the binary charter hands credentials to"
    );
    for dir in std::env::split_paths(&child_path) {
        assert!(dir.is_absolute(), "{} in {child_path}", dir.display());
    }
}

#[test]
fn a_childs_path_splits_back_into_exactly_the_directories_it_was_built_from() {
    let dirs: Vec<String> = GIT_DIRS.iter().map(|d| (*d).to_string()).collect();
    let back: Vec<String> = std::env::split_paths(&path_value(&dirs))
        .map(|d| d.display().to_string())
        .collect();
    assert_eq!(
        back, dirs,
        "joined with this platform's separator, not a literal `:`"
    );
}

#[cfg(unix)]
#[test]
fn a_directory_with_the_separator_in_it_is_left_out_rather_than_split_in_two() {
    let dirs = vec!["/home/a:b/.local/bin".to_string(), "/usr/bin".to_string()];
    assert_eq!(
        path_value(&dirs),
        "/usr/bin",
        "joined, it would reach the child as `/home/a` and a RELATIVE `b/.local/bin`"
    );
}

#[test]
fn where_a_person_keeps_their_git_config_is_given_to_the_child() {
    // #1476: global excludes kept at a custom XDG path are read, so a file they ignore is
    // not warned of as one that would be committed.
    let var = |name: &str| match name {
        "HOME" => Some("/home/a".into()),
        "XDG_CONFIG_HOME" => Some("/home/a/conf".into()),
        "GIT_CONFIG_GLOBAL" => Some("/elsewhere".into()),
        _ => None,
    };
    let env = child_env_from("/usr/bin", &var);
    assert!(env.contains(&("XDG_CONFIG_HOME", "/home/a/conf".to_owned())));
    assert!(!env.iter().any(|(k, _)| *k == "GIT_CONFIG_GLOBAL"));
    let unset = child_env_from("/usr/bin", &|name: &str| {
        (name == "XDG_CONFIG_HOME").then(std::ffi::OsString::new)
    });
    assert!(!unset.iter().any(|(k, _)| *k == "XDG_CONFIG_HOME"));
}

#[test]
fn nothing_that_names_a_program_git_runs_is_given_to_the_child() {
    // The execution surface, which `--local-env-vars` does not cover.
    let env = child_env("/usr/bin");
    for name in [
        "GIT_EXEC_PATH",
        "GIT_TRACE",
        "GIT_TRACE2",
        "GIT_TRACE2_EVENT",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "SSH_ASKPASS",
        "GIT_PROXY_COMMAND",
        "GIT_EXTERNAL_DIFF",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG",
        "GIT_DIR",
    ] {
        assert!(
            !env.iter().any(|(k, _)| *k == name),
            "{name} must not be given to the child"
        );
    }
}

#[test]
fn a_status_through_the_runner_never_takes_the_index_lock_even_without_the_flag() {
    // FD-11 (#651): `status` is the one read that writes the index — its refresh, through
    // `index.lock` — so the runner holds back git's optional locks for every `status`,
    // and a new reader that forgets `--no-optional-locks` cannot start a lock fight.
    let dir = repo();
    let index = dir.path().join(".git/index");
    for n in 0..10 {
        std::fs::write(dir.path().join(format!("{n}.txt")), "same\n").unwrap();
    }
    assert!(crate::testgit::run(dir.path(), &["add", "-A"]).ok());
    let was = std::fs::read(&index).unwrap();
    // Past a coarse clock, the same bytes again: stat data the index no longer matches.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    for n in 0..10 {
        std::fs::write(dir.path().join(format!("{n}.txt")), "same\n").unwrap();
    }

    let seen = run(
        dir.path(),
        &["-c", "core.quotePath=false", "status", "--porcelain"],
        READ,
    )
    .unwrap();

    assert!(seen.ok(), "{seen:?}");
    assert_eq!(
        std::fs::read(&index).unwrap(),
        was,
        "the index was rewritten"
    );
    // The control: git's own `status`, not through the runner and allowed its optional
    // lock, does rewrite it.
    let plain = crate::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["status", "--porcelain"]),
    )
    .unwrap();
    assert!(plain.status.success(), "{plain:?}");
    assert_ne!(
        std::fs::read(&index).unwrap(),
        was,
        "the control saw no rewrite"
    );
}

/// `git config --list --show-scope -z`, from `(scope, key, value)` triples.
fn listing(entries: &[(&str, &str, &str)]) -> Vec<u8> {
    entries
        .iter()
        .map(|(scope, key, value)| format!("{scope}\0{key}\n{value}\0"))
        .collect::<String>()
        .into_bytes()
}

/// D-1805-ruling: a call that would sign, in a repository whose own config names the signer,
/// is refused before git runs, with the sentence that says where the signer belongs.
#[test]
fn a_signed_call_where_the_repository_names_its_signer_is_refused_saying_where_it_belongs() {
    let names = String::from_utf8(listing(&[
        ("global", "user.name", "op"),
        ("local", "gpg.ssh.program", "./ssh-signer"),
    ]))
    .unwrap();
    assert!(repo_names_a_signer(&names));
    assert!(would_sign(&["commit", "-S", "-m", "x"], &names));
    assert!(would_sign(&["tag", "-s", "v1", "-m", "x"], &names));
    assert!(!would_sign(&["commit", "-m", "x"], &names));
    assert!(!would_sign(&["status"], &names));

    // `commit.gpgSign` from any scope, the command line's included, and the last one wins.
    let signs = String::from_utf8(listing(&[
        ("local", "gpg.program", "./signer"),
        ("global", "commit.gpgsign", "false"),
        ("command", "commit.gpgsign", "true"),
    ]))
    .unwrap();
    assert!(would_sign(&["commit", "-m", "x"], &signs));
    assert!(!would_sign(&["commit", "--no-gpg-sign", "-m", "x"], &signs));

    // The operator's own signer, in their global config, is theirs to run.
    let theirs = String::from_utf8(listing(&[
        ("global", "gpg.ssh.program", "/usr/local/bin/op-ssh-sign"),
        ("global", "commit.gpgsign", "true"),
    ]))
    .unwrap();
    assert!(!repo_names_a_signer(&theirs));

    assert!(SIGNER_IN_REPO.contains("names a signing program"));
    assert!(SIGNER_IN_REPO.contains("global git config"));
    assert!(SIGNER_IN_REPO.contains("includeIf"));
}

#[test]
fn every_program_the_repository_names_is_held_and_its_filter_is_no_longer_required() {
    let held = held_programs(
        false,
        &listing(&[
            ("local", "filter.Mixed.Case.clean", "./clean"),
            ("local", "filter.Mixed.Case.smudge", "./smudge"),
            ("local", "filter.Mixed.Case.required", "true"),
            ("worktree", "filter.p.process", "./process"),
            ("local", "diff.t.textconv", "./textconv"),
            ("local", "diff.x.command", "./command"),
            ("local", "diff.external", "./external"),
            ("local", "merge.x.driver", "./merge %O %A %B"),
            ("local", "merge.x.name", "a name, not a program"),
            ("local", "core.quotepath", "false"),
            ("local", "gpg.program", "./signer"),
            ("local", "gpg.ssh.program", "./ssh-signer"),
            ("local", "gpg.ssh.defaultkeycommand", "./key"),
            ("local", "core.editor", "./editor"),
            ("local", "sequence.editor", "./sequence"),
            ("local", "core.alternaterefscommand", "./refs"),
            ("local", "trailer.sign.cmd", "./trailer"),
            ("local", "trailer.sign.command", "./trailer"),
            // Read by a remote verb only, which is refused where one is named.
            ("local", "remote.origin.uploadpack", "./upload-pack"),
        ]),
    );
    assert_eq!(
        held,
        Some(
            [
                "filter.Mixed.Case.clean=",
                "filter.Mixed.Case.smudge=",
                "filter.p.process=",
                "diff.t.textconv=",
                "diff.x.command=",
                "diff.external=",
                "merge.x.driver=",
                "gpg.program=",
                "gpg.ssh.program=",
                "gpg.ssh.defaultkeycommand=",
                "core.editor=",
                "sequence.editor=",
                "core.alternaterefscommand=",
                "trailer.sign.cmd=",
                "trailer.sign.command=",
                "filter.Mixed.Case.required=false",
                "filter.p.required=false",
            ]
            .map(str::to_owned)
            .to_vec()
        )
    );
}

#[test]
fn every_hook_the_repository_configures_is_turned_off_by_its_name() {
    // A hook named in config (newer git) is not looked up through `core.hooksPath`, so it
    // is turned off by its own name. A name the repository gives an event or a command is
    // enough: an event added to the operator's own hook is the repository's choice too.
    let held = held_programs(
        false,
        &listing(&[
            ("local", "hook.Mine.command", "./hook"),
            ("local", "hook.Mine.event", "post-checkout"),
            ("local", "hook.Mine.event", "pre-commit"),
            ("worktree", "hook.theirs.event", "reference-transaction"),
            (
                "global",
                "hook.theirs.command",
                "/usr/local/bin/operators-hook",
            ),
            ("local", "hook.Mine.enabled", "true"),
            ("local", "hook.jobs", "4"),
        ]),
    );
    assert_eq!(
        held,
        Some(
            ["hook.Mine.enabled=false", "hook.theirs.enabled=false"]
                .map(str::to_owned)
                .to_vec()
        )
    );
    // The operator's own hooks, and purlis's command line, are left alone.
    assert_eq!(
        held_programs(
            false,
            &listing(&[
                ("global", "hook.op.command", "./op"),
                ("global", "hook.op.event", "pre-commit"),
                ("command", "hook.cli.command", "./cli"),
            ]),
        ),
        Some(Vec::new())
    );
    // A name a command line cannot carry refuses the call.
    assert_eq!(
        held_programs(false, &listing(&[("local", "hook.a=b.command", "./x")])),
        None
    );
}

#[test]
fn a_verb_that_writes_a_ref_is_given_the_listing_and_one_that_only_reads_is_not() {
    // A ref written fires the reference-transaction hook, which a hook named in config
    // runs whatever `core.hooksPath` says.
    for writes in [
        &["update-ref", "refs/heads/x", "HEAD"][..],
        &["update-ref", "-d", "refs/heads/x"],
        &["branch", "x"],
        &["branch", "-f", "release", "HEAD"],
        &["branch", "-d", "--", "x"],
        &["branch", "-m", "a", "b"],
        // A format is no listing of its own: this makes `made`.
        &["branch", "--format=%(refname)", "made"],
        &["symbolic-ref", "HEAD", "refs/heads/x"],
        &["symbolic-ref", "--delete", "HEAD"],
    ] {
        assert!(reaches_a_program(writes), "{writes:?}");
    }
    for reads in [
        &["branch", "--show-current"][..],
        &["branch"],
        &["branch", "-a", "-v"],
        &["branch", "--list", "x*"],
        &["branch", "--contains", "HEAD"],
        &["branch", "--format=%(refname)", "--merged", "main"],
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        &["symbolic-ref", "HEAD"],
        &["rev-parse", "HEAD"],
    ] {
        assert!(!reaches_a_program(reads), "{reads:?}");
    }
}

#[test]
fn the_operators_own_drivers_and_purlis_s_command_line_are_left_alone() {
    let held = held_programs(
        false,
        &listing(&[
            ("global", "filter.x.clean", "./clean"),
            ("system", "diff.t.textconv", "./textconv"),
            ("unknown", "merge.x.driver", "./merge"),
            ("command", "diff.external", "./external"),
        ]),
    );
    assert_eq!(held, Some(Vec::new()));
}

#[test]
fn the_standard_lfs_filter_runs_and_any_other_lfs_program_is_held() {
    let standard = [
        ("local", "filter.lfs.clean", "git-lfs clean -- %f"),
        ("local", "filter.lfs.smudge", "git-lfs smudge -- %f"),
        ("local", "filter.lfs.process", "git-lfs filter-process"),
        ("local", "filter.lfs.required", "true"),
    ];
    assert_eq!(held_programs(false, &listing(&standard)), Some(Vec::new()));

    let mut changed = standard;
    changed[0].2 = "git-lfs clean -- %f; ./planted";
    assert_eq!(
        held_programs(false, &listing(&changed)),
        Some(
            ["filter.lfs.clean=", "filter.lfs.required=false"]
                .map(str::to_owned)
                .to_vec()
        )
    );
}

#[test]
fn lfs_is_held_whole_where_the_repository_names_a_program_lfs_would_run() {
    for (key, value) in [
        ("lfs.customtransfer.x.path", "./agent"),
        ("lfs.extension.x.clean", "./clean"),
        ("lfs.extension.x.smudge", "./smudge"),
        ("lfs.standalonetransferagent", "x"),
        ("credential.helper", "./helper"),
        ("credential.https://host.helper", "./helper"),
        ("core.sshcommand", "./ssh"),
        ("core.askpass", "./askpass"),
    ] {
        // The operator's own LFS, which the repository's key would otherwise reach.
        let held = held_programs(
            false,
            &listing(&[
                ("global", "filter.lfs.process", "git-lfs filter-process"),
                ("local", key, value),
            ]),
        );
        assert_eq!(
            held,
            Some(
                [
                    "filter.lfs.clean=",
                    "filter.lfs.smudge=",
                    "filter.lfs.process=",
                    "filter.lfs.required=false",
                ]
                .map(str::to_owned)
                .to_vec()
            ),
            "{key}"
        );
    }
    // The operator's own credential helper is theirs, and LFS may ask it.
    assert_eq!(
        held_programs(
            false,
            &listing(&[("global", "credential.helper", "osxkeychain")])
        ),
        Some(Vec::new())
    );
}

#[test]
fn a_recent_objects_hook_the_repository_names_is_never_reached() {
    // #1853: `gc.recentObjectsHook` is a list of commands, so an empty `-c` adds one rather
    // than clearing them. What reaches it is held instead: the automatic gc and maintenance
    // a commit or a fetch starts are turned off, and a verb that collects garbage itself is
    // refused.
    let named = listing(&[("local", "gc.recentobjectshook", "./hook")]);
    assert_eq!(
        held_programs(false, &named),
        Some(vec![
            "gc.auto=0".to_owned(),
            "maintenance.auto=false".to_owned()
        ])
    );
    let said = String::from_utf8(named.clone()).unwrap();
    for collects in [
        &["gc"][..],
        &["gc", "--auto"],
        &["repack", "-a", "-d"],
        &["prune"],
        &["maintenance", "run"],
        &["pack-objects", "--cruft", "x"],
    ] {
        assert!(
            collects_garbage_with_a_hook(collects, &said),
            "{collects:?}"
        );
    }
    for reads in [&["commit", "-m", "x"][..], &["status"], &[]] {
        assert!(!collects_garbage_with_a_hook(reads, &said), "{reads:?}");
    }
    // The operator's own is theirs.
    let theirs = listing(&[("global", "gc.recentobjectshook", "./hook")]);
    assert_eq!(held_programs(false, &theirs), Some(Vec::new()));
    let theirs = String::from_utf8(theirs).unwrap();
    assert!(!collects_garbage_with_a_hook(&["gc"], &theirs));
}

#[test]
fn a_remote_verb_is_refused_where_the_repository_names_what_a_remote_starts() {
    // `vcs` picks the remote helper git starts for the remote (#1853).
    for key in [
        "remote.origin.uploadpack",
        "remote.up.receivepack",
        "remote.origin.vcs",
    ] {
        let named = listing(&[("local", key, "./program")]);
        assert_eq!(held_programs(true, &named), None, "{key}");
        assert_eq!(held_programs(false, &named), Some(Vec::new()), "{key}");
        // The operator's own is theirs.
        let theirs = listing(&[("global", key, "./program")]);
        assert_eq!(held_programs(true, &theirs), Some(Vec::new()));
    }
}

#[test]
fn a_remote_is_reached_by_the_verbs_that_ask_one_and_not_by_reading_its_config() {
    for asks in [
        &["fetch", "origin"][..],
        &["pull"],
        &["push", "origin", "main"],
        &["ls-remote", "origin"],
        &["remote", "update"],
        &["remote", "-v", "show", "origin"],
        &["remote", "set-head", "origin", "-a"],
    ] {
        assert!(reaches_a_remote(asks), "{asks:?}");
    }
    for reads in [
        &["remote", "get-url", "origin"][..],
        &["remote", "add", "origin", "https://example.invalid/x.git"],
        &["remote"],
        &["status"],
        &[],
    ] {
        assert!(!reaches_a_remote(reads), "{reads:?}");
    }
}

#[test]
fn a_driver_named_so_that_a_command_line_cannot_carry_it_refuses_the_call() {
    assert_eq!(
        held_programs(false, &listing(&[("local", "filter.a=b.clean", "./clean")])),
        None
    );
}

#[test]
fn a_patch_verb_runs_no_external_diff_and_a_repository_driver_is_never_started() {
    // Through the runner, in a repository whose own config names a program for each
    // driver the committed attributes turn on. Its git directory is kept beside it.
    let case = tempfile::tempdir().unwrap();
    let store = case.path().join("store");
    let work = case.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let store_arg = format!("--separate-git-dir={}", store.display());
    assert!(crate::testgit::run(&work, &["init", "-q", "-b", "main", &store_arg, "."]).ok());
    let dir = &work;
    let ran = case.path().join("ran");
    let program = case.path().join("planted.sh");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho \"$0\" >> '{}'\n",
            ran.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(dir.join(".gitattributes"), "f.txt filter=x diff=x\n").unwrap();
    std::fs::write(dir.join("f.txt"), "one\n").unwrap();
    assert!(crate::testgit::run(dir, &["add", "."]).ok());
    // With an identity: a runner's machine may have none.
    let committed = crate::testgit::run(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.invalid",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    );
    assert!(committed.ok(), "{committed:?}");
    std::fs::write(dir.join("f.txt"), "two\n").unwrap();
    let shown = program.display().to_string();
    for key in [
        "filter.x.clean",
        "filter.x.smudge",
        "diff.x.textconv",
        "diff.x.command",
        "diff.external",
    ] {
        assert!(crate::testgit::run(dir, &["config", key, &shown]).ok());
    }
    assert!(crate::testgit::run(dir, &["config", "filter.x.required", "true"]).ok());

    let diff = run(dir, &["diff"], READ).unwrap();
    assert!(diff.ok(), "{diff:?}");
    assert!(diff.out.contains("+two"), "not git's own diff: {diff:?}");
    let status = run(dir, &["status", "--porcelain"], READ).unwrap();
    assert!(status.ok(), "{status:?}");
    assert!(!ran.exists(), "a driver the repository named was started");
}

#[test]
fn the_verb_is_found_past_the_options_before_it() {
    assert_eq!(verb(&["status", "--porcelain"]), Some("status"));
    assert_eq!(
        verb(&["-c", "a=b", "--no-optional-locks", "status"]),
        Some("status")
    );
    assert_eq!(verb(&["-C", "status", "log"]), Some("log"));
    assert_eq!(verb(&["--git-dir=x", "add", "status"]), Some("add"));
    assert_eq!(verb(&["--no-pager"]), None);
}

#[test]
fn an_option_given_its_value_as_the_next_word_takes_that_word_with_it() {
    // #874: git reads `--git-dir X` as `--git-dir=X`, so `X` is never the verb.
    for option in [
        "--git-dir",
        "--work-tree",
        "--namespace",
        "--config-env",
        "--super-prefix",
        "--attr-source",
    ] {
        assert_eq!(
            verb(&[option, "status", "log"]),
            Some("log"),
            "{option} takes the next word"
        );
        assert_eq!(verb_at(&["-c", "a=b", option, "x", "status"]), Some(4));
    }
    assert_eq!(
        verb(&["--git-dir", "x", "--work-tree", "y", "worktree", "repair"]),
        Some("worktree")
    );
    // Left with no value, there is no verb.
    assert_eq!(verb(&["--git-dir"]), None);
    assert_eq!(verb(&["--git-dir", "x"]), None);
}

#[test]
fn output_larger_than_a_pipe_buffer_does_not_deadlock_the_deadline() {
    // ~64 KiB is where an undrained pipe blocks the writer, and a wait that cannot
    // complete becomes a timeout that looks like a slow machine.
    let dir = repo();
    // Long names on purpose: `status --porcelain` writes one line per file, and the
    // point is to pass the ~64 KiB where an undrained pipe blocks the writer.
    let long = "n".repeat(60);
    for i in 0..2000 {
        std::fs::write(dir.path().join(format!("{long}{i}")), "x").unwrap();
    }

    let seen = run(dir.path(), &["status", "--porcelain"], READ).unwrap();

    assert_eq!(seen.code, Some(0), "it finished rather than timing out");
    assert!(
        seen.out.len() > 64 * 1024,
        "and it really was past the buffer: {} bytes",
        seen.out.len()
    );
}

#[test]
fn a_failed_call_carries_its_code_and_its_stderr_with_nobody_reading_the_english() {
    let dir = tempfile::tempdir().unwrap();
    let answer = run(dir.path(), &["rev-parse", "--git-dir"], READ).unwrap();
    assert_eq!(answer.code, Some(128));
    assert!(!answer.err.is_empty());
}

#[test]
fn a_network_call_refuses_ssh_even_when_config_rewrites_https_to_it() {
    // The operator's own `url.<ssh>.insteadOf <https>` is the everyday way a clone of an
    // HTTPS URL ends up on SSH. Local config stands in for the global file here: `-c`
    // on the command line beats both.
    let dir = repo();
    run(
        dir.path(),
        &[
            "config",
            "url.ssh://git@127.0.0.1:9/.insteadOf",
            "https://example.invalid/",
        ],
        READ,
    )
    .unwrap();

    let fetched = run_network(
        dir.path(),
        None,
        &["fetch", "https://example.invalid/acme/x.git"],
    )
    .unwrap();

    assert!(!fetched.ok(), "{fetched:?}");
    // From the operator's global file git refuses the transport. Planted in the repository's
    // own config, as here, the rewrite to another host is refused before git runs (P-82-1).
    // Either way nothing connects.
    assert!(
        fetched.err.contains("transport 'ssh' not allowed")
            || fetched
                .err
                .contains("rewrites a network address to another host"),
        "the call was refused, and said why: {:?}",
        fetched.err
    );
}

#[test]
fn a_network_call_asks_only_the_helper_it_was_given() {
    // One credential: a helper the repo's (or the operator's) config names — a keychain,
    // a store file, another forge's CLI — is never consulted. git passes `-c` settings on
    // to a git it runs, so a `credential fill` run from inside the call sees exactly the
    // helper list the call itself would use.
    let dir = repo();
    let theirs = dir.path().join("theirs-asked");
    let mine = dir.path().join("mine-asked");
    run(
        dir.path(),
        &[
            "config",
            "credential.helper",
            &format!("!echo asked >> '{}'", theirs.display()),
        ],
        READ,
    )
    .unwrap();
    let fill =
        "alias.fill=!printf 'protocol=https\\nhost=example.invalid\\n\\n' | git credential fill";

    run_network(
        dir.path(),
        Some(&format!("!echo asked >> '{}'", mine.display())),
        &["-c", fill, "fill"],
    )
    .unwrap();

    assert!(mine.exists(), "the helper the call was given was asked");
    assert!(!theirs.exists(), "the helper config named was never asked");
}

/// The marker that tells a re-executed copy of this test binary it is the credential
/// child.
const CREDENTIAL_CHILD: &str = "CHARTER_TEST_CREDENTIAL_CHILD";

#[test]
fn a_network_call_hands_the_forge_cli_its_credential_and_no_other_call_does() {
    // git runs the credential helper with git's own environment, so a `gh` that keeps its
    // token in `GH_TOKEN` gets nothing unless the NETWORK call passes it. Every other call
    // must not: a read has no use for a token. Re-executed, because setting a variable in
    // this process is `unsafe` and would leak into every other test.
    if std::env::var_os(CREDENTIAL_CHILD).is_some() {
        let dir = repo();
        let seen = |name: &str| dir.path().join(name);
        let dump = |file: &std::path::Path| format!("alias.dump=!env > '{}'", file.display());
        let network = seen("network.env");
        let read = seen("read.env");
        run_network(dir.path(), None, &["-c", &dump(&network), "dump"]).unwrap();
        run(dir.path(), &["-c", &dump(&read), "dump"], READ).unwrap();

        let network = std::fs::read_to_string(network).expect("the alias ran");
        let read = std::fs::read_to_string(read).expect("the alias ran");
        assert!(network.contains("GH_TOKEN=tok-under-test"), "{network}");
        assert!(!read.contains("tok-under-test"), "{read}");
        assert!(
            !network.contains("CHARTER_TEST_NOT_A_CREDENTIAL"),
            "{network}"
        );
        return;
    }
    let me = std::env::current_exe().expect("the test binary");
    let out = crate::forklock::output(
            Command::new(me)
                .args([
                    "--exact",
                    "worktree::git::tests::a_network_call_hands_the_forge_cli_its_credential_and_no_other_call_does",
                    "--nocapture",
                ])
                .env(CREDENTIAL_CHILD, "1")
                .env("GH_TOKEN", "tok-under-test")
                .env("CHARTER_TEST_NOT_A_CREDENTIAL", "1"),
        )
        .expect("the test binary re-runs");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_call_stopped_at_its_deadline_returns_then_and_what_git_started_is_ended_with_it() {
    // `clone` runs `git-remote-https`, which runs the credential helper. Ending git alone
    // left them running, holding the pipes, and a reader that waits for every writer
    // would have outlived the deadline; here it left a `sleep 30` behind each run.
    let dir = repo();
    let said = dir.path().join("what-the-alias-started");
    // Should the runner leave it running after all, the test does not.
    let _ends = stand_in::Ends::named_in(&said);
    let hang = format!(
        "alias.hang=!sleep 30 & echo $! > '{0}.part'; mv '{0}.part' '{0}'; sleep 30",
        said.display()
    );
    let started = Instant::now();
    let answer = run(dir.path(), &["-c", &hang, "hang"], Duration::from_secs(1)).unwrap();

    assert_eq!(answer.code, None, "the deadline passed");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it returned near its deadline, not when the grandchild let go: {:?}",
        started.elapsed()
    );
    // A git too slow to have started its alias within the deadline started nothing.
    #[cfg(unix)]
    if let Some(background) = stand_in::pid_named_in(&said, Duration::from_secs(5)) {
        assert!(
            stand_in::is_gone(background, Duration::from_secs(10)),
            "what git started outlived the call that was stopped"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_child_that_left_the_group_holding_the_pipes_does_not_hold_the_call() {
    // What the group's end does not reach: a program that made a group of its own. It
    // keeps both pipes open, and the call still answers near its deadline. The test ends
    // it, by the id it wrote.
    let dir = repo();
    let said = dir.path().join("what-left-the-group");
    let ends = stand_in::Ends::named_in(&said);
    let hang = format!(
        "alias.hang=!perl -e 'setpgrp(0,0); sleep 20' & echo $! > '{0}.part'; \
             mv '{0}.part' '{0}'; sleep 30",
        said.display()
    );
    let started = Instant::now();
    let answer = run(dir.path(), &["-c", &hang, "hang"], Duration::from_secs(1)).unwrap();

    assert_eq!(answer.code, None, "the deadline passed");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it returned near its deadline, not when the one that left let go: {:?}",
        started.elapsed()
    );
    let left = stand_in::pid_named_in(&said, Duration::from_secs(5));
    drop(ends);
    if let Some(left) = left {
        assert!(
            stand_in::is_gone(left, Duration::from_secs(10)),
            "the test left its own fixture running"
        );
    }
}

/// Two linked worktrees of a repository kept outside any workspace, laid out as git lays
/// them out (no git is run): `sib`, whose entry names it back, and `moved`, whose entry
/// names a folder that is no longer there.
struct Siblings {
    _dir: tempfile::TempDir,
    root: PathBuf,
    sib: PathBuf,
    moved: PathBuf,
}

fn siblings() -> Siblings {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("resolves");
    let common = root.join("outside/.git");
    let entry = |name: &str, names: &Path| {
        let entry = common.join("worktrees").join(name);
        std::fs::create_dir_all(&entry).expect("an entry");
        std::fs::write(entry.join("commondir"), "../..\n").expect("commondir");
        std::fs::write(
            entry.join("gitdir"),
            format!("{}\n", names.join(".git").display()),
        )
        .expect("gitdir");
        entry
    };
    let sib = root.join("sib");
    let moved = root.join("moved");
    for (folder, at) in [
        (&sib, entry("sib", &sib)),
        (&moved, entry("moved", &root.join("was-here"))),
    ] {
        std::fs::create_dir_all(folder).expect("a folder");
        std::fs::write(folder.join(".git"), format!("gitdir: {}\n", at.display())).expect("a link");
    }
    Siblings {
        _dir: dir,
        root,
        sib,
        moved,
    }
}

/// Whether the runner refused the call in `dir` with one of the link check's sentences,
/// before any git ran.
fn refused_by_the_link(dir: &Path, args: &[&str]) -> Option<String> {
    let run = run(dir, args, READ).expect("answered");
    let said = run.err.trim().to_owned();
    [super::super::link::CHANGED, super::super::link::MOVED]
        .contains(&said.as_str())
        .then_some(said)
}

#[test]
fn a_connecting_verb_is_refused_outside_the_network_entry_point_before_git_runs() {
    // #1853: the one-credential rule, the network settings and the network log live on
    // `run_network`; any other runner refuses a verb that can reach another repository,
    // before git starts. A plain folder is enough: nothing is run to be refused.
    let dir = tempfile::tempdir().unwrap();
    let here = dir.path();
    let said = |raw: RawRun| {
        assert_eq!(raw.code, Some(128));
        String::from_utf8_lossy(&raw.err).trim().to_owned()
    };
    for args in [
        &["fetch", "origin"][..],
        &["push", "origin", "main"],
        &["-c", "a.b=c", "pull"],
        &["clone", "--", "/nowhere", "copy"],
        &["ls-remote", "origin"],
        &["remote", "update"],
        &["submodule", "update", "--init"],
    ] {
        let answers = [
            as_text_raw(run(here, args, READ).unwrap()),
            as_text_raw(run_untimed(here, args).unwrap()),
            as_text_raw(run_until(here, args, READ, &AtomicBool::new(false)).unwrap()),
            run_with_input(here, args, Vec::new(), READ).unwrap(),
            run_as_session(here, args, READ).unwrap(),
            run_in_hook(here, args, READ).unwrap(),
        ];
        for answer in answers {
            assert_eq!(said(answer), connecting::OFF_THE_NETWORK_ENTRY, "{args:?}");
        }
        assert!(
            matches!(spawn_fed(here, args), Ok(Err(_))),
            "a fed call is refused too: {args:?}"
        );
    }
    // The local-only lookups the network log uses are kept.
    for args in [
        &["ls-remote", "--get-url", "origin"][..],
        &["remote", "get-url", "--push", "origin"],
    ] {
        let answer = run(here, args, READ).unwrap();
        assert!(
            !answer.err.contains(connecting::OFF_THE_NETWORK_ENTRY),
            "{args:?}: {answer:?}"
        );
    }
    // And the network entry point runs them.
    let fetched = run_network(here, None, &["fetch", "origin"]).unwrap();
    assert!(!fetched.err.contains(connecting::OFF_THE_NETWORK_ENTRY));
}

fn as_text_raw(run: Run) -> RawRun {
    RawRun {
        code: run.code,
        out: run.out.into_bytes(),
        err: run.err.into_bytes(),
    }
}

#[test]
fn the_runners_that_answer_a_chats_own_git_questions_are_held_to_the_link_check() {
    // #1476: the guards' and the hook scan's runners refuse where every other runner
    // does, with the same sentence, before any git runs. A copied link is not named back
    // by its repository's record, which for any verb but a bare repair is the moved
    // folder's sentence (`link::checked`).
    use super::super::link::MOVED;
    let s = siblings();
    let impostor = s.root.join("impostor");
    std::fs::create_dir_all(&impostor).expect("a folder");
    std::fs::copy(s.sib.join(".git"), impostor.join(".git")).expect("a copied link");
    for (dir, said) in [(&s.moved, MOVED), (&impostor, MOVED)] {
        for raw in [
            run_as_session(dir, &["status", "--porcelain"], READ),
            run_in_hook(dir, &["status", "--porcelain"], READ),
        ] {
            let raw = raw.expect("answered");
            assert_eq!(raw.code, Some(128), "{}", dir.display());
            assert_eq!(String::from_utf8_lossy(&raw.err).trim(), said);
        }
    }
}

#[test]
fn the_repair_exemption_reaches_a_moved_folder_s_bare_repair_and_nothing_else() {
    use super::super::link::{CHANGED, MOVED};
    let s = siblings();
    // In the folder that was moved, any other verb, the words as another verb's
    // arguments, and the path form are held to the whole check.
    for args in [
        &["status", "--porcelain"][..],
        &["rev-parse", "worktree", "repair"][..],
        // The path form repairs the folders it names, not this one.
        &["worktree", "repair", "../sib"][..],
        &["worktree", "repair", "--", "."][..],
    ] {
        assert_eq!(
            refused_by_the_link(&s.moved, args).as_deref(),
            Some(MOVED),
            "{args:?}"
        );
    }
    // A folder whose link names an entry another folder still holds was not moved: a
    // repair there would take that folder's record.
    let impostor = s.root.join("impostor");
    std::fs::create_dir_all(&impostor).expect("a folder");
    std::fs::copy(s.sib.join(".git"), impostor.join(".git")).expect("a copied link");
    assert_eq!(
        refused_by_the_link(&impostor, &["worktree", "repair"]).as_deref(),
        Some(CHANGED)
    );
    assert_eq!(
        std::fs::read_to_string(s.root.join("outside/.git/worktrees/sib/gitdir")).unwrap(),
        format!("{}\n", s.sib.join(".git").display()),
        "the sibling's record was rewritten"
    );
    // The one thing let through: a bare `worktree repair` in the folder that was moved.
    // Asked last, since git may write the folder's record then.
    assert_eq!(refused_by_the_link(&s.moved, &["worktree", "repair"]), None);
    assert_eq!(
        refused_by_the_link(&s.moved, &["-c", "a.b=c", "worktree", "repair"]),
        None
    );
}
