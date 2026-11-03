// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::active::Place;
use crate::reopen::layout::{Direction, Node, Pane, PaneView, Tab};
use crate::reopen::{Chat, HandedFrom, Mode, Owed, Record, View};

/// A chat numbered `number` that works in `workspace` (`""` for the plane root), as its cwd
/// says.
fn chat(number: u32, workspace: &str) -> Chat {
    Chat {
        program: "claude".to_owned(),
        name: format!("steward {number}"),
        number: Some(number),
        cwd: Some(if workspace.is_empty() {
            "/plane".into()
        } else {
            format!("/plane/workspaces/{workspace}").into()
        }),
        ..Chat::default()
    }
}

fn task_of(number: u32, asker: u32, workspace: &str) -> Chat {
    Chat {
        from: Some(HandedFrom {
            chat: asker,
            id: None,
            name: format!("steward {asker}"),
            workspace: Place::PlaneRoot,
            report: Owed::Nothing,
            mode: Mode::Task,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
        }),
        ..chat(number, workspace)
    }
}

fn view(title: &str, workspace: Option<&str>, at: u32) -> View {
    View {
        from: None,
        view: "persona".to_owned(),
        key: title.to_owned(),
        title: title.to_owned(),
        workspace: workspace.map(str::to_owned),
        at,
        active: false,
        pinned: false,
        split: None,
    }
}

fn upgrade(record: &Record) -> Layout {
    upgraded(record, &|chat: &Chat| {
        let cwd = chat.cwd.as_deref()?.to_str()?;
        cwd.strip_prefix("/plane/workspaces/").map(str::to_owned)
    })
}

fn tabs_on<'a>(layout: &'a Layout, workspace: Option<&str>) -> Vec<&'a Tab> {
    layout
        .tabs
        .iter()
        .filter(|tab| tab.workspace.as_deref() == workspace)
        .collect()
}

fn one_tab(chat: u32, workspace: Option<&str>, front: bool) -> Tab {
    Tab {
        workspace: workspace.map(str::to_owned),
        pinned: false,
        window: 0,
        front,
        focused: Vec::new(),
        tree: Node::chat(chat),
    }
}

fn empty_tab(workspace: Option<&str>) -> Tab {
    Tab {
        workspace: workspace.map(str::to_owned),
        pinned: false,
        window: 0,
        front: true,
        focused: Vec::new(),
        tree: Node::Pane(Pane::Empty),
    }
}

#[test]
fn a_pinned_chat_becomes_a_pinned_tab_and_stays_first_on_its_strip() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            chat(2, "ops"),
            Chat {
                pinned: true,
                ..chat(3, "ops")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    let ops = tabs_on(&layout, Some("ops"));
    assert_eq!(
        ops.len(),
        2,
        "the pinned tab and the front tab stay: {ops:?}"
    );
    assert_eq!(
        *ops[0],
        Tab {
            pinned: true,
            ..one_tab(3, Some("ops"), false)
        }
    );
    assert_eq!(*ops[1], one_tab(1, Some("ops"), true));
}

#[test]
fn chats_sharing_a_beside_become_one_tab_in_a_flat_row_with_even_shares() {
    let record = Record {
        chats: vec![
            chat(1, "ops"),
            Chat {
                beside: Some(1),
                ..chat(2, "ops")
            },
            Chat {
                beside: Some(1),
                ..task_of(3, 1, "ops")
            },
            Chat {
                active: true,
                ..chat(4, "ops")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    let ops = tabs_on(&layout, Some("ops"));
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert_eq!(
        ops[0].tree,
        Node::split(
            Direction::Row,
            333,
            Node::chat(1),
            Node::split(Direction::Row, 500, Node::chat(2), Node::chat(3)),
        )
    );
    assert!(!ops[0].pinned && !ops[0].front);
    assert_eq!(ops[0].focused, vec![super::super::layout::Side::A]);
}

#[test]
fn the_front_tab_at_quit_stays_as_it_was_and_its_strip_is_the_one_drawn() {
    let record = Record {
        chats: vec![
            chat(1, ""),
            chat(2, "ops"),
            Chat {
                active: true,
                ..chat(3, "ops")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(layout.strip.as_deref(), Some("ops"));
    assert_eq!(
        tabs_on(&layout, Some("ops")),
        vec![&one_tab(3, Some("ops"), true)]
    );
    assert_eq!(layout.lead(), Some(3));
}

#[test]
fn a_front_tab_whose_pane_showed_a_task_still_shows_that_task() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                shows: Some(2),
                ..chat(1, "ops")
            },
            task_of(2, 1, "ops"),
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(
        tabs_on(&layout, Some("ops")),
        vec![&one_tab(2, Some("ops"), true)]
    );
}

#[test]
fn a_pane_never_shows_a_chat_that_is_not_a_task_below_its_own() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                shows: Some(2),
                ..chat(1, "ops")
            },
            chat(2, "ops"),
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(
        tabs_on(&layout, Some("ops")),
        vec![&one_tab(1, Some("ops"), true)]
    );
}

#[test]
fn plain_one_chat_tabs_close_into_the_sidebar() {
    let record = Record {
        chats: vec![
            chat(1, "ops"),
            chat(2, "ops"),
            Chat {
                active: true,
                ..chat(3, "ops")
            },
            chat(4, "ops"),
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(layout.chats(), vec![3], "only the front tab stays");
    assert_eq!(record.chats.len(), 4, "every chat still comes back");
}

#[test]
fn a_workspace_left_with_no_tab_shows_its_first_chat_in_the_old_strip_order() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            chat(5, "web"),
            chat(6, "web"),
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(
        tabs_on(&layout, Some("web")),
        vec![&one_tab(5, Some("web"), true)]
    );
}

#[test]
fn a_workspace_left_with_no_tab_shows_its_first_tab_s_chat_not_a_task_that_had_none() {
    let record = Record {
        chats: vec![task_of(7, 9, "web"), chat(8, "web")],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(
        tabs_on(&layout, Some("web")),
        vec![&one_tab(8, Some("web"), true)]
    );
}

#[test]
fn the_plane_root_always_keeps_one_tab_an_empty_pane_where_it_has_no_chat() {
    let record = Record {
        chats: vec![Chat {
            active: true,
            ..chat(1, "ops")
        }],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(tabs_on(&layout, None), vec![&empty_tab(None)]);
}

#[test]
fn a_record_with_nothing_in_it_is_one_empty_pane_on_the_plane_root() {
    let layout = upgrade(&Record::default());

    assert_eq!(
        layout,
        Layout {
            strip: None,
            tabs: vec![empty_tab(None)],
        }
    );
}

#[test]
fn a_view_tab_stays_with_its_pin_and_its_place_among_the_chats() {
    let mut pinned = view("steward", Some("ops"), 2);
    pinned.pinned = true;
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            chat(2, "ops"),
        ],
        views: vec![view("reviewer", Some("ops"), 0), pinned.clone()],
        ..Record::default()
    };

    let layout = upgrade(&record);

    let ops = tabs_on(&layout, Some("ops"));
    let trees: Vec<&Node> = ops.iter().map(|tab| &tab.tree).collect();
    assert_eq!(
        trees,
        vec![
            &Node::Pane(Pane::View(PaneView::of(pinned))),
            &Node::Pane(Pane::View(PaneView::of(view("reviewer", Some("ops"), 0)))),
            &Node::chat(1),
        ]
    );
    assert!(ops[0].pinned);
}

#[test]
fn a_view_in_front_at_quit_is_the_front_tab_and_its_strip_is_drawn() {
    let mut front = view("steward", Some("ops"), 0);
    front.active = true;
    let record = Record {
        chats: vec![chat(1, "web")],
        views: vec![front],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(layout.strip.as_deref(), Some("ops"));
    assert!(tabs_on(&layout, Some("ops"))[0].front);
    assert_eq!(layout.lead(), None);
}

#[test]
fn a_task_never_gets_a_tab_it_did_not_have() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            Chat {
                pinned: true,
                ..task_of(2, 1, "ops")
            },
            Chat {
                graded: true,
                pinned: true,
                ..chat(3, "ops")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(layout.chats(), vec![1]);
}

#[test]
fn a_task_the_person_opened_kept_its_tab_and_closes_into_the_sidebar_like_any_other() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            Chat {
                tab_opened: true,
                ..task_of(2, 1, "ops")
            },
        ],
        ..Record::default()
    };

    assert_eq!(upgrade(&record).chats(), vec![1]);
}

#[test]
fn every_strip_has_exactly_one_front_tab() {
    let record = Record {
        chats: vec![
            Chat {
                pinned: true,
                ..chat(1, "ops")
            },
            Chat {
                pinned: true,
                ..chat(2, "ops")
            },
            Chat {
                active: true,
                ..chat(3, "")
            },
            Chat {
                active: true,
                ..chat(4, "")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    for strip in [Some("ops"), None] {
        let fronts = tabs_on(&layout, strip)
            .iter()
            .filter(|tab| tab.front)
            .count();
        assert_eq!(fronts, 1, "{strip:?}: {layout:?}");
    }
    assert_eq!(layout.strip, None);
    assert_eq!(
        layout.lead(),
        Some(3),
        "the first chat that says it was in front"
    );
}

#[test]
fn a_chat_is_never_named_in_two_panes_whatever_the_record_says() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                shows: Some(3),
                ..chat(1, "ops")
            },
            Chat {
                beside: Some(1),
                ..task_of(3, 1, "ops")
            },
            Chat {
                beside: Some(1),
                ..chat(1, "ops")
            },
        ],
        ..Record::default()
    };

    let layout = upgrade(&record);

    assert_eq!(layout.named_twice(), None, "{layout:?}");
}

#[test]
fn an_upgraded_layout_survives_the_record_s_round_trip_and_is_not_upgraded_again() {
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                ..chat(1, "ops")
            },
            chat(2, "ops"),
        ],
        ..Record::default()
    };
    let layout = upgrade(&record);
    let plane = tempfile::tempdir().unwrap();
    crate::reopen::write(
        plane.path(),
        &Record {
            layout: Some(layout.clone()),
            ..record
        },
    )
    .unwrap();

    let back = crate::reopen::read(plane.path());

    assert_eq!(back.layout, Some(layout));
}
