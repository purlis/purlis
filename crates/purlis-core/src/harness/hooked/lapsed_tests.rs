// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #6007: a permission hook's ask that timed out stays answerable, and the answer reaches the
//! chat's same request. Time is handed in, so every test runs on a clock it moves itself.

use super::lapsed::{LAPSED_FOR, MOST_LAPSED, PRE_ANSWERED_FOR};
use super::*;
use crate::harness::asks::Admitted;

fn bash(command: &str) -> Value {
    serde_json::json!({
        "session_id": "abc",
        "hook_event_name": "PermissionRequest",
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
}

fn window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

fn approval() -> Answerer {
    Answerer::admitted(Admitted::Approval).expect("charter inbox answers")
}

fn hooks() -> HookAsks {
    HookAsks::new(Arc::new(Asks::new()))
}

/// When an ask raised at `t0` has timed out.
fn after_its_hook(t0: Instant) -> Instant {
    t0 + HOOK_TIMEOUT
}

/// Raises `payload` for chat `chat` at `t0` and lets it time out: its lapsed id.
fn timed_out(hooks: &HookAsks, chat: &str, source: Source, payload: &Value, t0: Instant) -> AskId {
    let held = hooks.raise(chat, source, payload, t0).expect("raised");
    assert_eq!(
        hooks.time_out(after_its_hook(t0)),
        std::slice::from_ref(&held.raised.id)
    );
    held.raised.id
}

#[test]
fn a_hook_ask_that_timed_out_is_still_listed_with_when_it_was_raised_and_ended() {
    let hooks = hooks();
    let t0 = Instant::now();
    let id = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);

    let late = after_its_hook(t0);
    assert!(hooks.pending(late).is_empty(), "no hook waits on it");
    let lapsed = hooks.lapsed(late);
    assert_eq!(lapsed.len(), 1);
    assert_eq!(lapsed[0].raised.id, id);
    assert_eq!(lapsed[0].at, t0);
    assert_eq!(lapsed[0].ended, late);
}

#[test]
fn answering_a_timed_out_ask_answers_the_chat_s_same_request_at_once_on_every_harness() {
    for source in [Source::ClaudeCode, Source::Codex, Source::Opencode] {
        let hooks = hooks();
        let t0 = Instant::now();
        // opencode's `permission.asked` properties, as its shim hands them on: a command of
        // opencode's is never allowed from the window, so its fetch is what is asked here.
        let payload = match source {
            Source::Opencode => serde_json::json!({
                "id": "per_1", "sessionID": "ses_1", "permission": "webfetch",
                "patterns": ["https://example.com"], "always": ["https://example.com"],
                "metadata": {}
            }),
            _ => bash("npm test"),
        };
        let id = timed_out(&hooks, "3", source, &payload, t0);
        let lapsed = &hooks.lapsed(after_its_hook(t0))[0].raised;
        let allow = lapsed
            .ask
            .options
            .iter()
            .find(|option| option.kind == ChoiceKind::Allow)
            .unwrap_or_else(|| panic!("{source:?} offers an allow"))
            .id
            .clone();

        let answered = after_its_hook(t0) + Duration::from_secs(5);
        let late = hooks
            .answer_late("3", &id, &allow, window(), answered)
            .expect("answers a lapsed ask");
        assert_eq!(late.chat, "3");
        assert_eq!(late.choice.id, allow);
        assert!(hooks.lapsed(answered).is_empty(), "{source:?}: answered");

        let again = answered + Duration::from_secs(5);
        let held = hooks
            .raise("3", source, &payload, again)
            .expect("its same request");
        assert_eq!(
            held.answered.try_recv().expect("answered at once").id,
            allow,
            "{source:?}"
        );
        assert!(hooks.pending(again).is_empty(), "{source:?}: nobody asked");
    }
}

#[test]
fn an_answer_waiting_for_the_same_request_is_spent_once() {
    let hooks = hooks();
    let t0 = Instant::now();
    let id = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    let now = after_its_hook(t0);
    hooks
        .answer_late("3", &id, "deny", window(), now)
        .expect("answers");

    let first = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), now)
        .expect("raised");
    assert_eq!(first.answered.try_recv().expect("at once").id, "deny");
    let second = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), now)
        .expect("raised");
    assert!(second.answered.try_recv().is_err(), "asked anew");
    assert_eq!(hooks.pending(now).len(), 1);
}

#[test]
fn an_answer_waits_only_for_the_same_request_of_the_same_chat() {
    let hooks = hooks();
    let t0 = Instant::now();
    let id = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    let now = after_its_hook(t0);
    hooks
        .answer_late("3", &id, "allow", window(), now)
        .expect("answers");

    let another_command = hooks
        .raise("3", Source::ClaudeCode, &bash("rm -rf build"), now)
        .expect("raised");
    let another_chat = hooks
        .raise("4", Source::ClaudeCode, &bash("npm test"), now)
        .expect("raised");

    assert!(another_command.answered.try_recv().is_err());
    assert!(another_chat.answered.try_recv().is_err());
    assert_eq!(hooks.pending(now).len(), 2);
}

#[test]
fn a_lapsed_ask_answered_as_another_chat_s_is_unknown_and_stays() {
    let hooks = hooks();
    let t0 = Instant::now();
    let id = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    let now = after_its_hook(t0);

    assert_eq!(
        hooks.answer_late("4", &id, "allow", window(), now),
        Err(Refused::Unknown)
    );
    assert_eq!(
        hooks.answer_late("3", &id, "maybe", window(), now),
        Err(Refused::NotAnOption("maybe".to_owned()))
    );
    assert_eq!(hooks.lapsed(now).len(), 1);
}

#[test]
fn a_lapsed_edit_is_allowed_only_from_the_window_as_a_live_one_is() {
    let hooks = hooks();
    let t0 = Instant::now();
    let edit = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {"file_path": "/w/a.txt", "content": "hello\n"},
    });
    let id = timed_out(&hooks, "3", Source::ClaudeCode, &edit, t0);
    let now = after_its_hook(t0);

    assert_eq!(
        hooks.answer_late("3", &id, "allow", approval(), now),
        Err(Refused::DrawnInTheWindow)
    );
    assert!(hooks.answer_late("3", &id, "allow", window(), now).is_ok());
}

#[test]
fn a_chat_that_asks_the_same_again_replaces_its_lapsed_ask_with_the_live_one() {
    let hooks = hooks();
    let t0 = Instant::now();
    timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    let now = after_its_hook(t0);

    let held = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), now)
        .expect("raised");

    assert!(held.answered.try_recv().is_err(), "nobody answered it");
    assert!(hooks.lapsed(now).is_empty());
    assert_eq!(hooks.pending(now).len(), 1);
}

#[test]
fn a_lapsed_ask_and_a_waiting_answer_each_go_after_their_time() {
    let hooks = hooks();
    let t0 = Instant::now();
    let first = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    timed_out(&hooks, "3", Source::ClaudeCode, &bash("cargo test"), t0);
    let ended = after_its_hook(t0);
    hooks
        .answer_late("3", &first, "allow", window(), ended)
        .expect("answers");

    let listed_still = ended + LAPSED_FOR - Duration::from_secs(1);
    assert_eq!(hooks.lapsed(listed_still).len(), 1);
    assert!(hooks.lapsed(ended + LAPSED_FOR).is_empty());

    let too_late = ended + PRE_ANSWERED_FOR;
    let held = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), too_late)
        .expect("raised");
    assert!(held.answered.try_recv().is_err(), "the answer had gone");
}

#[test]
fn a_chat_keeps_only_its_newest_lapsed_asks() {
    let hooks = hooks();
    let t0 = Instant::now();
    let commands: Vec<String> = (0..=MOST_LAPSED).map(|n| format!("echo {n}")).collect();
    for command in &commands {
        let held = hooks
            .raise("3", Source::ClaudeCode, &bash(command), t0)
            .expect("raised");
        hooks.lapse(&held.raised.id, t0);
    }

    let kept: Vec<String> = hooks
        .lapsed(t0)
        .into_iter()
        .map(|one| one.raised.ask.action.said())
        .collect();
    assert_eq!(kept.len(), MOST_LAPSED);
    assert!(!kept.contains(&format!("Run a command: {}", commands[0])));
}

#[test]
fn a_hook_that_gave_up_before_this_clock_saw_its_deadline_still_lapses() {
    let hooks = hooks();
    let t0 = Instant::now();
    let held = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), t0)
        .expect("raised");

    let early = t0 + Duration::from_secs(30);
    hooks.lapse(&held.raised.id, early);

    assert!(hooks.pending(early).is_empty());
    assert_eq!(hooks.lapsed(early)[0].raised.id, held.raised.id);
}

#[test]
fn a_question_to_the_person_is_not_kept_once_it_times_out() {
    let hooks = hooks();
    let t0 = Instant::now();
    let question = serde_json::json!({
        "tool_name": "AskUserQuestion",
        "tool_input": {"questions": [{
            "question": "Which one?",
            "header": "Pick",
            "multiSelect": false,
            "options": [{"label": "A", "description": "a"}, {"label": "B", "description": "b"}],
        }]},
    });
    hooks
        .raise("3", Source::ClaudeCode, &question, t0)
        .expect("raised");

    assert_eq!(hooks.time_out(after_its_hook(t0)).len(), 1);
    assert!(hooks.lapsed(after_its_hook(t0)).is_empty());
}

#[test]
fn a_chat_that_ended_keeps_neither_its_lapsed_asks_nor_its_waiting_answers() {
    let hooks = hooks();
    let t0 = Instant::now();
    let answered = timed_out(&hooks, "3", Source::ClaudeCode, &bash("npm test"), t0);
    timed_out(&hooks, "3", Source::ClaudeCode, &bash("cargo test"), t0);
    let now = after_its_hook(t0);
    hooks
        .answer_late("3", &answered, "allow", window(), now)
        .expect("answers");

    hooks.forget("3");

    assert!(hooks.lapsed(now).is_empty());
    let held = hooks
        .raise("3", Source::ClaudeCode, &bash("npm test"), now)
        .expect("raised");
    assert!(held.answered.try_recv().is_err());
}
