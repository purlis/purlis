// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The floor (#2071, the operator's ruling of 2026-10-14): the default for a project that has
//! not turned the sandbox on. Every host and every folder but what it holds; still no vault
//! read, nothing a later program runs written, and no folder on the person's `PATH` written.

use super::*;
use serde_json::Value;
use std::io::Write as _;

/// A project that has not turned the sandbox on, with a plain-file vault registered.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path().canonicalize().expect("a real folder");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    (dir, root)
}

/// A macOS machine whose `PATH` holds `bin`, a folder outside the project.
fn machine_with(bin: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    }
}

fn decided(harness: Harness, root: &Path, machine: &Machine) -> Option<Decided> {
    decide(harness, root, machine, &|_| true, None, None).expect("decided")
}

fn floor_of(decided: Option<Decided>) -> Applied {
    match decided {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not the floor: {other:?}"),
    }
}

fn settings_of(applied: &Applied) -> claude::Settings {
    match applied.form() {
        Form::ClaudeCode(settings) => settings.clone(),
        other => panic!("not Claude Code's: {other:?}"),
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_project_that_has_not_turned_the_sandbox_on_starts_its_chats_at_the_floor() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    assert_eq!(applied.level(), Level::Floor);
    // No host is listed: the chat's proxy carries every one ([`reach::Reach::anything`]).
    assert!(applied.confines().hosts.is_empty());
    let settings = settings_of(&applied);
    let network = &settings.sandbox["network"];
    // Nothing limits the network: Claude Code restricts a command's only while domains are set.
    assert!(network.get("allowedDomains").is_none(), "{network}");
    assert!(network.get("strictAllowlist").is_none(), "{network}");
    assert!(network.get("httpProxyPort").is_none(), "{network}");
    assert_eq!(network["allowAllUnixSockets"], Value::Bool(false));
    // Nothing limits the folders written but what it holds.
    assert_eq!(
        strings(&settings.sandbox["filesystem"]["allowWrite"]),
        ["/"]
    );
    // Its web tools are its own: not denied.
    assert!(!settings.deny.iter().any(|rule| rule == "WebFetch"));
    // Its own browser tools are, at the floor too (#2013): purlis's browser is the one the
    // person watches.
    assert!(
        settings
            .deny
            .iter()
            .any(|rule| rule == "mcp__claude-in-chrome")
    );
}

#[test]
fn the_floor_still_denies_the_vaults_and_what_a_later_program_runs() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let machine = machine_with(bin.path());
    let applied = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    let settings = settings_of(&applied);
    let deny_read = strings(&settings.sandbox["filesystem"]["denyRead"]);
    let deny_write: Vec<String> = strings(&settings.sandbox["filesystem"]["denyWrite"])
        .iter()
        .map(|it| claude::uncased(it))
        .collect();
    // The same classes the full sandbox denies, path for path.
    let full = Denied::of(&root, &machine);
    for denial in full.paths.iter().filter(|d| d.access == Access::ReadWrite) {
        let path = denial.path.display().to_string();
        assert!(deny_read.contains(&path), "{path} in {deny_read:?}");
    }
    for name in [".git/hooks", ".claude/settings.json"] {
        assert!(
            deny_write.iter().any(|rule| rule.ends_with(name)),
            "{name} in {deny_write:?}"
        );
    }
    assert!(
        settings.deny.iter().any(|rule| rule.starts_with("Read(")),
        "its tools are held to the vaults too"
    );
}

#[test]
fn the_floor_keeps_a_chat_from_writing_a_folder_on_the_persons_path() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    let machine = machine_with(bin.path());
    let applied = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    assert!(
        applied.confines().held.contains(&held),
        "{:?}",
        applied.confines().held
    );
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&held.display().to_string()),
        "{held:?} in {deny_write:?}"
    );
    // A folder a person let the chat write (its Allow) is not held.
    assert!(!floor_held(&root, &machine, std::slice::from_ref(&held)).contains(&held));
    // And without the sandbox (a person's opt-out), nothing is held.
    let off = decide(
        Harness::ClaudeCode,
        &root,
        &machine,
        &|_| true,
        Some(&OptOut::default()),
        None,
    )
    .expect("decided");
    assert!(matches!(off, Some(Decided::Unsandboxed(_))), "{off:?}");
}

#[test]
fn the_floor_never_holds_the_project_or_a_folder_above_the_home() {
    let (_dir, root) = project();
    let inside = root.join("node_modules/.bin");
    std::fs::create_dir_all(&inside).expect("a folder in the project");
    let machine = Machine {
        env: crate::secrets::Env::of(&[(
            "PATH",
            &format!("{}:{}:/", inside.display(), root.display()),
        )]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    };
    let held = floor_held(&root, &machine, &[]);
    assert!(
        !held
            .iter()
            .any(|dir| dir.starts_with(&root) || root.starts_with(dir)),
        "{held:?}"
    );
}

#[test]
fn the_floor_holds_the_program_its_chat_starts_and_the_full_sandbox_leaves_it_to_the_check() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let elsewhere = tempfile::tempdir().expect("an install folder");
    let program = real(elsewhere.path()).join("claude");
    std::fs::write(&program, "").expect("a program");
    let mut applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    applied.hold_programs(&[program.display().to_string()]);
    let folder = real(elsewhere.path());
    assert!(applied.confines().held.contains(&folder));
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&folder.display().to_string()),
        "{deny_write:?}"
    );
}

#[test]
fn a_person_may_still_start_a_chat_without_the_floor_and_it_is_audited() {
    let (_dir, root) = project();
    let decided = decide(
        Harness::ClaudeCode,
        &root,
        &machine_with(Path::new("/opt/none")),
        &|_| true,
        Some(&OptOut {
            reason: Some("why not".to_owned()),
        }),
        None,
    );
    assert_eq!(
        decided,
        Ok(Some(Decided::Unsandboxed(Lifted {
            by: By::Person,
            reason: Some("why not".to_owned()),
        })))
    );
}

#[test]
fn where_the_floor_cannot_be_applied_the_chat_starts_as_before_and_says_why() {
    let (_dir, root) = project();
    // A Linux kernel without the Landlock purlis's own wrap needs (#1040).
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    let no_landlock = |program: &str| program != backend::LANDLOCK;
    match decide(Harness::Codex, &root, &linux, &no_landlock, None, None) {
        // Its reason alone: not the refusal's "nothing starts", which is not so here.
        Ok(Some(Decided::Floorless(why))) => assert_eq!(
            why,
            "This chat runs with no sandbox: purlis could not apply the floor here, because \
             this machine cannot apply the sandbox: purlis's sandbox needs Landlock version 6 \
             (Linux 6.12 or later), and this kernel does not have it."
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_project_that_turned_the_sandbox_on_keeps_the_full_sandbox() {
    let (_dir, root) = project();
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    )
    .expect("charter.toml");
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    assert_eq!(applied.level(), Level::Full);
    let settings = settings_of(&applied);
    assert!(settings.sandbox["network"].get("allowedDomains").is_some());
    assert!(settings.deny.iter().any(|rule| rule == "WebFetch"));
}

/// The floor profile of a wrapped harness for a chat in `root`, reporting on `socket`, with its
/// proxy on `ports`.
fn wrapped_floor(
    harness: Harness,
    root: &Path,
    machine: &Machine,
    ports: &[u16],
    socket: Option<&Path>,
) -> String {
    let compiled = Compiled::floor(root, machine, &grant::Grants::default());
    let compile = compiler(harness).expect("a compiler");
    match compile(&compiled).expect("compiled") {
        Form::Codex(wrap) => codex::floor_profile(&wrap, ports, socket),
        Form::Opencode(wrap) => opencode::floor_profile(&wrap, ports, socket),
        Form::ClaudeCode(_) | Form::ClaudeWrapped(_) => unreachable!(),
    }
    .expect("a profile")
}

#[test]
fn a_wrapped_harness_at_the_floor_writes_everything_but_what_the_floor_holds() {
    use super::tests::writes;
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    let machine = machine_with(bin.path());
    let socket = root.join(".purlis/app/hooks.sock");
    for harness in [Harness::Codex, Harness::Opencode] {
        let profile = wrapped_floor(harness, &root, &machine, &[4040, 4041], Some(&socket));
        // Every folder written…
        assert!(writes(&profile, &root.join("src/main.rs")), "{harness:?}");
        assert!(
            writes(&profile, Path::new("/Users/op/scratch/a.txt")),
            "{harness:?}"
        );
        // …but a PATH folder, the hook socket's folder, and what a later program runs, at any
        // depth and at the person's own level.
        for not in [
            held.join("tool"),
            socket.parent().expect("a folder").join("other"),
            root.join("repo/.git/hooks/pre-commit"),
            PathBuf::from("/Users/op/.gitconfig"),
            PathBuf::from("/Users/op/.config/git/config"),
            PathBuf::from("/Users/op/.ssh/config"),
            PathBuf::from("/Users/op/.ssh/authorized_keys"),
            // A name spelled another way is the same file on a case-folding disk.
            PathBuf::from("/Users/op/library/launchagents/x.plist"),
            PathBuf::from("/Users/op/.GitConfig"),
            PathBuf::from("/Users/op/Library/LaunchAgents/x.plist"),
            PathBuf::from("/Users/op/.zshrc"),
            PathBuf::from("/Users/op/.config/fish/config.fish"),
            PathBuf::from("/Users/op/.claude/settings.json"),
            PathBuf::from("/Users/op/.claude/hooks/run.sh"),
            PathBuf::from("/Users/op/.codex/config.toml"),
            PathBuf::from("/Users/op/.config/opencode/opencode.json"),
            PathBuf::from("/Users/op/Library/Application Support/Code/User/tasks.json"),
            PathBuf::from("/Users/op/Library/Application Support/Cursor/User/settings.json"),
        ] {
            assert!(
                !writes(&profile, &not),
                "{harness:?}: {} is writable",
                not.display()
            );
        }
        // ssh adds to its known hosts on a first connection: that stays writable.
        assert!(
            writes(&profile, Path::new("/Users/op/.ssh/known_hosts")),
            "{harness:?}"
        );
    }
}

/// The folders of the home Docker's installers link `docker` and its credential helpers into,
/// which purlis runs outside any sandbox for a Docker pool, are never a chat's to write
/// (#2752): each as written and where a link in its place leads. A project's own `.docker`
/// folder, and Docker's settings beside them, stay the chat's.
#[test]
fn the_homes_docker_folders_are_never_written_by_a_chat_and_a_projects_own_docker_folder_is() {
    use super::tests::writes;
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let home_dir = tempfile::tempdir().expect("a home");
    let home = real(home_dir.path());
    // OrbStack's folder is a link to where it keeps its programs.
    let elsewhere = tempfile::tempdir().expect("where a link leads");
    let kept = real(elsewhere.path());
    std::fs::create_dir_all(home.join(".orbstack")).expect("a folder");
    std::os::unix::fs::symlink(&kept, home.join(".orbstack/bin")).expect("a link");
    let machine = Machine {
        home: Some(home.clone()),
        ..machine_with(bin.path())
    };
    for harness in [Harness::Codex, Harness::Opencode] {
        let profile = wrapped_floor(harness, &root, &machine, &[4040, 4041], None);
        for folder in crate::dockerpool::DOCKER_HOME_FOLDERS {
            for not in [
                home.join(folder),
                home.join(folder).join("docker"),
                home.join(folder).join("docker-credential-desktop"),
            ] {
                assert!(
                    !writes(&profile, &not),
                    "{harness:?}: {} is writable",
                    not.display()
                );
            }
        }
        assert!(
            !writes(&profile, &kept.join("docker")),
            "{harness:?}: where the link leads is writable"
        );
        for still in [
            home.join(".docker/config.json"),
            home.join(".docker/buildx/activity/default"),
            root.join(".docker/compose.yaml"),
            root.join(".docker/bin/build.sh"),
        ] {
            assert!(
                writes(&profile, &still),
                "{harness:?}: {} is not writable",
                still.display()
            );
        }
    }
    // Claude Code's own sandbox holds each by its path in the home.
    let floor = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    let deny_write = strings(&settings_of(&floor).sandbox["filesystem"]["denyWrite"]);
    for folder in crate::dockerpool::DOCKER_HOME_FOLDERS {
        let path = home.join(folder).display().to_string();
        assert!(deny_write.contains(&path), "{path} in {deny_write:?}");
    }
    assert!(
        deny_write.contains(&kept.display().to_string()),
        "{} in {deny_write:?}",
        kept.display()
    );
    assert!(
        !deny_write
            .iter()
            .any(|rule| rule.ends_with("**/.docker/bin")),
        "{deny_write:?}"
    );
}

/// The home's Docker folders are held under every spelling a case-folding disk takes (#2752):
/// where one is not there yet, a chat cannot make it under another spelling and put a program
/// in it. The folder that holds each `bin` is held as an entry alone (the person's ruling of
/// 2026-10-20): never made, moved aside or replaced, while what is in it beside `bin` is
/// written.
#[test]
fn the_homes_docker_folders_are_held_in_every_spelling_and_their_holders_as_entries() {
    use super::tests::writes;
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    // None of the folders is there yet.
    let home_dir = tempfile::tempdir().expect("a home");
    let home = real(home_dir.path());
    let machine = Machine {
        home: Some(home.clone()),
        ..machine_with(bin.path())
    };
    for harness in [Harness::Codex, Harness::Opencode] {
        let profile = wrapped_floor(harness, &root, &machine, &[4040, 4041], None);
        for not in [
            ".Docker/Bin/docker",
            ".docker/BIN/docker-credential-desktop",
            ".ORBSTACK/BIN/docker",
            ".Rd/bin/docker",
            // Each holder, as itself and in another spelling.
            ".docker",
            ".DOCKER",
            ".orbstack",
            ".rd",
            ".RD",
        ] {
            assert!(
                !writes(&profile, &home.join(not)),
                "{harness:?}: {not} is writable"
            );
        }
        // The engine's socket each installer keeps in the home, which a run of `docker` is
        // pointed at where the system has none (#2752).
        for not in [
            ".docker/run/docker.sock",
            ".Docker/Run/docker.sock",
            ".orbstack/run/docker.sock",
            ".rd/docker.sock",
            ".colima/default/docker.sock",
            ".Lima/default/sock/docker.sock",
        ] {
            assert!(
                !writes(&profile, &home.join(not)),
                "{harness:?}: {not} is writable"
            );
        }
        for still in [
            ".docker/config.json",
            ".Docker/config.json",
            ".docker/buildx/activity/default",
            ".rd/other",
        ] {
            assert!(
                writes(&profile, &home.join(still)),
                "{harness:?}: {still} is not writable"
            );
        }
    }
    let floor = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    let deny_write = strings(&settings_of(&floor).sandbox["filesystem"]["denyWrite"]);
    for folder in crate::dockerpool::DOCKER_HOME_FOLDERS {
        let path = home.join(folder).display().to_string();
        assert!(
            deny_write.contains(&claude::either_case(&path)),
            "{path} in either case in {deny_write:?}"
        );
    }
    // Claude Code holds what is below a path it is given, so a holder held as an entry alone is
    // not given to it: what is beside each `bin` stays written.
    let holder = home.join(".docker").display().to_string();
    assert!(!deny_write.contains(&holder), "{deny_write:?}");
}

/// The checked copies of Docker's settings purlis makes for each run of `docker` hold the
/// person's registry sign-ins: no chat reads or writes them (#2752).
#[test]
fn no_chat_reads_or_writes_the_copies_of_dockers_settings() {
    let (_dir, root) = project();
    let data = tempfile::tempdir().expect("a data home");
    let machine = Machine {
        env: crate::secrets::Env::of(&[(
            crate::datahome::HOME_VAR,
            &data.path().display().to_string(),
        )]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::MacOs,
    };
    let held = Denied::of(&root, &machine)
        .paths
        .into_iter()
        .find(|denial| denial.path.ends_with(crate::dockerpool::settings::DIR))
        .expect("the copies are denied");
    assert_eq!(held.access, Access::ReadWrite);
    assert_eq!(
        held.path,
        data.path().join(crate::dockerpool::settings::DIR)
    );
}

/// Where a Podman machine's helper and a rootless engine keep their socket, in the temporary
/// and the runtime folder, no chat writes either (D-2752-7).
#[test]
fn no_chat_writes_where_a_podman_machine_or_a_rootless_engine_keeps_its_socket() {
    let (_dir, root) = project();
    let machine = Machine {
        env: crate::secrets::Env::of(&[
            ("TMPDIR", "/var/folders/xy/T"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ]),
        home: Some(PathBuf::from("/home/op")),
        os: Os::Linux,
    };
    let denied: Vec<PathBuf> = Denied::of(&root, &machine)
        .paths
        .into_iter()
        .filter(|denial| denial.access == Access::Write)
        .map(|denial| denial.path)
        .collect();
    for held in [
        "/var/folders/xy/T/podman",
        "/run/user/1000/docker.sock",
        "/run/user/1000/podman",
        "/home/op/.colima",
        "/home/op/.lima",
    ] {
        assert!(
            denied.contains(&PathBuf::from(held)),
            "{held} in {denied:?}"
        );
    }
}

/// Podman's own folders are never a chat's to write (#2680, the person's ruling of 2026-10-20):
/// its settings, its storage and its run state, under every spelling on macOS; the folders
/// beside them stay the chat's.
#[test]
fn podmans_folders_are_never_written_by_a_chat() {
    use super::tests::writes;
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let home_dir = tempfile::tempdir().expect("a home");
    let home = real(home_dir.path());
    let config = home.join("cfg");
    let path = bin.path().display().to_string();
    let config_said = config.display().to_string();
    let machine = Machine {
        env: crate::secrets::Env::of(&[
            ("PATH", path.as_str()),
            ("XDG_CONFIG_HOME", config_said.as_str()),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ]),
        home: Some(home.clone()),
        os: Os::MacOs,
    };
    for harness in [Harness::Codex, Harness::Opencode] {
        let profile = wrapped_floor(harness, &root, &machine, &[4040, 4041], None);
        for not in [
            home.join(".config/containers/containers.conf"),
            home.join(".Config/Containers/containers.conf"),
            config.join("containers/containers.conf.d/x.conf"),
            home.join(".local/share/containers/storage/volumes/x"),
            PathBuf::from("/run/user/1000/containers/auth.json"),
            PathBuf::from("/run/user/1000/libpod/tmp/x"),
        ] {
            assert!(
                !writes(&profile, &not),
                "{harness:?}: {} is writable",
                not.display()
            );
        }
        for still in [
            home.join(".config/other/x"),
            home.join(".local/share/other/x"),
            PathBuf::from("/run/user/1000/other"),
            root.join("containers/x"),
        ] {
            assert!(
                writes(&profile, &still),
                "{harness:?}: {} is not writable",
                still.display()
            );
        }
    }
    // On Linux, whose disks do not fold case, each is held as it is spelled.
    let linux = Machine {
        os: Os::Linux,
        ..machine.clone()
    };
    let held: Vec<Denial> = Denied::of(&root, &linux)
        .paths
        .into_iter()
        .filter(|denial| denial.path.ends_with("containers") || denial.path.ends_with("libpod"))
        .collect();
    assert_eq!(held.len(), 5, "{held:?}");
    assert!(
        held.iter()
            .all(|denial| !denial.any_case && denial.access == Access::Write)
    );
}

/// Where `XDG_RUNTIME_DIR` is unset, as in an ssh session or one without systemd, Podman keeps
/// its run state in `/tmp` by the person's user id, and those folders are held from every
/// chat's writes instead (#2896).
#[test]
fn podmans_run_state_is_held_where_no_runtime_folder_is_named() {
    let (_dir, root) = project();
    let uid = rustix::process::getuid().as_raw();
    let held = |env: &[(&str, &str)]| -> Vec<PathBuf> {
        let machine = Machine {
            env: crate::secrets::Env::of(env),
            home: Some(PathBuf::from("/home/op")),
            os: Os::Linux,
        };
        Denied::of(&root, &machine)
            .paths
            .into_iter()
            .filter(|denial| denial.access == Access::Write)
            .map(|denial| denial.path)
            .collect()
    };
    let fallback = [
        PathBuf::from(format!("/tmp/podman-run-{uid}")),
        PathBuf::from(format!("/tmp/storage-run-{uid}")),
        PathBuf::from(format!("/tmp/containers-user-{uid}")),
    ];
    let without = held(&[]);
    for one in &fallback {
        assert!(without.contains(one), "{} in {without:?}", one.display());
    }
    // With a runtime folder named, its own are held as before, and not the fallback.
    let with = held(&[("XDG_RUNTIME_DIR", "/run/user/1000")]);
    assert!(with.contains(&PathBuf::from("/run/user/1000/containers")));
    assert!(with.contains(&PathBuf::from("/run/user/1000/libpod")));
    for one in &fallback {
        assert!(!with.contains(one), "{}", one.display());
    }
}

/// The floor holds what reaches outside it (#2071 review): the profile applied for real, where
/// this machine can apply one.
#[cfg(target_os = "macos")]
#[test]
fn the_floor_starts_nothing_outside_it_and_reaches_no_socket_or_port_of_this_machine() {
    let can_apply =
        crate::forklock::status(std::process::Command::new(backend::SANDBOX_EXEC).args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success());
    if !can_apply {
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this process is already sandboxed, so no second profile applies"
        );
        return;
    }
    let (_dir, root) = project();
    let sockets = tempfile::tempdir().expect("a folder for sockets");
    let hook = real(sockets.path()).join("hook.sock");
    let other = real(sockets.path()).join("other.sock");
    let _hook = std::os::unix::net::UnixListener::bind(&hook).expect("the hook socket");
    let _other = std::os::unix::net::UnixListener::bind(&other).expect("another socket");
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").expect("a proxy port");
    let service = std::net::TcpListener::bind("127.0.0.1:0").expect("a local service");
    let proxy_port = proxy.local_addr().expect("its port").port();
    let service_port = service.local_addr().expect("its port").port();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    let machine = machine_with(bin.path());
    let profile = wrapped_floor(Harness::Codex, &root, &machine, &[proxy_port], Some(&hook));
    let run = |script: &str| -> String {
        let out = crate::forklock::output(
            std::process::Command::new(backend::SANDBOX_EXEC)
                .args(["-p", &profile, "/bin/sh", "-c", script])
                .current_dir(&root),
        )
        .expect("it runs");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let tried = |what: &str| format!("{what} >/dev/null 2>&1 && echo yes || echo no");
    // Files: written where nothing holds them, and not where the floor does.
    assert_eq!(
        run(&tried(&format!("touch {}/notes.txt", root.display()))),
        "yes"
    );
    assert_eq!(run(&tried(&format!("touch {}/tool", held.display()))), "no");
    // Nothing started outside the profile.
    for program in [
        "/bin/launchctl list",
        "/usr/bin/osascript -e 1",
        "/usr/bin/open -h",
    ] {
        assert_eq!(run(&tried(program)), "no", "{program}");
    }
    // Nor by a copy of a program: what launchd is asked is held, not the program asking.
    let copied = real(sockets.path()).join("l");
    std::fs::copy("/bin/launchctl", &copied).expect("a copy of launchctl");
    assert_eq!(
        run(&tried(&format!(
            "{} submit -l purlis.floor.test -- /usr/bin/true",
            copied.display()
        ))),
        "no",
        "a job handed to launchd"
    );
    // The control (#2533): macOS 26.6 no longer counts `job-creation` in `(allow default)`, so
    // there the floor less its rule refuses a submit too, and only an allow can show what the
    // rule itself does (measured on the macos-latest runner, where `(allow job-creation)` lets
    // a sandboxed submit through). So: with `job-creation` allowed right after the floor's
    // `(allow default)`, as an older macOS counts it, the floor still refuses the submit, and
    // with it allowed in the rule's place the submit goes through. The copy must submit
    // outside any sandbox, or the refusal proves nothing.
    let submits = |wrap: Option<&str>, label: &str| -> bool {
        let script = tried(&format!(
            "{} submit -l {label} -- /usr/bin/true",
            copied.display()
        ));
        let mut command = match wrap {
            Some(profile) => {
                let mut command = std::process::Command::new(backend::SANDBOX_EXEC);
                command.args(["-p", profile, "/bin/sh", "-c", &script]);
                command
            }
            None => {
                let mut command = std::process::Command::new("/bin/sh");
                command.args(["-c", &script]);
                command
            }
        };
        let out = crate::forklock::output(command.current_dir(&root)).expect("it runs");
        let _ = crate::forklock::status(
            std::process::Command::new("/bin/launchctl").args(["remove", label]),
        );
        String::from_utf8_lossy(&out.stdout).trim() == "yes"
    };
    assert!(
        submits(None, "purlis.floor.unsandboxed"),
        "the copy of launchctl submits a job outside any sandbox"
    );
    let base = "(version 1)\n(allow default)\n";
    assert!(profile.starts_with(base), "{profile}");
    let counted = profile.replacen(base, &format!("{base}(allow job-creation)\n"), 1);
    assert!(
        !submits(Some(&counted), "purlis.floor.counted"),
        "the floor refuses a job where (allow default) counts job-creation"
    );
    assert!(profile.contains("(deny job-creation)\n"), "{profile}");
    let allowed = profile.replace("(deny job-creation)\n", "(allow job-creation)\n");
    assert!(
        submits(Some(&allowed), "purlis.floor.control"),
        "the same submit with job-creation allowed in the rule's place"
    );
    let _ = crate::forklock::status(
        std::process::Command::new("/bin/launchctl").args(["remove", "purlis.floor.test"]),
    );
    // A later-code name spelled another way is refused too. In a home of its own: the hook
    // socket's folder is held unwritten whatever is in it, so a home there proves nothing.
    let homes = tempfile::tempdir().expect("a folder for a home");
    let home = real(homes.path()).join("home");
    assert_eq!(
        run(&tried(&format!(
            "mkdir -p {0}/library/launchagents && touch {0}/library/launchagents/x.plist",
            home.display()
        ))),
        "no"
    );
    // The control: a name beside it that nothing holds is written in the same home.
    assert_eq!(
        run(&tried(&format!(
            "mkdir -p {0}/library/launchagentz && touch {0}/library/launchagentz/x.plist",
            home.display()
        ))),
        "yes"
    );
    // The hook socket and the proxy's port, and no other socket or port of this machine.
    let nc = |target: &str| tried(&format!("/usr/bin/nc -z -w 2 {target}"));
    // A socket is connected to by zsh's own module: macOS 26.6's `nc -U -z` fails on a socket
    // that is listening, with no sandbox at all (#2489).
    let connect = |socket: &Path| {
        tried(&format!(
            "/bin/zsh -c 'zmodload zsh/net/socket && zsocket {}'",
            socket.display()
        ))
    };
    assert_eq!(run(&connect(hook.as_path())), "yes");
    assert_eq!(run(&connect(other.as_path())), "no");
    assert_eq!(run(&nc(&format!("127.0.0.1 {proxy_port}"))), "yes");
    assert_eq!(run(&nc(&format!("127.0.0.1 {service_port}"))), "no");
}

#[test]
fn a_claude_code_chat_at_the_floor_reaches_every_host_through_its_proxy() {
    let (_dir, root) = project();
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    let settings = settings_of(&applied);
    // No domain list, so Claude Code's own list and prompts decide nothing; no port of this
    // machine but the proxy's.
    assert!(settings.sandbox["network"].get("allowedDomains").is_none());
    assert_eq!(
        settings.sandbox["network"]["allowLocalBinding"],
        Value::Bool(false)
    );
    let through = settings.through([4040, 4041]);
    assert_eq!(through.sandbox["network"]["httpProxyPort"], 4040);
    // The proxy carries any host, and still no address of this machine.
    let serving = applied.serving(egress::Refusals::default(), None);
    let reach = &serving.reach;
    assert!(reach.decide("example.org", 443, &[443], &[]).carries());
    assert!(
        reach
            .decide("registry.example", 8080, &[443], &[])
            .carries()
    );
    assert!(!reach.decide("127.0.0.1", 6379, &[443], &[]).carries());
}

#[test]
fn a_later_code_name_is_held_in_either_case() {
    assert_eq!(
        seatbelt::either_case(r"\.ssh/rc"),
        r"\.[sS][sS][hH]/[rR][cC]"
    );
}

#[test]
fn the_floor_holds_the_package_a_link_on_the_path_leads_to() {
    assert_eq!(
        package_root(Path::new("/opt/homebrew/Cellar/git/2.50.1/bin/git")),
        PathBuf::from("/opt/homebrew/Cellar/git")
    );
    assert_eq!(
        package_root(Path::new(
            "/opt/homebrew/lib/node_modules/@openai/codex/bin/codex.js"
        )),
        PathBuf::from("/opt/homebrew/lib/node_modules/@openai/codex")
    );
    assert_eq!(
        package_root(Path::new("/Users/op/.local/pipx/venvs/black/bin/black")),
        PathBuf::from("/Users/op/.local/pipx/venvs/black")
    );
    assert_eq!(
        package_root(Path::new("/usr/local/bin/tool")),
        PathBuf::from("/usr/local/bin")
    );
    // And a link on the PATH is followed to it.
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let keg = tempfile::tempdir().expect("a prefix");
    let program = real(keg.path()).join("Cellar/tool/1.0/bin/tool");
    std::fs::create_dir_all(program.parent().expect("a folder")).expect("a keg");
    std::fs::write(&program, "").expect("a program");
    std::os::unix::fs::symlink(&program, bin.path().join("tool")).expect("a link");
    let held = floor_held(&root, &machine_with(bin.path()), &[]);
    assert!(
        held.contains(&real(keg.path()).join("Cellar/tool")),
        "{held:?}"
    );
}

/// SO-10: at the floor a folder on the PATH is a block whose Allow lets the chat write it; at
/// the full sandbox it stays refused.
#[test]
fn a_path_folder_the_floor_holds_may_be_allowed() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let held = real(bin.path());
    // A harness's own temp root is never granted: a run inside one (a sandboxed developer
    // session) cannot stand a PATH folder anywhere else.
    if grant::HARNESS_TEMP.iter().any(|prefix| {
        held.display()
            .to_string()
            .contains(&format!("/tmp/{prefix}"))
    }) {
        let _ = writeln!(
            std::io::stderr(),
            "skipped: the temp folder here is a harness's own"
        );
        return;
    }
    let machine = machine_with(bin.path());
    let ground = grant::Ground::of(&root, &root, &machine);
    assert_eq!(
        grant::write(&held.display().to_string(), &ground.place()),
        Ok(held.clone())
    );
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("on");
    let ground = grant::Ground::of(&root, &root, &machine);
    assert!(grant::write(&held.display().to_string(), &ground.place()).is_err());
}

#[test]
fn the_floor_keeps_the_projects_certificate_checks() {
    let (_dir, root) = project();
    let machine = machine_with(Path::new("/opt/none"));
    assert!(
        Compiled::floor(&root, &machine, &grant::Grants::default())
            .widened
            .trust
    );
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox]\ncertificate-checks = false\n",
    )
    .expect("charter.toml");
    assert!(
        !Compiled::floor(&root, &machine, &grant::Grants::default())
            .widened
            .trust
    );
}

/// qw384's services on this Mac reach a chat at the floor as at the full sandbox: the one port
/// a person allowed, through the proxy and in the wrap's profile, and no other.
#[test]
fn a_service_on_this_mac_a_person_allowed_is_reached_at_the_floor() {
    let (_dir, root) = project();
    let machine = machine_with(Path::new("/opt/none"));
    let chat = grant::Grants {
        ports: vec![5432],
        ..grant::Grants::default()
    };
    let compiled = Compiled::floor(&root, &machine, &chat);
    assert_eq!(compiled.widened.ports, [5432]);
    assert!(
        compiled
            .reach
            .decide("127.0.0.1", 5432, &[443], &[])
            .carries()
    );
    assert!(
        !compiled
            .reach
            .decide("127.0.0.1", 6379, &[443], &[])
            .carries()
    );
    assert!(
        compiled
            .reach
            .decide("example.org", 443, &[443], &[])
            .carries()
    );
    let profile = wrapped_floor_of(Harness::Codex, &compiled, &[4040]);
    let allowed = profile
        .find(&localport::seatbelt_rule(5432))
        .expect("the port");
    let held = profile
        .find("(deny network-outbound (remote ip \"localhost:*\"))")
        .expect("the loopback held");
    assert!(held < allowed, "given back after the hold");
}

/// The floor profile of a wrapped harness compiled from `compiled`.
fn wrapped_floor_of(harness: Harness, compiled: &Compiled, ports: &[u16]) -> String {
    let compile = compiler(harness).expect("a compiler");
    match compile(compiled).expect("compiled") {
        Form::Codex(wrap) => codex::floor_profile(&wrap, ports, None),
        Form::Opencode(wrap) => opencode::floor_profile(&wrap, ports, None),
        Form::ClaudeCode(_) | Form::ClaudeWrapped(_) => unreachable!(),
    }
    .expect("a profile")
}

#[test]
fn the_floor_confines_a_chat_to_no_folder_so_one_with_none_of_its_own_still_starts() {
    let (_dir, root) = project();
    let applied = floor_of(decided(
        Harness::Codex,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    let words = || Words {
        program: "codex".to_owned(),
        command: Vec::new(),
        armed: Vec::new(),
        charters: Vec::new(),
    };
    // Past the folder checks, a line asks for the chat's proxy, which no test starts: that is
    // the refusal a chat that passed them meets here.
    let past_its_folder = |line: Result<Line, String>| {
        line.expect_err("no proxy here")
            .contains("egress proxy was not started")
    };
    // No folder of its own: read at the project's root, not refused for it.
    assert!(past_its_folder(applied.line(words(), &At::default())));
    // Nor one outside the project: the floor holds what it holds wherever the chat stands.
    let outside = tempfile::tempdir().expect("a folder outside");
    let outside = outside.path().canonicalize().expect("a real folder");
    let at = At {
        cwd: Some(&outside),
        ..At::default()
    };
    assert!(past_its_folder(applied.line(words(), &at)));
    // The full sandbox still refuses a chat with no folder of its own.
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("on");
    let full = floor_of(decided(
        Harness::Codex,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    let refused = full.line(words(), &At::default()).expect_err("refused");
    assert!(refused.contains("no folder of its own"), "{refused}");
}

/// **Another project's purlis files are held at the floor, and its vaults are not read**
/// (#2071): a chat in one project writes every folder, so the state folder and the manifests
/// of every project are held by name wherever they are, and no project's vaults are read.
#[cfg(target_os = "macos")]
#[test]
fn a_chat_at_the_floor_writes_no_other_projects_purlis_files_and_reads_no_vault() {
    let can_apply =
        crate::forklock::status(std::process::Command::new(backend::SANDBOX_EXEC).args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success());
    if !can_apply {
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this process is already sandboxed, so no second profile applies"
        );
        return;
    }
    let (_dir, root) = project();
    let (_sibling, other) = project();
    let kept = other.join(".purlis").join("vaults");
    std::fs::create_dir_all(&kept).expect("its vaults");
    std::fs::write(kept.join("ops.json"), "{}").expect("a vault");
    std::fs::write(other.join("notes.txt"), "x").expect("a note");
    let profile = wrapped_floor(
        Harness::Codex,
        &root,
        &machine_with(Path::new("/opt/none")),
        &[],
        None,
    );
    let tried = |script: &str| -> bool {
        crate::forklock::output(
            std::process::Command::new(backend::SANDBOX_EXEC)
                .args(["-p", &profile, "/bin/sh", "-c", script])
                .current_dir(&root),
        )
        .expect("it runs")
        .status
        .success()
    };
    let other = other.display();
    // The control: the other project itself is written and read.
    assert!(tried(&format!("echo y >> {other}/notes.txt")));
    assert!(tried(&format!("head -c 1 {other}/notes.txt")));
    let vault = format!("{other}/.purlis/vaults/ops.json");
    for refused in [
        // Its approvals: its next chat would run what nobody approved.
        format!("echo '{{}}' > {other}/.purlis/harness-profiles-launched.json"),
        format!("mkdir -p {other}/.charter && echo x > {other}/.charter/mcp-approved.json"),
        // Its manifests: taking its full sandbox away, or what its chats are handed.
        format!("echo x > {other}/purlis.toml"),
        format!("rm {other}/charter.toml"),
        format!("echo x > {other}/purlis.local.toml"),
        // However the name is spelled.
        format!("echo x > {other}/.PURLIS/x"),
        format!("echo x > {other}/Purlis.toml"),
        // Its vaults, written or read.
        format!("echo x > {vault}"),
        format!("head -c 1 {vault}"),
        format!("head -c 1 {other}/.Purlis/Vaults/ops.json"),
    ] {
        assert!(!tried(&refused), "{refused} was let through");
    }
}

#[test]
fn every_projects_purlis_files_are_held_by_name_and_no_projects_vault_is_read() {
    let (_dir, root) = project();
    let machine = machine_with(Path::new("/opt/none"));
    // The floor, for Codex and opencode: by name, anywhere, in either case.
    let profile = wrapped_floor(Harness::Codex, &root, &machine, &[], None);
    for rule in seatbelt::every_project_rules()
        .expect("rules")
        .into_iter()
        .chain(seatbelt::vaults_everywhere_rules().expect("rules"))
    {
        assert!(
            profile.contains(&rule),
            "{rule} is not in the floor's profile"
        );
    }
    assert!(
        profile.contains(r#"[pP][uU][rR][lL][iI][sS]"#),
        "a name in either case"
    );
    // The floor, for Claude Code: every project's state folder and manifests unwritten, and no
    // project's vaults read, by its commands and by its own tools.
    let settings = settings_of(&floor_of(decided(Harness::ClaudeCode, &root, &machine)));
    let fs = &settings.sandbox["filesystem"];
    let uncased =
        |list: Vec<String>| -> Vec<String> { list.iter().map(|it| claude::uncased(it)).collect() };
    let deny_write = uncased(strings(&fs["denyWrite"]));
    let deny_read = uncased(strings(&fs["denyRead"]));
    for state in crate::names::STATE_DIR.spellings() {
        assert!(deny_write.contains(&format!("/**/{state}/**")), "{state}");
        assert!(
            deny_read.contains(&format!("/**/{state}/vaults/**")),
            "{state}"
        );
        assert!(
            settings
                .deny
                .contains(&format!("Read(//**/{state}/vaults/**)")),
            "{state}"
        );
    }
    for name in MANIFESTS {
        assert!(deny_write.contains(&format!("/**/{name}")), "{name}");
    }
}

#[test]
fn no_projects_vault_is_read_at_the_full_sandbox_either() {
    let (_dir, root) = project();
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("on");
    let machine = machine_with(Path::new("/opt/none"));
    let compiled = match decided(Harness::Codex, &root, &machine) {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not sandboxed: {other:?}"),
    };
    let Form::Codex(wrap) = compiled.form() else {
        panic!("not Codex's form");
    };
    let profile = codex::profile(wrap, &root, Path::new("/tmp"), &[], None).expect("a profile");
    for rule in seatbelt::vaults_everywhere_rules().expect("rules") {
        assert!(profile.contains(&rule), "{rule} is not in the full profile");
    }
    // Nor does the full sandbox take on the floor's every-project write rules: it writes no
    // other project in the first place.
    for rule in seatbelt::every_project_rules().expect("rules") {
        assert!(!profile.contains(&rule), "{rule}");
    }
    let claude = match decided(Harness::ClaudeCode, &root, &machine) {
        Some(Decided::Sandboxed(applied)) => settings_of(&applied),
        other => panic!("not sandboxed: {other:?}"),
    };
    let deny_read: Vec<String> = strings(&claude.sandbox["filesystem"]["denyRead"])
        .iter()
        .map(|it| claude::uncased(it))
        .collect();
    for state in crate::names::STATE_DIR.spellings() {
        assert!(
            deny_read.contains(&format!("/**/{state}/vaults/**")),
            "{state}"
        );
    }
}

#[test]
fn the_persons_own_claude_code_settings_are_never_written_at_either_level() {
    // ADR 0067 as amended 2026-10-15: a sandboxed chat runs on the person's own Claude Code
    // settings and purlis's, so neither its commands nor its Edit tool write the person's.
    for manifest in [
        "schema = 1\n",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ] {
        let (_dir, root) = project();
        std::fs::write(root.join("charter.toml"), manifest).expect("charter.toml");
        let applied = floor_of(decided(
            Harness::ClaudeCode,
            &root,
            &machine_with(Path::new("/opt/none")),
        ));
        let level = applied.level();
        let settings = settings_of(&applied);
        let filesystem = &settings.sandbox["filesystem"];
        let allow_write = strings(&filesystem["allowWrite"]);
        let deny_write = strings(&filesystem["denyWrite"]);
        for file in ["settings.json", "settings.local.json"] {
            let path = format!("/home/op/.claude/{file}");
            // Its commands: not writable, by name or because nothing grants the home.
            let named = deny_write.contains(&path);
            let granted = allow_write
                .iter()
                .any(|allowed| allowed == "/" || path.starts_with(&format!("{allowed}/")));
            assert!(
                named || !granted,
                "{level:?}: {path} is written: {filesystem}"
            );
            // Its Edit tool: refused by name.
            let rule = format!("Edit(/{path})");
            assert!(
                settings.deny.contains(&rule),
                "{level:?}: no {rule} in {:?}",
                settings.deny
            );
        }
    }
}

/// #1040: at the floor on Linux, what a session starts or loads at login, in the person's
/// home, is held like macOS's launch agents; a file nothing names is written.
#[test]
fn at_the_floor_on_linux_nothing_a_session_starts_at_login_is_written() {
    let (_dir, root) = project();
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    let applied = floor_of(decided(Harness::Opencode, &root, &linux));
    let Form::Opencode(wrap) = applied.form() else {
        panic!("compiled for opencode: {:?}", applied.form());
    };
    let profile = opencode::floor_profile(wrap, &[4040], None).expect("a profile");
    let read = purlis_linux_sandbox::profile::Profile::parse(&profile).expect("read whole");
    let made = |path: &str, kind| {
        read.allows(&purlis_linux_sandbox::profile::Op::File {
            what: purlis_linux_sandbox::profile::FileOp::Create,
            path: Path::new(path),
            kind,
        })
    };
    use purlis_linux_sandbox::profile::Kind;
    for held in [
        "/home/op/.config/systemd/user/x.service",
        "/home/op/.config/systemd/user",
        "/home/op/.local/share/systemd/user/x.timer",
        "/home/op/.config/autostart/x.desktop",
        "/home/op/.config/environment.d/x.conf",
        "/home/op/.local/share/applications/x.desktop",
        "/home/op/.pam_environment",
        "/home/op/.xprofile",
        "/home/op/.config/Code/User/tasks.json",
    ] {
        assert!(!made(held, Kind::Regular), "{held}");
    }
    assert!(made("/home/op/notes.txt", Kind::Regular));
}

/// purlis's own plugin and git hooks, laid out outside the project: `(install, data, plugin,
/// git hooks)`, the two folders kept alive by the first two.
fn a_kit_outside() -> (tempfile::TempDir, tempfile::TempDir, PathBuf, PathBuf) {
    let install = tempfile::tempdir().expect("an install folder");
    let data = tempfile::tempdir().expect("a data folder");
    let plugin = real(install.path()).join("plugin");
    let git_hooks = real(data.path()).join("git-hooks");
    std::fs::create_dir_all(plugin.join("hooks")).expect("the plugin");
    std::fs::create_dir_all(&git_hooks).expect("the git hooks");
    (install, data, plugin, git_hooks)
}

/// What purlis arms a chat with, by where each lies.
fn armed<'a>(
    binary: Option<&'a Path>,
    plugin: Option<&'a Path>,
    git_hooks: Option<&'a Path>,
) -> ArmedWith<'a> {
    ArmedWith {
        binary,
        plugin,
        git_hooks,
        skill_sets: None,
    }
}

/// An app bundle outside the project, laid out as purlis's ships on a Mac, and its program.
fn an_app_bundle() -> (tempfile::TempDir, PathBuf) {
    let install = tempfile::tempdir().expect("an install folder");
    let contents = real(install.path()).join("purlis.app/Contents");
    std::fs::create_dir_all(contents.join("MacOS")).expect("the bundle");
    std::fs::create_dir_all(contents.join("Resources/plugin")).expect("its plugin");
    std::fs::write(contents.join("Info.plist"), "").expect("its Info.plist");
    let binary = contents.join("MacOS/purlis");
    std::fs::write(&binary, "").expect("its program");
    (install, binary)
}

#[test]
fn the_floor_holds_the_whole_app_bundle_or_the_package_purlis_runs_from() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let (install, binary) = an_app_bundle();
    let bundle = real(install.path()).join("purlis.app");
    assert_eq!(install_root(&binary), bundle);
    let mut applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    let plugin = bundle.join("Contents/Resources/plugin");
    applied
        .hold_kit(armed(Some(&binary), Some(&plugin), None), &root, &[])
        .expect("a bundle outside what the chat writes");
    assert!(
        applied.confines().held.contains(&bundle),
        "{:?}",
        applied.confines().held
    );
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&bundle.display().to_string()),
        "{deny_write:?}"
    );
    // A program that is not in a bundle: the folder it is in.
    let elsewhere = tempfile::tempdir().expect("an install folder");
    let program = real(elsewhere.path()).join("bin/purlis");
    std::fs::create_dir_all(program.parent().expect("a folder")).expect("its folder");
    std::fs::write(&program, "").expect("a program");
    assert_eq!(install_root(&program), real(elsewhere.path()).join("bin"));
}

#[test]
fn purlis_s_own_program_where_the_chat_writes_is_refused_at_either_level() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let in_project = root.join("target/debug/purlis");
    std::fs::create_dir_all(in_project.parent().expect("a folder")).expect("a build folder");
    std::fs::write(&in_project, "").expect("a program in the project");
    let mut floor = floor_of(decided(Harness::Codex, &root, &machine_with(bin.path())));
    assert_eq!(
        floor.hold_kit(armed(Some(&in_project), None, None), &root, &[]),
        Err(NotStarted::KitWritable(in_project.clone()))
    );
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
    )
    .expect("charter.toml");
    let (_app, binary) = an_app_bundle();
    let mut full = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    assert_eq!(full.level(), Level::Full);
    assert_eq!(
        full.hold_kit(armed(Some(&binary), None, None), &root, &[]),
        Err(NotStarted::KitWritable(binary.clone())),
        "a program in a temp folder"
    );
}

#[test]
fn what_a_chat_is_armed_with_is_held_inside_a_folder_a_person_let_it_write_too() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let (_install, data, plugin, git_hooks) = a_kit_outside();
    let mut applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    // A person let the chat write the whole data folder.
    applied.confines.writable.push(real(data.path()));
    applied
        .hold_kit(armed(None, Some(&plugin), Some(&git_hooks)), &root, &[])
        .expect("held");
    assert!(applied.confines().held.contains(&git_hooks));
    let deny_write = strings(&settings_of(&applied).sandbox["filesystem"]["denyWrite"]);
    assert!(
        deny_write.contains(&git_hooks.display().to_string()),
        "{deny_write:?}"
    );
}

#[test]
fn the_floor_holds_the_plugin_and_git_hooks_a_chat_is_armed_with() {
    use super::tests::writes;
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let (_install, _data, plugin, git_hooks) = a_kit_outside();
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let mut applied = floor_of(decided(harness, &root, &machine_with(bin.path())));
        applied
            .hold_kit(armed(None, Some(&plugin), Some(&git_hooks)), &root, &[])
            .expect("a kit outside what the chat writes");
        // Held for a run for the chat too.
        for folder in [&plugin, &git_hooks] {
            assert!(
                applied.confines().held.contains(folder),
                "{harness:?}: {}",
                folder.display()
            );
        }
        match applied.form() {
            Form::ClaudeCode(settings) => {
                let deny_write = strings(&settings.sandbox["filesystem"]["denyWrite"]);
                for folder in [&plugin, &git_hooks] {
                    assert!(
                        deny_write.contains(&folder.display().to_string()),
                        "{deny_write:?}"
                    );
                    assert!(
                        settings
                            .deny
                            .contains(&format!("Edit(/{}/**)", folder.display())),
                        "{:?}",
                        settings.deny
                    );
                }
            }
            form => {
                let profile = match form {
                    Form::Codex(wrap) => codex::floor_profile(wrap, &[], None),
                    Form::Opencode(wrap) => opencode::floor_profile(wrap, &[], None),
                    Form::ClaudeCode(_) | Form::ClaudeWrapped(_) => unreachable!(),
                }
                .expect("a profile");
                assert!(writes(&profile, &root.join("notes.txt")), "{harness:?}");
                for not in [
                    plugin.join("hooks/hooks.json"),
                    git_hooks.join("pre-commit"),
                ] {
                    assert!(
                        !writes(&profile, &not),
                        "{harness:?}: {} is writable",
                        not.display()
                    );
                }
            }
        }
    }
}

#[test]
fn a_plugin_where_the_chat_writes_is_refused_at_either_level() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let (_install, _data, plugin, git_hooks) = a_kit_outside();
    let in_project = root.join("tools/plugin");
    std::fs::create_dir_all(&in_project).expect("a plugin in the project");
    for harness in [Harness::ClaudeCode, Harness::Codex] {
        let mut floor = floor_of(decided(harness, &root, &machine_with(bin.path())));
        assert_eq!(
            floor.hold_kit(armed(None, Some(&in_project), Some(&git_hooks)), &root, &[]),
            Err(NotStarted::KitWritable(in_project.clone())),
            "{harness:?}"
        );
    }
    // The full sandbox lets a chat write the temp folders: a plugin there is refused too.
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
    )
    .expect("charter.toml");
    let mut full = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_with(bin.path()),
    ));
    assert_eq!(full.level(), Level::Full);
    let refused = full
        .hold_kit(armed(None, Some(&plugin), Some(&git_hooks)), &root, &[])
        .expect_err("a plugin in a temp folder");
    assert_eq!(refused, NotStarted::KitWritable(plugin.clone()));
    let said = refused.to_string();
    assert!(said.contains(&plugin.display().to_string()), "{said}");
    assert!(
        said.contains("Install purlis outside the project and outside what a chat may write"),
        "{said}"
    );
}

/// The plugin and git hooks a chat at the floor is armed with, held where this Mac applies the
/// profile for real; the project beside them is still written.
#[cfg(target_os = "macos")]
#[test]
fn a_chat_at_the_floor_on_a_mac_writes_neither_the_plugin_nor_the_git_hooks() {
    let can_apply =
        crate::forklock::status(std::process::Command::new(backend::SANDBOX_EXEC).args([
            "-p",
            "(version 1)(allow default)",
            "/usr/bin/true",
        ]))
        .is_ok_and(|status| status.success());
    if !can_apply {
        let _ = writeln!(
            std::io::stderr(),
            "skipped: this process is already sandboxed, so no second profile applies"
        );
        return;
    }
    let (_dir, root) = project();
    let (_install, _data, plugin, git_hooks) = a_kit_outside();
    let (_app, binary) = an_app_bundle();
    let mut applied = floor_of(decided(
        Harness::Codex,
        &root,
        &machine_with(Path::new("/opt/none")),
    ));
    applied
        .hold_kit(
            armed(Some(&binary), Some(&plugin), Some(&git_hooks)),
            &root,
            &[],
        )
        .expect("a kit outside what the chat writes");
    let bundle = install_root(&binary);
    let Form::Codex(wrap) = applied.form() else {
        panic!("not Codex's");
    };
    let profile = codex::floor_profile(wrap, &[], None).expect("a profile");
    let tried = |script: &str| -> bool {
        crate::forklock::output(
            std::process::Command::new(backend::SANDBOX_EXEC)
                .args(["-p", &profile, "/bin/sh", "-c", script])
                .current_dir(&root),
        )
        .expect("it runs")
        .status
        .success()
    };
    assert!(tried(&format!("echo x > {}/notes.txt", root.display())));
    for refused in [
        format!("echo x > {}/hooks/hooks.json", plugin.display()),
        format!("echo x > {}/pre-commit", git_hooks.display()),
        format!("mv {} {}.aside", plugin.display(), plugin.display()),
        format!("mv {} {}.aside", git_hooks.display(), git_hooks.display()),
        // A folder above one held is never moved aside with it.
        format!(
            "mv {0} {0}.aside",
            plugin.parent().expect("a parent").display()
        ),
        format!(
            "mv {0} {0}.aside",
            git_hooks.parent().expect("a parent").display()
        ),
        // purlis's own program, and everything else its bundle ships.
        format!("echo x > {}", binary.display()),
        format!("echo x > {}/Contents/Info.plist", bundle.display()),
        format!("mv {0} {0}.aside", bundle.display()),
    ] {
        assert!(!tried(&refused), "{refused} was let through");
    }
}

/// #2428: no profile purlis compiles for a chat it wraps on Linux reaches the system's name
/// lookup through its local sockets (systemd-resolved's, nscd's, sssd's, avahi's), at the floor
/// or at the full sandbox: a chat's names are looked up by purlis's proxy.
#[test]
fn on_linux_no_compiled_profile_reaches_the_system_s_name_lookup() {
    use purlis_linux_sandbox::profile::{Op, Profile, Remote};
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    for harness in [Harness::Codex, Harness::Opencode] {
        let (_floor_dir, floor_root) = project();
        let floor = floor_of(decided(harness, &floor_root, &linux));
        let (_full_dir, full_root) = project();
        std::fs::write(
            full_root.join("charter.toml"),
            "schema = 1\n[sandbox]\nmode = \"on\"\n",
        )
        .expect("charter.toml");
        let full = floor_of(decided(harness, &full_root, &linux));
        let tmp = full_root.join(".tmp-chat");
        std::fs::create_dir_all(&tmp).expect("a temp folder");
        let at_the_floor = match floor.form() {
            Form::Codex(wrap) => codex::floor_profile(wrap, &[4040], None),
            Form::Opencode(wrap) => opencode::floor_profile(wrap, &[4040], None),
            other => panic!("compiled for {harness:?}: {other:?}"),
        }
        .expect("the floor's profile");
        let at_full = match full.form() {
            Form::Codex(wrap) => codex::profile(wrap, &full_root, &tmp, &[4040], None),
            Form::Opencode(wrap) => opencode::profile(wrap, &full_root, &tmp, &[4040], None),
            other => panic!("compiled for {harness:?}: {other:?}"),
        }
        .expect("the full profile");
        for profile in [at_the_floor, at_full] {
            let read = Profile::parse(&profile).expect("read whole");
            for socket in crate::sandboxblock::RESOLVER_SOCKETS {
                assert!(
                    !read.allows(&Op::Connect(Remote::Unix(Some(Path::new(socket))))),
                    "{harness:?}: {socket}"
                );
            }
        }
    }
}

#[test]
fn no_chat_writes_a_workspaces_manifest_at_the_floor_or_in_the_full_sandbox() {
    let (_dir, root) = project();
    for workspace in ["alpha", "Beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(workspace)).expect("a workspace");
    }
    let machine = machine_with(Path::new("/opt/none"));

    // The floor, for Codex and opencode: `…/workspaces/<any>/workspace.json`, in any case.
    let profile = wrapped_floor(Harness::Codex, &root, &machine, &[], None);
    let rule = format!(
        "(deny file-write* file-link (regex {}))",
        seatbelt::string(&seatbelt::workspace_manifests_regex()).expect("a string")
    );
    assert!(
        profile.contains(&rule),
        "{rule} is not in the floor's profile"
    );
    let pattern = regex::Regex::new(&seatbelt::workspace_manifests_regex()).expect("a regex");
    for held in [
        "/p/workspaces/alpha/workspace.json",
        "/p/Workspaces/alpha/Workspace.JSON",
        "/elsewhere/workspaces/new/workspace.json",
    ] {
        assert!(pattern.is_match(held), "{held}");
    }
    for free in [
        "/p/workspaces/alpha/web/workspace.json",
        "/p/workspaces/workspace.json",
        "/p/workspaces/alpha/workspace.json.bak",
    ] {
        assert!(!pattern.is_match(free), "{free}");
    }

    // The floor, for Claude Code: its commands and its own tools.
    let settings = settings_of(&floor_of(decided(Harness::ClaudeCode, &root, &machine)));
    let deny_write: Vec<String> = strings(&settings.sandbox["filesystem"]["denyWrite"])
        .iter()
        .map(|it| claude::uncased(it))
        .collect();
    assert!(
        deny_write.contains(&"/**/workspaces/*/workspace.json".to_owned()),
        "{deny_write:?}"
    );

    // The full sandbox: each of the project's workspaces' by its path, in any case.
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("on");
    let compiled = match decided(Harness::Codex, &root, &machine) {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not sandboxed: {other:?}"),
    };
    for workspace in ["alpha", "Beta"] {
        let path = root
            .join("workspaces")
            .join(workspace)
            .join("workspace.json");
        assert!(
            compiled.denied.iter().any(|denial| denial.path == path
                && denial.any_case
                && denial.access == Access::Write),
            "{} is not held",
            path.display()
        );
    }
}

#[test]
fn no_planes_workspace_folder_is_moved_aside_and_any_other_workspaces_folder_is_written() {
    let (_dir, root) = project();
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("a workspace");
    let machine = machine_with(Path::new("/opt/none"));
    let rules = seatbelt::workspace_folder_rules(std::slice::from_ref(&root)).expect("rules");
    assert!(
        rules
            .iter()
            .all(|rule| rule.contains("(vnode-type DIRECTORY)")),
        "{rules:?}"
    );

    // The plane's own, at the floor and in the full sandbox, as a chat is given them.
    let at_the_floor = floor_of(decided(Harness::Codex, &root, &machine));
    let Form::Codex(floor_wrap) = at_the_floor.form() else {
        panic!("not Codex's form");
    };
    let floor = codex::floor_profile(floor_wrap, &[], None).expect("a profile");
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("on");
    let compiled = match decided(Harness::Codex, &root, &machine) {
        Some(Decided::Sandboxed(applied)) => applied,
        other => panic!("not sandboxed: {other:?}"),
    };
    let Form::Codex(wrap) = compiled.form() else {
        panic!("not Codex's form");
    };
    let full = codex::profile(wrap, &root, Path::new("/tmp"), &[], None).expect("a profile");
    for rule in &rules {
        assert!(floor.contains(rule), "{rule} is not in the floor's profile");
        assert!(full.contains(rule), "{rule} is not in the full profile");
    }

    // `mv workspaces/late /p/aside`, write `/p/aside/workspace.json`, `mv /p/aside
    // workspaces/late`: the first move names a folder entry the rule holds, so it is refused.
    let held: Vec<regex::Regex> = seatbelt::workspace_folders_regexes(&root)
        .iter()
        .map(|one| regex::Regex::new(one).expect("a regex"))
        .collect();
    let matched = |path: &str| held.iter().any(|one| one.is_match(path));
    let p = root.display().to_string();
    for entry in [
        format!("{p}/workspaces/late"),
        format!("{p}/workspaces"),
        format!("{p}/Workspaces/Late"),
    ] {
        assert!(matched(&entry), "{entry} is not held");
    }
    // What is in a workspace, a repository's own `workspaces` folder, and a test's plane in a
    // temporary folder are written as ever.
    let tmp = std::env::temp_dir().display().to_string();
    for free in [
        format!("{p}/workspaces/alpha/web"),
        format!("{p}/workspaces/alpha/web/src/workspaces"),
        format!("{p}/workspaces/alpha/web/src/workspaces/rename.rs"),
        "/Users/dev/repo/app/src-tauri/src/workspaces".to_owned(),
        format!("{tmp}/x/workspaces/alpha"),
        format!("{p}/aside"),
    ] {
        assert!(!matched(&free), "{free} is held");
    }
    // No rule holds a `workspaces` folder by its name alone.
    for profile in [&floor, &full] {
        assert!(
            !profile.contains(
                &seatbelt::string("^/(.*/)?[wW][oO][rR][kK][sS][pP][aA][cC][eE][sS](/[^/]+)?$")
                    .expect("a string")
            ),
            "a folder called workspaces anywhere is held"
        );
    }
}

/// #2019: a wrapped chat at the floor serves on its own socket, and reaches its door, as at the
/// full level: the rules that let it are written after the floor's denials, which hold the
/// folder the sockets are in and every port of this machine.
#[test]
fn a_wrapped_chat_at_the_floor_serves_on_its_own_socket_and_no_other() {
    let (_dir, root) = project();
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    let port = 5173;
    let mut chat = grant::Grants::default();
    chat.add(&grant::What::Listen(port));
    let compiled = Compiled::floor(&root, &linux, &chat);
    let folder = super::door::root_in(None).join("0123456789abcdef");
    let socket = localport::serve_socket(&folder, port);
    let tmp = root.join(".tmp-chat");
    let ports = [port];
    let served = Some((folder.as_path(), ports.as_slice()));
    for harness in [Harness::Codex, Harness::Opencode] {
        let compile = compiler(harness).expect("a compiler");
        let profile = match compile(&compiled).expect("compiled") {
            Form::Codex(wrap) => codex::profile_serving(&wrap, &root, &tmp, &[4040], None, served),
            Form::Opencode(wrap) => {
                opencode::profile_serving(&wrap, &root, &tmp, &[4040], None, served)
            }
            Form::ClaudeCode(_) | Form::ClaudeWrapped(_) => unreachable!(),
        }
        .expect("a profile");
        assert!(
            profile.contains(&localport::seatbelt_rule(port)),
            "{harness:?}: no way to its door"
        );
        let read = purlis_linux_sandbox::profile::Profile::parse(&profile).expect("read whole");
        assert!(read.serves_at(&socket), "{harness:?}");
        assert!(
            !read.serves_at(Path::new("/run/user/1000/bus")),
            "{harness:?}"
        );
    }
}

/// The same machine as [`machine_with`], on `os`, with its home at `home`.
fn machine_on(os: Os, home: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[("PATH", "/opt/none")]),
        home: Some(home.to_path_buf()),
        os,
    }
}

/// On Linux, Claude Code's sandbox puts a denied path that does not exist out of reach of a read
/// too, and git stops on a home config it cannot read. So the home's git config is made, empty,
/// before a Claude Code chat starts at the floor there: held as an existing file is, and read
/// as none.
#[test]
fn on_linux_the_floor_makes_the_homes_git_config_before_a_claude_code_chat_starts() {
    let (_dir, root) = project();
    let home = Path::new("/home/op");
    let on_linux = settings_of(&floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_on(Os::Linux, home),
    )));
    assert_eq!(on_linux.made_empty, vec![home.join(".gitconfig")]);
    // Still held: made, it is denied as before.
    assert!(
        strings(&on_linux.sandbox["filesystem"]["denyWrite"])
            .contains(&"/home/op/.gitconfig".to_owned()),
        "{}",
        on_linux.sandbox
    );
    // macOS's sandbox reads a denied path that does not exist as none.
    let on_macos = settings_of(&floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_on(Os::MacOs, home),
    )));
    assert!(on_macos.made_empty.is_empty(), "{:?}", on_macos.made_empty);
}

/// The full sandbox writes no rule on the home's git config, so it needs none made.
#[test]
fn the_full_sandbox_makes_nothing_in_the_home() {
    let (_dir, root) = project();
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("charter.toml");
    let applied = floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_on(Os::Linux, Path::new("/home/op")),
    ));
    assert_eq!(applied.level(), Level::Full);
    assert!(settings_of(&applied).made_empty.is_empty());
}

/// What is made is an empty file where there is nothing: a file already there keeps what it
/// holds, and a link, even to nothing, is never followed.
#[test]
fn the_homes_git_config_is_made_empty_only_where_there_is_none() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a home");
    let home = home.path().canonicalize().expect("a real folder");
    let settings = settings_of(&floor_of(decided(
        Harness::ClaudeCode,
        &root,
        &machine_on(Os::Linux, &home),
    )));
    let config = home.join(".gitconfig");

    settings.make_empty();
    assert_eq!(std::fs::read(&config).expect("made"), b"");

    std::fs::write(&config, "[user]\n\tname = op\n").expect("the person's own");
    settings.make_empty();
    assert_eq!(
        std::fs::read_to_string(&config).expect("kept"),
        "[user]\n\tname = op\n"
    );

    #[cfg(unix)]
    {
        std::fs::remove_file(&config).expect("removed");
        let target = home.join("dotfiles/gitconfig");
        std::os::unix::fs::symlink(&target, &config).expect("a link to nothing");
        settings.make_empty();
        assert!(!target.exists(), "the link was followed");
        assert!(
            std::fs::symlink_metadata(&config)
                .expect("still there")
                .file_type()
                .is_symlink()
        );
    }
}

/// What a forge program or kubectl loads when the person runs it is a later-code name, read as
/// usual.
#[test]
fn a_forge_programs_and_kubectls_settings_are_held_from_writing() {
    for held in [
        ".config/gh",
        ".config/glab-cli",
        ".local/share/gh",
        ".kube/kuberc",
    ] {
        assert!(
            PLANTED
                .iter()
                .any(|planted| planted.path == held && planted.reach == Reach::AndBelow),
            "{held} is not held"
        );
        assert!(crate::sandboxblock::is_planted(std::path::Path::new(
            &format!("{held}/some/file")
        )));
    }
}

/// What the floor makes is a later-code name it holds, the home's among them.
#[test]
fn the_homes_git_config_made_is_one_the_floor_holds() {
    assert!(
        PLANTED
            .iter()
            .any(|planted| planted.path == claude::HOME_GIT_CONFIG
                && planted.reach == Reach::AndBelow),
        "{} is not held",
        claude::HOME_GIT_CONFIG
    );
}

/// A machine whose home is `home`, with `env` beside an empty `PATH`.
fn homed(home: &Path, env: &[(&str, &str)]) -> Machine {
    Machine {
        env: crate::secrets::Env::of(env),
        home: Some(home.to_path_buf()),
        os: Os::MacOs,
    }
}

#[test]
fn the_floor_lists_the_homes_shared_folders_each_before_what_is_in_it() {
    let home = Path::new("/home/op");
    assert_eq!(
        usual_home_folders(Level::Floor, &homed(home, &[])),
        [
            home.join(".local"),
            home.join(".local/share"),
            home.join(".config"),
        ]
    );
    assert_eq!(
        usual_home_folders(Level::Full, &homed(home, &[])),
        Vec::<PathBuf>::new(),
        "the full sandbox keeps a chat out of the home anyway"
    );
}

#[test]
fn an_xdg_folder_is_listed_only_where_it_is_in_the_home() {
    let home = Path::new("/home/op");
    let moved = homed(
        home,
        &[
            ("XDG_DATA_HOME", "/home/op/my/data"),
            ("XDG_CONFIG_HOME", "/elsewhere/config"),
        ],
    );
    assert_eq!(
        usual_home_folders(Level::Floor, &moved),
        [
            home.join(".local"),
            home.join(".local/share"),
            home.join(".config"),
            home.join("my"),
            home.join("my/data"),
        ]
    );
}

#[cfg(unix)]
#[test]
fn folders_are_made_private_and_what_is_there_is_left_as_it_is() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("a folder");
    let home = dir.path().canonicalize().expect("a real folder");
    std::fs::create_dir(home.join(".config")).expect("a folder there already");
    std::fs::set_permissions(home.join(".config"), std::fs::Permissions::from_mode(0o755))
        .expect("its mode");
    std::fs::write(home.join(".local"), "a file holds the name").expect("a file");
    let folders = usual_home_folders(Level::Floor, &homed(&home, &[]));
    make_folders(&folders);
    let mode = |at: &Path| std::fs::metadata(at).expect("there").permissions().mode() & 0o777;
    assert_eq!(mode(&home.join(".config")), 0o755, "left as it was");
    assert_eq!(
        std::fs::read_to_string(home.join(".local")).expect("still a file"),
        "a file holds the name"
    );

    std::fs::remove_file(home.join(".local")).expect("the file taken away");
    make_folders(&folders);
    assert!(home.join(".local/share").is_dir());
    assert_eq!(mode(&home.join(".local")), 0o700);
    assert_eq!(mode(&home.join(".local/share")), 0o700);

    let gone = home.join("no-such-home");
    make_folders(&usual_home_folders(Level::Floor, &homed(&gone, &[])));
    assert!(!gone.exists(), "a home that is not there is not made");
}

#[test]
fn a_chat_starting_at_the_floor_finds_the_homes_shared_folders_made() {
    let (_project, root) = project();
    let dir = tempfile::tempdir().expect("a home");
    let home = dir.path().canonicalize().expect("a real folder");
    let machine = homed(&home, &[("PATH", "/opt/none")]);
    let applied = floor_of(decided(Harness::Codex, &root, &machine));
    assert!(!home.join(".local").exists(), "deciding makes nothing");
    // Whether the line is then given depends on the chat's proxy, which no test starts.
    let _ = applied.line(
        Words {
            program: "codex".to_owned(),
            command: Vec::new(),
            armed: Vec::new(),
            charters: Vec::new(),
        },
        &At::default(),
    );
    assert!(home.join(".local/share").is_dir());
    assert!(home.join(".config").is_dir());
}

#[test]
fn an_xdg_folder_that_climbs_out_of_the_home_is_not_listed() {
    let home = Path::new("/home/op");
    let climbing = homed(home, &[("XDG_DATA_HOME", "/home/op/../other/data")]);
    assert_eq!(
        usual_home_folders(Level::Floor, &climbing),
        [
            home.join(".local"),
            home.join(".local/share"),
            home.join(".config"),
        ]
    );
}

#[cfg(unix)]
#[test]
fn nothing_is_made_through_a_link_at_a_shared_folders_name() {
    let dir = tempfile::tempdir().expect("a home");
    let home = dir.path().canonicalize().expect("a real folder");
    let elsewhere = home.join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("a folder");
    std::os::unix::fs::symlink(&elsewhere, home.join(".local")).expect("a link");
    make_folders(&usual_home_folders(Level::Floor, &homed(&home, &[])));
    assert!(!elsewhere.join("share").exists(), "made through the link");
    assert!(home.join(".config").is_dir(), "the others are still made");
}

/// A link above a held path is held by its own name: replaced by one pointing elsewhere, the path
/// would no longer lead to what is denied.
#[cfg(unix)]
#[test]
fn a_link_on_the_way_to_a_held_path_is_held_by_its_own_name() {
    let dir = tempfile::tempdir().expect("a home");
    let home = dir.path().canonicalize().expect("a real folder");
    std::fs::create_dir_all(home.join("real/inner")).expect("a folder");
    std::fs::create_dir(home.join("second")).expect("a folder");
    std::os::unix::fs::symlink(home.join("real"), home.join("second/hop")).expect("a link");
    std::os::unix::fs::symlink(home.join("second/hop"), home.join(".local")).expect("a link");
    let held = home.join(".local/inner/auth.json");
    let denied = [crate::sandbox::Denial {
        class: Class::Integrity,
        path: held.clone(),
        access: Access::Write,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    }];
    let profile = seatbelt::floor_profile(&denied, &[], &seatbelt::Own::default(), &[], None)
        .expect("a profile");
    // The folder holding a link too: moved whole, another of that name could take its place.
    for link in [
        home.join(".local"),
        home.join("second/hop"),
        home.join("second"),
    ] {
        assert!(
            profile.contains(&format!("(literal \"{}\")", link.display())),
            "{} is not held: {profile}",
            link.display()
        );
    }
    // What goes through the link is still the chat's to write.
    assert!(!profile.contains(&format!("(subpath \"{}\")", home.join(".local").display())));
}

/// The same, applied for real.
#[cfg(target_os = "macos")]
#[test]
fn a_link_on_the_way_to_a_held_path_cannot_be_moved_away_at_the_floor() {
    let dir = tempfile::tempdir().expect("a home");
    let home = dir.path().canonicalize().expect("a real folder");
    std::fs::create_dir_all(home.join("real/inner")).expect("a folder");
    std::fs::create_dir(home.join("second")).expect("a folder");
    std::os::unix::fs::symlink(home.join("real"), home.join("second/hop")).expect("a link");
    let denied = [crate::sandbox::Denial {
        class: Class::Integrity,
        path: home.join("second/hop/inner/auth.json"),
        access: Access::Write,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    }];
    let profile = seatbelt::floor_profile(&denied, &[], &seatbelt::Own::default(), &[], None)
        .expect("a profile");
    let script = format!(
        "/bin/mv '{from}/hop' '{from}/x'; a=$?; /bin/mv '{from}' '{to}'; b=$?; \
         echo through > '{from}/hop/through'; c=$?; [ $a -ne 0 ] && [ $b -ne 0 ] && [ $c -eq 0 ]",
        from = home.join("second").display(),
        to = home.join("moved").display(),
    );
    let ran = crate::forklock::output(
        std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", &profile, "/bin/sh", "-c", &script]),
    )
    .expect("sandbox-exec runs");
    assert!(
        ran.status.success(),
        "the link and its folder stay, and what goes through it is written: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(home.join("real/through").exists());
}
