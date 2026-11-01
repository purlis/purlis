// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task's open question across a restart of the app (#4553, ADR 0107): each test is a chat
//! asking, the app starting again on what it kept, and a chat or the person answering.

use super::*;

const ASKER: u32 = 3;
const TASK: u32 = 9;
const NOW: u64 = 1_800_000_000;

/// A project with a record of messages, and the app's `Talk` on it, as it first needs it.
struct Project {
    root: tempfile::TempDir,
    record: PathBuf,
}

impl Project {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a project");
        let record = root.path().join("spool/records.json");
        Self { root, record }
    }

    fn app(&self) -> Talk {
        let mut talk = Talk::default();
        talk.keep_at(&self.record);
        talk
    }

    /// The task asks its asking chat `said`, left in a file for that chat's next turn.
    fn asks(&self, talk: &mut Talk, said: &str) -> u32 {
        talk.known_as(TASK, "task-id", ASKER);
        let message = Message {
            kind: Kind::Question,
            from: "task".to_owned(),
            chat: TASK,
            text: said.to_owned(),
            instruction: None,
        };
        let file = leave(self.root.path(), ASKER, &message).expect("left");
        talk.ask(TASK, said, Some(file)).expect("asked")
    }

    /// The app starts again, and the task is brought back.
    fn restarted(&self) -> (Talk, Restored) {
        let mut after = self.app();
        let back = after.restored(TASK, "task-id", self.root.path(), NOW);
        (after, back)
    }
}

#[test]
fn an_open_question_survives_a_restart_with_who_was_asked_and_how_it_stands() {
    let project = Project::new();
    let mut before = project.app();
    let number = project.asks(&mut before, "Which host?");
    before.reach(TASK, Reached::NoReply, None, NOW);

    let (after, back) = project.restarted();

    assert_eq!(back, Restored::Open);
    assert_eq!(after.open(TASK), Some((number, "Which host?")));
    assert_eq!(after.asked_of(TASK), Some(ASKER));
    assert_eq!(
        after.standing(TASK),
        Some(&Standing::ReachedPerson {
            why: Reached::NoReply,
            at: NOW,
            note: None,
        })
    );
}

#[test]
fn a_question_asked_after_a_restart_has_a_number_of_its_own() {
    let project = Project::new();
    let mut before = project.app();
    let first = project.asks(&mut before, "Which host?");
    before.close(TASK);

    let (mut after, back) = project.restarted();

    assert_eq!(back, Restored::Nothing);
    let second = project.asks(&mut after, "Which host?");
    assert!(second > first, "{second} after {first}");
}

#[test]
fn the_asking_chat_s_answer_after_a_restart_reaches_the_task() {
    let project = Project::new();
    let mut before = project.app();
    let number = project.asks(&mut before, "Which host?");

    let (mut after, _) = project.restarted();

    let given = Given {
        from: "steward".to_owned(),
        text: "The blue one.".to_owned(),
        file: None,
        by_person: false,
    };
    assert_eq!(after.answer(TASK, ASKER, "task", given), Ok(number));
    assert_eq!(
        after.answered(TASK).map(|given| given.text.as_str()),
        Some("The blue one.")
    );
    assert_eq!(after.asks(TASK), None);
}

#[test]
fn the_person_s_answer_after_a_restart_reaches_the_task_as_theirs_and_takes_the_unread_question() {
    let project = Project::new();
    let mut before = project.app();
    let number = project.asks(&mut before, "Which host?");

    let (mut after, _) = project.restarted();

    let unread = after
        .person_answers(
            TASK,
            "task",
            Some(ASKER),
            number,
            "Which host?",
            "The blue one.",
        )
        .expect("taken")
        .expect("the question still waited for the asking chat's next turn");
    assert!(unread.exists(), "{}", unread.display());
    assert!(after.answered(TASK).is_some_and(|given| given.by_person));
    assert_eq!(after.from_person(TASK).len(), 1);
}

#[test]
fn a_question_answered_and_closed_before_the_restart_does_not_come_back() {
    let project = Project::new();
    let mut before = project.app();
    project.asks(&mut before, "Which host?");
    let given = Given {
        from: "steward".to_owned(),
        text: "The blue one.".to_owned(),
        file: None,
        by_person: false,
    };
    before.answer(TASK, ASKER, "task", given).expect("answered");
    before.got_answer(TASK);

    let (after, back) = project.restarted();

    assert_eq!(back, Restored::Nothing);
    assert_eq!(after.open(TASK), None);
}

#[test]
fn the_person_s_answer_a_task_never_had_leaves_the_question_open_after_a_restart() {
    // What the person said is in no file (#1496), so a restart before the task has it asks
    // again: the person's word is never written where a chat could write one.
    let project = Project::new();
    let mut before = project.app();
    let number = project.asks(&mut before, "Which host?");
    before
        .person_answers(
            TASK,
            "task",
            Some(ASKER),
            number,
            "Which host?",
            "The blue one.",
        )
        .expect("taken");

    assert!(
        !std::fs::read_to_string(&project.record)
            .expect("kept")
            .contains("The blue one."),
    );
    let (after, back) = project.restarted();

    assert_eq!(back, Restored::Open);
    assert_eq!(after.open(TASK), Some((number, "Which host?")));
}

#[test]
fn a_saved_question_is_masked_as_a_report_is() {
    let project = Project::new();
    let token = format!("{}{}", "ghp_", "0123456789abcdefghijABCDEFGHIJ012345");
    let mut before = project.app();
    project.asks(&mut before, &format!("Is {token} the right token?"));

    assert!(
        !std::fs::read_to_string(&project.record)
            .expect("kept")
            .contains(&token),
    );
    let (after, _) = project.restarted();
    let (_, text) = after.open(TASK).expect("still open");
    assert!(!text.contains(&token), "{text}");
    assert!(text.starts_with("Is "), "{text}");
}

/// Rewrites what was kept of the task's question with `change`.
fn tampered(project: &Project, change: impl Fn(&mut serde_json::Value)) {
    let read = std::fs::read(&project.record).expect("kept");
    let mut kept: serde_json::Value = serde_json::from_slice(&read).expect("json");
    change(&mut kept["questions"]["task-id"]);
    std::fs::write(&project.record, serde_json::to_vec(&kept).expect("json")).expect("written");
}

#[test]
fn a_saved_question_past_a_message_s_bound_does_not_read_back_and_the_task_is_told_once() {
    let project = Project::new();
    let mut before = project.app();
    project.asks(&mut before, "Which host?");
    tampered(&project, |question| {
        question["text"] = serde_json::Value::String("a".repeat(5_000));
    });

    let (mut after, back) = project.restarted();

    assert_eq!(back, Restored::Lost);
    assert_eq!(after.open(TASK), None);
    let told = after.from_person(TASK);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(person_said(&told[0]).contains("ask it again"), "{told:?}");
    // And again from a kept record, nothing is told twice: it no longer reads as a question.
    let again = after.restored(TASK, "task-id", project.root.path(), NOW);
    assert_eq!(again, Restored::Nothing);
    assert_eq!(after.from_person(TASK).len(), 1);
}

#[test]
fn a_question_in_a_shape_this_build_cannot_read_is_said_and_the_other_records_stand() {
    let project = Project::new();
    let mut before = project.app();
    before.instruction("asker-id");
    project.asks(&mut before, "Which host?");
    tampered(&project, |question| {
        question["standing"] = serde_json::json!({ "standing": "somewhere_new" });
    });

    let (mut after, back) = project.restarted();

    assert_eq!(back, Restored::Lost);
    assert_eq!(after.instruction("asker-id"), 2, "the rest reads");
}

#[test]
fn a_record_that_does_not_read_tells_each_task_brought_back_to_ask_again() {
    let project = Project::new();
    let mut before = project.app();
    project.asks(&mut before, "Which host?");
    std::fs::write(&project.record, b"not json").expect("written");

    let (mut after, back) = project.restarted();

    assert_eq!(back, Restored::Lost);
    assert_eq!(after.from_person(TASK).len(), 1);
    // The asking chat's answer says why it reaches nothing.
    let given = Given {
        from: "steward".to_owned(),
        text: "The blue one.".to_owned(),
        file: None,
        by_person: false,
    };
    assert_eq!(
        after.answer(TASK, ASKER, "task", given),
        Err(asked_before_the_restart("task", TASK))
    );
}

#[test]
fn a_record_from_a_build_that_kept_no_questions_is_read_as_questions_lost() {
    let project = Project::new();
    let mut before = project.app();
    before.instruction("asker-id");
    let read = std::fs::read(&project.record).expect("kept");
    let mut kept: serde_json::Value = serde_json::from_slice(&read).expect("json");
    kept["v"] = serde_json::json!(1);
    std::fs::write(&project.record, serde_json::to_vec(&kept).expect("json")).expect("written");

    let (mut after, back) = project.restarted();

    assert_eq!(back, Restored::Lost);
    assert_eq!(after.instruction("asker-id"), 2);
}

#[test]
fn a_task_with_no_question_is_told_nothing_of_questions_as_it_comes_back() {
    let project = Project::new();
    project.app().instruction("asker-id");

    let (after, back) = project.restarted();

    assert_eq!(back, Restored::Nothing);
    assert!(after.from_person(TASK).is_empty());
}

#[test]
fn a_question_of_a_task_that_did_not_come_back_is_not_kept_on() {
    let project = Project::new();
    let mut before = project.app();
    project.asks(&mut before, "Which host?");

    let mut after = project.app();
    after.only_these_came_back(&[]);

    let (again, back) = project.restarted();
    assert_eq!(back, Restored::Nothing);
    assert_eq!(again.open(TASK), None);
}

#[test]
fn a_question_handed_before_a_restart_keeps_when_and_one_never_handed_waits_for_its_turn() {
    let project = Project::new();
    let mut before = project.app();
    let number = project.asks(&mut before, "Which host?");
    let (_, message) = waiting(project.root.path(), ASKER).remove(0);
    assert_eq!(message.text, "Which host?");

    let (after, _) = project.restarted();
    assert_eq!(after.handed_at(TASK), None);
    assert_eq!(after.open(TASK).map(|(n, _)| n), Some(number));

    // The file went before the app wrote that it was handed: the turn had it.
    let mut before = project.app();
    let _ = before.restored(TASK, "task-id", project.root.path(), NOW);
    take(project.root.path(), ASKER);
    let (after, _) = project.restarted();
    assert_eq!(after.handed_at(TASK), Some(NOW));
}

#[test]
fn a_chat_started_again_under_a_new_number_keeps_its_question_across_a_restart() {
    let project = Project::new();
    let mut before = project.app();
    project.asks(&mut before, "Which host?");
    before.followed(ASKER, 30);
    before.followed(TASK, 40);

    let mut after = project.app();
    let back = after.restored(40, "task-id", project.root.path(), NOW);

    assert_eq!(back, Restored::Open);
    assert_eq!(after.asked_of(40), Some(30));
}
