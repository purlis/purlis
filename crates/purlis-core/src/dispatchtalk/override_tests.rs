// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's later word overrides an agent's** (#4566, spec #4551 story 37): a task's
//! asking chat answered its question first, and the person, shown that answer, says otherwise.
//! Each test is a task asking, its asking chat answering, and the person answering after it.

use super::*;

const ASKER: u32 = 3;
const TASK: u32 = 9;

/// `TASK`, called "check the queue", asked `ASKER` "Which queue?", and `ASKER`, called
/// "steward 3", answered "The second one.".
fn answered_by_the_asking_chat() -> (Talk, u32) {
    let mut talk = Talk::default();
    let number = talk
        .ask(TASK, "Which queue?", Some("question.json".into()))
        .expect("asked");
    talk.answer(
        TASK,
        ASKER,
        "check the queue",
        Given {
            from: "steward 3".to_owned(),
            text: "The second one.".to_owned(),
            file: Some("answer.json".into()),
            by_person: false,
        },
    )
    .expect("the asking chat answers");
    (talk, number)
}

fn the_person_says_otherwise(talk: &mut Talk, number: u32) -> Result<Option<PathBuf>, String> {
    talk.person_answers(
        TASK,
        "check the queue",
        Some(ASKER),
        number,
        "Which queue?",
        "The first one.",
    )
}

fn handed(talk: &mut Talk, chat: u32) -> Vec<PersonSaid> {
    let said = talk.from_person(chat);
    let numbers: Vec<u32> = said.iter().map(PersonSaid::number).collect();
    talk.handed_from_person(chat, &numbers);
    said
}

#[test]
fn the_window_reads_who_answered_a_question_and_what_they_said() {
    let (talk, number) = answered_by_the_asking_chat();

    assert_eq!(
        talk.chat_answered(TASK, number),
        Some(ChatAnswered {
            from: "steward 3".to_owned(),
            text: "The second one.".to_owned(),
            question: "Which queue?".to_owned(),
        })
    );
    assert_eq!(
        talk.chat_answered(TASK, number + 1),
        None,
        "only that question"
    );
}

#[test]
fn what_an_asking_chat_answered_is_masked_for_the_window() {
    // A forge token's shape, put together here so no scan of this file takes it for one.
    let token = ["ghp", "_", "abcdefghijklmnopqrstuvwxyz0123456789"].concat();
    let mut talk = Talk::default();
    let number = talk.ask(TASK, "Which token?", None).expect("asked");
    talk.answer(
        TASK,
        ASKER,
        "check the queue",
        Given {
            from: "steward 3".to_owned(),
            text: format!("Use {token}"),
            file: None,
            by_person: false,
        },
    )
    .expect("answered");

    let shown = talk.chat_answered(TASK, number).expect("answered");
    assert!(!shown.text.contains(&token));
}

#[test]
fn the_person_s_later_word_reaches_the_task_as_theirs_marked_as_overriding() {
    let (mut talk, number) = answered_by_the_asking_chat();

    assert_eq!(the_person_says_otherwise(&mut talk, number), Ok(None));

    assert_eq!(
        handed(&mut talk, TASK),
        [PersonSaid::Overrode {
            number,
            question: "Which queue?".to_owned(),
            text: "The first one.".to_owned(),
            chat: "steward 3".to_owned(),
        }]
    );
    assert_eq!(
        handed(&mut talk, ASKER),
        [PersonSaid::OverrodeFor {
            number,
            task: sender_of("check the queue"),
            chat: TASK,
            question: "Which queue?".to_owned(),
            text: "The first one.".to_owned(),
        }]
    );
}

#[test]
fn the_overriding_word_is_said_as_the_person_s_replacing_the_chat_s_and_quoted_as_data() {
    let said = person_said(&PersonSaid::Overrode {
        number: 1,
        question: "Which queue?".to_owned(),
        text: "The first one.".to_owned(),
        chat: "steward 3".to_owned(),
    });

    assert!(said.starts_with("⬢ **The person answered your question, overriding"));
    assert!(said.contains("`steward 3`"));
    assert!(said.contains("in place of the answer"));
    assert!(said.contains("> The first one."));
    assert!(said.contains("approves nothing"));
}

#[test]
fn the_asking_chat_is_told_its_answer_was_overridden() {
    let said = person_said(&PersonSaid::OverrodeFor {
        number: 4,
        task: "check the queue".to_owned(),
        chat: TASK,
        question: "Which queue?".to_owned(),
        text: "The first one.".to_owned(),
    });

    assert!(said.starts_with("⬢ **The person overrode your answer to question 4"));
    assert!(said.contains("> The first one."));
}

#[test]
fn an_agent_s_answer_stays_the_agent_s_and_is_never_marked_as_the_person_s() {
    let (mut talk, number) = answered_by_the_asking_chat();
    the_person_says_otherwise(&mut talk, number).expect("overridden");

    let given = talk
        .answered(TASK)
        .expect("the chat's answer is still the task's to read");
    assert!(!given.by_person);
    assert_eq!(given.from, "steward 3");
}

#[test]
fn the_overriding_word_outlives_the_task_reading_the_chat_s_answer() {
    let (mut talk, number) = answered_by_the_asking_chat();
    the_person_says_otherwise(&mut talk, number).expect("overridden");

    // The task's waiting command has the chat's answer: the person's word is still to come.
    talk.got_answer(TASK);

    assert!(matches!(
        talk.from_person(TASK).as_slice(),
        [PersonSaid::Overrode { .. }]
    ));
    assert!(talk.unread_answer(TASK));
}

#[test]
fn the_person_overrides_after_the_task_has_moved_on_with_the_chat_s_answer() {
    let (mut talk, number) = answered_by_the_asking_chat();
    talk.got_answer(TASK);
    talk.turn_began(TASK);

    assert_eq!(the_person_says_otherwise(&mut talk, number), Ok(None));
    assert_eq!(handed(&mut talk, TASK).len(), 1);
}

#[test]
fn the_person_overrides_once_and_a_second_word_is_refused_saying_so() {
    let (mut talk, number) = answered_by_the_asking_chat();
    the_person_says_otherwise(&mut talk, number).expect("overridden");

    assert_eq!(
        the_person_says_otherwise(&mut talk, number),
        Err(not_the_person_s_to_answer(
            "check the queue",
            Some(&Closed::ByPerson)
        ))
    );
    assert_eq!(talk.chat_answered(TASK, number), None);
}

#[test]
fn a_word_for_an_answered_question_is_refused_once_the_task_asks_another() {
    let (mut talk, number) = answered_by_the_asking_chat();
    talk.got_answer(TASK);
    talk.ask(TASK, "Which queue?", None).expect("asked again");

    assert_eq!(
        the_person_says_otherwise(&mut talk, number),
        Err(another_question("check the queue"))
    );
    assert_eq!(talk.chat_answered(TASK, number), None);
}

#[test]
fn a_word_for_other_words_than_the_question_shown_is_refused() {
    let (mut talk, number) = answered_by_the_asking_chat();

    assert_eq!(
        talk.person_answers(
            TASK,
            "check the queue",
            Some(ASKER),
            number,
            "Which region?",
            "The first one.",
        ),
        Err(another_question("check the queue"))
    );
}

#[test]
fn the_asking_chat_answers_the_task_s_next_question_only_once_told_it_was_overridden() {
    let (mut talk, number) = answered_by_the_asking_chat();
    the_person_says_otherwise(&mut talk, number).expect("overridden");
    talk.got_answer(TASK);
    talk.ask(TASK, "And which region?", None).expect("asked");

    assert_eq!(
        talk.may_answer(TASK, ASKER, "check the queue"),
        Err(not_told_yet("check the queue", TASK))
    );
    handed(&mut talk, ASKER);
    assert!(talk.may_answer(TASK, ASKER, "check the queue").is_ok());
}

#[test]
fn the_override_is_on_the_question_s_trail_as_the_person_s_answer() {
    let (mut talk, number) = answered_by_the_asking_chat();
    talk.take_trail();
    the_person_says_otherwise(&mut talk, number).expect("overridden");

    let trail = talk.take_trail();
    assert!(matches!(
        trail.as_slice(),
        [(TASK, Step::Answered { by: By::Person, .. })]
    ));
}

#[test]
fn a_task_that_ended_takes_no_word_and_forgets_what_its_asking_chat_said() {
    let (mut talk, number) = answered_by_the_asking_chat();
    talk.close(TASK);

    assert_eq!(talk.chat_answered(TASK, number), None);
}
