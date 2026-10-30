// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::dispatchrecord::{Asker, Changed, ChatRef, Mode, Outcome, Place, Report, Worker};

use super::*;

fn a_task(outcome: Outcome, text: &str) -> Record {
    Record {
        v: dispatchrecord::VERSION,
        id: "01K6DISPATCH00000000000000".to_owned(),
        mode: Mode::Task,
        asker: Asker {
            chat: ChatRef {
                chat: 3,
                id: Some("asker".to_owned()),
                name: "steward 3".to_owned(),
                persona: Some("steward".to_owned()),
            },
            workspace: Some("alpha".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("devops".to_owned()),
        worker: Worker {
            chat: ChatRef {
                chat: 7,
                id: Some("worker".to_owned()),
                name: "devops 7".to_owned(),
                persona: Some("devops".to_owned()),
            },
            harness: Some("claude".to_owned()),
            profile: Some("work".to_owned()),
            session_record: None,
        },
        task: Some("check prod".to_owned()),
        place: Place {
            workspace: Some("beta".to_owned()),
            folder: Some("workspaces/beta".to_owned()),
            worktree: None,
        },
        brief: "Is the rollout healthy?".to_owned(),
        report_owed: true,
        started: "2026-10-07T12:00:00+00:00".to_owned(),
        ended: Some("2026-10-07T12:04:30+00:00".to_owned()),
        report: Some(Report {
            outcome,
            text: text.to_owned(),
            changed: Changed {
                said: Some("values.yaml: replicas 2 to 3".to_owned()),
                ..Changed::default()
            },
        }),
        needed_you: 0,
        messages: 0,
        talk: Vec::new(),
        last_note: None,
        usage: None,
        conversation: Some("9f2c-the-conversation".to_owned()),
        cleared: false,
        ended_by: None,
        ended_way: None,
        kept_open: false,
        did_not_start: false,
        attempts: 0,
        worked: 0,
        limit: None,
        undelivered: None,
        earlier: Vec::new(),
        asker_last_record: None,
    }
}

#[test]
fn a_secret_in_a_finished_task_s_report_is_masked_on_its_row() {
    // #1872: a token built here from its parts, so no line of this file holds one.
    let token = format!("{}{}", "ghp_", "0123456789abcdefghijABCDEFGHIJ012345");
    let record = a_task(Outcome::Done, &format!("Signed in with {token}."));
    let drawn = row(&record, 12, Finished::of(&record).unwrap());

    assert_eq!(drawn.report, "Signed in with ***.");
}

#[test]
fn a_finished_task_s_row_says_how_it_ended_and_carries_its_report_as_written() {
    let record = a_task(Outcome::Done, "<b>Healthy</b>, 3 of 3 pods.");
    let drawn = row(&record, 12, Finished::of(&record).unwrap());

    assert_eq!(
        drawn,
        FinishedTask {
            id: "01K6DISPATCH00000000000000".to_owned(),
            // The asking chat by the number it has now, whatever it had then.
            asker: 12,
            name: "check prod".to_owned(),
            // Read from the store alone, as after a restart: no pane was left on it.
            chat: None,
            persona: Some("devops".to_owned()),
            how: How::Done,
            outcome: "done".to_owned(),
            folds: true,
            // As written: the window draws it as text.
            report: "<b>Healthy</b>, 3 of 3 pods.".to_owned(),
            changed: Some("values.yaml: replicas 2 to 3".to_owned()),
            ended: Some("2026-10-07T12:04:30+00:00".to_owned()),
            place: "beta".to_owned(),
            branch: None,
            reopens: true,
            not_reopened: None,
            did_not_start: false,
            attempts: 0,
            waits: None,
        }
    );
}

#[test]
fn a_failure_is_a_row_of_its_own_and_a_task_with_no_conversation_is_not_reopened() {
    let failed = a_task(Outcome::Failed, "The cluster refused the login.");
    let drawn = row(&failed, 3, Finished::of(&failed).unwrap());
    assert_eq!((drawn.outcome.as_str(), drawn.folds), ("failed", false));

    let unreported = Record {
        conversation: None,
        ended_by: Some(dispatchrecord::EndedBy::Unreported),
        ended_way: None,
        ..a_task(Outcome::Failed, dispatchrecord::ENDED_WITHOUT_A_REPORT)
    };
    let drawn = row(&unreported, 3, Finished::of(&unreported).unwrap());
    assert_eq!(
        (drawn.outcome.as_str(), drawn.folds, drawn.reopens),
        ("ended without a report", false, false)
    );

    let closed = a_task(Outcome::Stopped, purlis_core::handback::STOPPED);
    let drawn = row(&closed, 3, Finished::of(&closed).unwrap());
    // A record from before the way was kept: read as closed, and said to the person as
    // theirs (#1488).
    assert_eq!(
        (drawn.outcome.as_str(), drawn.how, drawn.folds),
        ("closed by you", How::ClosedByPerson, false)
    );
}
