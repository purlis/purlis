// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A `Stop` hook hands the app the last message its turn ended on (#1981), on a line of its
//! own sent before the turn's report, so the app has it as it hears the turn end. What a task
//! that ends its turns without a report is reported from, as **a report purlis made from its
//! last message**. Never on the report's own line, which a spool keeps when the app is away.
//!
//! One adapter test per harness: Claude Code's and Codex's `last_assistant_message`, which
//! purlis's opencode plugin sends in the same field.

use crate::support;

use std::process::Stdio;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{
    CHAT_ENV, Hearing, Listener, Output, Report, SOCKET_ENV, TOKEN_ENV, TurnOutput, spool,
};

/// What the host heard of one `purlis hook stop` run as chat 7 with `payload`, in the order
/// it heard them: `Ok` a turn's output, `Err` a report.
fn stop(payload: &serde_json::Value) -> Vec<Result<TurnOutput, Report>> {
    let dir = tempfile::tempdir().expect("a directory");
    let path = support::hooks_socket(dir.path());
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, rx) = mpsc::channel();
    let reports = Mutex::new(tx.clone());
    let outputs = Mutex::new(tx);
    let _reading = listener.hear(Hearing {
        serve: purlis_core::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| purlis_core::secrets::brokered::not_answered(writer)),
        each: Box::new(move |report| {
            let _ = reports.lock().unwrap().send(Err(report));
            Ok(())
        }),
        answer: Box::new(|_, _| panic!("no ask")),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(|_| Ok(())),
        blocked: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(move |output| {
            let _ = outputs.lock().unwrap().send(Ok(output));
        }),
        permission: Box::new(|_| None),
    });
    let mut child = support::purlis()
        .args(["hook", "stop"])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("purlis runs");
    stand_in::feed(&mut child, payload.to_string().as_bytes());
    child.wait_with_output().expect("purlis finishes");
    // The output's line is told and not waited on, so it is waited for here.
    let mut heard: Vec<_> =
        std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(2)).ok()).collect();
    // The report is recorded before the hook exits, or spooled where the host was slower than
    // the hook waits (`support::told`).
    heard.extend(support::told(&path, &rx, |line| match line {
        spool::Spooled::Report(report) => Some(Err(report)),
        _ => None,
    }));
    support::distinct(heard)
}

#[test]
fn a_turn_s_last_message_reaches_the_app_before_the_turn_s_end() {
    let heard = stop(&serde_json::json!({
        "session_id": "s",
        "hook_event_name": "Stop",
        "stop_hook_active": false,
        "last_assistant_message": "Checked the logs: 3 errors, all from the cache.",
    }));
    assert_eq!(heard.len(), 2, "{heard:?}");
    let Ok(output) = &heard[0] else {
        panic!("the output came first: {heard:?}");
    };
    assert_eq!(output.chat, 7);
    assert_eq!(
        output.turn_output,
        Output::Final {
            text: "Checked the logs: 3 errors, all from the cache.".to_owned()
        }
    );
    let Err(report) = &heard[1] else {
        panic!("then the report: {heard:?}");
    };
    let line = serde_json::to_string(report).unwrap();
    assert!(
        !line.contains("3 errors"),
        "the report's own line carries none of it: {line}"
    );
}

#[test]
fn a_turn_with_no_last_message_sends_none() {
    for payload in [
        serde_json::json!({"session_id": "s", "hook_event_name": "Stop"}),
        serde_json::json!({"session_id": "s", "hook_event_name": "Stop",
            "last_assistant_message": ""}),
    ] {
        let heard = stop(&payload);
        assert!(
            heard.iter().all(Result::is_err),
            "only the report: {heard:?}"
        );
    }
}
