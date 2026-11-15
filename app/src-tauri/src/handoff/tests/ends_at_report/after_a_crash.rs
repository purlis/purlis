// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A window that crashed keeps the screen of a task that had finished** (#6504).
//!
//! With no quit, nothing kept the screen as the app went (#6305 keeps it at a quit), and the
//! session host still runs the task's chat. The next launch leaves it out of its put back, so
//! its sweep (`Chats::close_what_was_not_reattached`) would close it unread: it reads and keeps
//! the screen first, under the same caps and the same switch as a close.

use purlis_core::reopen::Choice;

use super::*;

const A_SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 80,
    rows: 24,
};

/// A steward chat and its task, which reported and ended its turn, and then the window crashed:
/// the record it last wrote, the task's id and the steward's. Nothing was let go of, so no
/// screen was kept on the way out.
struct Crashed {
    plane: Plane,
    at_crash: purlis_core::reopen::Record,
    steward: u32,
    task: u32,
    task_id: String,
    steward_id: String,
}

fn a_window_that_crashed_after_its_task_reported() -> Crashed {
    let (plane, host, crashed, id, held, steward, task) = a_steward_and_its_task();
    works(&held, task);
    reports(&held, &id, task);
    its_turn_ends(&held, task);
    let id_of = |chat: u32| {
        held.chats()
            .chat_at(chat)
            .and_then(|at| at.id)
            .expect("a chat's id")
    };
    let (task_id, steward_id) = (id_of(task), id_of(steward));
    let at_crash = held.chats().record();
    // The app is gone without letting go of anything: as in life, one project is never open
    // twice in one process, so it goes before the next launch opens it.
    bounded("the app that crashed, gone", move || {
        drop(held);
        drop(crashed);
        drop(host);
    });
    Crashed {
        plane,
        at_crash,
        steward,
        task,
        task_id,
        steward_id,
    }
}

/// The session host the next launch finds: it still runs both chats, each showing a screen.
fn a_host_that_kept_both(crashed: &Crashed) -> Pretend {
    let host = Pretend::default();
    host.leaves_running(crashed.steward, None);
    host.leaves_running(crashed.task, None);
    host.shows(crashed.steward, "the steward's own screen\r\n");
    host.shows(crashed.task, "check prod: all green\r\n");
    host
}

/// The next launch opens the project on `host` and puts the record back, as a launch does.
fn launches_on(host: &Pretend, crashed: &Crashed) -> (Planes, Arc<Held>) {
    launches_choosing(host, crashed, Choice::ReopenAll)
}

/// [`launches_on`], the person choosing `choice` for the record.
fn launches_choosing(host: &Pretend, crashed: &Crashed, choice: Choice) -> (Planes, Arc<Held>) {
    let (planes, held) = opens_on(host, crashed);
    held.reopen(A_SIZE, Ok(crashed.at_crash.clone()), choice);
    (planes, held)
}

/// The next launch opens the project on `host`, and has put nothing back yet.
fn opens_on(host: &Pretend, crashed: &Crashed) -> (Planes, Arc<Held>) {
    let planes = planes_on(host);
    let again = planes.open(&crashed.plane.root);
    let held = planes.held(&again).expect("held");
    (planes, held)
}

#[test]
fn a_task_that_had_reported_when_the_window_crashed_keeps_its_screen_before_its_session_closes() {
    let crashed = a_window_that_crashed_after_its_task_reported();
    let host = a_host_that_kept_both(&crashed);

    let (_planes, held) = launches_on(&host, &crashed);

    assert_eq!(
        host.reattached(),
        [crashed.steward],
        "precondition: put back"
    );
    assert_eq!(
        host.closed_left(),
        [crashed.task],
        "the finished task's session is closed"
    );
    assert_eq!(
        purlis_core::chatscreens::kept(held.root(), &crashed.task_id).as_deref(),
        Some("check prod: all green\r\n"),
        "and its screen was read and kept first, so its card's *Read it* works"
    );
    // The chat put back goes on, and keeps nothing.
    assert_eq!(
        purlis_core::chatscreens::kept(held.root(), &crashed.steward_id),
        None
    );
}

#[test]
fn a_launch_keeps_no_screen_of_a_task_left_running_where_this_machine_keeps_none() {
    let crashed = a_window_that_crashed_after_its_task_reported();
    std::fs::write(
        crashed
            .plane
            .root
            .join(purlis_core::settings::Which::Local.file_at(&crashed.plane.root)),
        "[task_screens]\nkeep = false\n",
    )
    .expect("written");
    let host = a_host_that_kept_both(&crashed);

    let (_planes, held) = launches_on(&host, &crashed);

    assert_eq!(host.closed_left(), [crashed.task], "it is still closed");
    assert_eq!(
        purlis_core::chatscreens::kept(held.root(), &crashed.task_id),
        None
    );
}

#[test]
fn a_launch_is_not_held_past_its_deadline_by_a_left_task_s_screen_that_will_not_come() {
    // As a quit is not (#6451's review): a view a host never answers holds the launch, and the
    // sweep after it, no longer than the deadline.
    let crashed = a_window_that_crashed_after_its_task_reported();
    let host = a_host_that_kept_both(&crashed);
    host.terminal_takes(std::time::Duration::from_secs(30));
    let (_planes, held) = opens_on(&host, &crashed);
    let began = std::time::Instant::now();

    held.reopen(A_SIZE, Ok(crashed.at_crash.clone()), Choice::ReopenAll);

    let took = began.elapsed();
    // The read left behind ends now, not 30 s on in a folder already deleted.
    host.terminal_answers();
    assert_eq!(
        host.closed_left(),
        [crashed.task],
        "the sweep still closed it"
    );
    // The put back alone is timed, with the slack the quit's own test gives.
    let slack = std::time::Duration::from_millis(1500);
    assert!(
        took < crate::chatcard::SCREENS_KEPT_WITHIN + slack,
        "the launch took {took:?}"
    );
}

#[test]
fn a_launch_that_starts_fresh_after_a_crash_still_keeps_a_finished_task_s_screen() {
    // #6504's review: starting fresh puts nothing back, but the task had finished all the same,
    // and the sweep closes it with the rest.
    let crashed = a_window_that_crashed_after_its_task_reported();
    let host = a_host_that_kept_both(&crashed);

    let (_planes, held) = launches_choosing(&host, &crashed, Choice::StartFresh);

    assert!(
        host.reattached().is_empty(),
        "precondition: nothing put back"
    );
    assert_eq!(
        purlis_core::chatscreens::kept(held.root(), &crashed.task_id).as_deref(),
        Some("check prod: all green\r\n")
    );
    assert_eq!(
        purlis_core::chatscreens::kept(held.root(), &crashed.steward_id),
        None,
        "the person's own chat keeps nothing"
    );
    assert_eq!(host.closed_left(), [crashed.task, crashed.steward]);
}
