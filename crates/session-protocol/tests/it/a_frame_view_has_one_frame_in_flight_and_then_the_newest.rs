// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A view of whole frames, a browser's, keeps one frame in flight (spec #2041, #2050): the host
//! writes a frame, then nothing more until the client has drawn all of it, and then the newest
//! frame there is, never the ones made meanwhile. So over a slow link a view never queues frames
//! ahead of what the person types, and what they see is never older than one frame.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::version::{Speaks, Version};
use purlis_session_protocol::view::{Attachment, Chunk, Limits, Reader, Source, Viewer, Views};
use tokio::io::duplex;
use tokio::sync::watch;
use tokio::time::timeout;

use crate::common;
use common::HELD;

async fn linked() -> (Link, Link) {
    let speaks = Speaks::new([Version { major: 1, minor: 6 }]);
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(a, speaks.clone(), Scope::LocalUi, HELD.of(Scope::LocalUi)),
        link::serve_any(b, speaks, &HELD)
    );
    (client.unwrap(), host.unwrap())
}

/// A program whose view is its latest frame, as a browser's is: each attach is the latest
/// frame, then each newer one.
#[derive(Clone)]
struct Latest(watch::Sender<Bytes>);

impl Latest {
    fn new() -> Self {
        Latest(watch::channel(Bytes::from_static(b"first")).0)
    }

    fn makes(&self, frame: &'static [u8]) {
        self.0.send_replace(Bytes::from_static(frame));
    }
}

impl Source for Latest {
    fn attach(&self) -> Attachment {
        let mut latest = self.0.subscribe();
        let snapshot = latest.borrow_and_update().clone();
        let live = futures::stream::unfold(latest, |mut latest| async move {
            latest.changed().await.ok()?;
            let frame = latest.borrow_and_update().clone();
            Some((frame, latest))
        });
        Attachment {
            snapshot,
            live: live.boxed(),
        }
    }

    fn one_frame_in_flight(&self) -> bool {
        true
    }
}

/// What the view sends within `within`, read whole, acknowledging nothing.
async fn read_unacknowledged(reader: &mut Reader, within: Duration) -> Vec<u8> {
    let mut got = Vec::new();
    while let Ok(Some(chunk)) = timeout(within, reader.next()).await {
        match chunk.unwrap() {
            Chunk::Snapshot(bytes) | Chunk::Live(bytes) => got.extend_from_slice(&bytes),
        }
    }
    got
}

#[tokio::test]
async fn a_frame_drawn_by_nobody_holds_back_every_frame_made_after_it() {
    let (mut client, host) = linked().await;
    let program = Latest::new();
    let views = Views::new(host.opener(), Limits::default());
    views.open(Arc::new(program.clone())).await.unwrap();
    let mut reader = Viewer::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();

    program.makes(b"second");
    program.makes(b"third");
    let sent = read_unacknowledged(&mut reader, Duration::from_millis(300)).await;

    assert_eq!(
        sent, b"first",
        "only the frame in flight was sent, the snapshot"
    );
}

#[tokio::test]
async fn once_the_frame_in_flight_is_drawn_the_newest_frame_comes_and_none_between() {
    let (mut client, host) = linked().await;
    let program = Latest::new();
    let views = Views::new(host.opener(), Limits::default());
    views.open(Arc::new(program.clone())).await.unwrap();
    let mut reader = Viewer::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    let snapshot = read_unacknowledged(&mut reader, Duration::from_millis(200)).await;
    assert_eq!(snapshot, b"first");

    program.makes(b"second");
    program.makes(b"third");
    tokio::time::sleep(Duration::from_millis(100)).await;
    reader.ack(snapshot.len()).await.unwrap();
    let next = read_unacknowledged(&mut reader, Duration::from_millis(300)).await;

    assert_eq!(next, b"third", "the newest frame, and nothing made between");
}

#[tokio::test]
async fn a_view_of_a_program_that_does_not_ask_for_it_keeps_sending_up_to_its_watermark() {
    /// The same program, without one frame in flight: what a terminal's view does.
    #[derive(Clone)]
    struct Streaming(Latest);
    impl Source for Streaming {
        fn attach(&self) -> Attachment {
            self.0.attach()
        }
    }
    let (mut client, host) = linked().await;
    let program = Latest::new();
    let views = Views::new(host.opener(), Limits::default());
    views
        .open(Arc::new(Streaming(program.clone())))
        .await
        .unwrap();
    let mut reader = Viewer::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    // Read before the next is made, so the latest-only stream yields each.
    let first = read_unacknowledged(&mut reader, Duration::from_millis(200)).await;
    program.makes(b"second");
    let then = read_unacknowledged(&mut reader, Duration::from_millis(200)).await;

    assert_eq!(first, b"first");
    assert_eq!(then, b"second", "sent with nothing drawn yet");
}
