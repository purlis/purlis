// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task on a harness purlis has no hooks into** (#4759, ADR 0090 as amended 2026-11-02):
//! it starts in no pane like every dispatch, and it can't say when it needs the person, so the
//! asking chat is told, beside the start, that the person must look at it.

use super::*;

/// What the window is told of chat 7, started from chat 1, as a task or a handoff, on a
/// harness that reports all or one that can't say when it waits.
fn arrived(task: bool, unreported: Option<&str>) -> Arrived {
    Arrived {
        plane: PlaneId::for_tests(Path::new("/home/dev/plane")),
        session: 7,
        name: "7".to_owned(),
        label: Some("check the queue".to_owned()),
        from: Some(crate::HandedFromNote {
            name: "steward 1".to_owned(),
            workspace: "alpha".to_owned(),
            chat: 1,
            task,
            tab: !task,
            reported: false,
            unreported: false,
            outcome: None,
            by_person: None,
            asker_waiting: None,
        }),
        workspace: Some("alpha".to_owned()),
        persona: None,
        harness: Some("codex".to_owned()),
        unreported: unreported.map(str::to_owned),
        announced: !task || unreported.is_some(),
    }
}

#[test]
fn a_task_whose_harness_cannot_report_asks_tells_its_asking_chat_the_person_must_look() {
    let note = cant_report_note(&arrived(true, Some("Codex can't say when it waits.")));
    assert_eq!(note.as_deref(), Some(CANT_REPORT_ASKS));
    assert_eq!(
        CANT_REPORT_ASKS,
        "its harness can't report asks, so tell the person they must look at it themselves"
    );
}

#[test]
fn a_task_whose_harness_reports_its_asks_is_told_nothing_more() {
    assert_eq!(cant_report_note(&arrived(true, None)), None);
}

#[test]
fn a_handoff_is_told_nothing_more_whatever_its_harness_its_asking_chat_hears_nothing_back() {
    assert_eq!(
        cant_report_note(&arrived(false, Some("Codex can't say when it waits."))),
        None
    );
}
