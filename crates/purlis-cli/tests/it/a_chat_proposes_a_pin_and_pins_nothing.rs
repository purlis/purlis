// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat proposes a pin; it never pins one** (#5963, ADR 0122 §1, spec #5948 story 13).
//!
//! In a chat the app started, `purlis artifact pin` and the `artifact_propose_pin` tool each
//! hand the app one and the same line, a proposal, and write no pins file. Only the person's
//! press of Pin to workspace in the window pins it (the app's side, `pinproposals.rs`). With no
//! app to hand it to, nothing is pinned either: the chat is told to ask the person.
//!
//! The app here is a listener that records each ask and answers a proposal as the app does
//! once it holds one.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};

use purlis_core::artifact::proposal::Proposed;
use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, Listener, ProposePinAsk, SOCKET_ENV, TOKEN_ENV,
};
use purlis_core::repocmd::Say;
use serde_json::{Value, json};

/// What the app says once it holds a proposal.
const HELD: &str = "shown to the person in purlis's window";

struct App {
    _reading: hookwire::Reading,
    socket: PathBuf,
    asked: Arc<Mutex<Vec<Ask>>>,
    token: hookwire::ChatToken,
}

/// An app with chat 7 open, holding every pin it is proposed.
fn an_app(within: &Path) -> App {
    let socket = within.join("app").join("hooks.sock");
    let listener = Listener::bind(within, &socket).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let was_asked = Arc::clone(&asked);
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            was_asked.lock().unwrap().push(ask.clone());
            match ask {
                Ask::ProposePin(_) => Answer::Said {
                    lines: vec![Say::Done(HELD.into())],
                    code: 0,
                },
                _ => Answer::No {
                    why: "not asked here".to_owned(),
                },
            }
        }),
    );
    App {
        _reading: reading,
        socket,
        asked,
        token,
    }
}

impl App {
    fn chat_env(&self) -> Vec<(String, String)> {
        vec![
            (SOCKET_ENV.to_owned(), self.socket.display().to_string()),
            (CHAT_ENV.to_owned(), "7".to_owned()),
            (TOKEN_ENV.to_owned(), self.token.expose().to_owned()),
        ]
    }

    fn asked(&self) -> Vec<Ask> {
        self.asked.lock().unwrap().clone()
    }
}

/// A project with workspace alpha, whose `artifacts/state/` holds a page.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let page = dir.path().join("workspaces/alpha/artifacts/state");
    std::fs::create_dir_all(&page).unwrap();
    std::fs::write(page.join("index.html"), "<title>Backlog state</title>").unwrap();
    dir
}

fn pins_file(root: &Path) -> PathBuf {
    root.join("workspaces/alpha/artifacts/pins.toml")
}

fn base(root: &Path, env: &[(String, String)]) -> std::process::Command {
    let mut command = support::purlis();
    command
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("CHARTER_ROOT", root)
        .env("CHARTER_WORKSPACE", "alpha")
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    command
}

fn command(root: &Path, env: &[(String, String)], args: &[&str]) -> Output {
    base(root, env)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

/// The `artifact_propose_pin` tool, called once over `purlis mcp` as a harness calls it: its
/// text, and whether it is an error.
fn tool(root: &Path, env: &[(String, String)], arguments: Value) -> (String, bool) {
    use std::io::{BufRead, BufReader, Write};
    let mut child = base(root, env)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("purlis mcp starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut said = |message: Value| {
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    };
    said(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "test-harness", "version": "1"}}}),
    );
    said(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    said(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "artifact_propose_pin", "arguments": arguments}}));
    let mut answered = (String::new(), false);
    for _ in 0..2 {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let answer: Value = serde_json::from_str(&line).unwrap();
        if answer["id"] == 2 {
            answered = (
                answer["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                answer["result"]["isError"] == json!(true),
            );
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    answered
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn proposed(name: Option<&str>, command: Option<&str>) -> Ask {
    Ask::ProposePin(Box::new(ProposePinAsk {
        chat: 7,
        pin: Proposed {
            folder: "state".to_owned(),
            name: name.map(str::to_owned),
            icon: None,
            command: command.map(str::to_owned),
            cwd: None,
            skill: None,
        },
    }))
}

#[test]
fn purlis_artifact_pin_in_a_chat_proposes_the_pin_and_writes_no_pins_file() {
    let project = project();
    let app = an_app(project.path());
    let out = command(
        project.path(),
        &app.chat_env(),
        &[
            "artifact",
            "pin",
            "state",
            "--name",
            "Backlog",
            "--command",
            "purlis skill run backlog-sync",
        ],
    );
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains(HELD), "{}", said(&out));
    assert_eq!(
        app.asked(),
        vec![proposed(
            Some("Backlog"),
            Some("purlis skill run backlog-sync")
        )]
    );
    assert!(
        !pins_file(project.path()).exists(),
        "a chat's pin is a proposal: nothing is written until the person pins it"
    );
}

#[test]
fn the_tool_proposes_the_same_pin_as_the_command_and_writes_no_pins_file() {
    let project = project();
    let app = an_app(project.path());
    let (text, error) = tool(
        project.path(),
        &app.chat_env(),
        json!({"folder": "state", "name": "Backlog"}),
    );
    assert!(!error, "{text}");
    assert!(text.contains(HELD), "{text}");
    assert_eq!(app.asked(), vec![proposed(Some("Backlog"), None)]);
    assert!(!pins_file(project.path()).exists());
}

#[test]
fn a_chat_with_no_app_to_hand_its_pin_to_pins_nothing() {
    let project = project();
    let gone = vec![
        (
            SOCKET_ENV.to_owned(),
            project.path().join("no-app.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "7".to_owned()),
        (TOKEN_ENV.to_owned(), "not-a-token".to_owned()),
    ];
    let out = command(project.path(), &gone, &["artifact", "pin", "state"]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("ask the person"), "{}", said(&out));
    let (text, error) = tool(project.path(), &gone, json!({"folder": "state"}));
    assert!(error, "{text}");
    assert!(!pins_file(project.path()).exists());
}
