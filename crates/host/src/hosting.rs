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
//! **Every hosted program is inside a chat to the admission check.** The holder the check is
//! given is the window, when the window started this host, or the host itself: a hosted
//! program, and everything it starts, is below both from its first instruction, so a peer among
//! them is refused every person's scope (ADR 0068 §5, FD-27's item 2). Each program's pid is
//! listed too, so one that left the host's tree and kept its session is refused as well.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use purlis_session_protocol::local::Chats;
use purlis_session_protocol::session::{
    self, Answer, Attached, Chat, Command, Exit, Launch, Peer, ProgramKind, Pushed, Readiness,
    Refusal, Since, StartProgram, ViewKind,
};
use purlis_session_protocol::view::{Attachment, Source, ViewId};
use purlis_terminal::engine::{AlacrittyEngine, SCROLLBACK, Size};
use purlis_terminal::session::{Session, Spec};
use tokio::sync::mpsc;

use crate::serving::Window;

/// A start, as the host hands it to its [`Compose`]: the project it is in, the number the chat
/// was dealt there, and what the client asked for.
pub struct Starting<'a> {
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
    fn compose(&self, starting: &Starting<'_>) -> Result<Spec, String>;
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
    fn compose(&self, starting: &Starting<'_>) -> Result<Spec, String> {
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
    fn compose(&self, _starting: &Starting<'_>) -> Result<Spec, String> {
        Err("this session host was given no way to compose a start".into())
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
    run: Box<dyn Run>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    projects: HashMap<String, Held>,
    programs: HashMap<String, Hosted>,
    /// Each project's highest number dealt: it only rises, so a number names one chat.
    dealt: HashMap<String, u32>,
    subscribers: Vec<mpsc::Sender<Pushed>>,
    /// The window that started this host, if one did.
    window: Option<Window>,
}

struct Held {
    path: PathBuf,
    holders: usize,
}

struct Hosted {
    project: String,
    number: u32,
    run: Option<String>,
    running: Arc<dyn Running>,
    size: Size,
    /// Its process as it started, kept for the admission check after it ends.
    process: Option<u32>,
    ended: bool,
}

impl Hosting {
    /// A host whose starts `compose` composes, each run in a terminal of its own.
    pub fn new(compose: impl Compose) -> Hosting {
        Hosting::running_with(compose, Terminals)
    }

    /// A host whose starts `compose` composes and `run` runs.
    pub fn running_with(compose: impl Compose, run: impl Run) -> Hosting {
        Hosting {
            inner: Arc::new(Inner {
                compose: Box::new(compose),
                run: Box::new(run),
                state: Mutex::default(),
            }),
        }
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

    /// Tell every subscriber, letting go of one that is gone or has stopped reading.
    fn tell(&self, pushed: &Pushed) {
        self.state()
            .subscribers
            .retain(|subscriber| subscriber.try_send(pushed.clone()).is_ok());
    }

    fn running(&self, program: &str) -> Result<Arc<dyn Running>, Refusal> {
        self.state()
            .programs
            .get(program)
            .map(|hosted| Arc::clone(&hosted.running))
            .ok_or_else(|| no_such_program(program))
    }

    fn list(&self) -> Vec<Chat> {
        let state = self.state();
        let mut chats: Vec<Chat> = state
            .programs
            .iter()
            .map(|(id, hosted)| Chat {
                chat: id.clone(),
                project: hosted.project.clone(),
                project_path: state
                    .projects
                    .get(&hosted.project)
                    .map(|held| held.path.display().to_string()),
                title: None,
                state: if hosted.ended { "ended" } else { "running" }.into(),
                number: Some(hosted.number),
                run: hosted.run.clone(),
            })
            .collect();
        chats.sort_by(|a, b| (&a.project, a.number).cmp(&(&b.project, b.number)));
        chats
    }

    fn hold(&self, project: String, path: String) -> Result<Answer, Refusal> {
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
        let held = state.projects.entry(project).or_insert(Held {
            path: path.clone(),
            holders: 0,
        });
        held.path = path;
        held.holders += 1;
        Ok(Answer::Done)
    }

    fn let_go(&self, project: &str) -> Result<Answer, Refusal> {
        let mut state = self.state();
        match state.projects.get_mut(project) {
            Some(held) if held.holders > 1 => held.holders -= 1,
            Some(_) => {
                state.projects.remove(project);
            }
            None => return Err(no_such_project(project)),
        }
        Ok(Answer::Done)
    }

    async fn start(&self, start: StartProgram) -> Result<Answer, Refusal> {
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
            let dealt = state.dealt.get(&start.project).copied().unwrap_or(0);
            let number = match start.number {
                Some(wanted) if wanted > 0 && !in_use(wanted, &state) => wanted,
                _ => dealt + 1,
            };
            state.dealt.insert(start.project.clone(), dealt.max(number));
            (program, number, path)
        };
        let spec = self
            .inner
            .compose
            .compose(&Starting {
                project: &start.project,
                project_path: &path,
                number,
                size,
                start: &start,
            })
            .map_err(not_started)?;
        // Starting a program opens a terminal and forks: off the runtime.
        let hosting = self.clone();
        let running = tokio::task::spawn_blocking(move || hosting.inner.run.run(spec))
            .await
            .map_err(|why| not_started(why.to_string()))?
            .map_err(not_started)?;
        let process = running.process();
        self.state().programs.insert(
            program.clone(),
            Hosted {
                project: start.project.clone(),
                number,
                run: start.run.clone(),
                running: Arc::clone(&running),
                size,
                process,
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

    async fn attach(&self, program: &str, peer: &Peer) -> Result<Answer, Refusal> {
        let (source, size) = {
            let state = self.state();
            let hosted = state
                .programs
                .get(program)
                .ok_or_else(|| no_such_program(program))?;
            (hosted.running.source(), hosted.size)
        };
        let view = peer.views().open(source).await.map_err(|why| Refusal {
            code: session::NOT_STARTED.into(),
            why: format!("the view could not be opened: {why}"),
        })?;
        Ok(Answer::Attached(Attached {
            view: view.0,
            cols: size.columns,
            rows: size.rows,
            kind: Some(ViewKind::Terminal),
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

    async fn close(&self, program: &str) -> Result<Answer, Refusal> {
        let hosted = self
            .state()
            .programs
            .remove(program)
            .ok_or_else(|| no_such_program(program))?;
        // Ended here, off the runtime; the views end with its output.
        let _ = tokio::task::spawn_blocking(move || {
            hosted.running.stop();
            drop(hosted);
        })
        .await;
        Ok(Answer::Done)
    }
}

impl session::Host for Hosting {
    async fn call(&self, command: Command, peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(self.list())),
            Command::HoldProject {
                project,
                project_path,
            } => self.hold(project, project_path),
            Command::LetGoOfProject { project } => self.let_go(&project),
            Command::StartProgram(start) => match start.kind {
                ProgramKind::Harness | ProgramKind::Shell => self.start(*start).await,
            },
            Command::AttachView {
                program,
                kind: ViewKind::Terminal,
            } => self.attach(&program, peer).await,
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
            Command::Close { program } => self.close(&program).await,
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

    async fn subscribe(
        &self,
        _since: Since,
        _peer: &Peer,
    ) -> Result<mpsc::Receiver<Pushed>, Refusal> {
        let (told, notices) = mpsc::channel(MOST_NOTICES_QUEUED);
        self.state().subscribers.push(told);
        Ok(notices)
    }
}

/// Who the admission check takes to be inside a chat: everything below the window that started
/// this host, or below the host itself, and every hosted program by its pid.
impl Chats for Hosting {
    fn programs(&self) -> Vec<u32> {
        self.state()
            .programs
            .values()
            .filter_map(|hosted| hosted.process)
            .collect()
    }

    fn holder(&self) -> Option<u32> {
        Some(match self.state().window {
            Some(Window(window)) => window,
            None => std::process::id(),
        })
    }
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
        let hosted: Vec<Hosted> = self
            .state()
            .programs
            .drain()
            .map(|(_, hosted)| hosted)
            .collect();
        let ending: Vec<_> = hosted
            .into_iter()
            .filter_map(|hosted| {
                std::thread::Builder::new()
                    .name("purlisd-ending".into())
                    .spawn(move || hosted.running.stop())
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
}
