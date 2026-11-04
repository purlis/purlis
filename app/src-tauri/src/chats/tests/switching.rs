// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The switch: a chat isn't a tab** (#4758). A record written before tabs were recorded is
//! read into one layout at the launch that finds it, as `purlis_core::reopen::upgrade` rules,
//! and from then on what is on screen is that layout's and no chat's tab.

use purlis_core::reopen::layout::{Layout, Node, Pane};
use purlis_core::reopen::{Chat, Record};

use super::Chats;
use crate::host::pretend::Pretend;

fn chats_in(project: &std::path::Path) -> Chats {
    Chats::on_host(
        Box::new(|_| {}),
        Box::new(Pretend::default()),
        project.to_path_buf(),
    )
}

fn shell(number: u32, project: &std::path::Path) -> Chat {
    Chat {
        program: "/bin/sh".to_owned(),
        name: format!("chat {number}"),
        number: Some(number),
        cwd: Some(project.to_path_buf()),
        ..Chat::default()
    }
}

#[test]
fn a_record_from_before_tabs_is_upgraded_at_the_launch_and_only_its_front_tab_stays() {
    let project = tempfile::tempdir().expect("a project");
    let chats = chats_in(project.path());
    let record = Record {
        chats: vec![
            shell(1, project.path()),
            Chat {
                active: true,
                ..shell(2, project.path())
            },
            shell(3, project.path()),
        ],
        dealt: 3,
        ..Record::default()
    };

    chats.begin_put_back(&record, &|_| None);

    let layout = chats.layout().expect("the upgrade's layout");
    assert_eq!(
        layout.chats(),
        vec![2],
        "plain tabs closed into the sidebar"
    );
    assert_eq!(chats.looked_at(), vec![2]);
}

#[test]
fn a_record_from_before_tabs_with_no_chat_is_one_empty_pane() {
    let project = tempfile::tempdir().expect("a project");
    let chats = chats_in(project.path());

    chats.begin_put_back(&Record::default(), &|_| None);

    let layout = chats.layout().expect("the upgrade's layout");
    assert_eq!(layout.tabs.len(), 1);
    assert_eq!(layout.tabs[0].tree, Node::Pane(Pane::Empty));
    assert!(chats.looked_at().is_empty());
}

#[test]
fn a_relaunch_never_gives_a_chat_a_tab_its_record_did_not_keep() {
    let project = tempfile::tempdir().expect("a project");
    let chats = chats_in(project.path());
    let kept = Layout {
        strip: None,
        tabs: vec![purlis_core::reopen::layout::Tab::of_chat(None, 1)],
    };
    let record = Record {
        chats: vec![
            Chat {
                active: true,
                pinned: true,
                ..shell(1, project.path())
            },
            Chat {
                active: true,
                pinned: true,
                ..shell(2, project.path())
            },
        ],
        dealt: 2,
        layout: Some(kept.clone()),
        ..Record::default()
    };

    chats.begin_put_back(&record, &|_| None);

    assert_eq!(chats.layout(), Some(kept));
}

#[test]
fn the_front_tab_of_a_record_written_before_numbers_comes_back_in_front() {
    // A record from before chats kept their numbers: the upgrade reads it once each chat has
    // the number it starts under, so the chat that was in front is in front.
    let project = tempfile::tempdir().expect("a project");
    let chats = chats_in(project.path());
    let record = Record {
        chats: vec![
            Chat {
                number: None,
                ..shell(1, project.path())
            },
            Chat {
                number: None,
                active: true,
                ..shell(2, project.path())
            },
        ],
        ..Record::default()
    };

    chats.begin_put_back(&record, &|_| None);

    let front = chats.front().expect("a chat in front");
    let layout = chats.layout().expect("the upgrade's layout");
    assert_eq!(layout.chats(), vec![front]);
    assert_eq!(chats.looked_at(), vec![front]);
}
