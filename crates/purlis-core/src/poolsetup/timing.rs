// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How long each of Pool setup's checks took, and what a slow one waits on** (#7191, ruled
//! 2026-11-18): every doctor and Connect check line carries its measured duration, and one that
//! has run past [`SLOW`] says what it is waiting on and why.
//!
//! A check is measured with a [`Stopwatch`], which also lists it as *running* for as long as it
//! runs, so a read of the tab made while it is still going ([`slow_now`]) can say what it waits
//! on. Nothing here is kept on disk, and nothing of the machine's answer is kept: only a cause, a
//! duration and words.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// **How long a check may run before its line says what it waits on** (ruled on #7191).
pub const SLOW: Duration = Duration::from_secs(30);

/// One check's duration, by the cause it would fail with (doctor's own key for it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timed {
    pub cause: &'static str,
    pub took: Duration,
}

impl Timed {
    /// Whole milliseconds, never less than one, as the window's lines count them.
    #[must_use]
    pub fn ms(&self) -> u32 {
        ms(self.took)
    }

    /// What the check waited on, where it took past [`SLOW`].
    #[must_use]
    pub fn waited_on(&self, host: &str) -> Option<String> {
        (self.took > SLOW).then(|| waiting_on(self.cause, host))
    }
}

/// A duration in words, as the window says it: `820 ms`, `4.2 s`, `1 min 4 s`.
#[must_use]
pub fn words(took: Duration) -> String {
    let ms = took.as_millis();
    if ms < 1000 {
        return format!("{} ms", ms.max(1));
    }
    let secs = took.as_secs();
    if secs < 60 {
        return format!("{}.{} s", secs, (ms % 1000) / 100);
    }
    format!("{} min {} s", secs / 60, secs % 60)
}

/// What the check with `cause` is, in a line of `purlis pool doctor`.
#[must_use]
pub fn about(cause: &str) -> &'static str {
    match cause {
        "unconfined" => "Docker's confinement",
        "unsupported-architecture" => "its platform",
        "image-unreachable" => "the runner image",
        "kernel-short" => "its kernel",
        "disk-full" => "its disk",
        "link" => "the link",
        "cluster" => "the cluster",
        _ => "the machine",
    }
}

/// **One check's time as the window is told it** (#7191): by cause, how long it took where it is
/// done, and what it waits on, or waited on, where that was past [`SLOW`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Took {
    pub cause: &'static str,
    pub took_ms: Option<u32>,
    pub waiting_on: Option<String>,
}

/// **The times of the checks that ran** `timed`, then those still running past [`SLOW`] on `host`
/// (each of those with no time yet, only what it waits on).
#[must_use]
pub fn lines(timed: &[Timed], host: &str) -> Vec<Took> {
    let mut lines: Vec<Took> = timed
        .iter()
        .map(|one| Took {
            cause: one.cause,
            took_ms: Some(one.ms()),
            waiting_on: one.waited_on(host),
        })
        .collect();
    for (cause, on, said) in slow_now() {
        if on == host && !lines.iter().any(|one| one.cause == cause) {
            lines.push(Took {
                cause,
                took_ms: None,
                waiting_on: Some(said),
            });
        }
    }
    lines
}

/// `took` in whole milliseconds, never less than one, and never past what a `u32` holds.
#[must_use]
pub fn ms(took: Duration) -> u32 {
    u32::try_from(took.as_millis().max(1)).unwrap_or(u32::MAX)
}

/// **What the check with `cause` waits on, and why**, for `host`: said where it took past
/// [`SLOW`], so the person knows whether to wait. A cause this doesn't know says it is waiting on
/// the machine's answer.
#[must_use]
pub fn waiting_on(cause: &str, host: &str) -> String {
    match cause {
        "unconfined" => format!(
            "Waiting on Docker on {host} to say how it confines containers. A busy or \
             overloaded machine answers slowly."
        ),
        "unsupported-architecture" => format!(
            "Waiting on Docker on {host} to say which platform it runs. A busy or overloaded \
             machine answers slowly."
        ),
        "image-unreachable" => format!(
            "Waiting on {host} to download the runner image from its registry. A first \
             download is large, and the registry or the network between may be slow."
        ),
        "kernel-short" => format!(
            "Waiting on {host} to start a small container and read its kernel. The first \
             container there may be waiting on the image."
        ),
        "disk-full" => format!(
            "Waiting on Docker on {host} to say how much disk it has free. A disk that is \
             nearly full answers slowly."
        ),
        "cluster" => format!(
            "Waiting on the API server of {host} to answer doctor's reads. A cluster far away, \
             or asleep, answers slowly."
        ),
        "link" => format!(
            "Waiting on {host} to answer a docker command over the connection the link rides. \
             The machine or the network between may be slow."
        ),
        _ => {
            format!("Waiting on {host} to answer. The machine or the network between may be slow.")
        }
    }
}

struct Running {
    id: u64,
    cause: &'static str,
    host: String,
    since: Instant,
}

fn running() -> &'static Mutex<Vec<Running>> {
    static RUNNING: OnceLock<Mutex<Vec<Running>>> = OnceLock::new();
    RUNNING.get_or_init(Mutex::default)
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// **A check being timed**: listed as running until it is [`done`](Stopwatch::done) or dropped.
pub struct Stopwatch {
    id: u64,
    cause: &'static str,
    started: Instant,
}

impl Stopwatch {
    /// Starts timing the check with `cause` on `host`.
    #[must_use]
    pub fn start(cause: &'static str, host: &str) -> Stopwatch {
        let id = next_id();
        let started = Instant::now();
        running()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Running {
                id,
                cause,
                host: host.to_owned(),
                since: started,
            });
        Stopwatch { id, cause, started }
    }

    /// Stops it: how long it took.
    #[must_use]
    pub fn done(self) -> Timed {
        Timed {
            cause: self.cause,
            took: self.started.elapsed(),
        }
    }
}

impl Drop for Stopwatch {
    fn drop(&mut self) {
        running()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|one| one.id != self.id);
    }
}

/// **Times `check`** as the check with `cause` on `host`, putting how long it took in `into`
/// whatever it answered.
pub fn time<T>(
    into: &mut Vec<Timed>,
    cause: &'static str,
    host: &str,
    check: impl FnOnce() -> T,
) -> T {
    let watch = Stopwatch::start(cause, host);
    let answered = check();
    into.push(watch.done());
    answered
}

/// **Which checks are running past [`SLOW`] at `now`**, each as its cause and what it waits on.
#[must_use]
pub fn slow_at(now: Instant) -> Vec<(&'static str, String, String)> {
    running()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .filter(|one| now.saturating_duration_since(one.since) > SLOW)
        .map(|one| {
            (
                one.cause,
                one.host.clone(),
                waiting_on(one.cause, &one.host),
            )
        })
        .collect()
}

/// [`slow_at`], now.
#[must_use]
pub fn slow_now() -> Vec<(&'static str, String, String)> {
    slow_at(Instant::now())
}

#[cfg(test)]
mod tests;
