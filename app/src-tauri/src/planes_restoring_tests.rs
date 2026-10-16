// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's open answers before any of its chats has started (`restoring.rs`): the
//! registry's half, on a host that runs nothing and whose starts a test can hold.
//!
//! The chats' own half — the order they start in, how many at once, what a record written
//! meanwhile holds — is `chats_restoring_tests.rs`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::reopen;

use super::{Opening, Planes};
use crate::hooks::Moved;
use crate::host::pretend::Pretend;

/// How long a test waits for the thread behind the window before it calls it stuck.
const SOON: Duration = Duration::from_secs(30);

/// A plane on disk holding a record of `count` shells, numbered from 1, the first in front.
fn a_plane_with_chats(at: &Path, count: u32) -> PathBuf {
    std::fs::create_dir_all(at).expect("the plane's directory");
    std::fs::write(at.join(purlis_core::plane::MANIFEST), "").expect("its manifest");
    reopen::write(
        at,
        &reopen::Record {
            chats: (1..=count)
                .map(|number| reopen::Chat {
                    program: "/bin/sh".to_owned(),
                    name: format!("chat {number}"),
                    number: Some(number),
                    active: number == 1,
                    ..Default::default()
                })
                .collect(),
            dealt: count,
            ..reopen::Record::default()
        },
    )
    .expect("the record is written");
    at.to_path_buf()
}

/// A registry as the app makes it, starting chats behind the window: each project it opens
/// runs on the next of `hosts`, in turn.
fn planes_on(hosts: &[&Pretend], config: &Path) -> Planes {
    let hosts: Mutex<Vec<Pretend>> =
        Mutex::new(hosts.iter().rev().map(|&one| one.clone()).collect());
    Planes::telling(
        Arc::new(|_: Moved| {}),
        crate::Shipped::default(),
        Some(config.to_path_buf()),
    )
    .restoring_behind_the_window()
    .running_sessions_on(Arc::new(move |_, _: &std::path::Path| {
        Box::new(
            hosts
                .lock()
                .unwrap()
                .pop()
                .expect("a host for each project"),
        )
    }))
}

/// Opens `root` as the window does: asked about, then approved.
fn approve_and_open(planes: &Planes, root: &Path) -> super::PlaneId {
    let shown = match planes.open_if_approved(root).expect("it is a plane") {
        Opening::Ask(ask) => ask.contributes,
        Opening::Open(plane) => panic!("it opened {} without asking", plane.as_str()),
    };
    planes.approve_and_open(root, &shown).expect("yes")
}

fn until(what: &str, done: impl Fn() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < SOON, "still waiting: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
#[test]
fn a_project_is_open_with_every_tab_to_draw_before_any_of_its_chats_has_started() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_chats(&dir.path().join("plane"), 6);
    let host = Pretend::default();
    // No start returns until the test lets it: the open must not be waiting on one.
    for number in 1..=6 {
        host.hangs_number(number);
    }
    let planes = planes_on(&[&host], &dir.path().join("config"));

    let plane = approve_and_open(&planes, &root);

    let held = planes.held(&plane).expect("it is held");
    assert!(held.chats().open_now().is_empty(), "a chat had started");
    let drawn = held.chats().drawn_now();
    assert_eq!(
        drawn.iter().map(|(one, _)| one.session).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6],
        "every chat has a tab to draw, in the record's order"
    );
    assert!(drawn.iter().all(|(_, restoring)| *restoring));

    // And they start behind it, as the host lets them.
    for number in 1..=6 {
        host.lets_go(number);
    }
    until("every chat to start", || held.chats().open_now().len() == 6);
    until("the launch to settle", || {
        held.chats()
            .drawn_now()
            .iter()
            .all(|(_, restoring)| !*restoring)
    });
    planes.let_go_of_all();
}

#[cfg(unix)]
#[test]
fn a_chat_that_hangs_as_it_starts_holds_up_neither_the_open_nor_another_project() {
    let dir = tempfile::tempdir().expect("a directory");
    let stuck = a_plane_with_chats(&dir.path().join("stuck"), 3);
    let other = a_plane_with_chats(&dir.path().join("other"), 3);
    let (stuck_host, other_host) = (Pretend::default(), Pretend::default());
    // The first project's chat in front never starts, for as long as the test looks.
    stuck_host.hangs_number(1);
    let planes = planes_on(&[&stuck_host, &other_host], &dir.path().join("config"));

    let first = approve_and_open(&planes, &stuck);
    let second = approve_and_open(&planes, &other);

    let (first, second) = (
        planes.held(&first).expect("it is held"),
        planes.held(&second).expect("it is held"),
    );
    until("the other project's chats", || {
        second.chats().open_now().len() == 3
    });
    until("the stuck project's other chats", || {
        first.chats().open_now().len() == 2
    });
    assert_eq!(
        first
            .chats()
            .drawn_now()
            .into_iter()
            .filter(|(_, restoring)| *restoring)
            .map(|(one, _)| one.session)
            .collect::<Vec<_>>(),
        [1],
        "only the chat that hangs is still restoring"
    );

    stuck_host.lets_go(1);
    until("the chat that hung", || first.chats().open_now().len() == 3);
    planes.let_go_of_all();
}

#[cfg(unix)]
#[test]
fn a_quit_while_chats_come_back_leaves_every_chat_in_the_record() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_chats(&dir.path().join("plane"), 5);
    let host = Pretend::default();
    // The chat in front hangs as it starts, and the quit comes while it does.
    host.hangs_number(1);
    let planes = planes_on(&[&host], &dir.path().join("config"));
    let plane = approve_and_open(&planes, &root);
    let held = planes.held(&plane).expect("it is held");
    until("the first start to be under way", || {
        host.most_at_once() >= 1
    });

    planes.let_go_of_all();

    let kept = reopen::read_or_refusal(&root).expect("the record reads");
    let mut names: Vec<String> = kept.chats.iter().map(|chat| chat.name.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        ["chat 1", "chat 2", "chat 3", "chat 4", "chat 5"],
        "a chat that had not started yet was not kept for the next launch"
    );
    // The start under way returns into a project that is closing, and is ended with it.
    host.lets_go(1);
    until("the launch to give up", || {
        held.chats().drawn_now().is_empty() && held.chats().open_now().is_empty()
    });
}

/// #2004: a window whose chats run in its own process puts none back before its gate lets it,
/// so a session host a crashed window left has ended the chats it kept first; and a gate that
/// refuses puts none back and leaves the record as it was.
#[cfg(unix)]
#[test]
fn no_chat_is_put_back_before_the_gate_lets_it_and_none_when_it_refuses() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_chats(&dir.path().join("plane"), 2);
    let host = Pretend::default();
    let (asked, entered) = std::sync::mpsc::channel::<()>();
    let (open, opened) = std::sync::mpsc::channel::<()>();
    let (asked, opened) = (Mutex::new(asked), Mutex::new(opened));
    let planes = planes_on(&[&host], &dir.path().join("config")).putting_back_chats_after(
        Arc::new(move || {
            let _ = asked.lock().unwrap().send(());
            opened
                .lock()
                .unwrap()
                .recv()
                .map_err(|_| "the gate stayed shut".to_owned())
        }),
    );

    let plane = approve_and_open(&planes, &root);
    let held = planes.held(&plane).expect("it is held");
    // The gate is asked, and holds: a start comes only after it answers.
    entered
        .recv_timeout(SOON)
        .expect("the gate is asked before anything starts");
    assert!(
        host.asked().is_empty(),
        "a chat started before the gate let it"
    );
    assert_eq!(held.chats().drawn_now().len(), 2, "each still has its tab");

    open.send(()).expect("the gate opens");
    until("both chats to start", || held.chats().open_now().len() == 2);
    planes.let_go_of_all();

    // A gate that refuses, on the thread that asks: once the open returns, the put back is
    // over, nothing started, and the record still holds every chat.
    let refusing = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_chats(&refusing.path().join("plane"), 2);
    let host = Pretend::default();
    let planes = {
        let host = host.clone();
        Planes::telling(
            Arc::new(|_: Moved| {}),
            crate::Shipped::default(),
            Some(refusing.path().join("config")),
        )
        .running_sessions_on(Arc::new(move |_, _: &std::path::Path| {
            Box::new(host.clone())
        }))
        .putting_back_chats_after(Arc::new(|| Err("a host still runs them".to_owned())))
    };
    let plane = approve_and_open(&planes, &root);
    let held = planes.held(&plane).expect("it is held");
    assert!(host.asked().is_empty(), "a chat started past a refusal");
    assert!(held.chats().open_now().is_empty());
    planes.let_go_of_all();
    let kept = reopen::read_or_refusal(&root).expect("the record reads");
    let mut names: Vec<String> = kept.chats.iter().map(|chat| chat.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["chat 1", "chat 2"], "the record lost a chat");
}
