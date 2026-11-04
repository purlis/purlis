// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat asks for a credential it lacks with the `secret_ask` tool or `purlis secret ask`,
//! and both send the app one and the same line** (#5072, ADR 0109). Codex and opencode use the
//! command; a harness with purlis's tools may use either.
//!
//! The app here is a listener that records each ask and answers it as the window's answer
//! would come back: filled, under the name the person stored it as. What is tested is the
//! chat's side, run as a harness runs it: the command's output and status, the tool's text,
//! and the line each sent.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::secrets::ask::Said;
use serde_json::{Value, json};

struct App {
    _reading: hookwire::Reading,
    socket: PathBuf,
    asked: Arc<Mutex<Vec<Ask>>>,
    token: hookwire::ChatToken,
}

/// An app with chat 7 open, answering every Secret ask with `said`.
fn an_app(within: &Path, said: Said) -> App {
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
                Ask::Secret(_) => Answer::Secret(said.clone()),
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

/// A project to run in.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

fn command(root: &Path, env: &[(String, String)], args: &[&str]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("CHARTER_ROOT", root)
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

/// The `secret_ask` tool, called once over `purlis mcp` as a harness calls it: its text.
fn tool(root: &Path, env: &[(String, String)], arguments: Value) -> String {
    use std::io::{BufRead, BufReader, Write};
    let mut child = support::purlis()
        .arg("mcp")
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("CHARTER_ROOT", root)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
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
        "params": {"name": "secret_ask", "arguments": arguments}}));
    let mut text = String::new();
    for _ in 0..2 {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let answer: Value = serde_json::from_str(&line).unwrap();
        if answer["id"] == 2 {
            text = answer["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    text
}

#[test]
fn the_tool_and_the_command_send_the_app_the_same_line_and_hear_the_final_name() {
    let project = project();
    let root = project.path();
    let app = an_app(
        root,
        Said::Filled {
            vault: "team".into(),
            key: "STRIPE_TEST_KEY".into(),
        },
    );
    let env = app.chat_env();

    let ran = command(
        root,
        &env,
        &[
            "secret",
            "ask",
            "STRIPE_KEY",
            "--purpose",
            "charge test cards in the checkout e2e",
        ],
    );
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let printed = String::from_utf8_lossy(&ran.stdout);
    assert!(printed.contains("filled team/STRIPE_TEST_KEY"), "{printed}");

    let told = tool(
        root,
        &env,
        json!({"key": "STRIPE_KEY", "purpose": "charge test cards in the checkout e2e"}),
    );
    assert_eq!(told.trim_end(), printed.trim_end());

    let asked = app.asked();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(asked[0], asked[1], "one line, whichever sent it");
    let Ask::Secret(line) = &asked[0] else {
        panic!("a secret ask: {asked:?}")
    };
    assert_eq!(
        (line.chat, line.key.as_str(), line.vault.as_deref()),
        (7, "STRIPE_KEY", None),
        "it names no persona and no vault: the app's record of the chat says whose"
    );
}

#[test]
fn a_decline_ends_the_command_with_a_failure_and_a_pending_ask_with_try_again_later() {
    let project = project();
    let root = project.path();
    for (said, code) in [
        (
            Said::Declined {
                vault: "team".into(),
                key: "K".into(),
                reason: "the person".into(),
            },
            1,
        ),
        (
            Said::Pending {
                vault: "team".into(),
                key: "K".into(),
            },
            75,
        ),
    ] {
        let app = an_app(root, said);
        let ran = command(
            root,
            &app.chat_env(),
            &["secret", "ask", "K", "--purpose", "p", "--vault", "team"],
        );
        assert_eq!(ran.status.code(), Some(code), "{ran:?}");
        let Ask::Secret(line) = &app.asked()[0] else {
            panic!("a secret ask")
        };
        assert_eq!(line.vault.as_deref(), Some("team"));
    }
}

#[test]
fn outside_a_chat_purlis_started_the_command_says_only_its_window_asks_and_sends_nothing() {
    let project = project();
    let ran = command(
        project.path(),
        &[],
        &["secret", "ask", "K", "--purpose", "p"],
    );
    assert_eq!(ran.status.code(), Some(1));
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(said.contains("purlis's window"), "{said}");
}
