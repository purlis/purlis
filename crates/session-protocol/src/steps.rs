// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each step of a remote start takes** (#7182): a [`tracing`] span per step, named
//! and closed with its elapsed milliseconds as one `info` line in the local diagnostic log.
//!
//! Local only (ADR 0075: no telemetry): nothing here leaves the machine, and a step's line
//! carries its name and, where there is one, the pool's alias. Never a host name, a user, a
//! path or a command's arguments.
//!
//! ```text
//! purlis: step ssh_forward_open done after 412 ms (pool: lab)
//! ```
//!
//! Hold the guard from [`begin`] for the step's length. It works across `.await`s, as it times
//! wall-clock from creation to drop.

use std::time::Instant;

/// The target of every step's line, so a filter or a test finds them alone.
pub const TARGET: &str = "purlis::step";

/// A step that is running: its line is written when this is dropped.
#[must_use = "a step is timed until its guard is dropped"]
pub struct Step {
    name: &'static str,
    started: Instant,
    span: tracing::Span,
    pool: Option<String>,
    failed: bool,
}

/// Starts timing `name`, a fixed word for the step (`ssh_forward_open`, `link_handshake`).
pub fn begin(name: &'static str) -> Step {
    start(name, None)
}

/// [`begin`], for a step of the pool named `alias` (the pool's own name, never its host).
pub fn begin_in(name: &'static str, alias: &str) -> Step {
    start(name, Some(alias.to_owned()))
}

fn start(name: &'static str, pool: Option<String>) -> Step {
    let span = tracing::info_span!(target: "purlis::step", "step", step = name);
    Step {
        name,
        started: Instant::now(),
        span,
        pool,
        failed: false,
    }
}

impl Step {
    /// Says the step ended in a refusal, not in what it was for: its line reads `refused`.
    pub fn refused(&mut self) {
        self.failed = true;
    }

    /// Ends the step by `answer`: refused if it is an `Err`, and handed back as it is.
    pub fn ended<T, E>(mut self, answer: Result<T, E>) -> Result<T, E> {
        if answer.is_err() {
            self.refused();
        }
        answer
    }
}

impl Drop for Step {
    fn drop(&mut self) {
        let elapsed_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let name = self.name;
        let outcome = if self.failed { "refused" } else { "done" };
        let pool = self.pool.as_deref().unwrap_or("");
        self.span.in_scope(|| {
            if pool.is_empty() {
                tracing::info!(
                    target: "purlis::step", step = name, elapsed_ms, outcome,
                    "purlis: step {name} {outcome} after {elapsed_ms} ms"
                );
            } else {
                tracing::info!(
                    target: "purlis::step", step = name, elapsed_ms, outcome, pool,
                    "purlis: step {name} {outcome} after {elapsed_ms} ms (pool: {pool})"
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
            let _step = begin("link_handshake");
            std::thread::sleep(std::time::Duration::from_millis(30));
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
        let out = said_while(|| drop(begin_in("ssh_forward_open", "lab")));
        assert!(out.contains("(pool: lab)"), "{out}");
    }

    #[test]
    fn a_step_that_ended_in_a_refusal_says_so() {
        let out = said_while(|| {
            let answer: Result<(), &str> = begin("link_check").ended(Err("no"));
            assert!(answer.is_err());
        });
        assert!(out.contains("link_check refused after"), "{out}");
        let out = said_while(|| drop(begin("link_check")));
        assert!(out.contains("link_check done after"), "{out}");
    }
}
