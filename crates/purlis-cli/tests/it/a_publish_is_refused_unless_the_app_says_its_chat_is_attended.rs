// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A4 asks the app whether its chat is attended** (T1-18, #6409; R-109, R-145; Q275), through
//! the real `purlis hook pretooluse` of a chat the app started. The hook asks the app of a line
//! A4 would refuse unattended, and takes the chat for attended only where the app says so: no
//! app, and an app that refuses the ask, both read as unattended. A line's own bypass bit lowers
//! the answer for that line whatever the app says. A merge the hold stands for is the hold's,
//! never A4's, in the bypass too.
//!
//! The app here is a listener that answers the attendance ask as a test sets it, and a merge
//! ask as the hold's test app confirms one. What the app holds and how it answers are in
//! `app/src-tauri/src/handoff/tests.rs` and `app/src-tauri/src/chats/tests.rs`.
#![cfg(unix)]

use crate::support;

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, Hearing, Listener, SOCKET_ENV, TOKEN_ENV, ToolCall,
};
use purlis_core::mergehold;
use purlis_core::toolgate::REASON_RELEASE_FLOOR;

const CHAT: u32 = 4;

/// A publish A4 refuses an unattended chat.
const RELEASE: &str = "gh release create v1.0.0";

/// What A4's refusal says.
const A4_SAYS: &str = "Publishing is on purlis's floor";

/// A plane, a home that is not the operator's, and the app that started chat [`CHAT`] in it,
/// or none.
struct Chat {
    _tmp: tempfile::TempDir,
    _reading: Option<hookwire::Reading>,
    root: PathBuf,
    home: PathBuf,
    socket: PathBuf,
    token: Option<hookwire::ChatToken>,
    /// What the app answers the attendance ask.
    attendance: Arc<Mutex<Answer>>,
    asked: Arc<Mutex<Vec<Ask>>>,
    tools: Option<Mutex<mpsc::Receiver<ToolCall>>>,
}

impl Chat {
    /// The plane and home of a chat, under a fresh folder.
    fn folders() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let home = base.join("home");
        std::fs::create_dir_all(&root).expect("the plane");
        std::fs::create_dir_all(&home).expect("the home");
        std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the marker");
        (tmp, base, root, home)
    }

    /// A chat whose app answers the attendance ask with `attendance`.
    fn answered(attendance: Answer) -> Self {
        let (tmp, base, root, home) = Self::folders();
        let socket = base.join("app").join("hooks.sock");
        let listener = Listener::bind(&base, &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
        let token = listener
            .tokens()
            .issue_to_this_process(CHAT)
            .expect("a token");
        let attendance = Arc::new(Mutex::new(attendance));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let (answers, was_asked) = (Arc::clone(&attendance), Arc::clone(&asked));
        let (tool, tools) = mpsc::channel();
        let tool = Mutex::new(tool);
        let reading = listener.hear(Hearing {
            serve: purlis_core::hookwire::serve::refusing(),
            secret_exec: Box::new(|_, _, writer| {
                purlis_core::secrets::brokered::not_answered(writer)
            }),
            each: Box::new(|_| Ok(())),
            noticed: Box::new(|_| {}),
            saved: Box::new(|_| {}),
            refused: Box::new(|_| Ok(())),
            tool: Box::new(move |call| {
                let _ = tool.lock().unwrap().send(call);
                Ok(())
            }),
            blocked: Box::new(|_| {}),
            doing: Box::new(|_| {}),
            output: Box::new(|_| {}),
            touching: Box::new(|_| {}),
            permission: Box::new(|_| None),
            answer: Box::new(move |_, ask| {
                was_asked.lock().unwrap().push(ask.clone());
                match ask {
                    Ask::Attendance(_) => answers.lock().unwrap().clone(),
                    Ask::Merge(merge) => Answer::MergeHeld {
                        said: mergehold::handed_said(&merge.request),
                    },
                    _ => Answer::No {
                        why: "not asked here".to_owned(),
                    },
                }
            }),
            reason: Box::new(|_| {}),
        });
        Self {
            _tmp: tmp,
            _reading: Some(reading),
            root,
            home,
            socket,
            token: Some(token),
            attendance,
            asked,
            tools: Some(Mutex::new(tools)),
        }
    }

    /// A chat whose app does not answer: its socket is not there.
    fn without_an_app() -> Self {
        let (tmp, base, root, home) = Self::folders();
        Self {
            _tmp: tmp,
            _reading: None,
            root,
            home,
            socket: base.join("app").join("hooks.sock"),
            token: None,
            attendance: Arc::new(Mutex::new(Answer::Attended { attended: true })),
            asked: Arc::default(),
            tools: None,
        }
    }

    /// The guard rule the hook's deny line of the last call named, as the app's event log
    /// reads it.
    fn rule(&self) -> Option<String> {
        let tools = self.tools.as_ref()?.lock().unwrap();
        loop {
            let call = tools.recv_timeout(Duration::from_secs(5)).ok()?;
            if call.tool_hook == "pretooluse" {
                return call.rule;
            }
        }
    }

    /// The `permissionDecisionReason` the chat's hook printed for `command` in `mode`, or
    /// `None` where it did not deny.
    fn denial(&self, command: &str, mode: &str) -> Option<String> {
        let payload = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "permission_mode": mode,
            "tool_input": {"command": command},
        })
        .to_string();
        let mut child = support::purlis()
            .args(["hook", "pretooluse"])
            .current_dir(&self.root)
            .env_clear()
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("CHARTER_HARNESS", "claude-code")
            .env("PATH", "/usr/bin:/bin")
            .env(SOCKET_ENV, &self.socket)
            .env(CHAT_ENV, CHAT.to_string())
            .envs(self.token.iter().map(|token| (TOKEN_ENV, token.expose())))
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
        assert_eq!(out.status.code(), Some(0), "{command}");
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let o = v.get("hookSpecificOutput")?;
        (o.get("permissionDecision")?.as_str()? == "deny").then(|| {
            o.get("permissionDecisionReason")?
                .as_str()
                .map(str::to_owned)
        })?
    }

    fn attendance_asked(&self) -> usize {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|ask| matches!(ask, Ask::Attendance(_)))
            .count()
    }
}

/// Where no app answers, the chat is taken for unattended: A4 refuses the publish even on a line
/// that says a person is asked.
#[test]
fn a_publish_is_refused_where_no_app_answers() {
    purlis_core::unsteered!();
    let chat = Chat::without_an_app();
    let reason = chat
        .denial(RELEASE, "default")
        .expect("refused with no app");
    assert!(reason.contains(A4_SAYS), "{reason}");
}

/// Where the app says the chat is attended, A4 lets the publish through; where it refuses the
/// ask, the chat is taken for unattended.
#[test]
fn a_publish_is_let_through_only_where_the_app_says_the_chat_is_attended() {
    purlis_core::unsteered!();
    let chat = Chat::answered(Answer::Attended { attended: false });
    let reason = chat.denial(RELEASE, "default").expect("unattended");
    assert!(reason.contains(A4_SAYS), "{reason}");
    assert_eq!(chat.rule().as_deref(), Some(REASON_RELEASE_FLOOR));

    *chat.attendance.lock().unwrap() = Answer::No {
        why: "chat 4 is not one this app has open".to_owned(),
    };
    let reason = chat.denial(RELEASE, "default").expect("refused ask");
    assert!(reason.contains(A4_SAYS), "{reason}");
    assert_eq!(chat.rule().as_deref(), Some(REASON_RELEASE_FLOOR));

    // Last, so no line of an allowed call is read for a refusal's rule.
    *chat.attendance.lock().unwrap() = Answer::Attended { attended: true };
    assert_eq!(chat.denial(RELEASE, "default"), None, "attended");
    assert_eq!(chat.attendance_asked(), 3, "the app was asked each time");
}

/// Q275: a line's own bypass bit lowers A4's answer for that line, whatever the app says of the
/// chat: a harness nested in an attended chat, its prompts off, may not publish.
#[test]
fn a_bypass_line_in_an_attended_chat_is_refused_a_publish() {
    purlis_core::unsteered!();
    let chat = Chat::answered(Answer::Attended { attended: true });
    let reason = chat
        .denial(RELEASE, "bypassPermissions")
        .expect("a bypass line is refused");
    assert!(reason.contains(A4_SAYS), "{reason}");
    assert_eq!(chat.rule().as_deref(), Some(REASON_RELEASE_FLOOR));
}

/// R-145: a merge in the bypass, in a chat the app says is unattended, is the merge hold's: it
/// is refused with the hold's reason, and A4 never answers it.
#[test]
fn a_merge_in_the_bypass_is_refused_by_the_hold_and_not_by_a4() {
    purlis_core::unsteered!();
    let chat = Chat::answered(Answer::Attended { attended: false });
    let reason = chat
        .denial("gh pr merge 12", "bypassPermissions")
        .expect("refused");
    assert!(reason.contains("a chat never merges"), "{reason}");
    assert!(!reason.contains(A4_SAYS), "{reason}");
    assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON));
}
