// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's link socket closes a connection from inside one of its chats before reading a
//! byte of it (#2044): the bridge a connector runs is never inside a chat, and a chat never
//! links as a device. The connection here says nothing at all, so a host that read before it
//! refused would wait out the handshake's deadline instead of answering at once.
//!
//! These bind a socket, which a sandboxed agent cannot: they first run on CI.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, LinkRefused, Liveness, Pinned,
};
use purlis_session_protocol::local::{Chats, Listener};
use tokio::io::AsyncReadExt;
use tokio::net::{UnixListener, UnixStream};

/// The chats a host holds, by their programs' pids.
struct Held(Vec<u32>);

impl Chats for Held {
    fn programs(&self) -> Vec<u32> {
        self.0.clone()
    }
}

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

#[tokio::test]
async fn a_connection_from_a_chats_program_is_refused_without_a_byte_read_or_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("purlisd.link.sock");
    let listener = Listener::new(UnixListener::bind(&path).unwrap());
    let mut client = UnixStream::connect(&path).await.unwrap();
    let peer = listener.accept().await.unwrap().expect("this user");

    // The connecting process is this one, held here as a chat's program.
    let chats = Arc::new(Held(vec![std::process::id()]));
    let runner = identity("01JB0RUNNER0000000000000000");
    let desktop = identity("01JA0DESKT0P00000000000000");
    let desktop = Pinned {
        device: desktop.device.clone(),
        key: desktop.key.public(),
    };
    let answered = tokio::time::timeout(
        Duration::from_secs(2),
        hostlink::answer_local(
            peer,
            &chats,
            Handshake::THIS_BUILD,
            &runner,
            &desktop,
            Liveness::default(),
        ),
    )
    .await
    .expect("refused at once, with nothing read");
    assert!(
        matches!(answered, Err(LinkRefused::InsideAChat(_))),
        "{:?}",
        answered.err()
    );

    // The host wrote nothing either: the connection is closed with no hello.
    let mut said = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(2), client.read_to_end(&mut said))
        .await
        .expect("closed");
    assert!(read.is_ok() && said.is_empty(), "{said:?}");
}
