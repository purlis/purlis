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
}

#[test]
fn the_floor_still_denies_the_vaults_and_what_a_later_program_runs() {
    let (_dir, root) = project();
    let bin = tempfile::tempdir().expect("a bin");
    let machine = machine_with(bin.path());
    let applied = floor_of(decided(Harness::ClaudeCode, &root, &machine));
    let settings = settings_of(&applied);
    let deny_read = strings(&settings.sandbox["filesystem"]["denyRead"]);
    let deny_write = strings(&settings.sandbox["filesystem"]["denyWrite"]);
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
    // Codex is wrapped on macOS only (#1040).
    let linux = Machine {
        os: Os::Linux,
        ..machine_with(Path::new("/opt/none"))
    };
    match decide(Harness::Codex, &root, &linux, &|_| true, None, None) {
        Ok(Some(Decided::Floorless(why))) => assert!(why.contains("no sandbox"), "{why}"),
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
        Form::ClaudeCode(_) => unreachable!(),
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
    let _ = crate::forklock::status(
        std::process::Command::new("/bin/launchctl").args(["remove", "purlis.floor.test"]),
    );
    // A later-code name spelled another way is refused too.
    let home = real(sockets.path()).join("home");
    assert_eq!(
        run(&tried(&format!(
            "mkdir -p {0}/library/launchagents && touch {0}/library/launchagents/x.plist",
            home.display()
        ))),
        "no"
    );
    // The hook socket and the proxy's port, and no other socket or port of this machine.
    let nc = |target: &str| tried(&format!("/usr/bin/nc -z -w 2 {target}"));
    assert_eq!(
        run(&tried(&format!("/usr/bin/nc -U -z {}", hook.display()))),
        "yes"
    );
    assert_eq!(
        run(&tried(&format!("/usr/bin/nc -U -z {}", other.display()))),
        "no"
    );
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
    assert!(compiled.reach.decide("127.0.0.1", 5432, &[443], &[]).carries());
    assert!(!compiled.reach.decide("127.0.0.1", 6379, &[443], &[]).carries());
    assert!(compiled.reach.decide("example.org", 443, &[443], &[]).carries());
    let profile = wrapped_floor_of(Harness::Codex, &compiled, &[4040]);
    let allowed = profile.find(&localport::seatbelt_rule(5432)).expect("the port");
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
        Form::ClaudeCode(_) => unreachable!(),
    }
    .expect("a profile")
}
