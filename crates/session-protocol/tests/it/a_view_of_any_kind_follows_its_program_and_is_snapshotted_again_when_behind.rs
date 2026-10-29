// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The host holds hosted programs, each shown through views of a kind (ADR 0068, amended for
//! #1994). What a view does is written once for every kind: it opens with the program's
//! snapshot, follows what the program writes without a thread of its own, and when it falls
//! behind it is dropped and opened again from a fresh snapshot. A kind brings only how its
//! snapshot is made: here a transcript, as a terminal's, and a last frame, as a browser's.

use std::sync::Arc;
use std::time::Duration;

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link::{self, Link};
use purlis_session_protocol::version::{Speaks, Version};
use purlis_session_protocol::view::{Chunk, Epoch, Limits, Reader, Viewer, Views};
use tokio::io::duplex;
use tokio::time::timeout;

use crate::common;
use common::HELD;
use common::programs::{Program, Snapshot};

async fn linked() -> (Link, Link) {
    let speaks = Speaks::new([Version { major: 1, minor: 1 }]);
    let (a, b) = duplex(256 * 1024);
    let (client, host) = tokio::join!(
        link::connect(a, speaks.clone(), Scope::LocalUi, HELD.of(Scope::LocalUi)),
        link::serve_any(b, speaks, &HELD)
    );
    (client.unwrap(), host.unwrap())
}

/// The snapshot, read whole and acked, then live bytes until `want` of them have come.
async fn snapshot_then(reader: &mut Reader, want: usize) -> (Vec<u8>, Vec<u8>) {
    let (mut snapshot, mut live) = (Vec::new(), Vec::new());
    timeout(Duration::from_secs(10), async {
        while !reader.snapshot_complete() {
            let Chunk::Snapshot(bytes) = reader.next().await.unwrap().unwrap() else {
                panic!("live bytes before the snapshot was complete");
            };
            reader.ack(bytes.len()).await.unwrap();
            snapshot.extend_from_slice(&bytes);
        }
        while live.len() < want {
            let Chunk::Live(bytes) = reader.next().await.unwrap().unwrap() else {
                panic!("a snapshot after it was complete");
            };
            reader.ack(bytes.len()).await.unwrap();
            live.extend_from_slice(&bytes);
        }
    })
    .await
    .expect("the view delivers in time");
    (snapshot, live)
}

#[tokio::test]
async fn a_view_opens_with_the_snapshot_then_follows_what_the_program_writes() {
    for kind in [Snapshot::Transcript, Snapshot::Frame] {
        let (mut client, host) = linked().await;
        let program = Program::new(kind);
        program.write(b"one ");
        program.write(b"two ");
        let views = Views::new(host.opener(), Limits::default());
        let view = views.open(Arc::new(program.clone())).await.unwrap();
        program.write(b"three");

        let mut reader = Viewer::default()
            .accept(client.accept().await.unwrap())
            .await
            .unwrap();
        assert_eq!((reader.view(), reader.epoch()), (view, Epoch(1)));
        let (snapshot, live) = snapshot_then(&mut reader, b"three".len()).await;
        let expected: &[u8] = match kind {
            Snapshot::Transcript => b"one two ",
            Snapshot::Frame => b"two ",
        };
        assert_eq!(snapshot, expected, "{kind:?}");
        assert_eq!(live, b"three", "{kind:?}");
        assert_eq!(views.open_views(), 1);
    }
}

#[tokio::test]
async fn each_view_on_a_link_gets_a_number_of_its_own() {
    let (_client, host) = linked().await;
    let views = Views::new(host.opener(), Limits::default());
    let program = Arc::new(Program::new(Snapshot::Transcript));
    let first = views.open(program.clone()).await.unwrap();
    let second = views.open(program).await.unwrap();
    assert_ne!(first, second);
    assert_eq!(views.open_views(), 2);
}

#[tokio::test]
async fn a_view_that_falls_behind_is_dropped_and_snapshotted_again_whatever_its_kind() {
    for kind in [Snapshot::Transcript, Snapshot::Frame] {
        let (mut client, host) = linked().await;
        let program = Program::new(kind);
        let limits = Limits {
            most_queued_bytes: 32 * 1024,
            high_watermark: 8 * 1024,
            low_watermark: 2 * 1024,
        };
        let views = Views::new(host.opener(), limits);
        let view = views.open(Arc::new(program.clone())).await.unwrap();
        let mut viewer = Viewer::default();
        // The client takes the view and never draws it.
        let stale = viewer.accept(client.accept().await.unwrap()).await.unwrap();
        assert_eq!(stale.epoch(), Epoch(1));

        // The program writes far past what the view may hold.
        for n in 0..64 {
            program.write(&[b'0' + (n % 10) as u8; 1024]);
        }
        let again = timeout(Duration::from_secs(10), client.accept())
            .await
            .expect("the view is attached again")
            .unwrap();
        let mut reader = viewer.accept(again).await.unwrap();
        assert_eq!(
            (reader.view(), reader.epoch()),
            (view, Epoch(2)),
            "{kind:?}"
        );
        let fresh = program.snapshot_now();
        program.write(b"after");
        let (snapshot, live) = snapshot_then(&mut reader, b"after".len()).await;
        assert_eq!(snapshot, fresh, "{kind:?}: the program as it is now");
        assert_eq!(live, b"after", "{kind:?}");
        assert_eq!(program.watched_by(), 1, "{kind:?}: the dropped view let go");
    }
}

#[tokio::test]
async fn a_closed_view_lets_go_of_its_program_and_the_program_runs_on() {
    let (mut client, host) = linked().await;
    let program = Program::new(Snapshot::Transcript);
    let views = Views::new(host.opener(), Limits::default());
    let view = views.open(Arc::new(program.clone())).await.unwrap();
    let mut reader = Viewer::default()
        .accept(client.accept().await.unwrap())
        .await
        .unwrap();
    assert_eq!(program.watched_by(), 1);

    assert!(views.close(view));
    assert!(!views.close(view), "closed once");
    timeout(Duration::from_secs(5), async {
        while program.watched_by() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the program is let go of at once");
    let ended = timeout(Duration::from_secs(5), async {
        while let Some(Ok(_)) = reader.next().await {}
    })
    .await;
    assert!(ended.is_ok(), "the client's stream ends");
    program.write(b"still running");
    assert_eq!(program.snapshot_now(), b"still running");
    assert_eq!(views.open_views(), 0);
}

#[tokio::test]
async fn once_its_link_has_ended_every_view_is_closed_and_none_opens() {
    let (_client, host) = linked().await;
    let program = Program::new(Snapshot::Transcript);
    let views = Views::new(host.opener(), Limits::default());
    for _ in 0..3 {
        views.open(Arc::new(program.clone())).await.unwrap();
    }
    assert_eq!(program.watched_by(), 3);

    views.end();
    timeout(Duration::from_secs(5), async {
        while program.watched_by() > 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("every view lets go at once, with nothing written");
    assert_eq!(views.open_views(), 0);
    assert!(views.open(Arc::new(program.clone())).await.is_err());
    assert_eq!(
        program.watched_by(),
        0,
        "a view refused leaves nothing behind"
    );
}

/// A view whose client drew nothing is attached again only after a pause, and its fresh
/// snapshot is taken after that pause, not before it: so the client gets the program as it is
/// when the view comes back, and nothing piles up for it while it waits.
#[tokio::test]
async fn the_fresh_snapshot_is_taken_after_the_pause_before_a_reattach() {
    for kind in [Snapshot::Transcript, Snapshot::Frame] {
        let (mut client, host) = linked().await;
        let program = Program::new(kind);
        let limits = Limits {
            most_queued_bytes: 32 * 1024,
            high_watermark: 8 * 1024,
            low_watermark: 2 * 1024,
        };
        let views = Views::new(host.opener(), limits);
        views.open(Arc::new(program.clone())).await.unwrap();
        let mut viewer = Viewer::default();
        let _stale = viewer.accept(client.accept().await.unwrap()).await.unwrap();
        for n in 0..64 {
            program.write(&[b'0' + (n % 10) as u8; 1024]);
        }

        // While the view waits out its pause, nothing holds the program's live bytes.
        timeout(Duration::from_secs(5), async {
            while program.watched_by() > 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{kind:?}: the dropped view still holds the program"));
        program.write(b"during the pause");

        let again = timeout(Duration::from_secs(10), client.accept())
            .await
            .expect("the view is attached again")
            .unwrap();
        let mut reader = viewer.accept(again).await.unwrap();
        assert_eq!(reader.epoch(), Epoch(2));
        let fresh = program.snapshot_now();
        let (snapshot, _) = snapshot_then(&mut reader, 0).await;
        assert_eq!(
            snapshot, fresh,
            "{kind:?}: the snapshot is the program after the pause"
        );
        assert!(snapshot.ends_with(b"during the pause"), "{kind:?}");
    }
}
