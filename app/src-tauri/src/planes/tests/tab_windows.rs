// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's window set (#4768): the window holding it and its tab windows, each tab window
//! told apart by label and filed under its number.

use std::path::Path;

use super::{Holding, PlaneId, Showing};

fn id(root: &str) -> PlaneId {
    PlaneId::for_tests(Path::new(root))
}

fn holding(planes: &[&PlaneId]) -> Holding {
    Holding {
        planes: planes.iter().map(|plane| (*plane).clone()).collect(),
        active: Some(0),
    }
}

#[test]
fn a_project_s_events_go_to_the_window_holding_it_and_to_each_of_its_tab_windows() {
    let (alpha, beta) = (id("/work/alpha"), id("/work/beta"));
    let showing = Showing::default();
    showing.in_window("main", holding(&[&alpha, &beta]));
    showing.tab_window_made("window-3", &alpha, 2);
    showing.tab_window_made("window-2", &alpha, 1);
    showing.tab_window_made("window-4", &beta, 1);

    assert_eq!(showing.window_set(&alpha), ["main", "window-2", "window-3"]);
    assert_eq!(showing.window_set(&beta), ["main", "window-4"]);
}

#[test]
fn a_tab_window_stays_its_project_s_when_the_project_moves_windows() {
    let alpha = id("/work/alpha");
    let showing = Showing::default();
    showing.in_window("main", holding(&[&alpha]));
    showing.tab_window_made("window-2", &alpha, 1);

    showing
        .move_into(
            "main",
            std::slice::from_ref(&alpha),
            Some(&alpha),
            "window-5",
        )
        .unwrap();

    assert_eq!(showing.window_set(&alpha), ["window-5", "window-2"]);
    assert_eq!(showing.window_of("window-5", &alpha), Some(0));
    assert_eq!(showing.window_of("window-2", &alpha), Some(1));
    assert_eq!(showing.window_of("main", &alpha), None, "it left main");
}

#[test]
fn a_window_reports_only_its_own_project_s_tabs() {
    let (alpha, beta) = (id("/work/alpha"), id("/work/beta"));
    let showing = Showing::default();
    showing.in_window("main", holding(&[&alpha]));
    showing.tab_window_made("window-2", &alpha, 1);

    assert_eq!(showing.window_of("window-2", &beta), None);
    assert_eq!(showing.window_of("main", &beta), None);
    assert_eq!(showing.window_of("window-9", &alpha), None);
}

#[test]
fn a_notification_about_a_chat_in_a_tab_window_is_asked_of_that_window() {
    // The silent failure: the project window has the keyboard, and the chat is on screen only
    // in a tab window. Asking the project window would hold back a notification about a chat
    // the person cannot see where they are looking.
    let alpha = id("/work/alpha");
    let showing = Showing::default();
    showing.in_window("main", holding(&[&alpha]));
    showing.tab_window_made("window-2", &alpha, 1);

    assert_eq!(
        showing.asked_of(&alpha, Some(1)).as_deref(),
        Some("window-2")
    );
    assert_eq!(showing.asked_of(&alpha, Some(0)).as_deref(), Some("main"));
    assert_eq!(showing.asked_of(&alpha, None).as_deref(), Some("main"));
    // A stale layout naming a tab window that is gone asks the project window.
    assert_eq!(showing.asked_of(&alpha, Some(4)).as_deref(), Some("main"));
}

#[test]
fn a_tab_window_gone_is_forgotten_once_and_its_label_is_never_handed_out_while_open() {
    let alpha = id("/work/alpha");
    let showing = Showing::default();
    let label = showing.fresh_label();
    showing.tab_window_made(&label, &alpha, 1);

    assert_ne!(showing.fresh_label(), label);
    assert_eq!(
        showing.tab_window_gone(&label).map(|tab| tab.number),
        Some(1)
    );
    assert_eq!(showing.tab_window_gone(&label), None);
    assert_eq!(showing.window_set(&alpha), Vec::<String>::new());
}
