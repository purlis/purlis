// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A hold is an action purlis blocks and turns into an ask** (R-114), through the real
//! `purlis hook pretooluse` of a chat the app started. The merge hold is the first: the chat's
//! own `gh pr merge` is refused to it, attended as well, and its request is handed to the app,
//! which holds exactly one merge ask for it however often the chat tries.
//!
//! The app here is a listener that answers the hand-over with the core's own
//! `purlis_core::mergehold::asked`, its forge read standing in for a fixed head: what the real
//! app does once it has found the repo and read its head. It confirms an ask with
//! `Answer::MergeHeld`, and refuses one more than a chat may have; the hook's deny line names a
//! hold only for a confirmed ask, and a block's reason otherwise (Q239), which the event log
//! writes as a block. The person's press is at the core's
//! seam, in `crates/purlis-core/tests/it/the_app_merges_on_the_persons_press_and_the_chat_never_merges.rs`.
#![cfg(unix)]

use crate::support;

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use purlis_core::forge::checks::{Checks, Ci};
use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, Hearing, Listener, SOCKET_ENV, TOKEN_ENV, ToolCall,
};
use purlis_core::mergehold::{self, Facts, Holds, MergeKey, Raised};

const CHAT: u32 = 4;
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// A plane, a home that is not the operator's, and the app that started chat [`CHAT`] in it.
struct Chat {
    _tmp: tempfile::TempDir,
    _reading: Option<hookwire::Reading>,
    root: PathBuf,
    home: PathBuf,
    socket: PathBuf,
    token: Option<hookwire::ChatToken>,
    holds: Arc<Holds>,
    asked: Arc<Mutex<Vec<Ask>>>,
    tools: Option<Mutex<mpsc::Receiver<ToolCall>>>,
}

impl Chat {
    fn new() -> Self {
        Self::answering(true)
    }

    /// A chat whose app is of another build than its purlis, where `of_this_build` is false: it
    /// cannot read the merge's hand-over, and answers with `hookwire::UNKNOWN_LINE` (#6960).
    fn answering(of_this_build: bool) -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let home = base.join("home");
        std::fs::create_dir_all(&root).expect("the plane");
        std::fs::create_dir_all(&home).expect("the home");
        // Today's blocking, explicitly (Q308: the default is `watch`).
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n[policy]\ntemplate = \"strict\"\n",
        )
        .expect("the marker");
        let socket = base.join("app").join("hooks.sock");
        let listener = Listener::bind(&base, &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
        let token = listener
            .tokens()
            .issue_to_this_process(CHAT)
            .expect("a token");
        let holds = Arc::new(Holds::default());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let (held, was_asked) = (Arc::clone(&holds), Arc::clone(&asked));
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
                let Ask::Merge(merge) = ask else {
                    return Answer::No {
                        why: "not asked here".to_owned(),
                    };
                };
                if !of_this_build {
                    return Answer::No {
                        why: hookwire::UNKNOWN_LINE.to_owned(),
                    };
                }
                let found = |request: &mergehold::Request| {
                    let key = MergeKey {
                        repo: request
                            .repo
                            .clone()
                            .unwrap_or_else(|| "o/r".try_into().unwrap()),
                        number: request.number,
                        head: HEAD.try_into().unwrap(),
                        base: "main".try_into().unwrap(),
                    };
                    let facts = Facts {
                        title: "Add the parser".to_owned(),
                        author: "octo".to_owned(),
                        checks: Checks {
                            total: Some(1),
                            ci: Ci::Passed,
                            why: None,
                            failed: Vec::new(),
                        },
                        ruleset: mergehold::Ruleset::Protects,
                        approved_by: None,
                        read_at: 1_000,
                    };
                    Ok((key, facts))
                };
                match mergehold::asked(&held, merge.chat, &merge.request, &found, 1_000) {
                    Ok((_, Raised::New | Raised::Already)) => Answer::MergeHeld {
                        said: mergehold::handed_said(&merge.request),
                    },
                    Ok((_, Raised::TooMany)) => Answer::No {
                        why: "this chat has as many merges waiting as it may.".to_owned(),
                    },
                    Err(why) => Answer::No { why },
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
            holds,
            asked,
            tools: Some(Mutex::new(tools)),
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

    /// A chat whose app does not answer: its socket is not there.
    fn without_an_app() -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let home = base.join("home");
        std::fs::create_dir_all(&root).expect("the plane");
        std::fs::create_dir_all(&home).expect("the home");
        // Today's blocking, explicitly (Q308: the default is `watch`).
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n[policy]\ntemplate = \"strict\"\n",
        )
        .expect("the marker");
        Self {
            _tmp: tmp,
            _reading: None,
            root,
            home,
            socket: base.join("app").join("hooks.sock"),
            token: None,
            holds: Arc::default(),
            asked: Arc::default(),
            tools: None,
        }
    }

    /// The `permissionDecisionReason` the chat's hook printed for `command` in `mode`, or
    /// `None` where it did not deny.
    fn denial(&self, command: &str, mode: &str) -> Option<String> {
        self.denial_of("Bash", serde_json::json!({"command": command}), mode)
    }

    /// The `permissionDecisionReason` the chat's hook printed for a call of `tool` with
    /// `input` in `mode`, or `None` where it did not deny.
    fn denial_of(&self, tool: &str, input: serde_json::Value, mode: &str) -> Option<String> {
        let command = input.to_string();
        let payload = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": tool,
            "permission_mode": mode,
            "tool_input": input,
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

    /// [`Self::denial`] attended, run with no chat's environment at all.
    fn denial_outside_a_chat(&self, command: &str) -> Option<String> {
        let payload = serde_json::json!({
            "cwd": ".",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "permission_mode": "default",
            "tool_input": {"command": command},
        })
        .to_string();
        let mut child = support::purlis()
            .args(["hook", "pretooluse"])
            .current_dir(&self.root)
            .env_clear()
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the hook runs");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("the payload");
        let out = child.wait_with_output().expect("the hook finishes");
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        v["hookSpecificOutput"]["permissionDecision"]
            .as_str()
            .filter(|decision| *decision == "deny")
            .map(str::to_owned)
    }

    fn merges_asked(&self) -> usize {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|ask| matches!(ask, Ask::Merge(_)))
            .count()
    }
}

#[test]
fn a_held_action_is_refused_to_the_chat_and_raises_one_ask() {
    let chat = Chat::new();
    for _ in 0..2 {
        let reason = chat
            .denial("gh pr merge 12 --squash", "default")
            .expect("an attended chat's merge is refused to it");
        assert!(reason.contains("a chat never merges"), "{reason}");
        assert!(reason.contains("Nothing was merged"), "{reason}");
        assert!(
            reason.contains("asked in Asks to merge request 12"),
            "{reason}"
        );
        // A confirmed ask is a hold, which the app records itself: no block.
        assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON));
    }
    // Tried twice, asked once: one merge ask waits, for request 12 at the forge's head.
    let waiting = chat.holds.every();
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert_eq!(waiting[0].session, CHAT);
    assert_eq!(waiting[0].key.number, 12);
    assert_eq!(waiting[0].key.head.as_str(), HEAD);
    assert_eq!(chat.merges_asked(), 2);
}

#[test]
fn a_merge_whose_request_cannot_be_read_is_refused_and_asks_nobody() {
    let chat = Chat::new();
    let reason = chat
        .denial("gh api -X PUT repos/o/r/pulls/12/merge", "default")
        .expect("refused");
    assert!(reason.contains("nobody was asked"), "{reason}");
    assert_eq!(chat.merges_asked(), 0);
    assert!(chat.holds.every().is_empty());
}

#[test]
fn what_is_no_merge_is_not_held() {
    let chat = Chat::new();
    for command in ["gh pr view 12", "gh pr create --fill", "git status"] {
        assert_eq!(chat.denial(command, "default"), None, "{command}");
    }
    assert_eq!(chat.merges_asked(), 0);
}

/// Where the app does not answer, the merge is still refused: the hold never fails open.
#[test]
fn a_held_merge_is_refused_where_no_app_answers() {
    let chat = Chat::without_an_app();
    let reason = chat
        .denial("gh pr merge 12", "default")
        .expect("refused with no app");
    assert!(reason.contains("Nothing was merged"), "{reason}");
    assert!(reason.contains("The app was not heard from"), "{reason}");
    assert!(reason.contains("nobody was asked"), "{reason}");
    assert!(!reason.contains("the person is asked in Asks"), "{reason}");
    // Outside a chat the app started, an attended merge is the person's own: A4's ground.
    let mut outside = Chat::without_an_app();
    outside.socket = PathBuf::new();
    assert_eq!(outside.denial_outside_a_chat("gh pr merge 12"), None);
}

/// Q239 (S3): a merge the app raises no ask for, here one more than a chat may have waiting, is
/// refused with a block's reason, and the chat is never told the person is asked.
#[test]
fn a_merge_the_app_raises_no_ask_for_is_a_block_and_says_nobody_was_asked() {
    let chat = Chat::new();
    for number in 1..=mergehold::AT_MOST_PER_CHAT {
        chat.denial(&format!("gh pr merge {number}"), "default")
            .expect("refused");
        assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON));
    }
    let reason = chat.denial("gh pr merge 99", "default").expect("refused");
    assert!(reason.contains("Nobody was asked"), "{reason}");
    assert!(!reason.contains("The person is asked"), "{reason}");
    assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON_NOT_ASKED));
    assert_eq!(chat.holds.every().len(), mergehold::AT_MOST_PER_CHAT);
}

/// #6960 (a): where the app answers the hand-over as a line from a purlis it does not know, the
/// chat is told the app and its purlis are different builds and what to do, restart purlis or
/// update the plugin, and is named no command, which would only be refused again.
#[test]
fn an_unknown_build_merge_answer_names_restart_or_update_and_no_refused_form() {
    let chat = Chat::answering(false);
    let reason = chat.denial("gh pr merge 12", "default").expect("refused");
    assert!(reason.contains("different builds"), "{reason}");
    assert!(reason.contains("restart purlis"), "{reason}");
    assert!(reason.contains("update the purlis plugin"), "{reason}");
    assert!(!reason.contains(hookwire::UNKNOWN_LINE), "{reason}");
    assert!(!reason.contains('`'), "names a command: {reason}");
    assert!(
        reason.contains("A yes in this chat merges nothing"),
        "{reason}"
    );
    assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON_NOT_ASKED));
    assert!(chat.holds.every().is_empty());
}

/// #6960 (c): one cell per way out `mergehold::ways_out` can name, each run as the chat or the
/// person would: it leads to an ask or a run, never to another refusal.
#[test]
fn every_named_way_out_leads_to_an_ask_or_a_run() {
    use mergehold::WayOut;
    for way in [
        WayOut::MergeOnItsOwn,
        WayOut::AskThePerson,
        WayOut::OneBuild,
    ] {
        match way {
            // Each form, its number filled in, run on its own: one ask, in Asks.
            WayOut::MergeOnItsOwn => {
                for (n, form) in way.forms().iter().enumerate() {
                    let chat = Chat::new();
                    let cmd = form.replace("<number>", &(n + 7).to_string());
                    let reason = chat.denial(&cmd, "default").expect("held");
                    assert!(
                        reason.contains("The person is asked in Asks"),
                        "{cmd}: {reason}"
                    );
                    assert_eq!(chat.rule().as_deref(), Some(mergehold::REASON), "{cmd}");
                    assert_eq!(chat.holds.every().len(), 1, "{cmd}");
                }
            }
            // The chat asks the person in words: the question runs.
            WayOut::AskThePerson => {
                let chat = Chat::new();
                assert!(way.forms().is_empty());
                assert_eq!(
                    chat.denial_of(
                        "mcp__purlis__ask_operator",
                        serde_json::json!({"question": "Will you merge request 12 in Asks?"}),
                        "default",
                    ),
                    None
                );
            }
            // Once the app and its purlis are one build, the same merge asks.
            WayOut::OneBuild => {
                assert!(way.forms().is_empty());
                let before = Chat::answering(false);
                assert!(before.denial("gh pr merge 12", "default").is_some());
                assert!(before.holds.every().is_empty());
                let after = Chat::answering(true);
                let reason = after.denial("gh pr merge 12", "default").expect("held");
                assert!(reason.contains("The person is asked in Asks"), "{reason}");
                assert_eq!(after.holds.every().len(), 1);
            }
        }
    }
}
