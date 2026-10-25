// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A keystroke into a runner's browser is never queued behind its frames (#2050, spec #2041
//! story 32): the person's input rides the control lane, which goes ahead of every view's bytes
//! waiting to be sent, and the view keeps one frame in flight, so what waits ahead of a key is
//! at most what the link already holds.
//!
//! **The link** is `common::netsim` in virtual time, shaped as a connector's stdio to a remote
//! runner: 40 ms round trip, 2 MB/s, a pipe's 64 KiB between the ends. **The browser** makes a
//! new 150 KB frame as fast as the view takes one, as a busy page at 1280 pixels wide does, and
//! the window draws each as it reads it. **The budget** is the round trip, plus what the link
//! holds ahead of the answer at its rate, plus a millisecond: the measured worst is printed
//! beside it.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::browsing::{Aim, Input, KeyAction};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Attached, Client, Command, Host, Peer, Refusal, ViewKind, ViewRole,
};
use purlis_session_protocol::view::{Attachment, Chunk, Source, Viewer};
use tokio::sync::watch;
use tokio::time::{Instant, sleep};

mod common;
use common::HELD;
use common::netsim::{self, END_PIPE, Shape};

const SHAPE: Shape = Shape {
    one_way: Duration::from_millis(20),
    bytes_per_second: 2e6,
    buffer: 64 * 1024,
    loss_one_in: 0,
    packet: 1460,
    retransmit: Duration::from_millis(40),
};

/// A busy page's frame at 1280 pixels wide, as JPEG at the screencast's quality.
const FRAME_BYTES: usize = 150 * 1024;

/// The largest frame the multiplexer sends, with its header.
const YAMUX_FRAME: usize = 16 * 1024 + 12;

const KEYS: usize = 20;

/// A browser whose page is always changing: a new frame every time the view takes one.
#[derive(Clone)]
struct BusyPage(watch::Sender<Bytes>);

impl Source for BusyPage {
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

/// A runner's host with one busy browser: it opens views of it and takes the person's keys.
#[derive(Clone)]
struct Runner(BusyPage);

impl Host for Runner {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::AttachView { .. } => {
                let view = peer
                    .views()
                    .open(Arc::new(self.0.clone()))
                    .await
                    .map_err(|why| Refusal {
                        code: session::NOT_STARTED.into(),
                        why: why.to_string(),
                    })?;
                Ok(Answer::Attached(Attached {
                    view: view.0,
                    cols: 1280,
                    rows: 800,
                    kind: Some(ViewKind::Browser),
                }))
            }
            _ => Ok(Answer::Done),
        }
    }
}

fn a_key() -> Input {
    Input::Key {
        aim: Aim {
            page: "p1".into(),
            document: 1,
        },
        action: KeyAction::Down,
        key: "a".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 0,
    }
}

#[tokio::test(start_paused = true)]
async fn a_keystrokes_round_trip_behind_a_busy_pages_frames_stays_within_the_links_budget() {
    let (client_end, host_end) = netsim::link(SHAPE, 11);
    let (client, host) = tokio::join!(
        link::connect(
            client_end,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(host_end, session::speaks(), &HELD)
    );
    let (page, _) = watch::channel(Bytes::from(vec![0u8; FRAME_BYTES]));
    let busy = BusyPage(page.clone());
    tokio::spawn(session::serve(host.unwrap(), Runner(busy), None));
    let (client, mut views, _events) = Client::new(client.unwrap());

    // The page changes all the time.
    let painting = tokio::spawn(async move {
        let mut n = 0u8;
        loop {
            n = n.wrapping_add(1);
            page.send_replace(Bytes::from(vec![n; FRAME_BYTES]));
            sleep(Duration::from_millis(5)).await;
        }
    });
    client
        .attach_browser_view("b1", ViewRole::Focused, None)
        .await
        .expect("the view attaches");
    let mut reader = Viewer::default()
        .accept(views.accept().await.unwrap())
        .await
        .unwrap();
    let drawn = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let drawing = {
        let drawn = Arc::clone(&drawn);
        tokio::spawn(async move {
            while let Some(Ok(Chunk::Snapshot(bytes) | Chunk::Live(bytes))) = reader.next().await
            {
                drawn.fetch_add(bytes.len(), std::sync::atomic::Ordering::Relaxed);
                if reader.ack(bytes.len()).await.is_err() {
                    return;
                }
            }
        })
    };
    sleep(Duration::from_secs(2)).await;

    let mut took = Vec::with_capacity(KEYS);
    for _ in 0..KEYS {
        let sent = Instant::now();
        client
            .browser_input("b1", a_key())
            .await
            .expect("the key is taken");
        took.push(sent.elapsed());
        sleep(Duration::from_millis(97)).await;
    }
    took.sort();
    let worst = took[KEYS - 1];
    // Ahead of the answer, at most: what the link and each end's pipe hold, and two of the
    // multiplexer's frames (the one being written and the answer's own).
    let ahead = SHAPE.buffer + 2 * END_PIPE + 2 * YAMUX_FRAME;
    let budget = 2 * SHAPE.one_way
        + Duration::from_secs_f64(ahead as f64 / SHAPE.bytes_per_second)
        + Duration::from_millis(1);
    let drawn = drawn.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!(
        "a key into a busy remote browser at 2 MB/s, 40 ms round trip: p50 {:?}, worst \
         {worst:?}, budget {budget:?}; {drawn} frame bytes drawn",
        took[KEYS / 2]
    );
    assert!(
        drawn > 4 * FRAME_BYTES,
        "frames kept coming while the keys were typed"
    );
    assert!(
        worst <= budget,
        "a key's round trip took {worst:?}, over the {budget:?} the link accounts for"
    );
    painting.abort();
    drawing.abort();
}
