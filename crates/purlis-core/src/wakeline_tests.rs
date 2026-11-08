// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::of;
use crate::dispatchrestart::CARRY_ON;
use crate::ownwork::OwnWork;

fn one_shell() -> OwnWork {
    OwnWork {
        shells: 1,
        until: Some(1_000),
        ..OwnWork::default()
    }
}

#[test]
fn a_chat_with_no_own_work_that_is_no_task_is_told_nothing() {
    assert_eq!(of(None, false), None);
    assert_eq!(of(Some(&OwnWork::default()), false), None);
}

#[test]
fn a_chat_that_had_left_one_shell_running_is_told_the_restart_ended_it() {
    let line = of(Some(&one_shell()), false).expect("a wake line");
    assert!(line.starts_with("purlis: purlis was restarted"), "{line}");
    assert!(line.contains(": 1 background shell."), "{line}");
    assert!(
        line.contains("start again whatever is still needed"),
        "{line}"
    );
}

#[test]
fn each_kind_in_flight_is_named_with_how_many() {
    let line = of(
        Some(&OwnWork {
            shells: 2,
            monitors: 1,
            helpers: 3,
            workflows: 1,
            ..OwnWork::default()
        }),
        false,
    )
    .expect("a wake line");
    assert!(
        line.contains(": 2 background shells, 1 monitor, 3 subagents, 1 workflow."),
        "{line}"
    );
}

#[test]
fn a_task_with_no_own_work_is_told_to_carry_on_word_for_word() {
    assert_eq!(of(None, true).as_deref(), Some(CARRY_ON));
}

#[test]
fn a_task_that_had_left_a_shell_running_gets_one_line_that_carries_on_and_names_it() {
    let line = of(Some(&one_shell()), true).expect("a wake line");
    assert!(line.starts_with(CARRY_ON), "{line}");
    assert!(
        line.contains("The restart also ended what this chat had left running in the background: 1 background shell."),
        "{line}"
    );
    assert_eq!(line.matches("purlis: ").count(), 1, "one message: {line}");
}

#[test]
fn a_wake_line_is_a_first_message_a_harness_can_be_started_with() {
    for line in [of(Some(&one_shell()), false), of(Some(&one_shell()), true)] {
        let line = line.expect("a wake line");
        assert_eq!(crate::handoff::bad_message(&line), None, "{line}");
        assert!(!line.contains('\n'), "{line}");
    }
}

#[test]
fn a_chat_restarted_alone_is_told_its_restart_ended_what_it_had_left_running() {
    let line = super::of_the_chat_restarted(Some(&one_shell())).expect("a wake line");
    assert!(
        line.starts_with("purlis: this chat was restarted"),
        "{line}"
    );
    assert!(!line.contains("purlis was restarted"), "{line}");
    assert!(line.contains(": 1 background shell."), "{line}");
    assert!(
        line.contains("start again whatever is still needed"),
        "{line}"
    );
}

#[test]
fn a_chat_restarted_alone_with_nothing_of_its_own_running_is_told_nothing() {
    assert_eq!(super::of_the_chat_restarted(None), None);
    assert_eq!(
        super::of_the_chat_restarted(Some(&OwnWork::default())),
        None
    );
}
