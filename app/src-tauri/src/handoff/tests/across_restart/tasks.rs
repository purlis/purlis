// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Dispatch across a restart, as the chat that asked sees it** (#5754, spec #5743; #1546,
//! #1463). The chat that dispatched tasks is cut off by the restart as its tasks are. Brought
//! back on its conversation, its wake line names each task the launch restored and where it
//! stands, and says which of its tasks' reports wait for it; and a `dispatch wait` on a task
//! that reported before the restart answers with the report its dispatch record keeps.

use purlis_core::dispatched::Waited;

use super::*;

/// The wake line chat `chat` was started with by `host`: the last word of its start.
fn wake_line_of(host: &Pretend, chat: u32) -> String {
    started_with(host, chat).last().cloned().unwrap_or_default()
}

#[test]
fn an_asker_with_two_running_tasks_is_told_after_a_restart_that_both_carry_on() {
    let (plane, host, planes, id, held, steward, first) = a_steward_and_a_working_task();
    let second = a_task_of(&held, &id, steward, "check staging");
    works(&held, second);
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    assert!(
        opened.iter().any(|open| open.session == steward),
        "precondition: the asker came back under its number"
    );
    let told = wake_line_of(&relaunched, steward);
    assert!(told.starts_with("purlis: purlis was restarted"), "{told}");
    for task in [first, second] {
        assert!(
            told.contains(&format!(
                "chat {task}: brought back on its conversation, carrying on"
            )),
            "chat {task} and its state are named: {told}"
        );
    }
    assert!(told.contains("dispatch none of them again"), "{told}");
}

#[test]
fn a_report_pending_at_quit_is_named_in_the_asker_s_wake_line_and_handed_with_it() {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_a_working_task();
    reports(&held, &id, task);
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, held, _opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    let told = wake_line_of(&relaunched, steward);
    assert!(told.starts_with("purlis: purlis was restarted"), "{told}");
    assert!(
        told.contains(&format!("chat {task} reported (done)")),
        "the report is named: {told}"
    );
    // It is still where the asker's first turn takes it from, as context, once.
    assert_eq!(
        waiting(&held, For::Chat(steward)),
        vec![("check prod".to_owned(), false, false)]
    );
}

#[test]
fn dispatch_wait_after_a_restart_answers_with_the_report_on_disk() {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_a_working_task();
    reports(&held, &id, task);
    // The asker's turn read the report before the quit: the app held its text in memory only.
    assert_eq!(waiting(&held, For::Chat(steward)).len(), 1);
    let at_quit = quits(&held);

    let (_relaunched, _planes, again, held, _opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    let said = asks(
        &held,
        &again,
        steward,
        What::Wait {
            of: task,
            within_secs: 1,
        },
    );
    let Answer::Task(answered) = said else {
        panic!("an answer, not {said:?}")
    };
    let Answered::Waited {
        what: Waited::Reported { report },
        ..
    } = *answered
    else {
        panic!("the report from its dispatch record, not {answered:?}")
    };
    assert_eq!(report.summary, "Forty are stuck.");
    assert_eq!(
        report.task.as_ref().map(|task| task.outcome),
        Some(purlis_core::handback::Outcome::Done)
    );
}

#[test]
fn an_asker_with_no_tasks_and_no_reports_is_still_told_nothing() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, _id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&_id).expect("held");
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, _held, _opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    let told = wake_line_of(&relaunched, steward);
    assert!(!told.starts_with("purlis:"), "{told}");
}
