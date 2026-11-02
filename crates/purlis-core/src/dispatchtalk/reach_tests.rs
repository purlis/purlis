// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task's question reaches the person by itself (#4556, ADR 0107): each test is a question
//! its asking chat did nothing with, a clock moving, and that chat closing.

use super::*;

const ASKER: u32 = 3;
const TASK: u32 = 9;
const NESTED: u32 = 12;
const NOW: u64 = 1_800_000_000;

fn nobody() -> HashSet<u32> {
    HashSet::new()
}

fn a_task_that_asked() -> Talk {
    let mut talk = Talk::default();
    talk.known_as(TASK, "task-id", ASKER);
    talk.ask(TASK, "Which host?", Some(PathBuf::from("said-3/q.json")))
        .expect("asked");
    talk
}

#[test]
fn a_question_no_turn_was_handed_waits_however_long_it_has_been() {
    let mut talk = a_task_that_asked();

    assert_eq!(talk.look(NOW + 86_400, &nobody(), &nobody()), vec![]);

    assert_eq!(talk.standing(TASK), Some(&Standing::WithAskingChat));
}

#[test]
fn the_clock_starts_when_a_turn_is_handed_the_question() {
    let mut talk = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);

    assert_eq!(talk.handed_at(TASK), Some(NOW));
    assert_eq!(
        talk.look(NOW + A_QUESTION_WAITS_SECS - 1, &nobody(), &nobody()),
        vec![]
    );
    assert_eq!(
        talk.look(NOW + A_QUESTION_WAITS_SECS, &nobody(), &nobody()),
        vec![(TASK, Reached::NoReply)]
    );
    assert_eq!(talk.reached()[0].why, Reached::NoReply);
}

#[test]
fn another_message_handed_to_a_turn_starts_no_clock() {
    let mut talk = a_task_that_asked();
    talk.handed(&["note.json".to_owned()], NOW);
    assert_eq!(talk.handed_at(TASK), None);
}

#[test]
fn a_wait_that_shows_the_asking_chat_the_question_starts_the_clock_too() {
    let mut talk = a_task_that_asked();
    talk.show(TASK);
    assert!(talk.handed_at(TASK).is_some());
}

#[test]
fn a_closed_or_crashed_asking_chat_sends_the_question_at_once_with_its_reason() {
    let mut talk = a_task_that_asked();

    let moved = talk.look(NOW, &HashSet::from([ASKER]), &nobody());

    assert_eq!(moved, vec![(TASK, Reached::AskerGone)]);
}

#[test]
fn a_question_that_reached_the_person_stays_with_no_limit_and_no_answer_of_its_own() {
    let mut talk = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);
    talk.look(NOW + A_QUESTION_WAITS_SECS, &nobody(), &nobody());
    let reached = talk.reached();

    assert_eq!(
        talk.look(NOW + 30 * 86_400, &HashSet::from([ASKER]), &nobody()),
        vec![]
    );

    assert_eq!(talk.reached(), reached, "the first reason and time stand");
    assert_eq!(talk.answered(TASK), None);
    assert!(talk.asks(TASK).is_some());
}

#[test]
fn a_question_answered_in_its_time_goes_nowhere() {
    let mut talk = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);
    let given = Given {
        from: "steward".to_owned(),
        text: "Blue.".to_owned(),
        file: None,
        by_person: false,
    };
    talk.answer(TASK, ASKER, "task", given).expect("answered");

    assert_eq!(talk.look(NOW + 500, &nobody(), &nobody()), vec![]);
    assert!(talk.reached().is_empty());
}

#[test]
fn a_nested_question_goes_to_the_chat_above_it_and_passed_on_it_is_one_item() {
    let mut talk = a_task_that_asked();
    talk.known_as(NESTED, "nested-id", TASK);
    talk.ask(NESTED, "Which port?", None).expect("asked");
    assert_eq!(talk.asked_of(NESTED), Some(TASK), "one level up");

    talk.pass_on(NESTED, TASK, "nested", None, NOW)
        .expect("passed");

    let reached = talk.reached();
    assert_eq!(reached.len(), 1, "{reached:?}");
    assert_eq!(reached[0].task, NESTED);
    assert_eq!(reached[0].text, "Which port?");
    assert_eq!(talk.standing(TASK), Some(&Standing::WithAskingChat));
}

#[test]
fn a_task_s_ending_closes_its_question_and_says_so_once() {
    let mut talk = a_task_that_asked();
    let number = talk.open(TASK).expect("open").0;

    talk.close(TASK);
    talk.forget(TASK);

    assert_eq!(
        talk.take_closed_unanswered(),
        vec![ClosedUnanswered { task: TASK, number }]
    );
    assert_eq!(talk.take_closed_unanswered(), vec![]);
    assert!(talk.reached().is_empty());
}

#[test]
fn an_answered_question_closes_with_nothing_to_say() {
    let mut talk = a_task_that_asked();
    let given = Given {
        from: "steward".to_owned(),
        text: "Blue.".to_owned(),
        file: None,
        by_person: false,
    };
    talk.answer(TASK, ASKER, "task", given).expect("answered");

    talk.close(TASK);

    assert_eq!(talk.take_closed_unanswered(), vec![]);
}

#[test]
fn an_asking_chat_stuck_on_a_prompt_or_a_limit_starts_the_clock_without_a_turn() {
    let mut talk = a_task_that_asked();
    let stuck = HashSet::from([ASKER]);

    assert_eq!(talk.look(NOW, &nobody(), &stuck), vec![]);
    assert_eq!(talk.handed_at(TASK), Some(NOW));
    assert_eq!(
        talk.look(NOW + A_QUESTION_WAITS_SECS - 1, &nobody(), &stuck),
        vec![]
    );

    assert_eq!(
        talk.look(NOW + A_QUESTION_WAITS_SECS, &nobody(), &stuck),
        vec![(TASK, Reached::NoReply)]
    );
}

#[test]
fn a_clock_a_stuck_chat_started_is_not_started_again() {
    let mut talk = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);

    talk.look(NOW + 50, &nobody(), &HashSet::from([ASKER]));

    assert_eq!(talk.handed_at(TASK), Some(NOW));
}

#[test]
fn the_asking_chats_not_yet_handed_a_question_are_the_ones_told_again() {
    let mut talk = a_task_that_asked();
    assert_eq!(talk.askers_not_handed(), HashSet::from([ASKER]));

    talk.handed(&["q.json".to_owned()], NOW);

    assert!(talk.askers_not_handed().is_empty());
}
