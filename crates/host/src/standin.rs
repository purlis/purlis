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

use purlis_session_protocol::session::{
    Confine, Confined, Listed, ReachChange, ReachNow, SandboxTold,
};

use crate::confining::{Askable, Confining, Tell};
use crate::hosting::{Looked, Poke, Run, Running, Save, Saving, ToSave, Working};
use crate::hostrecord::Started;

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

    /// A stand-in's own word, never the kernel's: its process is no real one.
    fn process_started(&self) -> Option<String> {
        Some("a stand-in's start".to_owned())
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

/// Runs every start as a stand-in, and keeps each, with the spec it was given, and each program
/// the last host left that it was asked to end.
#[derive(Clone, Default)]
pub struct StandIns {
    /// Every start it ran, with its spec, in order.
    ran: Arc<Mutex<Vec<(Spec, StandIn)>>>,
    /// Whether the programs it runs from now on ignore being stopped.
    hold_on: Arc<Mutex<bool>>,
    /// Each program the last host left running that it was asked to end, in order.
    ended_left: Arc<Mutex<Vec<Started>>>,
}

impl StandIns {
    /// Every start it ran, with its spec, in order.
    pub fn ran(&self) -> Vec<(Spec, StandIn)> {
        self.ran.lock().unwrap().clone()
    }

    /// The last start it ran.
    pub fn last(&self) -> StandIn {
        self.ran().last().expect("something ran").1.clone()
    }

    /// Each program the last host left running that it was asked to end, in order.
    pub fn ended_left(&self) -> Vec<Started> {
        self.ended_left.lock().unwrap().clone()
    }
}

impl StandIns {
    /// The programs it runs from now on ignore being stopped ([`Seen::holds_on`]).
    pub fn hold_on(&self) {
        *self.hold_on.lock().unwrap() = true;
    }
}

impl Run for StandIns {
    fn run(&self, spec: Spec) -> Result<Arc<dyn Running>, String> {
        let program = StandIn::default();
        program.seen().holds_on = *self.hold_on.lock().unwrap();
        self.ran.lock().unwrap().push((spec, program.clone()));
        Ok(Arc::new(program))
    }

    fn end_what_was_left(&self, left: &Started) {
        self.ended_left.lock().unwrap().push(left.clone());
    }
}

/// Stands in for a binary's saver: keeps each project it was asked to save, what it was poked
/// with, and whether its saving still runs.
#[derive(Clone, Default)]
pub struct Savers(Arc<Mutex<Saved>>, Arc<std::sync::atomic::AtomicBool>);

#[derive(Default)]
struct Saved {
    started: Vec<(String, std::path::PathBuf)>,
    running: usize,
    poked: Vec<Poke>,
    told: Vec<Arc<dyn Fn(Looked) + Send + Sync>>,
    working: Vec<Arc<dyn Fn() -> Vec<Working> + Send + Sync>>,
    asks: Vec<Arc<crate::hosting::AskTheWindow>>,
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

    /// Its saving of the project at `path` looks, and did `looked`.
    pub fn looks_at(&self, path: &std::path::Path, looked: Looked) {
        let told = {
            let saved = self.0.lock().unwrap();
            saved
                .started
                .iter()
                .rposition(|(_, at)| at == path)
                .and_then(|which| saved.told.get(which).cloned())
        };
        told.expect("a saving of that path")(looked);
    }

    /// Who its last saving is told works in its project.
    pub fn working(&self) -> Vec<Working> {
        let working = self.0.lock().unwrap().working.last().cloned();
        working.map(|working| working()).unwrap_or_default()
    }
}

impl Savers {
    /// Its last saving asks the window who is mid-turn in `workspace`, and waits.
    pub fn asks_the_window(&self, workspace: &str) -> Option<Vec<String>> {
        let ask = self.0.lock().unwrap().asks.last().cloned();
        ask.and_then(|ask| ask(workspace))
    }
}

impl Save for Savers {
    fn save(&self, saving: ToSave) -> Result<Box<dyn Saving>, String> {
        let mut saved = self.0.lock().unwrap();
        saved.started.push((saving.project, saving.project_path));
        saved.running += 1;
        saved.told.push(Arc::from(saving.told));
        saved.working.push(Arc::from(saving.working));
        saved.asks.push(Arc::from(saving.ask_the_window));
        Ok(Box::new(StoodIn(Arc::clone(&self.0), Arc::clone(&self.1))))
    }
}

impl Savers {
    /// From now on a saving that is stopped is in the middle of a save, and lets go only once
    /// [`Self::finish_saving`] says so.
    pub fn mid_save(&self) {
        self.1.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// The save a stopped saving was in the middle of is done.
    pub fn finish_saving(&self) {
        self.1.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

struct StoodIn(Arc<Mutex<Saved>>, Arc<std::sync::atomic::AtomicBool>);

impl Saving for StoodIn {
    fn poke(&self, poke: Poke) {
        self.0.lock().unwrap().poked.push(poke);
    }
}

impl Drop for StoodIn {
    fn drop(&mut self) {
        while self.1.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.0.lock().unwrap().running -= 1;
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
    /// What it was started reaching (#2013).
    pub reach: Option<purlis_session_protocol::browsing::BrowserReach>,
    /// Each act it was asked, with its id, in order (#2013).
    pub acts: Vec<(String, purlis_session_protocol::browsing::Act)>,
    /// The epoch each act was made under, in order (#2014).
    pub epochs: Vec<u64>,
    /// Whether the person drives it now, and its lease's epoch (#2014).
    pub taken_over: bool,
    pub epoch: u64,
    /// Each event of the person's it took, in order (#2014).
    pub inputs: Vec<purlis_session_protocol::browsing::Input>,
    /// The document its page shows: input aimed at another is missed, and each frame names it.
    pub document: u64,
    /// Its pages' size.
    pub size: (u16, u16),
    /// What it answers its next acts with, in order; once none is left, a settled page.
    answers: std::collections::VecDeque<Result<purlis_session_protocol::browsing::Browsed, String>>,
    sink: Option<crate::browsers::FrameSink>,
    ends: Option<Box<dyn FnOnce(Exit) + Send>>,
}

impl StandInScreen {
    pub fn shown(&self) -> std::sync::MutexGuard<'_, Shown> {
        self.0.lock().unwrap()
    }

    /// It makes `image` as its next frame, when the host is asking for frames.
    pub fn makes(&self, image: &[u8]) {
        let (sink, document) = {
            let mut shown = self.shown();
            if !shown.showing {
                return;
            }
            shown.made += 1;
            (shown.sink.clone(), shown.document)
        };
        if let Some(sink) = sink {
            sink(
                purlis_session_protocol::browsing::Aim {
                    page: "p1".into(),
                    document,
                },
                Bytes::copy_from_slice(image),
            );
        }
    }

    /// It answers its next act with `answer`.
    pub fn answers(&self, answer: Result<purlis_session_protocol::browsing::Browsed, String>) {
        self.shown().answers.push_back(answer);
    }

    /// The engine ended by itself: it acts no more.
    pub fn ends(&self, exit: Exit) {
        let tell = {
            let mut shown = self.shown();
            shown.stopped = true;
            shown.ends.take()
        };
        if let Some(tell) = tell {
            tell(exit);
        }
    }
}

impl crate::browsers::Screen for StandInScreen {
    /// A stand-in reaches nothing, whatever it is told.
    fn reach_now(
        &self,
        _reach: &purlis_session_protocol::browsing::BrowserReach,
    ) -> Result<(), String> {
        Ok(())
    }

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
        let size = self.shown().size;
        if size == (0, 0) { (1280, 800) } else { size }
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

    fn act(
        &self,
        call_id: &str,
        act: purlis_session_protocol::browsing::Act,
        epoch: u64,
    ) -> Result<purlis_session_protocol::browsing::Browsed, crate::browsers::NotActed> {
        let mut shown = self.shown();
        if shown.stopped {
            return Err(crate::browsers::NotActed::Ended);
        }
        if shown.taken_over || epoch != shown.epoch {
            return Err(crate::browsers::NotActed::Refused(
                "the person took this browser over while the call ran".into(),
            ));
        }
        shown.acts.push((call_id.to_owned(), act));
        shown.epochs.push(epoch);
        shown
            .answers
            .pop_front()
            .unwrap_or_else(|| {
                Ok(purlis_session_protocol::browsing::Browsed {
                    settled: true,
                    ..Default::default()
                })
            })
            .map_err(crate::browsers::NotActed::Refused)
    }

    fn take_over(&self) -> u64 {
        let mut shown = self.shown();
        shown.taken_over = true;
        shown.epoch += 1;
        shown.epoch
    }

    fn hand_back(&self) -> u64 {
        let mut shown = self.shown();
        shown.taken_over = false;
        shown.epoch += 1;
        shown.epoch
    }

    fn input(
        &self,
        input: &purlis_session_protocol::browsing::Input,
    ) -> Result<(), crate::browsers::NotTaken> {
        use purlis_session_protocol::browsing::Input;
        let mut shown = self.shown();
        if shown.stopped {
            return Err(crate::browsers::NotTaken::Ended);
        }
        if !shown.taken_over {
            return Err(crate::browsers::NotTaken::NotDriving);
        }
        let (Input::Mouse { aim, .. } | Input::Key { aim, .. }) = input;
        if aim.document != shown.document {
            return Err(crate::browsers::NotTaken::Missed);
        }
        shown.inputs.push(input.clone());
        Ok(())
    }

    fn size_to(&self, width: u16, height: u16) -> Result<(), crate::browsers::NotTaken> {
        let mut shown = self.shown();
        if !shown.taken_over {
            return Err(crate::browsers::NotTaken::NotDriving);
        }
        shown.size = (width, height);
        Ok(())
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

    fn start(
        &self,
        data: &std::path::Path,
        reach: Option<&purlis_session_protocol::browsing::BrowserReach>,
    ) -> Result<Arc<dyn crate::browsers::Screen>, String> {
        std::fs::create_dir_all(data).map_err(|why| why.to_string())?;
        let screen = StandInScreen::default();
        screen.shown().data = data.to_path_buf();
        screen.shown().reach = reach.cloned();
        self.started.lock().unwrap().push(screen.clone());
        Ok(Arc::new(screen))
    }
}
