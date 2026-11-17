// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve` hears `SIGTERM` as a quit (#6700, ADR 0068 §3 as amended for #6700): systemd
//! and launchd stop a runner's host by it, at a shutdown, a stop or `purlis serve --remove`. The
//! host ends each chat as a quit ends it (its run ended `stopped`, for `quit`), empties its host
//! record and exits 0, saying so. A host killed outright (`SIGKILL`, a power loss) still leaves
//! its chats for the next host, which settles each as ended by its crash.
//!
//! It binds sockets and runs a program in a terminal, which a sandboxed agent cannot: it first
//! runs on CI. Linux only, as `purlis_serve_is_this_users_one_session_host` is.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use purlis_core::eventlog::{Attributes, Began, Entry};
use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Client, Command, Launch, ProgramKind, StartProgram,
};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(30);
const CHAT: &str = "01K6W0RKER000000000000000C";
const RUN: &str = "01K6W0RKRUN00000000000000C";

/// Ends the host this test started, by its pid, however the test ends.
struct Ends(Child);

impl Drop for Ends {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `purlis serve` with its config, data and runtime homes in `dir`, once it says it is ready.
fn serving(dir: &Path) -> Ends {
    for folder in ["run", "config", "data"] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
    }
    std::fs::set_permissions(dir.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut serve = support::purlis();
    serve
        .arg("serve")
        .env("PURLIS_CONFIG_HOME", dir.join("config"))
        .env("PURLIS_DATA_HOME", dir.join("data"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("TMPDIR", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut host = Ends(serve.spawn().expect("purlis serve starts"));
    let mut line = String::new();
    BufReader::new(host.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    if line.trim_end() != "ready" {
        let _ = host.0.kill();
        let mut said = String::new();
        let _ = host.0.stderr.take().unwrap().read_to_string(&mut said);
        panic!("the host did not say it was ready: {line:?}\n{said}");
    }
    host
}

/// A chat that runs until it is ended.
fn a_chat_that_runs_on() -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "while :; do sleep 1; done".into()],
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
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

/// Every event the host's log holds, in the one device folder it writes under `data`.
fn the_hosts_events(data: &Path) -> Vec<purlis_core::eventlog::Event> {
    let Ok(devices) = std::fs::read_dir(data.join(purlis_core::eventlog::EVENTS)) else {
        return Vec::new();
    };
    devices
        .flatten()
        .filter(|device| device.path().is_dir())
        .flat_map(|device| purlis_core::eventlog::read(&device.path()).unwrap_or_default())
        .collect()
}

/// How the chat's run ended, as the host's log has it: its state and cause.
fn how_the_run_ended(data: &Path) -> Option<(String, String)> {
    the_hosts_events(data)
        .into_iter()
        .find(|event| event.kind == "run.ended" && event.run.as_deref() == Some(RUN))
        .map(|event| {
            (
                event.body["state"].as_str().unwrap_or_default().to_owned(),
                event.body["cause"].as_str().unwrap_or_default().to_owned(),
            )
        })
}

/// The host record, in the machine store of the config home in `dir`.
fn the_host_record(dir: &Path) -> PathBuf {
    purlis_core::machine::charterd(&dir.join("config")).join(purlis_host::HOST_RECORD)
}

fn a_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Holds the project at `root` on the host serving with its homes in `dir`, and starts the chat
/// in it, its run begun in the host's log as the window begins it.
fn a_chat_running_on(dir: &Path, root: &Path) {
    a_runtime().block_on(async {
        let socket = purlis_core::hostsocket::here_for(
            Some(dir.join("run").as_os_str()),
            Some(&dir.join("config")),
        );
        let credential = Credential::read(
            &purlis_core::machine::charterd(&dir.join("config")),
            Scope::LocalUi,
        )
        .expect("the window's credential");
        let stream = tokio::net::UnixStream::connect(&socket)
            .await
            .expect("the host's socket");
        let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
            .await
            .expect("admitted as the window");
        let (client, _views, _events): (Client, _, _) = Client::new(link);
        client
            .hold_project(PROJECT, root.to_str().unwrap())
            .await
            .expect("held");
        let number = client
            .start_program_told(a_chat_that_runs_on())
            .await
            .expect("it starts")
            .number
            .expect("its number");
        let begin = serde_json::to_value(Entry::Begin {
            plane: root.to_path_buf(),
            number,
            chat: CHAT.into(),
            run: RUN.into(),
            cause: Began::Start,
            attributes: Attributes::default(),
            held: false,
            trust: None,
        })
        .unwrap();
        match client.call(Command::Record { entry: begin }).await {
            Ok(Answer::Done) => {}
            other => panic!("the host did not record it: {other:?}"),
        }
    });
}

fn a_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("charter.toml"), "").unwrap();
    project
}

#[test]
fn sigterm_ends_each_chat_as_a_quit_empties_the_host_record_and_exits_0() {
    let (dir, project) = (tempfile::tempdir().unwrap(), a_project());
    let mut host = serving(dir.path());
    a_chat_running_on(dir.path(), project.path());

    // As systemd's stop and launchd's bootout send it.
    let sent = std::process::Command::new("kill")
        .args(["-TERM", &host.0.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(sent.success(), "SIGTERM was sent");

    let by = Instant::now() + PATIENCE;
    let exited = loop {
        if let Some(exited) = host.0.try_wait().unwrap() {
            break exited;
        }
        assert!(Instant::now() < by, "the host exited within {PATIENCE:?}");
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut said = String::new();
    let _ = host.0.stderr.take().unwrap().read_to_string(&mut said);
    assert_eq!(exited.code(), Some(0), "it exited 0, as a quit: {said}");
    assert!(said.contains(purlis_host::TOLD_TO_STOP), "{said}");
    assert_eq!(
        how_the_run_ended(&dir.path().join("data")),
        Some(("stopped".to_owned(), "quit".to_owned())),
        "the chat's run ended as a quit ends it: {said}"
    );
    let record = std::fs::read(the_host_record(dir.path())).unwrap_or_default();
    assert!(
        record.is_empty(),
        "nothing is left for the next host to settle: {}",
        String::from_utf8_lossy(&record)
    );
}

#[test]
fn a_host_killed_outright_leaves_its_chats_for_the_next_to_settle_as_a_crash() {
    let (dir, project) = (tempfile::tempdir().unwrap(), a_project());
    let host = serving(dir.path());
    a_chat_running_on(dir.path(), project.path());

    drop(host);
    let left = std::fs::read_to_string(the_host_record(dir.path())).unwrap_or_default();
    assert!(
        left.contains("\"started\""),
        "a host killed outright leaves its chat's start in its record: {left}"
    );
    // The next host settles what was left before it says it is ready.
    let mut next = serving(dir.path());
    let _ = next.0.kill();
    let _ = next.0.wait();
    let mut said = String::new();
    let _ = next.0.stderr.take().unwrap().read_to_string(&mut said);
    assert!(
        said.contains(
            "ended without ending 1 of its programs; each is settled as ended by its crash"
        ),
        "{said}"
    );
    let record = std::fs::read(the_host_record(dir.path())).unwrap_or_default();
    assert!(record.is_empty(), "the record was taken over");
}
