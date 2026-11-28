// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each step of a remote start takes** (#7182): a [`tracing`] span per step, named
//! and closed with its elapsed milliseconds as one line in the local diagnostic log.
//!
//! Local only (ADR 0075: no telemetry): nothing here leaves the machine. A step's line carries
//! its name and, where there is one, **the pool's alias**: the person's own name for their own
//! pool, written to their own local log, which is the only place it goes. Never a host name, a
//! user, a path or a command's arguments.
//!
//! ```text
//! purlis: step ssh_forward_open done after 412 ms (pool: lab)
//! purlis: step runner_start done after 38 ms (parent: container_check)
//! ```
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
//! runner) is in another sum.
//!
//! The top-level steps log at `info`; each `docker` run ([`begin_debug`]) at `debug`, so the
//! default log carries the shape of a start and `RUST_LOG=purlis::step=debug` its every call.
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
}

/// The steps a piece of work is inside, carried across a thread or a task (`spawn_blocking`,
/// `tokio::spawn`) that would not see them: a step begun under [`Context::run`] or
/// [`Context::future`] names the innermost one that still runs as its `parent`.
#[derive(Clone, Default)]
pub struct Context(Vec<Frame>);

/// The steps this thread is inside now.
pub fn current() -> Context {
    Context(INSIDE.with(|inside| inside.borrow().clone()))
}

impl Context {
    /// Runs `then` inside these steps.
    pub fn run<T>(&self, then: impl FnOnce() -> T) -> T {
        struct Restores(Vec<Frame>);
        impl Drop for Restores {
            fn drop(&mut self) {
                INSIDE.with(|inside| *inside.borrow_mut() = std::mem::take(&mut self.0));
            }
        }
        let _restores = Restores(INSIDE.with(|inside| inside.replace(self.0.clone())));
        then()
    }

    /// Polls `work` inside these steps, each time it is polled: wherever it is polled.
    pub async fn future<T>(self, work: impl Future<Output = T>) -> T {
        let mut work = std::pin::pin!(work);
        std::future::poll_fn(|task| self.run(|| work.as_mut().poll(task))).await
    }
}

/// **The steps of a profile's sign-in on a pool's runner** (#7939), each named here once, so a
/// stall in a sign-in shows in the log as the step it stalled in. When the contract's names
/// (#7864) land, each of [`sign_in::ALL`] joins them.
pub mod sign_in {
    /// From Sign in until the runner's login is started and driven, or refused.
    pub const START: &str = "profile_sign_in_start";
    /// From the login's start until its page or device code is told to the window: `refused`
    /// where it could not be followed, failed or was stopped first.
    pub const ADDRESS: &str = "profile_sign_in_address";
    /// From its page, or from a login's start in the runner's terminal, until the runner says it
    /// is signed in.
    pub const DONE: &str = "profile_sign_in_done";
    /// Every step of a sign-in, in the order it meets them.
    pub const ALL: [&str; 3] = [START, ADDRESS, DONE];
}

/// Starts timing `name`, a fixed word for the step (`ssh_forward_open`, `link_handshake`). A step
/// begun inside another that still runs names it as its parent.
pub fn begin(name: &'static str) -> Step {
    start(name, None, false)
}

/// [`begin`], for a step of the pool named `alias` (the pool's own name, never its host).
pub fn begin_in(name: &'static str, alias: &str) -> Step {
    start(name, Some(alias.to_owned()), false)
}

/// [`begin`], its line written at `debug`: a step that is a call inside a step, one a start
/// makes many of (a `docker` run). It names no parent.
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
        debug,
        outcome: Outcome::Refused,
        running: OnceLock::new(),
    }
}

impl Step {
    /// Says this step is inside `parent`, where it is so wherever it runs and no [`Context`] can
    /// say so, so its line names it and a sum can leave it out.
    pub fn under(mut self, parent: &'static str) -> Step {
        self.parent = Some(parent);
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
        let mut frames = current().0;
        frames.push(self.frame());
        Context(frames)
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
        let enabled = if self.debug {
            tracing::enabled!(target: "purlis::step", tracing::Level::DEBUG)
        } else {
            tracing::enabled!(target: "purlis::step", tracing::Level::INFO)
        };
        if !enabled {
            return;
        }
        let elapsed_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let name = self.name;
        let outcome = self.outcome.word();
        let mut tail = String::new();
        if let Some(pool) = &self.pool {
            tail.push_str(&format!("pool: {pool}"));
        }
        if let Some(parent) = self.parent {
            if !tail.is_empty() {
                tail.push_str(", ");
            }
            tail.push_str(&format!("parent: {parent}"));
        }
        let tail = if tail.is_empty() {
            String::new()
        } else {
            format!(" ({tail})")
        };
        let pool = self.pool.as_deref().unwrap_or("");
        let parent = self.parent.unwrap_or("");
        let debug = self.debug;
        self.span.in_scope(|| {
            if debug {
                tracing::debug!(
                    target: "purlis::step", step = name, elapsed_ms, outcome, pool, parent,
                    "purlis: step {name} {outcome} after {elapsed_ms} ms{tail}"
                );
            } else {
                tracing::info!(
                    target: "purlis::step", step = name, elapsed_ms, outcome, pool, parent,
                    "purlis: step {name} {outcome} after {elapsed_ms} ms{tail}"
                );
            }
        });
    }
}

#[cfg(test)]
mod tests;
