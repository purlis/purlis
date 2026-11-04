// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::reopen::{Record, read, write};
use std::fs;

fn two_chats_split(share: u16) -> Tab {
    Tab {
        workspace: Some("ops".to_owned()),
        pinned: false,
        window: 0,
        front: true,
        focused: vec![Side::B],
        tree: Node::split(Direction::Row, share, Node::chat(3), Node::chat(4)),
    }
}

fn file_view() -> PaneView {
    PaneView {
        from: None,
        view: "persona".to_owned(),
        key: "steward".to_owned(),
        title: "steward".to_owned(),
        split: None,
    }
}

fn record_of(layout: Layout) -> Record {
    Record {
        layout: Some(layout),
        ..Record::default()
    }
}

fn written_and_read(record: &Record) -> Record {
    let plane = tempfile::tempdir().unwrap();
    write(plane.path(), record).unwrap();
    read(plane.path())
}

fn read_from(json: &str) -> Record {
    let plane = tempfile::tempdir().unwrap();
    let file = crate::reopen::path(plane.path());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, json).unwrap();
    read(plane.path())
}

#[test]
fn two_chats_split_together_with_their_sizes_come_back_the_same() {
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![two_chats_split(333)],
    };

    let back = written_and_read(&record_of(layout.clone()));

    assert_eq!(back.layout, Some(layout));
}

#[test]
fn a_view_beside_a_chat_and_an_empty_pane_come_back() {
    let layout = Layout {
        strip: None,
        tabs: vec![Tab {
            workspace: None,
            pinned: true,
            window: 0,
            front: true,
            focused: vec![Side::A, Side::B],
            tree: Node::split(
                Direction::Column,
                700,
                Node::split(
                    Direction::Row,
                    EVEN,
                    Node::chat(7),
                    Node::Pane(Pane::View(file_view())),
                ),
                Node::Pane(Pane::Empty),
            ),
        }],
    };

    let back = written_and_read(&record_of(layout.clone()));

    assert_eq!(back.layout, Some(layout));
}

#[test]
fn each_tab_keeps_its_workspace_pin_window_and_whether_it_is_its_strip_s_front() {
    let mut outside = Tab::of_chat(None, 9);
    outside.front = true;
    let mut pinned_behind = Tab::of_chat(Some("ops"), 5);
    pinned_behind.pinned = true;
    let mut torn_off = Tab::of_chat(Some("ops"), 6);
    torn_off.window = 2;
    torn_off.front = true;
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![two_chats_split(EVEN), pinned_behind, outside, torn_off],
    };

    let back = written_and_read(&record_of(layout.clone()))
        .layout
        .expect("tabs were written");

    assert_eq!(back, layout);
    let fronts: Vec<(Option<&str>, u32)> = back
        .tabs
        .iter()
        .filter(|tab| tab.front)
        .map(|tab| (tab.workspace.as_deref(), tab.window))
        .collect();
    assert_eq!(fronts, [(Some("ops"), 0), (None, 0), (Some("ops"), 2)]);
}

#[test]
fn a_record_with_no_tabs_list_has_no_layout_and_is_read_the_old_way() {
    let back = read_from(
        r#"{"version": 1, "at": 0, "chats": [{"program": "claude", "number": 3, "beside": 2, "shows": 4}]}"#,
    );

    assert_eq!(back.layout, None);
    assert_eq!(back.chats[0].beside, Some(2));
    assert_eq!(back.chats[0].shows, Some(4));
}

#[test]
fn a_record_with_no_layout_writes_no_tabs_key_and_one_with_a_layout_writes_one() {
    let plane = tempfile::tempdir().unwrap();
    write(plane.path(), &Record::default()).unwrap();
    let text = fs::read_to_string(crate::reopen::path(plane.path())).unwrap();
    assert!(!text.contains("\"tabs\""), "{text}");

    write(plane.path(), &record_of(Layout::default())).unwrap();
    let text = fs::read_to_string(crate::reopen::path(plane.path())).unwrap();
    assert!(text.contains("\"tabs\": []"), "{text}");
    assert_eq!(read(plane.path()).layout, Some(Layout::default()));
}

#[test]
fn the_tree_is_written_in_the_shape_the_plane_format_names() {
    let plane = tempfile::tempdir().unwrap();
    write(
        plane.path(),
        &record_of(Layout {
            strip: Some("ops".to_owned()),
            tabs: vec![two_chats_split(250)],
        }),
    )
    .unwrap();
    let text = fs::read_to_string(crate::reopen::path(plane.path())).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(json["strip"], "ops");
    assert_eq!(
        json["tabs"][0],
        serde_json::json!({
            "workspace": "ops",
            "front": true,
            "focused": "b",
            "tree": {"direction": "row", "share": 250, "a": {"chat": 3}, "b": {"chat": 4}},
        })
    );
}

#[test]
fn a_node_that_is_not_one_reads_as_an_empty_pane_and_a_share_that_is_not_one_as_even() {
    let back = read_from(
        r#"{"version": 1, "at": 0, "chats": [], "strip": "ops", "tabs": [
            {"workspace": "ops", "tree": {"direction": "sideways", "share": 4000,
                "a": "chat 3", "b": {"chat": 0}}},
            {"workspace": "../etc", "tree": {"view": {"view": "../../x"}}}
        ]}"#,
    )
    .layout
    .expect("a tabs list");

    assert_eq!(
        back.tabs[0].tree,
        Node::split(
            Direction::Row,
            EVEN,
            Node::Pane(Pane::Empty),
            Node::Pane(Pane::Empty)
        )
    );
    assert_eq!(back.tabs[0].focused, [Side::A], "ends at a pane");
    assert_eq!(back.tabs[1].workspace, None, "not a workspace's name");
    assert_eq!(back.tabs[1].tree, Node::Pane(Pane::Empty));
}

#[test]
fn a_chat_named_twice_on_disk_keeps_its_first_pane_and_empties_the_rest() {
    let back = read_from(
        r#"{"version": 1, "at": 0, "chats": [], "tabs": [
            {"tree": {"chat": 3}},
            {"tree": {"direction": "row", "share": 500, "a": {"chat": 4}, "b": {"chat": 3}}}
        ]}"#,
    )
    .layout
    .expect("a tabs list");

    assert_eq!(back.named_twice(), None);
    assert_eq!(back.chats(), [3, 4]);
}

#[test]
fn a_tab_past_the_panes_one_holds_is_dropped_and_a_tree_past_the_deepest_is_cut() {
    let mut deep = serde_json::json!({"chat": 1});
    for _ in 0..(DEEPEST + 4) {
        deep =
            serde_json::json!({"direction": "row", "share": 500, "a": deep, "b": {"empty": true}});
    }
    // Balanced, so it is well within the deepest and still past the panes one tab holds.
    let mut wide = serde_json::json!({"empty": true});
    let mut panes = 1;
    while panes <= MOST_PANES {
        wide = serde_json::json!({"direction": "row", "share": 500, "a": wide.clone(), "b": wide});
        panes *= 2;
    }
    let json = serde_json::json!({"version": 1, "at": 0, "chats": [], "tabs": [
        {"tree": deep}, {"tree": wide}
    ]});

    let back = read_from(&json.to_string()).layout.expect("a tabs list");

    assert_eq!(back.tabs.len(), 1, "the wide one is dropped");
    assert_eq!(
        back.chats(),
        Vec::<u32>::new(),
        "the chat below the deepest is cut"
    );
}

#[test]
fn on_screen_is_every_chat_in_the_drawn_strip_s_front_tab_and_each_tab_window_s() {
    let mut other_strip = Tab::of_chat(None, 8);
    other_strip.front = true;
    let mut behind = Tab::of_chat(Some("ops"), 5);
    behind.front = false;
    let mut torn_off = Tab::of_chat(Some("ops"), 6);
    torn_off.window = 2;
    torn_off.front = true;
    let mut with_a_view = two_chats_split(EVEN);
    with_a_view.tree = Node::split(
        Direction::Row,
        EVEN,
        with_a_view.tree,
        Node::split(
            Direction::Column,
            EVEN,
            Node::Pane(Pane::View(file_view())),
            Node::Pane(Pane::Empty),
        ),
    );
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![with_a_view, behind, other_strip, torn_off],
    };

    assert_eq!(layout.on_screen(), [3, 4, 6]);
    assert_eq!(layout.lead(), Some(3));
}

#[test]
fn a_front_tab_of_only_a_view_or_empty_panes_puts_nothing_on_screen() {
    let mut tab = Tab::of_chat(Some("ops"), 1);
    tab.front = true;
    tab.tree = Node::split(
        Direction::Row,
        EVEN,
        Node::Pane(Pane::View(file_view())),
        Node::Pane(Pane::Empty),
    );
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![tab],
    };

    assert_eq!(layout.on_screen(), Vec::<u32>::new());
    assert_eq!(layout.lead(), None);
}

#[test]
fn a_record_whose_tabs_hold_only_a_view_has_something_to_put_back() {
    let mut tab = Tab::of_chat(None, 1);
    tab.tree = Node::Pane(Pane::View(file_view()));
    let record = record_of(Layout {
        strip: None,
        tabs: vec![tab],
    });

    assert!(record.holds_anything());
    assert!(!record_of(Layout::default()).holds_anything());
}

#[test]
fn a_tab_dropped_for_its_size_takes_no_chat_from_the_tabs_after_it() {
    let mut wide = serde_json::json!({"chat": 7});
    let mut panes = 1;
    while panes <= MOST_PANES {
        wide = serde_json::json!({"direction": "row", "share": 500, "a": wide.clone(), "b": wide});
        panes *= 2;
    }
    let json = serde_json::json!({"version": 1, "at": 0, "chats": [], "tabs": [
        {"tree": wide}, {"tree": {"chat": 7}}
    ]});

    let back = read_from(&json.to_string()).layout.expect("a tabs list");

    assert_eq!(
        back.chats(),
        [7],
        "the tab kept shows the chat the dropped one named"
    );
}

/// **A one-job split is never recorded** (#4761, spec #4750): a terminal editor or a forge
/// login opened beside a pane is left out, and the pane it was beside takes the split's place
/// with the focus, as the window draws it once the program exits.
#[test]
fn a_one_job_pane_leaves_its_split_to_the_pane_it_was_beside_with_the_focus() {
    let beside = Tab {
        focused: vec![Side::A, Side::B],
        tree: Node::split(
            Direction::Column,
            300,
            Node::split(Direction::Row, EVEN, Node::chat(3), Node::chat(9)),
            Node::chat(4),
        ),
        ..two_chats_split(EVEN)
    };
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![beside],
    };

    let kept = layout.without(|chat| chat == 9);

    assert_eq!(
        kept.tabs[0].tree,
        Node::split(Direction::Column, 300, Node::chat(3), Node::chat(4)),
    );
    assert_eq!(kept.tabs[0].focused, [Side::A]);
    assert_eq!(kept.on_screen(), [3, 4]);
}

#[test]
fn a_focus_outside_the_one_job_pane_stays_on_the_pane_it_was_on() {
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![Tab {
            focused: vec![Side::B, Side::B],
            tree: Node::split(
                Direction::Row,
                EVEN,
                Node::chat(9),
                Node::split(Direction::Column, 600, Node::chat(3), Node::chat(4)),
            ),
            ..two_chats_split(EVEN)
        }],
    };

    let kept = layout.without(|chat| chat == 9);

    assert_eq!(
        kept.tabs[0].tree,
        Node::split(Direction::Column, 600, Node::chat(3), Node::chat(4)),
    );
    assert_eq!(kept.tabs[0].focused, [Side::B]);
}

#[test]
fn a_tab_of_nothing_but_a_one_job_pane_keeps_an_empty_pane() {
    let layout = Layout {
        strip: None,
        tabs: vec![Tab::of_chat(None, 9)],
    };

    let kept = layout.without(|chat| chat == 9);

    assert_eq!(kept.tabs.len(), 1);
    assert_eq!(kept.tabs[0].tree, Node::Pane(Pane::Empty));
    assert!(kept.tabs[0].focused.is_empty());
}

#[test]
fn a_layout_with_no_one_job_pane_is_kept_as_it_is() {
    let layout = Layout {
        strip: Some("ops".to_owned()),
        tabs: vec![two_chats_split(333)],
    };

    assert_eq!(layout.clone().without(|_| false), layout);
}
