// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::reopen::layout::{Direction, EVEN, Node, Pane, Side, Tab};
use crate::reopen::{Record, read, write};

/// A tab of one pane showing `chat`, on `workspace`'s strip in `window`.
fn tab(workspace: Option<&str>, window: u32, front: bool, chat: u32) -> Tab {
    let mut tab = Tab::of_chat(workspace, chat);
    tab.window = window;
    tab.front = front;
    tab
}

fn ops(tabs: Vec<Tab>) -> Layout {
    Layout {
        strip: Some("ops".to_owned()),
        tabs,
    }
}

/// Each tab as (window, front, its chats), in order.
fn shape(layout: &Layout) -> Vec<(u32, bool, Vec<u32>)> {
    layout
        .tabs
        .iter()
        .map(|tab| (tab.window, tab.front, tab.tree.chats()))
        .collect()
}

// ----- what is on screen, across the project's windows -----

#[test]
fn a_minimised_window_puts_nothing_on_screen() {
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 3),
        tab(Some("ops"), 2, true, 6),
    ]);

    assert_eq!(layout.on_screen_but(&Away::default()), [3, 6]);
    assert_eq!(layout.on_screen_but(&Away::of([2])), [3]);
    assert_eq!(
        layout.on_screen_but(&Away::of([0])),
        [6],
        "the project window minimised counts as nothing too"
    );
    assert_eq!(layout.on_screen_but(&Away::of([0, 2])), Vec::<u32>::new());
}

#[test]
fn a_tab_window_shows_only_its_first_front_tab() {
    // A tab window has one strip, whatever workspaces its tabs are on: one tab in front.
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 2, true, 5),
        tab(None, 2, true, 6),
    ]);

    assert_eq!(layout.on_screen(), [1, 5]);
}

#[test]
fn the_window_showing_a_chat_is_the_one_whose_front_tab_holds_it() {
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
        tab(Some("ops"), 3, true, 7),
    ]);

    assert_eq!(layout.window_showing(1), Some(0));
    assert_eq!(layout.window_showing(7), Some(3));
    assert_eq!(layout.window_showing(2), None, "a tab behind shows nothing");
    assert_eq!(layout.window_showing(9), None);
}

#[test]
fn the_tab_windows_are_every_window_but_the_project_window_in_number_order() {
    let layout = ops(vec![
        tab(Some("ops"), 4, true, 1),
        tab(Some("ops"), 0, true, 2),
        tab(None, 1, true, 3),
        tab(None, 4, false, 5),
    ]);

    assert_eq!(layout.tab_windows(), [1, 4]);
}

// ----- one window's report -----

#[test]
fn a_window_s_report_replaces_its_own_tabs_and_keeps_every_other_window_s() {
    let held = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 2, true, 5),
        tab(Some("ops"), 0, false, 3),
    ]);
    let report = Layout {
        strip: None,
        tabs: vec![tab(None, 0, true, 4)],
    };

    let merged = held.with_window(0, report);

    assert_eq!(merged.strip, None, "the project window says its strip");
    assert_eq!(shape(&merged), [(0, true, vec![4]), (2, true, vec![5])]);
}

#[test]
fn a_tab_window_s_report_keeps_the_project_window_s_strip_and_is_held_to_its_own_number() {
    let held = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 2, true, 5),
    ]);
    // A tab window says its tabs; whatever window it writes on them, they are its own.
    let report = Layout {
        strip: None,
        tabs: vec![tab(Some("ops"), 0, false, 5), tab(Some("ops"), 7, true, 6)],
    };

    let merged = held.with_window(2, report);

    assert_eq!(merged.strip, Some("ops".to_owned()));
    assert_eq!(
        shape(&merged),
        [(0, true, vec![1]), (2, false, vec![5]), (2, true, vec![6])]
    );
}

// ----- closing a tab window -----

#[test]
fn closing_a_tab_window_sends_its_tabs_behind_each_strip_s_front_tab_in_the_project_window() {
    let mut layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
        tab(Some("ops"), 2, false, 5),
        tab(Some("ops"), 2, true, 6),
        tab(Some("ops"), 3, true, 9),
    ]);

    assert!(layout.send_back(2));

    assert_eq!(
        shape(&layout),
        [
            (0, true, vec![1]),
            (0, false, vec![5]),
            (0, false, vec![6]),
            (0, false, vec![2]),
            (3, true, vec![9]),
        ]
    );
    assert_eq!(layout.on_screen(), [1, 9], "nothing it held comes in front");
    assert!(
        !layout.send_back(2),
        "a window holding nothing sends nothing"
    );
}

#[test]
fn a_tab_sent_back_to_a_strip_with_no_tab_in_the_project_window_is_that_strip_s_front() {
    let mut layout = ops(vec![tab(Some("ops"), 0, true, 1), tab(None, 2, false, 5)]);

    layout.send_back(2);

    assert_eq!(shape(&layout), [(0, true, vec![1]), (0, true, vec![5])]);
    assert_eq!(layout.on_screen(), [1], "a strip not drawn shows nothing");
}

// ----- moving a tab or a pane to another window -----

#[test]
fn move_to_new_window_makes_the_lowest_free_tab_window_with_the_tab_in_front() {
    let mut layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
        tab(Some("ops"), 2, true, 5),
    ]);

    let to = layout
        .move_tab(0, None)
        .expect("a tab of the project window");

    assert_eq!(to, 1);
    assert_eq!(
        shape(&layout),
        [(0, true, vec![2]), (2, true, vec![5]), (1, true, vec![1])],
        "the tab beside it comes forward where it was"
    );
    assert_eq!(layout.on_screen(), [1, 2, 5]);
}

#[test]
fn move_to_window_lands_a_tab_as_the_front_tab_there() {
    let mut layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
        tab(Some("ops"), 2, true, 5),
    ]);

    assert_eq!(layout.move_tab(1, Some(2)), Ok(2));

    assert_eq!(
        shape(&layout),
        [(0, true, vec![1]), (2, false, vec![5]), (2, true, vec![2])]
    );
}

#[test]
fn a_tab_moved_into_the_project_window_brings_its_strip_forward() {
    let mut layout = ops(vec![tab(Some("ops"), 0, true, 1), tab(None, 2, true, 5)]);

    assert_eq!(layout.move_tab(1, Some(0)), Ok(0));

    assert_eq!(layout.strip, None);
    assert_eq!(layout.on_screen(), [5]);
    assert!(
        layout.tab_windows().is_empty(),
        "its last tab gone, the window goes"
    );
}

#[test]
fn the_last_tab_of_the_drawn_strip_leaves_an_empty_pane_behind() {
    let mut layout = ops(vec![tab(Some("ops"), 0, true, 1)]);

    assert_eq!(layout.move_tab(0, None), Ok(1));

    assert_eq!(
        shape(&layout),
        [(0, true, vec![]), (1, true, vec![1])],
        "the strip's last tab keeps an empty pane"
    );
    assert_eq!(layout.tabs[0].tree, Node::Pane(Pane::Empty));
    assert_eq!(layout.tabs[0].workspace.as_deref(), Some("ops"));
}

#[test]
fn a_move_into_the_window_it_is_in_or_one_that_is_not_there_is_refused_and_changes_nothing() {
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 2, true, 5),
    ]);

    let mut same = layout.clone();
    assert_eq!(same.move_tab(1, Some(2)), Err(Refused::AlreadyThere));
    let mut nowhere = layout.clone();
    assert_eq!(nowhere.move_tab(0, Some(9)), Err(Refused::NoSuchWindow(9)));
    let mut no_tab = layout.clone();
    assert_eq!(no_tab.move_tab(5, None), Err(Refused::NoSuchTab));

    for after in [same, nowhere, no_tab] {
        assert_eq!(after, layout);
    }
}

#[test]
fn a_pane_moved_to_a_new_window_leaves_its_split_to_its_sibling() {
    let mut split = tab(Some("ops"), 0, true, 1);
    split.tree = Node::split(
        Direction::Row,
        300,
        Node::chat(1),
        Node::split(Direction::Column, EVEN, Node::chat(2), Node::chat(3)),
    );
    split.focused = vec![Side::B, Side::A];
    let mut layout = ops(vec![split]);

    assert_eq!(layout.move_pane(0, &[Side::B, Side::A], None), Ok(1));

    assert_eq!(
        layout.tabs[0].tree,
        Node::split(Direction::Row, 300, Node::chat(1), Node::chat(3))
    );
    assert_eq!(
        layout.tabs[0].focused,
        [Side::B],
        "focus goes to its sibling"
    );
    assert_eq!(shape(&layout)[1], (1, true, vec![2]));
    assert_eq!(layout.tabs[1].workspace.as_deref(), Some("ops"));
}

#[test]
fn a_tab_s_only_pane_moves_as_the_tab_and_an_empty_pane_is_never_picked_up() {
    let mut layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
    ]);
    assert_eq!(layout.move_pane(1, &[], None), Ok(1));
    assert_eq!(shape(&layout), [(0, true, vec![1]), (1, true, vec![2])]);

    let mut empty = tab(Some("ops"), 0, false, 1);
    empty.tree = Node::split(Direction::Row, EVEN, Node::chat(4), Node::Pane(Pane::Empty));
    let mut layout = ops(vec![tab(Some("ops"), 0, true, 3), empty]);
    let before = layout.clone();
    assert_eq!(
        layout.move_pane(1, &[Side::B], None),
        Err(Refused::EmptyPane)
    );
    assert_eq!(layout, before);
}

#[test]
fn a_move_never_shows_a_chat_twice() {
    let mut layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 0, false, 2),
        tab(None, 0, true, 3),
    ]);

    layout.move_tab(0, None).unwrap();
    layout.move_tab(0, Some(1)).unwrap();
    layout.send_back(1);

    assert_eq!(layout.named_twice(), None);
    let mut chats = layout.chats();
    chats.sort_unstable();
    assert_eq!(chats, [1, 2, 3]);
}

// ----- the record -----

#[test]
fn tab_windows_past_the_most_a_launch_remakes_go_back_to_the_project_window() {
    let mut tabs =
        vec![serde_json::json!({"workspace": "ops", "front": true, "tree": {"chat": 1}})];
    for window in 1..=(MOST_TAB_WINDOWS as u32 + 2) {
        tabs.push(serde_json::json!({
            "workspace": "ops", "window": window * 10, "front": true,
            "tree": {"chat": window + 1}
        }));
    }
    let json =
        serde_json::json!({"version": 1, "at": 0, "chats": [], "strip": "ops", "tabs": tabs});
    let plane = tempfile::tempdir().unwrap();
    let file = crate::reopen::path(plane.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, json.to_string()).unwrap();

    let back = read(plane.path()).layout.expect("tabs");

    assert_eq!(back.tab_windows().len(), MOST_TAB_WINDOWS);
    let sent_back: Vec<&Tab> = back
        .tabs
        .iter()
        .filter(|tab| tab.window == 0 && tab.tree.chats() != [1])
        .collect();
    assert_eq!(sent_back.len(), 2);
    assert!(sent_back.iter().all(|tab| !tab.front));
    assert_eq!(back.on_screen().first(), Some(&1));
}

#[test]
fn each_tab_window_and_its_front_tab_come_back_after_a_relaunch() {
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 1, false, 4),
        tab(None, 1, true, 5),
        tab(Some("ops"), 3, true, 7),
    ]);
    let plane = tempfile::tempdir().unwrap();

    write(
        plane.path(),
        &Record {
            layout: Some(layout.clone()),
            ..Record::default()
        },
    )
    .unwrap();
    let back = read(plane.path()).layout.expect("tabs");

    assert_eq!(back, layout);
    assert_eq!(back.tab_windows(), [1, 3]);
    assert_eq!(back.on_screen(), [1, 5, 7]);
}

#[test]
fn a_window_names_a_tab_by_its_place_among_its_own_tabs() {
    let layout = ops(vec![
        tab(Some("ops"), 0, true, 1),
        tab(Some("ops"), 2, true, 5),
        tab(Some("ops"), 0, false, 3),
    ]);

    assert_eq!(layout.place_of(0, 1), Some(2));
    assert_eq!(layout.place_of(2, 0), Some(1));
    assert_eq!(layout.place_of(2, 1), None);
}
