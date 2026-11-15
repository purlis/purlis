// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **purlis installed on a machine over SSH, and a new version beside the old one, for real**
//! (RR-14, #723; ADR 0078 §5, ADR 0068 §8). Against the remote parity job's SSH runner fixture
//! (`tests/ssh-runner/up.sh`): an Ubuntu machine with an sshd, no purlis and no Docker, with
//! simulated latency and loss (QA-15).
//!
//! The binary of this build is signed with a key of the test's own, as the release signs a
//! runner asset, and verified as the desktop verifies one. Then, through the pinned route:
//!
//! 1. a copy changed on the way (a `sha256sum` that reports another digest, as for the fixture's
//!    `liar` user) is deleted, and nothing is installed; and for the fixture's `fisher` user,
//!    whose login shell is fish, which reads no POSIX `sh`, a version is installed, found there
//!    again and hashed before it is trusted, and placed again once it changed;
//! 2. version 1 is installed, paired, its host started, and five chats run on it;
//! 3. version 2 is installed beside it and its host started: version 1's binary is untouched,
//!    its five chats keep running and are still listed by its host through its own bridge, and
//!    new chats run on version 2's host, which holds none of version 1's;
//! 4. version 3 is installed while version 1's chats run: nothing is removed (#6688). Once they
//!    have ended and its host has been stopped by a signal, the next install removes version 1's
//!    folders, its binary's and its host's, and versions 2 and 3 stay.
//!
//! The issue asks for five live chats; each is a shell on the runner (the chat's harness is not
//! what this checks), so five cost CI next to nothing, and five are run.
//!
//! It needs the fixture: `SSH_RUNNER_TEST_ROUTE` (`ssh://runner@127.0.0.1:2224`),
//! `SSH_RUNNER_TEST_LIAR_ROUTE`, `SSH_RUNNER_TEST_FISH_ROUTE` and `SSH_RUNNER_TEST_PURLIS` (this
//! build's `purlis`, for the runner's platform). Without them it runs nothing and passes: a
//! sandboxed agent can't run it.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use purlis_core::runnerinstall::{self, Delivery, Installed, Refused, Verified};
use purlis_core::sshroute::{self, Reach, Route, connect};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::session::{Client, Launch, ProgramKind, StartProgram};

const PATIENCE: Duration = Duration::from_secs(90);
const PROJECT: &str = "01J9PRJCT00000000000000000";
const CHATS: usize = 5;
const V1: &str = "0.0.1-rr14.1";
const V2: &str = "0.0.1-rr14.2";
const V3: &str = "0.0.1-rr14.3";

struct Fixture {
    reach: Reach,
    liar: Reach,
    fish: Reach,
    binary: Vec<u8>,
    _settings: tempfile::TempDir,
}

fn fixture() -> Option<Fixture> {
    let route = Route::parse(&std::env::var("SSH_RUNNER_TEST_ROUTE").ok()?).expect("a route");
    let liar = Route::parse(&std::env::var("SSH_RUNNER_TEST_LIAR_ROUTE").ok()?).expect("a route");
    let fish = Route::parse(&std::env::var("SSH_RUNNER_TEST_FISH_ROUTE").ok()?).expect("a route");
    let binary = std::fs::read(std::env::var("SSH_RUNNER_TEST_PURLIS").ok()?).expect("purlis");
    let settings = tempfile::Builder::new()
        .prefix("rr")
        .tempdir_in("/tmp")
        .expect("a folder");
    let ssh = sshroute::ssh_here().expect("the system's ssh");
    // The machine met with nothing signed in, and the key it presented pinned: Trust this machine.
    let key = connect::meet(&ssh, &route, settings.path()).expect("the machine is met");
    let reach = |route: Route, name: &str| Reach {
        ssh: ssh.clone(),
        route,
        pool: name.to_owned(),
        key: key.clone(),
        settings_in: settings.path().to_path_buf(),
    };
    Some(Fixture {
        reach: reach(route, "rr14"),
        liar: reach(liar, "rr14-liar"),
        fish: reach(fish, "rr14-fish"),
        binary,
        _settings: settings,
    })
}

/// A key of the test's own, standing for the updater's (ADR 0042).
struct ReleaseKey(minisign::KeyPair);

impl ReleaseKey {
    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn public(&self) -> String {
        Self::b64(self.0.pk.to_box().unwrap().into_string().as_bytes())
    }

    /// `bytes` signed and verified as the release and the desktop do it.
    fn verified(&self, bytes: &[u8], version: &str, target: runnerinstall::Target) -> Verified {
        let trusted = format!(
            "timestamp:1790077121\tfile:{}\tversion:{version}",
            runnerinstall::asset_name(version, target)
        );
        let signature = minisign::sign(
            None,
            &self.0.sk,
            std::io::Cursor::new(bytes),
            Some(&trusted),
            None,
        )
        .unwrap();
        let signature = Self::b64(signature.into_string().as_bytes());
        runnerinstall::verify(&self.public(), bytes, &signature, version, target).expect("verified")
    }
}

/// What `command` printed on the runner, run there through the pinned route.
fn there(reach: &Reach, command: &str, input: Option<&str>) -> sshroute::Ran {
    let (_folder, known_hosts) = reach.known_hosts_for_a_run().expect("known_hosts");
    let mut args = sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts);
    args.pop();
    args.push(command.to_owned());
    sshroute::run_giving(&reach.ssh, &args, input, 64 * 1024).expect("ssh runs")
}

/// The runner paired with the desktop `ours`, through version `binary`'s bridge.
fn paired(reach: &Reach, binary: &str, ours: &Pinned) -> Pinned {
    let line = format!("{}\n", purlis_core::runnerlink::to_line(ours));
    let ran = there(reach, &format!("{binary} bridge --pair"), Some(&line));
    assert_eq!(ran.code, Some(0), "paired: {}", ran.said);
    purlis_core::runnerlink::from_line(&ran.printed).expect("the runner's key")
}

/// Version `binary`'s host started on the runner, detached from the session that starts it
/// (RR-15's supervisor is what keeps it up for a person).
fn host_started(reach: &Reach, binary: &str) {
    let ran = there(
        reach,
        &format!("setsid -f {binary} serve </dev/null >>\"$HOME/serve.log\" 2>&1"),
        None,
    );
    assert_eq!(ran.code, Some(0), "started: {}", ran.said);
}

/// A client of version `binary`'s host, through its own bridge over the pinned route: the
/// connector RR-1 builds on (`ssh <pinned options> host <binary> bridge`).
async fn linked(
    reach: &Reach,
    binary: &str,
    ours: Identity,
    runner: Pinned,
) -> (Client, purlis_core::dockerpool::settings::RunFolder) {
    let (folder, known_hosts) = reach.known_hosts_for_a_run().expect("known_hosts");
    let mut args = sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts);
    args.pop();
    args.push(format!("{binary} bridge"));
    let connector = CommandConnector::new(reach.ssh.clone(), args, Arc::new(|| Ok(())));
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        ours,
        runner,
        Liveness::default(),
    );
    match tokio::time::timeout(PATIENCE, links.next()).await {
        Ok(Some(Linked::Up(link))) => {
            let (client, _views, _events) = Client::new(*link);
            tokio::spawn(async move { while links.next().await.is_some() {} });
            (client, folder)
        }
        Ok(Some(Linked::Down(down))) => panic!("not linked to {binary}: {}", down.why),
        Ok(None) => panic!("the kept link ended"),
        Err(_) => panic!("no link to {binary} within the test's patience"),
    }
}

/// A chat on the runner: a shell that says `said` and stays.
async fn chat(client: &Client, said: &str) -> String {
    client
        .start_program(StartProgram {
            launch: Some(Launch {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), format!("echo '{said}'; exec sleep 600")],
                cwd: None,
                set: Vec::new(),
                strip: Vec::new(),
                pass: Vec::new(),
                declared: Vec::new(),
                harness: None,
                git_hooks: None,
                reporting: None,
                confinement: None,
                prompt_key: None,
                pools_held: None,
            }),
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .expect("the chat starts on the runner")
}

/// The chats a host lists that have not ended.
async fn running(client: &Client) -> Vec<String> {
    let mut ids: Vec<String> = client
        .list()
        .await
        .expect("listed")
        .into_iter()
        .filter(|chat| chat.state != "ended")
        .map(|chat| chat.chat)
        .collect();
    ids.sort();
    ids
}

/// Whether version `version`'s host has a folder of its own in `config` on the runner
/// (`<config>/purlisd/server/<version>`, under either spelling of the host's folder).
fn host_folder_there(reach: &Reach, config: &str, version: &str) -> bool {
    let folders: Vec<String> = purlis_core::names::DAEMON_DIR
        .spellings()
        .map(|daemon| format!("{config}/{daemon}/{}/{version}", runnerinstall::SERVER_DIR))
        .collect();
    let ran = there(
        reach,
        &format!(
            "for f in {}; do [ -e \"$f\" ] && exit 0; done; exit 1",
            folders.join(" ")
        ),
        None,
    );
    ran.code == Some(0)
}

/// Once version `binary`'s host has stopped on the runner.
async fn stopped(reach: &Reach, binary: &str) {
    let until = std::time::Instant::now() + PATIENCE;
    loop {
        let ran = there(
            reach,
            &format!("pgrep -f '^{binary} serve$' >/dev/null"),
            None,
        );
        if ran.code == Some(1) {
            return;
        }
        assert!(
            std::time::Instant::now() < until,
            "{binary}'s host is still serving"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn identity(key: &LinkKey) -> Identity {
    Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::from_private(key.private_half().unwrap()).unwrap(),
    }
}

#[test]
fn a_new_version_is_installed_beside_the_old_whose_chats_keep_running() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else {
        // No SSH runner fixture is named: nothing is run (the module's docs say where it runs).
        return;
    };
    let release = ReleaseKey(minisign::KeyPair::generate_unencrypted_keypair().unwrap());
    let probed = runnerinstall::probe(&fixture.reach).expect("the runner is probed");
    assert!(
        probed.versions.is_empty(),
        "no purlis there yet: {probed:?}"
    );
    let v1 = release.verified(&fixture.binary, V1, probed.target);
    let v2 = release.verified(&fixture.binary, V2, probed.target);
    let v3 = release.verified(&fixture.binary, V3, probed.target);

    // 1. A copy that isn't the one verified is deleted, and nothing is installed.
    let liar_probed = runnerinstall::probe(&fixture.liar).expect("the liar is probed");
    let refused = runnerinstall::install(
        &fixture.liar,
        &liar_probed,
        &v1,
        Delivery::Upload(&fixture.binary),
    )
    .unwrap_err();
    assert!(matches!(refused, Refused::NotTheSame { .. }), "{refused:?}");
    let left = there(
        &fixture.liar,
        &format!("ls -A {}", runnerinstall::server_dir(&liar_probed.config)),
        None,
    );
    assert_eq!(
        left.printed.trim(),
        "",
        "nothing left behind: {}",
        left.printed
    );

    // 1a. Through a login shell that is fish: installed, found again and checked, and placed
    // again once the copy there changed.
    let fish_probed = runnerinstall::probe(&fixture.fish).expect("the fish user is probed");
    let fish_bin = runnerinstall::install(
        &fixture.fish,
        &fish_probed,
        &v1,
        Delivery::Upload(&fixture.binary),
    )
    .expect("installed through fish")
    .binary()
    .to_owned();
    let fish_probed = runnerinstall::probe(&fixture.fish).unwrap();
    assert_eq!(fish_probed.versions, [V1]);
    assert!(matches!(
        runnerinstall::install(
            &fixture.fish,
            &fish_probed,
            &v1,
            Delivery::Upload(&fixture.binary)
        ),
        Ok(Installed::AlreadyThere { .. })
    ));
    let changed = there(
        &fixture.fish,
        &runnerinstall::for_login_shell(&format!("printf x >> {fish_bin}")),
        None,
    );
    assert_eq!(changed.code, Some(0), "{}", changed.said);
    assert!(matches!(
        runnerinstall::install(
            &fixture.fish,
            &fish_probed,
            &v1,
            Delivery::Upload(&fixture.binary)
        ),
        Ok(Installed::Placed { .. })
    ));
    let fish_digest = there(
        &fixture.fish,
        &runnerinstall::for_login_shell(&format!("sha256sum {fish_bin}")),
        None,
    );
    assert!(
        fish_digest.printed.starts_with(v1.digest()),
        "placed again: {}",
        fish_digest.printed
    );

    // 2. Version 1 installed, paired, and five chats on its host.
    let installed = runnerinstall::install(
        &fixture.reach,
        &probed,
        &v1,
        Delivery::Upload(&fixture.binary),
    )
    .expect("version 1 is installed");
    let bin_v1 = installed.binary().to_owned();
    assert_eq!(bin_v1, runnerinstall::binary_at(&probed.config, V1));
    let mode = there(&fixture.reach, &format!("stat -c %a {bin_v1}"), None);
    assert_eq!(mode.printed.trim(), "700");
    let digest_v1 = there(&fixture.reach, &format!("sha256sum {bin_v1}"), None).printed;
    // Installing it again sends nothing.
    let probed = runnerinstall::probe(&fixture.reach).unwrap();
    assert_eq!(probed.versions, [V1]);
    assert!(matches!(
        runnerinstall::install(
            &fixture.reach,
            &probed,
            &v1,
            Delivery::Upload(&fixture.binary)
        ),
        Ok(Installed::AlreadyThere { .. })
    ));

    let desktop_key = LinkKey::generate().unwrap();
    let ours = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: desktop_key.public(),
    };
    let runner = paired(&fixture.reach, &bin_v1, &ours);
    host_started(&fixture.reach, &bin_v1);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (old, _old_folder) = linked(
            &fixture.reach,
            &bin_v1,
            identity(&desktop_key),
            runner.clone(),
        )
        .await;
        old.hold_project(PROJECT, "")
            .await
            .expect("version 1's host holds the project");
        let mut on_v1 = Vec::new();
        for n in 0..CHATS {
            on_v1.push(chat(&old, &format!("on version 1, chat {n}")).await);
        }
        on_v1.sort();
        assert_eq!(running(&old).await, on_v1);

        // 3. Version 2 installed beside it, and its host started.
        let probed = runnerinstall::probe(&fixture.reach).unwrap();
        let installed = runnerinstall::install(
            &fixture.reach,
            &probed,
            &v2,
            Delivery::Upload(&fixture.binary),
        )
        .expect("version 2 is installed");
        let bin_v2 = installed.binary().to_owned();
        assert_ne!(bin_v2, bin_v1);
        assert_eq!(
            runnerinstall::probe(&fixture.reach).unwrap().versions,
            [V1, V2]
        );
        assert_eq!(
            there(&fixture.reach, &format!("sha256sum {bin_v1}"), None).printed,
            digest_v1,
            "version 1's binary is left as it was"
        );
        host_started(&fixture.reach, &bin_v2);

        let (new, _new_folder) = linked(
            &fixture.reach,
            &bin_v2,
            identity(&desktop_key),
            runner.clone(),
        )
        .await;
        assert!(
            running(&new).await.is_empty(),
            "version 2's host holds none of version 1's chats"
        );
        new.hold_project(PROJECT, "")
            .await
            .expect("version 2's host holds the project");
        let mut on_v2 = vec![
            chat(&new, "on version 2, chat 0").await,
            chat(&new, "on version 2, chat 1").await,
        ];
        on_v2.sort();
        assert_eq!(running(&new).await, on_v2);

        // Version 1's chats are all still running, on its own host, reached again through its own
        // bridge after version 2's host started: nothing ended them.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let (old_again, _again_folder) = linked(
            &fixture.reach,
            &bin_v1,
            identity(&desktop_key),
            runner.clone(),
        )
        .await;
        assert_eq!(running(&old_again).await, on_v1);
        assert_eq!(running(&old).await, on_v1);
        let serving = there(&fixture.reach, "pgrep -fa ' serve$' || true", None).printed;
        assert!(
            serving.contains(&bin_v1) && serving.contains(&bin_v2),
            "both hosts serve: {serving}"
        );

        // 4. Version 3 installed while version 1's chats run: version 1 stays, all of it.
        let probed = runnerinstall::probe(&fixture.reach).unwrap();
        let installed = runnerinstall::install(
            &fixture.reach,
            &probed,
            &v3,
            Delivery::Upload(&fixture.binary),
        )
        .expect("version 3 is installed");
        assert!(matches!(installed, Installed::Placed { .. }));
        assert!(
            installed.pruned().is_empty(),
            "a version with a live chat is never removed: {installed:?}"
        );
        assert_eq!(
            runnerinstall::probe(&fixture.reach).unwrap().versions,
            [V1, V2, V3]
        );
        assert!(host_folder_there(&fixture.reach, &probed.config, V1));
        assert_eq!(running(&old).await, on_v1, "version 1's chats still run");

        // Version 1 drains: its chats end, and its host is stopped as a person or a supervisor
        // stops it, by a signal (a desktop's link may not quit a runner's host; only the
        // window's may quit its own). Killed so, it empties nothing: its record keeps each
        // chat's start beside its end.
        for chat in &on_v1 {
            old.stop(chat).await.expect("a version 1 chat stops");
        }
        let killed = there(
            &fixture.reach,
            &format!("pkill -TERM -f '^{bin_v1} serve$'"),
            None,
        );
        assert_eq!(
            killed.code,
            Some(0),
            "version 1's host signalled: {}",
            killed.said
        );
        stopped(&fixture.reach, &bin_v1).await;

        // The next install removes version 1's folders, and versions 2 and 3 stay.
        let probed = runnerinstall::probe(&fixture.reach).unwrap();
        let installed = runnerinstall::install(
            &fixture.reach,
            &probed,
            &v3,
            Delivery::Upload(&fixture.binary),
        )
        .expect("version 3 is checked again");
        assert!(matches!(installed, Installed::AlreadyThere { .. }));
        assert_eq!(installed.pruned(), [V1]);
        assert_eq!(
            runnerinstall::probe(&fixture.reach).unwrap().versions,
            [V2, V3]
        );
        let left = there(
            &fixture.reach,
            &format!("ls -A {}", runnerinstall::server_dir(&probed.config)),
            None,
        );
        assert_eq!(
            left.printed.split_whitespace().collect::<Vec<_>>(),
            [V2, V3]
        );
        assert!(
            !host_folder_there(&fixture.reach, &probed.config, V1),
            "version 1's host folder is gone"
        );
        assert!(host_folder_there(&fixture.reach, &probed.config, V2));
        assert_eq!(running(&new).await, on_v2, "version 2's chats still run");

        for chat in &on_v2 {
            new.stop(chat).await.expect("a version 2 chat stops");
        }
        let _ = new.quit().await;
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    // What the hosts said is printed by the job when a test fails (`ci.yml`).
}
