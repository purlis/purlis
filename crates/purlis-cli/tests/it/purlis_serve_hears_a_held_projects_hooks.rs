// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve`, the runner image's session host, hears each held project's hooks as the
//! window's own host does (#2447): a chat it starts is issued its token on the project's hook
//! channel, and a line its hook writes there is pushed to the window that holds the project.
//!
//! It binds sockets and runs a program in a terminal, which a sandboxed agent cannot: it first
//! runs on CI. Linux only, as `purlis_serve_is_this_users_one_session_host` is.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::Duration;

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Client, Launch, ProgramKind, Pushed, StartProgram, Told,
};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PATIENCE: Duration = Duration::from_secs(30);

const CLAUDE_STOP: &str = r#"{"session_id":"11111111-2222-4333-8444-555555555555","cwd":"/tmp","hook_event_name":"Stop","stop_hook_active":false}"#;

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

/// A start the window composed: a shell whose Claude Code hook says its turn stopped, where its
/// start says its hooks report.
fn a_chat_whose_hook_says_it_stopped() -> StartProgram {
    StartProgram {
        launch: Some(Launch {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                "printf '%s' \"$1\" | \"$2\" hook stop; sleep 10".into(),
                "sh".into(),
                CLAUDE_STOP.into(),
                support::PURLIS.into(),
            ],
            cwd: None,
            set: Vec::new(),
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: None,
            git_hooks: None,
            reporting: None,
            confinement: None,
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

#[test]
fn a_line_a_chats_hook_writes_reaches_the_window_holding_its_project() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("charter.toml"), "").unwrap();
    let _host = serving(dir.path());
    // Where the host of this config home listens (#2114).
    let socket = purlis_core::hostsocket::here_for(
        Some(dir.path().join("run").as_os_str()),
        Some(&dir.path().join("config")),
    );
    let credential = Credential::read(
        &purlis_core::machine::charterd(&dir.path().join("config")),
        Scope::LocalUi,
    )
    .expect("the window's credential");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let stream = tokio::net::UnixStream::connect(&socket)
            .await
            .expect("the host's socket");
        let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
            .await
            .expect("admitted as the window");
        let (client, _views, mut events) = Client::new(link);
        client.subscribe(session::Since::new()).await.unwrap();
        client
            .hold_project(PROJECT, project.path().to_str().unwrap())
            .await
            .expect("held");

        let started = client
            .start_program_told(a_chat_whose_hook_says_it_stopped())
            .await
            .expect("it starts");
        let number = started.number.expect("its number");

        let line = loop {
            match tokio::time::timeout(PATIENCE, events.recv()).await {
                Ok(Some(Told::Pushed(Pushed::Heard { project, line, .. }))) => {
                    assert_eq!(project, PROJECT);
                    break line;
                }
                Ok(Some(_)) => {}
                other => panic!("no line of its hook was pushed: {other:?}"),
            }
        };
        assert_eq!(line["report"]["chat"], number, "{line}");
        assert_eq!(line["report"]["event"], "stop", "{line}");
    });
}
