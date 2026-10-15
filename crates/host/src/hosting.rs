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
//! **Every hosted program is inside a chat to the admission check.** The holder the check is
//! given is the window, when the window started this host, or the host itself: a hosted
//! program, and everything it starts, is below both from its first instruction, so a peer among
//! them is refused every person's scope (ADR 0068 §5, FD-27's item 2). Each program's pid is
//! listed too, so one that left the host's tree and kept its session is refused as well.
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
//! is composed, and tells every subscriber what each look did. One saver per project, however
//! often it is asked. A link that ends without a word leaves it saving, so the project is saved
//! while the window is gone for a moment; the window's own stop, or every window letting go of
//! the project, ends it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::grants::Power;
use purlis_session_protocol::local::Chats;
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Exit, Launch, Peer, ProgramKind, Pushed, Readiness,
    Refusal, SandboxTold, Since, StartProgram, ViewKind, ViewRole,
};
use purlis_session_protocol::version::Version;
use purlis_session_protocol::view::{Attachment, Source, ViewId};
use purlis_terminal::engine::{AlacrittyEngine, SCROLLBACK, Size};
use purlis_terminal::session::{Session, Spec};
use tokio::sync::mpsc;

use crate::browsers::{Browser, Browsers, NoBrowsers};
use crate::confining::{Confine, ConfinesNothing, Confining, ToConfine};
use crate::hearing::{Channel, HandOn, Hear, HearsNothing};
use crate::killswitch::Switch;
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
}

/// Turns a start into the program to run and the environment it runs with: the binary's, built
/// on the core, so the host links none of it (#1998).
pub trait Compose: Send + Sync + 'static {
    /// The program to run for `starting`, or why it cannot be started, in a sentence.
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String>;
}

/// Composes a start from the window's own [`Launch`], which `launched` turns into the program,
/// numbered and sized, as the window would have started it (#1998). A start with no launch, by
/// persona, profile and harness alone, is refused in a sentence until what composes one from
/// those lives in the host.
pub struct FromTheWindow<F>(pub F);

impl<F> Compose for FromTheWindow<F>
where
    F: Fn(&Launch, u32, Size) -> Result<Spec, String> + Send + Sync + 'static,
{
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        match &starting.start.launch {
            Some(launch) => (self.0)(launch, starting.number, starting.size),
            None => Err(
                "this session host starts a chat the window composed; a start by persona, \
                 profile and harness alone is not composed here yet"
                    .into(),
            ),
        }
    }
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
    /// says; `None` when it has no view of that kind. A terminal's is its terminal view.
    fn view(
        &self,
        kind: ViewKind,
        role: Option<ViewRole>,
        rate: Option<u32>,
    ) -> Option<Arc<dyn Source>> {
        let _ = (role, rate);
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
    /// Whether it is ready to be typed at.
    fn readiness(&self) -> Readiness;
    /// Calls `tell` once, as it ends.
    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>);
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
    state: Mutex<State>,
    /// Held while a saver is started, so two windows asking at once start one.
    starting_a_saver: Mutex<()>,
    switch: Switch,
}

#[derive(Default)]
struct State {
    projects: HashMap<String, Held>,
    programs: HashMap<String, Hosted>,
    /// The processes of programs closed or ended by the host, kept for the admission check for
    /// as long as the host runs: a process a program started may outlive it in its session, in
    /// a group of its own the close did not reach, and is still inside that chat. Keeping a
    /// number past its session errs towards refusing a peer, never towards admitting one.
    closed: Vec<u32>,
    /// Each project's highest number dealt: it only rises, so a number names one chat.
    dealt: HashMap<String, u32>,
    /// Who is told what the host pushes ([`Subscriber`]).
    subscribers: Vec<Subscriber>,
    /// The chats' confinements (#1999), by the id the window gave each.
    confinements: HashMap<String, Confinement>,
    /// Each question to the window waiting for its answer, by its number.
    questions: HashMap<u64, std::sync::mpsc::Sender<Option<Vec<String>>>>,
    /// The last question's number.
    asked: u64,
    /// The window that started this host, if one did.
    window: Option<Window>,
    /// Each project being saved, by its id: one saver each (#2002).
    saving: HashMap<String, Box<dyn Saving>>,
}

/// One subscriber, and whether its link may hear what a chat's sandbox does
/// ([`Power::Confine`]).
struct Subscriber {
    /// The link it came on ([`link_of`]).
    link: u64,
    told: mpsc::Sender<Pushed>,
    confines: bool,
}

/// A chat's confinement the host holds (#1999), and whose it is.
struct Confinement {
    project: String,
    owner: Owner,
    confining: Arc<dyn Confining>,
}

/// Whose a confinement is: the link that made it, until a start names it, then that program's.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Owner {
    Link(u64),
    Program(String),
}

struct Held {
    path: PathBuf,
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
    project: String,
    number: u32,
    run: Option<String>,
    running: Arc<dyn Running>,
    size: Size,
    /// Its process as it started, kept for the admission check after it ends.
    process: Option<u32>,
    /// Where it started, where its start said: what its project's saver asks.
    cwd: Option<PathBuf>,
    ended: bool,
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
        Hosting::built(compose, run, hear, confine, Switch::in_memory())
    }

    /// A host whose starts `compose` composes, each run in a terminal of its own, which hears
    /// no hooks and starts no confinement, with its kill switch kept in `folder`, its own, and
    /// read from there now: a host started again while stopped starts nothing until the window
    /// re-arms (ADR 0071). What `purlis serve` runs.
    pub fn keeping_its_switch_in(folder: &Path, compose: impl Compose) -> Hosting {
        Hosting::kept(folder, compose, Terminals, HearsNothing, ConfinesNothing)
    }

    /// [`Hosting::keeping_its_switch_in`], each start run by `run`.
    pub fn running_with_its_switch_in(
        folder: &Path,
        compose: impl Compose,
        run: impl Run,
    ) -> Hosting {
        Hosting::kept(folder, compose, run, HearsNothing, ConfinesNothing)
    }

    /// [`Hosting::with`], with its kill switch kept in `folder`, its own, and read from there
    /// now (ADR 0071): what the window's host runs.
    pub fn kept(
        folder: &Path,
        compose: impl Compose,
        run: impl Run,
        hear: impl Hear,
        confine: impl Confine,
    ) -> Hosting {
        Hosting::built(compose, run, hear, confine, Switch::kept_in(folder))
    }

    fn built(
        compose: impl Compose,
        run: impl Run,
        hear: impl Hear,
        confine: impl Confine,
        switch: Switch,
    ) -> Hosting {
        Hosting {
            inner: Arc::new(Inner {
                compose: Box::new(compose),
                confine: Box::new(confine),
                run: Box::new(run),
                save: std::sync::OnceLock::new(),
                hear: Box::new(hear),
                browsers: Box::new(NoBrowsers),
                state: Mutex::default(),
                starting_a_saver: Mutex::new(()),
                switch,
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

    /// The window that started this host and is the first thing its programs run below.
    pub fn started_by(self, window: Option<Window>) -> Hosting {
        self.state().window = window;
        self
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Tell every subscriber, letting go of one that is gone or has stopped reading. What a
    /// chat's sandbox did is told only to a link that may confine.
    fn tell(&self, pushed: &Pushed) {
        let sandbox = matches!(pushed, Pushed::Sandbox { .. });
        self.state().subscribers.retain(|subscriber| {
            if sandbox && !subscriber.confines {
                return !subscriber.told.is_closed();
            }
            subscriber.told.try_send(pushed.clone()).is_ok()
        });
    }

    /// Asks the window which chats are mid-turn in `workspace` of `project`, and waits for its
    /// answer, at most `within`: `None` with no window holding the project linked, or none that
    /// answered (#2002). Asked of the links that hold the project, as its hook lines are.
    fn ask_the_window(
        &self,
        project: &str,
        workspace: &str,
        within: Duration,
    ) -> Option<Vec<String>> {
        let (answered, answer) = std::sync::mpsc::channel();
        let question = {
            let mut state = self.state();
            let holders: Vec<u64> = state
                .projects
                .get(project)
                .map(|held| held.links.keys().copied().collect())
                .unwrap_or_default();
            state.asked += 1;
            let question = state.asked;
            let pushed = Pushed::WhoIsMidTurn {
                question,
                project: project.to_owned(),
                workspace: workspace.to_owned(),
            };
            let mut asked = false;
            state.subscribers.retain(|subscriber| {
                if !holders.contains(&subscriber.link) {
                    return true;
                }
                let sent = subscriber.told.try_send(pushed.clone()).is_ok();
                asked |= sent;
                sent
            });
            if !asked {
                return None;
            }
            state.questions.insert(question, answered);
            question
        };
        let said = answer.recv_timeout(within).ok().flatten();
        self.state().questions.remove(&question);
        said
    }

    /// Push line `id` that `project`'s hooks wrote to every subscriber whose link holds the
    /// project, letting go of one that is gone or has stopped reading: whether one was told.
    fn hand_on(&self, project: &str, id: u64, line: serde_json::Value) -> bool {
        let mut state = self.state();
        let Some(holders) = state
            .projects
            .get(project)
            .map(|held| held.links.keys().copied().collect::<Vec<u64>>())
        else {
            return false;
        };
        let pushed = Pushed::Heard {
            project: project.to_owned(),
            heard: id,
            line,
        };
        let mut told = false;
        state.subscribers.retain(|subscriber| {
            if !holders.contains(&subscriber.link) {
                return true;
            }
            let sent = subscriber.told.try_send(pushed.clone()).is_ok();
            told |= sent;
            sent
        });
        told
    }

    /// What hands a line `project`'s hooks wrote on to whoever holds it: nothing, once this
    /// host has gone.
    fn handing_on(&self, project: &str) -> HandOn {
        let host = Arc::downgrade(&self.inner);
        let project = project.to_owned();
        Arc::new(move |id, line| {
            host.upgrade()
                .is_some_and(|inner| Hosting { inner }.hand_on(&project, id, line))
        })
    }

    /// The hook channel of `project`, where it has one.
    fn channel(&self, project: &str) -> Option<Arc<dyn Channel>> {
        self.state()
            .projects
            .get(project)
            .and_then(|held| held.channel.clone())
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
        let state = self.state();
        let mut chats: Vec<Chat> = state
            .programs
            .iter()
            .filter(|(_, hosted)| knows_browsers || hosted.kind != ProgramKind::Browser)
            .map(|(id, hosted)| Chat {
                chat: id.clone(),
                project: hosted.project.clone(),
                project_path: state
                    .projects
                    .get(&hosted.project)
                    .map(|held| held.path.display().to_string()),
                title: None,
                state: if hosted.ended { "ended" } else { "running" }.into(),
                number: (hosted.kind != ProgramKind::Browser).then_some(hosted.number),
                run: hosted.run.clone(),
                kind: (hosted.kind == ProgramKind::Browser).then_some(ProgramKind::Browser),
            })
            .collect();
        chats.sort_by(|a, b| (&a.project, a.number).cmp(&(&b.project, b.number)));
        chats
    }

    fn hold(&self, project: String, path: String, peer: &Peer) -> Result<Answer, Refusal> {
        let path = PathBuf::from(path);
        if !path.is_absolute() || !path.is_dir() {
            return Err(Refusal {
                code: session::NO_SUCH_PROJECT.into(),
                why: format!(
                    "{} is not a folder on this machine, so project {project} is not held",
                    path.display()
                ),
            });
        }
        let mut state = self.state();
        if !state.projects.contains_key(&project) {
            // Opened under the hold, so two holds of a project never open two channels on one
            // socket. Opening binds a socket: no wait on anything else.
            let channel = match self
                .inner
                .hear
                .open(&project, &path, self.handing_on(&project))
            {
                Ok(channel) => channel,
                Err(why) => {
                    crate::say(&format!(
                        "project {project}'s hooks are not heard, so its chats' states and \
                         asks will not reach the window: {why}"
                    ));
                    None
                }
            };
            state.projects.insert(
                project.clone(),
                Held {
                    path: path.clone(),
                    links: HashMap::new(),
                    channel,
                },
            );
        }
        let Some(held) = state.projects.get_mut(&project) else {
            return Err(no_such_project(&project));
        };
        // A project held is where it was first held until every hold is let go of: no client
        // moves another's.
        if held.path != path {
            return Err(Refusal {
                code: session::NOT_ALLOWED.into(),
                why: format!(
                    "project {project} is held at {}, not at {}",
                    held.path.display(),
                    path.display()
                ),
            });
        }
        *held.links.entry(link_of(peer)).or_default() += 1;
        Ok(Answer::Done)
    }

    fn let_go(&self, project: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let link = link_of(peer);
        let mut state = self.state();
        let Some(held) = state.projects.get_mut(project) else {
            return Err(no_such_project(project));
        };
        match held.links.get_mut(&link) {
            Some(holds) if *holds > 1 => *holds -= 1,
            Some(_) => {
                held.links.remove(&link);
            }
            // A link lets go only of its own holds.
            None => return Err(no_such_project(project)),
        }
        let gone = held
            .links
            .is_empty()
            .then(|| state.projects.remove(project));
        // Nobody holds it now: it is saved no more. A link that ended is not a let go
        // ([`Self::let_go_of_the_links`]), so a crashed window's project goes on saving.
        let saving = gone
            .is_some()
            .then(|| state.saving.remove(project))
            .flatten();
        // Its hook channel stops outside the hold: a line it is handing on waits for the hold.
        drop(state);
        drop(gone);
        if let Some(saving) = saving {
            tokio::task::spawn_blocking(move || drop(saving));
        }
        Ok(Answer::Done)
    }

    /// A link ended: its holds go with it.
    fn let_go_of_the_links(&self, peer: &Peer) {
        let link = link_of(peer);
        let gone: Vec<Held> = {
            let mut state = self.state();
            let held = std::mem::take(&mut state.projects);
            let (kept, gone): (Vec<_>, Vec<_>) = held
                .into_iter()
                .map(|(project, mut held)| {
                    held.links.remove(&link);
                    (project, held)
                })
                .partition(|(_, held)| !held.links.is_empty());
            state.projects = kept.into_iter().collect();
            gone.into_iter().map(|(_, held)| held).collect()
        };
        // Their hook channels stop outside the hold, as a let go's does.
        drop(gone);
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
            .map(|hosted| Arc::clone(&hosted.running))
            .collect();
        let _ = tokio::task::spawn_blocking(move || stop_together(programs)).await;
        Answer::KillSwitch(switch)
    }

    async fn start(&self, start: StartProgram, peer: &Peer) -> Result<Answer, Refusal> {
        self.let_through(&start, peer)?;
        let size = Size {
            columns: start.cols.unwrap_or(80),
            rows: start.rows.unwrap_or(24),
        }
        .at_least_min();
        // The number and the id are taken under one hold, so two starts never share either.
        let (program, number, path) = {
            let mut state = self.state();
            let Some(path) = state
                .projects
                .get(&start.project)
                .map(|held| held.path.clone())
            else {
                return Err(no_such_project(&start.project));
            };
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
                    .any(|hosted| hosted.project == start.project && hosted.number == number)
            };
            // The confinement it runs in becomes the program's, under the same hold, so no two
            // programs run in one. Only one this link made for this project, and nothing runs in
            // yet.
            if let Some(confinement) = start.launch.as_ref().and_then(|l| l.confinement.as_ref()) {
                match state.confinements.get_mut(confinement) {
                    Some(held)
                        if held.project == start.project
                            && held.owner == Owner::Link(link_of(peer)) =>
                    {
                        held.owner = Owner::Program(program.clone());
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
            let dealt = state.dealt.get(&start.project).copied().unwrap_or(0);
            let number = match start.number {
                Some(wanted) if wanted > 0 && !in_use(wanted, &state) => wanted,
                _ => match dealt.checked_add(1) {
                    Some(next) if !in_use(next, &state) => next,
                    _ => return Err(no_number_left(&start.project)),
                },
            };
            state.dealt.insert(start.project.clone(), dealt.max(number));
            (program, number, path)
        };
        // Its token, issued by the project's hook channel and composed into its start: the
        // window's launch says where it runs, never where it reports (#2000).
        let channel = self.channel(&start.project);
        let mut start = start;
        if let (Some(channel), Some(launch)) = (&channel, start.launch.as_mut()) {
            match channel.report_to(number) {
                Ok(reporting) => launch.reporting = Some(reporting),
                Err(why) => {
                    self.end_confinements_of(&program);
                    return Err(not_started(format!(
                        "chat {number} could not be issued its token: {why}"
                    )));
                }
            }
        }
        // On every way it fails, nothing runs in its confinement: that ends with the start.
        let forget = |channel: &Option<Arc<dyn Channel>>| {
            if let Some(channel) = channel {
                channel.forget(number);
            }
        };
        let spec = match self.inner.compose.compose(&ToCompose {
            project: &start.project,
            project_path: &path,
            number,
            size,
            start: &start,
        }) {
            Ok(spec) => spec,
            Err(why) => {
                forget(&channel);
                self.end_confinements_of(&program);
                return Err(not_started(why));
            }
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
        self.state().programs.insert(
            program.clone(),
            Hosted {
                kind: start.kind,
                project: start.project.clone(),
                number,
                run: start.run.clone(),
                running: Arc::clone(&running),
                size,
                process,
                cwd: start
                    .launch
                    .as_ref()
                    .and_then(|launch| launch.cwd.as_ref())
                    .map(PathBuf::from),
                ended: false,
            },
        );
        // A stop thrown while this one was being started: it is listed by now, so either the
        // stop's sweep reached it or this check sees the switch thrown.
        if self.let_through(&start, peer).is_err() {
            let stopping = Arc::clone(&running);
            let _ = tokio::task::spawn_blocking(move || stopping.stop()).await;
        }
        let hosting = self.clone();
        let ended = program.clone();
        let run = start.run.clone();
        let project = start.project.clone();
        running.when_it_ends(Box::new(move |exit| {
            {
                let mut state = hosting.state();
                if let Some(hosted) = state.programs.get_mut(&ended) {
                    hosted.ended = true;
                }
                // A program's end is the moment its work is done: its project's saver hears it.
                if let Some(saving) = state.saving.get(&project) {
                    saving.poke(Poke::ProgramEnded);
                }
            }
            if let (Some(channel), Some(pid)) = (&channel, process) {
                channel.ended(number, pid);
            }
            hosting.tell(&Pushed::Ended {
                program: ended,
                run,
                exit,
            });
        }));
        Ok(Answer::ProgramStarted {
            program,
            number: Some(number),
            process,
        })
    }

    /// Start a browser (#2012): in a project this host holds, with a fresh data folder of its
    /// own under the host's store, removed once it has ended.
    async fn start_browser(&self, start: StartProgram, peer: &Peer) -> Result<Answer, Refusal> {
        // Nothing starts while the kill switch is thrown (ADR 0071), a browser included.
        self.let_through(&start, peer)?;
        let program = {
            let state = self.state();
            if !state.projects.contains_key(&start.project) {
                return Err(no_such_project(&start.project));
            }
            match &start.program {
                Some(id) if state.programs.contains_key(id) => {
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
            }
        };
        // Starting an engine forks and waits for it to answer: off the runtime.
        let hosting = self.clone();
        let named = program.clone();
        let started = tokio::task::spawn_blocking(move || {
            let data = hosting.inner.browsers.data_folders()?.join(&named);
            match hosting.inner.browsers.start(&data) {
                Ok(screen) => Ok((screen, data)),
                Err(why) => {
                    let _ = std::fs::remove_dir_all(&data);
                    Err(why)
                }
            }
        })
        .await
        .map_err(|why| not_started(why.to_string()))
        .and_then(|started| started.map_err(not_started));
        let (screen, data) = started?;
        let (columns, rows) = screen.size();
        let running: Arc<dyn Running> = Arc::new(Browser::new(screen));
        let process = running.process();
        self.state().programs.insert(
            program.clone(),
            Hosted {
                kind: ProgramKind::Browser,
                project: start.project.clone(),
                number: 0,
                run: start.run.clone(),
                running: Arc::clone(&running),
                size: Size { columns, rows },
                process,
                // A browser writes no project's files: its project's saver does not wait for it.
                cwd: None,
                ended: false,
            },
        );
        let hosting = self.clone();
        let ended = program.clone();
        let run = start.run.clone();
        running.when_it_ends(Box::new(move |exit| {
            if let Some(hosted) = hosting.state().programs.get_mut(&ended) {
                hosted.ended = true;
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
        })
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
            let source = hosted
                .running
                .view(kind, role, rate)
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
        let running = self.running(program)?;
        let _ = tokio::task::spawn_blocking(move || running.stop()).await;
        Ok(Answer::Done)
    }

    /// Deals the next number in held `project` (#2000): it only rises, so no chat has had it.
    fn deal(&self, project: &str) -> Result<Answer, Refusal> {
        let mut state = self.state();
        if !state.projects.contains_key(project) {
            return Err(no_such_project(project));
        }
        let dealt = state.dealt.entry(project.to_owned()).or_default();
        *dealt = dealt
            .checked_add(1)
            .ok_or_else(|| no_number_left(project))?;
        Ok(Answer::Dealt { number: *dealt })
    }

    /// The numbers up to `dealt` went out in held `project` before: none of them is dealt again.
    fn dealt_already(&self, project: &str, dealt: u32) -> Result<Answer, Refusal> {
        let mut state = self.state();
        if !state.projects.contains_key(project) {
            return Err(no_such_project(project));
        }
        let was = state.dealt.entry(project.to_owned()).or_default();
        *was = (*was).max(dealt);
        Ok(Answer::Done)
    }

    /// The window's reply to line `heard` of `project`'s hooks, from a link that holds it.
    fn reply_heard(
        &self,
        project: &str,
        heard: u64,
        reply: serde_json::Value,
        peer: &Peer,
    ) -> Result<Answer, Refusal> {
        let channel = {
            let state = self.state();
            let held = state
                .projects
                .get(project)
                .filter(|held| held.links.contains_key(&link_of(peer)))
                .ok_or_else(|| no_such_project(project))?;
            held.channel.clone()
        };
        if let Some(channel) = channel {
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
            state.closed.extend(hosted.process);
            let confinements = take_confinements(&mut state, |owner| {
                *owner == Owner::Program(program.to_owned())
            });
            (hosted, confinements)
        };
        // A line for it is not read from now on.
        if let Some(channel) = self.channel(&hosted.project) {
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
        let path = {
            let state = self.state();
            if state.confinements.contains_key(&id) {
                return Err(already_confined(&id));
            }
            state
                .projects
                .get(&confine.project)
                .map(|held| held.path.clone())
                .ok_or_else(|| no_such_project(&confine.project))?
        };
        let weak = Arc::downgrade(&self.inner);
        let tell = {
            let (weak, id) = (Weak::clone(&weak), id.clone());
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
            let id = id.clone();
            Arc::new(move || {
                weak.upgrade()
                    .is_some_and(|inner| Hosting { inner }.askable(&id))
            })
        };
        let hosting = self.clone();
        let confining = tokio::task::spawn_blocking(move || {
            hosting.inner.confine.confine(
                &ToConfine {
                    project_path: &path,
                    confine: &confine,
                },
                tell,
                askable,
            )
        })
        .await
        .map_err(|why| not_started(why.to_string()))?
        .map_err(not_started)?;
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
            state.confinements.insert(
                id.clone(),
                Confinement {
                    project,
                    owner: Owner::Link(link_of(peer)),
                    confining,
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

    /// Whether confinement `id`'s proxy may hold a connection while the person is asked: a
    /// program runs in it, and a window that may answer is there to be asked.
    fn askable(&self, id: &str) -> bool {
        let state = self.state();
        let claimed = state
            .confinements
            .get(id)
            .is_some_and(|held| matches!(held.owner, Owner::Program(_)));
        claimed
            && state
                .subscribers
                .iter()
                .any(|subscriber| subscriber.confines && !subscriber.told.is_closed())
    }

    fn confinement(&self, id: &str) -> Result<Arc<dyn Confining>, Refusal> {
        self.state()
            .confinements
            .get(id)
            .map(|held| Arc::clone(&held.confining))
            .ok_or_else(|| no_such_confinement(id))
    }

    /// Ends confinement `id`, whatever runs in it, and answers once it has ended: its proxy
    /// carries nothing by the time the client hears so. One the host does not hold is let go
    /// of already.
    async fn let_go_of_confinement(&self, id: &str) -> Answer {
        let ending: Vec<Arc<dyn Confining>> = self
            .state()
            .confinements
            .remove(id)
            .map(|held| held.confining)
            .into_iter()
            .collect();
        if !ending.is_empty() {
            // A proxy's end waits on its listeners: off the runtime, and waited for.
            let _ = tokio::task::spawn_blocking(move || drop(ending)).await;
        }
        Answer::Done
    }
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

/// Ends `confinements` off the runtime: a proxy's end waits on its listeners.
fn end_off_the_runtime(confinements: Vec<Arc<dyn Confining>>) {
    if confinements.is_empty() {
        return;
    }
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => {
            runtime.spawn_blocking(move || drop(confinements));
        }
        Err(_) => drop(confinements),
    }
}

impl Hosting {
    /// Save `project`, which a link holds, unless it is saved already: one saver per project.
    async fn start_saving(&self, project: String) -> Result<Answer, Refusal> {
        let path = self
            .state()
            .projects
            .get(&project)
            .map(|held| held.path.clone())
            .ok_or_else(|| no_such_project(&project))?;
        // A saver's start reads the project and starts threads: off the runtime.
        let hosting = self.clone();
        tokio::task::spawn_blocking(move || hosting.saver_for(project, path))
            .await
            .map_err(|why| not_started(why.to_string()))?
    }

    fn saver_for(&self, project: String, path: PathBuf) -> Result<Answer, Refusal> {
        let _one_at_a_time = self
            .inner
            .starting_a_saver
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.state().saving.contains_key(&project) {
            return Ok(Answer::Done);
        }
        let Some(save) = self.inner.save.get() else {
            return Err(not_started(
                "this session host was given no way to save a project".into(),
            ));
        };
        // Each weakly: the saver is the host's, and must not keep it.
        let told = {
            let host = Arc::downgrade(&self.inner);
            let project = project.clone();
            Box::new(move |looked: Looked| {
                if let Some(inner) = host.upgrade() {
                    Hosting { inner }.tell(&Pushed::Saving {
                        project: project.clone(),
                        saved: looked.saved,
                        moved_tree: looked.moved_tree,
                    });
                }
            })
        };
        let working = {
            let host = Arc::downgrade(&self.inner);
            let project = project.clone();
            Box::new(move || {
                host.upgrade()
                    .map(|inner| Hosting { inner }.working_in(&project))
                    .unwrap_or_default()
            })
        };
        let ask_the_window = {
            let host = Arc::downgrade(&self.inner);
            let project = project.clone();
            Box::new(move |workspace: &str| {
                host.upgrade().and_then(|inner| {
                    Hosting { inner }.ask_the_window(&project, workspace, ASK_WITHIN)
                })
            })
        };
        let saving = save
            .save(ToSave {
                project: project.clone(),
                project_path: path,
                told,
                working,
                ask_the_window,
            })
            .map_err(not_started)?;
        self.state().saving.insert(project, saving);
        Ok(Answer::Done)
    }

    /// The programs running in `project` now, by number.
    fn working_in(&self, project: &str) -> Vec<Working> {
        let mut working: Vec<Working> = self
            .state()
            .programs
            .values()
            .filter(|hosted| {
                hosted.project == project && !hosted.ended && hosted.kind != ProgramKind::Browser
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
    async fn stop_saving(&self, project: &str) -> Result<Answer, Refusal> {
        let saving = self.state().saving.remove(project);
        if let Some(saving) = saving {
            let _ = tokio::task::spawn_blocking(move || drop(saving)).await;
        }
        Ok(Answer::Done)
    }

    fn fetch(&self, project: &str) -> Result<Answer, Refusal> {
        let state = self.state();
        let saving = state.saving.get(project).ok_or_else(|| Refusal {
            code: session::NO_SUCH_PROJECT.into(),
            why: format!("this session host does not save project {project}"),
        })?;
        saving.poke(Poke::Fetch);
        Ok(Answer::Done)
    }
}

impl session::Host for Hosting {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(self.list(peer.version()))),
            Command::HoldProject {
                project,
                project_path,
            } => self.hold(project, project_path, peer),
            Command::LetGoOfProject { project } => self.let_go(&project, peer),
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
            Command::Deal { project } => self.deal(&project),
            Command::DealtAlready { project, dealt } => self.dealt_already(&project, dealt),
            Command::ReplyHeard {
                project,
                heard,
                reply,
            } => self.reply_heard(&project, heard, reply, peer),
            Command::StartSaving { project } => self.start_saving(project).await,
            Command::StopSaving { project } => self.stop_saving(&project).await,
            Command::FetchProject { project } => self.fetch(&project),
            Command::MidTurn { question, chats } => {
                // An answer nobody waits for any more (it came late) is done all the same.
                if let Some(waiting) = self.state().questions.remove(&question) {
                    let _ = waiting.send(chats);
                }
                Ok(Answer::Done)
            }
            Command::Confine(confine) => self.confine(*confine, peer).await,
            Command::LetGoOfConfinement { confinement } => {
                Ok(self.let_go_of_confinement(&confinement).await)
            }
            Command::ChangeReach {
                confinement,
                change,
            } => {
                let confining = self.confinement(&confinement)?;
                confining
                    .change(&change)
                    .map(|retry| Answer::ReachChanged { retry })
                    .map_err(not_started)
            }
            Command::ReachOf { confinement } => {
                Ok(Answer::Reach(self.confinement(&confinement)?.now()))
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
            Command::Answer { chat, .. } => Err(Refusal {
                code: session::NO_SUCH_CHAT.into(),
                why: format!("program {chat} has no ask this host holds"),
            }),
            // `serve` keeps these: 1.0's words come as the hosted-program ones, and the
            // subscription is the link's.
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
        // What the link confined that nothing runs in yet goes with it; what a program runs in
        // stays with the program (#1999).
        let link = link_of(peer);
        let ending = take_confinements(&mut self.state(), |owner| *owner == Owner::Link(link));
        end_off_the_runtime(ending);
    }

    async fn subscribe(
        &self,
        _since: Since,
        peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let (told, notices) = mpsc::channel(MOST_NOTICES_QUEUED);
        self.state().subscribers.push(Subscriber {
            link: link_of(peer),
            told,
            confines: peer.scope().may(Power::Confine),
        });
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
}

/// Whether `id` may name a folder: letters, digits, `-` and `_`, as a ULID is.
fn a_plain_name(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn no_such_program(program: &str) -> Refusal {
    Refusal {
        code: session::NO_SUCH_CHAT.into(),
        why: format!("this session host holds no program {program}"),
    }
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

fn already_confined(id: &str) -> Refusal {
    Refusal {
        code: session::MALFORMED.into(),
        why: format!("this session host already holds a confinement {id}"),
    }
}

fn not_started(why: String) -> Refusal {
    Refusal {
        code: session::NOT_STARTED.into(),
        why,
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
            let hosted: Vec<Hosted> = state.programs.drain().map(|(_, hosted)| hosted).collect();
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
