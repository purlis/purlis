// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Two hosted programs that stand in for real ones, one per way a view's snapshot is made: a
//! **transcript**, whose snapshot is everything it ever wrote, as a terminal's screen and
//! scrollback are; and a **frame** program, whose snapshot is only the last thing it wrote, as a
//! browser's latest frame is. Neither needs a terminal or a browser, so the views' machinery is
//! tested apart from both.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::view::{Attachment, Source};
use tokio::sync::mpsc;

#[derive(Default)]
struct Written {
    /// Everything written, in order.
    all: Vec<u8>,
    /// The last thing written.
    last: Bytes,
    /// Each attachment's live bytes, while its view holds them.
    watching: Vec<mpsc::UnboundedSender<Bytes>>,
}

/// How a program's snapshot is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snapshot {
    /// Everything it ever wrote.
    Transcript,
    /// The last thing it wrote.
    Frame,
}

/// A program the test writes for, which any number of views may show.
#[derive(Clone)]
pub struct Program {
    snapshot: Snapshot,
    written: Arc<Mutex<Written>>,
}

impl Program {
    pub fn new(snapshot: Snapshot) -> Program {
        Program {
            snapshot,
            written: Arc::default(),
        }
    }

    /// The program writes `bytes`: kept for the next snapshot, and sent to every view on it.
    pub fn write(&self, bytes: &[u8]) {
        let mut written = self.written.lock().unwrap();
        written.all.extend_from_slice(bytes);
        written.last = Bytes::copy_from_slice(bytes);
        let bytes = Bytes::copy_from_slice(bytes);
        written
            .watching
            .retain(|view| view.send(bytes.clone()).is_ok());
    }

    /// How many views still hold this program's live bytes.
    pub fn watched_by(&self) -> usize {
        let mut written = self.written.lock().unwrap();
        written.watching.retain(|view| !view.is_closed());
        written.watching.len()
    }

    /// The snapshot a view attached now would get.
    pub fn snapshot_now(&self) -> Vec<u8> {
        let written = self.written.lock().unwrap();
        match self.snapshot {
            Snapshot::Transcript => written.all.clone(),
            Snapshot::Frame => written.last.to_vec(),
        }
    }
}

impl Source for Program {
    fn attach(&self) -> Attachment {
        // One lock takes the snapshot and starts the live bytes after it, so nothing written
        // is lost or doubled between them.
        let mut written = self.written.lock().unwrap();
        let snapshot = match self.snapshot {
            Snapshot::Transcript => Bytes::from(written.all.clone()),
            Snapshot::Frame => written.last.clone(),
        };
        let (live, mut follows) = mpsc::unbounded_channel();
        written.watching.push(live);
        Attachment {
            snapshot,
            live: futures::stream::poll_fn(move |cx| follows.poll_recv(cx)).boxed(),
        }
    }
}
