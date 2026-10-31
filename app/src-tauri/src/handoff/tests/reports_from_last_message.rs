// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task whose turns end without a report is asked for it once, and then reported for from its
//! last message (#1981): the chat that asked always hears back.
//!
//! Against the app's own records, on a pretend session host that remembers what was written to
//! each chat's terminal. What a turn ended saying reaches the app on its own line before the
//! turn's end (`dispatched::said`), as the hook channel hands it on.

use purlis_core::handback::Handback;
use purlis_core::hookwire::{Output, TurnOutput};

use super::*;

/// A steward chat and the one task it dispatched, on a pretend host.
fn a_steward_and_its_task() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "check prod");
    (plane, host, planes, id, held, steward, task)
}

/// Chat `chat`'s harness says a prompt began a turn, and the app hears it.
fn a_turn_begins(held: &Held, chat: u32) {
    the_board_hears(held, chat, Event::UserPromptSubmit);
    crate::dispatched::heard(held, &its_own(held, chat, Event::UserPromptSubmit));
}

/// Chat `chat`'s turn ends on `last`, and the app hears both, the message first.
fn its_turn_ends_saying(held: &Held, chat: u32, last: &str) {
    crate::dispatched::said(
        held,
        TurnOutput {
            chat,
            turn_output: Output::last_message(last).expect("a message"),
            speaker: purlis_core::hookwire::Speaker::default(),
        },
    );
    the_board_hears(held, chat, Event::Stop);
    crate::dispatched::heard(held, &its_own(held, chat, Event::Stop));
}

/// A report of `event` from chat `chat`'s own harness.
fn its_own(held: &Held, chat: u32, event: Event) -> purlis_core::hookwire::Report {
    use purlis_core::hookwire::Conversation;
    purlis_core::hookwire::Report {
        chat,
        event,
        conversation: held
            .board()
            .conversation(chat)
            .map_or(Conversation::Unknown, Conversation::Named),
        pid: Some(4000 + chat),
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

/// Everything that waits for `whose`, whole, oldest first: taken, as its next turn takes it.
fn told(held: &Held, whose: For<'_>) -> Vec<Handback> {
    purlis_core::handback::take(held.root(), whose)
}

/// The line purlis types into a task whose turn ended without its report.
fn the_report_ask() -> Vec<u8> {
    format!(
        "{}\r",
        crate::curation::bracketed(&purlis_core::dispatched::nudge(&[
            purlis_core::dispatched::Landed::ReportOwed
        ]))
    )
    .into_bytes()
}

#[test]
fn a_task_that_ends_two_turns_without_a_report_is_asked_once_then_reported_for() {
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Looking at the logs.");

    // Asked once, in purlis's line, and nothing reaches the chat that asked yet.
    assert_eq!(host.typed(task), vec![the_report_ask()]);
    assert!(told(&held, For::Chat(steward)).is_empty());

    // The turn that line started ends without a report too.
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Checked: 3 errors, all from the cache.");

    let word = told(&held, For::Chat(steward));
    assert_eq!(word.len(), 1, "{word:?}");
    assert_eq!(word[0].summary, "Checked: 3 errors, all from the cache.");
    assert_eq!(
        word[0].task.as_ref().map(|task| task.outcome),
        Some(Outcome::LastMessage)
    );
    // Typed once: the made report is not asked for.
    assert_eq!(host.typed(task), vec![the_report_ask()]);
    // Its record says how, and its program runs on: it may still send its own report.
    let (record_id, _) = crate::finished::task_record(&held, task).expect("it has a record");
    let recorded = |held: &Held| {
        purlis_core::dispatchrecord::read(held.root(), &record_id)
            .and_then(|record| record.report)
            .map(|report| (report.outcome, report.text))
    };
    assert_eq!(
        recorded(&held),
        Some((
            purlis_core::dispatchrecord::Outcome::LastMessage,
            "Checked: 3 errors, all from the cache.".to_owned()
        ))
    );
    assert!(open_chats(&held).contains(&task));

    // Its own report, sent later, reaches the chat that asked as well.
    let said = reports(&held, &id, task);
    assert!(
        matches!(said, Answer::Reported { .. } | Answer::Finished { .. }),
        "{said:?}"
    );
    let word = told(&held, For::Chat(steward));
    assert_eq!(word.len(), 1, "{word:?}");
    assert_eq!(
        word[0].task.as_ref().map(|task| task.outcome),
        Some(Outcome::Done)
    );
    // And it takes the made one's place on the task's dispatch record.
    assert_eq!(
        recorded(&held).map(|(outcome, _)| outcome),
        Some(purlis_core::dispatchrecord::Outcome::Done)
    );
}

#[test]
fn a_task_that_reported_is_never_asked_and_nothing_is_made_for_it() {
    let (_plane, host, _planes, id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);
    let said = reports(&held, &id, task);
    assert!(
        matches!(said, Answer::Reported { .. } | Answer::Finished { .. }),
        "{said:?}"
    );
    its_turn_ends_saying(&held, task, "Reported.");
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Still here.");

    assert_eq!(host.typed(task), Vec::<Vec<u8>>::new());
    let word = told(&held, For::Chat(steward));
    assert_eq!(word.len(), 1, "its own report alone: {word:?}");
    assert_eq!(
        word[0].task.as_ref().map(|task| task.outcome),
        Some(Outcome::Done)
    );
}

#[test]
fn what_a_chat_that_is_no_task_said_is_not_kept() {
    let (_plane, _host, _planes, _id, held, steward, _task) = a_steward_and_its_task();
    a_turn_begins(&held, steward);
    its_turn_ends_saying(&held, steward, "The steward's own words.");
    assert_eq!(held.tasks().ledger().last_said(steward), None);
}

// ----- reading what a task said and ran (#1981) ----------------------------------------------

#[test]
fn the_chat_that_asked_reads_what_its_task_said_and_ran_and_no_other_chat_does() {
    use purlis_core::dispatched::{Answered, Ran, What};
    let (plane, _host, _planes, id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);
    crate::dispatched::said(
        &held,
        TurnOutput {
            chat: task,
            turn_output: Output::Ran {
                command: "cargo test -p core".to_owned(),
                exit: Some(101),
                failed: true,
            },
            speaker: purlis_core::hookwire::Speaker::default(),
        },
    );
    its_turn_ends_saying(&held, task, "Two tests fail in core.");

    let said = asks(&held, &id, steward, What::Output { of: task });
    let Answer::Task(answered) = said else {
        panic!("answered, not {said:?}");
    };
    assert_eq!(
        *answered,
        Answered::Output {
            of: task,
            name: "check prod".to_owned(),
            finals: vec!["Two tests fail in core.".to_owned()],
            ran: vec![Ran::shown("cargo test -p core", Some(101), true)],
        }
    );

    // Another chat of the project, which did not dispatch it, is refused.
    let alpha = held.root().join("workspaces").join("alpha");
    let other = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    let said = asks(&held, &id, other, What::Output { of: task });
    assert!(matches!(said, Answer::No { .. }), "{said:?}");
    // And the task reads nothing of itself through it.
    let said = asks(&held, &id, task, What::Output { of: task });
    assert!(matches!(said, Answer::No { .. }), "{said:?}");
}

// ----- a task gone quiet (#1981, D-6) ----------------------------------------------------------

#[test]
fn a_task_unheard_for_its_quiet_limit_is_noted_to_the_chat_that_asked_and_on_its_tab() {
    use std::time::{Duration, Instant};
    let (_plane, _host, _planes, _id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);

    // Inside the limit: nothing.
    crate::silence::look(&held, Instant::now() + Duration::from_secs(14 * 60));
    assert_eq!(held.tasks().quiet().note_of(task), None);

    // Past it: purlis's note, on its tab and for the chat that asked. Nothing ends.
    crate::silence::look(&held, Instant::now() + Duration::from_secs(16 * 60));
    let said = held.tasks().quiet().note_of(task).expect("noted");
    assert!(
        said.starts_with(&format!(
            "task 'check prod' (chat {task}) has shown no activity for"
        )),
        "{said}"
    );
    let words = held.tasks().ledger().talk.from_person(steward);
    assert_eq!(words.len(), 1, "{words:?}");
    assert!(
        purlis_core::dispatchtalk::person_said(&words[0]).contains(&said),
        "{words:?}"
    );
    assert!(open_chats(&held).contains(&task), "it ends nothing");

    // Once a stretch.
    crate::silence::look(&held, Instant::now() + Duration::from_secs(40 * 60));
    assert_eq!(held.tasks().ledger().talk.from_person(steward).len(), 1);

    // Heard from again: its tab's note comes down.
    crate::dispatched::said(
        &held,
        TurnOutput {
            chat: task,
            turn_output: Output::Ran {
                command: "ls".to_owned(),
                exit: Some(0),
                failed: false,
            },
            speaker: purlis_core::hookwire::Speaker::default(),
        },
    );
    assert_eq!(held.tasks().quiet().note_of(task), None);
}

#[test]
fn a_task_whose_turn_has_ended_is_not_noted_as_quiet() {
    use std::time::{Duration, Instant};
    let (_plane, _host, _planes, _id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Done for now.");
    crate::silence::look(&held, Instant::now() + Duration::from_secs(60 * 60));
    assert_eq!(held.tasks().quiet().note_of(task), None);
    assert!(
        held.tasks()
            .ledger()
            .talk
            .from_person(steward)
            .iter()
            .all(|one| !matches!(one, purlis_core::dispatchtalk::PersonSaid::Quiet { .. }))
    );
}

// ----- closing a finished task (#1981 D-4) ---------------------------------------------------

#[test]
fn the_chat_that_asked_closes_its_finished_task_and_its_row_and_nothing_else() {
    use purlis_core::dispatched::{Answered, What};
    let (plane, _host, _planes, id, held, steward, task) = a_steward_and_its_task();

    // Still owing its report: not finished, and not closed.
    let said = asks(&held, &id, steward, What::Close { of: task });
    assert!(matches!(said, Answer::No { .. }), "{said:?}");
    assert!(open_chats(&held).contains(&task));

    // Its turns end without a report, and purlis reports for it from its last message: it is
    // finished, and stays open owing one more.
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Looked.");
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Done looking.");
    assert!(open_chats(&held).contains(&task));

    // Another chat of the project, which did not dispatch it, cannot close it.
    let alpha = held.root().join("workspaces").join("alpha");
    let other = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    let said = asks(&held, &id, other, What::Close { of: task });
    assert!(matches!(said, Answer::No { .. }), "{said:?}");
    assert!(open_chats(&held).contains(&task));

    let said = asks(&held, &id, steward, What::Close { of: task });
    let Answer::Task(answered) = said else {
        panic!("closed, not {said:?}");
    };
    assert_eq!(
        *answered,
        Answered::Closed {
            of: task,
            name: "check prod".to_owned()
        }
    );
    assert!(!open_chats(&held).contains(&task), "its program is ended");
    assert!(
        crate::finished::listed(&held)
            .iter()
            .all(|row| row.chat != Some(task)),
        "its row is cleared"
    );
}

/// A steward chat and its task, which has reported (purlis made its report from its last
/// message) and stays open: a finished task its asking chat may close.
fn a_finished_open_task() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_its_task();
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Looked.");
    a_turn_begins(&held, task);
    its_turn_ends_saying(&held, task, "Done looking.");
    assert!(open_chats(&held).contains(&task));
    (plane, host, planes, id, held, steward, task)
}

#[test]
fn a_finished_task_is_not_closed_while_it_or_the_person_is_at_it() {
    use purlis_core::dispatched::What;
    type Arrange = fn(&Plane, &PlaneId, &Held, u32, u32);
    // A finished task the person stops is ended at once (its stop has no report to wait on), so
    // a close never meets one being stopped from the window; that refusal is the lock's guard.
    let cases: [(&str, Arrange); 5] = [
        ("in a turn", |_, _, held, _, task| a_turn_begins(held, task)),
        ("a prompt showing", |_, _, held, _, task| {
            a_turn_begins(held, task);
            the_board_hears(held, task, Event::Notification);
        }),
        ("the person typed in it", |_, _, held, _, task| {
            crate::dispatched::person_typed(held, task, b"wa", Some(Some(2)));
        }),
        ("a task of its own at work", |_, id, held, _, task| {
            let _below = a_task_of(held, id, task, "look deeper");
        }),
        ("the person looking at it", |_, _, held, _, task| {
            held.chats().bring_to_front(Some(task));
        }),
    ];
    for (why, arrange) in cases {
        let (plane, _host, _planes, id, held, steward, task) = a_finished_open_task();
        arrange(&plane, &id, &held, steward, task);
        let said = asks(&held, &id, steward, What::Close { of: task });
        assert!(matches!(said, Answer::No { .. }), "{why}: {said:?}");
        assert!(open_chats(&held).contains(&task), "{why}: still open");
    }
}

#[test]
fn closing_a_task_is_pre_allowed_for_claude_code_by_its_own_name() {
    // #3785, ruled 2026-10-29: tidying its own tasks is the chat's own. The app refuses a close
    // of a task that is not finished, that the person typed in or is looking at, so the
    // harness's prompt beside it asked nothing the person decides.
    assert!(
        purlis_core::harness::claude::DISPATCH_TASK_ALLOW
            .contains(&"Bash(purlis dispatch close *)")
    );
}
