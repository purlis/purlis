// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task question's trail (#4557, ADR 0075): each test is a question living its life, and what
//! the audit and telemetry were handed of it.

use super::*;
use crate::eventlog::questions::{By, Failure, Step};

const ASKER: u32 = 3;
const TASK: u32 = 9;
const NOW: u64 = 1_800_000_000;

const QUESTION: &str = "Which secret-project-host should it use?";
const NOTE: &str = "The migration-window note";
const ANSWER: &str = "The blue-production-one";

fn a_task_that_asked() -> (Talk, u32) {
    let mut talk = Talk::default();
    talk.known_as(TASK, "task-id", ASKER);
    let number = talk
        .ask(TASK, QUESTION, Some(PathBuf::from("said-3/q.json")))
        .expect("asked");
    (talk, number)
}

fn steps(talk: &mut Talk) -> Vec<Step> {
    talk.take_trail()
        .into_iter()
        .map(|(_, step)| step)
        .collect()
}

#[test]
fn a_question_the_person_answers_leaves_each_step_and_who_answered() {
    let (mut talk, number) = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);
    talk.pass_on(TASK, ASKER, "task", Some(NOTE.to_owned()), NOW)
        .expect("passed");
    talk.person_answers(TASK, "task", Some(ASKER), number, QUESTION, ANSWER)
        .expect("taken");

    let said = steps(&mut talk);

    assert_eq!(said[0], Step::Asked { number });
    assert_eq!(said[1], Step::Shown { number });
    assert_eq!(
        said[2],
        Step::PassedOn {
            number,
            noted: true
        }
    );
    assert!(matches!(
        said[3],
        Step::Reached {
            why: Reached::PassedOn,
            ..
        }
    ));
    assert!(matches!(said[4], Step::Answered { by: By::Person, .. }));
    assert_eq!(
        said[5],
        Step::Closed {
            number,
            by: By::Person
        }
    );
    assert_eq!(said.len(), 6, "{said:?}");
    assert!(steps(&mut talk).is_empty(), "each is handed over once");
}

#[test]
fn a_question_its_asking_chat_answers_is_recorded_as_that_chat_s() {
    let (mut talk, number) = a_task_that_asked();
    let given = Given {
        from: "steward".to_owned(),
        text: ANSWER.to_owned(),
        file: None,
        by_person: false,
    };
    talk.answer(TASK, ASKER, "task", given).expect("answered");

    let said = steps(&mut talk);

    assert!(said.contains(&Step::Closed {
        number,
        by: By::AskingChat
    }));
}

#[test]
fn a_question_that_reaches_the_person_by_itself_says_why_and_how_long_it_had_waited() {
    let (mut talk, _) = a_task_that_asked();
    talk.take_trail();
    talk.questions.get_mut(&TASK).expect("open").asked_at = NOW;
    talk.handed(&["q.json".to_owned()], NOW + 10);

    talk.look(NOW + 10 + A_QUESTION_WAITS_SECS, &HashSet::new());

    assert!(steps(&mut talk).iter().any(|step| matches!(
        step,
        Step::Reached {
            why: Reached::NoReply,
            waited_secs: 110,
            ..
        }
    )));
}

#[test]
fn a_task_ending_closes_its_question_as_nobody_s() {
    let (mut talk, number) = a_task_that_asked();
    talk.close(TASK);

    assert!(steps(&mut talk).contains(&Step::Closed {
        number,
        by: By::Nobody
    }));
}

#[test]
fn neither_the_audit_nor_telemetry_ever_holds_a_question_s_words() {
    let (mut talk, number) = a_task_that_asked();
    talk.handed(&["q.json".to_owned()], NOW);
    talk.pass_on(TASK, ASKER, "task", Some(NOTE.to_owned()), NOW)
        .expect("passed");
    talk.person_answers(TASK, "task", Some(ASKER), number, QUESTION, ANSWER)
        .expect("taken");
    talk.failed(TASK, Failure::NotRedirected);

    let written: Vec<String> = talk
        .take_trail()
        .into_iter()
        .map(|(_, step)| format!("{} {}", step.kind(), step.body()))
        .collect();

    assert!(written.len() > 5, "{written:?}");
    for line in &written {
        for word in [
            "secret-project",
            "migration-window",
            "blue-production",
            QUESTION,
        ] {
            assert!(!line.contains(word), "{word}: {line}");
        }
    }
}

#[test]
fn a_failure_is_said_once_however_often_it_happens() {
    let (mut talk, _) = a_task_that_asked();
    talk.take_trail();

    talk.failed(TASK, Failure::NotRedirected);
    talk.failed(TASK, Failure::NotRedirected);

    assert_eq!(
        steps(&mut talk),
        vec![Step::Failed {
            what: Failure::NotRedirected
        }]
    );
    talk.failed(TASK, Failure::NotRedirected);
    assert!(steps(&mut talk).is_empty());
}

#[test]
fn a_question_that_does_not_read_back_is_a_failure_said_once() {
    let folder = tempfile::tempdir().expect("a project");
    let path = folder.path().join("records.json");
    std::fs::write(&path, b"not json").expect("written");
    let mut talk = Talk::default();
    talk.keep_at(&path);

    talk.restored(TASK, "task-id", folder.path(), NOW);
    talk.restored(TASK, "task-id", folder.path(), NOW);

    assert_eq!(
        steps(&mut talk),
        vec![Step::Failed {
            what: Failure::NotReadBack
        }]
    );
}
