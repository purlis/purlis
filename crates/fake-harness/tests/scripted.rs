// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `fake-harness --script --dialect` (T1.1-04, #6851), with a stand-in `purlis` as the hook
//! binary that writes down each call and answers as a test tells it. What needs purlis's wrap
//! (the native write, the MCP call, the fidelity to the recordings) is in `under_the_wrap.rs`.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

const DIALECTS: [&str; 3] = ["claude", "codex", "opencode"];

/// A stand-in `purlis`: each `hook <word>` call is written to `$STUB_LOG` as its word and its
/// standard input; `$STUB_DENY` names a word it denies with a reason, `$STUB_EXIT2` one it
/// refuses with exit 2, `$STUB_CONTEXT` one it adds context on.
const STUB: &str = r#"#!/bin/sh
word="$2"
input=$(cat)
printf '%s\t%s\n' "$word" "$input" >> "$STUB_LOG"
if [ "$word" = "$STUB_DENY" ]; then
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"STUB-DENY: not this one"}}'
fi
if [ "$word" = "$STUB_EXIT2" ]; then
  echo "STUB-EXIT2: refused" >&2
  exit 2
fi
if [ "$word" = "$STUB_CONTEXT" ]; then
  printf '{"hookSpecificOutput":{"hookEventName":"X","additionalContext":"STUB-CONTEXT"}}'
fi
exit 0
"#;

struct Run {
    dir: tempfile::TempDir,
}

struct Played {
    stdout: String,
    hooks: Vec<(String, Value)>,
    transcript: Vec<Value>,
}

impl Run {
    fn new() -> Run {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::create_dir_all(dir.path().join("work")).expect("work");
        std::fs::create_dir_all(dir.path().join("outside")).expect("outside");
        stand_in::program(dir.path(), "purlis", STUB);
        Run { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn work(&self) -> PathBuf {
        self.path("work").canonicalize().expect("work")
    }

    fn script(&self, script: &Value) -> PathBuf {
        let path = self.path("script.json");
        std::fs::write(&path, script.to_string()).expect("the script");
        path
    }

    /// The fake in `dialect`, with `words` after its own, `typed` written to its input and then
    /// closed, and `env` beside the stand-in's.
    fn play(&self, dialect: &str, words: &[&str], typed: &[&str], env: &[(&str, &str)]) -> Played {
        let log = self.path("hooks.log");
        let transcript = self.path("transcript.jsonl");
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_file(&transcript);
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
            .args(["--script"])
            .arg(self.path("script.json"))
            .args(["--dialect", dialect, "--transcript"])
            .arg(&transcript)
            .args(["--place", &format!("outside={}", self.path("outside").display())])
            .args(words)
            .current_dir(self.work())
            .env("CHARTER_HOOK_BINARY", self.path("purlis"))
            .env("STUB_LOG", &log)
            .env("FAKE_HARNESS_STATE", self.path("state"))
            .env("CLAUDE_CONFIG_DIR", self.path("profile"))
            .env("CODEX_HOME", self.path("profile"))
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the fake starts");
        {
            let mut stdin = child.stdin.take().expect("stdin");
            for line in typed {
                // A line typed while a turn may still be running waits as the next prompt.
                stdin.write_all(line.as_bytes()).expect("typed");
                stdin.write_all(b"\r").expect("Enter");
            }
        }
        let out = child.wait_with_output().expect("the fake ends");
        assert!(
            out.status.success(),
            "the fake failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let hooks = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (word, input) = l.split_once('\t')?;
                Some((word.to_owned(), serde_json::from_str(input).unwrap_or(Value::Null)))
            })
            .collect();
        let transcript = std::fs::read_to_string(&transcript)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("a transcript line is JSON"))
            .collect();
        Played {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            hooks,
            transcript,
        }
    }
}

impl Played {
    fn words(&self) -> Vec<&str> {
        self.hooks.iter().map(|(w, _)| w.as_str()).collect()
    }

    fn events(&self) -> BTreeSet<String> {
        self.hooks
            .iter()
            .filter_map(|(_, i)| i.get("hook_event_name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect()
    }

    fn session(&self) -> String {
        self.stdout
            .lines()
            .find_map(|l| l.split_once(" session ").map(|(_, s)| s.trim().to_owned()))
            .expect("the fake says its session")
    }
}

/// The first message, as purlis hands it to each dialect.
fn first_message(dialect: &str, text: &str) -> Vec<String> {
    purlis_core::handoff::first_message_argv(dialect, text).expect("a measured harness")
}

fn strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn routes(dialect: &str, env: &[(&str, &str)]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .args(["--routes", "--script", "-", "--dialect", dialect])
        .envs(env.iter().copied())
        .output()
        .expect("the routes");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("the routes are JSON")
}

#[test]
fn the_fake_harness_routes_come_from_purlis_core_for_every_dialect() {
    let binary = "/opt/purlis/bin/purlis";
    let env = [("CHARTER_HOOK_BINARY", binary)];
    let triples = |doc: &Value| -> BTreeSet<(String, Option<String>, String, u64)> {
        doc["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .map(|r| {
                (
                    r["event"].as_str().unwrap_or_default().to_owned(),
                    r["matcher"].as_str().map(str::to_owned),
                    r["word"].as_str().unwrap_or_default().to_owned(),
                    r["timeout"].as_u64().unwrap_or_default(),
                )
            })
            .collect()
    };
    let fires = |doc: &Value| -> BTreeSet<String> {
        doc["fires"]
            .as_array()
            .expect("fires")
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    };

    // Claude Code: every hook of the generated plugin, and the permission hook on `--settings`.
    let claude = routes("claude", &env);
    let plugin: Value = serde_json::from_str(&purlis_core::plugin::hooks_json()).expect("JSON");
    let mut want = BTreeSet::new();
    for (event, groups) in plugin["hooks"].as_object().expect("events") {
        for group in groups.as_array().expect("groups") {
            for hook in group["hooks"].as_array().expect("hooks") {
                let command = hook["command"].as_str().expect("a command");
                let word = command.rsplit(' ').next().expect("a word").to_owned();
                want.insert((
                    event.clone(),
                    group["matcher"].as_str().map(str::to_owned),
                    word,
                    hook["timeout"].as_u64().expect("a timeout"),
                ));
            }
        }
    }
    want.insert((
        "PermissionRequest".to_owned(),
        None,
        purlis_core::harness::hooked::WORD.to_owned(),
        purlis_core::harness::hooked::HOOK_TIMEOUT.as_secs(),
    ));
    assert_eq!(triples(&claude), want, "Claude Code's routes are its plugin's");
    for handler in purlis_core::hookreg::HANDLERS {
        assert!(
            fires(&claude).contains(handler.event),
            "purlis-core arms {} on {}, which the fake's Claude Code never fires",
            handler.name,
            handler.event
        );
    }

    // Codex: what purlis-core arms a Codex chat with, and nothing more.
    let codex = routes("codex", &env);
    let codex_words: BTreeSet<String> = triples(&codex).into_iter().map(|t| t.2).collect();
    for word in purlis_core::plugin::CODEX {
        assert!(codex_words.contains(word), "Codex is armed with {word}");
    }
    for (event, _, word, _) in triples(&codex) {
        assert!(
            fires(&codex).contains(&event),
            "purlis-core arms Codex's {word} on {event}, which the fake's Codex never fires"
        );
        assert!(
            event != "PostToolUseFailure" && event != "Notification",
            "Codex has no {event}"
        );
    }
    for route in codex["routes"].as_array().expect("routes") {
        assert!(
            route["command"]
                .as_str()
                .is_some_and(|c| c.starts_with(&format!("'{binary}' hook "))),
            "a Codex hook names purlis by its path: {route}"
        );
    }

    // OpenCode: the shim's table, each word's timeout the registry's.
    let opencode = routes("opencode", &env);
    let got = triples(&opencode);
    for tool in purlis_core::opencode::TOOLS {
        let pre = tool.pre.unwrap_or(purlis_core::opencode::DEFAULT_PRE);
        assert!(
            got.iter().any(|(e, m, w, t)| e == "PreToolUse"
                && m.as_deref() == Some(tool.id)
                && w == pre
                && *t == u64::from(purlis_core::hookreg::find(pre).expect("a word").timeout)),
            "OpenCode's {} goes to {pre} first",
            tool.id
        );
        if let Some(post) = tool.post {
            assert!(
                got.iter().any(|(e, m, w, _)| e == "PostToolUse"
                    && m.as_deref() == Some(tool.id)
                    && w == post),
                "OpenCode's {} goes to {post} after",
                tool.id
            );
        }
    }
    for (event, _, word, _) in &got {
        assert!(
            fires(&opencode).contains(event),
            "the shim runs {word} on {event}, which the fake's OpenCode never fires"
        );
    }
}

fn covering_like(steps: Value) -> Value {
    json!({"turns": [{"prompt": "go", "calls": steps, "says": "done"}]})
}

#[test]
fn a_codex_turn_fires_no_posttoolusefailure_and_no_notification() {
    let run = Run::new();
    run.script(&covering_like(json!([
        {"step": "failing-bash", "tool": "bash", "run": "echo failing >&2; exit 2"},
        {"step": "write", "tool": "write", "input": {"path": "<outside>/w.txt", "content": "w"}},
    ])));
    let played = run.play("codex", &strs(&first_message("codex", "go")), &[], &[]);
    let events = played.events();
    assert!(!events.contains("PostToolUseFailure"), "{:?}", played.words());
    assert!(!events.contains("Notification"), "{:?}", played.words());
    assert!(
        !played.words().contains(&"notification"),
        "{:?}",
        played.words()
    );
    // The failed command is a PostToolUse, with what it printed.
    let post = played
        .hooks
        .iter()
        .find(|(w, i)| w == "posttooluse-blocked" && i["tool_input"]["command"] == "echo failing >&2; exit 2")
        .expect("the failed command's PostToolUse");
    assert_eq!(post.1["tool_response"], "failing\n");
    // The same calls in Claude Code do fire PostToolUseFailure: the difference is the dialect's.
    let claude = run.play("claude", &strs(&first_message("claude", "go")), &[], &[]);
    assert!(claude.events().contains("PostToolUseFailure"), "{:?}", claude.words());
}

#[test]
fn a_pretooluse_deny_stops_the_call_and_its_reason_reaches_the_transcript() {
    for dialect in DIALECTS {
        for (how, env) in [
            ("deny", [("STUB_DENY", "pretooluse")]),
            ("exit 2", [("STUB_EXIT2", "pretooluse")]),
        ] {
            let run = Run::new();
            run.script(&covering_like(json!([
                {"step": "refused", "tool": "bash", "run": "echo ran > ran.txt"},
            ])));
            let played = run.play(dialect, &strs(&first_message(dialect, "go")), &[], &env);
            assert!(
                !run.work().join("ran.txt").exists(),
                "{dialect} ({how}): the refused command ran"
            );
            assert!(
                !played.words().contains(&"posttooluse-blocked"),
                "{dialect} ({how}): a refused call has no PostToolUse: {:?}",
                played.words()
            );
            let result = played
                .transcript
                .iter()
                .find(|l| l["kind"] == "tool_result" && l["step"] == "refused")
                .unwrap_or_else(|| panic!("{dialect} ({how}): no result: {:?}", played.transcript));
            let reason = if how == "deny" {
                "STUB-DENY: not this one"
            } else {
                "STUB-EXIT2: refused"
            };
            assert_eq!(result["reason"], reason, "{dialect} ({how})");
            assert_eq!(result["hook"], "pretooluse", "{dialect} ({how})");
            assert!(
                result["text"].as_str().is_some_and(|t| t.contains(reason)),
                "{dialect} ({how}): the model is shown the reason: {result}"
            );
        }
    }
}

#[test]
fn a_line_purlis_types_into_the_chat_reaches_the_transcript() {
    // The Tell-it-to-stop line, the restart message, the vault's Allow: each a line purlis
    // types into the chat's terminal.
    let typed = "purlis: stop after this command and wait for the person.";
    for dialect in DIALECTS {
        let run = Run::new();
        run.script(&json!({"turns": [
            {"prompt": "go", "calls": [{"step": "one", "tool": "bash", "run": "echo one"}]},
            {"prompt": typed, "calls": []},
        ]}));
        let played = run.play(dialect, &strs(&first_message(dialect, "go")), &[typed], &[]);
        assert!(
            played.transcript.iter().any(|l| l["kind"] == "user_text"
                && l["typed"] == true
                && l["text"].as_str().is_some_and(|t| t.contains(typed))),
            "{dialect}: {:#?}",
            played.transcript
        );
        assert!(
            played
                .hooks
                .iter()
                .any(|(w, i)| w == "userpromptsubmit" && i["prompt"] == typed),
            "{dialect}: the typed line is a prompt to its hooks: {:?}",
            played.words()
        );
    }
}

#[test]
fn a_relaunched_script_continues_at_its_next_turn() {
    for dialect in DIALECTS {
        let run = Run::new();
        run.script(&json!({"turns": [
            {"prompt": "first", "calls": [{"step": "one", "tool": "bash", "run": "echo one >> turns.txt"}]},
            {"prompt": "second", "calls": [{"step": "two", "tool": "bash", "run": "echo two >> turns.txt"}]},
        ]}));
        let first = run.play(dialect, &strs(&first_message(dialect, "first")), &[], &[]);
        let session = first.session();
        assert_eq!(
            std::fs::read_to_string(run.work().join("turns.txt")).unwrap_or_default(),
            "one\n",
            "{dialect}: the first launch plays the first turn alone"
        );
        let cursor = run.path("state").join(format!("{session}.json"));
        assert!(cursor.is_file(), "{dialect}: the cursor is kept by session id");
        assert!(
            !run.path("script.json").with_file_name(format!("{session}.json")).exists(),
            "{dialect}: never beside the script"
        );
        // Relaunched as purlis relaunches it, with the dialect's resume argument.
        let resume: Vec<String> = match dialect {
            "claude" => vec!["--resume".into(), session.clone(), "second".into()],
            "codex" => vec!["resume".into(), session.clone(), "second".into()],
            _ => vec!["-s".into(), session.clone(), "--prompt".into(), "second".into()],
        };
        let second = run.play(dialect, &strs(&resume), &[], &[]);
        assert_eq!(second.session(), session, "{dialect}: the same session");
        assert_eq!(
            std::fs::read_to_string(run.work().join("turns.txt")).unwrap_or_default(),
            "one\ntwo\n",
            "{dialect}: the relaunch plays the next turn, and not the first again"
        );
        let start = second
            .hooks
            .iter()
            .find(|(w, _)| w == "sessionstart")
            .unwrap_or_else(|| panic!("{dialect}: {:?}", second.words()));
        assert_eq!(start.1["source"], "resume", "{dialect}");
        assert_eq!(start.1["session_id"], session.as_str(), "{dialect}");
    }
}

#[test]
fn the_claude_dialect_applies_permissions_deny_before_native_io() {
    let run = Run::new();
    let outside = run.path("outside").canonicalize().expect("outside");
    let work = run.work();
    std::fs::write(work.join("secret.txt"), "SECRET-MARKER").expect("a secret");
    let settings = json!({"permissions": {"deny": [
        format!("Edit(/{})", outside.display()),
        format!("Edit(/{}/**)", outside.display()),
        format!("Read(/{}/secret.txt)", work.display()),
        "WebFetch",
    ]}})
    .to_string();
    run.script(&covering_like(json!([
        {"step": "denied-write", "tool": "write", "input": {"path": format!("{}/by-write.txt", outside.display()), "content": "x"}},
        {"step": "denied-read", "tool": "read", "input": {"path": format!("{}/secret.txt", work.display())}},
        {"step": "denied-web", "tool": "web", "input": {"url": "http://127.0.0.1:9/"}},
        {"step": "allowed-write", "tool": "write", "input": {"path": format!("{}/ok.txt", work.display()), "content": "ok"}},
    ])));
    let mut words = vec!["--settings".to_owned(), settings];
    words.extend(first_message("claude", "go"));
    let played = run.play("claude", &strs(&words), &[], &[]);

    assert!(!outside.join("by-write.txt").exists(), "the denied write happened");
    assert_eq!(
        std::fs::read_to_string(work.join("ok.txt")).ok().as_deref(),
        Some("ok"),
        "the positive control: a write the rules allow is done"
    );
    let result = |step: &str| {
        played
            .transcript
            .iter()
            .find(|l| l["step"] == step)
            .unwrap_or_else(|| panic!("no result for {step}: {:#?}", played.transcript))
            .clone()
    };
    assert!(
        result("denied-write")["text"]
            .as_str()
            .is_some_and(|t| t.contains("denied by your permission settings"))
    );
    assert!(
        !result("denied-read")["text"]
            .as_str()
            .unwrap_or_default()
            .contains("SECRET-MARKER"),
        "the denied file's contents reached the model"
    );
    assert!(
        result("denied-web")["text"]
            .as_str()
            .is_some_and(|t| t.contains("No such tool available: WebFetch"))
    );
    // Held before any hook: the only file-tool guard that ran is the allowed write's.
    let guarded: Vec<&Value> = played
        .hooks
        .iter()
        .filter(|(w, _)| w.starts_with("pretooluse"))
        .map(|(_, i)| &i["tool_input"])
        .collect();
    assert_eq!(guarded.len(), 1, "{guarded:?}");
    assert!(guarded[0]["file_path"].as_str().is_some_and(|p| p.ends_with("ok.txt")));
}

#[test]
fn one_script_runs_unchanged_in_all_three_dialects() {
    let run = Run::new();
    let script = covering_like(json!([
        {"step": "bash", "tool": "bash", "run": "echo hello"},
        {"step": "write", "tool": "write", "input": {"path": "<work>/w.txt", "content": "w"}},
        {"step": "edit", "tool": "edit", "input": {"path": "<work>/w.txt", "old": "w", "new": "v"}},
    ]));
    for dialect in DIALECTS {
        run.script(&script);
        let _ = std::fs::remove_file(run.work().join("w.txt"));
        let played = run.play(dialect, &strs(&first_message(dialect, "go")), &[], &[]);
        assert_eq!(
            std::fs::read_to_string(run.work().join("w.txt")).ok().as_deref(),
            Some(if dialect == "codex" { "v\n" } else { "v" }),
            "{dialect}: {:#?}",
            played.transcript
        );
        assert!(
            played
                .transcript
                .iter()
                .any(|l| l["step"] == "bash" && l["text"].as_str().is_some_and(|t| t.contains("hello"))),
            "{dialect}"
        );
        assert!(played.events().contains("PreToolUse"), "{dialect}");
    }
}
