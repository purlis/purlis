// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat attended by its start, reporting `bypassPermissions` anywhere but its armed hook, is
//! attended** (T1-22, #6413; R-109, R-111, R-112; Q271, Q276), through the real hooks of a chat
//! the app started: A4's (`pretooluse`), the briefing's (`sessionstart`) and the commit gate's
//! (`userpromptsubmit`). Each asks the app for the fact it holds, and none reads the payload's
//! mode in its place.
//!
//! **The fixture chat** is a Codex chat a person started: its fact is attended (R-289), and
//! every one of its payloads says `bypassPermissions`, which Codex names for every chat and
//! which no armed hook of a harness purlis measures has said. A Claude Code line's own bypass
//! bit still lowers A4's answer for that line (Q276); that is pinned in
//! `a_publish_is_refused_unless_the_app_says_its_chat_is_attended.rs`.
//!
//! **Each fails closed**: where no app answers a chat it started, the chat is unattended,
//! whatever its payload says. Dispatch's reading of the same chat is in
//! `app/src-tauri/src/handoff/tests.rs`, where the fact is held.
#![cfg(unix)]

use crate::support;
use crate::support::attending::AttendingApp;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use purlis_core::hookwire::{CHAT_ENV, SOCKET_ENV};

/// The chat the app started.
const CHAT: u32 = 6;

/// What every payload of the fixture chat says of its mode.
const ITS_PAYLOADS_SAY: &str = "bypassPermissions";

/// A publish A4 refuses an unattended chat.
const RELEASE: &str = "gh release create v1.0.0";

/// What A4's refusal says.
const A4_SAYS: &str = "Publishing is on purlis's floor";

/// What the briefing tells a chat nobody is at, with no workspace (ADR 0046's recorded words).
const THE_STOP_SAYS: &str = "this run has no workspace and nobody to ask";

/// A prompt with a fork in it, on a session's first prompt: the commit gate fires on it in an
/// attended chat.
const FORKED: &str = "refactor the importer so it is maybe cleaner across every repo";

/// What the commit gate says.
const THE_GATE_SAYS: &str = "Commitment point";

/// A plane with two workspaces and none pinned, so the briefing's workspace gate speaks, and a
/// home that is not the operator's.
struct Plane {
    _tmp: tempfile::TempDir,
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
}

impl Plane {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let home = base.join("home");
        std::fs::create_dir_all(&home).expect("the home");
        for ws in ["alpha", "beta"] {
            let at = root.join("workspaces").join(ws);
            std::fs::create_dir_all(&at).expect("a workspace");
            std::fs::write(at.join("workspace.md"), format!("# {ws}\n")).expect("its charter");
        }
        std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the marker");
        Self {
            _tmp: tmp,
            base,
            root,
            home,
        }
    }

    /// The app that started chat [`CHAT`] here, saying it is attended where `attended`.
    fn app(&self, attended: bool) -> AttendingApp {
        AttendingApp::answering(&self.base, CHAT, attended)
    }

    /// What chat [`CHAT`] runs with where its app has gone: its number and a socket nothing
    /// listens on.
    fn without_an_app(&self) -> Vec<(String, String)> {
        vec![
            (
                SOCKET_ENV.to_owned(),
                self.base
                    .join("gone")
                    .join("hooks.sock")
                    .display()
                    .to_string(),
            ),
            (CHAT_ENV.to_owned(), CHAT.to_string()),
        ]
    }

    /// `purlis hook <word>` of a Codex chat, run in `cwd` with `payload` on stdin and `chat`
    /// set beside the plain variables: its stdout.
    fn hook(&self, word: &str, cwd: &Path, chat: &[(String, String)], payload: &str) -> String {
        let mut child = support::purlis()
            .args(["hook", word])
            .current_dir(cwd)
            .env_clear()
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("CHARTER_HARNESS", "codex")
            .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
            .env("PATH", "/usr/bin:/bin")
            .envs(chat.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the hook runs");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("the payload");
        let out = child.wait_with_output().expect("the hook finishes");
        assert_eq!(
            out.status.code(),
            Some(0),
            "{word}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("UTF-8")
    }

    /// A4's answer to [`RELEASE`]: its stdout.
    fn publish(&self, chat: &[(String, String)]) -> String {
        let payload = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "permission_mode": ITS_PAYLOADS_SAY,
            "tool_input": {"command": RELEASE},
        });
        self.hook("pretooluse", &self.root, chat, &payload.to_string())
    }

    /// The chat's briefing, from outside the plane as a run with no workspace starts: its
    /// stdout.
    fn briefing(&self, chat: &[(String, String)], mode: &str) -> String {
        let payload = serde_json::json!({
            "session_id": "fixture-session-1",
            "cwd": ".",
            "hook_event_name": "SessionStart",
            "permission_mode": mode,
        });
        self.hook("sessionstart", &self.home, chat, &payload.to_string())
    }

    /// What the chat's first prompt, [`FORKED`], is told: its stdout.
    fn first_prompt(&self, chat: &[(String, String)], mode: &str) -> String {
        let payload = serde_json::json!({
            "session_id": "first-prompt",
            "hook_event_name": "UserPromptSubmit",
            "prompt": FORKED,
            "permission_mode": mode,
        });
        self.hook("userpromptsubmit", &self.root, chat, &payload.to_string())
    }
}

/// **R-109**: A4 lets the fixture chat's publish through, its payload saying the bypass, because
/// the app says it is attended.
#[test]
fn a4_treats_a_chat_reporting_bypass_outside_its_armed_hook_as_attended() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let app = plane.app(true);
    let said = plane.publish(&app.chat_env());
    assert!(
        !said.contains(A4_SAYS),
        "an attended chat was refused: {said}"
    );
    assert_eq!(app.attendance_asked(), 1, "the app was asked");
}

/// **R-111**: the fixture chat's briefing is the attended one: it is never told to stop for
/// want of anyone to ask, and the hook asked the app.
///
/// Red before T1-22: the briefing's hook read the payload's mode as the one-way mark did, so a
/// Codex chat a person was at was told to stop.
#[test]
fn the_briefing_treats_a_chat_reporting_bypass_outside_its_armed_hook_as_attended() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let app = plane.app(true);
    let said = plane.briefing(&app.chat_env(), ITS_PAYLOADS_SAY);
    assert!(!said.is_empty(), "the chat is briefed");
    assert!(!said.contains(THE_STOP_SAYS), "{said}");
    assert!(app.attendance_asked() >= 1, "the app was asked");
}

/// **R-112**: the fixture chat's first forked prompt is told to scout and ask, as an attended
/// chat's is.
///
/// Red before T1-22: the commit gate's caller handed it the one-way mark heard from the payload,
/// so the gate was silent.
#[test]
fn the_commit_gate_treats_a_chat_reporting_bypass_outside_its_armed_hook_as_attended() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let app = plane.app(true);
    let said = plane.first_prompt(&app.chat_env(), ITS_PAYLOADS_SAY);
    assert!(said.contains(THE_GATE_SAYS), "{said}");
    assert_eq!(app.attendance_asked(), 1, "the app was asked");
}

/// **The app's word is the fact**: where it says nobody is at the chat, the briefing tells it to
/// stop and the gate stays silent, though its payload names a mode that asks.
///
/// Red before T1-22 for both: each read the payload alone, which says `default`.
#[test]
fn the_briefing_and_the_commit_gate_read_a_chat_the_app_says_nobody_is_at_as_unattended() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let app = plane.app(false);
    let briefed = plane.briefing(&app.chat_env(), "default");
    assert!(briefed.contains(THE_STOP_SAYS), "{briefed}");
    let prompted = plane.first_prompt(&app.chat_env(), "default");
    assert!(!prompted.contains(THE_GATE_SAYS), "{prompted}");
}

/// **Fails closed**: where no app answers a chat it started, A4, the briefing and the commit gate
/// each take it for unattended, though its payload names a mode that asks.
///
/// Red before T1-22 for the briefing and the gate, which read the payload alone.
#[test]
fn a_chat_whose_app_does_not_answer_is_unattended_to_every_hook() {
    purlis_core::unsteered!();
    let plane = Plane::new();
    let gone = plane.without_an_app();

    let refused = plane.hook(
        "pretooluse",
        &plane.root,
        &gone,
        &serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "permission_mode": "default",
            "tool_input": {"command": RELEASE},
        })
        .to_string(),
    );
    assert!(refused.contains(A4_SAYS), "{refused}");
    let briefed = plane.briefing(&gone, "default");
    assert!(briefed.contains(THE_STOP_SAYS), "{briefed}");
    let prompted = plane.first_prompt(&gone, "default");
    assert!(!prompted.contains(THE_GATE_SAYS), "{prompted}");
}
