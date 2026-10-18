// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the supervisor refused, told to whoever listens** (#2344).
//!
//! Each call the supervisor refuses by the chat's profile is a [`Refusal`]: the operation in
//! Seatbelt's own word (`file-write-create`, `network-outbound`), the exact target the walk
//! arrived at (a path, a socket's path, an address and port), and the name of the process
//! that asked. It is the same record macOS's kernel writes for a refusal of its sandbox, so the
//! purlis program that started the wrap sorts it into a sandbox block with the one sorter every
//! block takes, and the window raises the same Notice.
//!
//! **Telling never changes the answer.** The chat is refused before anything is told, and the
//! [`Teller`] hands what it tells to a thread of its own through a short queue it never waits
//! on: a listener that is slow or never returns holds up no call, and a refusal it could not
//! take is a refusal all the same. The wrap starts the teller once it holds itself, so its
//! thread is held as the wrap is. A listener that panics, in a build that unwinds, is let go
//! and the next refusal told; a release build aborts on a panic, which ends the wrap and its
//! chat with it.
//!
//! **No flood.** The same operation on the same target is told once a [`WINDOW`], and at most
//! [`PER_WINDOW`] different ones are told in one: a build that fails on a thousand files is
//! one pattern, not a thousand Notices. The app's own throttle holds each chat to the same.
//! An internet address a program dialled past purlis's proxy is told once a window whatever the
//! address, apart from the rest ([`Teller::past_the_proxy`]).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the same refusal is told once.
pub const WINDOW: Duration = Duration::from_secs(60);

/// The most different refusals told in one [`WINDOW`].
pub const PER_WINDOW: usize = 10;

/// The operation a connection is told as, in Seatbelt's word for it.
pub const DIALLED: &str = "network-outbound";

/// The most refusals waiting to be told.
pub const QUEUED: usize = PER_WINDOW;

/// One call the supervisor refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The operation, in Seatbelt's word for it.
    pub operation: &'static str,
    /// What it was refused on, whole: an absolute path, or an address and port.
    pub target: String,
    /// The process that asked, by the name the kernel keeps for it, where it could be read.
    pub process: Option<String>,
}

/// What each refusal the [`Teller`] lets through is told to, on the teller's own thread.
pub type Tell = Box<dyn Fn(&Refusal) + Send + Sync + 'static>;

/// Tells refusals to a [`Tell`] on a thread of its own, once each a [`WINDOW`] and at most
/// [`PER_WINDOW`] a window, without ever waiting on it.
pub struct Teller {
    seen: Mutex<Vec<(&'static str, String, Instant)>>,
    /// When an address dialled past the proxy was last told ([`Teller::past_the_proxy`]).
    dialled: Mutex<Option<Instant>>,
    queue: SyncSender<Refusal>,
    /// Refusals queued and not yet told, the one being told among them.
    pending: Arc<AtomicUsize>,
}

impl Teller {
    /// A teller telling `tell`.
    pub fn new(tell: Tell) -> Teller {
        let (queue, told) = sync_channel(QUEUED);
        let pending = Arc::new(AtomicUsize::new(0));
        let left = Arc::clone(&pending);
        std::thread::spawn(move || tell_each(&told, &tell, &left));
        Teller {
            seen: Mutex::new(Vec::new()),
            dialled: Mutex::new(None),
            queue,
            pending,
        }
    }

    /// Tells `operation` refused on `target` at `now`, by the process `process` names, unless
    /// it was told in its window or the window is full; `process` is asked only for one that is
    /// told. Returns at once whatever the listener does.
    pub fn refused(
        &self,
        operation: &'static str,
        target: String,
        process: impl FnOnce() -> Option<String>,
        now: Instant,
    ) {
        if !self.lets(operation, &target, now) {
            return;
        }
        let refusal = Refusal {
            operation,
            target,
            process: process(),
        };
        self.pending.fetch_add(1, Ordering::SeqCst);
        if let Err(not_taken) = self.queue.try_send(refusal) {
            // A full queue or a listener gone: the refusal stands, untold, and is told when it
            // comes again.
            self.pending.fetch_sub(1, Ordering::SeqCst);
            let refusal = match not_taken {
                std::sync::mpsc::TrySendError::Full(it)
                | std::sync::mpsc::TrySendError::Disconnected(it) => it,
            };
            self.forget(refusal.operation, &refusal.target);
        }
    }

    /// **Tells a connection to `target`, an internet address a program dialled past purlis's
    /// proxy, refused at `now`** (#2423): once a [`WINDOW`], whatever the address, and apart
    /// from every other refusal's window, so addresses take nothing of what a file or a socket
    /// is told. Returns at once whatever the listener does.
    pub fn past_the_proxy(
        &self,
        target: String,
        process: impl FnOnce() -> Option<String>,
        now: Instant,
    ) {
        {
            let mut dialled = self
                .dialled
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if dialled.is_some_and(|at| now.saturating_duration_since(at) < WINDOW) {
                return;
            }
            *dialled = Some(now);
        }
        let refusal = Refusal {
            operation: DIALLED,
            target,
            process: process(),
        };
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.queue.try_send(refusal).is_err() {
            // Untold: the next address is told instead.
            self.pending.fetch_sub(1, Ordering::SeqCst);
            *self
                .dialled
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        }
    }

    /// Waits, at most `within`, for what is still being told: the chat has ended, and the wrap
    /// with it, which would otherwise drop the refusals that ended it untold.
    pub fn flush(&self, within: Duration) {
        let until = Instant::now() + within;
        while self.pending.load(Ordering::SeqCst) > 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn seen(&self) -> std::sync::MutexGuard<'_, Vec<(&'static str, String, Instant)>> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lets(&self, operation: &'static str, target: &str, now: Instant) -> bool {
        let mut seen = self.seen();
        seen.retain(|(_, _, at)| now.saturating_duration_since(*at) < WINDOW);
        if seen
            .iter()
            .any(|(was, on, _)| *was == operation && on == target)
            || seen.len() >= PER_WINDOW
        {
            return false;
        }
        seen.push((operation, target.to_owned(), now));
        true
    }

    fn forget(&self, operation: &'static str, target: &str) {
        self.seen()
            .retain(|(was, on, _)| !(*was == operation && on == target));
    }
}

/// Each refusal `told` brings, to `tell`; where a build unwinds, one that panics is let go and
/// the next is told.
fn tell_each(told: &Receiver<Refusal>, tell: &Tell, pending: &AtomicUsize) {
    while let Ok(refusal) = told.recv() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tell(&refusal)));
        pending.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests;
