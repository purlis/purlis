// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A hook that runs inside the chat's sandbox leaves purlis's state alone** (#7178).
//!
//! Where a harness's hooks run inside the chat's sandbox (Codex, opencode), the sandbox denies
//! the hook spool to it. Writing a line there anyway would make the sandbox refuse purlis's own
//! hook, and that refusal would be drafted as a Report of a purlis bug. So such a hook does not try:
//! the line is lost, as the spool's own words say. A Claude Code hook runs outside the sandbox and
//! still spools.

use crate::support;

use std::process::Stdio;

use purlis_core::hookwire::{CHAT_ENV, Listener, SANDBOXED_ENV, SOCKET_ENV, TOKEN_ENV, spool};

const STOP: &str = r#"{"session_id":"11111111-2222-4333-8444-555555555555","cwd":"/tmp","hook_event_name":"Stop","stop_hook_active":false}"#;

/// Runs `purlis hook stop` for chat 7 under `harness`, sandboxed or not, with the app gone, and
/// answers whether anything was spooled for the chat.
fn spooled_by(harness: &str, sandboxed: &str) -> bool {
    spooled_by_with(harness, sandboxed, &[])
}

/// [`spooled_by`], with `more` set in the hook's environment as well.
fn spooled_by_with(harness: &str, sandboxed: &str, more: &[(&str, &str)]) -> bool {
    let dir = tempfile::tempdir().expect("a directory");
    let path = support::hooks_socket(dir.path());
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    // The app has gone: the socket is not there, and the key its token checks under is.
    drop(listener);
    let mut child = support::command(support::PURLIS)
        .args(["hook", "stop"])
        .current_dir(std::env::temp_dir())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .env("CHARTER_HARNESS", harness)
        .env(SANDBOXED_ENV, sandboxed)
        .envs(more.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("purlis runs");
    stand_in::feed(&mut child, STOP.as_bytes());
    child.wait().expect("purlis finishes");
    let spool = spool::dir_for(&path).join("7");
    std::fs::read_dir(spool).is_ok_and(|mut lines| lines.next().is_some())
}

#[test]
fn a_hook_inside_the_chats_sandbox_spools_nothing() {
    for harness in ["codex", "opencode"] {
        assert!(
            !spooled_by(harness, "1"),
            "{harness}: its hook wrote where the sandbox denies it"
        );
    }
}

#[test]
fn a_hook_outside_the_chats_sandbox_still_spools() {
    // Claude Code's hooks run outside its sandbox, and a chat with none has no spool denied.
    assert!(spooled_by("claude-code", "1"), "a Claude Code hook spools");
    assert!(spooled_by("codex", "0"), "a chat with no sandbox spools");
}

#[test]
fn a_hook_of_a_chat_purlis_wraps_whole_spools_nothing() {
    // The chat's line says it is wrapped whole, so its Claude Code hooks are inside the wrap too.
    let wrapped = [(purlis_core::hookwire::WRAPPED_ENV, "1")];
    assert!(!spooled_by_with("claude-code", "1", &wrapped));
}
