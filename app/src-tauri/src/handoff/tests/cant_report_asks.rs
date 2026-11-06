// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task that can't report asks** (#4759, ADR 0090 as amended 2026-11-02):
//! it starts in no pane like every dispatch, and it can't say when it needs the person, so the
//! asking chat is told, beside the start, that the person must look at it, and why: its
//! harness's own reason, or that purlis has no adapter for its program (#5271).

use super::*;

/// What the window is told of chat 7, started from chat 1, as a task or a handoff, on a
/// harness that says when it waits or one that can't.
fn arrived(task: bool, cant_report_asks: bool) -> Arrived {
    on(task, cant_report_asks, true)
}

/// [`arrived`], on a harness purlis has an adapter for, or not.
fn on(task: bool, cant_report_asks: bool, adapter: bool) -> Arrived {
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
        no_adapter: !adapter,
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

#[test]
fn a_task_on_a_program_purlis_has_no_adapter_for_is_told_so_rather_than_blaming_a_harness() {
    let note = cant_report_note(&on(true, true, false));
    assert_eq!(note.as_deref(), Some(NO_ADAPTER_CANT_REPORT_ASKS));
    assert_eq!(
        NO_ADAPTER_CANT_REPORT_ASKS,
        "purlis has no adapter for its program, so it can't report asks: tell the person they \
         must look at it themselves"
    );
}

/// **A task started on a program purlis has no adapter for** (#5271), the whole start: a chat
/// on `work` dispatches a task on `sh`, a profile of `shelly`, a harness this machine declares
/// on its own. The asking chat is told why the person must look at it, the window announces
/// it, and its row keeps the faint hand and the "new" mark.
#[cfg(unix)]
#[test]
fn a_task_started_on_a_program_with_no_adapter_tells_why_is_announced_and_keeps_the_faint_hand() {
    let plane = Plane::new();
    let runs = plane.root.join("runs");
    let program = stand_in::program(
        plane._dir.path(),
        "shelly-stand-in",
        &format!(
            "#!/bin/sh\nat=$(mktemp {runs:?}/.writing.XXXXXX) || exit 1\n\
             mv \"$at\" {runs:?}/run.shelly.$$\nsleep 10\n"
        ),
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
    let told = Mutex::new(None);
    let Ask::Dispatch(mut ask) = a_task_into_alpha(asking, &ticket, Some("check the queue")) else {
        unreachable!("a dispatch")
    };
    ask.profile = Some("sh".to_owned());

    let said = answer(&held, &id, &tickets, 1, Ask::Dispatch(ask), &|arrived| {
        *told.lock().unwrap() = Some(arrived)
    });

    let Answer::Dispatched { chat, note, .. } = said else {
        panic!("dispatched, not {said:?}")
    };
    assert!(
        note.as_deref()
            .is_some_and(|note| note.contains(NO_ADAPTER_CANT_REPORT_ASKS)),
        "the asking chat is told why: {note:?}"
    );
    assert!(
        !note
            .as_deref()
            .unwrap_or_default()
            .contains(CANT_REPORT_ASKS),
        "and not that a harness is to blame: {note:?}"
    );
    let told = told.into_inner().unwrap().expect("the window is told");
    assert!(
        told.cant_report_asks && told.no_adapter && told.announced,
        "{told:?}"
    );
    let open = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == chat)
        .expect("it runs");
    let row = crate::OpenChat::from(open);
    assert_eq!(row.cant_report_asks, Some(true), "the faint hand");
    assert_eq!(row.unseen, Some(true), "the \"new\" mark");
}
