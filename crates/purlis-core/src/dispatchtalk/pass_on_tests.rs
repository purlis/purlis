// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An asking chat passes a task's question on to the person (#4554, ADR 0107): each test is a
//! task asking, its asking chat passing the question on, and the person or the chat answering.

use super::*;

const ASKER: u32 = 3;
const TASK: u32 = 9;
const NOW: u64 = 1_800_000_000;

fn a_task_that_asked(said: &str) -> (Talk, u32) {
    let mut talk = Talk::default();
    let number = talk.ask(TASK, said, None).expect("asked");
    (talk, number)
}

#[test]
fn a_question_with_the_asking_chat_is_not_waiting_on_the_person() {
    let (talk, _) = a_task_that_asked("Which host?");
    assert_eq!(talk.standing(TASK), Some(&Standing::WithAskingChat));
    assert!(talk.reached().is_empty());
}

#[test]
fn a_passed_on_question_reaches_the_person_in_the_task_s_own_words_with_its_reason() {
    let (mut talk, number) = a_task_that_asked("Which host?");

    let passed = talk.pass_on(
        TASK,
        ASKER,
        "task",
        Some("I can't tell which one is live.".to_owned()),
        NOW,
    );

    assert_eq!(passed, Ok(number));
    assert_eq!(
        talk.reached(),
        vec![ReachedYou {
            task: TASK,
            number,
            text: "Which host?".to_owned(),
            why: Reached::PassedOn,
            at: NOW,
            note: Some("I can't tell which one is live.".to_owned()),
        }]
    );
}

#[test]
fn a_question_is_passed_on_once() {
    let (mut talk, _) = a_task_that_asked("Which host?");
    talk.pass_on(TASK, ASKER, "task", None, NOW)
        .expect("passed");

    assert_eq!(
        talk.pass_on(TASK, ASKER, "task", None, NOW + 1),
        Err(passed_on_already("task", TASK))
    );
    assert_eq!(talk.reached()[0].at, NOW);
}

#[test]
fn a_question_that_is_not_open_cannot_be_passed_on() {
    let mut talk = Talk::default();
    assert_eq!(
        talk.pass_on(TASK, ASKER, "task", None, NOW),
        Err(no_question("task", TASK))
    );
}

#[test]
fn a_question_that_reached_the_person_by_itself_keeps_when_it_did_as_its_asking_chat_passes_it_on()
{
    let (mut talk, _) = a_task_that_asked("Which host?");
    talk.reach(TASK, Reached::NoReply, None, NOW);

    talk.pass_on(TASK, ASKER, "task", Some("Yours.".to_owned()), NOW + 50)
        .expect("passed");

    let reached = &talk.reached()[0];
    assert_eq!(reached.why, Reached::PassedOn);
    assert_eq!(reached.at, NOW);
    assert_eq!(reached.note.as_deref(), Some("Yours."));
}

#[test]
fn the_person_s_answer_to_a_passed_on_question_reaches_the_task_as_theirs() {
    let (mut talk, number) = a_task_that_asked("Which host?");
    talk.pass_on(TASK, ASKER, "task", None, NOW)
        .expect("passed");

    talk.person_answers(
        TASK,
        "task",
        Some(ASKER),
        number,
        "Which host?",
        "The blue one.",
    )
    .expect("taken");

    assert!(talk.answered(TASK).is_some_and(|given| given.by_person));
    assert!(talk.reached().is_empty(), "one answer clears it");
    assert_eq!(
        talk.from_person(TASK),
        vec![PersonSaid::Answered {
            number,
            question: "Which host?".to_owned(),
            text: "The blue one.".to_owned(),
        }]
    );
}

#[test]
fn the_asking_chat_s_answer_to_a_passed_on_question_is_never_the_person_s() {
    let (mut talk, _) = a_task_that_asked("Which host?");
    talk.pass_on(TASK, ASKER, "task", None, NOW)
        .expect("passed");

    let given = Given {
        from: "steward".to_owned(),
        text: "The blue one.".to_owned(),
        file: None,
        by_person: false,
    };
    talk.answer(TASK, ASKER, "task", given).expect("first wins");

    assert!(talk.answered(TASK).is_some_and(|given| !given.by_person));
    assert!(talk.reached().is_empty());
    assert!(talk.from_person(TASK).is_empty());
}

#[test]
fn a_note_is_one_bounded_line_and_masked() {
    assert_eq!(pass_on_note(None), Ok(None));
    assert_eq!(pass_on_note(Some("  ")), Ok(None));
    assert_eq!(
        pass_on_note(Some(" Yours. ")),
        Ok(Some("Yours.".to_owned()))
    );
    assert!(pass_on_note(Some("one\ntwo")).is_err());
    assert!(pass_on_note(Some(&"a".repeat(MOST_NOTE_BYTES + 1))).is_err());
    assert!(pass_on_note(Some("bell\u{7}")).is_err());
    let token = format!("{}{}", "ghp_", "0123456789abcdefghijABCDEFGHIJ012345");
    let kept = pass_on_note(Some(&format!("Is {token} it?")))
        .expect("kept")
        .expect("a note");
    assert!(!kept.contains(&token), "{kept}");
}

#[test]
fn the_question_as_it_is_handed_over_says_what_is_always_passed_on() {
    let told = said(&Message {
        kind: Kind::Question,
        from: "task".to_owned(),
        chat: TASK,
        text: "Which host?".to_owned(),
        instruction: None,
    });
    assert!(told.contains("purlis dispatch pass-on 9"), "{told}");
    for rule in ["Security", "cost", "anything public", "amends a ruling"] {
        assert!(told.contains(rule), "{rule}: {told}");
    }
}
