// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The folder every run's forge route is made in is denied to every chat, with a route or
//! without, and a chat with one is given back its own socket and no other path in it (ADR 0128
//! D3a). Read off what each harness is handed: Claude Code's settings and each wrap's profile.

use std::path::{Path, PathBuf};

use super::tests::writes;
use super::*;

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// A started sandbox for `harness` on `os`, where the app made the route folder `root`: found
/// from `$TMPDIR` on macOS and `$XDG_RUNTIME_DIR` on Linux, as the app finds it.
struct Started {
    _plane: tempfile::TempDir,
    _home: tempfile::TempDir,
    _runtime: tempfile::TempDir,
    root: PathBuf,
    cwd: PathBuf,
    applied: Applied,
}

fn started(harness: Harness, os: Os) -> Started {
    started_as(harness, os, None)
}

/// [`started`], for a chat running as `persona`.
fn started_as(harness: Harness, os: Os, persona: Option<&str>) -> Started {
    let plane = tempfile::tempdir().expect("a plane");
    // A persona's chat is guarded only where the sandbox holds the person's credentials back,
    // which the full sandbox does and the plain floor does not.
    let manifest = if persona.is_some() {
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n"
    } else {
        "schema = 1\n"
    };
    std::fs::write(plane.path().join("charter.toml"), manifest).expect("charter.toml");
    let project = real(plane.path());
    let cwd = project.join("w");
    std::fs::create_dir_all(&cwd).expect("a chat's folder");
    let home = tempfile::tempdir().expect("a home");
    let runtime = tempfile::tempdir().expect("a runtime folder");
    let runtime_path = real(runtime.path());
    let var = if os == Os::Linux {
        "XDG_RUNTIME_DIR"
    } else {
        "TMPDIR"
    };
    let machine = Machine {
        env: crate::secrets::Env::of(&[(var, runtime_path.to_str().expect("text"))]),
        home: Some(real(home.path())),
        os,
    };
    let root = crate::forgewire::route_root(&machine).expect("a route folder");
    assert!(root.starts_with(&runtime_path), "{}", root.display());
    std::fs::create_dir_all(&root).expect("the app makes the route folder");
    let applied = for_start_as(harness, &project, &machine, &|_| true, persona)
        .expect("starts")
        .expect("sandboxed");
    Started {
        _plane: plane,
        _home: home,
        _runtime: runtime,
        root,
        cwd,
        applied,
    }
}

/// A run's folder in `root` with its socket and CA, as the app makes it.
fn run_in(root: &Path, name: &str) -> (PathBuf, PathBuf) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("a run's folder");
    (dir.join("proxy.sock"), dir.join("ca.pem"))
}

fn inputs(root: &Path, socket: PathBuf, ca_cert: PathBuf) -> crate::forgewire::Inputs {
    crate::forgewire::Inputs {
        gh_socket: socket,
        ca_cert,
        sentinel: crate::forgewire::SecretString::from("SENTINEL-0123456789abcdef"),
        helper_exe: PathBuf::from("/opt/purlis/purlis"),
        repos: vec![
            crate::forgewire::Repo::new(
                "github.com",
                "acme",
                "app",
                crate::forgewire::ForgeKind::GitHub,
            )
            .expect("a repo"),
        ],
        door: "127.0.0.1:43001".parse().expect("an address"),
        handoff: crate::sandbox::egress::handoff::Secret::random().expect("a secret"),
        hosts: vec!["github.com".to_owned()],
        root: root.to_path_buf(),
    }
}

/// A route for the chat's run over repos on `forges`, and the hosts they are on.
fn inputs_over(
    it: &Started,
    forges: &[(&str, crate::forgewire::ForgeKind)],
) -> crate::forgewire::Inputs {
    let (socket, ca) = run_in(&it.root, "100-aaaaaaaa");
    let mut route = inputs(&it.root, socket, ca);
    route.repos = forges
        .iter()
        .map(|(host, kind)| {
            crate::forgewire::Repo::new(host, "acme", "app", *kind).expect("a repo")
        })
        .collect();
    route.hosts = forges.iter().map(|(host, _)| (*host).to_owned()).collect();
    route
}

/// The programs the chat's persona has a guard shim for.
fn guarded(applied: &Applied) -> Vec<String> {
    applied
        .tools
        .as_ref()
        .map(|resolved| {
            resolved
                .guards
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn profile_of(applied: &Applied, cwd: &Path) -> String {
    let tmp = Path::new("/private/var/folders/xx/chat-tmp");
    match applied.form_in(Some(cwd)) {
        Form::Opencode(wrap) if applied.level() == Level::Floor => {
            opencode::floor_profile(&wrap, &[4040], None)
        }
        Form::Codex(wrap) if applied.level() == Level::Floor => {
            codex::floor_profile(&wrap, &[4040], None)
        }
        Form::Opencode(wrap) => opencode::profile(&wrap, cwd, tmp, &[4040], None),
        Form::Codex(wrap) => codex::profile(&wrap, cwd, tmp, &[4040], None),
        other => panic!("not a wrap: {other:?}"),
    }
    .expect("a profile")
}

fn list<'a>(settings: &'a claude::Settings, section: &str, key: &str) -> Vec<&'a str> {
    settings.sandbox[section][key]
        .as_array()
        .map(|list| list.iter().filter_map(|it| it.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn a_wrapped_chat_without_a_route_reaches_nothing_in_the_route_folder() {
    for harness in [Harness::Codex, Harness::Opencode] {
        let it = started(harness, Os::MacOs);
        let (socket, ca) = run_in(&it.root, "100-aaaaaaaa");
        let profile = profile_of(&it.applied, &it.cwd);
        for path in [&socket, &ca, &it.root.join("quarantine/x")] {
            assert!(
                !writes(&profile, path),
                "{harness:?}: {} is reached",
                path.display()
            );
        }
        assert!(
            !profile.contains(&socket.display().to_string()),
            "{harness:?}: a socket is named without a route"
        );
    }
}

#[test]
fn a_wrapped_chat_with_a_route_reaches_its_own_socket_and_no_other_path_in_the_folder() {
    for harness in [Harness::Codex, Harness::Opencode] {
        let it = started(harness, Os::MacOs);
        let (own, own_ca) = run_in(&it.root, "100-aaaaaaaa");
        let (other, other_ca) = run_in(&it.root, "100-bbbbbbbb");
        let applied =
            it.applied
                .clone()
                .with_forge_route(inputs(&it.root, own.clone(), own_ca.clone()));
        let profile = profile_of(&applied, &it.cwd);
        assert!(
            writes(&profile, &own),
            "{harness:?}: its own socket is held"
        );
        for not in [&other, &own_ca, &other_ca] {
            assert!(
                !writes(&profile, not),
                "{harness:?}: {} is reached",
                not.display()
            );
        }
        let network = |socket: &Path| {
            format!(
                "(allow network-outbound (remote unix-socket (path-literal \"{}\")))",
                socket.display()
            )
        };
        assert!(
            profile.contains(&network(&own)),
            "{harness:?}: its own socket is not connectable"
        );
        assert!(
            !profile.contains(&other.display().to_string()),
            "{harness:?}: another run's socket is named"
        );
    }
}

#[test]
fn claude_code_on_macos_is_denied_the_route_folder_with_a_route_or_without() {
    let it = started(Harness::ClaudeCode, Os::MacOs);
    let root = it.root.display().to_string();
    let (own, own_ca) = run_in(&it.root, "100-aaaaaaaa");
    let (other, _) = run_in(&it.root, "100-bbbbbbbb");
    let own_name = own.display().to_string();
    let other_name = other.display().to_string();

    let Form::ClaudeCode(bare) = it.applied.form_in(Some(&it.cwd)) else {
        panic!("Claude Code's own sandbox");
    };
    for key in ["denyRead", "denyWrite"] {
        assert!(
            list(&bare, "filesystem", key).contains(&root.as_str()),
            "{key} lacks the route folder"
        );
    }
    for (section, key) in [("network", "allowUnixSockets"), ("filesystem", "allowRead")] {
        assert!(
            !list(&bare, section, key)
                .iter()
                .any(|p| p.starts_with(&root)),
            "{key} names a path of the route folder without a route"
        );
    }

    let wired = it
        .applied
        .clone()
        .with_forge_route(inputs(&it.root, own.clone(), own_ca.clone()));
    let Form::ClaudeCode(settings) = wired.form_in(Some(&it.cwd)) else {
        panic!("Claude Code's own sandbox");
    };
    for key in ["denyRead", "denyWrite"] {
        assert!(
            list(&settings, "filesystem", key).contains(&root.as_str()),
            "{key} lost the route folder"
        );
    }
    for (section, key) in [("network", "allowUnixSockets"), ("filesystem", "allowRead")] {
        let named = list(&settings, section, key);
        assert!(named.contains(&own_name.as_str()), "{key}: {named:?}");
        assert!(
            !named.contains(&other_name.as_str()) && !named.iter().any(|p| p.contains("ca.pem")),
            "{key} names more than its own socket: {named:?}"
        );
    }
    assert_eq!(
        settings.sandbox["network"]["allowAllUnixSockets"],
        serde_json::json!(false),
        "every socket is opened"
    );
}

#[test]
fn claude_code_on_linux_is_wrapped_and_holds_the_route_folder_but_for_its_own_socket() {
    let tmp = Path::new("/tmp/chat-tmp");
    for routed in [false, true] {
        let it = started(Harness::ClaudeCode, Os::Linux);
        let (own, own_ca) = run_in(&it.root, "100-aaaaaaaa");
        let (other, other_ca) = run_in(&it.root, "100-bbbbbbbb");
        let applied = if routed {
            it.applied
                .clone()
                .with_forge_route(inputs(&it.root, own.clone(), own_ca.clone()))
        } else {
            it.applied.clone()
        };
        let Form::ClaudeWrapped(wrap) = applied.form_in(Some(&it.cwd)) else {
            panic!("Claude Code on Linux is wrapped");
        };
        let profile = claude_wrap::profile(&wrap, &it.cwd, tmp, &[4040], None).expect("a profile");
        assert_eq!(
            writes(&profile, &own),
            routed,
            "routed {routed}: own socket"
        );
        for not in [&other, &own_ca, &other_ca] {
            assert!(!writes(&profile, not), "routed {routed}: {}", not.display());
        }
    }
}

#[test]
fn the_route_folder_is_denied_whether_or_not_the_app_has_made_it_yet() {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("charter.toml");
    let runtime = tempfile::tempdir().expect("a folder");
    let machine = Machine {
        env: crate::secrets::Env::of(&[("TMPDIR", real(runtime.path()).to_str().expect("text"))]),
        home: None,
        os: Os::MacOs,
    };
    let root = crate::forgewire::route_root(&machine).expect("a route folder");
    let denied = Denied::of(&real(plane.path()), &machine);
    // Held before the app has made it: a chat that starts first must not find it open once it is.
    assert!(denied.paths.iter().any(|it| it.path == root));
    std::fs::create_dir_all(&root).expect("made");
    let denied = Denied::of(&real(plane.path()), &machine);
    let held: Vec<_> = denied.paths.iter().filter(|it| it.path == root).collect();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].access, Access::ReadWrite);
    assert_eq!(held[0].reach, Reach::AndBelow);
}

#[test]
fn a_chat_with_ports_off_still_reaches_its_own_socket_and_no_other_runs() {
    for harness in [Harness::Codex, Harness::Opencode] {
        let it = started(harness, Os::MacOs);
        let (own, own_ca) = run_in(&it.root, "100-aaaaaaaa");
        let (other, _) = run_in(&it.root, "100-bbbbbbbb");
        let mut applied =
            it.applied
                .clone()
                .with_forge_route(inputs(&it.root, own.clone(), own_ca.clone()));
        match &mut *applied.form {
            Form::Codex(codex::Wrap { widened, .. })
            | Form::Opencode(opencode::Wrap { widened, .. }) => {
                widened.every_local = Some(Vec::new());
            }
            other => panic!("not a wrap: {other:?}"),
        }
        let profile = profile_of(&applied, &it.cwd);
        // Every local socket is let by (`ports = "off"`), then the route folder is held, then this
        // chat's own socket is given back: the last word is the one that counts.
        let every = profile.rfind("(allow network-outbound (remote unix-socket))");
        let held = profile.rfind("(deny network-outbound (remote unix-socket (path-regex #\"^");
        let own_rule = profile.rfind(&format!(
            "(allow network-outbound (remote unix-socket (path-literal \"{}\")))",
            own.display()
        ));
        let (every, held, own_rule) = (
            every.expect("ports off lets every socket"),
            held.expect("the route folder is held"),
            own_rule.expect("its own socket is given back"),
        );
        assert!(
            every < held && held < own_rule,
            "{harness:?}: the order is wrong"
        );
        assert!(
            !profile.contains(&format!(
                "(allow network-outbound (remote unix-socket (path-literal \"{}\")))",
                other.display()
            )),
            "{harness:?}: another run's socket is named"
        );
    }
}

#[test]
fn an_unwired_chat_with_a_persona_is_still_guarded_from_a_bare_glab() {
    let it = started_as(Harness::Codex, Os::MacOs, Some("dev"));
    assert_eq!(guarded(&it.applied), ["glab"]);
}

/// What holds a confinement a test builds, which has nothing to withdraw.
#[cfg(feature = "forge-wiring")]
#[derive(Debug)]
struct NoHolder;

#[cfg(feature = "forge-wiring")]
impl HeldElsewhere for NoHolder {
    fn id(&self) -> &str {
        "01TEST"
    }
    fn withdraw_port(&self, _port: u16) -> Result<(), String> {
        Ok(())
    }
    fn board(&self) -> Option<std::sync::Arc<dyn Board>> {
        None
    }
}

/// The sentinel [`inputs`] hands a chat.
#[cfg(feature = "forge-wiring")]
const SENTINEL: &str = "SENTINEL-0123456789abcdef";

/// A Claude Code chat of the persona `dev` whose route covers `forges`, and its start's line:
/// with a confinement (the proxy its wiring is composed for) where `confined`, none where not (an
/// unwrapped Claude Code below the proxy-port floor, which has no proxy to route through).
/// Answers the line and what the line arms ([`Applied::tools_for_line`]).
#[cfg(feature = "forge-wiring")]
fn started_line(
    forges: &[(&str, crate::forgewire::ForgeKind)],
    confined: bool,
) -> (Line, Option<tools::Resolved>, Started) {
    let it = started_as(Harness::ClaudeCode, Os::MacOs, Some("dev"));
    let route = inputs_over(&it, forges);
    // The CA the app made for the run, which the wiring copies.
    std::fs::write(&route.ca_cert, "CA").expect("the run's CA");
    let mut applied = it.applied.clone().with_forge_route(route);
    if confined {
        applied.answered("2.1.296 (Claude Code)");
    }
    let tmp = tempfile::tempdir().expect("a chat's temp folder");
    let confinement = Confinement::held_on_another_machine(
        Placed {
            http: 50_961,
            socks: 50_962,
            tmp: Some(tmp.path().to_path_buf()),
            routed: None,
            ssh: None,
            left_out: Vec::new(),
            host_name: None,
        },
        Box::new(NoHolder),
    );
    let Form::ClaudeCode(settings) = applied.form_in(Some(&it.cwd)) else {
        panic!("compiled for Claude Code");
    };
    let armed = vec![
        "--settings".to_owned(),
        serde_json::json!({
            "enabledPlugins": {},
            "permissions": { "allow": [], "deny": settings.deny },
            "sandbox": settings.sandbox,
        })
        .to_string(),
    ];
    let at = At {
        cwd: Some(&it.cwd),
        hook_socket: None,
        confinement: confined.then_some(&confinement),
        no_opt_out: false,
        purlis: None,
    };
    let armed_tools = applied
        .tools_for_line(at.confinement)
        .map(std::borrow::Cow::into_owned);
    let line = applied
        .line(
            Words {
                program: "claude".to_owned(),
                command: Vec::new(),
                armed,
                charters: Vec::new(),
            },
            &at,
        )
        .expect("a line");
    (line, armed_tools, it)
}

/// Whether a guard's shim is in a folder the line puts first on a `PATH`.
#[cfg(feature = "forge-wiring")]
fn guard_shim_on_path(line: &Line) -> bool {
    line.env
        .iter()
        .filter(|(key, _)| key == "PATH" || key == tools::FIRST_ON_PATH)
        .flat_map(|(_, dirs)| dirs.split(':'))
        .any(|dir| {
            std::fs::read_to_string(Path::new(dir).join("glab"))
                .is_ok_and(|text| text.contains("never falls back to the person's own login"))
        })
}

/// **The guard is dropped exactly where the line carries the sentinel** (ADR 0128): a wired chat
/// over a GitLab host has no guard shim on its `PATH` and `GITLAB_TOKEN` is the sentinel. With a
/// `purlis` found (the re-run below names one) the check reads the folder the shims were written
/// in, and so is not vacuous.
#[cfg(feature = "forge-wiring")]
#[test]
fn a_wired_chat_with_a_gitlab_host_has_no_guard_shim_and_the_sentinel_on_its_line() {
    use crate::forgewire::ForgeKind::{GitHub, GitLab};
    let (line, armed, _it) = started_line(&[("gitlab.com", GitLab), ("github.com", GitHub)], true);
    let token = line.env.iter().find(|(key, _)| key == "GITLAB_TOKEN");
    assert_eq!(token.map(|(_, value)| value.as_str()), Some(SENTINEL));
    assert!(armed.is_none_or(|tools| tools.guards.is_empty()));
    assert!(!guard_shim_on_path(&line), "{:?}", line.env);
}

#[cfg(feature = "forge-wiring")]
#[test]
fn the_guard_checks_of_a_wired_line_are_made_with_a_purlis_found() {
    let dir = tempfile::tempdir().expect("a folder");
    let purlis = dir.path().join("purlis");
    std::fs::write(&purlis, "").expect("a stand-in purlis");
    crate::testrun::rerun(
        &[
            "sandbox::forge_route_tests::a_wired_chat_with_a_gitlab_host_has_no_guard_shim_and_the_sentinel_on_its_line",
            "sandbox::forge_route_tests::a_chat_with_a_route_and_no_confinement_keeps_the_guard_and_has_no_sentinel",
        ],
        &[("PURLIS_BINARY", purlis.as_os_str())],
    );
}

/// **F1**: a chat whose inputs name a GitLab host but that has no confinement (so no proxy and no
/// wiring) keeps the guard: dropping it would leave `glab` with neither the shim nor a route.
#[cfg(feature = "forge-wiring")]
#[test]
fn a_chat_with_a_route_and_no_confinement_keeps_the_guard_and_has_no_sentinel() {
    use crate::forgewire::ForgeKind::GitLab;
    let (line, armed, _it) = started_line(&[("gitlab.com", GitLab)], false);
    assert!(
        line.env.iter().all(|(key, _)| key != "GITLAB_TOKEN"),
        "{:?}",
        line.env
    );
    let armed = armed.expect("the guard is armed");
    let names: Vec<&str> = armed.guards.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["glab"]);
    if tools::purlis_binary().is_some() {
        assert!(guard_shim_on_path(&line), "{:?}", line.env);
    }
}

#[cfg(feature = "forge-wiring")]
#[test]
fn a_wired_chat_with_no_gitlab_host_keeps_the_guard() {
    use crate::forgewire::ForgeKind::GitHub;
    let (_, armed, _it) = started_line(&[("github.com", GitHub)], true);
    assert_eq!(armed.expect("the guard is armed").guards.len(), 1);
}
