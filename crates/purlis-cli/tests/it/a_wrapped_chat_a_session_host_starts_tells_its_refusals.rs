// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A wrapped chat that a session host starts tells the window what its wrap refused** (#2426).
//!
//! The wrap's supervisor reads the hook socket, the chat's number, its token and its folder from
//! the environment the chat is started with. Here that environment is the session host's
//! composition (`purlis serve`, the runner image's host, which a container or a pod runs), and
//! the socket is the host's own hook channel for the project. The refusal is pushed to the
//! window holding the project as the block macOS's kernel line of it would be.
//!
//! It binds sockets, runs a program in a terminal and needs Linux's Landlock, which a sandboxed
//! agent cannot: it first runs on CI, and in a container wherever Docker runs it.

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

const PROJECT: &str = "01J9PRJCT00000000000000001";
const PATIENCE: Duration = Duration::from_secs(30);

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

/// The profile of a chat in `chat` whose hooks report at `socket`: it reads anything, writes its
/// own folder alone, and reaches only that socket.
fn profile(chat: &Path, socket: &Path) -> String {
    format!(
        "(version 1)\n(deny default)\n(allow process-exec)\n(allow file-read*)\n\
         (allow network-outbound (remote unix-socket (path-literal \"{}\")))\n\
         (allow file-write* (subpath \"{}\") (literal \"/dev/null\"))\n",
        socket.display(),
        chat.display()
    )
}

/// A start the window composed: a Codex chat on purlis's own wrap, in `chat`, whose shell tries
/// to write `elsewhere`, started sandboxed.
fn a_wrapped_chat_writing(chat: &Path, socket: &Path, elsewhere: &Path) -> StartProgram {
    let script = format!("touch {} 2>/dev/null; sleep 10", elsewhere.display());
    StartProgram {
        launch: Some(Launch {
            program: support::PURLIS.into(),
            args: purlis_linux_sandbox::line(
                &profile(chat, socket),
                "/bin/sh".to_owned(),
                vec!["-c".to_owned(), script],
            ),
            cwd: Some(chat.display().to_string()),
            // What the window's composition sets for a sandboxed chat of its harness.
            set: vec![
                (
                    purlis_core::hookwire::SANDBOXED_ENV.to_owned(),
                    "1".to_owned(),
                ),
                (
                    purlis_core::hookwire::HARNESS_ENV.to_owned(),
                    "codex".to_owned(),
                ),
                (
                    purlis_core::sandboxblock::CHAT_DIR_ENV.to_owned(),
                    chat.display().to_string(),
                ),
            ],
            strip: Vec::new(),
            pass: Vec::new(),
            declared: Vec::new(),
            harness: Some("codex".to_owned()),
            git_hooks: None,
            reporting: None,
            confinement: None,
            prompt_key: None,
        }),
        ..StartProgram::of(PROJECT, ProgramKind::Harness)
    }
}

#[test]
fn a_write_the_wrap_of_a_chat_the_host_started_refuses_reaches_the_window_as_its_block() {
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let dir = tempfile::tempdir().unwrap();
    // Inside the host's own temporary folder, which a test build holds every plane it resolves
    // to.
    let root = dir.path().canonicalize().unwrap().join("project");
    let chat = root.join("workspaces/a/repo");
    std::fs::create_dir_all(&chat).unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(root.join("workspaces/b")).unwrap();
    let elsewhere = root.join("workspaces/b/notes.md");
    // Where the host binds this project's hook channel, as the window's own does.
    let socket = purlis_core::hookchannel::socket_for(Some(&root)).socket;
    let _host = serving(dir.path());
    let at = purlis_core::hostsocket::here_for(
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
        let stream = tokio::net::UnixStream::connect(&at)
            .await
            .expect("the host's socket");
        let link = link::connect(stream, session::speaks(), Scope::LocalUi, &credential)
            .await
            .expect("admitted as the window");
        let (client, _views, mut events) = Client::new(link);
        client.subscribe(session::Since::new()).await.unwrap();
        client
            .hold_project(PROJECT, root.to_str().unwrap())
            .await
            .expect("held");

        let started = client
            .start_program_told(a_wrapped_chat_writing(&chat, &socket, &elsewhere))
            .await
            .expect("it starts");
        let number = started.number.expect("its number");

        let line = loop {
            match tokio::time::timeout(PATIENCE, events.recv()).await {
                // A block, as the host hands one on (`hookwire::relay::Heard::Blocked`).
                Ok(Some(Told::Pushed(Pushed::Heard { project, line, .. })))
                    if line.get("blocked").is_some() =>
                {
                    assert_eq!(project, PROJECT);
                    break line["blocked"].clone();
                }
                Ok(Some(_)) => {}
                other => panic!("no block of the chat's wrap was pushed: {other:?}"),
            }
        };
        assert_eq!(line["chat"], number, "{line}");
        assert_eq!(line["sandbox_blocked"]["operation"], "write", "{line}");
        assert_eq!(line["sandbox_blocked"]["kind"], "project-files", "{line}");
        assert_eq!(line["sandbox_blocked"]["ours"], false, "{line}");
        assert_eq!(line["harness"], "codex", "{line}");
        assert_eq!(
            line["target"],
            elsewhere.display().to_string(),
            "what Allow would name: {line}"
        );
    });
    assert!(!elsewhere.exists(), "the write stays refused");
}
