// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The hosted programs a session host runs, and the session protocol they answer (#1998).
//!
//! **A start is composed by the binary, and run here.** The host holds projects, deals each
//! chat's number in its project, and runs, writes to, resizes, views, stops and closes what it
//! started. How a start becomes a program and its environment is the binary's: the window's
//! binary and `purlis` each hand the host a [`Compose`] built on the core, so a chat in the host
//! starts with the environment and `PATH` the same code gives it in the window, and this crate
//! links no core.
//!
//! **What runs a composed start is a seam too** ([`Run`]): a terminal ([`Terminals`]), or a
//! test's stand-in, so what the host does with its programs is tested without a terminal.
//!
//! **Each held project's hook channel is the host's** (#2000, [`Hear`]): the host binds the
//! socket its chats' hooks write to, deals each chat its number and issues it its token, records
//! its program, and admits every line as the window did; each line it admits is pushed to the
//! window that holds the project ([`Pushed::Heard`]), whose reply comes back
//! ([`Command::ReplyHeard`]). The host is each chat's ancestor, so a hook admits it.
//!
//! **Each held project's board is kept here too** (#2194): the window tells the channel each
//! move it makes (`board_moved`) and reads the board whole as it holds the project (`board`).
//! A project outlives the link that held it, with its channel and its board, while it has a
//! program the window has not closed, running or ended: its chats are heard while no window is
//! there, and a window that comes back reads how each one ended.
//!
//! **Every hosted program is inside a chat to the admission check.** The holder the check is
//! given is the window, when the window started this host, or the host itself: a hosted
//! program, and everything it starts, is below both from its first instruction, so a peer among
//! them is refused every person's scope (ADR 0068 §5, FD-27's item 2). Each program's pid is
//! listed too, so one that left the host's tree and kept its session is refused as well.
//!
//! **The event log is the host's, and so is what a program's end leaves** (#2003, [`Record`]):
//! the window's writes reach it as `record`, and as each program ends the host tells its binary
//! how and why, with whether a window held its project then, so the end is kept with the window
//! gone.
//!
//! **The kill switch is the host's** (#2001, ADR 0071): `stop_all` ends every program it runs
//! and starts none until the window re-arms, and the stop is kept in the host's own folder
//! ([`crate::killswitch`]), so a host started again while stopped starts nothing either. The
//! one start let through while stopped is a shell the window opens: looking at what the agents
//! did is the operator's act (ADR 0071, ruled 3), and the window, which alone may re-arm, says
//! which shell is the operator's.
//!
//! **A held project is saved here when the window asks** (#2002): the host runs the binary's
//! saver for it ([`Save`]), the project's watcher and auto-save, built on the core as a start
//! is composed, and tells the window what each look did: only a link that may save a project
//! ([`Power::Save`]) and holds it there hears how saving it goes, its saver's question of who is
//! mid-turn included. One saver per project at each place it is held (#2178), however often it
//! is asked. A link that ends without a word leaves it saving, so the project is saved while the
//! window is gone for a
//! moment; the window's own stop, or every window letting go of the project, ends it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::browsing::{Driver, Input};
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::local::Chats;
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Confined, Exit, HeldProgram, Peer, ProgramKind, Pushed,
    Readiness, Refusal, SandboxTold, Since, StartProgram, ViewKind, ViewRole,
};
use purlis_session_protocol::version::Version;
use purlis_session_protocol::view::{Attachment, Source, ViewId};
use purlis_terminal::engine::{AlacrittyEngine, SCROLLBACK, Size};
use purlis_terminal::session::{Session, Spec};
use tokio::sync::mpsc;

use crate::browsers::{Browser, Browsers, NoBrowsers};
use crate::confining::{Confine, ConfinesNothing, Confining, Planned, ToConfine};
use crate::hearing::{Channel, HandOn, Hear, HearsNothing};
use crate::hostrecord::{HostRecord, Started};
use crate::killswitch::{NoStopsElsewhere, StopsElsewhere, Switch};
use crate::recording::{Ended, Record, RecordsNothing, Stopped};
use crate::serving::Window;

/// A start, as the host hands it to its [`Compose`]: the project it is in, the number the chat
/// was dealt there, and what the client asked for.
pub struct ToCompose<'a> {
    /// The project's stable id.
    pub project: &'a str,
    /// Where the project is on this machine.
    pub project_path: &'a Path,
    /// The chat's number in its project.
    pub number: u32,
    /// The size its terminal starts at.
    pub size: Size,
    /// The start, as the client sent it.
    pub start: &'a StartProgram,
    /// Where its hooks report, and the token they carry, as its project's channel issued them:
    /// none where the host hears no channel for the project (#2000). A window's launch carries
    /// the same.
    pub reporting: Option<&'a session::Reporting>,
    /// The scope of the link the start came on: what its run says started it (#2171).
    pub started_by: Scope,
    /// What the host does beside a start its binary composes itself (#2171).
    pub beside: &'a dyn Beside,
}

/// Turns a start into the program to run and the environment it runs with: the binary's, built
/// on the core, so the host links none of it (#1998).
pub trait Compose: Send + Sync + 'static {
    /// The program to run for `starting`, or why it cannot be started, in a sentence. Called
    /// off the host's runtime: it may read files and wait on [`ToCompose::beside`].
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String>;

    /// **How the run of `spec` is sandboxed, as this host applies it** (#2636), where
    /// `confined` says whether a confinement holds it: read from what is started, never from
    /// the start. A binary that wraps its programs says by which sandbox; by default a
    /// confined run is held by its proxy, and any other is off.
    fn sandbox_of(&self, spec: &Spec, confined: bool) -> session::RunSandbox {
        let _ = spec;
        if confined {
            session::RunSandbox::By("proxy".to_owned())
        } else {
            session::RunSandbox::Off
        }
    }
}

/// **What the host does beside a start its binary composes itself** (#2171): a start with no
/// launch is composed in the host from the project's own files, and what the window does beside
/// its own start is done here.
pub trait Beside {
    /// Starts the confinement `plan` plans for the program being started, in its project: the
    /// program's own from the first, so it ends with the program, or with the start where the
    /// start fails. Its proxy asks nobody, and no link hears it, unless the start named its
    /// confinement ([`StartProgram::confinement`], #2407): then it is named to the link the
    /// start came on, which hears it and is asked.
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String>;
    /// Writes `entry` in the event log this host holds, durably, or says why not.
    fn record(&self, entry: serde_json::Value) -> Result<(), String>;
}

/// A host whose binary composes no start: every start is refused with a sentence.
pub struct ComposesNothing;

impl Compose for ComposesNothing {
    fn compose(&self, _starting: &ToCompose<'_>) -> Result<Spec, String> {
        Err("this session host was given no way to compose a start".into())
    }
}

/// What one look of a project's auto-save did, as its saver tells it (#2002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Looked {
    /// It saved the project.
    pub saved: bool,
    /// What it did moved files in the tree: a fast-forward, a save's rebase.
    pub moved_tree: bool,
}

/// What a project's saver is told between its looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poke {
    /// A program in the project ended: its work is most likely done.
    ProgramEnded,
    /// The window came back into focus: fetch, unless it fetched a moment ago.
    Fetch,
}

/// A program running in a project, as its saver asks who could be writing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Working {
    /// Its number in its project.
    pub number: u32,
    /// Where it started, where the start said.
    pub cwd: Option<PathBuf>,
}

/// A project to save, as the host hands it to its [`Save`].
pub struct ToSave {
    /// The project's stable id.
    pub project: String,
    /// Where the project is on this machine.
    pub project_path: PathBuf,
    /// Told what each look that did something did.
    pub told: Box<dyn Fn(Looked) + Send + Sync>,
    /// The programs running in the project now.
    pub working: Box<dyn Fn() -> Vec<Working> + Send + Sync>,
    /// The chats mid-turn in a workspace of the project, as the window's board says, by the
    /// names the window shows; `None` when no window is linked, or it did not answer within
    /// [`ASK_WITHIN`], or it cannot tell. Asked from the saver's own thread: it waits.
    pub ask_the_window: Box<AskTheWindow>,
}

/// Asks the window which chats are mid-turn in a workspace ([`ToSave::ask_the_window`]).
pub type AskTheWindow = dyn Fn(&str) -> Option<Vec<String>> + Send + Sync;

/// How long the host waits for the window to say who is mid-turn, before the saver takes every
/// chat it runs there as mid-turn.
pub const ASK_WITHIN: Duration = Duration::from_secs(2);

/// Saves a held project: its watcher and its auto-save, the binary's, built on the core, so
/// this crate links none of it (#2002).
pub trait Save: Send + Sync + 'static {
    /// Start saving `saving`'s project, or say why it cannot be saved, in a sentence. Dropping
    /// what it answers stops it.
    fn save(&self, saving: ToSave) -> Result<Box<dyn Saving>, String>;
}

/// A project being saved. Dropping it stops the saving.
pub trait Saving: Send + 'static {
    /// Something to act on at its next look.
    fn poke(&self, poke: Poke);
}

/// A host whose binary saves nothing: every `start_saving` is refused with a sentence.
pub struct SavesNothing;

impl Save for SavesNothing {
    fn save(&self, _saving: ToSave) -> Result<Box<dyn Saving>, String> {
        Err("this session host was given no way to save a project".into())
    }
}

/// Runs a composed start.
pub trait Run: Send + Sync + 'static {
    fn run(&self, spec: Spec) -> Result<Arc<dyn Running>, String>;

    /// **Ends what a program the last host ran left behind** (#2007), as that host's host record
    /// wrote it down: its process group, where its process is still the one that was started.
    /// Called before this host serves anyone, for each program the last host left running.
    /// Nothing, for what runs no process of its own.
    fn end_what_was_left(&self, left: &Started) {
        let _ = left;
    }
}

/// A program the host runs, as the host drives it. Each call that can wait on the program
/// (`stop`) is made off the host's runtime.
pub trait Running: Send + Sync + 'static {
    /// Bytes typed to it, as they are.
    fn write(&self, bytes: &[u8]) -> Result<(), String>;
    /// Its terminal's new size.
    fn resize(&self, size: Size) -> Result<(), String>;
    /// What a view of it attaches to.
    fn source(&self) -> Arc<dyn Source>;
    /// What a view of `kind` attaches to, as `role`, at most `rate` frames a second where it
    /// says, its frames read as `reads` says its link reads them (frames that name what they
    /// show, 1.14; the page's origin, 1.35); `None` when it has no view of that kind. A
    /// terminal's is its terminal view.
    fn view(
        &self,
        kind: ViewKind,
        role: Option<ViewRole>,
        rate: Option<u32>,
        reads: crate::browsers::Reads,
    ) -> Option<Arc<dyn Source>> {
        let _ = (role, rate, reads);
        (kind == ViewKind::Terminal).then(|| self.source())
    }
    /// The size its views are drawn at, where it is not its terminal's: a browser's page, in
    /// CSS pixels.
    fn view_size(&self) -> Option<(u16, u16)> {
        None
    }
    /// Ends it and everything it started, keeping its last screen; returns once it is gone.
    fn stop(&self);
    /// Its process, while it runs.
    fn process(&self) -> Option<u32>;
    /// When its process started, as the kernel says it, where it says: what the host record
    /// tells it apart from a later process given the same number by (#2007).
    fn process_started(&self) -> Option<String> {
        self.process().and_then(purlis_same_user::Parents::started)
    }
    /// Whether it is ready to be typed at.
    fn readiness(&self) -> Readiness;
    /// Calls `tell` once, as it ends.
    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>);
    /// Carries out a browser's act (#2013): refused for every program that is not a browser.
    fn act(
        &self,
        call_id: &str,
        act: purlis_session_protocol::browsing::Act,
    ) -> Result<purlis_session_protocol::browsing::Browsed, crate::browsers::NotActed> {
        let _ = (call_id, act);
        Err(crate::browsers::NotActed::Refused(
            "this program is not a browser, so it takes no browser's act".into(),
        ))
    }
    /// purlis performs `login` in it (#2016): refused for every program that is not a browser.
    fn log_in(
        &self,
        login: purlis_session_protocol::browsing::LogIn,
    ) -> Result<purlis_session_protocol::browsing::Browsed, crate::browsers::NotActed> {
        drop(login);
        Err(crate::browsers::NotActed::Refused(
            "this program is not a browser, so purlis logs nothing in to it".into(),
        ))
    }
    /// What a browser may reach from now on (#2018): nothing to do for any other program.
    fn reach_now(
        &self,
        reach: &purlis_session_protocol::browsing::BrowserReach,
    ) -> Result<(), String> {
        let _ = reach;
        Ok(())
    }
    /// The person takes it over (#2014): refused for every program that is not a browser.
    fn take_over(&self) -> Result<Driver, Refusal> {
        Err(not_a_browser())
    }
    /// The person hands it back (#2014).
    fn hand_back(&self) -> Result<Driver, Refusal> {
        Err(not_a_browser())
    }
    /// One event of the person's (#2014).
    fn input(&self, input: &Input) -> Result<(), Refusal> {
        let _ = input;
        Err(not_a_browser())
    }
    /// The person sets its pages' size (#2014).
    fn size_browser(&self, width: u16, height: u16) -> Result<(), Refusal> {
        let _ = (width, height);
        Err(not_a_browser())
    }
    /// Who drives it, where it is a browser (#2014).
    fn driver(&self) -> Option<Driver> {
        None
    }
}

/// Who drives a browser now, as an answer.
fn driving(driver: Driver) -> Answer {
    Answer::Driving { driver }
}

/// Why a browser did not act, as the protocol refuses it.
fn not_acted(not: crate::browsers::NotActed) -> Refusal {
    match not {
        crate::browsers::NotActed::Ended => Refusal {
            code: session::BROWSER_ENDED.into(),
            why: crate::browsers::AN_ENDED_BROWSER.into(),
        },
        crate::browsers::NotActed::Refused(why) => Refusal {
            code: session::BROWSER_REFUSED.into(),
            why,
        },
    }
}

/// What a browser's command for a program that is no browser is refused with.
fn not_a_browser() -> Refusal {
    Refusal {
        code: session::MALFORMED.into(),
        why: "this program is not a browser".into(),
    }
}

/// Runs each start in a terminal of its own: a pseudo-terminal, and the engine that keeps its
/// screen, with the scrollback the window's panes keep.
pub struct Terminals;

impl Run for Terminals {
    fn run(&self, spec: Spec) -> Result<Arc<dyn Running>, String> {
        let engine = AlacrittyEngine::new(spec.size, SCROLLBACK as usize);
        let session = Session::spawn(spec, Box::new(engine)).map_err(|why| why.to_string())?;
        Ok(Arc::new(Terminal(Arc::new(session))))
    }

    /// A terminal's program leads a session of its own, so its process group's id is its pid,
    /// and the group holds what it started that did not leave on purpose. The group is killed
    /// only while its leader is the very process that was started, by the kernel's word on when
    /// it started, in this boot of the machine: a number that has since gone to another process
    /// is left alone, and so is a leader the kernel cannot say anything of, and anything noted
    /// in another boot.
    fn end_what_was_left(&self, left: &Started) {
        let (Some(process), Some(started)) = (left.process, left.process_started.as_deref()) else {
            return;
        };
        if !crate::hostrecord::of_this_boot(
            left.boot.as_deref(),
            crate::hostrecord::this_boot().as_deref(),
        ) {
            return;
        }
        if process <= 1 || purlis_same_user::Parents::started(process).as_deref() != Some(started) {
            return;
        }
        let Ok(group) = i32::try_from(process) else {
            return;
        };
        let group = nix::unistd::Pid::from_raw(group);
        if nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL).is_ok() {
            crate::say(&format!(
                "chat {} of {} was still running after its session host ended, and is ended now",
                left.number,
                left.project_path.display()
            ));
        }
    }
}

struct Terminal(Arc<Session>);

impl Running for Terminal {
    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.0.write(bytes).map_err(|why| why.to_string())
    }

    fn resize(&self, size: Size) -> Result<(), String> {
        self.0.resize(size).map_err(|why| why.to_string())
    }

    fn source(&self) -> Arc<dyn Source> {
        Arc::new(TerminalSource(Arc::clone(&self.0)))
    }

    fn stop(&self) {
        self.0.stopper().stop();
    }

    fn process(&self) -> Option<u32> {
        self.0.process_id()
    }

    fn readiness(&self) -> Readiness {
        Readiness {
            edits_lines: self.0.edits_lines(),
            quiet_for_ms: u64::try_from(self.0.quiet_for().as_millis()).unwrap_or(u64::MAX),
        }
    }

    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        self.0.when_it_ends(Box::new(move |exit| {
            tell(match exit {
                purlis_terminal::session::Exit::Code(code) => Exit::Code(code),
                purlis_terminal::session::Exit::Signal(signal) => Exit::Signal(signal),
            });
        }));
    }
}

/// A terminal as a view shows it: its screen as it is now, then what its program writes after,
/// followed without a thread ([`Session::follow`]).
struct TerminalSource(Arc<Session>);

impl Source for TerminalSource {
    fn attach(&self) -> Attachment {
        let following = self.0.follow();
        Attachment {
            snapshot: Bytes::from(following.snapshot),
            live: following.live.map(Bytes::from).boxed(),
        }
    }
}

/// The most windows that came back a host keeps as holding its chats, beside the first.
const MOST_WINDOWS_KEPT: usize = 16;

/// The most notices a subscriber's queue holds before it is let go of.
const MOST_NOTICES_QUEUED: usize = 1024;

/// The hosted programs and the projects they are in: what answers the session protocol on
/// every link. A clone is the same host.
#[derive(Clone)]
pub struct Hosting {
    inner: Arc<Inner>,
}

struct Inner {
    compose: Box<dyn Compose>,
    confine: Box<dyn Confine>,
    run: Box<dyn Run>,
    save: std::sync::OnceLock<Box<dyn Save>>,
    hear: Box<dyn Hear>,
    /// What starts a browser (#2012).
    browsers: Box<dyn Browsers>,
    /// What writes the event log and keeps a program's end (#2003): nothing until the binary
    /// gives one ([`Hosting::recording_with`]).
    record: OnceLock<Box<dyn Record>>,
    /// Where each program is written down as it starts and as it ends, so the next host
    /// settles what this one leaves running if it is killed outright (#2007): in the host's own
    /// folder, for a host kept there; none for a host kept in memory only (a test's).
    host_record: Option<HostRecord>,
    state: Mutex<State>,
    /// Held while a saver is started, so two windows asking at once start one.
    starting_a_saver: Mutex<()>,
    /// Held while a confinement's ports are withdrawn as its last link ends, or given back as its
    /// window takes it back (#2625): each acts only on the ports still [`Confinement::unheld`],
    /// so whichever runs first, a port is carried after a take-back.
    porting: Mutex<()>,
    switch: Switch,
    /// What reads a stop written elsewhere (#2190): the binary's, or nothing.
    elsewhere: OnceLock<Box<dyn StopsElsewhere>>,
    /// The stop written elsewhere the last look found: the host acts on each stop as it is
    /// written, so files the window has yet to clear after its re-arm stop nothing again, and
    /// a stop written after that, while the files still say stopped, is heard.
    seen_elsewhere: Mutex<Option<String>>,
    /// What links a window's link to a runner this machine keeps, by the runner's name (#2679):
    /// the binary's, or nothing, and every `relay_to` is refused.
    relays: OnceLock<crate::routing::RelaysTo>,
    /// The window each link this host relays is named, across every relay its links make
    /// (#2744): one link holds a name at a time.
    claimed: crate::relay::Claimed,
    /// What moves code in a project this host keeps (#2046): the binary's, or nothing, and every
    /// transfer is refused.
    code: OnceLock<Arc<dyn crate::code::Code>>,
    /// Where this host keeps its own projects (#2046): a runner's `<data>/projects`, each in a
    /// folder named by its id. None, and a hold that names no place is refused.
    projects: OnceLock<PathBuf>,
    /// The most bytes one bundle may be here: [`purlis_session_protocol::code::MOST_BUNDLE_BYTES`]
    /// unless a test holds it lower.
    most_bundle: OnceLock<u64>,
    /// Which runner it relays to drains (#2865): the binary's, or nothing.
    drains: OnceLock<crate::routing::DrainsTo>,
    /// Set once the window has quit (#2006): whoever serves this host stops.
    quit: tokio::sync::watch::Sender<bool>,
    /// Told whenever a window links or goes, or a project is held or let go of: what the host's
    /// life with a window that may come back waits on (`crate::lifetime`).
    changed: tokio::sync::Notify,
}

#[derive(Default)]
struct State {
    /// Each project held, at each place it is held: one project is at another path in every
    /// clone, and a window may hold two clones of it at once, each on a link of its own (#2178).
    projects: HashMap<HeldAt, Held>,
    programs: HashMap<String, Hosted>,
    /// The processes of programs closed or ended by the host, kept for the admission check for
    /// as long as the host runs: a process a program started may outlive it in its session, in
    /// a group of its own the close did not reach, and is still inside that chat. Keeping a
    /// number past its session errs towards refusing a peer, never towards admitting one.
    closed: Vec<u32>,
    /// Each held place's highest number dealt: it only rises, so a number names one chat of
    /// that clone, as the clone's own record keeps them.
    dealt: HashMap<HeldAt, u32>,
    /// Who is told what the host pushes ([`Subscriber`]).
    subscribers: Vec<Subscriber>,
    /// The chats' confinements (#1999), by the id the window gave each.
    confinements: HashMap<String, Confinement>,
    /// The window each link between hosts is for, by the link's number, as the relaying host
    /// named it ([`Command::TakeBack`], #2511). A link's name ends with it.
    relayed_for: HashMap<u64, String>,
    /// Each question to the window waiting for its answer, by its number.
    questions: HashMap<u64, std::sync::mpsc::Sender<Option<Vec<String>>>>,
    /// The last question's number.
    asked: u64,
    /// The window that started this host, if one did.
    window: Option<Window>,
    /// Each project being saved, at the place it is held: one saver each (#2002, #2178).
    saving: HashMap<HeldAt, Box<dyn Saving>>,
    /// The window's links now (#2006): each `local-ui` link that subscribed or held a project.
    windows: HashSet<u64>,
    /// A window has linked, or the one that started this host has gone: from then on, a host
    /// with no window and nothing a window could come back for stops.
    had_a_window: bool,
    /// Since when no window has been linked, once one had.
    windowless_since: Option<Instant>,
    /// Each project kept or saved with no link holding it, and since when (#2006, #2262).
    unheld_since: HashMap<HeldAt, Instant>,
    /// Each process a window linked from to the host the window started, but the one that
    /// started it: a window that came back holds the chats too (#2006).
    came_back: Vec<u32>,
    /// How many browsers are starting now (#2611): each holds a place in the device's budget
    /// from its count to its start, so two starts never both take the last one.
    browsers_starting: usize,
    /// The engine downloads of the browsers starting now, by program (#2207): what
    /// `cancel_engine_download` cancels.
    downloads: HashMap<String, crate::browsers::Download>,
}

/// One subscriber, and whether its link may hear what a chat's sandbox does
/// ([`Power::Confine`]) and how saving a project it holds goes ([`Power::Save`]).
struct Subscriber {
    /// The link it came on ([`link_of`]).
    link: u64,
    told: mpsc::Sender<Pushed>,
    confines: bool,
    saves: bool,
    /// Whether its link knows a browser engine's download (1.37, #2207) and may start a
    /// browser ([`Power::Browser`]): only such a link is pushed one.
    downloads: bool,
}

/// A chat's confinement the host holds (#1999), whose it is, and who may act on it (#2444).
struct Confinement {
    project: String,
    /// Where its project is on this machine, as the host held it when it was started: what a
    /// port given back is checked against this machine's policy for (#2738).
    path: std::path::PathBuf,
    owner: Owner,
    /// The links the host named it to ([`may_act_on`]): the link that made it, from the first;
    /// none, for one the host made for a chat it composed; each window that took its chat
    /// back; and, for a chat of a relayed window, each link that named that window (#2511). A
    /// link's names end with it.
    named_to: HashSet<u64>,
    confining: Arc<dyn Confining>,
    /// The plan it was started from, as the window sent it (#2006).
    plan: session::ConfinePlan,
    /// The relayed window that made it or started its chat, by its relaying host's name for
    /// it (#2511): what takes it back after a re-link. None for any other: the host's own
    /// window's, or one made or composed for a link that named no window.
    window: Option<String>,
    /// The ports of the machine its plan gave that the host withdrew because no link was left
    /// to withdraw one (#2407), and gives back as its window takes it back (#2625). Never one a
    /// window withdrew, which stays withdrawn.
    unheld: Vec<u16>,
}

/// Whose a confinement is: the link that made it, until a start names it, then that program's.
/// One a relayed window's link made is that window's on each of its links while nothing runs in
/// it, and passes to another of them as the link that made it ends (#2624).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Owner {
    Link(u64),
    Program(String),
}

/// **Whether `link` may act on confinement `held`, and hear what its proxy does** (#2444): it
/// was named to the link, and it is the link's own while nothing runs in it, or the link holds
/// the project of the program that runs in it at the place that program runs. Every other
/// confinement is one the host does not hold, to that link.
fn may_act_on(state: &State, held: &Confinement, link: u64) -> bool {
    held.named_to.contains(&link)
        && match &held.owner {
            Owner::Link(made_by) => of_one_window(state, *made_by, link),
            Owner::Program(program) => state.programs.get(program).is_some_and(|hosted| {
                held_by(state, &held.project, link).is_some_and(|at| *at == hosted.at)
            }),
        }
}

/// **Whether `subscriber` hears what confinement `held`'s proxy does** (#2444): its link may
/// confine, and may act on that confinement ([`may_act_on`]). What `sandbox` is pushed to, and
/// who a proxy holds a connection for an ask while.
fn hears_its_proxy(state: &State, held: &Confinement, subscriber: &Subscriber) -> bool {
    subscriber.confines && may_act_on(state, held, subscriber.link)
}

/// **The relayed window `peer`'s link is for** (#2511): the name its relaying host gave it, on a
/// link between hosts that named one. A confinement made or composed for that link is that
/// window's, to take back after a re-link; one made or composed for any other link is no
/// relayed window's.
fn relayed_for(state: &State, peer: &Peer) -> Option<String> {
    if peer.scope() != Scope::RemoteLink {
        return None;
    }
    state.relayed_for.get(&link_of(peer)).cloned()
}

/// Who `peer` opens a program for: this host's window, or the window a link between hosts
/// named (#2050).
fn opened_by(state: &State, peer: &Peer) -> OpenedBy {
    if peer.scope() == Scope::RemoteLink {
        OpenedBy::Relayed(relayed_for(state, peer))
    } else {
        OpenedBy::ThisHost
    }
}

/// The browser `command` acts on, drives, watches or logs in, where it is such a command.
fn browser_acted_on(command: &Command) -> Option<&str> {
    match command {
        Command::Browse { program, .. }
        | Command::LogIn { program, .. }
        | Command::TakeOver { program }
        | Command::HandBack { program }
        | Command::BrowserInput { program, .. }
        | Command::SizeBrowser { program, .. }
        | Command::CancelEngineDownload { program } => Some(program),
        Command::AttachView {
            program,
            kind: ViewKind::Browser,
            ..
        } => Some(program),
        _ => None,
    }
}

/// **Each link between hosts that named `window`** (#2511): a link a renewal replaces and the
/// one that replaces it, while both last. None for no window.
fn links_of_window(state: &State, window: Option<&str>) -> Vec<u64> {
    let Some(window) = window else {
        return Vec::new();
    };
    state
        .relayed_for
        .iter()
        .filter(|(_, named)| named.as_str() == window)
        .map(|(link, _)| *link)
        .collect()
}

/// **Whether links `one` and `other` are the same, or links between hosts that named one
/// window** (#2624): a renewal's two links, while both last.
fn of_one_window(state: &State, one: u64, other: u64) -> bool {
    one == other
        || state
            .relayed_for
            .get(&one)
            .is_some_and(|window| state.relayed_for.get(&other) == Some(window))
}

/// **Each confinement `link` made that nothing runs in yet passes to another link of its
/// window** (#2624), as `link` ends: a renewal's new link starts its chat in it. One whose
/// window has no other link, or that names no window, stays `link`'s, to end with it.
fn hand_on(state: &mut State, link: u64) {
    let mut handed: Vec<(String, u64)> = Vec::new();
    for (id, held) in &state.confinements {
        if held.owner != Owner::Link(link) {
            continue;
        }
        let other = links_of_window(state, held.window.as_deref())
            .into_iter()
            .find(|other| *other != link);
        if let Some(other) = other {
            handed.push((id.clone(), other));
        }
    }
    for (id, other) in handed {
        if let Some(held) = state.confinements.get_mut(&id) {
            held.owner = Owner::Link(other);
            held.named_to.insert(other);
        }
    }
}

/// A place: a project, by its stable id, at the folder it is held at on this machine, by the
/// folder's canonical path, so one folder is one place however its path is spelled. What the
/// host holds, deals numbers in, hears hooks from and saves. The id is the project's on the
/// wire; which of its clones a command means is the one its link holds (#2178).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HeldAt {
    project: String,
    path: PathBuf,
}

/// The place `link` holds `project` at, if it holds it.
fn held_by<'a>(state: &'a State, project: &str, link: u64) -> Option<&'a HeldAt> {
    state
        .projects
        .iter()
        .find(|(at, held)| at.project == project && held.links.contains_key(&link))
        .map(|(at, _)| at)
}

/// The places a link holds `project` at: not one kept only for the programs in it the window
/// has not closed (#2194).
fn held_by_a_link<'a>(state: &'a State, project: &'a str) -> impl Iterator<Item = &'a HeldAt> {
    state
        .projects
        .iter()
        .filter(move |(at, held)| at.project == project && !held.links.is_empty())
        .map(|(at, _)| at)
}

/// Where `project` is for a command from `link`: the place the link holds it at, or, for a link
/// that holds it nowhere, the one place a link holds it at. A project held at two places,
/// neither the link's, is not guessed at; nothing starts, is dealt or is confined in a place no
/// link holds.
fn held_at(state: &State, project: &str, link: u64) -> Result<HeldAt, Refusal> {
    if let Some(at) = held_by(state, project, link) {
        return Ok(at.clone());
    }
    let mut places = held_by_a_link(state, project);
    match (places.next(), places.next()) {
        (None, _) => Err(no_such_project(project)),
        (Some(at), None) => Ok(at.clone()),
        (Some(_), Some(_)) => Err(Refusal {
            code: session::NO_SUCH_PROJECT.into(),
            why: format!(
                "project {project} is held at more than one place on this host, and this link \
                 holds it at none of them"
            ),
        }),
    }
}

/// Where `project` is saved for a command from `link`: as [`held_at`] says while any place
/// holds it, or, once nobody holds it any more (a window that ended leaves it saved), the one
/// place it is still saved, if there is one.
fn saved_at(state: &State, project: &str, link: u64) -> Result<Option<HeldAt>, Refusal> {
    if held_by_a_link(state, project).next().is_some() {
        return held_at(state, project, link).map(Some);
    }
    let mut saved = state.saving.keys().filter(|at| at.project == project);
    Ok(match (saved.next(), saved.next()) {
        (Some(at), None) => Some(at.clone()),
        _ => None,
    })
}

struct Held {
    /// How many holds each link has on it, by the link.
    links: HashMap<u64, usize>,
    /// Its hook channel, where the host hears hooks and it could be opened.
    channel: Option<Arc<dyn Channel>>,
}

/// A link, as the host tells its holds and confinements apart: its number, the same for every
/// command of the link and never another link's, live or ended ([`Peer::link`]).
fn link_of(peer: &Peer) -> u64 {
    peer.link()
}

struct Hosted {
    kind: ProgramKind,
    /// The project it is in, at the place it was started there.
    at: HeldAt,
    number: u32,
    run: Option<String>,
    running: Arc<dyn Running>,
    size: Size,
    /// Its process as it started, kept for the admission check after it ends.
    process: Option<u32>,
    /// Where it started, where its start said: what its project's saver asks.
    cwd: Option<PathBuf>,
    ended: bool,
    /// How it ended, once it has: what a window that comes back is told (#2006).
    exit: Option<Exit>,
    /// The host's own act that is ending it, where one is ([`Stopped`]).
    stopping: Arc<Mutex<Option<Stopped>>>,
    /// How its run is sandboxed, as this host applied it (#2636).
    sandbox: session::RunSandbox,
    /// The window's note of it (1.39, #2025): its own words, kept for it and never read here.
    note: Option<session::ProgramNote>,
    /// The identity of each login purlis performed in it, or of the kept login it opened from
    /// (1.39, #2025): the host's own record, which no note of the window's unsays.
    logins: Arc<Mutex<Vec<String>>>,
    /// Who opened it, for a browser (#2050): this host's own window, or a link between hosts
    /// for the window it named, which alone may act on it from such a link.
    opened_by: OpenedBy,
}

/// **Who opened a hosted program** (#2050): a link of this host's own (its window), or a link
/// between hosts, for the window it named (none where it named none).
#[derive(Debug, Clone, PartialEq, Eq)]
enum OpenedBy {
    ThisHost,
    Relayed(Option<String>),
}

/// Says the host's act `by` is ending a program, keeping the highest where another is already.
fn stopping_by(stopping: &Mutex<Option<Stopped>>, by: Stopped) {
    let mut stopping = stopping
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *stopping = (*stopping).max(Some(by));
}

impl Hosting {
    /// A host whose starts `compose` composes, each run in a terminal of its own, its kill
    /// switch kept in memory only: a test's.
    pub fn new(compose: impl Compose) -> Hosting {
        Hosting::running_with(compose, Terminals)
    }

    /// A host whose starts `compose` composes and `run` runs, which hears no hooks and starts no
    /// chat's confinement, its kill switch kept in memory only: a test's.
    pub fn running_with(compose: impl Compose, run: impl Run) -> Hosting {
        Hosting::with(compose, run, HearsNothing, ConfinesNothing)
    }

    /// A host whose starts `compose` composes and `run` runs, and whose held projects' hooks
    /// `hear` hears (#2000), its kill switch kept in memory only: a test's.
    pub fn hearing_with(compose: impl Compose, run: impl Run, hear: impl Hear) -> Hosting {
        Hosting::with(compose, run, hear, ConfinesNothing)
    }

    /// A host whose starts `compose` composes and `run` runs, and whose chats' confinements
    /// `confine` starts (#1999), its kill switch kept in memory only: a test's.
    pub fn confining(compose: impl Compose, confine: impl Confine, run: impl Run) -> Hosting {
        Hosting::with(compose, run, HearsNothing, confine)
    }

    /// A host whose starts `compose` composes and `run` runs, whose held projects' hooks `hear`
    /// hears (#2000), and whose chats' confinements `confine` starts (#1999), its kill switch
    /// kept in memory only: a test's.
    pub fn with(
        compose: impl Compose,
        run: impl Run,
        hear: impl Hear,
        confine: impl Confine,
    ) -> Hosting {
        Hosting::built(compose, run, hear, confine, Switch::in_memory(), None)
    }

    /// A host whose starts `compose` composes and `run` runs, which hears no hooks and starts
    /// no confinement, with its kill switch kept in `folder`, its own, and read from there now:
    /// a host started again while stopped starts nothing until the window re-arms (ADR 0071).
    pub fn running_with_its_switch_in(
        folder: &Path,
        compose: impl Compose,
        run: impl Run,
    ) -> Hosting {
        Hosting::kept(folder, compose, run, HearsNothing, ConfinesNothing)
    }

    /// [`Hosting::with`], with its kill switch kept in `folder`, its own, and read from there
    /// now (ADR 0071), and its host record kept there (#2007): what the window's host and
    /// `purlis serve` run.
    pub fn kept(
        folder: &Path,
        compose: impl Compose,
        run: impl Run,
        hear: impl Hear,
        confine: impl Confine,
    ) -> Hosting {
        Hosting::built(
            compose,
            run,
            hear,
            confine,
            Switch::kept_in(folder),
            Some(HostRecord::in_folder(folder)),
        )
    }

    fn built(
        compose: impl Compose,
        run: impl Run,
        hear: impl Hear,
        confine: impl Confine,
        switch: Switch,
        host_record: Option<HostRecord>,
    ) -> Hosting {
        Hosting {
            inner: Arc::new(Inner {
                compose: Box::new(compose),
                confine: Box::new(confine),
                run: Box::new(run),
                save: std::sync::OnceLock::new(),
                hear: Box::new(hear),
                browsers: Box::new(NoBrowsers),
                record: OnceLock::new(),
                host_record,
                state: Mutex::default(),
                starting_a_saver: Mutex::new(()),
                porting: Mutex::new(()),
                switch,
                elsewhere: OnceLock::new(),
                relays: OnceLock::new(),
                claimed: Arc::default(),
                code: OnceLock::new(),
                projects: OnceLock::new(),
                most_bundle: OnceLock::new(),
                drains: OnceLock::new(),
                seen_elsewhere: Mutex::new(None),
                quit: tokio::sync::watch::Sender::new(false),
                changed: tokio::sync::Notify::new(),
            }),
        }
    }

    /// A host that saves the projects it is asked to with `save` (#2002). Said once, before the
    /// host serves; a host given none refuses to save.
    pub fn saving_with(self, save: impl Save) -> Hosting {
        let _ = self.inner.save.set(Box::new(save));
        self
    }

    /// A host whose browsers `browsers` starts, each with a data folder of its own in the
    /// folder it names (#2012). Given before the host is shared.
    pub fn with_browsers(mut self, browsers: impl Browsers) -> Hosting {
        let inner =
            Arc::get_mut(&mut self.inner).expect("browsers are given before the host is shared");
        inner.browsers = Box::new(browsers);
        self
    }

    /// This host hears a stop written elsewhere through `stops` (#2190, ADR 0071): `purlis
    /// stop --all` with no window there. Set once, as the binary builds its host; a second is let
    /// go of. A host given none hears only `stop_all`.
    pub fn heeding(self, stops: impl StopsElsewhere) -> Hosting {
        let _ = self.inner.elsewhere.set(Box::new(stops));
        self
    }

    /// **Looks for a stop written elsewhere, now and then every [`crate::killswitch::LOOK_EVERY`]**
    /// (#2190), for as long as the host serves: what [`crate::serve`] runs beside its socket. A
    /// stop it finds is the host's own, kept as `stop_all` keeps one, and every program the host
    /// runs ends; only the window re-arms. The host also looks before every start.
    pub async fn heed_stops_written_elsewhere(self) {
        loop {
            self.heed_a_stop_written_elsewhere().await;
            tokio::time::sleep(crate::killswitch::LOOK_EVERY).await;
        }
    }

    /// Throws the kill switch where a stop was written elsewhere since the last look, and the
    /// host is not stopped already. Each stop is acted on as it is written: files that still
    /// hold the stop the host was re-armed past, before the window clears them, stop nothing
    /// again, and any stop written after it does.
    async fn heed_a_stop_written_elsewhere(&self) {
        let stops: &dyn StopsElsewhere = match self.inner.elsewhere.get() {
            Some(stops) => stops.as_ref(),
            None => &NoStopsElsewhere,
        };
        let now = stops.last_stop();
        let written = {
            let mut seen = self
                .inner
                .seen_elsewhere
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let written = now.is_some() && *seen != now;
            *seen = now;
            written
        };
        if written && !self.inner.switch.is_stopped() {
            crate::say(
                "every agent was stopped from elsewhere (purlis stop --all); the host stops \
                 every program it runs",
            );
            self.stop_all().await;
        }
    }

    /// This host writes the event log, and keeps each program's end, with `record` (#2003). Set
    /// once, as the binary builds its host; a second is let go of.
    pub fn recording_with(self, record: impl Record) -> Hosting {
        let _ = self.inner.record.set(Box::new(record));
        self
    }

    /// A host that relays a window's link to a runner this machine keeps, as `relays` links to
    /// it (`relay_to`, 1.33, #2679). Set once, as the binary builds its host; a second is let go
    /// of. A host given none relays no link.
    pub fn relaying_with(self, relays: crate::routing::RelaysTo) -> Hosting {
        let _ = self.inner.relays.set(relays);
        self
    }

    /// This host, telling by `drains` which runner it relays to drains (#2865).
    pub fn draining_with(self, drains: crate::routing::DrainsTo) -> Hosting {
        let _ = self.inner.drains.set(drains);
        self
    }

    /// Which runner it relays to drains, where it is told.
    pub(crate) fn drains(&self) -> Option<crate::routing::DrainsTo> {
        self.inner.drains.get().cloned()
    }

    /// What links a window's link to a runner, where this host relays one.
    pub(crate) fn relays(&self) -> Option<crate::routing::RelaysTo> {
        self.inner.relays.get().cloned()
    }

    /// The names this host's relayed links hold (#2744), which every relay it makes shares.
    pub(crate) fn claimed(&self) -> crate::relay::Claimed {
        Arc::clone(&self.inner.claimed)
    }

    /// A host that moves code in the projects it keeps with `code` (`take_code`, `give_code`,
    /// 1.34, #2046). Set once, as the binary builds its host; a second is let go of. A host
    /// given none refuses every transfer.
    pub fn moving_code_with(self, code: impl crate::code::Code) -> Hosting {
        let _ = self.inner.code.set(Arc::new(code));
        self
    }

    /// A host that keeps its own projects in `folder` (1.34, #2046): a hold that names no place
    /// holds the project in `folder/<project id>`, made there if it is not, and only a project
    /// held so has code moved in it. Set once; a second is let go of.
    pub fn keeping_projects_in(self, folder: PathBuf) -> Hosting {
        let _ = self.inner.projects.set(folder);
        self
    }

    /// The most bytes one bundle may be on this host.
    pub(crate) fn most_bundle(&self) -> u64 {
        self.inner
            .most_bundle
            .get()
            .copied()
            .unwrap_or(purlis_session_protocol::code::MOST_BUNDLE_BYTES)
    }

    /// A host that holds every bundle to `most` bytes: a test's, which cannot send a gibibyte.
    #[cfg(test)]
    pub(crate) fn holding_bundles_to(self, most: u64) -> Hosting {
        let _ = self.inner.most_bundle.set(most);
        self
    }

    /// What moves code in this host: the binary's, or nothing.
    pub(crate) fn code(&self) -> Arc<dyn crate::code::Code> {
        match self.inner.code.get() {
            Some(code) => Arc::clone(code),
            None => Arc::new(crate::code::MovesNoCode),
        }
    }

    /// The folder this host keeps `project` in: `<projects>/<project id>`, for an id that is a
    /// ULID; or why not.
    fn kept_place(&self, project: &str) -> Result<PathBuf, Refusal> {
        let Some(folder) = self.inner.projects.get() else {
            return Err(Refusal {
                code: session::NO_SUCH_PROJECT.into(),
                why: format!(
                    "this session host keeps no projects of its own, so project {project} is \
                     held only at a place the hold names"
                ),
            });
        };
        if !a_ulid(project) {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "`{project}` is not a project's id, so no folder was made or held for it"
                ),
            });
        }
        let place = folder.join(project);
        // A link in its place would send what the desktop holds there elsewhere on this machine.
        if std::fs::symlink_metadata(&place).is_ok_and(|meta| !meta.is_dir()) {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "the folder for project {project} is a link or a file, so it is not held \
                     and no code is moved there"
                ),
            });
        }
        Ok(place)
    }

    /// Where `peer` holds `project`, held where this host keeps it (#2046): the one place code
    /// is moved. A project held at a place a client named, or not held, is refused.
    pub(crate) fn kept_project_held(&self, project: &str, peer: &Peer) -> Result<PathBuf, Refusal> {
        let kept = self.kept_place(project)?;
        let at = held_at(&self.state(), project, link_of(peer))?;
        let kept = std::fs::canonicalize(&kept).unwrap_or(kept);
        if at.path != kept {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "project {project} is held at a place the hold named, not where this host \
                     keeps it, so no code is moved there"
                ),
            });
        }
        Ok(at.path)
    }

    /// What writes this host's event log: the binary's, or nothing.
    fn record(&self) -> &dyn Record {
        match self.inner.record.get() {
            Some(record) => record.as_ref(),
            None => &RecordsNothing,
        }
    }

    /// Whether a link holds the project at `at` now: a window that decides what a program's end
    /// there moves.
    fn watched(&self, at: &HeldAt) -> bool {
        self.state()
            .projects
            .get(at)
            .is_some_and(|held| !held.links.is_empty())
    }

    /// The window that started this host and is the first thing its programs run below.
    pub fn started_by(self, window: Option<Window>) -> Hosting {
        self.state().window = window;
        self
    }

    /// The window's scope was admitted from process `pid` (#2006): on a host a window started,
    /// a window that comes back holds the chats as the first one did, so a peer below it is
    /// inside one from now on. Whether it may be served: holders are only ever added, and past
    /// [`MOST_WINDOWS_KEPT`] a window from yet another process is refused.
    pub fn a_window_linked_from(&self, pid: u32) -> bool {
        let mut state = self.state();
        let Some(Window(first)) = state.window else {
            return true;
        };
        if pid == first || state.came_back.contains(&pid) {
            return true;
        }
        if state.came_back.len() >= MOST_WINDOWS_KEPT {
            return false;
        }
        state.came_back.push(pid);
        true
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Tell every subscriber, letting go of one that is gone or has stopped reading. What a
    /// chat's sandbox did is told only to a link that may confine and may act on that
    /// confinement ([`may_act_on`]).
    fn tell(&self, pushed: &Pushed) {
        let mut state = self.state();
        let hearers: Option<Vec<u64>> = match pushed {
            Pushed::Sandbox { confinement, .. } => Some(
                state
                    .confinements
                    .get(confinement)
                    .map(|held| {
                        state
                            .subscribers
                            .iter()
                            .filter(|subscriber| hears_its_proxy(&state, held, subscriber))
                            .map(|subscriber| subscriber.link)
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            _ => None,
        };
        let a_download = matches!(pushed, Pushed::EngineDownload { .. });
        state.subscribers.retain(|subscriber| {
            if hearers
                .as_ref()
                .is_some_and(|hearers| !hearers.contains(&subscriber.link))
                || (a_download && !subscriber.downloads)
            {
                return !subscriber.told.is_closed();
            }
            match subscriber.told.try_send(pushed.clone()) {
                Ok(()) => true,
                // A download's progress a reader has no room for is passed over: the next one
                // says more, so it never costs the link its events.
                Err(mpsc::error::TrySendError::Full(_)) if a_download => true,
                Err(_) => false,
            }
        });
    }

    /// Tell `pushed` to every subscriber whose link holds the project `at` that place, letting go
    /// of one that is gone or has stopped reading: whether one was told. One clone's news is
    /// never told to the window of another (#2178). How saving goes, its saver's question of
    /// who is mid-turn included, is `for_savers`: told only to a link that may save the project
    /// ([`Power::Save`]).
    fn tell_holders(state: &mut State, at: &HeldAt, pushed: &Pushed, for_savers: bool) -> bool {
        let holders: Vec<u64> = state
            .projects
            .get(at)
            .map(|held| held.links.keys().copied().collect())
            .unwrap_or_default();
        let mut told = false;
        state.subscribers.retain(|subscriber| {
            if (for_savers && !subscriber.saves) || !holders.contains(&subscriber.link) {
                return true;
            }
            let sent = subscriber.told.try_send(pushed.clone()).is_ok();
            told |= sent;
            sent
        });
        told
    }

    /// Asks the window which chats are mid-turn in `workspace` of the project held `at`, and
    /// waits for its answer, at most `within`: `None` with no window holding the project there
    /// linked, or none that answered (#2002). Asked of the links that may save the project and
    /// hold it there ([`Self::tell_holders`]), which alone answer it.
    fn ask_the_window(
        &self,
        at: &HeldAt,
        workspace: &str,
        within: Duration,
    ) -> Option<Vec<String>> {
        let (answered, answer) = std::sync::mpsc::channel();
        let question = {
            let mut state = self.state();
            state.asked += 1;
            let question = state.asked;
            let pushed = Pushed::WhoIsMidTurn {
                question,
                project: at.project.clone(),
                workspace: workspace.to_owned(),
            };
            if !Self::tell_holders(&mut state, at, &pushed, true) {
                return None;
            }
            state.questions.insert(question, answered);
            question
        };
        let said = answer.recv_timeout(within).ok().flatten();
        self.state().questions.remove(&question);
        said
    }

    /// Push line `id` that the hooks of the project held `at` wrote to every subscriber whose
    /// link holds it there: whether one was told.
    fn hand_on(&self, at: &HeldAt, id: u64, line: serde_json::Value) -> bool {
        // This host's channel records each line under the project as it holds it, whatever
        // path the window names, so the window need name none (1.40, #3400).
        let pushed = Pushed::Heard {
            project: at.project.clone(),
            heard: id,
            line,
            pathless: true,
        };
        Self::tell_holders(&mut self.state(), at, &pushed, false)
    }

    /// What hands a line the hooks of the project held `at` wrote on to whoever holds it
    /// there: nothing, once this host has gone.
    fn handing_on(&self, at: &HeldAt) -> HandOn {
        let host = Arc::downgrade(&self.inner);
        let at = at.clone();
        Arc::new(move |id, line| {
            host.upgrade()
                .is_some_and(|inner| Hosting { inner }.hand_on(&at, id, line))
        })
    }

    /// The hook channel of the project held `at`, where it has one.
    fn channel(&self, at: &HeldAt) -> Option<Arc<dyn Channel>> {
        self.state()
            .projects
            .get(at)
            .and_then(|held| held.channel.clone())
    }

    /// **Whether `peer` may act on browser `program`** (#2050): a link between hosts acts only
    /// on a browser opened over such a link for the window it names, never on one this host's
    /// own window opened. Any other link acts on any it may reach, as before.
    fn opened_for(&self, program: &str, peer: &Peer) -> Result<(), Refusal> {
        if peer.scope() != Scope::RemoteLink {
            return Ok(());
        }
        let state = self.state();
        let Some(hosted) = state.programs.get(program) else {
            return Ok(());
        };
        if hosted.opened_by == OpenedBy::Relayed(relayed_for(&state, peer)) {
            return Ok(());
        }
        Err(no_such_program(program))
    }

    fn running(&self, program: &str) -> Result<Arc<dyn Running>, Refusal> {
        self.state()
            .programs
            .get(program)
            .map(|hosted| Arc::clone(&hosted.running))
            .ok_or_else(|| no_such_program(program))
    }

    /// The programs it holds, as `version` lists them: a browser only from 1.6 on, so a client
    /// one version behind never takes one for a chat.
    fn list(&self, version: Version) -> Vec<Chat> {
        let knows_browsers = version >= Version { major: 1, minor: 6 };
        let knows_drivers = version
            >= Version {
                major: 1,
                minor: 14,
            };
        let state = self.state();
        let mut chats: Vec<Chat> = state
            .programs
            .iter()
            .filter(|(_, hosted)| knows_browsers || hosted.kind != ProgramKind::Browser)
            .map(|(id, hosted)| Chat {
                chat: id.clone(),
                project: hosted.at.project.clone(),
                project_path: state
                    .projects
                    .contains_key(&hosted.at)
                    .then(|| hosted.at.path.display().to_string()),
                title: None,
                state: if hosted.ended { "ended" } else { "running" }.into(),
                number: (hosted.kind != ProgramKind::Browser).then_some(hosted.number),
                run: hosted.run.clone(),
                kind: (hosted.kind == ProgramKind::Browser).then_some(ProgramKind::Browser),
                driver: hosted.running.driver().filter(|_| knows_drivers),
            })
            .collect();
        chats.sort_by(|a, b| (&a.project, a.number).cmp(&(&b.project, b.number)));
        chats
    }

    fn hold(&self, project: String, path: String, peer: &Peer) -> Result<Answer, Refusal> {
        let given = if path.is_empty() {
            // Where this host keeps it (1.34, #2046), made there if it is not.
            let kept = self.kept_place(&project)?;
            std::fs::create_dir_all(&kept).map_err(|why| Refusal {
                code: session::NOT_STARTED.into(),
                why: format!("the folder for project {project} could not be made: {why}"),
            })?;
            kept
        } else {
            PathBuf::from(path)
        };
        // One folder is one place however its path is spelled.
        let path = match std::fs::canonicalize(&given) {
            Ok(path) if given.is_absolute() && path.is_dir() => path,
            _ => {
                return Err(Refusal {
                    code: session::NO_SUCH_PROJECT.into(),
                    why: format!(
                        "{} is not a folder on this machine, so project {project} is not held",
                        given.display()
                    ),
                });
            }
        };
        let link = link_of(peer);
        let at = HeldAt { project, path };
        let mut state = self.state();
        // A project held is where this link first held it until the link lets go: a link holds
        // each project at one place, and no hold moves where another link holds it (#2178).
        if let Some(elsewhere) =
            held_by(&state, &at.project, link).filter(|held| held.path != at.path)
        {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "this link holds project {} at {}, not at {}",
                    at.project,
                    elsewhere.path.display(),
                    at.path.display()
                ),
            });
        }
        if !state.projects.contains_key(&at) {
            // Opened under the hold, so two holds of a place never open two channels on one
            // socket. Opening binds a socket: no wait on anything else.
            let channel = match self
                .inner
                .hear
                .open(&at.project, &at.path, self.handing_on(&at))
            {
                Ok(channel) => channel,
                Err(why) => {
                    crate::say(&format!(
                        "project {} at {}: its hooks are not heard, so its chats' states and \
                         asks will not reach the window: {why}",
                        at.project,
                        at.path.display()
                    ));
                    None
                }
            };
            state.projects.insert(
                at.clone(),
                Held {
                    links: HashMap::new(),
                    channel,
                },
            );
        }
        let Some(held) = state.projects.get_mut(&at) else {
            return Err(no_such_project(&at.project));
        };
        *held.links.entry(link).or_default() += 1;
        state.unheld_since.remove(&at);
        if peer.scope() == Scope::LocalUi {
            note_a_window(&mut state, link);
        }
        drop(state);
        self.inner.changed.notify_one();
        Ok(Answer::Done)
    }

    fn let_go(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let link = link_of(peer);
        let mut state = self.state();
        // A link lets go only of its own holds.
        let Some(at) = held_by(&state, project, link).cloned() else {
            return Err(no_such_project(project));
        };
        let Some(held) = state.projects.get_mut(&at) else {
            return Err(no_such_project(project));
        };
        match held.links.get_mut(&link) {
            Some(holds) if *holds > 1 => *holds -= 1,
            Some(_) => {
                held.links.remove(&link);
            }
            None => return Err(no_such_project(project)),
        }
        let gone = held.links.is_empty().then(|| state.projects.remove(&at));
        // Nobody holds it now: it is saved no more. A link that ended is not a let go
        // ([`Self::let_go_of_the_links`]), so a crashed window's project goes on saving.
        let saving = gone.is_some().then(|| state.saving.remove(&at)).flatten();
        if gone.is_some() {
            state.unheld_since.remove(&at);
        }
        // Its hook channel stops outside the hold: a line it is handing on waits for the hold.
        drop(state);
        drop(gone);
        if let Some(saving) = saving {
            tokio::task::spawn_blocking(move || drop(saving));
        }
        Ok(Answer::Done)
    }

    /// A link ended: its holds go with it. A project that has a program the window has not
    /// closed, running or ended, is kept with its hook channel and its board, held by no link, so
    /// its chats are heard and a window that holds it again reads them as they are, how each
    /// ended included (#2194).
    fn let_go_of_the_links(&self, peer: &Peer) {
        let link = link_of(peer);
        let mut no_window: Vec<Arc<dyn Channel>> = Vec::new();
        let gone: Vec<Held> = {
            let mut state = self.state();
            let held = std::mem::take(&mut state.projects);
            // Every program not yet closed, running or ended: closing is what takes one out.
            let unclosed: HashSet<HeldAt> = state
                .programs
                .values()
                .map(|hosted| hosted.at.clone())
                .collect();
            let (kept, gone): (Vec<_>, Vec<_>) = held
                .into_iter()
                .map(|(at, mut held)| {
                    // This link was the last that held it: its channel is told no window does.
                    if held.links.remove(&link).is_some() && held.links.is_empty() {
                        no_window.extend(held.channel.clone());
                    }
                    (at, held)
                })
                .partition(|(at, held)| !held.links.is_empty() || unclosed.contains(at));
            // Kept by no link from now, or saved by no link: each is let go of when its grace
            // period passes, where the host waits for its window (#2006, #2262).
            let now = Instant::now();
            for (at, held) in &kept {
                if held.links.is_empty() {
                    state.unheld_since.entry(at.clone()).or_insert(now);
                }
            }
            for (at, _) in &gone {
                if state.saving.contains_key(at) {
                    state.unheld_since.entry(at.clone()).or_insert(now);
                }
            }
            state.projects = kept.into_iter().collect();
            gone.into_iter().map(|(_, held)| held).collect()
        };
        // Their hook channels stop outside the hold, as a let go's does; the ones kept are told
        // outside it too.
        drop(gone);
        for channel in no_window {
            channel.no_window();
        }
    }

    /// Whether `start`, from `peer`, is let through the kill switch: nothing is while it is
    /// thrown, but a shell the window opens.
    fn let_through(&self, start: &StartProgram, peer: &Peer) -> Result<(), Refusal> {
        let the_operators_shell =
            start.kind == ProgramKind::Shell && peer.scope() == Scope::LocalUi;
        if self.inner.switch.is_stopped() && !the_operators_shell {
            return Err(Refusal {
                code: session::STOPPED.into(),
                why: self.inner.switch.refusal(),
            });
        }
        Ok(())
    }

    /// Throws the kill switch: kept first, so nothing starts from here on, then every program
    /// the host runs is ended, all at once. Each stays, with its last screen, until closed.
    async fn stop_all(&self) -> Answer {
        let switch = self.inner.switch.stop();
        let programs: Vec<Arc<dyn Running>> = self
            .state()
            .programs
            .values()
            .filter(|hosted| !hosted.ended)
            .map(|hosted| {
                stopping_by(&hosted.stopping, Stopped::Killed);
                Arc::clone(&hosted.running)
            })
            .collect();
        let _ = tokio::task::spawn_blocking(move || stop_together(programs)).await;
        Answer::KillSwitch(switch)
    }

    async fn start(&self, start: StartProgram, peer: &Peer) -> Result<Answer, Refusal> {
        // A stop written elsewhere since the last look is heard before anything starts (#2190).
        self.heed_a_stop_written_elsewhere().await;
        self.let_through(&start, peer)?;
        a_note_that_fits(start.note.as_ref())?;
        let start = ids_of(start, peer.scope());
        let size = Size {
            columns: start.cols.unwrap_or(80),
            rows: start.rows.unwrap_or(24),
        }
        .at_least_min();
        // The number and the id are taken under one hold, so two starts never share either.
        let (program, number, at) = {
            let mut state = self.state();
            let at = held_at(&state, &start.project, link_of(peer))?;
            let program = match &start.program {
                Some(id) if state.programs.contains_key(id) => {
                    return Err(Refusal {
                        code: session::MALFORMED.into(),
                        why: format!("this host already holds a program {id}"),
                    });
                }
                Some(id) => id.clone(),
                None => ulid::Ulid::generate().to_string(),
            };
            let in_use = |number: u32, state: &State| {
                state
                    .programs
                    .values()
                    .any(|hosted| hosted.at == at && hosted.number == number)
            };
            // The confinement a start the host composes is to have, named to this link (#2407):
            // never one beside the window's own launch, nor an id the host already holds.
            // What the window allowed it alone is left out of the confinement composed for it,
            // and asked about over the link that confinement is named to (#2590).
            if !start.alone.is_empty() && (start.confinement.is_none() || start.launch.is_some()) {
                return Err(Refusal {
                    code: session::MALFORMED.into(),
                    why: "a start names what the window allowed the chat alone only beside the \
                          confinement the host composes for it"
                        .into(),
                });
            }
            if let Some(named) = &start.confinement {
                if start.launch.is_some() {
                    return Err(Refusal {
                        code: session::MALFORMED.into(),
                        why: "a start with its own launch names its confinement in the launch"
                            .into(),
                    });
                }
                if state.confinements.contains_key(named) {
                    return Err(already_confined(named));
                }
            }
            // The confinement it runs in becomes the program's, under the same hold, so no two
            // programs run in one. Only one this link made for this project, and nothing runs in
            // yet.
            if let Some(confinement) = start.launch.as_ref().and_then(|l| l.confinement.as_ref()) {
                let links = links_of_window(&state, relayed_for(&state, peer).as_deref());
                let link = link_of(peer);
                match state.confinements.get_mut(confinement) {
                    // Made by this link, or by another link of its window (#2624).
                    Some(held)
                        if held.project == start.project
                            && matches!(held.owner, Owner::Link(made_by)
                                if made_by == link || links.contains(&made_by)) =>
                    {
                        held.owner = Owner::Program(program.clone());
                        // A chat is its window's on each of that window's links (#2511).
                        held.named_to.extend(links);
                    }
                    _ => {
                        return Err(not_started(format!(
                            "this session host holds no confinement {confinement} for a new \
                             program in project {}",
                            start.project
                        )));
                    }
                }
            }
            let dealt = state.dealt.get(&at).copied().unwrap_or(0);
            let number = match start.number {
                Some(wanted) if wanted > 0 && !in_use(wanted, &state) => wanted,
                _ => match dealt.checked_add(1) {
                    Some(next) if !in_use(next, &state) => next,
                    _ => return Err(no_number_left(&start.project)),
                },
            };
            state.dealt.insert(at.clone(), dealt.max(number));
            (program, number, at)
        };
        // Its token, issued by the project's hook channel and composed into its start: the
        // window's launch says where it runs, never where it reports (#2000).
        let channel = self.channel(&at);
        let mut start = start;
        // The id it runs under, as its composition names it.
        start.program = Some(program.clone());
        let started_by = peer.scope();
        let reporting = match channel.as_ref().map(|channel| channel.report_to(number)) {
            Some(Ok(reporting)) => Some(reporting),
            Some(Err(why)) => {
                self.end_confinements_of(&program);
                return Err(not_started(format!(
                    "chat {number} could not be issued its token: {why}"
                )));
            }
            None => None,
        };
        if let (Some(reporting), Some(launch)) = (&reporting, start.launch.as_mut()) {
            launch.reporting = Some(reporting.clone());
        }
        // On every way it fails, nothing runs in its confinement: that ends with the start.
        let forget = |channel: &Option<Arc<dyn Channel>>| {
            if let Some(channel) = channel {
                channel.forget(number);
            }
        };
        // Composed off the runtime: a start the host composes itself reads the project's files
        // and starts its confinement (#2171).
        let composed = {
            let hosting = self.clone();
            let start = start.clone();
            let link = link_of(peer);
            let window = relayed_for(&self.state(), peer);
            let at = at.clone();
            let program = program.clone();
            tokio::task::spawn_blocking(move || {
                let beside = ForTheProgram {
                    hosting: hosting.clone(),
                    program,
                    at: at.clone(),
                    named: Mutex::new(start.confinement.clone().map(|confinement| NamedTo {
                        confinement,
                        link,
                        window,
                        alone: start.alone.clone(),
                    })),
                };
                hosting.inner.compose.compose(&ToCompose {
                    project: &at.project,
                    project_path: &at.path,
                    number,
                    size,
                    start: &start,
                    reporting: reporting.as_ref(),
                    started_by,
                    beside: &beside,
                })
            })
            .await
            .map_err(|why| why.to_string())
            .and_then(|composed| composed)
        };
        let spec = match composed {
            Ok(spec) => spec,
            Err(why) => {
                forget(&channel);
                self.end_confinements_of(&program);
                return Err(not_started(why));
            }
        };
        // Its link ended while it was being composed: the name it gave the confinement composed
        // for it goes with the link, as the link's end took every other (#2407). Checked after
        // that confinement is kept, so either this or the link's end finds it.
        if peer.has_ended() {
            let unheld = unname(&mut self.state(), peer, |owner| {
                *owner == Owner::Program(program.clone())
            });
            let weak = Arc::downgrade(&self.inner);
            off_the_runtime(move || withdraw(&weak, unheld));
        }
        // Where it runs, for a start the host composed: what its project's saver waits on.
        let cwd = spec.cwd.clone();
        // How it runs sandboxed, read from what is started and the confinement that holds it
        // now, never from the start (#2636).
        let sandbox = {
            let confined = self
                .state()
                .confinements
                .values()
                .any(|held| held.owner == Owner::Program(program.clone()));
            self.inner.compose.sandbox_of(&spec, confined)
        };
        // Starting a program opens a terminal and forks: off the runtime.
        let hosting = self.clone();
        let running = match tokio::task::spawn_blocking(move || hosting.inner.run.run(spec)).await {
            Ok(Ok(running)) => running,
            Ok(Err(why)) => {
                forget(&channel);
                self.end_confinements_of(&program);
                return Err(not_started(why));
            }
            Err(why) => {
                forget(&channel);
                self.end_confinements_of(&program);
                return Err(not_started(why.to_string()));
            }
        };
        let process = running.process();
        // Its lines are read from its program, and from what runs inside it.
        if let (Some(channel), Some(pid)) = (&channel, process) {
            channel.bind(number, pid);
        }
        let stopping: Arc<Mutex<Option<Stopped>>> = Arc::default();
        self.state().programs.insert(
            program.clone(),
            Hosted {
                kind: start.kind,
                at: at.clone(),
                number,
                run: start.run.clone(),
                running: Arc::clone(&running),
                size,
                process,
                cwd: start
                    .launch
                    .as_ref()
                    .and_then(|launch| launch.cwd.as_ref())
                    .map(PathBuf::from)
                    .or(cwd),
                ended: false,
                exit: None,
                stopping: Arc::clone(&stopping),
                sandbox: sandbox.clone(),
                note: start.note.clone(),
                logins: Arc::default(),
                opened_by: OpenedBy::ThisHost,
            },
        );
        // Written down before the start is answered, so a host killed from here on leaves it
        // for the next to settle (#2007).
        if let Some(host_record) = &self.inner.host_record {
            host_record.started(Started {
                program: program.clone(),
                project: start.project.clone(),
                project_path: at.path.clone(),
                number,
                run: start.run.clone(),
                process,
                process_started: running.process_started(),
                boot: crate::hostrecord::this_boot(),
                sandbox: Some(sandbox.clone()),
            });
        }
        // A stop thrown while this one was being started: it is listed by now, so either the
        // stop's sweep reached it or this check sees the switch thrown.
        if self.let_through(&start, peer).is_err() {
            stopping_by(&stopping, Stopped::Killed);
            let ending = Arc::clone(&running);
            let _ = tokio::task::spawn_blocking(move || ending.stop()).await;
        }
        let hosting = self.clone();
        let ended = program.clone();
        let run = start.run.clone();
        running.when_it_ends(Box::new(move |exit| {
            {
                let mut state = hosting.state();
                if let Some(hosted) = state.programs.get_mut(&ended) {
                    hosted.ended = true;
                    hosted.exit = Some(exit.clone());
                }
                // A program's end is the moment its work is done: its project's saver hears it.
                if let Some(saving) = state.saving.get(&at) {
                    saving.poke(Poke::ProgramEnded);
                }
            }
            if let (Some(channel), Some(pid)) = (&channel, process) {
                channel.ended(number, pid);
            }
            // What its end leaves is kept before anyone is told it ended, window or no window
            // (#2003).
            let stopped = *stopping
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            hosting.record().ended(&Ended {
                project_path: &at.path,
                number,
                exit: exit.clone(),
                stopped,
                watched: hosting.watched(&at),
            });
            // With no window to tell its board, the board the host keeps is told (#2194).
            if let Some(channel) = channel.as_ref().filter(|_| !hosting.watched(&at)) {
                channel.ended_with_no_window(number, &exit);
            }
            // Only once its end is kept: a host killed before then leaves it to be settled.
            if let Some(host_record) = &hosting.inner.host_record {
                host_record.ended(&ended);
            }
            hosting.tell(&Pushed::Ended {
                program: ended,
                run,
                exit,
            });
        }));
        // Where the confinement composed for it is, where the start named one to this link and
        // its composition started one (#2407).
        let confined = start.confinement.as_ref().and_then(|id| {
            let state = self.state();
            state
                .confinements
                .get(id)
                .filter(|held| held.owner == Owner::Program(program.clone()))
                .map(|held| Confined {
                    confinement: id.clone(),
                    ..held.confining.confined()
                })
        });
        // How it runs sandboxed, as recorded above: what a window that started it by its names
        // judges it by (1.29, #2692).
        Ok(Answer::ProgramStarted {
            program,
            number: Some(number),
            process,
            confined,
            sandbox: Some(sandbox),
        })
    }

    /// Start a browser (#2012): in a project this host holds, with a fresh data folder of its
    /// own under the host's store, removed once it has ended.
    async fn start_browser(&self, start: StartProgram, peer: &Peer) -> Result<Answer, Refusal> {
        // Nothing starts while the kill switch is thrown (ADR 0071), a browser included.
        self.heed_a_stop_written_elsewhere().await;
        self.let_through(&start, peer)?;
        a_note_that_fits(start.note.as_ref())?;
        // Read before the lock: it may read this machine's store.
        let budget = self.inner.browsers.budget();
        let (program, at, place, download) = {
            let mut state = self.state();
            let at = held_at(&state, &start.project, link_of(peer))?;
            let program = match &start.program {
                // A start still downloading its engine holds its id too (#2207).
                Some(id) if state.programs.contains_key(id) || state.downloads.contains_key(id) => {
                    return Err(Refusal {
                        code: session::MALFORMED.into(),
                        why: format!("this host already holds a program {id}"),
                    });
                }
                Some(id) if !a_plain_name(id) => {
                    return Err(Refusal {
                        code: session::MALFORMED.into(),
                        why: "a program's id is letters, digits, `-` and `_`".into(),
                    });
                }
                Some(id) => id.clone(),
                None => ulid::Ulid::generate().to_string(),
            };
            let place = PlaceInTheBudget::taken(self, &mut state, budget)?;
            // Its engine's download, told to the windows as it goes and cancelled by one
            // (#2207): its id taken under the same hold as the check above.
            let download = {
                let telling = self.clone();
                let of = program.clone();
                crate::browsers::Download::telling(Arc::new(move |done, total| {
                    telling.tell(&Pushed::EngineDownload {
                        program: of.clone(),
                        done,
                        total,
                    });
                }))
            };
            state.downloads.insert(program.clone(), download.clone());
            (program, at, place, download)
        };
        // Starting an engine forks and waits for it to answer: off the runtime.
        let hosting = self.clone();
        let named = program.clone();
        let reach = start.reach.clone();
        let kept = start.kept.clone();
        let started = tokio::task::spawn_blocking(move || {
            let data = hosting
                .inner
                .browsers
                .data_folders()
                .map_err(crate::browsers::NotStarted::Failed)?
                .join(&named);
            let started =
                hosting
                    .inner
                    .browsers
                    .start(&data, reach.as_ref(), kept.as_ref(), &download);
            hosting.state().downloads.remove(&named);
            match started {
                Ok(screen) => Ok((screen, data)),
                Err(why) => {
                    let _ = std::fs::remove_dir_all(&data);
                    Err(why)
                }
            }
        })
        .await
        .map_err(|why| not_started(why.to_string()))
        .and_then(|started| started.map_err(browser_not_started));
        let (screen, data) = started?;
        let (columns, rows) = screen.size();
        // The host's own act that ends it, for the line its end leaves in the host's log
        // (#2745); a browser's end leaves nothing in the event log.
        let stopping: Arc<Mutex<Option<Stopped>>> = Arc::default();
        let running: Arc<dyn Running> =
            Arc::new(Browser::new(screen, program.clone(), Arc::clone(&stopping)));
        let process = running.process();
        let mut state = self.state();
        let opened_by = opened_by(&state, peer);
        // From here it counts as running: its place is given back in the same step.
        place.started(&mut state);
        state.programs.insert(
            program.clone(),
            Hosted {
                kind: ProgramKind::Browser,
                at,
                number: 0,
                run: start.run.clone(),
                running: Arc::clone(&running),
                size: Size { columns, rows },
                process,
                // A browser writes no project's files: its project's saver does not wait for it.
                cwd: None,
                ended: false,
                exit: None,
                stopping,
                // An engine the host starts outside every chat's sandbox, its reach its own
                // proxy's: no chat's run.
                sandbox: session::RunSandbox::Off,
                note: start.note.clone(),
                logins: Arc::new(Mutex::new(
                    start
                        .kept
                        .iter()
                        .map(|kept| kept.identity.clone())
                        .collect(),
                )),
                opened_by,
            },
        );
        drop(state);
        let hosting = self.clone();
        let ended = program.clone();
        let run = start.run.clone();
        running.when_it_ends(Box::new(move |exit| {
            if let Some(hosted) = hosting.state().programs.get_mut(&ended) {
                hosted.ended = true;
                hosted.exit = Some(exit.clone());
            }
            // A fresh browser keeps nothing once it has ended.
            let _ = std::fs::remove_dir_all(&data);
            hosting.tell(&Pushed::Ended {
                program: ended,
                run,
                exit,
            });
        }));
        Ok(Answer::ProgramStarted {
            program,
            number: None,
            process,
            confined: None,
            sandbox: Some(session::RunSandbox::Off),
        })
    }

    /// One act of browser `program`'s (#2013), carried out off the runtime, since it waits on
    /// the engine: what the browser answered, or its driver's refusal in its words.
    async fn browse(
        &self,
        program: &str,
        call_id: String,
        act: purlis_session_protocol::browsing::Act,
        reach: Option<purlis_session_protocol::browsing::BrowserReach>,
    ) -> Result<Answer, Refusal> {
        let running = self.running(program)?;
        tokio::task::spawn_blocking(move || {
            // What it reaches is what the window says now, before the act (#2018).
            if let Some(reach) = &reach {
                running
                    .reach_now(reach)
                    .map_err(crate::browsers::NotActed::Refused)?;
            }
            running.act(&call_id, act)
        })
        .await
        .map_err(|why| not_started(why.to_string()))?
        .map(|browsed| Answer::Browsed(Box::new(browsed)))
        .map_err(not_acted)
    }

    /// purlis performs `login` in browser `program` (#2016), off the runtime: it waits on the
    /// page.
    async fn log_in(
        &self,
        program: &str,
        login: purlis_session_protocol::browsing::LogIn,
    ) -> Result<Answer, Refusal> {
        let running = self.running(program)?;
        // Recorded before it is tried, whatever it comes to: a login that timed out may have
        // signed the browser in all the same (#2025).
        if let Some(hosted) = self.state().programs.get(program) {
            let identity = login
                .identity
                .clone()
                .or_else(|| login.keep.as_ref().map(|kept| kept.identity.clone()))
                .unwrap_or_default();
            let mut logins = hosted
                .logins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !logins.contains(&identity) {
                logins.push(identity);
            }
        }
        tokio::task::spawn_blocking(move || running.log_in(login))
            .await
            .map_err(|why| not_started(why.to_string()))?
            .map(|browsed| Answer::Browsed(Box::new(browsed)))
            .map_err(not_acted)
    }

    /// One of the person's commands on browser `program` (#2014), carried out off the runtime.
    async fn driving(
        &self,
        program: &str,
        command: impl FnOnce(&dyn Running) -> Result<Answer, Refusal> + Send + 'static,
    ) -> Result<Answer, Refusal> {
        let running = self.running(program)?;
        tokio::task::spawn_blocking(move || command(&*running))
            .await
            .map_err(|why| not_started(why.to_string()))?
    }

    async fn attach(
        &self,
        program: &str,
        kind: ViewKind,
        role: Option<ViewRole>,
        rate: Option<u32>,
        peer: &Peer,
    ) -> Result<Answer, Refusal> {
        let (source, size) = {
            let state = self.state();
            let hosted = state
                .programs
                .get(program)
                .ok_or_else(|| no_such_program(program))?;
            let reads = crate::browsers::Reads::of(peer.version(), peer.scope());
            let source = hosted
                .running
                .view(kind, role, rate, reads)
                .ok_or_else(|| Refusal {
                    code: session::MALFORMED.into(),
                    why: format!("program {program} has no view of that kind"),
                })?;
            let size = match hosted.running.view_size() {
                Some((columns, rows)) => Size { columns, rows },
                None => hosted.size,
            };
            (source, size)
        };
        let view = peer.views().open(source).await.map_err(|why| Refusal {
            code: session::NOT_STARTED.into(),
            why: format!("the view could not be opened: {why}"),
        })?;
        Ok(Answer::Attached(Attached {
            view: view.0,
            cols: size.columns,
            rows: size.rows,
            kind: Some(kind),
        }))
    }

    /// **`note` is the window's note of `program` from now on** (1.39, #2025): kept, never read,
    /// and answered with the program to a window that comes back. One past its bound is refused
    /// and the one before is kept.
    fn note_program(
        &self,
        program: &str,
        note: session::ProgramNote,
        peer: &Peer,
    ) -> Result<Answer, Refusal> {
        a_note_that_fits(Some(&note))?;
        let mut state = self.state();
        // Only of a program in a project the link holds.
        let at = state
            .programs
            .get(program)
            .map(|hosted| hosted.at.clone())
            .ok_or_else(|| no_such_program(program))?;
        if !state
            .projects
            .get(&at)
            .is_some_and(|held| held.links.contains_key(&link_of(peer)))
        {
            return Err(no_such_program(program));
        }
        if let Some(hosted) = state.programs.get_mut(program) {
            hosted.note = Some(note);
        }
        Ok(Answer::Done)
    }

    /// Every harness and shell `project` still has, running or ended, at the place the link
    /// `peer` came on holds it, which it must (#2006): what a window that comes back takes back,
    /// each with the place of the confinement it runs in. Each confinement it answers is named to
    /// that link (#2444), which may act on it and hear it from then on. Its browsers too, to a
    /// link of 1.39 or later (#2025), each with who drives it; and each program with the note
    /// the window keeps of it here.
    fn programs_of(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let knows_browsers = peer.version() >= session::KNOWS_BROWSERS_TAKEN_BACK;
        let mut state = self.state();
        let at = held_by(&state, project, link_of(peer))
            .cloned()
            .ok_or_else(|| no_such_project(project))?;
        let mut programs: Vec<HeldProgram> = state
            .programs
            .iter()
            .filter(|(_, hosted)| {
                hosted.at == at && (knows_browsers || hosted.kind != ProgramKind::Browser)
            })
            .map(|(id, hosted)| HeldProgram {
                program: id.clone(),
                kind: hosted.kind,
                number: hosted.number,
                process: (!hosted.ended).then_some(hosted.process).flatten(),
                run: hosted.run.clone(),
                exit: hosted.exit.clone(),
                confined: state
                    .confinements
                    .iter()
                    .find(|(_, held)| held.owner == Owner::Program(id.clone()))
                    .map(|(confinement, held)| Confined {
                        confinement: confinement.clone(),
                        ..held.confining.confined()
                    }),
                plan: state
                    .confinements
                    .values()
                    .find(|held| held.owner == Owner::Program(id.clone()))
                    .map(|held| held.plan.clone()),
                sandbox: Some(hosted.sandbox.clone()),
                cols: hosted.size.columns,
                rows: hosted.size.rows,
                driver: hosted.running.driver().filter(|_| knows_browsers),
                note: hosted.note.clone().filter(|_| knows_browsers),
                logins: if knows_browsers {
                    hosted
                        .logins
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone()
                } else {
                    Vec::new()
                },
            })
            .collect();
        programs.sort_by(|a, b| (a.number, &a.program).cmp(&(b.number, &b.program)));
        // Each confinement answered is named to the window taking its chat back (#2444).
        for confined in programs
            .iter()
            .filter_map(|program| program.confined.as_ref())
        {
            if let Some(held) = state.confinements.get_mut(&confined.confinement) {
                held.named_to.insert(link_of(peer));
            }
        }
        Ok(Answer::Programs(programs))
    }

    /// **`peer`'s link is `window`'s** (#2511, ruled 2026-10-17): the relaying host names the
    /// window on each link it makes for it, and the host names this link each confinement of a
    /// chat that window started here over the links before, made or composed for them, and
    /// answers with their ids, and one a link of that window made that nothing runs in yet, while
    /// that link lasts (#2624). It names no other: not one the host's own window made, nor one
    /// made or composed for another window's link. A link that named another window is refused.
    fn take_back(&self, window: String, peer: &Peer) -> Result<Answer, Refusal> {
        let link = link_of(peer);
        let mut state = self.state();
        match state.relayed_for.get(&link) {
            Some(named) if *named != window => {
                return Err(Refusal {
                    code: session::MALFORMED.into(),
                    why: format!("this link is window {named}'s, so it is not window {window}'s"),
                });
            }
            Some(_) => {}
            None => {
                state.relayed_for.insert(link, window.clone());
            }
        }
        let mut taken: Vec<String> = Vec::new();
        let relayed_for = state.relayed_for.clone();
        for (id, held) in &mut state.confinements {
            if held.window.as_deref() != Some(window.as_str()) {
                continue;
            }
            // A chat's, or one a link of this window made that nothing runs in yet (#2624).
            let its_windows = match &held.owner {
                Owner::Program(_) => true,
                Owner::Link(made_by) => relayed_for.get(made_by) == Some(&window),
            };
            if its_windows {
                held.named_to.insert(link);
                taken.push(id.clone());
            }
        }
        taken.sort();
        // Each port withdrawn as its last link ended is given back, as the person gave it
        // (#2625).
        let giving: Vec<String> = taken
            .iter()
            .filter(|id| {
                state
                    .confinements
                    .get(*id)
                    .is_some_and(|held| !held.unheld.is_empty())
            })
            .cloned()
            .collect();
        drop(state);
        if !giving.is_empty() {
            let weak = Arc::downgrade(&self.inner);
            off_the_runtime(move || {
                for id in giving {
                    give_back(&weak, &id);
                }
            });
        }
        Ok(Answer::TakenBack {
            confinements: taken,
        })
    }

    /// The window quit (#2006, ADR 0068 §2): every program ends as a quit would, all at once,
    /// and the answer comes once they are gone; then whoever serves this host stops
    /// ([`Self::until_it_quits`]).
    async fn quit(&self) -> Answer {
        let hosting = self.clone();
        let _ = tokio::task::spawn_blocking(move || hosting.end_all()).await;
        self.inner.quit.send_replace(true);
        Answer::Done
    }

    /// Resolves once the window has quit: the host stops serving.
    pub async fn until_it_quits(&self) {
        let mut quit = self.inner.quit.subscribe();
        let _ = quit.wait_for(|quit| *quit).await;
    }

    fn resize(&self, program: &str, size: Size) -> Result<Answer, Refusal> {
        let size = size.at_least_min();
        let running = {
            let mut state = self.state();
            let hosted = state
                .programs
                .get_mut(program)
                .ok_or_else(|| no_such_program(program))?;
            hosted.size = size;
            Arc::clone(&hosted.running)
        };
        running.resize(size).map_err(not_started)?;
        Ok(Answer::Done)
    }

    async fn stop(&self, program: &str) -> Result<Answer, Refusal> {
        let running = {
            let state = self.state();
            let hosted = state
                .programs
                .get(program)
                .ok_or_else(|| no_such_program(program))?;
            stopping_by(&hosted.stopping, Stopped::Operator);
            Arc::clone(&hosted.running)
        };
        let _ = tokio::task::spawn_blocking(move || running.stop()).await;
        Ok(Answer::Done)
    }

    /// Deals the next number in held `project`, at the place `peer`'s link holds it (#2000): it
    /// only rises, so no chat there has had it.
    fn deal(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let mut state = self.state();
        let at = held_at(&state, project, link_of(peer))?;
        let dealt = state.dealt.entry(at).or_default();
        *dealt = dealt
            .checked_add(1)
            .ok_or_else(|| no_number_left(project))?;
        Ok(Answer::Dealt { number: *dealt })
    }

    /// The numbers up to `dealt` went out in held `project`, at the place `peer`'s link holds
    /// it, before: none of them is dealt again.
    fn dealt_already(&self, project: &str, dealt: u32, peer: &Peer) -> Result<Answer, Refusal> {
        let mut state = self.state();
        let at = held_at(&state, project, link_of(peer))?;
        let was = state.dealt.entry(at).or_default();
        *was = (*was).max(dealt);
        Ok(Answer::Done)
    }

    /// The hook channel of `project`, where it has one, for the link `peer` came on, which must
    /// hold the project.
    fn held_channel(
        &self,
        project: &str,
        peer: &Peer,
    ) -> Result<Option<Arc<dyn Channel>>, Refusal> {
        let state = self.state();
        let at = held_by(&state, project, link_of(peer)).ok_or_else(|| no_such_project(project))?;
        Ok(state.projects.get(at).and_then(|held| held.channel.clone()))
    }

    /// The window's reply to line `heard` of `project`'s hooks, from a link that holds it.
    fn reply_heard(
        &self,
        project: &str,
        heard: u64,
        reply: serde_json::Value,
        peer: &Peer,
    ) -> Result<Answer, Refusal> {
        if let Some(channel) = self.held_channel(project, peer)? {
            channel.replied(heard, reply);
        }
        Ok(Answer::Done)
    }

    /// Forgets `program` at once, so its number is free when this answers, and ends it off the
    /// runtime; its views end with its output.
    fn close(&self, program: &str) -> Result<Answer, Refusal> {
        let (hosted, confinements) = {
            let mut state = self.state();
            let hosted = state
                .programs
                .remove(program)
                .ok_or_else(|| no_such_program(program))?;
            stopping_by(&hosted.stopping, Stopped::Closed);
            state.closed.extend(hosted.process);
            let confinements = take_confinements(&mut state, |owner| {
                *owner == Owner::Program(program.to_owned())
            });
            (hosted, confinements)
        };
        // A line for it is not read from now on.
        if let Some(channel) = self.channel(&hosted.at) {
            channel.forget(hosted.number);
        }
        tokio::task::spawn_blocking(move || {
            hosted.running.stop();
            drop(hosted);
            drop(confinements);
        });
        Ok(Answer::Done)
    }

    /// Ends the confinements `program` runs in, off the runtime.
    fn end_confinements_of(&self, program: &str) {
        let ending = take_confinements(&mut self.state(), |owner| {
            *owner == Owner::Program(program.to_owned())
        });
        end_off_the_runtime(ending);
    }

    /// Starts the confinement the window planned (#1999), in a project the host holds, as the
    /// link `peer` came on's until a start names it.
    async fn confine(&self, confine: session::Confine, peer: &Peer) -> Result<Answer, Refusal> {
        let id = confine.confinement.clone();
        let confine_project = confine.project.clone();
        let plan = confine.plan.clone();
        let path = {
            let state = self.state();
            if state.confinements.contains_key(&id) {
                return Err(already_confined(&id));
            }
            held_at(&state, &confine.project, link_of(peer))?.path
        };
        let project_path = path.clone();
        let (tell, askable) = self.heard_as(&id);
        let hosting = self.clone();
        // A plan from any link but this machine's window was made on another machine: it is
        // held to this one's own folders and services before anything starts (#2445).
        let planned = if peer.scope() == Scope::LocalUi {
            Planned::Here
        } else {
            Planned::Elsewhere
        };
        let confining = tokio::task::spawn_blocking(move || {
            let confining = ToConfine {
                project_path: &path,
                confine: &confine,
                planned,
                alone: &[],
            };
            if planned == Planned::Elsewhere {
                hosting
                    .inner
                    .confine
                    .fits(&confining)
                    .map_err(not_fitting)?;
            }
            hosting
                .inner
                .confine
                .confine(&confining, tell, askable)
                .map_err(not_started)
        })
        .await
        .map_err(|why| not_started(why.to_string()))??;
        let confining: Arc<dyn Confining> = Arc::from(confining);
        let mut confined = confining.confined();
        confined.confinement = id.clone();
        let project = confine_project;
        {
            // One started meanwhile under the same id keeps it, and this one ends.
            let mut state = self.state();
            if state.confinements.contains_key(&id) {
                drop(state);
                end_off_the_runtime(vec![confining]);
                return Err(already_confined(&id));
            }
            let window = relayed_for(&state, peer);
            // A window's on each of that window's links (#2624), as a chat is (#2511).
            let mut named_to: HashSet<u64> = std::iter::once(link_of(peer)).collect();
            named_to.extend(links_of_window(&state, window.as_deref()));
            state.confinements.insert(
                id.clone(),
                Confinement {
                    project,
                    path: project_path,
                    owner: Owner::Link(link_of(peer)),
                    named_to,
                    confining,
                    plan,
                    window,
                    unheld: Vec::new(),
                },
            );
        }
        // Its link ended while it was being started: nobody holds it, and it ends now. Checked
        // after it is kept, so either this or the link's end finds it.
        if peer.has_ended() {
            let ending = take_confinements(&mut self.state(), |owner| {
                *owner == Owner::Link(link_of(peer))
            });
            end_off_the_runtime(ending);
            return Err(not_started(format!(
                "the link that asked for confinement {id} ended before it was started"
            )));
        }
        Ok(Answer::Confined(confined))
    }

    /// What confinement `id`'s proxy tells, pushed to the links that may hear it
    /// ([`hears_its_proxy`]), and whether it may hold a connection while the person is asked,
    /// now.
    fn heard_as(&self, id: &str) -> (crate::confining::Tell, crate::confining::Askable) {
        let weak = Arc::downgrade(&self.inner);
        let tell = {
            let (weak, id) = (Weak::clone(&weak), id.to_owned());
            Arc::new(move |told: SandboxTold| {
                if let Some(inner) = weak.upgrade() {
                    Hosting { inner }.tell(&Pushed::Sandbox {
                        confinement: id.clone(),
                        told,
                    });
                }
            })
        };
        let askable = {
            let id = id.to_owned();
            Arc::new(move || {
                weak.upgrade()
                    .is_some_and(|inner| Hosting { inner }.askable(&id))
            })
        };
        (tell, askable)
    }

    /// Whether confinement `id`'s proxy may hold a connection while the person is asked: a
    /// program runs in it, and a window that may answer it is there to be asked (#2444).
    fn askable(&self, id: &str) -> bool {
        let state = self.state();
        let Some(held) = state.confinements.get(id) else {
            return false;
        };
        matches!(held.owner, Owner::Program(_))
            && state.subscribers.iter().any(|subscriber| {
                !subscriber.told.is_closed() && hears_its_proxy(&state, held, subscriber)
            })
    }

    /// Confinement `id`, for a command from `peer`: one it may not act on is one the host does
    /// not hold (#2444).
    fn confinement(&self, id: &str, peer: &Peer) -> Result<Arc<dyn Confining>, Refusal> {
        let state = self.state();
        state
            .confinements
            .get(id)
            .filter(|held| may_act_on(&state, held, link_of(peer)))
            .map(|held| Arc::clone(&held.confining))
            .ok_or_else(|| no_such_confinement(id))
    }

    /// Ends confinement `id`, whatever runs in it, and answers once it has ended: its proxy
    /// carries nothing by the time the client hears so. One the host does not hold is let go
    /// of already; one `peer` may not act on is refused, and goes on (#2444).
    async fn let_go_of_confinement(&self, id: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let ending = {
            let mut state = self.state();
            match state.confinements.get(id) {
                None => return Ok(Answer::Done),
                Some(held) if !may_act_on(&state, held, link_of(peer)) => {
                    return Err(no_such_confinement(id));
                }
                Some(_) => state.confinements.remove(id).map(|held| held.confining),
            }
        };
        if let Some(ending) = ending {
            // A proxy's end waits on its listeners: off the runtime, and waited for.
            let _ = tokio::task::spawn_blocking(move || drop(ending)).await;
        }
        Ok(Answer::Done)
    }
}

/// `link`, a `local-ui` link, is the window's (#2006): the host has a window now.
fn note_a_window(state: &mut State, link: u64) {
    state.windows.insert(link);
    state.had_a_window = true;
    state.windowless_since = None;
}

/// Takes every confinement whose owner `whose` says, out of `state`.
fn take_confinements(state: &mut State, whose: impl Fn(&Owner) -> bool) -> Vec<Arc<dyn Confining>> {
    let ids: Vec<String> = state
        .confinements
        .iter()
        .filter(|(_, held)| whose(&held.owner))
        .map(|(id, _)| id.clone())
        .collect();
    ids.iter()
        .filter_map(|id| state.confinements.remove(id))
        .map(|held| held.confining)
        .collect()
}

/// **The ids a start begins under** (#2171): the chat's own and its run, as the window names
/// them, which it alone keeps for its chats (ADR 0066). On a runner the window's start comes
/// over the link from its one paired desktop, which relays it for the window alone (#2446), so
/// the runner keeps them too, and what it writes of the chat joins the window's own. From every
/// other scope the host mints both, so what the host writes under a chat's id is only ever
/// written for the chat the id was minted for. An id the host already holds is refused either
/// way. A start from the link that names no run, as the desktop's relay sends on the start of
/// a client that is not its window, begins a run the host mints, so its run is written as any
/// other's.
fn ids_of(start: StartProgram, scope: Scope) -> StartProgram {
    match scope {
        Scope::LocalUi => return start,
        Scope::RemoteLink => {
            return StartProgram {
                run: start
                    .run
                    .or_else(|| Some(ulid::Ulid::generate().to_string())),
                ..start
            };
        }
        _ => {}
    }
    StartProgram {
        program: None,
        run: Some(ulid::Ulid::generate().to_string()),
        ..start
    }
}

/// [`Beside`] for program `program`, being started in the project held `at` (#2171).
struct ForTheProgram {
    hosting: Hosting,
    program: String,
    at: HeldAt,
    /// What the start named for the confinement composed for it (#2407): taken by the first
    /// confinement started.
    named: Mutex<Option<NamedTo>>,
}

/// The id a start gave the confinement composed for it, the link it is named to (#2407), the
/// relayed window that link is for (#2511), and what the window allowed the chat alone (#2590).
struct NamedTo {
    confinement: String,
    link: u64,
    window: Option<String>,
    alone: Vec<u16>,
}

impl Beside for ForTheProgram {
    fn confine(&self, plan: &session::ConfinePlan) -> Result<session::Confined, String> {
        let taken = self
            .named
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let (id, mut named_to, window, alone) = match taken {
            Some(NamedTo {
                confinement,
                link,
                window,
                alone,
            }) => (confinement, HashSet::from([link]), window, alone),
            // Named to no link: the host made it, for a chat it composed (#2444).
            None => (
                ulid::Ulid::generate().to_string(),
                HashSet::new(),
                None,
                Vec::new(),
            ),
        };
        let (tell, askable) = self.hosting.heard_as(&id);
        let confining = self.hosting.inner.confine.confine(
            &ToConfine {
                project_path: &self.at.path,
                confine: &session::Confine {
                    confinement: id.clone(),
                    project: self.at.project.clone(),
                    plan: plan.clone(),
                    // It asks the link it is named to; one named to none refuses instead.
                    asks: !named_to.is_empty(),
                },
                planned: Planned::Composed,
                alone: &alone,
            },
            tell,
            askable,
        )?;
        let confining: Arc<dyn Confining> = Arc::from(confining);
        let mut confined = confining.confined();
        confined.confinement = id.clone();
        {
            let mut state = self.hosting.state();
            // One started meanwhile under the same id keeps it, and this one ends.
            if state.confinements.contains_key(&id) {
                drop(state);
                end_off_the_runtime(vec![confining]);
                return Err(already_confined(&id).why);
            }
            // A chat is its window's on each of that window's links (#2511).
            named_to.extend(links_of_window(&state, window.as_deref()));
            state.confinements.insert(
                id,
                Confinement {
                    project: self.at.project.clone(),
                    path: self.at.path.clone(),
                    owner: Owner::Program(self.program.clone()),
                    named_to,
                    confining,
                    plan: plan.clone(),
                    window,
                    unheld: Vec::new(),
                },
            );
        }
        Ok(confined)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.hosting.record().record(entry)
    }
}

/// A chat's confinement whose ports of the machine nobody is left to withdraw (#2407): those
/// it was given, to withdraw now.
struct Unheld {
    id: String,
    confining: Arc<dyn Confining>,
    ports: Vec<u16>,
}

/// **Takes the name of `peer`'s link off each confinement `whose` picks** (#2444), once that
/// link has ended. A chat's confinement left named to no link, by a link that cannot take its
/// chat back, is to carry none of the machine's ports it was given from now on: nobody is left
/// to withdraw one (#2407). Those are answered, and kept as withdrawn in its plan until its
/// window takes it back, which gives them back (#2625).
fn unname(state: &mut State, peer: &Peer, whose: impl Fn(&Owner) -> bool) -> Vec<Unheld> {
    let link = link_of(peer);
    let comes_back = peer.scope().may(Power::Reattach);
    let mut unheld = Vec::new();
    for (id, held) in &mut state.confinements {
        if !whose(&held.owner) || !held.named_to.remove(&link) {
            continue;
        }
        if comes_back || !held.named_to.is_empty() || !matches!(held.owner, Owner::Program(_)) {
            continue;
        }
        let ports: Vec<u16> = held
            .plan
            .given
            .iter()
            .copied()
            .filter(|port| !held.plan.withdrawn.contains(port))
            .collect();
        if !ports.is_empty() {
            held.plan.withdrawn.extend(&ports);
            held.unheld.extend(&ports);
            unheld.push(Unheld {
                id: id.clone(),
                confining: Arc::clone(&held.confining),
                ports,
            });
        }
    }
    unheld
}

/// Withdraws each of `unheld`'s ports from its confinement, as a window's withdrawal would
/// (#2407). One whose port cannot be withdrawn is ended: it carries no port nobody is left to
/// withdraw. Blocking: a proxy's end waits on its listeners.
fn withdraw(hosting: &Weak<Inner>, unheld: Vec<Unheld>) {
    let Some(inner) = hosting.upgrade() else {
        return;
    };
    let _porting = lock_porting(&inner);
    for Unheld {
        id,
        confining,
        ports,
    } in unheld
    {
        // A port given back meanwhile, its window having taken the chat back, stays carried.
        let still = hosting_of(&inner)
            .state()
            .confinements
            .get(&id)
            .map(|held| held.unheld.clone())
            .unwrap_or_default();
        let ports: Vec<u16> = ports
            .into_iter()
            .filter(|port| still.contains(port))
            .collect();
        let withdrawn = ports.into_iter().all(|port| {
            confining
                .change(&session::ReachChange::WithdrawPort { port })
                .is_ok()
        });
        if !withdrawn {
            let ending = hosting.upgrade().and_then(|inner| {
                Hosting { inner }
                    .state()
                    .confinements
                    .remove(&id)
                    .map(|held| held.confining)
            });
            drop((confining, ending));
        }
    }
}

fn hosting_of(inner: &Arc<Inner>) -> Hosting {
    Hosting {
        inner: Arc::clone(inner),
    }
}

fn lock_porting(inner: &Inner) -> std::sync::MutexGuard<'_, ()> {
    inner
        .porting
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **Gives back each port the host withdrew from confinement `id` because no link was left**
/// (#2625), now its window has taken the chat back: as the plan gave it. Only a port still
/// [`Confinement::unheld`], so not one the window withdrew since. Each port is checked, given
/// back and marked so under the host's state lock, which a window's withdrawal marks its port
/// under before its proxy hears it: whichever comes first, a port the window withdrew ends
/// withdrawn. A port the proxy cannot give back stays withdrawn. Blocking, on the porting lane.
///
/// **Each port is first checked against this machine's policy as it is now** (#2738), not as
/// it was when the plan was made: one the policy no longer gives stays withdrawn for want of a
/// window, so a later take-back, once the policy gives it again, gives it back.
fn give_back(hosting: &Weak<Inner>, id: &str) {
    let Some(inner) = hosting.upgrade() else {
        return;
    };
    let _porting = lock_porting(&inner);
    let hosting = hosting_of(&inner);
    let Some((ports, path)) = hosting
        .state()
        .confinements
        .get(id)
        .map(|held| (held.unheld.clone(), held.path.clone()))
    else {
        return;
    };
    for port in ports {
        // The policy is read off the state lock.
        if let Err(why) = inner.confine.gives_now(&path, port) {
            crate::say(&format!(
                "port {port} stays withdrawn from confinement {id}: {why}"
            ));
            continue;
        }
        let mut state = hosting.state();
        let Some(held) = state.confinements.get_mut(id) else {
            return;
        };
        if !held.unheld.contains(&port) {
            continue;
        }
        // A set removal in the proxy: cheap enough to make under the lock.
        if let Err(why) = held.confining.give_back(port) {
            drop(state);
            crate::say(&format!(
                "port {port} stays withdrawn from confinement {id}, which could not carry it \
                 again: {why}"
            ));
            continue;
        }
        held.unheld.retain(|unheld| *unheld != port);
        held.plan.withdrawn.retain(|withdrawn| *withdrawn != port);
    }
}

/// Runs `blocking` off the runtime, where there is one: a proxy's end waits on its listeners.
fn off_the_runtime(blocking: impl FnOnce() + Send + 'static) {
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
            runtime.spawn_blocking(blocking);
        }
        Err(_) => blocking(),
    }
}

/// Ends `confinements` off the runtime: a proxy's end waits on its listeners.
fn end_off_the_runtime(confinements: Vec<Arc<dyn Confining>>) {
    if !confinements.is_empty() {
        off_the_runtime(move || drop(confinements));
    }
}

impl Hosting {
    /// Save `project`, at the place `peer`'s link holds it, unless it is saved there already:
    /// one saver per place (#2178).
    async fn start_saving(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let at = held_at(&self.state(), project, link_of(peer))?;
        // A saver's start reads the project and starts threads: off the runtime.
        let hosting = self.clone();
        tokio::task::spawn_blocking(move || hosting.saver_for(at))
            .await
            .map_err(|why| not_started(why.to_string()))?
    }

    fn saver_for(&self, at: HeldAt) -> Result<Answer, Refusal> {
        let _one_at_a_time = self
            .inner
            .starting_a_saver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.state().saving.contains_key(&at) {
            return Ok(Answer::Done);
        }
        let Some(save) = self.inner.save.get() else {
            return Err(not_started(
                "this session host was given no way to save a project".into(),
            ));
        };
        // Each weakly: the saver is the host's, and must not keep it. What each look did is
        // told to the links that hold the project at this place, as its hook lines are: another
        // clone's window is never told it (#2178).
        let told = {
            let host = Arc::downgrade(&self.inner);
            let at = at.clone();
            Box::new(move |looked: Looked| {
                if let Some(inner) = host.upgrade() {
                    let pushed = Pushed::Saving {
                        project: at.project.clone(),
                        saved: looked.saved,
                        moved_tree: looked.moved_tree,
                    };
                    Hosting::tell_holders(&mut Hosting { inner }.state(), &at, &pushed, true);
                }
            })
        };
        let working = {
            let host = Arc::downgrade(&self.inner);
            let at = at.clone();
            Box::new(move || {
                host.upgrade()
                    .map(|inner| Hosting { inner }.working_in(&at))
                    .unwrap_or_default()
            })
        };
        let ask_the_window = {
            let host = Arc::downgrade(&self.inner);
            let at = at.clone();
            Box::new(move |workspace: &str| {
                host.upgrade()
                    .and_then(|inner| Hosting { inner }.ask_the_window(&at, workspace, ASK_WITHIN))
            })
        };
        let saving = save
            .save(ToSave {
                project: at.project.clone(),
                project_path: at.path.clone(),
                told,
                working,
                ask_the_window,
            })
            .map_err(not_started)?;
        self.state().saving.insert(at, saving);
        Ok(Answer::Done)
    }

    /// The programs running in the project held `at` now, by number.
    fn working_in(&self, at: &HeldAt) -> Vec<Working> {
        let mut working: Vec<Working> = self
            .state()
            .programs
            .values()
            .filter(|hosted| {
                hosted.at == *at && !hosted.ended && hosted.kind != ProgramKind::Browser
            })
            .map(|hosted| Working {
                number: hosted.number,
                cwd: hosted.cwd.clone(),
            })
            .collect();
        working.sort_by_key(|one| one.number);
        working
    }

    /// Stop saving `project`, answered once its saver has been let go of. Nothing saved is done.
    async fn stop_saving(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let saving = {
            let mut state = self.state();
            saved_at(&state, project, link_of(peer))?.and_then(|at| state.saving.remove(&at))
        };
        if let Some(saving) = saving {
            let _ = tokio::task::spawn_blocking(move || drop(saving)).await;
        }
        Ok(Answer::Done)
    }

    fn fetch(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let state = self.state();
        let saving = saved_at(&state, project, link_of(peer))?
            .and_then(|at| state.saving.get(&at))
            .ok_or_else(|| Refusal {
                code: session::NO_SUCH_PROJECT.into(),
                why: format!("this session host does not save project {project}"),
            })?;
        saving.poke(Poke::Fetch);
        Ok(Answer::Done)
    }
}

impl session::Host for Hosting {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        if let Some(program) = browser_acted_on(&command) {
            self.opened_for(program, peer)?;
        }
        match command {
            Command::List => Ok(Answer::Chats(self.list(peer.version()))),
            Command::HoldProject {
                project,
                project_path,
            } => self.hold(project, project_path, peer),
            Command::LetGoOfProject { project } => self.let_go(&project, peer),
            Command::TakeCode {
                project,
                repo,
                transfer,
                refs,
                checkout,
            } => {
                let way = crate::code::Way::In { refs, checkout };
                self.move_code(&project, repo, transfer, way, peer).await
            }
            Command::GiveCode {
                project,
                repo,
                transfer,
                have,
            } => {
                self.move_code(
                    &project,
                    repo,
                    transfer,
                    crate::code::Way::Out { have },
                    peer,
                )
                .await
            }
            Command::StartProgram(start) => match start.kind {
                ProgramKind::Harness | ProgramKind::Shell => self.start(*start, peer).await,
                ProgramKind::Browser => self.start_browser(*start, peer).await,
            },
            Command::AttachView {
                program,
                kind,
                role,
                rate,
            } => self.attach(&program, kind, role, rate, peer).await,
            Command::DetachView { program, view } => {
                self.running(&program)?;
                peer.views().close(ViewId(view));
                Ok(Answer::Done)
            }
            Command::Write { chat, bytes } => {
                self.running(&chat)?.write(&bytes).map_err(not_started)?;
                Ok(Answer::Done)
            }
            Command::Resize { chat, cols, rows } => self.resize(
                &chat,
                Size {
                    columns: cols,
                    rows,
                },
            ),
            Command::Readiness { program } => {
                Ok(Answer::Readiness(self.running(&program)?.readiness()))
            }
            Command::Stop { chat } => self.stop(&chat).await,
            Command::Close { program } => self.close(&program),
            Command::Deal { project } => self.deal(&project, peer),
            Command::DealtAlready { project, dealt } => self.dealt_already(&project, dealt, peer),
            Command::ReplyHeard {
                project,
                heard,
                reply,
            } => self.reply_heard(&project, heard, reply, peer),
            Command::StartSaving { project } => self.start_saving(&project, peer).await,
            Command::StopSaving { project } => self.stop_saving(&project, peer).await,
            Command::FetchProject { project } => self.fetch(&project, peer),
            Command::MidTurn { question, chats } => {
                // An answer nobody waits for any more (it came late) is done all the same.
                if let Some(waiting) = self.state().questions.remove(&question) {
                    let _ = waiting.send(chats);
                }
                Ok(Answer::Done)
            }
            Command::BoardMoved {
                project,
                chat,
                moved,
            } => {
                if let Some(channel) = self.held_channel(&project, peer)? {
                    channel.moved(chat, moved);
                }
                Ok(Answer::Done)
            }
            Command::Board { project } => match self.held_channel(&project, peer)? {
                Some(channel) => channel.board().map(Answer::Board).map_err(|why| Refusal {
                    code: session::TOO_LARGE.into(),
                    why,
                }),
                None => Ok(Answer::Board(serde_json::Value::Null)),
            },
            Command::Confine(confine) => self.confine(*confine, peer).await,
            Command::LetGoOfConfinement { confinement } => {
                self.let_go_of_confinement(&confinement, peer).await
            }
            Command::ChangeReach {
                confinement,
                change,
            } => {
                let confining = self.confinement(&confinement, peer)?;
                // A port the window withdraws stays withdrawn: never one to give back (#2625).
                // Marked so before the proxy hears it, under the lock a give-back checks each
                // port under, so a give-back either ran before it or never gives this port back.
                let mut carried = None;
                if let session::ReachChange::WithdrawPort { port } = change
                    && let Some(held) = self.state().confinements.get_mut(&confinement)
                {
                    let was_unheld = held.unheld.contains(&port);
                    held.unheld.retain(|unheld| *unheld != port);
                    if !held.plan.withdrawn.contains(&port) {
                        held.plan.withdrawn.push(port);
                        // Its proxy carries it now, unless the host withdrew it for want of a
                        // window, which it stays.
                        carried = (!was_unheld).then_some(port);
                    }
                }
                let retry = confining.change(&change).map_err(|why| {
                    // A withdrawal the proxy refused leaves the port as it carries it (#2744):
                    // given in the plan, so it is withdrawn as the last link ends, as every
                    // given port is, and never left carried with nobody to withdraw it.
                    if let Some(port) = carried
                        && let Some(held) = self.state().confinements.get_mut(&confinement)
                    {
                        held.plan.withdrawn.retain(|withdrawn| *withdrawn != port);
                    }
                    not_started(why)
                })?;
                Ok(Answer::ReachChanged { retry })
            }
            Command::ReachOf { confinement } => {
                Ok(Answer::Reach(self.confinement(&confinement, peer)?.now()))
            }
            Command::StopAll => Ok(self.stop_all().await),
            // Only the window's link reaches this: the grants refuse every other.
            Command::Rearm => self
                .inner
                .switch
                .rearm()
                .map(Answer::KillSwitch)
                .map_err(|why| Refusal {
                    code: session::STOPPED.into(),
                    why,
                }),
            Command::KillSwitch => Ok(Answer::KillSwitch(self.inner.switch.state())),
            // Only the window's link reaches this: the grants refuse every other. A write is
            // made durable, so off the runtime.
            Command::Record { entry } => {
                let hosting = self.clone();
                tokio::task::spawn_blocking(move || hosting.record().record(entry))
                    .await
                    .map_err(|why| not_recorded(why.to_string()))?
                    .map(|()| Answer::Done)
                    .map_err(not_recorded)
            }
            Command::Browse {
                program,
                call_id,
                act,
                reach,
            } => self.browse(&program, call_id, act, reach).await,
            Command::LogIn { program, login } => self.log_in(&program, login).await,
            Command::ProgramsOf { project } => self.programs_of(&project, peer),
            // Only the window's link reaches this: the grants refuse every other.
            Command::NoteProgram { program, note } => self.note_program(&program, note, peer),
            // Only the window's link reaches this: the grants refuse every other.
            Command::Quit => Ok(self.quit().await),
            // The person's own (#2014): each may wait on the engine (a take-over on an act's
            // last input, a key on a read of the page), so off the runtime.
            Command::TakeOver { program } => {
                self.driving(&program, |running| running.take_over().map(driving))
                    .await
            }
            Command::CancelEngineDownload { program } => {
                if let Some(download) = self.state().downloads.get(&program) {
                    download.cancel();
                }
                Ok(Answer::Done)
            }
            Command::HandBack { program } => {
                self.driving(&program, |running| running.hand_back().map(driving))
                    .await
            }
            Command::BrowserInput { program, input } => {
                self.driving(&program, move |running| {
                    running.input(&input).map(|()| Answer::Done)
                })
                .await
            }
            Command::SizeBrowser {
                program,
                width,
                height,
            } => {
                self.driving(&program, move |running| {
                    running.size_browser(width, height).map(|()| Answer::Done)
                })
                .await
            }
            Command::TakeBack { window } => self.take_back(window, peer),
            // The window takes its own chats back with `programs_of`: a host that relays
            // nothing has nothing to do with its name (#2623).
            Command::NameWindow { .. } => Ok(Answer::Done),
            Command::Answer { chat, .. } => Err(Refusal {
                code: session::NO_SUCH_CHAT.into(),
                why: format!("program {chat} has no ask this host holds"),
            }),
            // `serve` keeps these: 1.0's words come as the hosted-program ones, and the
            // subscription is the link's.
            // A runner's host carries it out on its link from the desktop it is paired with
            // (`serving`), before this host hears of it (1.25, #2525).
            Command::Unpair => Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "this host is paired with no desktop over this link, so there is nothing to \
                      unpair"
                    .into(),
            }),
            // A runner's host keeps its desktop's profile on its link from that desktop
            // (`serving`) before this host hears of it (1.43, #2049).
            Command::KeepProfile(_) => Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "this host is paired with no desktop over this link, so it keeps no \
                      desktop's profile"
                    .into(),
            }),
            // The desktop's host relays a window's link to a runner before this host hears of
            // the link (`serving`, 1.33, #2679): a host that hears it relays to no runner.
            Command::RelayTo { .. } => Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: "this host relays to no runner, so the link was not relayed".into(),
            }),
            Command::Start(_)
            | Command::Attach { .. }
            | Command::Detach { .. }
            | Command::Subscribe { .. }
            | Command::Unsubscribe => Err(Refusal {
                code: session::MALFORMED.into(),
                why: "a command the link carries out itself".into(),
            }),
        }
    }

    async fn link_ended(&self, peer: &Peer) {
        self.let_go_of_the_links(peer);
        {
            let mut state = self.state();
            if state.windows.remove(&link_of(peer)) && state.windows.is_empty() {
                state.windowless_since = Some(Instant::now());
            }
        }
        self.inner.changed.notify_one();
        // What the link confined that nothing runs in yet goes with it; what a program runs in
        // stays with the program (#1999).
        let link = link_of(peer);
        let (ending, unheld) = {
            let mut state = self.state();
            let unheld = unname(&mut state, peer, |_| true);
            // What it made for a relayed window passes to that window's other link (#2624).
            hand_on(&mut state, link);
            let ending = take_confinements(&mut state, |owner| *owner == Owner::Link(link));
            state.relayed_for.remove(&link);
            (ending, unheld)
        };
        // Its chats' ports are withdrawn before what it confined ends, on one blocking thread.
        let weak = Arc::downgrade(&self.inner);
        off_the_runtime(move || {
            withdraw(&weak, unheld);
            drop(ending);
        });
    }

    async fn subscribe(
        &self,
        _since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let (told, notices) = mpsc::channel(MOST_NOTICES_QUEUED);
        {
            let mut state = self.state();
            state.subscribers.push(Subscriber {
                link: link_of(peer),
                told,
                confines: peer.scope().may(Power::Confine),
                saves: peer.scope().may(Power::Save),
                downloads: peer.version() >= session::KNOWS_ENGINE_DOWNLOADS
                    && peer.scope().may(Power::Browser),
            });
            if peer.scope() == Scope::LocalUi {
                note_a_window(&mut state, link_of(peer));
            }
        }
        self.inner.changed.notify_one();
        Ok(notices)
    }
}

/// Who the admission check takes to be inside a chat: everything below the window that started
/// this host, or below the host itself, and every hosted program by its pid.
impl Chats for Hosting {
    fn programs(&self) -> Vec<u32> {
        let state = self.state();
        state
            .programs
            .values()
            .filter_map(|hosted| hosted.process)
            .chain(state.closed.iter().copied())
            .collect()
    }

    fn holder(&self) -> Option<u32> {
        Some(match self.state().window {
            Some(Window(window)) => window,
            None => std::process::id(),
        })
    }

    /// The window that started this host, and each window that came back to it since (#2006):
    /// a peer below any of them is inside a chat. Only ever more of them: a doubt refuses.
    fn holders(&self) -> Vec<u32> {
        let state = self.state();
        match state.window {
            Some(Window(window)) => std::iter::once(window)
                .chain(state.came_back.iter().copied())
                .collect(),
            None => vec![std::process::id()],
        }
    }
}

/// Whether `id` may name a folder: letters, digits, `-` and `_`, as a ULID is.
fn a_plain_name(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// **A note the host keeps for the window, where one is, fits its bound** (1.39, #2025): one past
/// [`session::ProgramNote::MOST`] bytes is refused [`session::TOO_LARGE`].
fn a_note_that_fits(note: Option<&session::ProgramNote>) -> Result<(), Refusal> {
    match note {
        Some(note) if !note.fits() => Err(Refusal {
            code: session::TOO_LARGE.into(),
            why: format!(
                "a program's note is at most {} bytes, and this one is {}",
                session::ProgramNote::MOST,
                note.bytes()
            ),
        }),
        _ => Ok(()),
    }
}

fn no_such_program(program: &str) -> Refusal {
    Refusal {
        code: session::NO_SUCH_CHAT.into(),
        why: format!("this session host holds no program {program}"),
    }
}

/// Whether `id` is a ULID as purlis writes one: twenty-six of Crockford's base32 digits, upper
/// case, the first no more than `7`. A project's id names its folder, so nothing else does.
fn a_ulid(id: &str) -> bool {
    id.len() == 26
        && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z'))
        && id.as_bytes()[0] <= b'7'
}

fn no_such_project(project: &str) -> Refusal {
    Refusal {
        code: session::NO_SUCH_PROJECT.into(),
        why: format!("this session host does not hold project {project}"),
    }
}

/// A project whose numbers have all been dealt: none is dealt again, so none wraps round to
/// a chat's that is running.
fn no_number_left(project: &str) -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why: format!("project {project} has no chat number left to deal"),
    }
}

fn no_such_confinement(id: &str) -> Refusal {
    Refusal {
        code: session::NO_SUCH_CHAT.into(),
        why: format!("this session host holds no confinement {id}"),
    }
}

/// A plan made on another machine that does not fit this one (#2445): what it asks is not
/// this machine's to give.
fn not_fitting(why: String) -> Refusal {
    Refusal {
        code: session::NOT_ALLOWED.into(),
        why,
    }
}

fn already_confined(id: &str) -> Refusal {
    Refusal {
        code: session::MALFORMED.into(),
        why: format!("this session host already holds a confinement {id}"),
    }
}

fn not_recorded(why: String) -> Refusal {
    Refusal {
        code: session::NOT_RECORDED.into(),
        why,
    }
}

/// **A browser's place in the device's budget while it starts** (#2611): taken under the host's
/// state lock with the count, and given back when it has started, in the same step that lists
/// it, or when it does not start.
struct PlaceInTheBudget {
    hosting: Option<Hosting>,
}

impl PlaceInTheBudget {
    /// A place for one more browser, or `device_full` where the browsers running and starting
    /// fill `budget` already. With no budget, every start has a place.
    fn taken(hosting: &Hosting, state: &mut State, budget: Option<usize>) -> Result<Self, Refusal> {
        if let Some(most) = budget {
            let running = state
                .programs
                .values()
                .filter(|hosted| hosted.kind == ProgramKind::Browser && !hosted.ended)
                .count();
            if running + state.browsers_starting >= most {
                return Err(Refusal {
                    code: session::DEVICE_FULL.into(),
                    // Whole on its own: a window before 1.32 shows it as it is.
                    why: format!(
                        "This device runs at most {most} {} at once, and that many run now: \
                         close one first, or raise the budget in Settings › This machine › \
                         Browsers",
                        if most == 1 { "browser" } else { "browsers" }
                    ),
                });
            }
        }
        state.browsers_starting += 1;
        Ok(PlaceInTheBudget {
            hosting: Some(hosting.clone()),
        })
    }

    /// The browser started, and is listed while `state` is held: it counts from now on.
    fn started(mut self, state: &mut State) {
        if self.hosting.take().is_some() {
            state.browsers_starting -= 1;
        }
    }
}

impl Drop for PlaceInTheBudget {
    /// It did not start: its place is free.
    fn drop(&mut self) {
        if let Some(hosting) = self.hosting.take() {
            hosting.state().browsers_starting -= 1;
        }
    }
}

/// The refusal of a browser that did not start (#2899): `no_engine_sandbox` where its engine
/// cannot make its sandbox here, `not_started` for anything else; in the binary's words.
fn browser_not_started(why: crate::browsers::NotStarted) -> Refusal {
    match why {
        crate::browsers::NotStarted::NoEngineSandbox(why) => Refusal {
            code: session::NO_ENGINE_SANDBOX.into(),
            why,
        },
        crate::browsers::NotStarted::Failed(why) => not_started(why),
    }
}

fn not_started(why: String) -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why,
    }
}

/// **What the host's life with a window that may come back reads and does** (#2006,
/// `crate::lifetime`).
impl Hosting {
    /// The window that started this host has gone: from now on a host with no window linked
    /// waits only as long as the grace period for one to come back.
    pub(crate) fn its_starter_went(&self) {
        let mut state = self.state();
        state.had_a_window = true;
        if state.windows.is_empty() && state.windowless_since.is_none() {
            state.windowless_since = Some(Instant::now());
        }
    }

    /// Whether this host has nothing more to wait for at `now`: no window is linked since one
    /// was, and either nothing is left a window could come back for, or the grace period has
    /// passed since the last one went.
    pub(crate) fn nobody_to_come_back_for(&self, grace: Duration, now: Instant) -> bool {
        let state = self.state();
        let nothing_kept =
            state.projects.is_empty() && state.saving.is_empty() && state.programs.is_empty();
        let waited_out = state
            .windowless_since
            .is_some_and(|since| now.saturating_duration_since(since) >= grace);
        state.windows.is_empty() && state.had_a_window && (nothing_kept || waited_out)
    }

    /// When the next grace period passes, if one is running: a project's no link holds, or the
    /// host's own with no window.
    pub(crate) fn next_grace_ends(&self, grace: Duration) -> Option<Instant> {
        let state = self.state();
        state
            .unheld_since
            .values()
            .copied()
            .chain(state.windowless_since)
            .min()
            .map(|since| since + grace)
    }

    /// Lets go of every project no link has held for `grace` by `now`, as a quit would (#2006,
    /// #2262): its programs end all at once, running or ended, each kept as ended by a quit,
    /// then its confinements, its saving, its hook channel and its board go. Waits for the
    /// programs as [`Self::end_all`] does: off the runtime.
    pub(crate) fn let_go_of_what_nobody_came_back_for(&self, grace: Duration, now: Instant) {
        let (hosted, confinements, held, saving) = {
            let mut state = self.state();
            let due: Vec<HeldAt> = state
                .unheld_since
                .iter()
                .filter(|(at, since)| {
                    now.saturating_duration_since(**since) >= grace
                        && state
                            .projects
                            .get(*at)
                            .is_none_or(|held| held.links.is_empty())
                })
                .map(|(at, _)| at.clone())
                .collect();
            if due.is_empty() {
                return;
            }
            for at in &due {
                state.unheld_since.remove(at);
            }
            let ids: Vec<String> = state
                .programs
                .iter()
                .filter(|(_, hosted)| due.contains(&hosted.at))
                .map(|(id, _)| id.clone())
                .collect();
            let hosted: Vec<Hosted> = ids
                .iter()
                .filter_map(|id| state.programs.remove(id))
                .inspect(|hosted| stopping_by(&hosted.stopping, Stopped::Quit))
                .collect();
            let processes: Vec<u32> = hosted.iter().filter_map(|hosted| hosted.process).collect();
            state.closed.extend(processes);
            let confinements = take_confinements(
                &mut state,
                |owner| matches!(owner, Owner::Program(program) if ids.contains(program)),
            );
            let held: Vec<Held> = due
                .iter()
                .filter_map(|at| state.projects.remove(at))
                .collect();
            let saving: Vec<Box<dyn Saving>> = due
                .iter()
                .filter_map(|at| state.saving.remove(at))
                .collect();
            (hosted, confinements, held, saving)
        };
        // Saving first, as at a quit: nothing starts a save of a project whose chats are ending.
        drop(saving);
        stop_together(
            hosted
                .iter()
                .map(|hosted| Arc::clone(&hosted.running))
                .collect(),
        );
        drop(confinements);
        drop(held);
        drop(hosted);
    }

    /// Resolves once something a window's coming back depends on has changed.
    pub(crate) async fn changed(&self) {
        self.inner.changed.notified().await;
    }
}

/// How long [`Hosting::end_all`] gives every program, together, to end.
pub const ENDS_WITHIN: Duration = Duration::from_secs(5);

impl Hosting {
    /// Ends every program, all at once, and returns once they are gone or [`ENDS_WITHIN`] has
    /// passed: what the host does as it stops, so a quit still ends every chat.
    pub fn end_all(&self) {
        // Saving first: nothing starts a save of a project whose chats are being ended.
        let saving: Vec<Box<dyn Saving>> = self
            .state()
            .saving
            .drain()
            .map(|(_, saving)| saving)
            .collect();
        drop(saving);
        let (hosted, confinements) = {
            let mut state = self.state();
            let hosted: Vec<Hosted> = state
                .programs
                .drain()
                .map(|(_, hosted)| {
                    stopping_by(&hosted.stopping, Stopped::Quit);
                    hosted
                })
                .collect();
            let processes: Vec<u32> = hosted.iter().filter_map(|hosted| hosted.process).collect();
            state.closed.extend(processes);
            let confinements: Vec<Arc<dyn Confining>> = state
                .confinements
                .drain()
                .map(|(_, held)| held.confining)
                .collect();
            (hosted, confinements)
        };
        stop_together(hosted.into_iter().map(|hosted| hosted.running).collect());
        // Each chat's proxy ends with the programs: no chat reaches anything once the host has.
        drop(confinements);
        // Every program was ended as the host stopped, whether or not its end was told in
        // time: there is nothing for the next host to settle (#2007).
        if let Some(host_record) = &self.inner.host_record {
            host_record.empty();
        }
    }

    /// **Settles what the last host left running** (#2007, ADR 0068 §3), and answers how many
    /// programs that was. The host record is read and forgotten; each program it says that host
    /// started and never ended is ended by what runs programs where its process is still the
    /// one started ([`Run::end_what_was_left`]), and told to the binary, which keeps its end as
    /// a crash's ([`Record::left_by_its_last_host`]). Called once, as the host starts serving,
    /// under its lock and before anyone is served. Nothing for a host kept in memory only.
    pub fn take_over_from_the_last_host(&self) -> usize {
        let Some(host_record) = &self.inner.host_record else {
            return 0;
        };
        let left = host_record.take_over();
        if !left.is_empty() {
            crate::say(&format!(
                "the last session host ended without ending {} of its programs; each is \
                 settled as ended by its crash",
                left.len()
            ));
        }
        for program in &left {
            self.inner.run.end_what_was_left(program);
            self.record().left_by_its_last_host(program);
        }
        left.len()
    }
}

/// Stops every one of `programs` at once, and returns once they are gone or [`ENDS_WITHIN`]
/// has passed.
fn stop_together(programs: Vec<Arc<dyn Running>>) {
    let ending: Vec<_> = programs
        .into_iter()
        .filter_map(|running| {
            std::thread::Builder::new()
                .name("purlisd-ending".into())
                .spawn(move || running.stop())
                .ok()
        })
        .collect();
    let deadline = std::time::Instant::now() + ENDS_WITHIN;
    for thread in ending {
        while !thread.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
