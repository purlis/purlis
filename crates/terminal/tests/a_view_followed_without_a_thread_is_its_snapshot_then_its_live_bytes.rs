// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A session host follows each view of a terminal without a thread of its own (spec #1974,
//! ADR 0082): the view is handed over taken apart, the screen as it is now on one side and an
//! asynchronous stream of what the program writes after it on the other, taken together so
//! nothing is lost or doubled between them (#1998).
#![cfg(unix)]

use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::executor::block_on;
use purlis_terminal::engine::{AlacrittyEngine, Size};
use purlis_terminal::session::{Session, Spec};

const SIZE: Size = Size {
    columns: 60,
    rows: 10,
};
const PATIENCE: Duration = Duration::from_secs(20);

fn shell(script: &str) -> Session {
    Session::spawn(
        Spec::new("/bin/sh", SIZE).args(["-c", script]),
        Box::new(AlacrittyEngine::new(SIZE, 100)),
    )
    .expect("the program starts")
}

fn until_on_screen(session: &Session, text: &str) {
    let deadline = Instant::now() + PATIENCE;
    while !session
        .screen()
        .lines
        .iter()
        .any(|line| line.contains(text))
    {
        assert!(Instant::now() < deadline, "{text:?} never appeared");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_snapshot_draws_what_was_written_and_the_live_bytes_follow_it() {
    let session = shell("printf 'written before'; read line; printf 'written after'; read line");
    until_on_screen(&session, "written before");

    let following = session.follow();
    assert_eq!(following.size, SIZE);
    let snapshot = String::from_utf8_lossy(&following.snapshot).into_owned();
    assert!(snapshot.contains("written before"), "{snapshot:?}");

    session.write(b"go\r").expect("the program reads");
    let mut live = following.live;
    let mut after = String::new();
    while !after.contains("written after") {
        let bytes = block_on(live.next()).expect("the program still writes");
        after.push_str(&String::from_utf8_lossy(&bytes));
    }
    assert!(
        !after.contains("written before"),
        "the live bytes do not repeat the snapshot: {after:?}"
    );
}

#[test]
fn the_live_bytes_end_when_the_program_does() {
    let session = shell("printf ready; read line; printf bye");
    until_on_screen(&session, "ready");
    let mut live = session.follow().live;

    session.write(b"\r").expect("the program reads");

    let mut rest = Vec::new();
    while let Some(bytes) = block_on(live.next()) {
        rest.extend_from_slice(&bytes);
    }
    assert!(String::from_utf8_lossy(&rest).contains("bye"));
}

#[test]
fn letting_go_of_the_live_bytes_closes_the_view() {
    let session = shell("printf ready; exec cat");
    until_on_screen(&session, "ready");
    let following = session.follow();
    assert_eq!(session.views(), 1);

    drop(following);

    assert_eq!(session.views(), 0);
}
