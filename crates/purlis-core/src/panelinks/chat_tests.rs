// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The chat kind (#6217): `chat 641` in a pane, resolved through the project's chat lookup.

use std::path::{Path, PathBuf};

use super::*;
use crate::chatlookup::OpenChat;
use crate::dispatchrecord::{
    self, Asker, ChatRef, Ending, Mode, Opening as Dispatch, Outcome, Place, Report, Worker,
};
use crate::panelinks::{Asks, Candidate, Hover, Kind, Opening, Pane, Resolved, Resolver, Target};

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

const WORKER_ID: &str = "01K6W0RKER000000000000000B";
const MINE_ID: &str = "01K6M1NE00000000000000000C";
const ELSEWHERE_ID: &str = "01K6E15EWHERE0000000000000";

/// A task chat 641 worked, opened in `root`'s records.
fn task_641(root: &Path) -> String {
    opened_641(root, Mode::Task, true)
}

/// Chat 641's dispatch as `mode`, owing a report or not, opened in `root`'s records.
fn opened_641(root: &Path, mode: Mode, report_owed: bool) -> String {
    let opening = Dispatch {
        mode,
        asker: Asker {
            chat: ChatRef {
                chat: 652,
                id: Some(MINE_ID.to_owned()),
                name: "Grill #5820: chat links".to_owned(),
                persona: Some("steward".to_owned()),
            },
            workspace: Some("smart-ide".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("steward".to_owned()),
        worker: Worker {
            chat: ChatRef {
                chat: 641,
                id: Some(WORKER_ID.to_owned()),
                name: "Signed commits #5607".to_owned(),
                persona: Some("steward".to_owned()),
            },
            harness: Some("claude".to_owned()),
            profile: None,
            session_record: None,
        },
        task: Some("Signed commits #5607".to_owned()),
        place: Place {
            workspace: Some("smart-ide".to_owned()),
            folder: Some("workspaces/smart-ide".to_owned()),
            worktree: None,
        },
        brief: "Sign the commits.".to_owned(),
        report_owed,
    };
    dispatchrecord::open(root, opening, at("2026-11-11T10:00:00Z"))
        .unwrap()
        .id
}

/// [`task_641`], finished with a report.
fn finished_641(root: &Path) -> String {
    let id = task_641(root);
    dispatchrecord::close(
        root,
        &id,
        Ending {
            report: Some(Report {
                outcome: Outcome::Done,
                text: "Signed all 12.".to_owned(),
                changed: Default::default(),
            }),
            usage: None,
        },
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    id
}

fn open(number: u32, id: &str) -> OpenChat {
    OpenChat {
        number,
        id: Some(id.to_owned()),
    }
}

fn chat(text: &str) -> Candidate {
    Candidate {
        kind: Kind::Chat,
        text: text.to_owned(),
        target: None,
    }
}

/// Chat 652's pane, in the project at `root`.
fn pane_of_652(root: &Path) -> Pane {
    Pane {
        project: Some(root.to_owned()),
        session: Some(652),
        ..Pane::default()
    }
}

/// The resolver the window builds for `root`, given the chats open now.
fn links(open: Vec<OpenChat>) -> Resolver {
    Resolver::default().with_kind(Box::new(ChatNumber::with_open(open)))
}

fn goes(resolved: Resolved) -> (u32, Goes) {
    match resolved {
        Resolved::Link(Target::Chat(chat)) => {
            let chat = *chat;
            (chat.number, chat.goes)
        }
        other => panic!("expected a chat link, got {other:?}"),
    }
}

#[test]
fn an_open_chat_links_and_opening_it_shows_it() {
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID), open(660, ELSEWHERE_ID)]);

    let resolved = links.resolve(&chat("chat 660"), &pane_of_652(&root));
    assert_eq!(goes(resolved), (660, Goes::Open));
    assert!(matches!(
        links.open(&chat("chat 660"), &pane_of_652(&root), &Asks::default(), None),
        Opening::Open(Target::Chat(at)) if matches!(at.goes, Goes::Open)
    ));
}

#[test]
fn an_open_chat_in_another_workspace_of_the_project_links_the_same_way() {
    // The lookup is the project's: a chat open in another of its workspaces is open, and the
    // window shows it in the workspace and tab it is in.
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID), open(641, WORKER_ID)]);

    assert_eq!(
        goes(links.resolve(&chat("chat 641"), &pane_of_652(&root))),
        (641, Goes::Open)
    );
}

#[test]
fn an_ended_chat_links_to_its_record_and_opening_it_shows_its_card() {
    let (_dir, root) = project();
    let id = finished_641(&root);
    let links = links(vec![open(652, MINE_ID)]);

    let (number, ended) = goes(links.resolve(&chat("chat 641"), &pane_of_652(&root)));
    assert_eq!(number, 641);
    let Goes::Ended(record) = ended else {
        panic!("not ended: {ended:?}");
    };
    assert_eq!(record.id, id);
    assert!(matches!(
        links.open(&chat("chat 641"), &pane_of_652(&root), &Asks::default(), None),
        Opening::Open(Target::Chat(at)) if matches!(at.goes, Goes::Ended(_))
    ));
}

#[test]
fn a_chat_no_dispatch_started_links_to_its_closed_record() {
    // #6304: a chat the person started by hand and closed, kept by the app as it closed.
    let (_dir, root) = project();
    let closed = crate::chatrecords::ChatRecord {
        number: 702,
        id: "01K6CHATS0000000000000000C".to_owned(),
        title: "Tidy the changelog".to_owned(),
        started: "2026-11-12T09:00:00Z".to_owned(),
        ended: "2026-11-12T09:40:00Z".to_owned(),
        started_by: crate::chatrecords::StartedBy::Person,
        place: "smart-ide".to_owned(),
    };
    crate::chatrecords::keep(&root, &closed).unwrap();
    let links = links(vec![open(652, MINE_ID)]);

    assert_eq!(
        goes(links.resolve(&chat("chat 702"), &pane_of_652(&root))),
        (702, Goes::Closed(Box::new(closed)))
    );
}

#[test]
fn a_closed_handoff_links_to_its_closed_record() {
    // A handoff's ended record draws no card: the chat's own record, kept as it closed, does.
    let (_dir, root) = project();
    let id = opened_641(&root, Mode::Handoff, false);
    dispatchrecord::close(
        &root,
        &id,
        Ending {
            report: None,
            usage: None,
        },
        at("2026-11-11T11:08:00Z"),
    )
    .unwrap();
    let closed = crate::chatrecords::ChatRecord {
        number: 641,
        id: WORKER_ID.to_owned(),
        title: "Signed commits #5607".to_owned(),
        started: "2026-11-11T10:00:00Z".to_owned(),
        ended: "2026-11-11T11:08:00Z".to_owned(),
        started_by: crate::chatrecords::StartedBy::Chat {
            name: "Grill #5820: chat links".to_owned(),
        },
        place: "smart-ide".to_owned(),
    };
    crate::chatrecords::keep(&root, &closed).unwrap();
    let links = links(vec![open(652, MINE_ID)]);

    assert_eq!(
        goes(links.resolve(&chat("chat 641"), &pane_of_652(&root))),
        (641, Goes::Closed(Box::new(closed)))
    );
}

#[test]
fn a_restarted_chat_is_reached_through_its_ulid_under_its_number_now() {
    let (_dir, root) = project();
    finished_641(&root);
    // Chat 641 started again: it keeps its ULID and is dealt 700.
    let links = links(vec![open(652, MINE_ID), open(700, WORKER_ID)]);

    let (number, goes) = goes(links.resolve(&chat("chat 641"), &pane_of_652(&root)));
    assert_eq!((number, goes), (700, Goes::Open));
}

#[test]
fn a_number_from_another_project_is_not_a_link() {
    let (_other_dir, other) = project();
    finished_641(&other);
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID)]);

    // Chat 641 is the other project's: this pane's project has none.
    assert_eq!(
        links.resolve(&chat("chat 641"), &pane_of_652(&root)),
        Resolved::Quiet
    );
    assert!(matches!(
        links.open(
            &chat("chat 641"),
            &pane_of_652(&root),
            &Asks::default(),
            None
        ),
        Opening::Refused(_)
    ));
}

#[test]
fn an_unknown_number_is_not_a_link_and_a_hover_says_nothing() {
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID)]);

    assert_eq!(
        links.hover(&chat("chat 9999"), &pane_of_652(&root)),
        Hover::Quiet
    );
    assert!(matches!(
        links.open(
            &chat("chat 9999"),
            &pane_of_652(&root),
            &Asks::default(),
            None
        ),
        Opening::Refused(_)
    ));
}

#[test]
fn its_own_number_is_this_chat_and_opening_it_does_nothing_more() {
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID)]);

    assert_eq!(
        goes(links.resolve(&chat("chat 652"), &pane_of_652(&root))),
        (652, Goes::ThisChat)
    );
    assert!(matches!(
        links.open(&chat("chat 652"), &pane_of_652(&root), &Asks::default(), None),
        Opening::Open(Target::Chat(at)) if matches!(at.goes, Goes::ThisChat)
    ));
}

#[test]
fn its_own_old_number_after_a_restart_is_this_chat_too() {
    let (_dir, root) = project();
    finished_641(&root);
    // This pane's chat is 641 started again, now 652.
    let links = links(vec![open(652, WORKER_ID)]);

    assert_eq!(
        goes(links.resolve(&chat("chat 641"), &pane_of_652(&root))),
        (652, Goes::ThisChat)
    );
}

#[test]
fn a_pane_with_no_project_links_no_chat() {
    let links = links(vec![open(652, MINE_ID)]);

    assert_eq!(
        links.resolve(&chat("chat 652"), &Pane::default()),
        Resolved::Quiet
    );
}

#[test]
fn only_the_word_chat_and_a_number_reads_as_one() {
    let (_dir, root) = project();
    let links = links(vec![open(652, MINE_ID), open(660, ELSEWHERE_ID)]);
    for text in [
        "#660",
        "660",
        "chat #660",
        "chat  660",
        "chat 660 ",
        "chat 0660",
        "chat 66O",
        "chat 99999999999",
        "Docs site: the setup page",
        "chats 660",
    ] {
        assert!(
            matches!(
                links.resolve(&chat(text), &pane_of_652(&root)),
                Resolved::NotALink(_)
            ),
            "{text:?} linked"
        );
    }
    // The word as a sentence starts it is the same word.
    assert_eq!(
        goes(links.resolve(&chat("Chat 660"), &pane_of_652(&root))),
        (660, Goes::Open)
    );
}

#[test]
fn a_chat_link_is_shown_and_copied_as_its_words_with_its_number_now() {
    let (_dir, root) = project();
    finished_641(&root);
    let links = links(vec![open(652, MINE_ID), open(700, WORKER_ID)]);

    let Resolved::Link(target) = links.resolve(&chat("chat 641"), &pane_of_652(&root)) else {
        panic!("no link");
    };
    assert_eq!(target.kind(), Kind::Chat);
    assert_eq!(target.shown(), "chat 700");
}

#[test]
fn a_batch_reads_the_project_s_records_once() {
    let (_dir, root) = project();
    finished_641(&root);
    let links = links(vec![open(652, MINE_ID)]);

    assert!(matches!(
        goes(links.resolve(&chat("chat 641"), &pane_of_652(&root))),
        (641, Goes::Ended(_))
    ));
    // Gone from the disk after the first lookup: the batch still answers from what it read.
    std::fs::remove_dir_all(dispatchrecord::dir(&root)).unwrap();
    assert!(matches!(
        goes(links.resolve(&chat("chat 641"), &pane_of_652(&root))),
        (641, Goes::Ended(_))
    ));
    // A new batch reads again, and finds none.
    assert_eq!(
        self::links(vec![open(652, MINE_ID)]).resolve(&chat("chat 641"), &pane_of_652(&root)),
        Resolved::Quiet
    );
}
