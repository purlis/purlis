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
//! purlis: step docker_exec done after 38 ms (parent: runner_start)
//! ```
//!
//! **A step that is not closed with [`Step::done`] reads `refused`.** The guard is dropped on
//! every `?` and every early return, so its default is the failing way out; a success says so.
//! A miss that is the answer (an inspect that finds no container on a normal start) is a
//! success of the step: the step is done and its caller reads the answer.
//!
//! **A step inside another names it** ([`Step::under`]): a `parent` field on its line. The
//! parent's time contains its children's, so a table that sums every line counts them twice;
//! sum the lines with no parent, or one parent's children.
//!
//! The top-level steps log at `info`; each `docker` run ([`begin_debug`]) at `debug`, so the
//! default log carries the shape of a start and `RUST_LOG=purlis::step=debug` its every call.
//!
//! Hold the guard from [`begin`] for the step's length. It works across `.await`s, as it times
//! wall-clock from creation to drop.

use std::time::Instant;

/// The target of every step's line, so a filter or a test finds them alone.
pub const TARGET: &str = "purlis::step";

/// A step that is running: its line is written when this is dropped, `refused` unless
/// [`Step::done`] ended it.
#[must_use = "a step is timed until its guard is dropped"]
pub struct Step {
    name: &'static str,
    started: Instant,
    span: tracing::Span,
    pool: Option<String>,
    parent: Option<&'static str>,
    debug: bool,
    succeeded: bool,
}

/// Starts timing `name`, a fixed word for the step (`ssh_forward_open`, `link_handshake`).
pub fn begin(name: &'static str) -> Step {
    start(name, None, false)
}

/// [`begin`], for a step of the pool named `alias` (the pool's own name, never its host).
pub fn begin_in(name: &'static str, alias: &str) -> Step {
    start(name, Some(alias.to_owned()), false)
}

/// [`begin`], its line written at `debug`: a step that is a call inside a step, one a start
/// makes many of (a `docker` run).
pub fn begin_debug(name: &'static str) -> Step {
    start(name, None, true)
}

fn start(name: &'static str, pool: Option<String>, debug: bool) -> Step {
    let span = tracing::info_span!(target: "purlis::step", "step", step = name);
    Step {
        name,
        started: Instant::now(),
        span,
        pool,
        parent: None,
        debug,
        succeeded: false,
    }
}

impl Step {
    /// Says this step is inside `parent`, so its line names it and a sum can leave it out.
    pub fn under(mut self, parent: &'static str) -> Step {
        self.parent = Some(parent);
        self
    }

    /// Ends the step as it was for: its line reads `done`.
    pub fn done(mut self) {
        self.succeeded = true;
    }

    /// Ends the step by `answer`: done if it is an `Ok`, refused if it is an `Err`, and handed
    /// back as it is.
    pub fn ended<T, E>(mut self, answer: Result<T, E>) -> Result<T, E> {
        self.succeeded = answer.is_ok();
        answer
    }
}

impl Drop for Step {
    fn drop(&mut self) {
        let elapsed_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let name = self.name;
        let outcome = if self.succeeded { "done" } else { "refused" };
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
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Said(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Said {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Said {
        type Writer = Said;
        fn make_writer(&'a self) -> Said {
            self.clone()
        }
    }

    fn said_while(then: impl FnOnce()) -> String {
        let said = Said::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(said.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .finish();
        tracing::subscriber::with_default(subscriber, then);
        String::from_utf8(said.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn a_step_says_its_name_and_how_many_milliseconds_it_took_at_info() {
        let out = said_while(|| {
            let step = begin("link_handshake");
            std::thread::sleep(std::time::Duration::from_millis(30));
            step.done();
        });
        assert!(out.contains("INFO"), "{out}");
        assert!(out.contains("step=\"link_handshake\"") || out.contains("step=link_handshake"));
        let ms: u64 = out
            .split("elapsed_ms=")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("no elapsed_ms in {out}"));
        assert!((30..2000).contains(&ms), "{ms} ms in {out}");
    }

    #[test]
    fn a_step_in_a_pool_names_the_pool_by_its_alias() {
        let out = said_while(|| begin_in("ssh_forward_open", "lab").done());
        assert!(out.contains("(pool: lab)"), "{out}");
    }

    #[test]
    fn a_step_dropped_without_being_done_says_refused() {
        // An early return or a `?` drops the guard: that is the failing way out.
        fn leaves_early() -> Result<(), &'static str> {
            let _step = begin("harness_spawn");
            Err("no")?;
            Ok(())
        }
        let out = said_while(|| {
            let _ = leaves_early();
        });
        assert!(out.contains("harness_spawn refused after"), "{out}");
    }

    #[test]
    fn a_step_closed_with_done_says_done() {
        let out = said_while(|| begin("link_check").done());
        assert!(out.contains("link_check done after"), "{out}");
    }

    #[test]
    fn a_step_ended_by_an_answer_is_done_on_ok_and_refused_on_err() {
        let out = said_while(|| {
            let answer: Result<(), &str> = begin("link_check").ended(Err("no"));
            assert!(answer.is_err());
        });
        assert!(out.contains("link_check refused after"), "{out}");
        let out = said_while(|| {
            let _ = begin("link_check").ended(Ok::<_, &str>(()));
        });
        assert!(out.contains("link_check done after"), "{out}");
    }

    #[test]
    fn a_miss_that_is_the_answer_is_a_step_done_not_refused() {
        // `docker container inspect` finding nothing on a normal start is `Ok(ran)` with a
        // non-zero exit: the run was made, and its caller reads the answer.
        let out = said_while(|| {
            let ran: Result<i32, &str> = Ok(1);
            let _ = begin_debug("docker_container").ended(ran);
        });
        assert!(!out.contains("refused"), "{out}");
    }

    #[test]
    fn a_step_inside_another_names_its_parent() {
        let out = said_while(|| begin("runner_start").under("container_check").done());
        assert!(out.contains("(parent: container_check)"), "{out}");
        let out = said_while(|| begin_in("runner_start", "lab").under("x").done());
        assert!(out.contains("(pool: lab, parent: x)"), "{out}");
    }

    #[test]
    fn a_docker_call_is_written_at_debug_and_a_top_level_step_at_info() {
        let at_info = said_while(|| begin_debug("docker_exec").done());
        assert!(at_info.is_empty(), "{at_info}");
        let said = Said::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(said.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        tracing::subscriber::with_default(subscriber, || begin_debug("docker_exec").done());
        let out = String::from_utf8(said.0.lock().unwrap().clone()).unwrap();
        assert!(out.contains("DEBUG") && out.contains("docker_exec done"), "{out}");
    }
}
