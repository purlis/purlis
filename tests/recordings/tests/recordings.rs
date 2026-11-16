// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The recordings, the mask and the drift diff** (T1.1-03, #6850): the recorder writes every
//! hook call; the mask hides only its closed list; every pinned version has a recording; each
//! recording keeps what the model was shown; and the nightly re-record's diff opens an issue on
//! drift and fails on a refused call that changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use purlis_recordings::drift;
use purlis_recordings::mask::{differences, mask, secrets};
use purlis_recordings::projection::{Api, project};
use purlis_recordings::recorder;
use purlis_recordings::recording::{self, HARNESSES, Recording};
use serde_json::{Value, json};

// ---- the recorder and the mask -----------------------------------------------------------

/// Runs the recorder for one hook call, as a harness runs its hook entry: `stdin` on its input,
/// the chat's variables in its environment, a real `purlis` that answers with `answer`.
#[cfg(unix)]
fn hook_call(dir: &Path, word: &str, stdin: &str, extra: &[(&str, &str)]) -> (i32, String) {
    let real = stand_in::program(
        dir,
        "purlis",
        "#!/bin/sh\ninput=$(cat)\nprintf '{\"decision\":\"approve\",\"word\":\"%s\",\"read\":%s}' \"$2\" \"${#input}\"\n",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_purlis-recorder"))
        .args(["hook", word])
        .env(recorder::REAL, &real)
        .env(recorder::OUT, dir.join("recorder.jsonl"))
        .env("PURLIS_CHAT", "1")
        .env("PURLIS_CHAT_TOKEN", "a-fresh-token-every-chat")
        .env("PURLIS_HOOK_SOCKET", dir.join(".purlis/app/hooks.sock"))
        .env("UNRELATED_MACHINE_VARIABLE", "kept out")
        // The run's own, under purlis's other spelling too: never a chat's.
        .env("CHARTER_RECORDER_OUT", "kept out")
        .env("PURLIS_RECORD_HARNESS", "kept out")
        .env("CHARTER_PLANE_FENCE", "kept out")
        .envs(extra.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the recorder runs");
    stand_in::feed(&mut child, stdin.as_bytes());
    let output = child.wait_with_output().expect("the recorder ends");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[cfg(unix)]
#[test]
fn the_recorder_writes_every_hook_call_and_masks_only_the_closed_list() {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("the folder");
    let first = json!({
        "session_id": "5c1e0f2a-0000-4000-8000-000000000001",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_use_id": "toolu_01ABCDEF",
        "tool_input": {"command": format!("cat {}/notes.txt", root.display())},
        "cwd": root.display().to_string(),
        "timestamp": "2026-11-16T04:39:00.123Z",
    });
    let second = json!({
        "session_id": "5c1e0f2a-0000-4000-8000-000000000001",
        "hook_event_name": "Stop",
        // A credential the harness handed a hook: it survives the mask, and fails the recording.
        // Assembled here so no key-shaped text is committed.
        "transcript_note": format!("ANTHROPIC_API_KEY=sk-{}-api03-{}", "ant", "abcdefghijklmnopqrstuvwxyz0123"),
    });
    let (code, answered) = hook_call(&root, "pretooluse", &first.to_string(), &[]);
    assert_eq!(code, 0);
    assert!(answered.contains("\"word\":\"pretooluse\""), "{answered}");
    let (code, _) = hook_call(&root, "stop", &second.to_string(), &[]);
    assert_eq!(code, 0);
    // A call the recorder answers itself with exit 2 says so.
    let (code, _) = hook_call(
        &root,
        "userpromptsubmit",
        "{}",
        &[(recorder::EXIT2, "userpromptsubmit")],
    );
    assert_eq!(code, 2);

    // Every hook call, each with what it was handed and what it answered.
    let lines = recorder::read(&root.join("recorder.jsonl")).expect("the recorder's file");
    let calls = recording::hook_calls(&lines, &[]);
    assert_eq!(calls.len(), 3, "{lines:#?}");
    assert_eq!(calls[0].argv, ["hook", "pretooluse"]);
    assert_eq!(calls[0].stdin, first);
    // Every variable's name, whoever set it, and no value at all.
    for name in [
        "PURLIS_CHAT",
        "PURLIS_CHAT_TOKEN",
        "UNRELATED_MACHINE_VARIABLE",
        "PURLIS_RECORD_HARNESS",
        "CHARTER_PLANE_FENCE",
    ] {
        assert_eq!(
            calls[0].env.get(name).map(String::as_str),
            Some(recorder::WITHHELD),
            "{name}"
        );
    }
    // But the recorder's own, which tell it what to do.
    for name in [recorder::REAL, recorder::OUT, "CHARTER_RECORDER_OUT"] {
        assert!(
            !calls[0].env.contains_key(name),
            "{name} is the recorder's own"
        );
    }
    assert_eq!(calls[0].reply.as_ref().expect("a reply").exit, 0);
    assert!(calls[0].gap_ms.is_null());
    assert!(calls[1].gap_ms.is_u64());
    assert_eq!(calls[2].forced.as_deref(), Some("exit2"));
    assert_eq!(calls[2].reply.as_ref().expect("a reply").exit, 2);

    let made = |calls| Recording {
        harness: "claude".into(),
        version: "2.1.325".into(),
        level: "full".into(),
        os: "linux".into(),
        conditions: Default::default(),
        script: "covering".into(),
        prompts: vec!["COVERING-SCRIPT".into()],
        forced: Default::default(),
        roots: [("work".to_owned(), root.display().to_string())].into(),
        hook_calls: calls,
        model: vec![],
        shown: vec![],
    };
    let masked = recording::masked(&made(calls.clone()));
    // The closed list is masked: session and tool-use ids, absolute paths, times, gaps.
    let stdin = &masked["hook_calls"][0]["stdin"];
    assert_eq!(stdin["session_id"], "<session-1>");
    assert_eq!(stdin["tool_use_id"], "<tool-use-1>");
    assert_eq!(stdin["cwd"], "<work>");
    assert_eq!(stdin["tool_input"]["command"], "cat <work>/notes.txt");
    assert_eq!(stdin["timestamp"], "<time>");
    assert_eq!(masked["hook_calls"][1]["gap_ms"], "<gap>");
    assert!(masked.get("roots").is_none());
    // And nothing else: the secret-shaped value survives, as a failure of the recording.
    assert!(
        masked["hook_calls"][1]["stdin"]["transcript_note"]
            .as_str()
            .is_some_and(|t| t.contains("sk-ant-api03-")),
        "the mask hid a credential"
    );
    let problems = recording::problems(&masked);
    assert!(
        problems.iter().any(|p| p.contains("an Anthropic key")),
        "{problems:?}"
    );
    // A changed field survives too: the same calls with one command changed are not the same
    // recording.
    let mut changed = calls.clone();
    changed[0].stdin["tool_input"]["command"] = json!(format!("cat {}/other.txt", root.display()));
    let found = differences(&masked, &recording::masked(&made(changed)));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("/hook_calls/0/stdin/tool_input/command"),
        "{found:?}"
    );
    // While another run's ids, paths and times are the same recording.
    let other = tempfile::tempdir().expect("another folder");
    let other_root = other.path().canonicalize().expect("the folder");
    let mut rerun = calls;
    for call in &mut rerun {
        let text = call
            .stdin
            .to_string()
            .replace(
                &root.display().to_string(),
                &other_root.display().to_string(),
            )
            .replace(
                "5c1e0f2a-0000-4000-8000-000000000001",
                "99999999-0000-4000-8000-000000000009",
            )
            .replace("toolu_01ABCDEF", "toolu_99ZZZZ")
            .replace("04:39:00.123", "23:01:59.999");
        call.stdin = serde_json::from_str(&text).expect("JSON");
        call.gap_ms = call
            .gap_ms
            .as_u64()
            .map_or(Value::Null, |g| json!(g + 1234));
        for value in call.env.values_mut() {
            *value = value.replace(
                &root.display().to_string(),
                &other_root.display().to_string(),
            );
        }
    }
    let mut again = made(rerun.clone());
    again.roots = [("work".to_owned(), other_root.display().to_string())].into();
    assert_eq!(
        differences(&masked, &recording::masked(&again)),
        Vec::<String>::new()
    );
    // A variable the hook was not handed before is a difference too, by its name.
    let mut more = rerun;
    more[0]
        .env
        .insert("PURLIS_NEW_VARIABLE".into(), recorder::WITHHELD.into());
    let mut more = made(more);
    more.roots = [("work".to_owned(), other_root.display().to_string())].into();
    assert_eq!(
        differences(&masked, &recording::masked(&more)),
        ["/hook_calls/0/env/PURLIS_NEW_VARIABLE: (absent) != \"<withheld>\""]
    );

    // Hooks of different events that merely overlapped keep the order they began in, ungrouped,
    // and the mask leaves it: OpenCode's real "Stop, then PostToolUse".
    let line = |id: &str, phase: &str, at_ms: u64, stdin: Value| recorder::Line {
        id: id.into(),
        phase: phase.into(),
        at_ms,
        argv: vec!["hook".into(), id.into()],
        stdin: (phase == "call").then(|| stdin.to_string()),
        env: None,
        exit: (phase == "reply").then_some(0),
        stdout: None,
        stderr: None,
        forced: None,
    };
    let bash = json!({"tool_name": "Bash", "tool_input": {"command": "sleep 120"}});
    let post = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash",
        "tool_input": bash["tool_input"]});
    let mixed = [
        line("stop", "call", 100, json!({"hook_event_name": "Stop"})),
        line("posttooluse", "call", 101, post.clone()),
        line("stop", "reply", 150, Value::Null),
        line("posttooluse", "reply", 151, Value::Null),
    ];
    let calls = recording::hook_calls(&mixed, &[]);
    assert!(calls.iter().all(|c| c.group.is_none()), "{calls:#?}");
    let masked = recording::masked(&made(calls));
    let order: Vec<&str> = masked["hook_calls"]
        .as_array()
        .expect("calls")
        .iter()
        .map(|c| c["argv"][1].as_str().expect("a word"))
        .collect();
    assert_eq!(order, ["stop", "posttooluse"]);
    assert_eq!(recording::overlapping(&mixed), Vec::<String>::new());
    // While a PreToolUse and a PostToolUse of one tool use that overlap, a harness that did not
    // wait for the verdict, fail the recording outright.
    let mut pre = bash.clone();
    pre["hook_event_name"] = json!("PreToolUse");
    let unwaited = [
        line("pretooluse", "call", 100, pre),
        line("posttooluse", "call", 120, post),
        line("pretooluse", "reply", 150, Value::Null),
        line("posttooluse", "reply", 151, Value::Null),
    ];
    let found = recording::overlapping(&unwaited);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].contains("did not wait for the verdict"),
        "{found:?}"
    );
}

// ---- every pinned version has a recording --------------------------------------------------

#[test]
fn a_version_file_without_a_recording_fails_the_check() {
    let dir = tempfile::tempdir().expect("a folder");
    // Every pinned version recorded but codex's.
    for harness in HARNESSES.iter().filter(|h| **h != "codex") {
        let version = recording::pinned_version(harness).expect("a pinned version");
        let folder = dir.path().join(harness).join(&version);
        fs::create_dir_all(&folder).expect("a folder");
        fs::write(folder.join("recording.json"), "{}").expect("a recording");
        fs::write(folder.join("turns.json"), "{}").expect("a turn script");
    }
    let missing = recording::versions_without_a_recording(dir.path());
    let codex = recording::pinned_version("codex").expect("codex's version");
    assert_eq!(missing.len(), 2, "{missing:#?}");
    assert!(
        missing
            .iter()
            .all(|m| m.contains("codex.txt") && m.contains(&codex)),
        "{missing:#?}"
    );
}

#[test]
fn every_pinned_version_has_a_recording_and_a_turn_script() {
    let missing =
        recording::versions_without_a_recording(&purlis_recordings::root().join("recorded"));
    assert!(
        missing.is_empty(),
        "record each with CI's harness recording job and check it in:\n{}",
        missing.join("\n")
    );
}

/// Every checked-in recording, with its path.
fn checked_in() -> Vec<(PathBuf, Value)> {
    let mut found = Vec::new();
    for harness in HARNESSES {
        let dir = purlis_recordings::root().join("recorded").join(harness);
        let Ok(versions) = fs::read_dir(&dir) else {
            continue;
        };
        for version in versions {
            let mut files: Vec<PathBuf> = fs::read_dir(version.expect("an entry").path())
                .expect("a version's folder")
                .map(|f| f.expect("a file").path())
                .filter(|p| recording::is_recording(p))
                .collect();
            files.sort();
            for path in files {
                let text = fs::read_to_string(&path).expect("the recording reads");
                found.push((
                    path,
                    serde_json::from_str(&text).expect("the recording is JSON"),
                ));
            }
        }
    }
    found
}

/// The covering recording of each harness's pinned version.
fn covering(harness: &str) -> Recording {
    let version = recording::pinned_version(harness).expect("a pinned version");
    let path = recording::folder(harness, &version).join("recording.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("the recording")).expect("a recording")
}

/// The hook words a recording heard, in order.
fn words(recording: &Recording) -> Vec<&str> {
    recording
        .hook_calls
        .iter()
        .filter_map(|c| c.argv.get(1).map(String::as_str))
        .collect()
}

#[test]
fn each_recording_covers_what_the_ticket_asks_of_it() {
    // A turn that ended normally: its Stop, for each harness that has one.
    for harness in ["claude", "codex"] {
        let made = covering(harness);
        assert!(
            words(&made).contains(&"stop"),
            "{harness}: no Stop: {:?}",
            words(&made)
        );
        assert_eq!(made.prompts.len(), 2, "{harness}: a second turn");
    }
    // purlis's SessionStart context reaches the model: the stall is held elsewhere.
    for harness in HARNESSES {
        let made = covering(harness);
        assert_ne!(
            made.forced.get("stall").map(String::as_str),
            Some("sessionstart")
        );
        assert!(
            made.hook_calls.iter().any(|c| c
                .argv
                .get(1)
                .is_some_and(|w| w.starts_with("sessionstart"))
                && c.reply.is_some()),
            "{harness}: no SessionStart answered"
        );
        // What purlis's SessionStart answered reaches the model, wherever the harness puts it.
        let context: Vec<String> = made
            .hook_calls
            .iter()
            .filter(|c| c.argv.get(1).is_some_and(|w| w.starts_with("sessionstart")))
            .filter_map(|c| {
                c.reply.as_ref()?.stdout["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .and_then(|t| t.lines().next())
                    .map(str::to_owned)
            })
            .collect();
        assert!(
            !context.is_empty(),
            "{harness}: SessionStart gave no context"
        );
        for line in &context {
            assert!(
                made.shown.iter().any(|s| s.text.contains(line.as_str())),
                "{harness}: the model was not shown purlis's SessionStart context {line:?}"
            );
        }
        assert_eq!(
            made.conditions.get("summary").map(String::as_str),
            Some(recording::FULL_BYPASSED),
            "{harness}: the recording's header says what it was made under"
        );
    }
    // Codex writes with its native apply_patch, and its hooks hear that.
    let codex = covering("codex");
    for step in ["refused-write", "refused-edit"] {
        let call = codex
            .model
            .iter()
            .flat_map(|a| &a.calls)
            .find(|c| c.step == step)
            .unwrap_or_else(|| panic!("codex has no {step}"));
        assert_eq!(call.tool, "apply_patch", "{step}");
    }
    // Claude Code's own prompts, in a pass that keeps them: its PermissionRequest and its
    // Notification.
    let version = recording::pinned_version("claude").expect("a version");
    let asks: Recording = serde_json::from_str(
        &fs::read_to_string(recording::folder("claude", &version).join("asks-recording.json"))
            .expect("Claude Code's asks recording"),
    )
    .expect("a recording");
    assert_eq!(
        asks.conditions.get("permission_mode").map(String::as_str),
        Some("default")
    );
    for word in ["permissionrequest", "notification"] {
        assert!(
            words(&asks).contains(&word),
            "no {word}: {:?}",
            words(&asks)
        );
    }
}

#[test]
fn every_checked_in_recording_is_masked_and_reproduced_by_its_turn_script() {
    for (path, value) in checked_in() {
        assert_eq!(mask(&value), value, "{} is not masked", path.display());
        assert_eq!(
            recording::problems(&value),
            Vec::<String>::new(),
            "{}",
            path.display()
        );
        let parsed: Recording = serde_json::from_value(value).expect("a recording");
        let turns: Value = serde_json::from_str(
            &fs::read_to_string(recording::turns_beside(&path)).expect("its turn script"),
        )
        .expect("JSON");
        assert_eq!(
            turns,
            serde_json::to_value(purlis_recordings::turns::of(&parsed)).expect("JSON"),
            "{}'s turn script is not the one it makes",
            path.display()
        );
    }
}

// ---- what the model is shown ---------------------------------------------------------------

#[test]
fn each_recording_keeps_the_models_received_requests() {
    // The projection, in each API: the user's text, hook context, tool results with their deny
    // reasons; never the system prompt or the tool schemas.
    let messages = json!({
        "system": [{"type": "text", "text": "HARNESS SYSTEM PROMPT"}],
        "tools": [{"name": "Bash", "description": "TOOL SCHEMA"}],
        "messages": [
            {"role": "user", "content": [{"type": "text",
                "text": "<system-reminder>purlis: SessionStart context</system-reminder>\nCOVERING-SCRIPT go"}]},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "Bash", "input": {}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1",
                "is_error": true, "content": "purlis refused: the sandbox does not list refused.example.invalid"}]}
        ]
    });
    let responses = json!({
        "instructions": "HARNESS SYSTEM PROMPT",
        "tools": [{"type": "function", "name": "shell_command"}],
        "input": [
            {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": "HARNESS SYSTEM PROMPT, as Codex sends it"}]},
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "<environment_context>"}]},
            {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": "hook context"}]},
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "COVERING-SCRIPT go"}]},
            {"type": "function_call", "call_id": "call_1", "name": "shell_command", "arguments": "{}"},
            {"type": "function_call_output", "call_id": "call_1", "output": "refused by a hook"}
        ]
    });
    let chat = json!({
        "tools": [{"type": "function", "function": {"name": "bash"}}],
        "messages": [
            {"role": "system", "content": "HARNESS SYSTEM PROMPT"},
            {"role": "user", "content": [{"type": "text", "text": "COVERING-SCRIPT go"}]},
            {"role": "assistant", "tool_calls": [{"id": "call_1"}]},
            {"role": "tool", "tool_call_id": "call_1", "content": "Error: refused"}
        ]
    });
    for (api, body, context) in [
        (
            Api::Messages,
            &messages,
            Some("purlis: SessionStart context"),
        ),
        (Api::Responses, &responses, Some("hook context")),
        (Api::ChatCompletions, &chat, None),
    ] {
        let shown = project(api, body);
        let text = serde_json::to_string(&shown).expect("JSON");
        assert!(!text.contains("HARNESS SYSTEM PROMPT"), "{api:?}: {text}");
        assert!(!text.contains("TOOL SCHEMA"), "{api:?}: {text}");
        assert!(
            shown
                .iter()
                .any(|s| s.kind == "user_text" && s.text == "COVERING-SCRIPT go"),
            "{api:?}: {shown:#?}"
        );
        assert!(
            shown
                .iter()
                .any(|s| s.kind == "tool_result" && s.text.contains("refused")),
            "{api:?}: {shown:#?}"
        );
        if let Some(context) = context {
            assert!(
                shown
                    .iter()
                    .any(|s| s.kind == "context" && s.text == context),
                "{api:?}: {shown:#?}"
            );
        }
    }
    // And each checked-in recording keeps it, masked: every prompt it was given, and a result
    // for every refused call the harness made; never what the refused read aimed at.
    let recordings = checked_in();
    assert!(
        recordings.len() > HARNESSES.len(),
        "a covering recording for each harness, and Claude Code's asks pass"
    );
    for (path, value) in recordings {
        let parsed: Recording = serde_json::from_value(value.clone()).expect("a recording");
        for prompt in &parsed.prompts {
            assert!(
                parsed
                    .shown
                    .iter()
                    .any(|s| s.kind == "user_text" && s.text.contains(prompt.as_str())),
                "{} keeps no {prompt:?}: {:#?}",
                path.display(),
                parsed.shown
            );
        }
        assert!(
            !value.to_string().contains("COVERING-VAULT-MARKER"),
            "{}: the refused vault's contents reached the model",
            path.display()
        );
        for refused in parsed
            .model
            .iter()
            .flat_map(|a| &a.calls)
            .filter(|c| c.step.starts_with("refused-"))
        {
            assert!(
                parsed
                    .shown
                    .iter()
                    .any(|s| s.tool_use_id.as_deref() == Some(refused.tool_use_id.as_str())),
                "{}: the model was shown no result of {}",
                path.display(),
                refused.step
            );
        }
        assert!(secrets(&value).is_empty(), "{}", path.display());
    }
}

// ---- the nightly re-record's diff ----------------------------------------------------------

fn sample() -> Value {
    mask(&json!({
        "harness": "codex", "version": "0.169.0", "level": "full", "os": "linux",
        "conditions": {}, "script": "covering", "prompts": ["COVERING-SCRIPT"],
        "hook_calls": [
            {"argv": ["hook", "pretooluse"], "stdin": {"tool_name": "Bash", "call_id": "call_cs_1_0",
                "tool_input": {"command": "echo covered"}}, "env": {}, "gap_ms": null,
                "reply": {"exit": 0, "stdout": "", "stderr": ""}},
            {"argv": ["hook", "pretooluse"], "stdin": {"tool_name": "Bash", "call_id": "call_cs_2_0",
                "tool_input": {"command": "curl https://refused.example.invalid/"}}, "env": {},
                "gap_ms": 3, "reply": {"exit": 0, "stdout": "", "stderr": ""}}
        ],
        "model": [
            {"turn": 1, "calls": [{"step": "bash", "tool": "shell_command",
                "input": {"command": "echo covered"}, "tool_use_id": "call_cs_1_0"}]},
            {"turn": 1, "calls": [{"step": "refused-bash-net", "tool": "shell_command",
                "input": {"command": "curl https://refused.example.invalid/"}, "tool_use_id": "call_cs_2_0"}]}
        ],
        "shown": [
            {"kind": "tool_result", "text": "covered", "tool_use_id": "call_cs_1_0"},
            {"kind": "tool_result", "text": "curl: (56) CONNECT tunnel failed, response 403", "tool_use_id": "call_cs_2_0"}
        ]
    }))
}

#[test]
fn the_nightly_rerecord_diff_opens_an_issue_on_drift() {
    let checked_in = sample();
    // The same recording: nothing to open.
    let same = drift::compare(&checked_in, &sample());
    assert_eq!(same, drift::Diff::default());
    assert_eq!(
        drift::issue("codex", "0.169.0", "pinned", &same, "run"),
        None
    );

    // One field changed outside any refused call: drift, and an issue labelled harness-drift.
    let mut drifted = sample();
    drifted["hook_calls"][0]["stdin"]["tool_input"]["command"] = json!("echo moved");
    let diff = drift::compare(&checked_in, &drifted);
    assert_eq!(diff.escape, Vec::<String>::new());
    assert_eq!(diff.drift.len(), 1, "{diff:#?}");
    let issue =
        drift::issue("codex", "0.170.0", "latest", &diff, "https://ci/run/1").expect("an issue");
    assert!(issue.labels.contains(&drift::DRIFT_LABEL.to_owned()));
    assert!(
        issue.title.contains("codex 0.170.0 (latest)"),
        "{}",
        issue.title
    );
    assert!(
        issue
            .body
            .contains("/hook_calls/0/stdin/tool_input/command"),
        "{}",
        issue.body
    );
    let commands = drift::gh_commands(&issue, "purlis/purlis");
    assert!(
        commands.iter().any(|c| c[..2] == ["issue", "create"]
            && c.contains(&"harness-drift,area:harness".to_owned())),
        "{commands:#?}"
    );

    // A refused call whose result changed is an escape, never drift: no drift issue.
    let mut escaped = sample();
    escaped["shown"][1]["text"] = json!("<!doctype html> the page");
    let diff = drift::compare(&checked_in, &escaped);
    assert_eq!(diff.drift, Vec::<String>::new());
    assert_eq!(diff.escape.len(), 1, "{diff:#?}");
    assert_eq!(
        drift::issue("codex", "0.169.0", "pinned", &diff, "run"),
        None
    );

    // OpenCode's hooks are handed no tool-use id: its refused read is found by what it aimed at,
    // wherever it stands. A refused read that comes back allowed is an escape, never drift.
    let opencode = |allowed: bool| {
        let read = json!({"filePath": "<work>/.purlis/vaults/covering.json"});
        let pre = json!({"argv": ["hook", "pretooluse-read"], "stdin": {"hook_event_name": "PreToolUse",
            "tool_name": "Read", "tool_input": {"file_path": "<work>/.purlis/vaults/covering.json"}},
            "env": {}, "gap_ms": "<gap>", "reply": {"exit": 0, "stdout": if allowed {
                json!({})
            } else {
                json!({"hookSpecificOutput": {"permissionDecision": "deny"}})
            }, "stderr": ""}});
        let mut calls = vec![
            json!({"argv": ["hook", "sessionstart"], "stdin": {"hook_event_name": "SessionStart"},
                "env": {}, "gap_ms": "<gap>", "reply": {"exit": 0, "stdout": "", "stderr": ""}}),
            pre,
        ];
        if allowed {
            // A PostToolUse heard first, before the session's other hooks.
            calls.insert(
                0,
                json!({"argv": ["hook", "posttooluse"], "stdin": {
                "hook_event_name": "PostToolUse", "tool_name": "Read",
                "tool_input": {"file_path": "<work>/.purlis/vaults/covering.json"}},
                "env": {}, "gap_ms": "<gap>", "reply": {"exit": 0, "stdout": "", "stderr": ""}}),
            );
        }
        mask(&json!({
            "harness": "opencode", "version": "1.19.5", "level": "full", "os": "linux",
            "conditions": {}, "script": "covering", "prompts": ["COVERING-SCRIPT"],
            "hook_calls": calls,
            "model": [{"turn": 1, "calls": [{"step": "refused-read", "tool": "read",
                "input": read, "tool_use_id": "call_cs_6_0"}]}],
            "shown": [{"kind": "tool_result", "tool_use_id": "call_cs_6_0", "text": if allowed {
                "the vault"
            } else {
                "Error: purlis refused the read"
            }}]
        }))
    };
    let diff = drift::compare(&opencode(false), &opencode(true));
    assert_eq!(diff.drift, Vec::<String>::new(), "{diff:#?}");
    assert!(
        diff.escape
            .iter()
            .any(|d| d.contains("/refused/refused-read/hook_calls")),
        "{diff:#?}"
    );
    assert!(
        diff.escape.iter().any(|d| d.contains("permissionDecision")),
        "{diff:#?}"
    );

    // A later night's comment on the open drift issue says what differs now.
    let mut drifted = sample();
    drifted["hook_calls"][0]["stdin"]["tool_input"]["command"] = json!("echo moved");
    let diff = drift::compare(&checked_in, &drifted);
    let comment = drift::comment(&diff, "https://ci/run/2");
    assert!(
        comment.starts_with("Drifted again: https://ci/run/2"),
        "{comment}"
    );
    assert!(
        comment.contains("/hook_calls/0/stdin/tool_input/command"),
        "{comment}"
    );

    // The binary the nightly runs, dry, in each of its two entries' modes.
    let dir = tempfile::tempdir().expect("a folder");
    let write = |name: &str, value: &Value| {
        let path = dir.path().join(name);
        fs::write(&path, value.to_string()).expect("a recording");
        path
    };
    let (old, new, esc) = (
        write("old.json", &checked_in),
        write("new.json", &drifted),
        write("esc.json", &escaped),
    );
    let run = |fresh: &Path, extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_harness-drift"))
            .arg(&old)
            .arg(fresh)
            .args(["latest", "--dry-run"])
            .args(extra)
            .env_remove("GITHUB_REPOSITORY")
            .output()
            .expect("harness-drift runs")
    };
    // The drift entry opens an issue on drift, or comments on the open one with what differs.
    let drifted_run = run(&new, &["--mode", "drift"]);
    let said = String::from_utf8_lossy(&drifted_run.stdout);
    assert!(drifted_run.status.success(), "{said}");
    assert!(said.contains("would run: gh issue create"), "{said}");
    assert!(said.contains("harness-drift"), "{said}");
    let again = run(&new, &["--mode", "drift", "--open", "42"]);
    let said = String::from_utf8_lossy(&again.stdout);
    assert!(said.contains("would run: gh issue comment 42"), "{said}");
    assert!(
        said.contains("/hook_calls/0/stdin/tool_input/command"),
        "{said}"
    );
    // It is never red on an escape: the escape entry is.
    let escaped_run = run(&esc, &["--mode", "drift"]);
    let said = String::from_utf8_lossy(&escaped_run.stdout);
    assert!(escaped_run.status.success(), "{said}");
    assert!(!said.contains("gh issue create"), "{said}");
    // The escape entry is red on an escape alone, and opens nothing.
    let escaped_run = run(&esc, &["--mode", "escape"]);
    let said = String::from_utf8_lossy(&escaped_run.stdout);
    assert_eq!(escaped_run.status.code(), Some(1), "{said}");
    assert!(said.contains("may be an escape"), "{said}");
    let drifted_run = run(&new, &["--mode", "escape"]);
    let said = String::from_utf8_lossy(&drifted_run.stdout);
    assert!(drifted_run.status.success(), "{said}");
    assert!(!said.contains("gh issue"), "{said}");
    // A run that cannot read what it compares is an ordinary failure, not an escape.
    let unread = run(&dir.path().join("missing.json"), &["--mode", "escape"]);
    assert_eq!(unread.status.code(), Some(2));
}

// ---- Codex against the Responses stand-in --------------------------------------------------

/// **Codex runs the covering script against the Responses stand-in** (`PURLIS_CODEX`, the
/// pinned build CI installs): every response the script gives Codex is asked for, and each call
/// comes back with a result, so the stand-in speaks the Responses API as Codex reads it. Outside
/// purlis's wrap: the recording job runs it inside.
#[cfg(unix)]
#[test]
#[ignore = "runs the real Codex: CI's harness installs set PURLIS_CODEX"]
fn codex_runs_a_covering_script_against_the_responses_stand_in() {
    use purlis_recordings::script::Script;
    use purlis_recordings::standin::{self, Conversation, StandIn};
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    let codex = PathBuf::from(std::env::var("PURLIS_CODEX").expect("PURLIS_CODEX names Codex"));
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("the folder");
    let work = root.join("work");
    let home = root.join("codex-home");
    let outside = root.join("outside");
    for d in [&work, &home, &outside] {
        fs::create_dir_all(d).expect("a folder");
    }
    fs::write(outside.join("existing.txt"), "before\n").expect("a file");
    let places = [
        ("work", work.display().to_string()),
        ("outside", outside.display().to_string()),
        ("denied_read", root.join("nothing").display().to_string()),
        ("refused_url", "http://127.0.0.1:9/".to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let mut script = Script::named("covering")
        .expect("the covering script")
        .for_harness("codex", &places);
    // Outside a sandbox nothing is refused, and nothing is pressed: one turn, `codex exec`'s, and
    // no wait in it.
    script.responses.retain(|r| !r.esc);
    script.then.clear();
    let stand_in = Arc::new(StandIn::new("codex", script.clone()));
    let (tx, rx) = mpsc::channel();
    {
        let stand_in = Arc::clone(&stand_in);
        std::thread::spawn(move || {
            let _ = standin::serve(stand_in, |port| {
                let _ = tx.send(port);
            });
        });
    }
    let port = rx.recv_timeout(Duration::from_secs(10)).expect("a port");
    let output = Command::new(&codex)
        .current_dir(&work)
        .env("CODEX_HOME", &home)
        .env("OPENAI_API_KEY", "stand-in-model-no-credential")
        // Only the stand-in: every other host, any call Codex makes of its own (an update
        // check, its model catalogue, its telemetry), goes to a proxy that refuses it.
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("NO_PROXY", "127.0.0.1")
        .args([
            "exec",
            "--skip-git-repo-check",
            "--dangerously-bypass-approvals-and-sandbox",
            "-c",
            "model_provider=\"standin\"",
            "-c",
            // One its own catalogue lists with a freeform apply_patch, as the recording runs it.
            "model=\"gpt-5.5\"",
            "-c",
            "model_providers.standin.name=\"stand-in\"",
            "-c",
            &format!("model_providers.standin.base_url=\"http://127.0.0.1:{port}/v1\""),
            "-c",
            "model_providers.standin.wire_api=\"responses\"",
            "-c",
            "model_providers.standin.request_max_retries=0",
            "-c",
            "model_providers.standin.stream_max_retries=0",
            &script.prompt,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("codex runs");
    let heard = stand_in.heard.lock().expect("the log").clone();
    let said = format!(
        "status {}\nstdout:\n{}\nstderr:\n{}\nheard: {heard:#?}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let asked = stand_in.asked();
    assert_eq!(asked.len(), script.responses.len(), "{said}");
    assert!(
        heard
            .iter()
            .any(|h| h.conversation == Conversation::Main
                && h.response == Some(script.responses.len())),
        "Codex never came back for the end of the turn: {said}"
    );
    let shown = stand_in.shown();
    for call in asked.iter().flat_map(|a| &a.calls) {
        assert!(
            shown
                .iter()
                .any(|s| s.tool_use_id.as_deref() == Some(call.tool_use_id.as_str())),
            "no result of {} ({}): {said}",
            call.step,
            call.tool
        );
    }
    // Each call reached a tool Codex has: none came back as one it does not offer.
    assert!(
        !shown.iter().any(|s| s.text.contains("unsupported")),
        "a call reached no tool of Codex's: {shown:#?}"
    );
    assert_eq!(
        fs::read_to_string(work.join("covered.txt")).ok().as_deref(),
        Some("covered\n"),
        "{said}"
    );
    // Its native write is its own apply_patch, which wrote outside: nothing refuses it here.
    for call in asked
        .iter()
        .flat_map(|a| &a.calls)
        .filter(|c| c.step == "refused-write")
    {
        assert_eq!(call.tool, "apply_patch", "{said}");
    }
    assert_eq!(
        fs::read_to_string(outside.join("by-write.txt"))
            .ok()
            .as_deref(),
        Some("by-write\n"),
        "{said}"
    );
}
