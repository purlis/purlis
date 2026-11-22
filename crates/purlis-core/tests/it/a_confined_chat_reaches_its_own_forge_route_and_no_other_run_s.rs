// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, K2: `gh` and `git` really work inside the real confined sandbox, and a chat reaches
//! its own run's socket and no other's (ADR 0128 D3a).
//!
//! The scenario is the product's: a sandbox is started for a chat the way the app starts one
//! ([`for_start`], then [`Applied::with_forge_route`] with the run's inputs), the chat's wiring is
//! composed by [`forgewire::compose`], and a process runs **under the profile that sandbox
//! generates** with that environment. What is real: the profile, the sandbox's kernel rules
//! (`sandbox-exec`), the real `gh` and `git`, and the real listener (`RunRoute`-style: a unix
//! socket and a `CONNECT` door) answering from a fake forge. The route folder is the one the
//! sandbox denies every chat, and the run's folders are made in it as the app makes them.
//!
//! It runs where the sandbox is applied by `sandbox-exec`, the macOS runner. On Linux the sandbox
//! is applied by purlis's own program, which this test binary does not have, so it says it
//! skips (`PURLIS_PROXY_REAL_CLIENTS` makes a missing tool a failure, as for the other real-client
//! tests).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use purlis_core::forge::proxy::EndpointKind;
use purlis_core::forge::proxy::listener::ForgeListener;
use purlis_core::forgewire::{self, ForgeKind, Inputs, Repo, SecretString};
use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Form, Machine, Os};

use crate::a_chats_forge_listener_serves_the_real_gh_glab_and_git::{clean, make_forge, run, tool};
use crate::support::forge_listener::World;

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// A folder only the app may enter, as the route's own are.
fn private(path: &Path) {
    std::fs::create_dir_all(path).expect("a folder");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).expect("private");
}

/// Runs `script` under `profile` with `env` over a clean environment, in `cwd`.
fn confined(
    profile: &str,
    home: &Path,
    cwd: &Path,
    env: &[(String, String)],
    script: &str,
) -> (bool, String) {
    let (program, args) = sandbox::wrapper::line(
        Os::MacOs,
        None,
        profile.to_owned(),
        "sh".to_owned(),
        vec!["-c".to_owned(), script.to_owned()],
    )
    .expect("sandbox-exec's line");
    let mut command = Command::new(program);
    clean(&mut command, home);
    command
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().cloned());
    run(command)
}

#[test]
fn gh_and_git_work_under_the_generated_profile_and_another_run_s_socket_is_refused() {
    purlis_core::unsteered!();
    if !cfg!(target_os = "macos") {
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            b"skipping: the sandbox is applied by sandbox-exec here\n",
        );
        return;
    }
    // Each says it is missing, and fails where the workflow says every tool must be there.
    if !(tool("gh") && tool("git") && tool("curl")) {
        return;
    }

    // The app's route folder, found as the app finds it: `pf-<uid>` in the temporary directory.
    let temp = tempfile::Builder::new()
        .prefix("k2")
        .tempdir_in("/tmp")
        .expect("a temporary directory");
    let home = tempfile::tempdir_in("/tmp").expect("a home");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("TMPDIR", real(temp.path()).to_str().unwrap())]),
        home: Some(real(home.path())),
        os: Os::MacOs,
    };
    let root = forgewire::route_root(&machine).expect("a route folder");
    private(&root);

    // Two runs, each with its own folder, listener and socket, as the app makes them.
    let world = World::new();
    let mut other_world = World::new();
    let forge = make_forge(temp.path());
    let mut wiring = world.wiring.clone();
    wiring.upstream = forge.clone();
    other_world.wiring.upstream = forge.clone();
    let own_dir = root.join("100-aaaaaaaa");
    let other_dir = root.join("100-bbbbbbbb");
    let own = ForgeListener::serve_socket(wiring.clone(), &own_dir).expect("own socket");
    let other =
        ForgeListener::serve_socket(other_world.wiring.clone(), &other_dir).expect("other socket");
    let connect = ForgeListener::serve_connect(wiring).expect("the door");
    let (EndpointKind::UnixSocket(own_socket), EndpointKind::UnixSocket(other_socket)) =
        (own.kind().clone(), other.kind().clone())
    else {
        panic!("sockets");
    };
    let EndpointKind::Loopback(door) = connect.kind().clone() else {
        panic!("a loopback door");
    };
    let ca = own_dir.join("ca.pem");
    std::fs::write(&ca, connect.ca_certificate_pem().expect("a CA")).expect("the CA");

    // The chat's project, and the sandbox the app starts for it.
    let plane = tempfile::Builder::new()
        .prefix("k2plane")
        .tempdir_in("/tmp")
        .expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("charter.toml");
    let project = real(plane.path());
    let cwd = project.join("w");
    std::fs::create_dir_all(&cwd).expect("the chat's folder");
    let helper = temp.path().join("helper.sh");
    std::fs::write(
        &helper,
        "#!/bin/sh\ncat >/dev/null\necho username=x-access-token\necho password=$PURLIS_FORGE_SENTINEL\n",
    )
    .expect("a helper");
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).expect("executable");

    let inputs = Inputs {
        gh_socket: own_socket.clone(),
        ca_cert: ca,
        sentinel: SecretString::from(world.sentinel.expose().to_owned()),
        helper_exe: helper,
        repos: vec![Repo::new("github.com", "acme", "app", ForgeKind::GitHub).expect("a repo")],
        door,
        handoff: world.wiring.handoff.clone(),
        hosts: vec!["github.com".to_owned()],
        root: root.clone(),
    };
    let applied = sandbox::for_start(Harness::Codex, &project, &machine, &|_| true)
        .expect("starts")
        .expect("sandboxed")
        .with_forge_route(inputs.clone());
    let Form::Codex(wrap) = applied.form_in(Some(&cwd)) else {
        panic!("a wrapped Codex");
    };
    // The chat's tmp, and its egress port: a relay that opens the door as the chat's egress proxy
    // does, proving the run's hand-off secret, since the door takes nothing else.
    let relay = crate::support::forge_listener::relay_to_door(door, world.wiring.handoff.clone());
    let tmp = tempfile::Builder::new()
        .prefix("k2tmp")
        .tempdir_in("/tmp")
        .expect("the chat's tmp");
    let profile =
        sandbox::codex::profile(&wrap, &cwd, &real(tmp.path()), &[relay], None).expect("a profile");

    let wired = forgewire::compose(&inputs.route(format!("http://127.0.0.1:{relay}")), &project)
        .expect("the wiring");
    let mut env = wired.env.clone();
    env.push((
        "PATH".to_owned(),
        format!(
            "{}:{}",
            wired.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    ));
    env.push(("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()));
    env.push(("GIT_CONFIG_NOSYSTEM".to_owned(), "1".to_owned()));
    env.push(("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".to_owned()));
    env.push(("GH_PROMPT_DISABLED".to_owned(), "1".to_owned()));
    env.push(("GH_NO_UPDATE_NOTIFIER".to_owned(), "1".to_owned()));
    env.push((
        "GIT_CONFIG_COUNT".to_owned(),
        wired.git_pairs.len().to_string(),
    ));
    for (n, (key, value)) in wired.git_pairs.iter().enumerate() {
        env.push((format!("GIT_CONFIG_KEY_{n}"), key.clone()));
        env.push((format!("GIT_CONFIG_VALUE_{n}"), value.clone()));
    }
    let chat_home = cwd.join(".home");
    std::fs::create_dir_all(&chat_home).expect("a home");
    env.push(("TMPDIR".to_owned(), real(tmp.path()).display().to_string()));
    let say = |script: &str| confined(&profile, &chat_home, &cwd, &env, script);

    // gh: through its own socket, to the fake forge.
    let (ok, text) = say("gh api repos/acme/app");
    assert!(
        ok && text.contains("acme/app"),
        "gh api under the sandbox: {text}"
    );
    // git: through the door, under the chat's own CA copy.
    let (ok, text) = say("git ls-remote https://github.com/acme/app.git");
    assert!(
        ok && text.contains("refs/heads/main"),
        "git ls-remote under the sandbox: {text}"
    );
    // The forge only ever saw the real token, never the sentinel.
    for request in forge.seen.lock().unwrap().iter() {
        assert!(
            !format!("{request:?}").contains(world.sentinel.expose()),
            "the sentinel reached the forge"
        );
    }

    // Another run's socket is refused, where the same socket answers outside the sandbox: the
    // sandbox is what refuses it, and the chat's own is connected to.
    let curl = |socket: &Path| {
        format!(
            "curl --unix-socket '{}' -sS -o /dev/null --max-time 20 http://github.localhost/user",
            socket.display()
        )
    };
    let (_, own_text) = say(&curl(&own_socket));
    assert!(
        !own_text.contains("Couldn't connect") && !own_text.contains("Operation not permitted"),
        "its own socket is refused: {own_text}"
    );
    let (other_ok, other_text) = say(&curl(&other_socket));
    assert!(
        !other_ok
            && (other_text.contains("Couldn't connect")
                || other_text.contains("Operation not permitted")),
        "another run's socket was reached: ok={other_ok} {other_text}"
    );
    let mut outside = Command::new("curl");
    clean(&mut outside, home.path());
    let (_, outside_text) = run({
        outside.args([
            "--unix-socket",
            other_socket.to_str().unwrap(),
            "-sS",
            "-o",
            "/dev/null",
            "--max-time",
            "20",
            "http://github.localhost/user",
        ]);
        outside
    });
    assert!(
        !outside_text.contains("Couldn't connect"),
        "that socket is not answering outside the sandbox either, so the refusal proves nothing: {outside_text}"
    );
    // Nor can the chat list the route folder, or read another run's certificate.
    let (listed, text) = say(&format!("ls '{}'", root.display()));
    assert!(!listed, "the route folder was listed: {text}");
    drop((own, other, connect));
}
