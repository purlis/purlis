// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's Allow of a persona's tool on a pool (#2047): what a runner chat's command waits
//! on, and what answers it.

use std::time::Duration;

use super::*;

fn builds(image: &str) -> Bound {
    Bound {
        pool: "builds".into(),
        definition: "d".repeat(64),
        image: image.repeat(64),
        runner: "01JB0RUNNER0000000000000000".into(),
        runner_key: "ab".repeat(32),
        runner_image: "a".repeat(64),
        engine: "this machine's Docker".into(),
    }
}

fn kubectl(session: u32, bound: Bound) -> PoolToolWaits {
    PoolToolWaits {
        session,
        persona: "devops".into(),
        bound,
        tool: "kubectl".into(),
        whole: "kubectl with kubeconfig from vault team as KUBECONFIG file".into(),
        vault: "team".into(),
        since: 1_700_000_000,
        changed: false,
    }
}

fn waits(asked: Asked) -> (mpsc::Receiver<bool>, u64) {
    match asked {
        Asked::Waits(answered, number) => (answered, number),
        other => panic!("it should wait on the person: {other:?}"),
    }
}

#[test]
fn a_tool_never_allowed_on_the_pool_waits_on_the_person_and_goes_on_once_allowed() {
    let tools = PoolTools::default();

    let (answered, _) = waits(tools.ask(kubectl(6, builds("a")), false));
    assert_eq!(tools.waiting(), [kubectl(6, builds("a"))], "one Ask");
    let answered_now = tools.answer(6, "kubectl", true);

    assert_eq!(answered_now.len(), 1);
    assert_eq!(answered.recv_timeout(Duration::from_secs(1)), Ok(true));
    assert!(tools.waiting().is_empty(), "the Ask is gone");
    assert!(
        matches!(tools.ask(kubectl(6, builds("a")), false), Asked::Allowed),
        "the next run goes on with no Ask"
    );
}

#[test]
fn a_command_that_waited_for_the_desktop_is_asked_about_afresh() {
    let tools = PoolTools::default();
    let _ = tools.ask(kubectl(6, builds("a")), false);
    tools.answer(6, "kubectl", true);

    let (answered, _) = waits(tools.ask(kubectl(6, builds("a")), true));
    tools.answer(6, "kubectl", false);

    assert_eq!(answered.recv_timeout(Duration::from_secs(1)), Ok(false));
}

#[test]
fn keep_off_refuses_the_tool_on_the_pool_until_it_changes() {
    let tools = PoolTools::default();
    let (answered, _) = waits(tools.ask(kubectl(6, builds("a")), false));

    tools.answer(6, "kubectl", false);

    assert_eq!(answered.recv_timeout(Duration::from_secs(1)), Ok(false));
    assert!(matches!(
        tools.ask(kubectl(6, builds("a")), false),
        Asked::KeptOff
    ));
    let _ = waits(tools.ask(kubectl(6, builds("b")), false));
    assert!(
        tools.waiting()[0].changed,
        "a changed pool asks again, and says so"
    );
}

#[test]
fn an_allow_answers_every_chat_of_the_persona_whose_same_tool_waits_on_that_pool() {
    let tools = PoolTools::default();
    let (first, _) = waits(tools.ask(kubectl(6, builds("a")), false));
    let (second, _) = waits(tools.ask(kubectl(7, builds("a")), false));
    let (elsewhere, _) = waits(tools.ask(kubectl(8, builds("b")), false));

    let answered = tools.answer(6, "kubectl", true);

    assert_eq!(answered.len(), 2);
    assert_eq!(first.recv_timeout(Duration::from_secs(1)), Ok(true));
    assert_eq!(second.recv_timeout(Duration::from_secs(1)), Ok(true));
    assert!(
        elsewhere.try_recv().is_err(),
        "the pool as another Ask showed it is not answered"
    );
    assert_eq!(tools.waiting().len(), 1);
}

#[test]
fn a_closed_chats_asks_go_and_what_waits_on_them_is_let_go_of() {
    let tools = PoolTools::default();
    let (answered, _) = waits(tools.ask(kubectl(6, builds("a")), false));

    tools.forget(6);

    assert!(tools.waiting().is_empty());
    assert_eq!(
        answered.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
    assert!(tools.answer(6, "kubectl", true).is_empty());
}

#[test]
fn a_command_that_stopped_waiting_takes_the_ask_with_it_only_when_it_was_the_last() {
    let tools = PoolTools::default();
    let (_first, one) = waits(tools.ask(kubectl(6, builds("a")), false));
    let (second, two) = waits(tools.ask(kubectl(6, builds("a")), false));

    tools.gave_up(6, "kubectl", one);
    assert_eq!(tools.waiting().len(), 1, "another still waits on it");
    tools.gave_up(6, "kubectl", two);

    assert!(tools.waiting().is_empty());
    assert!(second.try_recv().is_err());
}
