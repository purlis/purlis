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
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("charter.toml");
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
    let applied = for_start(harness, &project, &machine, &|_| true)
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
        hosts: vec!["github.com".to_owned()],
        root: root.to_path_buf(),
    }
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
fn a_chat_in_a_plane_where_the_app_made_no_route_folder_is_denied_none() {
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
    assert!(!denied.paths.iter().any(|it| it.path == root));
    std::fs::create_dir_all(&root).expect("made");
    let denied = Denied::of(&real(plane.path()), &machine);
    let held: Vec<_> = denied.paths.iter().filter(|it| it.path == root).collect();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].access, Access::ReadWrite);
    assert_eq!(held[0].reach, Reach::AndBelow);
}
