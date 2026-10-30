// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(future)
}

#[test]
fn a_tool_that_panics_answers_an_error_and_never_hangs() {
    let answered = run(blocking(|| panic!("a tool went wrong")));
    let why = answered.expect_err("an error");
    assert!(why.contains("failed inside purlis"), "{why}");
}

fn env_of(harness: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
    move |name| match name {
        purlis_core::hookwire::CHAT_ENV => Some("7".to_owned()),
        purlis_core::hookwire::SOCKET_ENV => Some("/tmp/s.sock".to_owned()),
        purlis_core::hookwire::HARNESS_ENV => harness.map(str::to_owned),
        _ => None,
    }
}

#[test]
fn a_harness_with_no_notice_of_its_own_has_the_server_say_the_wait_and_its_end() {
    use purlis_core::harness::model::Prompt;
    use purlis_core::state::{Event, Notified};
    for harness in ["codex", "opencode"] {
        let env = env_of(Some(harness));
        let waits = wait_report(true, &env).expect("the wait is said");
        assert_eq!(waits.chat, 7);
        assert_eq!(waits.event, Event::Notification);
        assert_eq!(waits.detail.notified, Notified::Asks(Prompt::Form));
        assert!(!waits.detail.idle, "{harness}");
        assert_eq!(waits.agent, None, "{harness}");
        let ends = wait_report(false, &env).expect("the end is said");
        assert_eq!(ends.detail.notified, Notified::Answered, "{harness}");
    }
}

#[test]
fn claude_code_says_its_own_wait_so_the_server_says_nothing() {
    // Claude Code reports `elicitation_dialog` and `elicitation_complete` itself; a chat
    // that names no harness is read as Claude Code, and one purlis does not know says
    // nothing it has not measured.
    for harness in [Some("claude-code"), None, Some("aider")] {
        let env = env_of(harness);
        assert!(wait_report(true, &env).is_none(), "{harness:?}");
        assert!(wait_report(false, &env).is_none(), "{harness:?}");
    }
}

#[test]
fn a_server_no_chat_of_the_app_s_runs_says_nothing() {
    let env = |name: &str| (name == purlis_core::hookwire::HARNESS_ENV).then(|| "codex".to_owned());
    assert!(wait_report(true, &env).is_none());
}

/// A [`Waiting`] that records what it says.
fn recording() -> (Waiting, std::sync::Arc<std::sync::Mutex<Vec<bool>>>) {
    let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let into = std::sync::Arc::clone(&said);
    let waiting = Waiting::telling(Box::new(move |waits| into.lock().unwrap().push(waits)));
    (waiting, said)
}

#[test]
fn a_question_in_flight_is_said_once_and_ended_when_its_call_ends() {
    let (waiting, said) = recording();
    {
        let _one = waiting.asks();
        let _two = waiting.asks();
        assert_eq!(*said.lock().unwrap(), [true]);
    }
    // Both answered: the end once, and nothing more on a later call.
    waiting.heard_again();
    waiting.over();
    assert_eq!(*said.lock().unwrap(), [true, false]);
}

#[test]
fn a_question_in_flight_whose_call_is_dropped_still_ends_its_wait() {
    let (waiting, said) = recording();
    // A call's future dropped mid-wait (the harness cancelled it) drops its guard.
    let pending = {
        let waiting = &waiting;
        async move {
            let _asked = waiting.asks();
            std::future::pending::<()>().await;
        }
    };
    let mut pending = Box::pin(pending);
    let waker = std::task::Waker::noop();
    let _ = pending
        .as_mut()
        .poll(&mut std::task::Context::from_waker(waker));
    assert_eq!(*said.lock().unwrap(), [true]);
    drop(pending);
    assert_eq!(*said.lock().unwrap(), [true, false]);
}

#[test]
fn a_question_handed_to_the_harness_ends_at_its_next_call_or_the_session_s_end() {
    let (waiting, said) = recording();
    waiting.returned();
    waiting.returned();
    waiting.heard_again();
    assert_eq!(*said.lock().unwrap(), [true, false]);

    waiting.returned();
    waiting.over();
    assert_eq!(*said.lock().unwrap(), [true, false, true, false]);
}

#[test]
fn nothing_is_said_where_nothing_waits() {
    let (waiting, said) = recording();
    waiting.heard_again();
    waiting.over();
    assert!(said.lock().unwrap().is_empty());
}

#[test]
fn a_tool_that_returns_is_answered_as_it_returned() {
    assert_eq!(
        run(blocking(|| Ok("done".to_owned()))),
        Ok("done".to_owned())
    );
}

#[test]
fn what_the_app_attaches_reaches_the_model_after_the_text_an_image_as_an_image() {
    use purlis_core::hookwire::framed::Attached;
    let CallToolResponse::Complete(result) = done_with(
        Ok("where it works".to_owned()),
        vec![
            Attached {
                media_type: "image/png".to_owned(),
                bytes: vec![0, 255, 10],
            },
            Attached {
                media_type: "application/pdf".to_owned(),
                bytes: vec![1],
            },
        ],
    ) else {
        panic!("a complete answer");
    };
    assert_eq!(result.is_error, Some(false));
    let content = serde_json::to_value(&result.content).expect("JSON");
    assert_eq!(content[0]["text"], "where it works");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["data"], "AP8K");
    assert_eq!(content[1]["mimeType"], "image/png");
    assert_eq!(content[2]["type"], "resource");
    assert_eq!(content[2]["resource"]["blob"], "AQ==");
    assert_eq!(content[2]["resource"]["mimeType"], "application/pdf");
}
