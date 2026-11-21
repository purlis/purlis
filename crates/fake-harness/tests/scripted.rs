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
/// standard input (and, where `$STUB_SIZE_LOG` names a file, its word and the `COLUMNS` and
/// `LINES` it was run with); `$STUB_DENY` names a word it denies with a reason, `$STUB_EXIT2` one it
/// refuses with exit 2, `$STUB_CONTEXT` one it adds context on.
const STUB: &str = r#"#!/bin/sh
word="$2"
input=$(cat)
printf '%s\t%s\n' "$word" "$input" >> "$STUB_LOG"
if [ -n "$STUB_SIZE_LOG" ]; then
  printf '%s %s %s\n' "$word" "${COLUMNS-unset}" "${LINES-unset}" >> "$STUB_SIZE_LOG"
fi
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

/// Bytes typed into the fake, once the stand-in heard the hook `once_heard` names (at once
/// where it names none).
struct Keys {
    once_heard: Option<&'static str>,
    bytes: String,
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
        let keys: Vec<Keys> = typed
            .iter()
            .map(|line| Keys {
                once_heard: None,
                bytes: format!("{line}\r"),
            })
            .collect();
        self.play_keys(dialect, words, &keys, env)
    }

    /// As [`Run::play`], each of `keys` typed only once its hook was heard, as a person answers
    /// what the harness asks once it asks.
    fn play_keys(
        &self,
        dialect: &str,
        words: &[&str],
        keys: &[Keys],
        env: &[(&str, &str)],
    ) -> Played {
        let log = self.path("hooks.log");
        let transcript = self.path("transcript.jsonl");
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_file(&transcript);
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
            .args(["--script"])
            .arg(self.path("script.json"))
            .args(["--dialect", dialect, "--transcript"])
            .arg(&transcript)
            .args([
                "--place",
                &format!("outside={}", self.path("outside").display()),
            ])
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
            for key in keys {
                if let Some(word) = key.once_heard {
                    let since = std::time::Instant::now();
                    while !std::fs::read_to_string(&log)
                        .unwrap_or_default()
                        .lines()
                        .any(|l| l.split('\t').next() == Some(word))
                    {
                        if since.elapsed() > std::time::Duration::from_secs(60) {
                            let _ = child.kill();
                            panic!(
                                "{word} was never heard; the hooks heard:\n{}",
                                std::fs::read_to_string(&log).unwrap_or_default()
                            );
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
                // A line typed while a turn may still be running waits as the next prompt.
                stdin.write_all(key.bytes.as_bytes()).expect("typed");
                stdin.flush().expect("typed");
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
                Some((
                    word.to_owned(),
                    serde_json::from_str(input).unwrap_or(Value::Null),
                ))
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
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
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

    // Claude Code: every hook of the generated plugin, and on `--settings` the guards (#7067)
    // and the permission hook.
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
    for guard in purlis_core::hookreg::HANDLERS
        .iter()
        .filter(|h| h.guards_a_call())
    {
        want.insert((
            guard.event.to_owned(),
            guard.matcher.map(str::to_owned),
            guard.name.to_owned(),
            u64::from(guard.timeout),
        ));
    }
    want.insert((
        "PermissionRequest".to_owned(),
        None,
        purlis_core::harness::hooked::WORD.to_owned(),
        purlis_core::harness::hooked::HOOK_TIMEOUT.as_secs(),
    ));
    assert_eq!(
        triples(&claude),
        want,
        "Claude Code's routes are its plugin's, its guards' and its permission hook"
    );
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
    // An event purlis-core arms that the fake cannot fire is named, with why, and never also
    // listed as fired.
    let never_fired: BTreeSet<String> = codex["never_fired"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(event, why)| {
            assert!(why.as_str().is_some_and(|w| !w.is_empty()), "{event}: why?");
            event.clone()
        })
        .collect();
    assert!(
        never_fired.is_disjoint(&fires(&codex)),
        "{never_fired:?} is both fired and never fired"
    );
    for (event, _, word, _) in triples(&codex) {
        assert!(
            fires(&codex).contains(&event) || never_fired.contains(&event),
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
    assert!(
        !events.contains("PostToolUseFailure"),
        "{:?}",
        played.words()
    );
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
        .find(|(w, i)| {
            w == "posttooluse-blocked" && i["tool_input"]["command"] == "echo failing >&2; exit 2"
        })
        .expect("the failed command's PostToolUse");
    assert_eq!(post.1["tool_response"], "failing\n");
    // The same calls in Claude Code do fire PostToolUseFailure: the difference is the dialect's.
    let claude = run.play("claude", &strs(&first_message("claude", "go")), &[], &[]);
    assert!(
        claude.events().contains("PostToolUseFailure"),
        "{:?}",
        claude.words()
    );
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
        assert!(
            cursor.is_file(),
            "{dialect}: the cursor is kept by session id"
        );
        assert!(
            !run.path("script.json")
                .with_file_name(format!("{session}.json"))
                .exists(),
            "{dialect}: never beside the script"
        );
        // Relaunched as purlis relaunches it, with the dialect's resume argument.
        let resume: Vec<String> = match dialect {
            "claude" => vec!["--resume".into(), session.clone(), "second".into()],
            "codex" => vec!["resume".into(), session.clone(), "second".into()],
            _ => vec![
                "-s".into(),
                session.clone(),
                "--prompt".into(),
                "second".into(),
            ],
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
    // Claude Code's own prompts bypassed, as its covering recording ran: its deny rules hold.
    let mut words = vec![
        "--settings".to_owned(),
        settings,
        "--permission-mode".to_owned(),
        "bypassPermissions".to_owned(),
    ];
    words.extend(first_message("claude", "go"));
    let played = run.play("claude", &strs(&words), &[], &[]);

    assert!(
        !outside.join("by-write.txt").exists(),
        "the denied write happened"
    );
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
    assert!(
        guarded[0]["file_path"]
            .as_str()
            .is_some_and(|p| p.ends_with("ok.txt"))
    );
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
        // Claude Code's own prompts bypassed, as its covering recording ran.
        let mut words = first_message(dialect, "go");
        if dialect == "claude" {
            words.extend([
                "--permission-mode".to_owned(),
                "bypassPermissions".to_owned(),
            ]);
        }
        let played = run.play(dialect, &strs(&words), &[], &[]);
        assert_eq!(
            std::fs::read_to_string(run.work().join("w.txt"))
                .ok()
                .as_deref(),
            Some(if dialect == "codex" { "v\n" } else { "v" }),
            "{dialect}: {:#?}",
            played.transcript
        );
        assert!(
            played
                .transcript
                .iter()
                .any(|l| l["step"] == "bash"
                    && l["text"].as_str().is_some_and(|t| t.contains("hello"))),
            "{dialect}"
        );
        assert!(played.events().contains("PreToolUse"), "{dialect}");
    }
}

#[test]
fn a_claude_write_in_the_default_mode_asks_the_person_after_permissionrequest_and_notification() {
    // As Claude Code 2.1.325's `asks` recording shows it (T1.1-03): in the `default` permission
    // mode a Write is asked about; purlis's permission hook hears the ask first, Claude Code
    // tells its Notification hook a person is wanted, and the write is done once allowed.
    let run = Run::new();
    run.script(&covering_like(json!([
        {"step": "asked-write", "tool": "Write", "input": {"file_path": "<work>/asked.txt", "content": "asked"}},
    ])));
    let asked = run.work().join("asked.txt");
    let played = run.play_keys(
        "claude",
        &strs(&first_message("claude", "go")),
        &[Keys {
            once_heard: Some("notification"),
            bytes: "\r".to_owned(),
        }],
        &[],
    );
    let words = played.words();
    let at = |word: &str| {
        words
            .iter()
            .position(|w| *w == word)
            .unwrap_or_else(|| panic!("no {word}: {words:?}"))
    };
    assert!(
        at("pretooluse-edit") < at("permissionrequest")
            && at("permissionrequest") < at("notification")
            && at("notification") < at("posttooluse"),
        "the guard, the ask, the notification, then the write's result: {words:?}"
    );
    let input = |word: &str| &played.hooks[at(word)].1;
    let request = input("permissionrequest");
    assert_eq!(request["hook_event_name"], "PermissionRequest");
    assert_eq!(request["tool_name"], "Write");
    assert_eq!(request["permission_mode"], "default");
    assert_eq!(
        request["permission_suggestions"],
        json!([{"type": "setMode", "mode": "acceptEdits", "destination": "session"}])
    );
    assert!(
        request.get("tool_use_id").is_none(),
        "Claude Code hands its permission hook no call id: {request}"
    );
    let notification = input("notification");
    assert_eq!(notification["message"], "Claude needs your permission");
    assert_eq!(notification["notification_type"], "permission_prompt");
    assert!(
        notification.get("permission_mode").is_none() && notification.get("effort").is_none(),
        "{notification}"
    );
    assert_eq!(
        std::fs::read_to_string(&asked).ok().as_deref(),
        Some("asked")
    );
    let result = played
        .transcript
        .iter()
        .find(|l| l["step"] == "asked-write")
        .expect("the write's result");
    assert_eq!(
        result["text"],
        format!(
            "File created successfully at: {} (file state is current in your context \u{2014} no need to Read it back)",
            asked.display()
        )
    );
    assert!(result.get("is_error").is_none(), "{result}");
}

#[test]
fn a_claude_write_the_person_refuses_is_not_done() {
    let run = Run::new();
    run.script(&covering_like(json!([
        {"step": "asked-write", "tool": "write", "input": {"path": "<work>/refused.txt", "content": "x"}},
    ])));
    let played = run.play_keys(
        "claude",
        &strs(&first_message("claude", "go")),
        &[Keys {
            once_heard: Some("notification"),
            bytes: "\x1b".to_owned(),
        }],
        &[],
    );
    assert!(!run.work().join("refused.txt").exists());
    let result = played
        .transcript
        .iter()
        .find(|l| l["step"] == "asked-write")
        .expect("the write's result");
    assert_eq!(result["is_error"], true, "{result}");
    assert!(
        !played.words().contains(&"posttooluse"),
        "{:?}",
        played.words()
    );
}

/// Each event the fake says it fires (`--routes`' `fires`), heard by the stand-in in a run of
/// `script`, answered with `keys`.
fn fires_heard(dialect: &str, script: &Value, words: &[String], keys: &[Keys]) {
    let run = Run::new();
    std::fs::write(run.path("outside/existing.txt"), "before").expect("a file outside");
    run.script(script);
    let played = run.play_keys(dialect, &strs(words), keys, &[]);
    let listed = routes(
        dialect,
        &[("CHARTER_HOOK_BINARY", "/opt/purlis/bin/purlis")],
    );
    // Each hook heard, by the event its payload names, else by the event its word is armed on
    // (OpenCode's shim hands its permission hook OpenCode's own ask).
    let mut heard = played.events();
    for word in played.words() {
        for route in listed["routes"].as_array().expect("routes") {
            if route["word"] == word {
                heard.insert(route["event"].as_str().expect("an event").to_owned());
            }
        }
    }
    for event in listed["fires"].as_array().expect("fires") {
        let event = event.as_str().expect("an event");
        assert!(
            heard.contains(event),
            "the fake's {dialect} lists {event} as fired, and a run firing everything it can never fired it; heard: {heard:?}"
        );
    }
}

#[test]
fn every_event_the_fake_lists_as_fired_is_fired_in_a_scripted_run() {
    // Claude Code: a `/<command>` typed, a call that ends well and one that fails, a write it
    // asks about, a call its own check refuses, a sub-agent, and a turn an API error ends.
    let claude = json!({"turns": [
        {"prompt": "/purlis:demo now", "calls": [
            {"step": "ok", "tool": "bash", "run": "true"},
            {"step": "failing", "tool": "bash", "run": "exit 3"},
            {"step": "asked", "tool": "write", "input": {"path": "<work>/asked.txt", "content": "a"}},
            {"step": "denied", "tool": "bash", "run": "true", "harness_denies": "the classifier refused it"},
            {"step": "agent", "tool": "subagent", "input": {"description": "d", "prompt": "p"}, "calls": [
                {"step": "inner", "tool": "bash", "run": "true"},
            ], "says": "inner done"},
        ], "says": "done"},
        {"prompt": "second", "fails": "rate_limit"},
    ]});
    fires_heard(
        "claude",
        &claude,
        &first_message("claude", "/purlis:demo now"),
        &[
            Keys {
                once_heard: Some("notification"),
                bytes: "\r".to_owned(),
            },
            Keys {
                once_heard: Some("stop"),
                bytes: "second\r".to_owned(),
            },
        ],
    );
    // Codex: a call that ends well, one the model asks to run escalated, which Codex asks
    // about, and the end of its session.
    fires_heard(
        "codex",
        &covering_like(json!([
            {"step": "ok", "tool": "bash", "run": "true"},
            {"step": "escalated", "tool": "exec_command", "input": {"cmd": "true", "sandbox_permissions": "require_escalated", "justification": "j"}},
        ])),
        &first_message("codex", "go"),
        &[Keys {
            once_heard: Some("permissionrequest"),
            bytes: "\r".to_owned(),
        }],
    );
    // OpenCode: a call, and an edit of a file outside the chat's folder, which it asks about.
    fires_heard(
        "opencode",
        &covering_like(json!([
            {"step": "ok", "tool": "bash", "run": "true"},
            {"step": "outside", "tool": "edit", "input": {"path": "<outside>/existing.txt", "old": "before", "new": "after"}},
        ])),
        &first_message("opencode", "go"),
        &[],
    );
}

#[test]
fn claude_hooks_are_told_the_terminal_size_the_fake_runs_in_not_a_fixed_one() {
    let run = Run::new();
    run.script(&covering_like(json!([])));
    let sizes = run.path("sizes.log");
    let sizes_at = sizes.display().to_string();
    run.play(
        "claude",
        &strs(&first_message("claude", "go")),
        &[],
        &[
            ("COLUMNS", "91"),
            ("LINES", "27"),
            ("STUB_SIZE_LOG", &sizes_at),
        ],
    );
    let said = std::fs::read_to_string(&sizes).expect("the hooks ran");
    assert!(!said.is_empty());
    for line in said.lines() {
        assert!(line.ends_with(" 91 27"), "{line}");
    }
}

#[test]
#[cfg_attr(target_os = "linux", ignore = "bisecting #7304")]
fn a_tool_command_says_its_group_in_the_file_the_scenario_names() {
    let run = Run::new();
    run.script(&covering_like(json!([
        {"step": "bash", "tool": "bash", "run": "echo hello"},
    ])));
    for dialect in DIALECTS {
        let groups = run.path("groups");
        let _ = std::fs::remove_file(&groups);
        let words = first_message(dialect, "go");
        let played = run.play(
            dialect,
            &strs(&words),
            &[],
            &[("FAKE_HARNESS_GROUPS", groups.to_str().expect("utf-8"))],
        );
        let said = std::fs::read_to_string(&groups).unwrap_or_default();
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "{dialect}: {said:?} {:#?}",
            played.transcript
        );
        assert!(
            lines[0].parse::<u32>().is_ok_and(|g| g > 1),
            "{dialect}: {said:?}"
        );
    }
}

#[test]
#[cfg_attr(target_os = "linux", ignore = "bisecting #7304")]
fn a_tool_command_fails_loudly_where_its_group_cannot_be_said() {
    let run = Run::new();
    run.script(&covering_like(json!([
        {"step": "bash", "tool": "bash", "run": "echo hello"},
    ])));
    // A folder where the file should be: opening it for append fails.
    let groups = run.path("groups-is-a-folder");
    std::fs::create_dir_all(&groups).expect("a folder");
    for dialect in DIALECTS {
        let words = first_message(dialect, "go");
        let played = run.play(
            dialect,
            &strs(&words),
            &[],
            &[("FAKE_HARNESS_GROUPS", groups.to_str().expect("utf-8"))],
        );
        let result = played
            .transcript
            .iter()
            .find(|l| l["kind"] == "tool_result" && l["step"] == "bash")
            .unwrap_or_else(|| panic!("{dialect}: no result {:#?}", played.transcript));
        assert!(
            result["stderr"]
                .as_str()
                .is_some_and(|e| e.contains("FAKE_HARNESS_GROUPS")),
            "{dialect}: {result}"
        );
        assert_eq!(result["exit"], 70, "{dialect}: {result}");
    }
}
