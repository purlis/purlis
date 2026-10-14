// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn listening() -> (tempfile::TempDir, std::path::PathBuf, UnixListener) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("charterd.sock");
    let listener = UnixListener::bind(&path).unwrap();
    (dir, path, listener)
}

#[tokio::test]
async fn a_connection_from_this_uid_is_handed_on() {
    let (_dir, path, listener) = listening();
    let listener = Listener::new(listener);
    let mut client = UnixStream::connect(&path).await.unwrap();

    let accepted = listener.accept().await.unwrap().expect("admitted");
    assert_eq!(
        accepted.pid(),
        std::process::id(),
        "the peer's pid is this test's"
    );
    let mut stream = accepted.stream;

    client.write_all(b"x").await.unwrap();
    assert_eq!(stream.read_u8().await.unwrap(), b'x');
}

/// The listener's verdict on this test's own connection, with the owner and the way the
/// peer is identified set by the test.
async fn verdict(
    owner: Uid,
    identify: Identify,
) -> (Result<SameUser, NotThisUser>, UnixStream, tempfile::TempDir) {
    let (dir, path, listener) = listening();
    let listener = Listener {
        listener,
        owner,
        identify,
    };
    let client = UnixStream::connect(&path).await.unwrap();
    (listener.accept().await.unwrap(), client, dir)
}

async fn closed_unread(mut client: UnixStream) {
    let mut said = Vec::new();
    client.read_to_end(&mut said).await.unwrap();
    assert!(said.is_empty());
}

#[tokio::test]
async fn a_connection_from_another_uid_is_closed_before_it_says_anything() {
    // No second user to connect as, so the listener is told it serves a uid this test is
    // not: this test's own connection is then the other user's.
    let ours = Uid::effective();
    let theirs = Uid::from_raw(ours.as_raw().wrapping_add(1));
    let (refused, client, _dir) = verdict(theirs, purlis_same_user::peer_of).await;

    assert!(
        matches!(refused, Err(NotThisUser::OtherUid { peer, .. }) if peer == ours),
        "{refused:?}"
    );
    closed_unread(client).await;
}

#[tokio::test]
async fn a_peer_the_socket_will_not_identify_is_closed_unread() {
    let (refused, client, _dir) = verdict(Uid::effective(), |_| {
        Err(std::io::Error::other("no peer credentials"))
    })
    .await;

    assert!(
        matches!(refused, Err(NotThisUser::Unidentified(_))),
        "{refused:?}"
    );
    closed_unread(client).await;
}

#[tokio::test]
async fn an_unmapped_peer_is_refused_even_by_a_host_that_is_unmapped_too() {
    let (refused, client, _dir) = verdict(Uid::OVERFLOW, |_| Ok(Uid::OVERFLOW)).await;

    assert!(
        matches!(refused, Err(NotThisUser::Unmapped { .. })),
        "{refused:?}"
    );
    closed_unread(client).await;
}

/// A process tree for the checks that read one: each pid's parent, and each pid's session.
struct Tree {
    parents: &'static [(u32, u32)],
    sessions: &'static [(u32, u32)],
}

impl Tree {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.parents
            .iter()
            .find(|(child, _)| *child == pid)
            .map(|(_, up)| *up)
    }

    fn session(&self, pid: u32) -> std::io::Result<u32> {
        self.sessions
            .iter()
            .find(|(of, _)| *of == pid)
            .map(|(_, sid)| *sid)
            .ok_or_else(|| std::io::Error::other("gone"))
    }

    fn inside(
        &self,
        peer: u32,
        programs: &[u32],
        holder: Option<u32>,
    ) -> std::io::Result<Option<u32>> {
        inside_of(
            peer,
            programs,
            holder,
            |pid| self.parent(pid),
            |pid| self.session(pid),
        )
    }
}

/// The window (100) was started by launchd (1) and leads its session; it started a session host
/// (101) and a chat's program (200), which leads its own session and ran a tool (201). A
/// terminal (300) the person opened is in a session of its own and ran `purlis attach` (301).
const IN_THE_WINDOW: Tree = Tree {
    parents: &[
        (100, 1),
        (101, 100),
        (200, 100),
        (201, 200),
        (300, 1),
        (301, 300),
    ],
    sessions: &[
        (100, 100),
        (101, 100),
        (200, 200),
        (201, 200),
        (300, 300),
        (301, 300),
    ],
};

#[test]
fn a_process_below_the_window_that_holds_the_chats_is_inside_one() {
    assert_eq!(
        IN_THE_WINDOW.inside(201, &[], Some(100)).unwrap(),
        Some(100)
    );
    assert_eq!(
        IN_THE_WINDOW.inside(200, &[], Some(100)).unwrap(),
        Some(100)
    );
}

#[test]
fn the_window_that_holds_the_chats_is_not_inside_one() {
    assert_eq!(IN_THE_WINDOW.inside(100, &[], Some(100)).unwrap(), None);
}

#[test]
fn a_process_the_person_started_outside_the_window_is_not_inside_a_chat() {
    assert_eq!(IN_THE_WINDOW.inside(301, &[], Some(100)).unwrap(), None);
}

#[test]
fn a_process_below_the_window_whose_ancestry_cannot_be_read_is_a_doubt() {
    const BROKEN: Tree = Tree {
        parents: &[(100, 1)],
        sessions: &[(100, 100), (201, 200)],
    };
    assert!(BROKEN.inside(201, &[], Some(100)).is_err());
}

#[test]
fn the_hosts_own_chats_are_still_checked_beside_the_window() {
    assert_eq!(
        IN_THE_WINDOW.inside(301, &[300], Some(100)).unwrap(),
        Some(300)
    );
}

#[test]
fn with_no_chats_and_no_window_holding_any_nothing_is_read() {
    let nothing = Tree {
        parents: &[],
        sessions: &[],
    };
    assert_eq!(nothing.inside(201, &[], None).unwrap(), None);
}
