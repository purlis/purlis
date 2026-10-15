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

use purlis_session_protocol::session::{
    Confine, Confined, Listed, ReachChange, ReachNow, SandboxTold,
};

use crate::confining::{Askable, Confining, Tell};
use crate::hosting::{Run, Running};

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

/// What a stand-in confinement was planned with and asked, and whether it has ended (#1999).
pub struct Confinement {
    pub confine: Confine,
    pub changes: Vec<ReachChange>,
    pub ended: bool,
    tell: Tell,
    askable: Askable,
}

/// Starts every confinement as a stand-in that listens on nothing: it says it is at the ports
/// it is told to, keeps every change the window makes, and tells the window what a test says its
/// proxy did (#1999). Only tests build it (`test-support`).
#[derive(Clone, Default)]
pub struct StandInConfines(
    Arc<Mutex<Vec<Confinement>>>,
    Arc<Mutex<Option<String>>>,
    Arc<Gate>,
);

/// What holds a stand-in's starts until a test lets them go, and what it refuses.
#[derive(Default)]
struct Gate {
    held: Mutex<Held>,
    changed: std::sync::Condvar,
}

#[derive(Default)]
struct Held {
    holding: bool,
    waiting: usize,
    refusing_changes: Option<String>,
    /// How long each takes to end, as a proxy waits on its listeners.
    ending_takes: std::time::Duration,
}

impl StandInConfines {
    /// Each confinement it started, in order.
    pub fn seen(&self) -> std::sync::MutexGuard<'_, Vec<Confinement>> {
        self.0.lock().unwrap()
    }

    /// The confinement it started as `id`.
    fn with<T>(&self, id: &str, what: impl FnOnce(&mut Confinement) -> T) -> T {
        let mut seen = self.seen();
        what(
            seen.iter_mut()
                .find(|one| one.confine.confinement == id)
                .expect("a confinement it started"),
        )
    }

    /// Its proxy did `told`: the window is told as the host tells it.
    pub fn proxy_did(&self, id: &str, told: SandboxTold) {
        let tell = self.with(id, |one| Arc::clone(&one.tell));
        tell(told);
    }

    /// Whether its proxy may hold a connection while the person is asked, now.
    pub fn askable(&self, id: &str) -> bool {
        let askable = self.with(id, |one| Arc::clone(&one.askable));
        askable()
    }

    /// Whether it has ended.
    pub fn ended(&self, id: &str) -> bool {
        self.with(id, |one| one.ended)
    }

    /// It refuses every start from now on, saying `why`.
    pub fn refuse(&self, why: &str) {
        *self.1.lock().unwrap() = Some(why.to_owned());
    }

    /// Each start from now on waits, before it starts anything, until [`Self::let_starts_go`].
    pub fn hold_starts(&self) {
        self.2.held.lock().unwrap().holding = true;
    }

    /// How many starts wait now.
    pub fn waiting(&self) -> usize {
        self.2.held.lock().unwrap().waiting
    }

    /// The starts that wait go on.
    pub fn let_starts_go(&self) {
        self.2.held.lock().unwrap().holding = false;
        self.2.changed.notify_all();
    }

    /// Each takes `long` to end from now on, as a proxy's end waits on its listeners.
    pub fn take_to_end(&self, long: std::time::Duration) {
        self.2.held.lock().unwrap().ending_takes = long;
    }

    /// It refuses every change the window makes from now on, saying `why`.
    pub fn refuse_changes(&self, why: &str) {
        self.2.held.lock().unwrap().refusing_changes = Some(why.to_owned());
    }
}

struct StandInConfining {
    id: String,
    all: StandInConfines,
}

impl crate::confining::Confine for StandInConfines {
    fn confine(
        &self,
        confining: &crate::confining::ToConfine<'_>,
        tell: Tell,
        askable: Askable,
    ) -> Result<Box<dyn Confining>, String> {
        if let Some(why) = self.1.lock().unwrap().clone() {
            return Err(why);
        }
        {
            let mut held = self.2.held.lock().unwrap();
            held.waiting += 1;
            while held.holding {
                held = self.2.changed.wait(held).unwrap();
            }
            held.waiting -= 1;
        }
        self.seen().push(Confinement {
            confine: confining.confine.clone(),
            changes: Vec::new(),
            ended: false,
            tell,
            askable,
        });
        Ok(Box::new(StandInConfining {
            id: confining.confine.confinement.clone(),
            all: self.clone(),
        }))
    }
}

impl Confining for StandInConfining {
    fn confined(&self) -> Confined {
        let given = self
            .all
            .with(&self.id, |one| one.confine.plan.given.clone());
        Confined {
            confinement: self.id.clone(),
            http: 50811,
            socks: 50812,
            tmp: Some("/tmp/charter-chat-standin".into()),
            routed: None,
            ssh: given.first().map(|port| format!("/tmp/given-{port}")),
        }
    }

    fn change(&self, change: &ReachChange) -> Result<Vec<String>, String> {
        if let Some(why) = self.all.2.held.lock().unwrap().refusing_changes.clone() {
            return Err(why);
        }
        self.all
            .with(&self.id, |one| one.changes.push(change.clone()));
        Ok(match change {
            ReachChange::Allow { host, .. } => vec![format!("{host}:443")],
            _ => Vec::new(),
        })
    }

    fn now(&self) -> ReachNow {
        self.all.with(&self.id, |one| ReachNow {
            asked: Vec::new(),
            allowed_live: one
                .changes
                .iter()
                .filter_map(|change| match change {
                    ReachChange::Allow { host, layer } => Some(Listed {
                        host: host.clone(),
                        layer: *layer,
                    }),
                    _ => None,
                })
                .collect(),
        })
    }
}

impl Drop for StandInConfining {
    fn drop(&mut self) {
        let takes = self.all.2.held.lock().unwrap().ending_takes;
        std::thread::sleep(takes);
        self.all.with(&self.id, |one| one.ended = true);
    }
}

/// A browser that stands in for an engine's, for tests of what the host does with browsers: it
/// makes a frame when the test says so, and only while the host has asked it to show frames;
/// it keeps what it was asked.
#[derive(Clone, Default)]
pub struct StandInScreen(Arc<Mutex<Shown>>);

/// What a stand-in browser was asked.
#[derive(Default)]
pub struct Shown {
    /// The folder it was started with.
    pub data: std::path::PathBuf,
    /// Whether the host asks it for frames now.
    pub showing: bool,
    /// Each time the host asked it for frames, and each time it stopped asking.
    pub shows: u32,
    pub hides: u32,
    /// The least time between frames the host last asked for.
    pub every: Option<std::time::Duration>,
    /// Frames it made: only while showing.
    pub made: u32,
    pub stopped: bool,
    sink: Option<crate::browsers::FrameSink>,
    ends: Option<Box<dyn FnOnce(Exit) + Send>>,
}

impl StandInScreen {
    pub fn shown(&self) -> std::sync::MutexGuard<'_, Shown> {
        self.0.lock().unwrap()
    }

    /// It makes `image` as its next frame, when the host is asking for frames.
    pub fn makes(&self, image: &[u8]) {
        let sink = {
            let mut shown = self.shown();
            if !shown.showing {
                return;
            }
            shown.made += 1;
            shown.sink.clone()
        };
        if let Some(sink) = sink {
            sink(Bytes::copy_from_slice(image));
        }
    }

    /// The engine ended by itself.
    pub fn ends(&self, exit: Exit) {
        let tell = self.shown().ends.take();
        if let Some(tell) = tell {
            tell(exit);
        }
    }
}

impl crate::browsers::Screen for StandInScreen {
    fn show(&self, frames: crate::browsers::FrameSink) {
        let mut shown = self.shown();
        shown.showing = true;
        shown.shows += 1;
        shown.sink = Some(frames);
    }

    fn pace(&self, every: std::time::Duration) {
        self.shown().every = Some(every);
    }

    fn hide(&self) {
        let mut shown = self.shown();
        shown.showing = false;
        shown.hides += 1;
        shown.sink = None;
    }

    fn size(&self) -> (u16, u16) {
        (1280, 800)
    }

    fn process(&self) -> Option<u32> {
        Some(5151)
    }

    fn stop(&self) {
        {
            let mut shown = self.shown();
            shown.stopped = true;
            shown.showing = false;
            shown.sink = None;
        }
        self.ends(Exit::Code(0));
    }

    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        self.shown().ends = Some(tell);
    }
}

/// Starts every browser as a stand-in, each with its data folder in one folder, and keeps each.
#[derive(Clone)]
pub struct StandInBrowsers {
    folder: std::path::PathBuf,
    started: Arc<Mutex<Vec<StandInScreen>>>,
}

impl StandInBrowsers {
    /// Browsers whose data folders are made in `folder`.
    pub fn in_folder(folder: impl Into<std::path::PathBuf>) -> Self {
        StandInBrowsers {
            folder: folder.into(),
            started: Arc::default(),
        }
    }

    /// Every browser it started, in order.
    pub fn started(&self) -> Vec<StandInScreen> {
        self.started.lock().unwrap().clone()
    }
}

impl crate::browsers::Browsers for StandInBrowsers {
    fn data_folders(&self) -> Result<std::path::PathBuf, String> {
        Ok(self.folder.clone())
    }

    fn start(&self, data: &std::path::Path) -> Result<Arc<dyn crate::browsers::Screen>, String> {
        std::fs::create_dir_all(data).map_err(|why| why.to_string())?;
        let screen = StandInScreen::default();
        screen.shown().data = data.to_path_buf();
        self.started.lock().unwrap().push(screen.clone());
        Ok(Arc::new(screen))
    }
}
