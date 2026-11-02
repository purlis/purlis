// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task's question on its route (ADR 0107): chats asking, answering and passing on over the
//! app's own channel, and the person answering from the window.

use super::*;

use crate::asking::{AnswerPath, AskSource, ReachedWhy, every_ask};
use purlis_core::dispatchtalk::Kind;
use std::time::SystemTime;

fn a_steward_and_a_working_task() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "check prod");
    works(&held, task);
    (plane, host, planes, id, held, steward, task)
}

/// What is waiting on the person of `task`'s question.
fn asks_of(held: &Held, task: u32) -> Vec<crate::asking::Shown> {
    every_ask(held)
        .asks
        .into_iter()
        .filter(|ask| ask.session == task && ask.source == AskSource::TaskQuestion)
        .collect()
}

#[test]
fn a_question_with_its_asking_chat_is_not_one_of_the_person_s_asks() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    asks(
        &held,
        &id,
        task,
        What::Question {
            text: "Which one?".to_owned(),
        },
    );

    assert!(asks_of(&held, task).is_empty());
}

#[test]
fn a_passed_on_question_is_one_ask_in_the_task_s_own_words_with_the_chat_s_note_apart() {
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    asks(
        &held,
        &id,
        task,
        What::Question {
            text: "Which one?".to_owned(),
        },
    );

    let said = asks(
        &held,
        &id,
        steward,
        What::PassOn {
            to: task,
            note: Some("It's your call.".to_owned()),
        },
    );

    assert!(matches!(said, Answer::Task(_)), "{said:?}");
    let listed = asks_of(&held, task);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].says, "Which one?");
    assert!(matches!(
        &listed[0].answer,
        AnswerPath::TaskQuestion {
            why: ReachedWhy::PassedOn,
            note: Some(note),
            ..
        } if note == "It's your call."
    ));
}

#[test]
fn only_a_task_s_asking_chat_may_pass_its_question_on() {
    let (plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    let alpha = plane.root.join("workspaces").join("alpha");
    let another = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    asks(
        &held,
        &id,
        task,
        What::Question {
            text: "Which one?".to_owned(),
        },
    );

    for sender in [another, task] {
        let said = asks(
            &held,
            &id,
            sender,
            What::PassOn {
                to: task,
                note: None,
            },
        );
        assert!(matches!(said, Answer::No { .. }), "{said:?}");
    }
    assert!(asks_of(&held, task).is_empty());
}

#[test]
fn the_person_s_answer_to_a_passed_on_question_clears_it_and_reaches_the_task_as_theirs() {
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    asks(
        &held,
        &id,
        task,
        What::Question {
            text: "Which one?".to_owned(),
        },
    );
    asks(
        &held,
        &id,
        steward,
        What::PassOn {
            to: task,
            note: None,
        },
    );
    let shown = crate::dispatched::question_of(&held, task).expect("open");

    crate::dispatched::person_answers(&held, task, shown.number, &shown.question, "The blue one")
        .expect("taken");

    assert!(asks_of(&held, task).is_empty());
    assert!(
        held.tasks()
            .ledger()
            .talk
            .answered(task)
            .is_some_and(|given| given.by_person)
    );
}

/// What the app answers chat `chat`'s harness asking to route `text`.
fn routed(held: &Held, id: &PlaneId, chat: u32, text: &str) -> Answer {
    asks(
        held,
        id,
        chat,
        What::Route {
            text: text.to_owned(),
        },
    )
}

#[test]
fn a_task_s_own_question_goes_to_its_asking_chat_by_the_app_s_record() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();

    let said = routed(&held, &id, task, "Which one?");

    assert!(
        matches!(&said, Answer::Task(answered)
            if matches!(**answered, Answered::Sent { kind: Kind::Question, .. })),
        "{said:?}"
    );
    assert_eq!(held.tasks().ledger().talk.asks(task), Some("Which one?"));
}

#[test]
fn a_task_paused_on_a_question_is_told_so_and_its_second_goes_nowhere() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    routed(&held, &id, task, "Which one?");

    let said = routed(&held, &id, task, "And when?");

    assert!(
        matches!(&said, Answer::Task(answered)
            if matches!(**answered, Answered::AlreadyAsking { .. })),
        "{said:?}"
    );
    assert_eq!(held.tasks().ledger().talk.asks(task), Some("Which one?"));
}

#[test]
fn a_chat_with_no_asking_chat_asks_the_person_as_it_always_did() {
    let (_plane, _host, _planes, id, held, steward, _task) = a_steward_and_a_working_task();

    let said = routed(&held, &id, steward, "Which one?");

    assert!(
        matches!(&said, Answer::Task(answered) if matches!(**answered, Answered::ToThePerson)),
        "{said:?}"
    );
    assert_eq!(held.tasks().ledger().talk.asks(steward), None);
}

#[test]
fn a_redirect_the_app_refuses_leaves_the_question_to_the_person() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();

    let said = routed(&held, &id, task, &"a".repeat(5_000));

    assert!(matches!(said, Answer::No { .. }), "{said:?}");
    assert_eq!(held.tasks().ledger().talk.asks(task), None);
}

#[test]
fn a_question_whose_asking_chat_has_closed_reaches_the_person_with_that_reason() {
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    routed(&held, &id, task, "Which one?");

    // The close itself moves it: nothing waits for the clock.
    held.close_chat(steward).expect("closed");

    let listed = asks_of(&held, task);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        matches!(
            &listed[0].answer,
            AnswerPath::TaskQuestion {
                why: ReachedWhy::AskerGone,
                from,
                ..
            } if !from.is_empty()
        ),
        "the row names the chat that closed"
    );
}

#[test]
fn a_quit_is_no_asking_chat_going_and_a_question_keeps_its_route() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    routed(&held, &id, task, "Which one?");

    held.chats().end_all();
    crate::dispatched::look_at_questions(&held);

    assert_eq!(
        held.tasks().ledger().talk.standing(task),
        Some(&purlis_core::dispatchtalk::Standing::WithAskingChat)
    );
}

#[test]
fn a_handoff_asks_the_person_as_it_always_did() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let (said, _) = a_handoff(&held, &id, steward, None, None, INTO_ALPHA);
    let Answer::Opened { chat, .. } = said else {
        panic!("a handoff opens a chat")
    };

    let said = routed(&held, &id, chat, "Which one?");

    assert!(
        matches!(&said, Answer::Task(answered) if matches!(**answered, Answered::ToThePerson)),
        "{said:?}"
    );
}

#[test]
fn a_task_the_person_started_from_a_tab_asks_the_person_as_it_always_did() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let theirs = ask_from_the_tab(&held, &id, steward, "devops", "check prod")
        .expect("the person's ask starts")
        .session;

    let said = routed(&held, &id, theirs, "Which one?");

    assert!(
        matches!(&said, Answer::Task(answered) if matches!(**answered, Answered::ToThePerson)),
        "{said:?}"
    );
}

#[test]
fn a_question_a_turn_was_handed_waits_its_time_and_then_reaches_the_person() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    routed(&held, &id, task, "Which one?");
    crate::dispatched::look_at_questions(&held);
    assert!(asks_of(&held, task).is_empty(), "no turn has seen it");

    let long_ago = purlis_core::dispatchtalk::A_QUESTION_WAITS_SECS + 1;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("the clock")
        .as_secs();
    held.tasks().ledger().talk.show(task);
    let gone = std::collections::HashSet::new();
    let moved = held
        .tasks()
        .ledger()
        .talk
        .look(now + long_ago, &gone, &gone);

    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(asks_of(&held, task).len(), 1);
}
