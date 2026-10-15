// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A program that stands in for a terminal's, for tests of what drives the host's programs: it
//! echoes what is typed, keeps what it was asked, views like a terminal (its snapshot is all it
//! wrote) and ends when stopped. Only tests build it (`test-support`).

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::session::{Exit, Readiness};
use purlis_session_protocol::view::{Attachment, Source};
use purlis_terminal::engine::Size;
use purlis_terminal::session::Spec;
use tokio::sync::mpsc;

use crate::hosting::{Run, Running};

/// What a stand-in program was asked, and what it wrote.
#[derive(Default)]
pub struct Seen {
    pub typed: Vec<u8>,
    pub sizes: Vec<Size>,
    pub stopped: bool,
    pub written: Vec<u8>,
    watching: Vec<mpsc::UnboundedSender<Bytes>>,
    ends: Option<Box<dyn FnOnce(Exit) + Send>>,
}

/// A program that echoes what is typed to it, and ends with code 0 when stopped.
#[derive(Clone, Default)]
pub struct StandIn(Arc<Mutex<Seen>>);

impl StandIn {
    /// What it was asked, and what it wrote.
    pub fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.0.lock().unwrap()
    }

    /// It writes `bytes`, to its next snapshot and every view.
    pub fn writes(&self, bytes: &[u8]) {
        let mut seen = self.seen();
        seen.written.extend_from_slice(bytes);
        let bytes = Bytes::copy_from_slice(bytes);
        seen.watching
            .retain(|view| view.send(bytes.clone()).is_ok());
    }

    /// It ends with `exit`: its views' live bytes end, and whoever asked is told.
    pub fn ends(&self, exit: Exit) {
        let tell = {
            let mut seen = self.seen();
            seen.watching.clear();
            seen.ends.take()
        };
        if let Some(tell) = tell {
            tell(exit);
        }
    }
}

impl Source for StandIn {
    fn attach(&self) -> Attachment {
        let mut seen = self.seen();
        let (live, mut follows) = mpsc::unbounded_channel();
        seen.watching.push(live);
        Attachment {
            snapshot: Bytes::from(seen.written.clone()),
            live: futures::stream::poll_fn(move |cx| follows.poll_recv(cx)).boxed(),
        }
    }
}

impl Running for StandIn {
    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.seen().typed.extend_from_slice(bytes);
        self.writes(bytes);
        Ok(())
    }

    fn resize(&self, size: Size) -> Result<(), String> {
        self.seen().sizes.push(size);
        Ok(())
    }

    fn source(&self) -> Arc<dyn Source> {
        Arc::new(self.clone())
    }

    fn stop(&self) {
        self.seen().stopped = true;
        self.ends(Exit::Signal("Interrupt".into()));
    }

    fn process(&self) -> Option<u32> {
        Some(4242)
    }

    fn readiness(&self) -> Readiness {
        Readiness {
            edits_lines: Some(false),
            quiet_for_ms: 1500,
        }
    }

    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        self.seen().ends = Some(tell);
    }
}

/// Runs every start as a stand-in, and keeps each, with the spec it was given.
#[derive(Clone, Default)]
pub struct StandIns(Arc<Mutex<Vec<(Spec, StandIn)>>>);

impl StandIns {
    /// Every start it ran, with its spec, in order.
    pub fn ran(&self) -> Vec<(Spec, StandIn)> {
        self.0.lock().unwrap().clone()
    }

    /// The last start it ran.
    pub fn last(&self) -> StandIn {
        self.ran().last().expect("something ran").1.clone()
    }
}

impl Run for StandIns {
    fn run(&self, spec: Spec) -> Result<Arc<dyn Running>, String> {
        let program = StandIn::default();
        self.0.lock().unwrap().push((spec, program.clone()));
        Ok(Arc::new(program))
    }
}
