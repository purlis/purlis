// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task that can't report asks** (#4759, ADR 0090 as amended 2026-11-02):
//! it starts in no pane like every dispatch, and it can't say when it needs the person, so the
//! asking chat is told, beside the start, that the person must look at it. A program purlis has
//! no adapter for is never started as a task at all (#5271).

use super::*;

/// What the window is told of chat 7, started from chat 1, as a task or a handoff, on a
/// harness that says when it waits or one that can't.
fn arrived(task: bool, cant_report_asks: bool) -> Arrived {
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
        cant_report_asks,
        announced: !task || cant_report_asks,
    }
}

#[test]
fn a_task_whose_harness_cannot_report_asks_tells_its_asking_chat_the_person_must_look() {
    let note = cant_report_note(&arrived(true, true));
    assert_eq!(note.as_deref(), Some(CANT_REPORT_ASKS));
    assert_eq!(
        CANT_REPORT_ASKS,
        "its harness can't report asks, so tell the person they must look at it themselves"
    );
}

#[test]
fn a_task_whose_harness_reports_its_asks_is_told_nothing_more() {
    assert_eq!(cant_report_note(&arrived(true, false)), None);
}

#[test]
fn a_handoff_is_told_nothing_more_whatever_its_harness_its_asking_chat_hears_nothing_back() {
    assert_eq!(cant_report_note(&arrived(false, true)), None);
}

/// **A task on a program purlis has no adapter for is refused before it starts** (#5271): a
/// chat on `work` dispatches a task on `sh`, a profile of `shelly`, a harness this machine
/// declares on its own. Nothing knows how to hand that program the brief, so the asking chat is
/// told so and nothing starts: no chat is announced, none is listed, and no asking chat is ever
/// told that such a task's harness can't report asks.
#[cfg(unix)]
#[test]
fn a_task_on_a_program_with_no_adapter_is_refused_before_it_starts_and_nothing_is_announced() {
    let plane = Plane::new();
    let program = stand_in::program(
        plane._dir.path(),
        "shelly-stand-in",
        "#!/bin/sh\nsleep 10\n",
    );
    let local = plane.root.join(purlis_core::profiles::LOCAL_FILE);
    let mut text = std::fs::read_to_string(&local).expect("the local file");
    // A declaration names a bare program; this machine's profile points it at the stand-in.
    text.push_str(&format!(
        "[harness.sh]\nkind = \"shelly\"\ncommand = [{:?}]\n\n\
         [harnesses.shelly]\nprogram = \"shelly\"\n",
        program.display().to_string()
    ));
    std::fs::write(&local, text).expect("the declaration");
    // Nothing compiles a sandbox for a harness with no adapter: this is about the start.
    crate::sandboxing::no_sandbox_here_for_tests(&plane.root);
    // The operator's yes, as the window's dialog records it: to the profile, and to the
    // declaration it runs on.
    let set = purlis_core::profiles::current(&plane.root);
    let sh = set.get("sh").expect("sh is a profile");
    let shown = purlis_core::profiletrust::shown(&plane.root, sh);
    purlis_core::profiletrust::approve(&plane.root, sh, &shown).expect("approved");
    // An opt-out of the sandbox is always recorded, so the event log is open.
    let planes = planes_recording_events(&plane.root.join("events"));
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let asking = a_chat_on_work(&held, &plane.root);
    let tickets = Tickets::default();
    let ticket = ticket(&held, &id, &tickets, asking);
    let before = held.chats().open_now().len();
    let told = Mutex::new(None);
    let Ask::Dispatch(mut ask) = a_task_into_alpha(asking, &ticket, Some("check the queue")) else {
        unreachable!("a dispatch")
    };
    ask.profile = Some("sh".to_owned());

    let said = answer(&held, &id, &tickets, 1, Ask::Dispatch(ask), &|arrived| {
        *told.lock().unwrap() = Some(arrived)
    });

    let Answer::No { why } = said else {
        panic!("refused, not {said:?}")
    };
    assert_eq!(
        why,
        "profile 'sh' runs a harness purlis has not measured the first message of, so it cannot \
         be started on the brief."
    );
    assert!(told.into_inner().unwrap().is_none(), "nothing is announced");
    assert_eq!(held.chats().open_now().len(), before, "nothing is listed");
}
