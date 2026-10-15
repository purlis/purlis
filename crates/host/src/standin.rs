// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A program that stands in for a terminal's, for tests of what drives the host's programs: it
//! echoes what is typed, keeps what it was asked, views like a terminal (its snapshot is all it
//! wrote) and ends when stopped. And a saver that stands in for the binary's ([`Savers`]): it
//! keeps what it was asked and saves nothing. Only tests build it (`test-support`).

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::session::{Exit, Readiness};
use purlis_session_protocol::view::{Attachment, Source};
use purlis_terminal::engine::Size;
use purlis_terminal::session::Spec;
use tokio::sync::mpsc;

use crate::hosting::{Looked, Poke, Run, Running, Save, Saving, ToSave, Working};

/// What a stand-in program was asked, and what it wrote.
#[derive(Default)]
pub struct Seen {
    /// It ignores being stopped: it ends only when told to ([`StandIn::ends`]).
    pub holds_on: bool,
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
        let holds_on = {
            let mut seen = self.seen();
            seen.stopped = true;
            seen.holds_on
        };
        if !holds_on {
            self.ends(Exit::Signal("Interrupt".into()));
        }
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
pub struct StandIns(Arc<Mutex<Vec<(Spec, StandIn)>>>, Arc<Mutex<bool>>);

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

impl StandIns {
    /// The programs it runs from now on ignore being stopped ([`Seen::holds_on`]).
    pub fn hold_on(&self) {
        *self.1.lock().unwrap() = true;
    }
}

impl Run for StandIns {
    fn run(&self, spec: Spec) -> Result<Arc<dyn Running>, String> {
        let program = StandIn::default();
        program.seen().holds_on = *self.1.lock().unwrap();
        self.0.lock().unwrap().push((spec, program.clone()));
        Ok(Arc::new(program))
    }
}

/// Stands in for a binary's saver: keeps each project it was asked to save, what it was poked
/// with, and whether its saving still runs.
#[derive(Clone, Default)]
pub struct Savers(Arc<Mutex<Saved>>);

#[derive(Default)]
struct Saved {
    started: Vec<(String, std::path::PathBuf)>,
    running: usize,
    poked: Vec<Poke>,
    told: Vec<Arc<dyn Fn(Looked) + Send + Sync>>,
    working: Vec<Arc<dyn Fn() -> Vec<Working> + Send + Sync>>,
}

impl Savers {
    /// Each project it was asked to save, by its id and path, in order.
    pub fn started(&self) -> Vec<(String, std::path::PathBuf)> {
        self.0.lock().unwrap().started.clone()
    }

    /// How many of its savings still run.
    pub fn running(&self) -> usize {
        self.0.lock().unwrap().running
    }

    /// What its savings were poked with, in order.
    pub fn poked(&self) -> Vec<Poke> {
        self.0.lock().unwrap().poked.clone()
    }

    /// Its last saving looks, and did `looked`.
    pub fn looks(&self, looked: Looked) {
        let told = self.0.lock().unwrap().told.last().cloned();
        if let Some(told) = told {
            told(looked);
        }
    }

    /// Who its last saving is told works in its project.
    pub fn working(&self) -> Vec<Working> {
        let working = self.0.lock().unwrap().working.last().cloned();
        working.map(|working| working()).unwrap_or_default()
    }
}

impl Save for Savers {
    fn save(&self, saving: ToSave) -> Result<Box<dyn Saving>, String> {
        let mut saved = self.0.lock().unwrap();
        saved.started.push((saving.project, saving.project_path));
        saved.running += 1;
        saved.told.push(Arc::from(saving.told));
        saved.working.push(Arc::from(saving.working));
        Ok(Box::new(StoodIn(Arc::clone(&self.0))))
    }
}

struct StoodIn(Arc<Mutex<Saved>>);

impl Saving for StoodIn {
    fn poke(&self, poke: Poke) {
        self.0.lock().unwrap().poked.push(poke);
    }
}

impl Drop for StoodIn {
    fn drop(&mut self) {
        self.0.lock().unwrap().running -= 1;
    }
}
