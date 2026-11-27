// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each step of a remote start takes** (#7182), and **the contract its lines keep**
//! (#7864, decided in #7189): a [`tracing`] span per step, named and closed with its elapsed
//! milliseconds as one line in the local diagnostic log.
//!
//! Local only (ADR 0075: no telemetry): nothing here leaves the machine, and nothing here is a
//! store of its own: the lines go to the 7-day diagnostic log (`charter.<date>.log`) and nowhere
//! else.
//!
//! ```text
//! purlis: step ssh_forward_open done after 412 ms (pool: lab)
//! purlis: step runner_start done after 38 ms (parent: container_check)
//! purlis: step link_dial refused after 2210 ms (pool: lab, cause: refused, attempt: 3)
//! purlis: step harness_spawn done after 95 ms (start: 01JD3Q4M0R8W2ZK7A6F1B9CXYE)
//! ```
//!
//! **The contract** (#7864):
//!
//! - **The names** are [`Name`]'s, listed once in [`Name::ALL`]: renaming one is a breaking
//!   change, with a changelog line. A `docker` call's line ([`begin_debug`]) is not part of it.
//! - **The levels**: `done`, `cancelled` and `gave_up` at `info`; `refused` at `warn`. A `docker`
//!   call's line is `debug`, however it ended.
//! - **Always**: `step`, `outcome`, `ms`. **Where known**: `pool` (the pool's alias), `parent`,
//!   a typed `cause` ([`Cause`], on a step that was not done), `attempt`, and `start`, the id the
//!   window's Start made ([`StartId`]), which goes over the link to the runner in the start it
//!   asks for, so the host's `harness_spawn` line there carries it too.
//! - **Never** a host, a user, a path, a `docker` argument or a token. A pool's alias is written
//!   only where it has the shape of a pool's name (lower-case letters, digits and dashes), and a
//!   start's id only where it has the shape of one, so a value of any other shape, handed in by
//!   mistake or by a peer, is left out of the line.
//!
//! **A step that is not closed ends `refused`.** The guard is dropped on every `?` and every
//! early return, so its default is the failing way out; a success says [`Step::done`]. Two more
//! ways out are not refusals: [`Step::cancelled`] (the person cancelled it) and
//! [`Step::gave_up`] (the wait ran out while the work it waits for may still finish), and
//! [`Step::quiet`] writes nothing, for a step that found nothing to do. A miss that is the
//! answer (an inspect that finds no container) is a success of the step: the step is done and its
//! caller reads the answer.
//!
//! **A step inside another names it**: a `parent` field on its line. The parent's time contains
//! its children's, so a table that sums every line counts them twice; **sum the lines with no
//! parent**. A step begun while another runs names it on its own, wherever the work crosses a
//! thread or a task, as long as the crossing carries the [`Context`] ([`Step::inside`],
//! [`Step::context`], [`current`]). A parent is named only while it still runs. A `docker` call
//! ([`begin_debug`]) is inside whichever step made it and names none: it is a `debug` line,
//! never part of a sum. A step in another process's log (the host's `harness_spawn`, on the
//! runner) is in another sum, joined to this one by its `start`.
//!
//! The default log carries the shape of a start; Settings › Diagnostics' "Detailed remote log
//! for one hour" lets `purlis::step`'s `debug` lines through, every `docker` call among them.
//!
//! Hold the guard from [`begin`] for the step's length. It works across `.await`s, as it times
//! wall-clock from creation to drop.

use std::cell::RefCell;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// The target of every step's line, so a filter or a test finds them alone.
pub const TARGET: &str = "purlis::step";

/// **A step of the contract, by its name** (#7864). Renaming one is a breaking change: a person's
/// grep, a filter or a table that reads the log finds it by this word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Name {
    /// Lock, resolve, `ssh -L` spawn, wait for the socket (`sshroute/forward.rs`).
    SshForwardOpen,
    /// The `docker info` that pins the engine, through the forward (`sshroute/connect.rs`).
    EnginePinCheck,
    /// The runner container's check before each link (`Docker::resumed`).
    ContainerCheck,
    /// `docker start` of a stopped runner (`Docker::resumed`).
    RunnerStart,
    /// One try of a kept link to another host: its connector and its handshake, with its
    /// `attempt` since the link was last up and, where it failed, its `cause`
    /// (`hostlink::keep_linked`).
    LinkDial,
    /// A link's check (`host/connector.rs`).
    LinkCheck,
    /// The checked copy of Docker's settings (`host/connector.rs`).
    LinkPrepare,
    /// Spawning `docker exec -i … purlis bridge` (`host/connector.rs`).
    LinkExecSpawn,
    /// Hello, encrypted handshake, session proof (`hostlink::connect_kept`).
    LinkHandshake,
    /// A one-off link's whole making (`runnerlinks.rs`).
    FirstLink,
    /// The pool's profile check and keep, before the harness starts (`chats.rs`).
    ProfileReady,
    /// The window's hold of a project on its host, the runner's for a pool (`hostedsessions.rs`).
    HoldProject,
    /// The window's send of a repository's code to the host (`hostedsessions.rs::send_code`).
    CodeSend,
    /// The window's wait for `StartProgram`, over the link and relay (`hostedsessions.rs`).
    StartProgramCall,
    /// The host's own start of the program, on the runner for a pool (`host/hosting.rs`).
    HarnessSpawn,
    /// A browser engine's whole prepare (`browser-engine`).
    BrowserEnginePrepare,
    /// A browser engine's download, inside its prepare (`browser-engine`).
    BrowserEngineDownload,
    /// The engine's process start until it answers (`browser-engine`).
    BrowserEngineStart,
    /// From the engine answering until its first page is attached and its frames known
    /// (`browser-engine`'s `drive::Browser`).
    BrowserFirstPage,
    /// From a view's stream being drawn until its first frame or chunk is (`hostedsessions.rs`).
    ViewFirstFrame,
}

impl Name {
    /// Every step of the contract, in the order a start meets them.
    pub const ALL: [Name; 20] = [
        Name::SshForwardOpen,
        Name::EnginePinCheck,
        Name::ContainerCheck,
        Name::RunnerStart,
        Name::LinkDial,
        Name::LinkCheck,
        Name::LinkPrepare,
        Name::LinkExecSpawn,
        Name::LinkHandshake,
        Name::FirstLink,
        Name::ProfileReady,
        Name::HoldProject,
        Name::CodeSend,
        Name::StartProgramCall,
        Name::HarnessSpawn,
        Name::BrowserEnginePrepare,
        Name::BrowserEngineDownload,
        Name::BrowserEngineStart,
        Name::BrowserFirstPage,
        Name::ViewFirstFrame,
    ];

    /// The step's word, as its line says it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Name::SshForwardOpen => "ssh_forward_open",
            Name::EnginePinCheck => "engine_pin_check",
            Name::ContainerCheck => "container_check",
            Name::RunnerStart => "runner_start",
            Name::LinkDial => "link_dial",
            Name::LinkCheck => "link_check",
            Name::LinkPrepare => "link_prepare",
            Name::LinkExecSpawn => "link_exec_spawn",
            Name::LinkHandshake => "link_handshake",
            Name::FirstLink => "first_link",
            Name::ProfileReady => "profile_ready",
            Name::HoldProject => "hold_project",
            Name::CodeSend => "code_send",
            Name::StartProgramCall => "start_program_call",
            Name::HarnessSpawn => "harness_spawn",
            Name::BrowserEnginePrepare => "browser_engine_prepare",
            Name::BrowserEngineDownload => "browser_engine_download",
            Name::BrowserEngineStart => "browser_engine_start",
            Name::BrowserFirstPage => "browser_first_page",
            Name::ViewFirstFrame => "view_first_frame",
        }
    }
}

/// **Why a step did not get done**, as a word of a fixed set, never free text (#7864): free text
/// is where a host name or a path would get in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cause {
    /// The far end could not be reached: its connector did not connect.
    Unreachable,
    /// The far end answers, but refuses: its sign-in, its Docker or its forwarding fails (#7187).
    Refused,
    /// The machine the far end is reached through presents another host key, CA or engine than
    /// the pinned one (#7187).
    HostKeyChanged,
    /// The host at the far end answered with another key than the one pinned at its pairing.
    KeyChanged,
    /// The far end is gone for good: its runner was removed outside purlis (#4149).
    Gone,
    /// The link's hello or encrypted handshake failed.
    Handshake,
    /// The wait for it ran out.
    TimedOut,
    /// What it ran over ended before it did.
    Ended,
}

impl Cause {
    /// Every cause, each word once.
    pub const ALL: [Cause; 8] = [
        Cause::Unreachable,
        Cause::Refused,
        Cause::HostKeyChanged,
        Cause::KeyChanged,
        Cause::Gone,
        Cause::Handshake,
        Cause::TimedOut,
        Cause::Ended,
    ];

    /// The cause's word, as a line says it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Cause::Unreachable => "unreachable",
            Cause::Refused => "refused",
            Cause::HostKeyChanged => "host_key_changed",
            Cause::KeyChanged => "key_changed",
            Cause::Gone => "gone",
            Cause::Handshake => "handshake",
            Cause::TimedOut => "timed_out",
            Cause::Ended => "ended",
        }
    }
}

/// **The id of one Start the person pressed** (#7864): made by the window as the start begins,
/// carried on every step of it, and over the link in the start it asks for
/// (`StartProgram::start`, session protocol 1.98), so the runner's `harness_spawn` line names it
/// too and the two logs join up. A ULID's shape alone: [`StartId::parse`] takes nothing else,
/// whoever sent it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StartId(Arc<str>);

/// How many characters a start's id has: a ULID's.
const START_ID_LENGTH: usize = 26;

/// Crockford's base32, as a ULID spells one.
const BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

impl StartId {
    /// A new id, from the operating system's generator: 26 characters of Crockford's base32.
    pub fn mint() -> StartId {
        let mut bytes = [0u8; 16];
        // The generator failing leaves a clock-made id: an id only joins two logs' lines up, and
        // one that is not unique joins too many, which nobody is harmed by.
        if getrandom::fill(&mut bytes).is_err() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            bytes = now.to_be_bytes();
        }
        let value = u128::from_be_bytes(bytes);
        let spelled: String = (0..26)
            .rev()
            .map(|at| char::from(BASE32[((value >> (at * 5)) & 0x1f) as usize]))
            .collect();
        StartId(spelled.into())
    }

    /// `said` as a start's id, where it has the shape of one [`StartId::mint`] makes: 26
    /// characters of Crockford's base32. Anything else is none, so a peer cannot write a host,
    /// a path or a token (a hex one, a cloud key) into a line by it.
    pub fn parse(said: &str) -> Option<StartId> {
        (said.len() == START_ID_LENGTH && said.bytes().all(|b| BASE32.contains(&b)))
            .then(|| StartId(said.into()))
    }

    /// The id, as a line and the link say it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StartId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `alias` where it has the shape of a pool's name (`runners::valid_pool_name`: lower-case
/// letters, digits and dashes): anything else is left out of the line.
fn pool_named(alias: &str) -> Option<String> {
    (!alias.is_empty()
        && alias.len() <= 63
        && !alias.starts_with('-')
        && alias
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
    .then(|| alias.to_owned())
}

/// How a step ended, as its line says it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Not closed: the failing way out of every `?` and early return.
    Refused,
    Done,
    /// The person cancelled it: not a refusal.
    Cancelled,
    /// The wait for it ran out and the caller gave up, so the work may yet finish: neither a
    /// refusal nor a success of the step.
    GaveUp,
    /// Nothing was done, so nothing is written.
    Quiet,
}

impl Outcome {
    fn word(self) -> &'static str {
        match self {
            Outcome::Refused => "refused",
            Outcome::Quiet => "quiet",
            Outcome::Done => "done",
            Outcome::Cancelled => "cancelled",
            Outcome::GaveUp => "gave_up",
        }
    }
}

/// A step that is running: its line is written when this is dropped, `refused` unless it was
/// ended another way ([`Step::done`], [`Step::cancelled`], [`Step::gave_up`], [`Step::quiet`]).
#[must_use = "a step is timed until its guard is dropped"]
pub struct Step {
    name: &'static str,
    started: Instant,
    span: tracing::Span,
    pool: Option<String>,
    parent: Option<&'static str>,
    cause: Option<Cause>,
    attempt: Option<u32>,
    start: Option<StartId>,
    debug: bool,
    outcome: Outcome,
    /// Whether this step still runs, shared with every [`Context`] made inside it, so a context
    /// kept past its step names it no more.
    running: OnceLock<Arc<AtomicBool>>,
}

/// One step a [`Context`] is inside, and whether it still runs.
#[derive(Clone)]
struct Frame {
    name: &'static str,
    running: Arc<AtomicBool>,
}

thread_local! {
    /// The steps this thread is inside, innermost last.
    static INSIDE: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    /// The Start this thread works for, if any.
    static START: RefCell<Option<StartId>> = const { RefCell::new(None) };
}

/// The steps a piece of work is inside, and the Start it is for, carried across a thread or a
/// task (`spawn_blocking`, `tokio::spawn`) that would not see them: a step begun under
/// [`Context::run`] or [`Context::future`] names the innermost one that still runs as its
/// `parent`, and the Start as its `start`.
#[derive(Clone, Default)]
pub struct Context {
    frames: Vec<Frame>,
    start: Option<StartId>,
}

/// The steps this thread is inside now, and the Start it works for.
pub fn current() -> Context {
    Context {
        frames: INSIDE.with(|inside| inside.borrow().clone()),
        start: start_id(),
    }
}

/// The Start this thread works for, if any: what the window puts in the start it asks a host
/// for, so the host's lines name it too.
pub fn start_id() -> Option<StartId> {
    START.with(|start| start.borrow().clone())
}

/// **This thread works for the Start `id` until the answer is dropped**: what the window holds
/// for the length of a chat's start, so every step it makes, and every [`Context`] it hands to
/// another thread, names `id`. The Start it worked for before is its own again after.
pub fn starting(id: StartId) -> Starting {
    let before = START.with(|start| start.replace(Some(id.clone())));
    Starting { id, before }
}

/// A Start this thread works for ([`starting`]), until it is dropped.
#[must_use = "a start is named only while it is held"]
pub struct Starting {
    id: StartId,
    before: Option<StartId>,
}

impl Starting {
    /// The Start's id.
    pub fn id(&self) -> &StartId {
        &self.id
    }
}

impl Drop for Starting {
    fn drop(&mut self) {
        let before = self.before.take();
        START.with(|start| *start.borrow_mut() = before);
    }
}

impl Context {
    /// What is running now, working for the Start `id`: the window's Start of a chat, under
    /// which every step it makes names `id`.
    pub fn of_start(id: StartId) -> Context {
        Context {
            start: Some(id),
            ..current()
        }
    }

    /// [`Context::of_start`] by an id a peer said, where it has the shape of one; else what is
    /// running now, with no Start.
    pub fn of_start_said(said: &str) -> Context {
        Context {
            start: StartId::parse(said),
            ..current()
        }
    }

    /// These steps, for no Start: for work begun inside a Start that outlives it, as a kept
    /// link does, whose every later redial is for no Start at all.
    pub fn without_start(self) -> Context {
        Context {
            start: None,
            ..self
        }
    }

    /// Runs `then` inside these steps, for this Start.
    pub fn run<T>(&self, then: impl FnOnce() -> T) -> T {
        struct Restores(Vec<Frame>, Option<StartId>);
        impl Drop for Restores {
            fn drop(&mut self) {
                INSIDE.with(|inside| *inside.borrow_mut() = std::mem::take(&mut self.0));
                START.with(|start| *start.borrow_mut() = self.1.take());
            }
        }
        let _restores = Restores(
            INSIDE.with(|inside| inside.replace(self.frames.clone())),
            START.with(|start| start.replace(self.start.clone())),
        );
        then()
    }

    /// Polls `work` inside these steps, each time it is polled: wherever it is polled.
    pub async fn future<T>(self, work: impl Future<Output = T>) -> T {
        let mut work = std::pin::pin!(work);
        std::future::poll_fn(|task| self.run(|| work.as_mut().poll(task))).await
    }
}

/// Starts timing `name`. A step begun inside another that still runs names it as its parent,
/// and one begun for a Start names it.
pub fn begin(name: Name) -> Step {
    start(name.as_str(), None, false)
}

/// [`begin`], for a step of the pool named `alias` (the pool's own name, never its host).
pub fn begin_in(name: Name, alias: &str) -> Step {
    start(name.as_str(), pool_named(alias), false)
}

/// A line at `debug` for a call inside a step, one a start makes many of (a `docker` run,
/// `docker_<verb>`): not part of the contract, and it names no parent.
pub fn begin_debug(name: &'static str) -> Step {
    start(name, None, true)
}

fn start(name: &'static str, pool: Option<String>, debug: bool) -> Step {
    // Made only where a line would be written: a step at a level nobody reads costs a clock read.
    let span = if debug {
        if tracing::enabled!(target: "purlis::step", tracing::Level::DEBUG) {
            tracing::debug_span!(target: "purlis::step", "step", step = name)
        } else {
            tracing::Span::none()
        }
    } else if tracing::enabled!(target: "purlis::step", tracing::Level::INFO) {
        tracing::info_span!(target: "purlis::step", "step", step = name)
    } else {
        tracing::Span::none()
    };
    let parent = if debug {
        None
    } else {
        INSIDE.with(|inside| {
            inside
                .borrow()
                .iter()
                .rev()
                .find(|frame| frame.running.load(Ordering::Acquire))
                .map(|frame| frame.name)
        })
    };
    Step {
        name,
        started: Instant::now(),
        span,
        pool,
        parent,
        cause: None,
        attempt: None,
        start: if debug { None } else { start_id() },
        debug,
        outcome: Outcome::Refused,
        running: OnceLock::new(),
    }
}

impl Step {
    /// Says this step is inside `parent`, where it is so wherever it runs and no [`Context`] can
    /// say so, so its line names it and a sum can leave it out.
    pub fn under(mut self, parent: Name) -> Step {
        self.parent = Some(parent.as_str());
        self
    }

    /// Says which try this is, counted from 1 since the work last got done.
    pub fn attempt(mut self, attempt: u32) -> Step {
        self.attempt = Some(attempt);
        self
    }

    /// Says why the step will not get done, where it does not: written on a line that is not
    /// `done`, never on one that is.
    pub fn because(mut self, cause: Cause) -> Step {
        self.cause = Some(cause);
        self
    }

    /// [`Step::because`] on a step already held: what a caller learns before it lets go.
    pub fn found(&mut self, cause: Cause) {
        self.cause = Some(cause);
    }

    /// Says the Start this step is for, where no [`Context`] carries it: the host's
    /// `harness_spawn`, told it in the start it was asked for. `None` changes nothing.
    pub fn of_start(mut self, start: Option<&StartId>) -> Step {
        if let Some(start) = start {
            self.start = Some(start.clone());
        }
        self
    }

    /// Ends the step as it was for: its line reads `done`.
    pub fn done(mut self) {
        self.outcome = Outcome::Done;
    }

    /// Ends the step because the person cancelled it: its line reads `cancelled`, not `refused`.
    pub fn cancelled(mut self) {
        self.outcome = Outcome::Cancelled;
    }

    /// Ends the step because the wait for it ran out, though the work it waits for may still
    /// finish: its line reads `gave_up`, so a start that worked is never logged as refused.
    pub fn gave_up(mut self) {
        self.outcome = Outcome::GaveUp;
    }

    /// Ends the step with no line: it found nothing to do.
    pub fn quiet(mut self) {
        self.outcome = Outcome::Quiet;
    }

    /// Ends the step by `answer`: done if it is an `Ok`, refused if it is an `Err`, and handed
    /// back as it is.
    pub fn ended<T, E>(mut self, answer: Result<T, E>) -> Result<T, E> {
        self.outcome = if answer.is_ok() {
            Outcome::Done
        } else {
            Outcome::Refused
        };
        answer
    }

    fn frame(&self) -> Frame {
        Frame {
            name: self.name,
            running: Arc::clone(self.running.get_or_init(|| Arc::new(AtomicBool::new(true)))),
        }
    }

    /// What is running now, with this step inside it: for work this step starts on another
    /// thread or task, which [`Context::run`] or [`Context::future`] puts it in.
    pub fn context(&self) -> Context {
        let mut context = current();
        context.frames.push(self.frame());
        if self.start.is_some() {
            context.start = self.start.clone();
        }
        context
    }

    /// Runs `then` inside this step: a step it begins names this one its parent.
    pub fn inside<T>(&self, then: impl FnOnce() -> T) -> T {
        self.context().run(then)
    }
}

impl Drop for Step {
    fn drop(&mut self) {
        if let Some(running) = self.running.get() {
            running.store(false, Ordering::Release);
        }
        if self.outcome == Outcome::Quiet {
            return;
        }
        let level = match (self.debug, self.outcome) {
            (true, _) => tracing::Level::DEBUG,
            (false, Outcome::Refused) => tracing::Level::WARN,
            (false, _) => tracing::Level::INFO,
        };
        let enabled = match level {
            tracing::Level::DEBUG => {
                tracing::enabled!(target: "purlis::step", tracing::Level::DEBUG)
            }
            tracing::Level::WARN => tracing::enabled!(target: "purlis::step", tracing::Level::WARN),
            _ => tracing::enabled!(target: "purlis::step", tracing::Level::INFO),
        };
        if !enabled {
            return;
        }
        let ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let step = self.name;
        let outcome = self.outcome.word();
        // A cause says why a step did not get done: a step that was done has none.
        let cause = self
            .cause
            .filter(|_| self.outcome != Outcome::Done)
            .map(Cause::as_str);
        let pool = self.pool.as_deref();
        let parent = self.parent;
        let attempt = self.attempt;
        let start = self.start.as_ref().map(StartId::as_str);
        let mut said: Vec<String> = Vec::new();
        if let Some(pool) = pool {
            said.push(format!("pool: {pool}"));
        }
        if let Some(parent) = parent {
            said.push(format!("parent: {parent}"));
        }
        if let Some(cause) = cause {
            said.push(format!("cause: {cause}"));
        }
        if let Some(attempt) = attempt {
            said.push(format!("attempt: {attempt}"));
        }
        if let Some(start) = start {
            said.push(format!("start: {start}"));
        }
        let tail = if said.is_empty() {
            String::new()
        } else {
            format!(" ({})", said.join(", "))
        };
        self.span.in_scope(|| match level {
            tracing::Level::DEBUG => tracing::debug!(
                target: "purlis::step", step, outcome, ms, pool, parent, cause, attempt, start,
                "purlis: step {step} {outcome} after {ms} ms{tail}"
            ),
            tracing::Level::WARN => tracing::warn!(
                target: "purlis::step", step, outcome, ms, pool, parent, cause, attempt, start,
                "purlis: step {step} {outcome} after {ms} ms{tail}"
            ),
            _ => tracing::info!(
                target: "purlis::step", step, outcome, ms, pool, parent, cause, attempt, start,
                "purlis: step {step} {outcome} after {ms} ms{tail}"
            ),
        });
    }
}

#[cfg(test)]
mod tests;
