// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a test needs to drive a runner on a machine behind an sshd, for real**: a command run
//! there through the pinned route, one run as the machine's root from outside, a client of the
//! runner's host through its bridge, and chats on it. Shared by the systemd runner's test
//! (`an_ssh_runner_host_outlives_the_sign_out_and_a_restart`) and the fault suite
//! (`a_live_remote_chat_survives_the_faults_that_define_trust`, #7188).

#![cfg(unix)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use purlis_core::runnerinstall::{self, Verified};
use purlis_core::sshroute::{self, Reach};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{self, Handshake, Identity, Linked, Liveness, Pinned};
use purlis_session_protocol::session::{Client, Launch, ProgramKind, StartProgram};

pub const PATIENCE: Duration = Duration::from_secs(90);
pub const PROJECT: &str = "01J9PRJCT00000000000000000";

/// `bytes` signed with a key of the test's own as `version` and verified as the desktop verifies
/// an asset.
pub fn verified(bytes: &[u8], version: &str, target: runnerinstall::Target) -> Verified {
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let keys = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let trusted = format!(
        "timestamp:1790077121\tfile:{}\tversion:{version}",
        runnerinstall::asset_name(version, target)
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
    runnerinstall::verify(&public, bytes, &signature, version, target).expect("verified")
}

/// The arguments that open the runner's bridge over the pinned route, and the folder holding the
/// pinned `known_hosts` for as long as it lives.
pub fn bridge_args(
    reach: &Reach,
    binary: &str,
) -> (purlis_core::dockerpool::settings::RunFolder, Vec<String>) {
    let (folder, known_hosts) = reach.known_hosts_for_a_run().expect("known_hosts");
    let mut args =
        sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts).expect("its arguments");
    args.pop();
    args.push(format!("{binary} bridge"));
    (folder, args)
}

/// A shell chat that runs `script` as `/bin/sh -c`.
pub fn shell_launch(script: String) -> Launch {
    Launch {
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script],
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
    }
}

/// What `command` printed on the runner, run there through the pinned route.
pub fn there(reach: &Reach, command: &str, input: Option<&str>) -> sshroute::Ran {
    let (_folder, known_hosts) = reach.known_hosts_for_a_run().expect("known_hosts");
    let mut args =
        sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts).expect("its arguments");
    args.pop();
    args.push(command.to_owned());
    sshroute::run_giving(&reach.ssh, &args, input, 256 * 1024).expect("ssh runs")
}

/// What `command` printed on the machine as its root, from outside it (what an operator at its
/// console sees): never through the account under test, so it signs nobody in.
pub fn as_root(machine: &str, command: &str) -> (bool, String) {
    let out = purlis_core::forklock::output(
        std::process::Command::new("docker").args(["exec", machine, "sh", "-c", command]),
    )
    .expect("docker runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// Waits until `check` holds, up to `patience`, saying `what` where it never does.
pub fn until_within(patience: Duration, what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + patience;
    while !check() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// A client of the host through its bridge over the pinned route, and the task keeping its link.
pub async fn linked(
    reach: &Reach,
    binary: &str,
    ours: Identity,
    runner: Pinned,
) -> (
    Client,
    purlis_core::dockerpool::settings::RunFolder,
    tokio::task::JoinHandle<()>,
) {
    let (folder, args) = bridge_args(reach, binary);
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

pub async fn chat(client: &Client, said: &str) -> String {
    client
        .start_program(StartProgram {
            launch: Some(shell_launch(format!("echo '{said}'; exec sleep 6000"))),
            ..StartProgram::of(PROJECT, ProgramKind::Shell)
        })
        .await
        .expect("the chat starts on the runner")
}

pub async fn running(client: &Client) -> Vec<String> {
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

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}
