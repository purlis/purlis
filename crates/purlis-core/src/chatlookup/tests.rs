// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dispatchrecord::{
    self, Asker, Changed, ChatRef, EndedBy, EndedWay, Ending, Mode, Opening, Outcome, Place,
    Report, Worker,
};

fn project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

const ASKER_ID: &str = "01K6ASKER0000000000000000A";
const WORKER_ID: &str = "01K6W0RKER000000000000000B";

/// A task chat 641 worked for `steward 3`, opened and not yet ended.
fn task_641(root: &std::path::Path) -> String {
    task_641_as(root, Some(WORKER_ID))
}

/// [`task_641`], its chat given `id`: none for a task no chat ever was.
fn task_641_as(root: &std::path::Path, id: Option<&str>) -> String {
    let opening = Opening {
        mode: Mode::Task,
        asker: Asker {
            chat: ChatRef {
                chat: 3,
                id: Some(ASKER_ID.to_owned()),
                name: "steward 3".to_owned(),
                persona: Some("steward".to_owned()),
            },
            workspace: Some("smart-ide".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("steward".to_owned()),
        worker: Worker {
            chat: ChatRef {
                chat: 641,
                id: id.map(str::to_owned),
                name: "Signed commits #5607".to_owned(),
                persona: Some("steward".to_owned()),
            },
            harness: Some("claude".to_owned()),
            profile: None,
            session_record: None,
        },
        task: Some("Signed commits #5607".to_owned()),
        place: Place {
            workspace: Some("smart-ide".to_owned()),
            folder: Some("workspaces/smart-ide".to_owned()),
            worktree: None,
        },
        brief: "Sign the commits.".to_owned(),
        report_owed: true,
    };
    dispatchrecord::open(root, opening, at("2026-11-11T10:00:00Z"))
        .unwrap()
        .id
}

fn report(outcome: Outcome, text: &str) -> Ending {
    Ending {
        report: Some(Report {
            outcome,
            text: text.to_owned(),
            changed: Changed::default(),
        }),
        usage: None,
    }
}

fn open_chat(number: u32, id: &str) -> OpenChat {
    OpenChat {
        number,
        id: Some(id.to_owned()),
    }
}

#[test]
fn an_open_chat_is_found_open_under_its_number() {
    let (_dir, root) = project();
    let found = by_number_in(&root, 641, &[open_chat(641, WORKER_ID)]);
    assert_eq!(found, Found::Open { number: 641 });
}

#[test]
fn a_finished_task_is_found_ended_with_its_record() {
    let (_dir, root) = project();
    let id = task_641(&root);
    dispatchrecord::close(
        &root,
        &id,
        report(Outcome::Done, "Signed all 12."),
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    let Found::Ended(ended) = by_number_in(&root, 641, &[open_chat(3, ASKER_ID)]) else {
        panic!("not found ended");
    };
    assert_eq!(ended.id, id);
    assert_eq!(ended.worker.chat.id.as_deref(), Some(WORKER_ID));
    assert_eq!(
        dispatchrecord::Finished::of(&ended),
        Some(dispatchrecord::Finished::Done)
    );
}

#[test]
fn a_task_the_person_closed_is_found_ended_as_closed() {
    let (_dir, root) = project();
    let id = task_641(&root);
    dispatchrecord::close_as(
        &root,
        &id,
        report(Outcome::Stopped, "closed by the person"),
        Some(EndedBy::Person),
        Some(EndedWay::Closed),
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    let Found::Ended(ended) = by_number_in(&root, 641, &[]) else {
        panic!("not found ended");
    };
    assert_eq!(
        dispatchrecord::Finished::of(&ended),
        Some(dispatchrecord::Finished::ClosedByThePerson)
    );
}

#[test]
fn a_chat_started_again_is_found_under_its_new_number() {
    // A restart keeps the ULID and deals a new number: 641 is the record's, 668 the chat's now.
    let (_dir, root) = project();
    task_641(&root);
    let found = by_number_in(&root, 641, &[open_chat(668, WORKER_ID)]);
    assert_eq!(found, Found::Restarted { now: 668 });
}

#[test]
fn a_finished_task_reopened_is_found_under_its_new_number() {
    let (_dir, root) = project();
    let id = task_641(&root);
    dispatchrecord::close(
        &root,
        &id,
        report(Outcome::Done, "ok"),
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    let found = by_number_in(&root, 641, &[open_chat(668, WORKER_ID)]);
    assert_eq!(found, Found::Restarted { now: 668 });
}

#[test]
fn an_unknown_number_is_no_chat() {
    let (_dir, root) = project();
    task_641(&root);
    assert_eq!(
        by_number_in(&root, 9999, &[open_chat(3, ASKER_ID)]),
        Found::NoChat
    );
}

#[test]
fn a_running_task_whose_chat_is_not_open_is_no_chat() {
    // Its record is open and no chat has it: nothing to show a card of yet.
    let (_dir, root) = project();
    task_641(&root);
    assert_eq!(by_number_in(&root, 641, &[]), Found::NoChat);
}

#[test]
fn the_newest_record_of_a_number_answers() {
    let (_dir, root) = project();
    let first = task_641(&root);
    dispatchrecord::close(
        &root,
        &first,
        report(Outcome::Blocked, "stuck"),
        at("2026-11-11T10:30:00Z"),
    )
    .unwrap();
    let second = task_641(&root);
    dispatchrecord::close(
        &root,
        &second,
        report(Outcome::Done, "done"),
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    let Found::Ended(ended) = by_number_in(&root, 641, &[]) else {
        panic!("not found ended");
    };
    assert_eq!(ended.id, second);
}

#[test]
fn a_task_that_never_started_is_no_chat() {
    // Its number was dealt for a chat that never was, and names nobody (#1497).
    let (_dir, root) = project();
    let id = task_641_as(&root, None);
    dispatchrecord::did_not_start(&root, &id, "no profile", 1, at("2026-11-11T10:01:00Z")).unwrap();
    assert_eq!(by_number_in(&root, 641, &[]), Found::NoChat);
}

// #6304: a chat no dispatch started, kept by the app as it closed (`chatrecords`).

const ORDINARY_ID: &str = "01K6CHATS0000000000000000C";

/// Chat 702, started by the person in the window and closed by them.
fn closed_702() -> crate::chatrecords::ChatRecord {
    crate::chatrecords::ChatRecord {
        number: 702,
        id: ORDINARY_ID.to_owned(),
        title: "Tidy the changelog".to_owned(),
        started: "2026-11-12T09:00:00Z".to_owned(),
        ended: "2026-11-12T09:40:00Z".to_owned(),
        started_by: crate::chatrecords::StartedBy::Person,
        place: "smart-ide".to_owned(),
    }
}

#[test]
fn an_ordinary_chat_that_closed_is_found_closed_with_its_record() {
    let (_dir, root) = project();
    crate::chatrecords::keep(&root, &closed_702()).unwrap();
    assert_eq!(
        by_number_in(&root, 702, &[open_chat(3, ASKER_ID)]),
        Found::Closed(Box::new(closed_702()))
    );
}

#[test]
fn an_ordinary_chat_open_again_is_found_under_its_new_number() {
    let (_dir, root) = project();
    crate::chatrecords::keep(&root, &closed_702()).unwrap();
    assert_eq!(
        by_number_in(&root, 702, &[open_chat(731, ORDINARY_ID)]),
        Found::Restarted { now: 731 }
    );
}

#[test]
fn a_dispatch_record_answers_before_the_chat_record_of_its_number() {
    // A task's chat is kept both ways as it closes: its dispatch record is the fuller card.
    let (_dir, root) = project();
    let id = task_641(&root);
    dispatchrecord::close(
        &root,
        &id,
        report(Outcome::Done, "ok"),
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    crate::chatrecords::keep(
        &root,
        &crate::chatrecords::ChatRecord {
            number: 641,
            id: WORKER_ID.to_owned(),
            ..closed_702()
        },
    )
    .unwrap();
    assert!(matches!(by_number_in(&root, 641, &[]), Found::Ended(_)));
}

#[test]
fn the_records_read_once_answer_as_one_read_does() {
    let (_dir, root) = project();
    crate::chatrecords::keep(&root, &closed_702()).unwrap();
    let known = Known::read(&root);
    assert_eq!(by_number(702, &[], &known), by_number_in(&root, 702, &[]));
    assert_eq!(by_number(9999, &[], &known), Found::NoChat);
}

// A dispatch record answers only where it makes a card: a handoff's, or a task's that owed no
// report, has none (`Finished::of` is None), so the chat's own record answers for it.

/// [`task_641`] as `mode`, owing a report or not, ended with no report.
fn ended_unreported(root: &std::path::Path, mode: Mode, report_owed: bool) {
    let opening = Opening {
        mode,
        asker: Asker {
            chat: ChatRef {
                chat: 3,
                id: Some(ASKER_ID.to_owned()),
                name: "steward 3".to_owned(),
                persona: Some("steward".to_owned()),
            },
            workspace: Some("smart-ide".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("steward".to_owned()),
        worker: Worker {
            chat: ChatRef {
                chat: 641,
                id: Some(WORKER_ID.to_owned()),
                name: "Signed commits #5607".to_owned(),
                persona: Some("steward".to_owned()),
            },
            harness: Some("claude".to_owned()),
            profile: None,
            session_record: None,
        },
        task: Some("Signed commits #5607".to_owned()),
        place: Place {
            workspace: Some("smart-ide".to_owned()),
            folder: Some("workspaces/smart-ide".to_owned()),
            worktree: None,
        },
        brief: "Sign the commits.".to_owned(),
        report_owed,
    };
    let id = dispatchrecord::open(root, opening, at("2026-11-11T10:00:00Z"))
        .unwrap()
        .id;
    dispatchrecord::close(
        root,
        &id,
        Ending {
            report: None,
            usage: None,
        },
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
}

/// Chat 641's own record, as the app kept it as it closed: handed off to by `steward 3`.
fn closed_641() -> crate::chatrecords::ChatRecord {
    crate::chatrecords::ChatRecord {
        number: 641,
        id: WORKER_ID.to_owned(),
        title: "Signed commits #5607".to_owned(),
        started: "2026-11-11T10:00:00Z".to_owned(),
        ended: "2026-11-11T11:08:00Z".to_owned(),
        started_by: crate::chatrecords::StartedBy::Chat {
            name: "steward 3".to_owned(),
        },
        place: "smart-ide".to_owned(),
    }
}

#[test]
fn a_closed_handoff_is_found_closed_with_its_chat_record() {
    let (_dir, root) = project();
    ended_unreported(&root, Mode::Handoff, false);
    crate::chatrecords::keep(&root, &closed_641()).unwrap();
    assert_eq!(
        by_number_in(&root, 641, &[open_chat(3, ASKER_ID)]),
        Found::Closed(Box::new(closed_641()))
    );
}

#[test]
fn a_task_that_ended_owing_no_report_is_found_closed_with_its_chat_record() {
    let (_dir, root) = project();
    ended_unreported(&root, Mode::Task, false);
    crate::chatrecords::keep(&root, &closed_641()).unwrap();
    assert_eq!(
        by_number_in(&root, 641, &[]),
        Found::Closed(Box::new(closed_641()))
    );
}

#[test]
fn a_running_task_whose_chat_closed_is_found_closed_with_its_chat_record() {
    let (_dir, root) = project();
    task_641(&root);
    crate::chatrecords::keep(&root, &closed_641()).unwrap();
    assert_eq!(
        by_number_in(&root, 641, &[]),
        Found::Closed(Box::new(closed_641()))
    );
}
