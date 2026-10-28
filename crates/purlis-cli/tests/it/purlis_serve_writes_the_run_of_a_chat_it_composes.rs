// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis serve`, the runner image's session host, writes the run of a chat it composes by its
//! names in this machine's event log, under the chat and run the window named (#2446, ADR 0066).
//! The window writes no run of a chat on a runner: the runner writes it, where it runs.
//!
//! It binds a socket, which a sandboxed agent cannot: it first runs on CI. Linux only, as
//! `purlis_serve_is_this_users_one_session_host` is.

#![cfg(target_os = "linux")]

use crate::support;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use purlis_session_protocol::auth::{Credential, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client, ProgramKind, StartProgram};

const PROJECT: &str = "01J9PRJCT00000000000000000";
const CHAT: &str = "01J9CHAT000000000000000000";
const RUN: &str = "01J9RUN0000000000000000000";

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

/// The project at `root` has the persona `steward` and the profile `work`, of Claude Code,
/// running a stand-in that waits for a line, as the person approved it.
fn a_profile(root: &Path, bin: &Path) {
    let program = stand_in::claude_code(bin, "claude", "#!/bin/sh\nread line\n");
    let persona = root.join("personas").join("steward");
    std::fs::create_dir_all(&persona).unwrap();
    std::fs::write(persona.join("persona.md"), "# steward\n").unwrap();
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{:?}]\n",
            program.display().to_string()
        ),
    )
    .unwrap();
    let declared = purlis_core::profiles::current(root);
    purlis_core::profiletrust::record_launched(
        root,
        "work",
        &purlis_core::profiletrust::fingerprint(declared.get("work").unwrap()),
    )
    .unwrap();
}

/// Every event the host wrote in its data home, whichever device it named itself.
fn the_hosts_events(data: &Path) -> Vec<purlis_core::eventlog::Event> {
    let Ok(devices) = std::fs::read_dir(data.join(purlis_core::eventlog::EVENTS)) else {
        return Vec::new();
    };
    devices
        .flatten()
        .flat_map(|device| purlis_core::eventlog::read(&device.path()).unwrap_or_default())
        .collect()
}

#[test]
fn purlis_serve_writes_the_run_of_a_chat_it_composes_under_the_windows_ids() {
    let dir = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join(purlis_core::plane::MANIFEST), "").unwrap();
    a_profile(project.path(), bin.path());
    let _host = serving(dir.path());
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
        let (client, _views, _events) = Client::new(link);
        client
            .hold_project(PROJECT, project.path().to_str().unwrap())
            .await
            .expect("held");
        client
            .start_program(StartProgram {
                program: Some(CHAT.into()),
                run: Some(RUN.into()),
                profile: Some("work".into()),
                persona: Some("steward".into()),
                // Out of the sandbox, as `purlis serve` has no plugin to carry one to a Claude
                // Code chat (#2490).
                without_sandbox: true,
                ..StartProgram::of(PROJECT, ProgramKind::Harness)
            })
            .await
            .expect("composed and started");
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let events = the_hosts_events(&dir.path().join("data"));
        if events.iter().any(|event| {
            event.kind == "run.started"
                && event.chat.as_deref() == Some(CHAT)
                && event.run.as_deref() == Some(RUN)
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "no run was written: {events:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}
