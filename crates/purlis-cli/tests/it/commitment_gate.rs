// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The commitment gate on `userpromptsubmit` (charter#369): a prompt that asks for work AND
//! leaves a real fork open is told to scout and then ask before it builds. Everything else is
//! silent, because a nudge that fires on a lookup is one the reader learns to skip. It is
//! considered only on a chat's first prompt and on a later one that asks for a code change
//! (#1988), never on a short reply or the answer to a question.
//!
//! Run the way a harness runs it: `charter hook userpromptsubmit` in a plane, the payload on
//! stdin.

use crate::support;

use std::process::Stdio;

fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join("charter.toml"), "schema = 1\n").expect("a manifest");
    dir
}

/// `charter hook userpromptsubmit` in `dir` with `payload` on stdin, and no app around it.
fn prompt_in(dir: &std::path::Path, payload: &serde_json::Value) -> String {
    prompt_as(dir, payload, &[])
}

/// The same, with `env` set as well — the app's chat number, say.
fn prompt_as(dir: &std::path::Path, payload: &serde_json::Value, env: &[(&str, &str)]) -> String {
    let mut child = support::purlis()
        .args(["hook", "userpromptsubmit"])
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("charter runs");
    stand_in::feed(&mut child, payload.to_string().as_bytes());
    let out = child.wait_with_output().expect("charter finishes");
    assert_eq!(out.status.code(), Some(0));
    String::from_utf8(out.stdout).expect("UTF-8")
}

fn asking(prompt: &str) -> serde_json::Value {
    in_session(prompt, "s-1")
}

/// `prompt` as harness session `session` hands it.
fn in_session(prompt: &str, session: &str) -> serde_json::Value {
    serde_json::json!({"session_id": session, "prompt": prompt, "permission_mode": "default"})
}

fn context_of(stdout: &str) -> String {
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("one line of JSON");
    assert_eq!(
        doc["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    doc["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context")
        .to_owned()
}

const FORKED: &str = "refactor the importer so it is maybe cleaner across every repo";

/// Forked work that does not open with its ask: read on a session's first prompt only.
const OPENS_WITH_MAYBE: &str = "maybe refactor the importer so it is cleaner across every repo";

/// A design answer in a grill, as the ruling quotes it, that reads as forked work by the
/// fork signals alone: an action, a fork, and a symptom word.
const DESIGN_ANSWER: &str =
    "agree at all, but I think the error dialog should add a retry across every repo";

#[test]
fn work_with_a_fork_in_it_is_told_to_scout_and_ask_before_building() {
    let plane = a_plane();

    let context = context_of(&prompt_in(plane.path(), &asking(FORKED)));

    assert!(context.contains("Commitment point"), "{context}");
    assert!(context.contains("open-ended wording"), "{context}");
    assert!(context.contains("broad scope"), "{context}");
    assert!(context.contains("Scout first"), "{context}");
}

#[test]
fn a_lookup_or_work_with_no_fork_says_nothing() {
    let plane = a_plane();

    for quiet in [
        "why is the importer slow? maybe the cache",
        "fix the typo on line 4",
        "what does refactor mean across every repo",
        "/review maybe refactor everything",
    ] {
        assert_eq!(prompt_in(plane.path(), &asking(quiet)), "", "{quiet}");
    }
}

#[test]
fn it_is_quiet_for_the_next_prompts_while_the_answer_is_being_worked_out() {
    let plane = a_plane();
    assert!(!prompt_in(plane.path(), &asking(FORKED)).is_empty());

    for _ in 0..3 {
        assert_eq!(prompt_in(plane.path(), &asking(FORKED)), "");
    }
    assert!(
        !prompt_in(plane.path(), &asking(FORKED)).is_empty(),
        "due again"
    );
}

#[test]
fn the_quiet_is_counted_in_prompts_of_any_kind() {
    // The answers to its own question are what the quiet is for, and they seldom look like a
    // fork: "the second option", "yes". Three of them spend it.
    let plane = a_plane();
    assert!(!prompt_in(plane.path(), &asking(FORKED)).is_empty());

    for answer in ["the second option", "yes", "go on"] {
        assert_eq!(prompt_in(plane.path(), &asking(answer)), "", "{answer}");
    }
    assert!(
        !prompt_in(plane.path(), &asking(FORKED)).is_empty(),
        "a new fork after the quiet is told"
    );
}

#[test]
fn a_sessions_first_prompt_is_read_for_a_fork_whatever_it_opens_with() {
    let plane = a_plane();

    let context = context_of(&prompt_in(plane.path(), &asking(OPENS_WITH_MAYBE)));

    assert!(context.contains("Commitment point"), "{context}");
}

#[test]
fn a_design_answer_on_a_later_turn_says_nothing() {
    let plane = a_plane();
    // The same words open a fresh session with the gate's advice, so the silence below is the
    // later-turn rule and nothing else.
    assert!(
        !prompt_in(plane.path(), &in_session(DESIGN_ANSWER, "fresh")).is_empty(),
        "a first prompt"
    );
    assert!(
        !prompt_in(plane.path(), &in_session(OPENS_WITH_MAYBE, "fresh-2")).is_empty(),
        "a first prompt"
    );
    assert_eq!(
        prompt_in(plane.path(), &asking("let us grill the importer")),
        ""
    );

    for answer in [
        DESIGN_ANSWER,
        OPENS_WITH_MAYBE,
        "agree at all",
        "yes",
        "the second option, maybe across repos",
    ] {
        assert_eq!(prompt_in(plane.path(), &asking(answer)), "", "{answer}");
    }
}

#[test]
fn a_design_answer_as_the_first_prompt_is_still_read_for_a_fork() {
    // The first prompt is read whatever it opens with; only later prompts must ask for a
    // code change. The same words, second, are silent (above).
    let plane = a_plane();

    let context = context_of(&prompt_in(plane.path(), &asking(DESIGN_ANSWER)));

    assert!(context.contains("Commitment point"), "{context}");
}

#[test]
fn a_later_prompt_that_asks_for_a_code_change_is_told() {
    let plane = a_plane();
    assert_eq!(prompt_in(plane.path(), &asking("hello")), "");

    let context = context_of(&prompt_in(plane.path(), &asking(FORKED)));

    assert!(context.contains("Commitment point"), "{context}");
}

#[test]
fn each_session_has_its_own_first_prompt() {
    let plane = a_plane();
    assert_eq!(prompt_in(plane.path(), &asking("hello")), "");

    assert!(
        !prompt_in(plane.path(), &in_session(DESIGN_ANSWER, "s-2")).is_empty(),
        "s-2's first prompt"
    );
    assert_eq!(
        prompt_in(plane.path(), &asking(DESIGN_ANSWER)),
        "",
        "s-1's second"
    );
}

#[cfg(unix)]
#[test]
fn a_session_whose_record_cannot_be_written_is_past_its_first_prompt() {
    use std::os::unix::fs::PermissionsExt;
    let plane = a_plane();
    let gate = plane.path().join(".charter").join("commit-gate");
    std::fs::create_dir_all(&gate).expect("the gate's folder");
    std::fs::set_permissions(&gate, std::fs::Permissions::from_mode(0o500)).expect("read-only");
    if std::fs::write(gate.join("probe"), b"").is_ok() {
        // Run as a user the mode does not bind (root in a container): nothing to show here.
        std::fs::set_permissions(&gate, std::fs::Permissions::from_mode(0o700)).unwrap();
        return;
    }

    let design = prompt_in(plane.path(), &asking(DESIGN_ANSWER));
    let ask = prompt_in(plane.path(), &asking(FORKED));
    std::fs::set_permissions(&gate, std::fs::Permissions::from_mode(0o700)).expect("writable");

    // DESIGN_ANSWER is told as a first prompt where the record can be kept (above).
    assert_eq!(design, "", "read as a later prompt");
    let context = context_of(&ask);
    assert!(context.contains("Commitment point"), "{context}");
}

#[test]
fn an_unattended_run_is_never_told_to_ask_anyone() {
    let plane = a_plane();
    let mut payload = asking(FORKED);
    payload["permission_mode"] = "bypassPermissions".into();

    assert_eq!(prompt_in(plane.path(), &payload), "");
}

#[test]
fn outside_a_plane_it_says_nothing() {
    let dir = tempfile::tempdir().expect("a directory");

    assert_eq!(prompt_in(dir.path(), &asking(FORKED)), "");
}

#[test]
fn a_symptom_is_told_to_diagnose_rather_than_to_design() {
    let plane = a_plane();

    let context = context_of(&prompt_in(
        plane.path(),
        &asking("fix the importer crash, maybe it's the cache"),
    ));

    assert!(context.contains("symptom to diagnose"), "{context}");
    assert!(context.contains("failing test"), "{context}");
}

#[test]
fn the_gate_and_a_report_back_arrive_as_one_context() {
    use purlis_core::handback::{self, For, Handback};
    let plane = a_plane();
    let report = Handback {
        from: "drop commons".to_owned(),
        from_workspace: purlis_core::active::Place::Workspace("platform-next".to_owned()),
        to: "steward 3".to_owned(),
        to_workspace: purlis_core::active::Place::Workspace("ops".to_owned()),
        summary: "Dropped it.".to_owned(),
        task: None,
        answered: None,
        stopped: None,
    };
    handback::leave(plane.path(), For::Chat(3), &report).unwrap();

    let out = prompt_as(
        plane.path(),
        &asking(FORKED),
        &[(purlis_core::hookwire::CHAT_ENV, "3")],
    );

    assert_eq!(out.lines().count(), 1, "{out}");
    let context = context_of(&out);
    assert!(context.contains("Commitment point"), "{context}");
    assert!(context.ends_with("> Dropped it."), "{context}");
}
