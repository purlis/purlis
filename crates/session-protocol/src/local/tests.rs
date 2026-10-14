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
