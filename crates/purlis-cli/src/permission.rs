// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis hook permissionrequest`: a harness's permission prompt, asked in purlis's window
//! as well as in the pane (HP-6): Claude Code's and Codex's `PermissionRequest`, and opencode's
//! `permission.asked` as purlis's shim hands it on (#1691). Which one it is comes from the
//! harness the app put into the chat's environment ([`Source::of_harness`]), and the app holds
//! the ask only where that is the harness it started the chat as (#1703).
//!
//! The hook hands its payload to the app on the hook channel and waits for the operator's
//! answer there; when it comes, the hook prints its harness's own decision for the option they
//! chose ([`purlis_core::harness::hooked::decision`]), and the harness carries it out without
//! the pane ever having focus.
//!
//! **A task's own question goes to its asking chat** (#4555, ADR 0107): before anything is held,
//! the app is asked to route it ([`redirected`]), and the hook refuses the prompt with the
//! sentence that says where it went. Where that fails, it is an ask as any other.
//!
//! **Every other way out prints nothing**, and then the harness's own prompt decides in the
//! pane: no app listening, a payload that will not read, an app that will not hold the ask,
//! the deadline passing. This hook never allows or denies anything by itself.

use std::process::ExitCode;
use std::time::Duration;

use purlis_core::harness::hooked::{self, Source};
use purlis_core::hookwire::{self, PermissionAsked};

/// How long the hook waits for the app's reply: under the harness's timeout for it, so the
/// hook prints before the harness stops waiting. The app answers "none" before this, at the
/// ask's deadline.
const WAITS_AT_MOST: Duration = hooked::HOOK_TIMEOUT.saturating_sub(Duration::from_secs(1));

/// Asks the app, and prints Claude Code's decision for what the operator chose.
pub fn permissionrequest(payload: &str) -> ExitCode {
    // The harness's timeout runs from the hook's start, so the app counts the ask's deadline
    // from here, and this hook's own wait too (#1063).
    let began = std::time::Instant::now();
    let Some(socket) = purlis_core::envvar::var_os(hookwire::SOCKET_ENV) else {
        return ExitCode::SUCCESS;
    };
    let Some(chat) =
        purlis_core::envvar::var(hookwire::CHAT_ENV).and_then(|chat| chat.parse().ok())
    else {
        return ExitCode::SUCCESS;
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload) else {
        return ExitCode::SUCCESS;
    };
    // A harness this does not know decides nothing, and its own prompt asks in the pane.
    let Some(source) =
        Source::of_harness(hookwire::chat_harness(&purlis_core::envvar::var).as_deref())
    else {
        return ExitCode::SUCCESS;
    };
    if let Some(decision) = redirected(source, &payload, &crate::dispatch::route) {
        println!("{decision}");
        return ExitCode::SUCCESS;
    }
    let asked = PermissionAsked {
        chat,
        permission_request: source,
        payload,
        ran_for_ms: Some(ran_for_ms(began)),
    };
    match hookwire::ask_permission(
        std::path::Path::new(&socket),
        hookwire::ChatToken::from_env().as_ref(),
        &asked,
        WAITS_AT_MOST.saturating_sub(began.elapsed()),
    ) {
        Ok(Some(chosen)) => {
            if let Some(decision) = hooked::decision(source, &asked.payload, &chosen) {
                println!("{decision}");
            }
        }
        Ok(None) => {
            if let Some(report) = left_to_its_terminal(source, &asked.payload, &|name| {
                purlis_core::envvar::var(name)
            }) {
                // Best effort, as every report is: the prompt asks in the pane either way.
                let _ = hookwire::deliver_report(
                    std::path::Path::new(&socket),
                    hookwire::ChatToken::from_env().as_ref(),
                    &report,
                );
            }
        }
        // Said where a zero-exit hook's stderr goes, the harness's debug log, and the pane asks.
        Err(why) => eprintln!("purlis: the app did not take this permission ask ({why})"),
    }
    ExitCode::SUCCESS
}

/// **The hook's refusal of a question `route` sent to the task's asking chat** (#4555), or none
/// where it is no question, the harness has no adapter, or the person is to be asked: then the
/// hook goes on as for any ask, and the question is in the pane and the Inbox. `route` is the
/// app's to answer, from its own record of the chat.
fn redirected(
    source: Source,
    payload: &serde_json::Value,
    route: &dyn Fn(&str) -> crate::dispatch::Routed,
) -> Option<String> {
    use crate::dispatch::Routed;
    let text = hooked::asked_text(source, payload)?;
    let message = match route(&text) {
        Routed::AskingChat(to) => purlis_core::dispatchtalk::redirected(&to),
        Routed::AlreadyAsking(why) => why,
        Routed::ThePerson => return None,
    };
    hooked::refused_with(source, &message)
}

/// **`purlis hook taskquestion`: Codex's own question prompt, before it is asked** (#4558, ADR
/// 0107), on the `PreToolUse` Codex runs for `request_user_input`
/// ([`hooked::QUESTION_WORD`]). A task's question goes to its asking chat, and Codex is told
/// so as the tool's result ([`codex_question_redirected`]).
///
/// **It fails open, and never drops a question.** No app, a payload that will not read, a
/// harness that is not Codex, or a question the app says is the person's: it prints nothing,
/// and Codex asks in its pane. There it is said to the app as a question waiting on the person
/// ([`question_left_in_its_pane`]), so the chat is in the Inbox and does not wait unseen,
/// since Codex says nothing of its own about it.
pub fn taskquestion(payload: &str) -> ExitCode {
    let Some(socket) = purlis_core::envvar::var_os(hookwire::SOCKET_ENV) else {
        return ExitCode::SUCCESS;
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload) else {
        return ExitCode::SUCCESS;
    };
    let Some(source) =
        Source::of_harness(hookwire::chat_harness(&purlis_core::envvar::var).as_deref())
    else {
        return ExitCode::SUCCESS;
    };
    if let Some(decision) = codex_question_redirected(source, &payload, &crate::dispatch::route) {
        println!("{decision}");
        return ExitCode::SUCCESS;
    }
    if source == Source::Codex
        && let Some(report) =
            question_left_in_its_pane(&payload, &|name| purlis_core::envvar::var(name))
    {
        // Best effort, as every report is: Codex asks in its pane either way.
        let _ = hookwire::deliver_report(
            std::path::Path::new(&socket),
            hookwire::ChatToken::from_env().as_ref(),
            &report,
        );
    }
    ExitCode::SUCCESS
}

/// **The hook's refusal of a Codex question `route` sent to the task's asking chat** (#4558), or
/// none where it is no question of Codex's or the person is to be asked: then Codex asks in its
/// pane. `route` is the app's to answer, from its own record of the chat, as for Claude Code's
/// prompt ([`redirected`]).
fn codex_question_redirected(
    source: Source,
    payload: &serde_json::Value,
    route: &dyn Fn(&str) -> crate::dispatch::Routed,
) -> Option<String> {
    use crate::dispatch::Routed;
    if source != Source::Codex {
        return None;
    }
    let text = hooked::codex_asked_text(payload)?;
    let message = match route(&text) {
        Routed::AskingChat(to) => purlis_core::dispatchtalk::redirected(&to),
        Routed::AlreadyAsking(why) => why,
        Routed::ThePerson => return None,
    };
    Some(hooked::codex_question_refused(&message))
}

/// **The report that Codex's own question now waits in its pane** (#4558): a question waiting
/// on the person, in the word purlis's opencode shim says opencode's in, which the app reads as
/// one. Nothing of the question is on the line ([`notification`]).
fn question_left_in_its_pane(
    payload: &serde_json::Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<hookwire::Report> {
    hooked::codex_asked_text(payload)?;
    notification("question", payload, env)
}

/// How long this hook has run since `began`, in milliseconds, as its line says it.
fn ran_for_ms(began: std::time::Instant) -> u64 {
    u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// **The report that the prompt now waits in the harness's own terminal** (#1691), for a
/// harness that has no notice of its own to say so: Codex, which has no `Notification`. The
/// app took the ask and nobody answered it in time, or would not hold it, so the hook decides
/// nothing and the harness asks in its pane; the chat is then listed as waiting there, so it
/// does not wait unseen. Claude Code says it itself (`permission_prompt`), and purlis's opencode
/// shim says it as opencode asks.
pub fn left_to_its_terminal(
    source: Source,
    payload: &serde_json::Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<hookwire::Report> {
    if source != Source::Codex {
        return None;
    }
    notification("permission_prompt", payload, env)
}

/// **A `Notification` purlis says itself**, for a harness that has no notice of its own to say
/// it ([`left_to_its_terminal`], and `ask_operator`'s wait in `purlis mcp`, #1089): the report
/// of `notification_type` `kind`, read as a hook's own would be ([`hookwire::Report::read`]).
///
/// **It carries nothing but `kind` and the run `payload` names**: its `session_id`, and the
/// helper's `agent_id` and `agent_type`, each only where it is a string. Whatever else the
/// payload holds (a command, a question) is never on the line. `kind` is one of the words the
/// app already reads (`purlis_core::hookwire`'s `notified`), never a new one.
pub fn notification(
    kind: &str,
    payload: &serde_json::Value,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<hookwire::Report> {
    let mut said = serde_json::json!({
        "hook_event_name": "Notification",
        "notification_type": kind,
    });
    for field in ["session_id", "agent_id", "agent_type"] {
        if let Some(value) = payload.get(field).filter(|value| value.is_string()) {
            said[field] = value.clone();
        }
    }
    hookwire::Report::read(
        purlis_core::state::Event::Notification,
        &said.to_string(),
        env,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::harness::model::Prompt;
    use purlis_core::state::{Event, Notified};

    fn env(name: &str) -> Option<String> {
        match name {
            hookwire::CHAT_ENV => Some("7".to_owned()),
            hookwire::SOCKET_ENV => Some("/tmp/s.sock".to_owned()),
            _ => None,
        }
    }

    fn a_question() -> serde_json::Value {
        serde_json::json!({
            "tool_name": "AskUserQuestion",
            "tool_input": { "questions": [{
                "question": "Which host?",
                "options": [{ "label": "blue" }],
            }] },
        })
    }

    fn message_of(decision: &str) -> String {
        let said: serde_json::Value = serde_json::from_str(decision).expect("json");
        said["hookSpecificOutput"]["decision"]["message"]
            .as_str()
            .expect("a message")
            .to_owned()
    }

    #[test]
    fn a_task_s_question_prompt_is_refused_with_where_it_went() {
        use crate::dispatch::Routed;
        let decision = redirected(Source::ClaudeCode, &a_question(), &|text| {
            assert_eq!(text, "Which host?\n- blue");
            Routed::AskingChat("steward 3".to_owned())
        })
        .expect("redirected");

        assert_eq!(
            message_of(&decision),
            purlis_core::dispatchtalk::redirected("steward 3")
        );
    }

    #[test]
    fn a_task_paused_on_a_question_is_told_so_and_never_reaches_around_its_asking_chat() {
        use crate::dispatch::Routed;
        let decision = redirected(Source::ClaudeCode, &a_question(), &|_| {
            Routed::AlreadyAsking("It is paused.".to_owned())
        })
        .expect("refused");

        assert_eq!(message_of(&decision), "It is paused.");
    }

    #[test]
    fn a_question_for_the_person_or_a_failed_redirect_goes_on_as_any_ask() {
        use crate::dispatch::Routed;
        assert_eq!(
            redirected(Source::ClaudeCode, &a_question(), &|_| Routed::ThePerson),
            None
        );
    }

    #[test]
    fn a_permission_and_a_harness_with_no_adapter_are_never_routed() {
        use crate::dispatch::Routed;
        let route = |_: &str| -> Routed { panic!("not routed") };
        let bash = serde_json::json!({ "tool_name": "Bash", "tool_input": {"command": "ls"} });
        assert_eq!(redirected(Source::ClaudeCode, &bash, &route), None);
        assert_eq!(redirected(Source::Codex, &a_question(), &route), None);
        assert_eq!(redirected(Source::Opencode, &a_question(), &route), None);
    }

    #[test]
    fn a_codex_prompt_left_to_its_pane_is_reported_as_a_permission_waiting_there() {
        let payload = serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "tool_name": "Bash", "tool_input": {"command": "npm test"}
        });
        let report = left_to_its_terminal(Source::Codex, &payload, &env).expect("a report");
        assert_eq!(report.chat, 7);
        assert_eq!(report.event, Event::Notification);
        assert_eq!(report.detail.notified, Notified::Asks(Prompt::Permission));
        assert!(!report.detail.idle);
    }

    #[test]
    fn the_hook_s_line_says_how_long_it_has_run() {
        let began = std::time::Instant::now() - Duration::from_millis(1_500);
        let said = ran_for_ms(began);
        assert!((1_500..60_000).contains(&said), "{said}");
    }

    /// Codex's `request_user_input` on its `PreToolUse` (#4558).
    fn a_codex_question() -> serde_json::Value {
        serde_json::json!({
            "session_id": "11111111-2222-4333-8444-555555555555",
            "hook_event_name": "PreToolUse",
            "tool_name": "request_user_input",
            "tool_input": { "questions": [{
                "id": "host", "header": "Host", "question": "Which host?",
                "options": [{ "label": "blue", "description": "the old one" }],
            }] },
        })
    }

    fn reason_of(decision: &str) -> String {
        let said: serde_json::Value = serde_json::from_str(decision).expect("json");
        assert_eq!(said["hookSpecificOutput"]["permissionDecision"], "deny");
        said["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .expect("a reason")
            .to_owned()
    }

    #[test]
    fn a_codex_task_s_question_prompt_is_denied_with_where_it_went() {
        use crate::dispatch::Routed;
        let decision = codex_question_redirected(Source::Codex, &a_codex_question(), &|text| {
            assert_eq!(text, "Which host?\n- blue: the old one");
            Routed::AskingChat("steward 3".to_owned())
        })
        .expect("redirected");

        assert_eq!(
            reason_of(&decision),
            purlis_core::dispatchtalk::redirected("steward 3")
        );
    }

    #[test]
    fn a_codex_task_paused_on_a_question_is_told_so_and_never_reaches_around_its_asking_chat() {
        use crate::dispatch::Routed;
        let decision = codex_question_redirected(Source::Codex, &a_codex_question(), &|_| {
            Routed::AlreadyAsking("It is paused.".to_owned())
        })
        .expect("refused");

        assert_eq!(reason_of(&decision), "It is paused.");
    }

    #[test]
    fn a_codex_question_for_the_person_or_a_failed_redirect_is_asked_in_its_pane() {
        use crate::dispatch::Routed;
        assert_eq!(
            codex_question_redirected(Source::Codex, &a_codex_question(), &|_| Routed::ThePerson),
            None
        );
    }

    #[test]
    fn only_codex_s_own_question_is_routed_on_this_hook() {
        use crate::dispatch::Routed;
        let route = |_: &str| -> Routed { panic!("not routed") };
        let bash = serde_json::json!({ "tool_name": "Bash", "tool_input": {"command": "ls"} });
        assert_eq!(
            codex_question_redirected(Source::Codex, &bash, &route),
            None
        );
        for source in [Source::ClaudeCode, Source::Opencode] {
            assert_eq!(
                codex_question_redirected(source, &a_codex_question(), &route),
                None
            );
        }
    }

    #[test]
    fn a_codex_question_left_to_its_pane_is_reported_as_a_question_waiting_on_the_person() {
        let report = question_left_in_its_pane(&a_codex_question(), &env).expect("a report");
        assert_eq!(report.chat, 7);
        assert_eq!(report.event, Event::Notification);
        assert_eq!(report.detail.notified, Notified::Asks(Prompt::Question));
    }

    #[test]
    fn a_harness_with_a_notice_of_its_own_is_left_to_say_it() {
        let payload = serde_json::json!({"session_id": "s"});
        for source in [Source::ClaudeCode, Source::Opencode] {
            assert!(
                left_to_its_terminal(source, &payload, &env).is_none(),
                "{source:?}"
            );
        }
    }
}
