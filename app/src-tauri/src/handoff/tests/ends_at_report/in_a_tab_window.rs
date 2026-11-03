// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A task on screen in a tab window (#4768, ruled on #4701): the person reads it there as they
//! would in the project window, so its due end waits for them; a minimised tab window, and one
//! closed, show nothing, and the end goes ahead.
//!
//! **On the real clock**, as `working_again.rs` is, for its reason: none of these is green
//! because the clock was off.

use super::*;
use purlis_core::reopen::layout::{Layout, Tab};

/// Longer than a reported task's moment to settle.
fn past_the_settle() {
    std::thread::sleep(
        purlis_core::dispatched::A_TURN_SETTLES_WITHIN + std::time::Duration::from_millis(900),
    );
}

/// Waits until `task` is no longer an open chat, or fails after a bound.
fn ends_within_moments(held: &Held, task: u32) {
    let began = Instant::now();
    let bound = purlis_core::dispatched::A_TURN_SETTLES_WITHIN + std::time::Duration::from_secs(10);
    while open_chats(held).contains(&task) && began.elapsed() < bound {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!open_chats(held).contains(&task), "chat {task} never ended");
}

/// The project window shows `steward` in its front tab, and tab window 1 shows `task` in its.
fn torn_off(steward: u32, task: u32) -> Layout {
    let mut project = Tab::of_chat(None, steward);
    project.front = true;
    let mut window = Tab::of_chat(None, task);
    window.window = 1;
    window.front = true;
    Layout {
        strip: None,
        tabs: vec![project, window],
    }
}

/// A steward chat and its one task, with the end's clock on, the task torn off into a tab
/// window, reported and past its settle.
fn reported_in_a_tab_window() -> (Plane, Pretend, Planes, Arc<Held>, u32, u32) {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_its_task();
    held.tasks().on_the_clock();
    held.chats()
        .lay_out(torn_off(steward, task))
        .expect("each chat once, across the windows");
    works(&held, task);
    reports(&held, &id, task);
    its_turn_ends(&held, task);
    past_the_settle();
    crate::dispatched::end_look(&held, task, Looked::WaitedOut);
    (plane, host, planes, held, steward, task)
}

#[test]
fn a_task_on_screen_in_a_tab_window_is_never_ended_under_the_person() {
    let (_plane, _host, _planes, held, steward, task) = reported_in_a_tab_window();

    assert_eq!(held.chats().looked_at(), vec![steward, task]);
    assert!(
        open_chats(&held).contains(&task),
        "ended under the person reading it in a tab window"
    );
    assert_eq!(held.tasks().ledger().held_back(), [task]);
}

#[test]
fn a_task_in_a_tab_window_the_person_minimises_is_ended() {
    let (_plane, _host, _planes, held, steward, task) = reported_in_a_tab_window();

    assert!(held.chats().window_away(1, true), "the window is away now");
    crate::dispatched::front_moved(&held);

    assert_eq!(held.chats().looked_at(), vec![steward]);
    ends_within_moments(&held, task);
}

#[test]
fn a_task_whose_tab_window_closes_goes_behind_in_the_project_window_and_is_ended() {
    let (_plane, _host, _planes, held, steward, task) = reported_in_a_tab_window();

    assert!(held.chats().send_back(1));
    crate::dispatched::front_moved(&held);

    assert_eq!(held.chats().looked_at(), vec![steward]);
    ends_within_moments(&held, task);
}

#[test]
fn a_task_whose_tab_moves_to_a_new_window_stays_on_screen_and_held() {
    let (_plane, _host, _planes, _id, held, steward, task) = a_steward_and_its_task();
    held.tasks().on_the_clock();
    let mut behind = Tab::of_chat(None, task);
    behind.front = false;
    let mut front = Tab::of_chat(None, steward);
    front.front = true;
    held.chats()
        .lay_out(Layout {
            strip: None,
            tabs: vec![front, behind],
        })
        .unwrap();

    // Move to new window: the task's tab is the front tab of a window of its own.
    assert_eq!(held.chats().move_tab(0, 1, None), Ok(1));

    assert_eq!(held.chats().looked_at(), vec![steward, task]);
    assert_eq!(held.chats().window_showing(task), Some(1));
}
