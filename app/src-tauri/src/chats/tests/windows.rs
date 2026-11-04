// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's windows, as the core holds them (#4768, ruled on #4701): each window reports
//! its own tabs, a chat is shown once across all of them, a minimised window shows nothing, and
//! a move between windows is one step of the core's.

use std::sync::Arc;

use purlis_core::engine::Size;
use purlis_core::reopen::Chat;
use purlis_core::reopen::layout::{Layout, Tab};

use super::Chats;
use crate::host::SessionHost;
use crate::host::pretend::Pretend;

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};

fn a_shell(name: &str) -> Chat {
    Chat {
        program: "/bin/sh".to_owned(),
        name: name.to_owned(),
        ..Default::default()
    }
}

/// A project on a pretend host with `count` chats open, and their numbers.
fn with_chats(count: usize) -> (Pretend, Arc<Chats>, Vec<u32>) {
    let host = Pretend::default();
    let chats = Arc::new(Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        super::tests::no_project(),
    ));
    let numbers = (0..count)
        .map(|at| {
            chats
                .start(&a_shell(&format!("shell {at}")), SIZE)
                .expect("a shell starts on a pretend host")
        })
        .collect();
    (host, chats, numbers)
}

/// A report of one window's tabs: each a tab of one pane on the strip outside every workspace,
/// the first in front.
fn report(chats: &[u32]) -> Layout {
    Layout {
        strip: None,
        tabs: chats
            .iter()
            .enumerate()
            .map(|(at, chat)| {
                let mut tab = Tab::of_chat(None, *chat);
                tab.front = at == 0;
                tab
            })
            .collect(),
    }
}

#[test]
fn each_window_reports_its_own_tabs_and_what_is_on_screen_spans_them_all() {
    let (_host, chats, numbers) = with_chats(3);
    let [one, two, three] = numbers[..] else {
        unreachable!()
    };

    chats.lay_out_window(0, report(&[one, two])).unwrap();
    chats.lay_out_window(1, report(&[three])).unwrap();
    // The project window says its tabs again: tab window 1's stand.
    chats.lay_out_window(0, report(&[two, one])).unwrap();

    assert_eq!(chats.looked_at(), vec![two, three]);
    let held = chats.layout().expect("held");
    assert_eq!(held.tab_windows(), [1]);
    assert_eq!(chats.record().layout, Some(held), "and the record keeps it");
    chats.end_all();
}

#[test]
fn a_window_that_names_a_chat_another_window_shows_is_refused_and_changes_nothing() {
    let (_host, chats, numbers) = with_chats(2);
    let [one, two] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one])).unwrap();
    chats.lay_out_window(1, report(&[two])).unwrap();
    let before = chats.layout();

    let refused = chats.lay_out_window(1, report(&[one])).unwrap_err();

    assert!(
        refused.contains(&format!("chat {one} in two panes")),
        "{refused}"
    );
    assert_eq!(chats.layout(), before);
    chats.end_all();
}

#[test]
fn a_minimised_window_counts_as_nothing_until_it_is_back() {
    let (_host, chats, numbers) = with_chats(2);
    let [one, two] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one])).unwrap();
    chats.lay_out_window(1, report(&[two])).unwrap();

    assert!(chats.window_away(1, true));
    assert!(!chats.window_away(1, true), "already away");
    assert_eq!(chats.looked_at(), vec![one]);
    assert!(!chats.looks_at(two));
    assert!(chats.window_away(0, true));
    assert_eq!(chats.looked_at(), Vec::<u32>::new());

    chats.window_away(1, false);
    chats.window_away(0, false);
    assert_eq!(chats.looked_at(), vec![one, two]);
    chats.end_all();
}

#[test]
fn the_window_showing_a_chat_is_the_tab_window_it_was_torn_off_into() {
    let (_host, chats, numbers) = with_chats(2);
    let [one, two] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one, two])).unwrap();
    assert_eq!(
        chats.window_showing(two),
        None,
        "a tab behind shows nothing"
    );

    let to = chats
        .move_tab(0, 1, None)
        .expect("a tab of the project window");

    assert_eq!(to, 1);
    assert_eq!(chats.window_showing(two), Some(1));
    assert_eq!(chats.window_showing(one), Some(0));
    assert_eq!(chats.looked_at(), vec![one, two]);
    chats.end_all();
}

#[test]
fn closing_a_tab_window_sends_its_tabs_back_and_every_chat_keeps_running() {
    let (host, chats, numbers) = with_chats(2);
    let [one, two] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one])).unwrap();
    chats.lay_out_window(1, report(&[two])).unwrap();
    chats.window_away(1, true);

    assert!(chats.send_back(1));

    let held = chats.layout().expect("held");
    assert!(held.tab_windows().is_empty());
    assert_eq!(
        held.chats(),
        [one, two],
        "behind the project window's front tab"
    );
    assert_eq!(chats.looked_at(), vec![one]);
    assert_eq!(host.running().len(), 2, "closing never ends a chat");
    // A window numbered 1 again later is not away for having been away once.
    assert_eq!(chats.move_tab(0, 1, None), Ok(1));
    assert_eq!(chats.looked_at(), vec![one, two]);
    chats.end_all();
}

#[test]
fn a_move_with_no_layout_held_or_into_a_window_that_is_not_there_is_refused() {
    let (_host, chats, numbers) = with_chats(1);

    let refused = chats.move_tab(0, 0, None).unwrap_err();
    assert!(refused.starts_with("purlis did not move that"), "{refused}");

    chats.lay_out_window(0, report(&numbers)).unwrap();
    let refused = chats.move_tab(0, 0, Some(4)).unwrap_err();
    assert!(refused.contains("no tab window 4"), "{refused}");
    assert_eq!(chats.layout().map(|held| held.tab_windows()), Some(vec![]));
    chats.end_all();
}

#[test]
fn a_press_on_a_chat_in_no_pane_goes_to_the_window_whose_pane_was_used_last() {
    let (_host, chats, numbers) = with_chats(3);
    let [one, two, three] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one])).unwrap();
    chats.lay_out_window(1, report(&[two])).unwrap();
    assert_eq!(
        chats.window_pressed(three),
        None,
        "no pane used yet: the pressing window's"
    );

    assert!(chats.pane_used(1));
    assert!(chats.pane_used(0));
    assert!(chats.pane_used(1));
    assert!(!chats.pane_used(1), "the latest already");

    assert_eq!(chats.window_pressed(three), Some(1));
    assert_eq!(
        chats.window_pressed(one),
        Some(0),
        "a chat in a pane goes to its window"
    );
    chats.window_away(1, true);
    assert_eq!(
        chats.window_pressed(three),
        Some(0),
        "a minimised one is passed over"
    );
    chats.window_away(1, false);
    assert!(chats.send_back(1));
    assert_eq!(
        chats.window_pressed(three),
        Some(0),
        "a closed one is never pressed into"
    );
    // A window numbered 1 again later has not been used for having been used once.
    assert_eq!(chats.move_tab(0, 1, None), Ok(1));
    assert_eq!(chats.window_pressed(three), Some(0));
    chats.end_all();
}

#[test]
fn a_tab_window_emptied_by_a_move_is_forgotten_so_a_new_one_of_its_number_is_unused() {
    let (_host, chats, numbers) = with_chats(3);
    let [one, two, three] = numbers[..] else {
        unreachable!()
    };
    chats.lay_out_window(0, report(&[one])).unwrap();
    chats.lay_out_window(1, report(&[two])).unwrap();
    chats.pane_used(0);
    chats.pane_used(1);

    // Its last tab moved back: tab window 1 goes, never closed by the person.
    assert_eq!(chats.move_tab(1, 0, Some(0)), Ok(0));
    assert_eq!(chats.move_tab(0, 1, None), Ok(1), "a new tab window 1");

    assert_eq!(chats.window_pressed(three), Some(0));
    chats.end_all();
}
