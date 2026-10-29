// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The chat that dispatched a task cancels it, on the person's stop machine (#1459): one bounded
//! turn to report, then its end, and what the asking chat is told is its report, as
//! `cancelled`.
//!
//! Against the app's own records, on a pretend session host that remembers what was written to
//! each chat's terminal. The clocks are the test's (`stopping::*_in_a_test`, and the look for a
//! reported task's end), so nothing here sleeps.

use purlis_core::dispatched::{Answered, Looked, What};
use purlis_core::dispatchrecord::Outcome as Recorded;
use purlis_core::handback::Handback;

use super::*;
use crate::stopping::Way;

/// A steward chat and the one task it dispatched, on a pretend host.
fn a_steward_and_its_task() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "check prod");
    (plane, host, planes, id, held, steward, task)
}

/// The steward cancels its task, and is told it is cancelling.
fn cancels(held: &Held, id: &PlaneId, steward: u32, task: u32) {
    let said = asks(held, id, steward, What::Cancel { of: task });
    assert!(
        matches!(&said, Answer::Task(answered)
            if matches!(&**answered, Answered::Cancelling { of, .. } if *of == task)),
        "{said:?}"
    );
}

/// Chat `chat`'s harness says a prompt began a turn, and the app hears it, its stop too.
fn a_turn_begins(held: &Held, chat: u32) {
    use purlis_core::hookwire::Conversation;
    the_board_hears(held, chat, Event::UserPromptSubmit);
    crate::dispatched::heard(
        held,
        &purlis_core::hookwire::Report {
            chat,
            event: Event::UserPromptSubmit,
            conversation: held
                .board()
                .conversation(chat)
                .map_or(Conversation::Unknown, Conversation::Named),
            pid: Some(4000 + chat),
            agent: None,
            detail: purlis_core::state::Detail::default(),
        },
    );
    let held = kept(held);
    bounded("the start of a cancelled task's turn", move || {
        crate::stopping::reported(&held, chat);
    });
}

/// Chat `chat`'s harness says its turn ended, and the app hears it, its stop too.
fn its_turn_ends(held: &Held, chat: u32) {
    the_board_hears(held, chat, Event::Stop);
    crate::dispatched::moved(held, chat);
    let held = kept(held);
    bounded("the end of a cancelled task's turn", move || {
        crate::stopping::reported(&held, chat);
    });
}

/// The moment an interrupted turn is given to stop has passed.
fn its_turn_has_stopped(held: &Held, chat: u32) {
    let held = kept(held);
    bounded("the look after an interrupt", move || {
        crate::stopping::settled_in_a_test(&held, chat);
    });
}

/// A clock of chat `chat`'s cancel ran out: its line went unheard, or its turn took too long.
fn its_clock_runs_out(held: &Held, chat: u32, unheard: bool) {
    let held = kept(held);
    bounded("a cancel's clock running out", move || {
        crate::stopping::clock_ran_out_in_a_test(&held, chat, unheard);
    });
}

/// The moment a reported task is given to settle has passed: purlis ends it where its turn is
/// over (#1485).
fn settles(held: &Held, task: u32) {
    let held = kept(held);
    bounded("the look for a reported task's end", move || {
        crate::dispatched::end_look(&held, task, Looked::Settled);
    });
}

/// Everything that waits for `whose`, whole, oldest first: taken, as its next turn takes it.
fn told(held: &Held, whose: For<'_>) -> Vec<Handback> {
    purlis_core::handback::take(held.root(), whose)
}

/// The one report the steward is handed: cancelled, and no word of a stop.
fn the_cancelled_report(held: &Held, steward: u32) -> Handback {
    let word = told(held, For::Chat(steward));
    assert_eq!(word.len(), 1, "{word:?}");
    let report = word[0].clone();
    assert_eq!(
        report.stopped, None,
        "a cancel is not the person's stop: {report:?}"
    );
    assert_eq!(
        report.task.as_ref().map(|task| task.outcome),
        Some(Outcome::Cancelled)
    );
    report
}

const ESCAPE: &[u8] = purlis_core::dispatched::INTERRUPT;

/// The line a cancel types: purlis's own, which says the asking chat cancelled the task.
fn the_cancel_line() -> Vec<u8> {
    format!(
        "{}\r",
        crate::curation::bracketed(purlis_core::dispatched::CANCEL_PROMPT)
    )
    .into_bytes()
}

#[test]
fn a_waiting_task_is_asked_at_once_and_its_report_arrives_as_cancelled_then_it_ends() {
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    rests(&held, task);

    cancels(&held, &id, steward, task);

    assert_eq!(host.typed(task), vec![the_cancel_line()]);
    // On the stop machine, and not drawn as the person's stop.
    assert!(held.stopping().is_cancelling(task));
    assert!(!held.stopping().is_stopping(task));
    assert_eq!(held.stopping().now(), Vec::<u32>::new());
    assert!(held.tasks().ledger().cancelling(task));

    // Its one short turn: whatever it says of itself, it was cancelled.
    a_turn_begins(&held, task);
    reports(&held, &id, task);
    let report = the_cancelled_report(&held, steward);
    assert_eq!(report.summary, "Forty are stuck.");
    let record = record_of(&held, task);
    assert_eq!(
        record.report.map(|report| report.outcome),
        Some(Recorded::Cancelled)
    );
    assert_eq!((record.ended_by, record.ended_way), (None, None));

    // It ends at its report, as every task does: once that turn is over.
    assert!(open_chats(&held).contains(&task), "mid-turn");
    its_turn_ends(&held, task);
    assert!(!held.stopping().is_cancelling(task), "the cancel let go");
    settles(&held, task);
    assert!(!open_chats(&held).contains(&task));
    assert_eq!(host.typed(task), vec![the_cancel_line()], "one line");
}

#[test]
fn a_running_task_is_interrupted_then_asked_and_one_that_ends_its_turn_silent_is_reported_for() {
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);

    cancels(&held, &id, steward, task);

    assert_eq!(host.typed(task), vec![ESCAPE.to_vec()]);
    its_turn_has_stopped(&held, task);
    assert_eq!(host.typed(task), vec![ESCAPE.to_vec(), the_cancel_line()]);

    // The line begins a turn, which ends with no report: purlis writes one.
    a_turn_begins(&held, task);
    its_turn_ends(&held, task);
    let report = the_cancelled_report(&held, steward);
    assert_eq!(
        report.summary,
        purlis_core::dispatched::CANCELLED_UNREPORTED
    );
    settles(&held, task);
    assert!(!open_chats(&held).contains(&task));
}

#[test]
fn a_cancel_waits_for_the_person_s_answer_to_a_prompt_and_is_never_what_ends_one() {
    // D-1459-2: a task showing the person a prompt is typed nothing, and its bound passing
    // ends nothing while the prompt is in front of them.
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);
    the_board_hears(&held, task, Event::Notification);

    cancels(&held, &id, steward, task);
    its_clock_runs_out(&held, task, false);

    assert_eq!(
        host.typed(task),
        Vec::<Vec<u8>>::new(),
        "nothing into a prompt"
    );
    assert!(open_chats(&held).contains(&task));
    assert!(held.stopping().is_cancelling(task));
    assert_eq!(told(&held, For::Chat(steward)), Vec::new());

    // The turn ends once the prompt is answered: the task is asked, with no Escape first.
    its_turn_ends(&held, task);
    assert_eq!(host.typed(task), vec![the_cancel_line()]);
}

#[test]
fn a_prompt_answered_mid_turn_lets_the_cancel_end_that_turn_as_it_would_have() {
    // D-1459-2: answered in the window, the turn goes on, and is ended as a cancel ends one.
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);
    the_board_hears(&held, task, Event::Notification);
    cancels(&held, &id, steward, task);
    assert_eq!(host.typed(task), Vec::<Vec<u8>>::new());

    a_turn_begins(&held, task);

    assert_eq!(host.typed(task), vec![ESCAPE.to_vec()]);
    its_turn_has_stopped(&held, task);
    assert_eq!(host.typed(task), vec![ESCAPE.to_vec(), the_cancel_line()]);
}

#[test]
fn a_task_the_person_has_typed_in_is_not_cancelled_by_its_asking_chat() {
    // #3785, ruled 2026-10-29 (replacing D-1459-3's wait for the person's keys): a task the
    // person has typed into is theirs, so its asking chat's cancel is refused in one sentence,
    // as its close is, and nothing is typed or recorded.
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    rests(&held, task);
    crate::dispatched::person_typed(&held, task, b"wa", None);

    let said = asks(&held, &id, steward, What::Cancel { of: task });
    assert!(
        matches!(&said, Answer::No { why }
        if why == &format!(
            "the person has typed in 'check prod' (chat {task}), so it is theirs to \
             cancel, from its tab."
        )),
        "{said:?}"
    );
    assert_eq!(
        host.typed(task),
        Vec::<Vec<u8>>::new(),
        "nothing over their keys"
    );
    assert!(!held.stopping().is_cancelling(task));
    assert!(open_chats(&held).contains(&task));
}

#[test]
fn a_cancel_is_bounded_by_a_line_nobody_hears_and_by_a_turn_that_does_not_end() {
    // D-1459-1: the bounds the person's stop has.
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_its_task();
    rests(&held, task);
    cancels(&held, &id, steward, task);

    its_clock_runs_out(&held, task, true);

    let report = the_cancelled_report(&held, steward);
    assert_eq!(
        report.summary,
        purlis_core::dispatched::CANCELLED_UNREPORTED
    );
    assert!(!held.stopping().is_cancelling(task));

    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_its_task();
    rests(&held, task);
    cancels(&held, &id, steward, task);
    a_turn_begins(&held, task);
    its_clock_runs_out(&held, task, true);
    assert_eq!(
        told(&held, For::Chat(steward)),
        Vec::new(),
        "a turn began on it"
    );

    its_clock_runs_out(&held, task, false);

    // Still at work when its time ran out: not said to have ended its turn (review).
    let report = the_cancelled_report(&held, steward);
    assert_eq!(
        report.summary,
        purlis_core::dispatched::CANCELLED_UNREPORTED_IN_TIME
    );
}

#[test]
fn a_cancelled_task_whose_program_ends_is_reported_for_as_cancelled_once() {
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);
    cancels(&held, &id, steward, task);

    bounded("the exit", {
        let host = host.clone();
        move || host.program_ends(task, KILLED())
    });

    let report = the_cancelled_report(&held, steward);
    assert_eq!(report.summary, purlis_core::dispatched::ENDED_UNREPORTED);
    assert_eq!(
        record_of(&held, task).report.map(|report| report.outcome),
        Some(Recorded::Cancelled)
    );
}

#[test]
fn a_cancelled_task_starts_no_chat_while_it_ends_and_the_tasks_it_started_still_may() {
    // D-1459-4: from the cancel to the end of its program. A cancel takes the task alone.
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_its_task();
    let below = a_task_of(&held, &id, task, "count rows");
    works(&held, task);
    works(&held, below);

    cancels(&held, &id, steward, task);

    let refused = dispatch(&held, &id, &Tickets::default(), task, None, "one more").0;
    assert_eq!(
        refused,
        Answer::No {
            why: crate::stopping::STARTS_NOTHING.to_owned()
        }
    );
    assert!(!crate::stopping::refuses_a_start(&held, below));

    // It reports, and ends at its report: until its program is gone it starts nothing.
    its_turn_has_stopped(&held, task);
    a_turn_begins(&held, task);
    reports(&held, &id, task);
    its_turn_ends(&held, task);
    assert!(!held.stopping().is_cancelling(task));
    assert!(
        open_chats(&held).contains(&task),
        "held while a task of its works"
    );
    assert!(crate::stopping::refuses_a_start(&held, task));
    assert!(!crate::stopping::refuses_a_start(&held, below));
}

#[test]
fn the_person_s_stop_takes_a_cancelled_task_over_and_the_asking_chat_is_told_of_the_stop() {
    // D-T59-j3: the stop is the later word. The cancel stands down, and the stop runs as for
    // any task: its own line, and purlis's word that the person stopped it.
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);
    cancels(&held, &id, steward, task);
    assert_eq!(host.typed(task), vec![ESCAPE.to_vec()]);

    let held_ = kept(&held);
    bounded("the person's stop", move || {
        crate::stopping::end_task_in_a_test(&held_, task, Way::Report, false)
    })
    .expect("stopping");

    assert!(held.stopping().is_stopping(task));
    assert!(!held.stopping().is_cancelling(task));
    assert!(!held.tasks().ledger().cancelling(task));
    assert_eq!(held.stopping().now(), vec![task]);
    its_turn_has_stopped(&held, task);
    let typed = host.typed(task);
    assert_eq!(
        typed.last(),
        Some(&crate::stopping::sent_as(true).into_bytes()),
        "the stop's own line: {typed:?}"
    );
    assert!(!typed.contains(&the_cancel_line()));

    a_turn_begins(&held, task);
    reports(&held, &id, task);
    its_turn_ends(&held, task);
    let word = told(&held, For::Chat(steward));
    assert_eq!(word.len(), 1, "{word:?}");
    assert!(word[0].stopped.is_some(), "{word:?}");
}

#[test]
fn a_task_being_stopped_whose_project_settings_change_is_ended_and_not_started_again() {
    // ADR 0067 as amended 2026-10-16, on #1448's terms: a restart would take it out of the
    // person's stop, so it is ended, as the stop would end it.
    let (_plane, host, _planes, _id, held, _steward, task) = a_steward_and_its_task();
    works(&held, task);
    let held_ = kept(&held);
    bounded("the person's stop", move || {
        crate::stopping::end_task_in_a_test(&held_, task, Way::Report, false)
    })
    .expect("stopping");
    assert!(held.stopping().is_stopping(task));
    let started_before = host.openings().len();

    let why = held
        .contain_project_settings(task)
        .expect_err("not started again");

    assert!(why.contains("stopping"), "{why}");
    assert_eq!(host.openings().len(), started_before, "nothing was started");
    assert!(!open_chats(&held).contains(&task), "it was ended");
}

#[test]
fn a_task_s_run_ended_to_start_it_again_does_not_end_its_dispatch() {
    // The end of a run replaced at once is no end of the task: its record still runs, and
    // nothing is reported for it.
    let (_plane, host, _planes, _id, held, _steward, task) = a_steward_and_its_task();
    held.chats().replaced_in_a_test(task);

    host.program_ends(task, purlis_core::session::Exit::Code(1));

    let record = record_of(&held, task);
    assert!(record.running(), "{record:?}");
    assert_eq!(record.report, None);
    assert!(open_chats(&held).contains(&task));
}
