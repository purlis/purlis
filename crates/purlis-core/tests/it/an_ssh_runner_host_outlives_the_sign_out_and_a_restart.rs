// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's session host kept by systemd outlives an SSH drop and a sign-out, and comes back
//! after a restart, for real** (RR-15, #724; ADR 0068 §3 as amended for #724). Against the remote
//! parity job's systemd runner fixture (`tests/ssh-runner/up-systemd.sh`): an Ubuntu machine that
//! boots systemd, with an sshd whose sign-ins are logind sessions and no purlis.
//!
//! Through the pinned route, as the desktop will:
//!
//! 1. this build's purlis is installed as a version (RR-14) and paired, and
//!    `purlis serve --install` hands its host to the account's systemd user manager: the unit is
//!    enabled and started, and linger is turned on by the account itself;
//! 2. doctor finds the unit installed, linger on and the host answering;
//! 3. two chats run on the host; then every SSH connection of the account is cut (its `sshd`
//!    processes killed, as a dropped link leaves them) and the account is signed out: logind has
//!    no session of it and says it lingers. The unit is still active, both chats' processes still
//!    run, and the host, reached again through its bridge, still lists both;
//! 4. the machine restarts (`docker restart`: systemd shuts down and boots again). Nothing is
//!    run over SSH to start the host, and it serves again; the two chats ended with the machine,
//!    and the host's first act was to settle each from its host record as ended by the restart:
//!    the record is emptied and its log says so. Doctor is green again.
//!
//! The chats are shells (the chat's harness is not what this checks). A harness chat's end is
//! kept in the event log by the same take-over (`left_by_its_last_host`), which
//! `a_host_started_after_one_that_crashed_settles_what_it_left` checks in the host's own tests.
//!
//! It needs the fixture: `SSH_RUNNER_SYSTEMD_TEST_ROUTE` (`ssh://keeper@127.0.0.1:2225`),
//! `SSH_RUNNER_SYSTEMD_TEST_MACHINE` (the container, which the test restarts and looks into as
//! its root) and `SSH_RUNNER_TEST_PURLIS` (this build's `purlis`). Without them it runs nothing
//! and passes: a sandboxed agent can't run it.

#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use purlis_core::runnerinstall::{self, Delivery, Verified};
use purlis_core::sshroute::{self, Reach, Route, connect};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::session::{Client, Launch, ProgramKind, StartProgram};

const PATIENCE: Duration = Duration::from_secs(90);
const PROJECT: &str = "01J9PRJCT00000000000000000";
const VERSION: &str = "0.0.1-rr15.1";
const CHATS: usize = 2;

struct Fixture {
    reach: Reach,
    machine: String,
    binary: Vec<u8>,
    _settings: tempfile::TempDir,
}

fn fixture() -> Option<Fixture> {
    let route =
        Route::parse(&std::env::var("SSH_RUNNER_SYSTEMD_TEST_ROUTE").ok()?).expect("a route");
    let machine = std::env::var("SSH_RUNNER_SYSTEMD_TEST_MACHINE").ok()?;
    let binary = std::fs::read(std::env::var("SSH_RUNNER_TEST_PURLIS").ok()?).expect("purlis");
    let settings = tempfile::Builder::new()
        .prefix("rr")
        .tempdir_in("/tmp")
        .expect("a folder");
    let ssh = sshroute::ssh_here().expect("the system's ssh");
    let key = connect::meet(&ssh, &route, settings.path()).expect("the machine is met");
    Some(Fixture {
        reach: Reach {
            ssh,
            route,
            pool: "rr15".to_owned(),
            key,
            settings_in: settings.path().to_path_buf(),
        },
        machine,
        binary,
        _settings: settings,
    })
}

/// `bytes` signed with a key of the test's own and verified as the desktop verifies an asset.
fn verified(bytes: &[u8], target: runnerinstall::Target) -> Verified {
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let trusted = format!(
        "timestamp:1790077121\tfile:{}\tversion:{VERSION}",
        runnerinstall::asset_name(VERSION, target)
    );
    let signature = minisign::sign(
        None,
        &keys.sk,
        std::io::Cursor::new(bytes),
        Some(&trusted),
        None,
    )
    .unwrap();
    let public = b64(keys.pk.to_box().unwrap().into_string().as_bytes());
    let signature = b64(signature.into_string().as_bytes());
    runnerinstall::verify(&public, bytes, &signature, VERSION, target).expect("verified")
}

/// What `command` printed on the runner, run there through the pinned route.
fn there(reach: &Reach, command: &str, input: Option<&str>) -> sshroute::Ran {
    let (_folder, known_hosts) = reach.known_hosts_for_a_run().expect("known_hosts");
    let mut args = sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts);
    args.pop();
    args.push(command.to_owned());
    sshroute::run_giving(&reach.ssh, &args, input, 256 * 1024).expect("ssh runs")
}

/// What `command` printed on the machine as its root, from outside it (what an operator at its
/// console sees): never through the account under test, so it signs nobody in.
fn as_root(machine: &str, command: &str) -> (bool, String) {
    let out = purlis_core::forklock::output(
        std::process::Command::new("docker").args(["exec", machine, "sh", "-c", command]),
    )
    .expect("docker runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// Waits until `check` holds, up to the test's patience, saying `what` where it never does.
fn until(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !check() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Doctor's rows on the runner, by name: each `(status, detail)`.
fn doctor(reach: &Reach, binary: &str) -> std::collections::HashMap<String, (String, String)> {
    let ran = there(reach, &format!("{binary} doctor --json"), None);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&ran.printed)
        .unwrap_or_else(|why| panic!("doctor's rows read ({why}): {}\n{}", ran.printed, ran.said));
    rows.into_iter()
        .map(|row| {
            let field = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
            (field("name"), (field("status"), field("detail")))
        })
        .collect()
}

fn assert_doctor_green(reach: &Reach, binary: &str) {
    let rows = doctor(reach, binary);
    for name in [
        purlis_core::runnerhost::SERVICE_ROW,
        purlis_core::runnerhost::LINGER_ROW,
        purlis_core::runnerhost::REACHABLE_ROW,
    ] {
        let (status, detail) = rows
            .get(name)
            .unwrap_or_else(|| panic!("doctor has no {name} row: {rows:?}"));
        assert_eq!(status, "ok", "{name}: {detail}");
    }
}

/// A client of the host through its bridge over the pinned route, and the task keeping its link.
async fn linked(
    reach: &Reach,
    binary: &str,
    ours: Identity,
    runner: Pinned,
) -> (
    Client,
    purlis_core::dockerpool::settings::RunFolder,
    tokio::task::JoinHandle<()>,
) {
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
            let kept = tokio::spawn(async move { while links.next().await.is_some() {} });
            (client, folder, kept)
        }
        Ok(Some(Linked::Down(down))) => panic!("not linked: {}", down.why),
        Ok(None) => panic!("the kept link ended"),
        Err(_) => panic!("no link within the test's patience"),
    }
}

async fn chat(client: &Client, said: &str) -> String {
    client
        .start_program(StartProgram {
            launch: Some(Launch {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), format!("echo '{said}'; exec sleep 6000")],
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

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// The account's chats' processes on the machine, counted from outside it.
fn chat_processes(machine: &str) -> usize {
    let (_, out) = as_root(machine, "pgrep -u keeper -fc '^sleep 6000$' || true");
    out.trim().parse().unwrap_or(0)
}

#[test]
fn a_runners_host_outlives_the_sign_out_and_comes_back_after_a_restart() {
    purlis_core::unsteered!();
    let Some(fixture) = fixture() else {
        // No systemd runner fixture is named: nothing is run (the module's docs say where).
        return;
    };
    let reach = &fixture.reach;
    let machine = fixture.machine.as_str();

    // 1. Installed as a version, paired, and its host handed to systemd.
    let probed = runnerinstall::probe(reach).expect("the runner is probed");
    let binary = runnerinstall::install(
        reach,
        &probed,
        &verified(&fixture.binary, probed.target),
        Delivery::Upload(&fixture.binary),
    )
    .expect("installed")
    .binary()
    .to_owned();
    let desktop_key = LinkKey::generate().unwrap();
    let ours = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: desktop_key.public(),
    };
    let line = format!("{}\n", purlis_core::runnerlink::to_line(&ours));
    let ran = there(reach, &format!("{binary} bridge --pair"), Some(&line));
    assert_eq!(ran.code, Some(0), "paired: {}", ran.said);
    let runner = purlis_core::runnerlink::from_line(&ran.printed).expect("the runner's key");
    let identity = || Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::from_private(desktop_key.private_half().unwrap()).unwrap(),
    };

    let installed = there(reach, &format!("{binary} serve --install"), None);
    assert_eq!(installed.code, Some(0), "installed: {}", installed.said);
    assert!(
        installed.printed.contains("systemd starts it again"),
        "{}",
        installed.printed
    );
    assert!(
        installed
            .printed
            .contains("Your chats keep running after you sign out."),
        "linger turned on by the account itself: {}",
        installed.printed
    );
    let unit = purlis_core::runnerhost::Service {
        binary: binary.clone().into(),
        version: Some(VERSION.to_owned()),
    }
    .unit_name();
    let (lingers, said) = as_root(machine, "ls /var/lib/systemd/linger");
    assert!(lingers && said.contains("keeper"), "linger is on: {said}");

    // 2. Doctor: the unit installed, linger on, the host answering.
    until("the host answers", || {
        there(
            reach,
            &format!("systemctl --user is-active --quiet {unit}"),
            None,
        )
        .code
            == Some(0)
    });
    assert_doctor_green(reach, &binary);

    // 3. Two chats; then every connection cut and the account signed out.
    let rt = runtime();
    let chats = rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the host holds the project");
        let mut chats = Vec::new();
        for n in 0..CHATS {
            chats.push(chat(&client, &format!("chat {n}")).await);
        }
        chats.sort();
        assert_eq!(running(&client).await, chats);
        kept.abort();
        chats
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    // A dropped link: each of the account's SSH connections ends without a word.
    let _ = as_root(machine, "pkill -KILL -u keeper '^sshd' || true");
    until("the account is signed out, its manager lingering", || {
        let (_, sessions) = as_root(machine, "loginctl list-sessions --no-legend | grep keeper");
        let (_, state) = as_root(
            machine,
            "loginctl show-user keeper --property=State --value",
        );
        sessions.trim().is_empty() && state.trim() == "lingering"
    });
    let (active, said) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-active {unit}"),
    );
    assert!(
        active,
        "the host is still running after the sign-out: {said}"
    );
    assert_eq!(
        chat_processes(machine),
        CHATS,
        "both chats still run after the sign-out"
    );
    let rt = runtime();
    rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        assert_eq!(
            running(&client).await,
            chats,
            "the host still lists both chats, reached again"
        );
        kept.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    let (_, record) = as_root(
        machine,
        "cat /home/keeper/.config/*/purlisd/server/*/hosts.jsonl",
    );
    assert_eq!(
        record.matches("\"started\"").count(),
        CHATS,
        "the host noted both chats: {record}"
    );

    // 4. The machine restarts: nothing is run over SSH to start the host.
    let restarted = purlis_core::forklock::output(
        std::process::Command::new("docker").args(["restart", "--time", "60", machine]),
    )
    .expect("docker runs");
    assert!(restarted.status.success(), "the machine restarted");
    until("the machine is up again", || {
        as_root(
            machine,
            "systemctl is-system-running --wait >/dev/null 2>&1; true",
        )
        .0 && as_root(
            machine,
            &format!("systemctl --user --machine=keeper@ is-active --quiet {unit}"),
        )
        .0
    });
    let (_, journal) = as_root(
        machine,
        &format!("journalctl -b --no-pager _SYSTEMD_USER_UNIT={unit}"),
    );
    until("the host settled what the restart ended", || {
        let (_, journal) = as_root(
            machine,
            &format!("journalctl -b --no-pager _SYSTEMD_USER_UNIT={unit}"),
        );
        journal.contains(&format!(
            "ended without ending {CHATS} of its programs; each is settled as ended by its crash"
        ))
    });
    let (_, record) = as_root(
        machine,
        "wc -c < /home/keeper/.config/*/purlisd/server/*/hosts.jsonl",
    );
    assert_eq!(record.trim(), "0", "the record was taken over: {journal}");
    assert_eq!(
        chat_processes(machine),
        0,
        "the chats ended with the machine"
    );
    let rt = runtime();
    rt.block_on(async {
        let (client, _folder, kept) = linked(reach, &binary, identity(), runner.clone()).await;
        assert!(
            running(&client).await.is_empty(),
            "nothing of the last boot runs, and the host serves"
        );
        kept.abort();
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    assert_doctor_green(reach, &binary);

    // Taken back: it stops and its unit goes.
    let removed = there(reach, &format!("{binary} serve --remove"), None);
    assert_eq!(removed.code, Some(0), "removed: {}", removed.said);
    let (still, _) = as_root(
        machine,
        &format!("systemctl --user --machine=keeper@ is-active --quiet {unit}"),
    );
    assert!(!still, "the host stopped with its unit");
}
