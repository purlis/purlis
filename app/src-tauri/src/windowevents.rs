// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A window's moves, resizes and focus are heard on the main thread and handled off it**
//! (felt speed round 4, H14). The main thread draws every window, so it must never wait on a
//! lock another thread holds. These three events (`Focused`, `Resized`, `Moved`) used to reach
//! the chats' and the planes' locks from it, on every frame of a drag, and a core stall of up
//! to 30 s (a lock held across a session-host round trip) froze every window with nothing
//! named. They now join a lane of their own, and a thread of its own handles each in turn.
//!
//! **Pending events of one kind for one window are one**: what a handler reads is the window as
//! it is when it runs, so a drag's fifty frames behind a slow handler are one look, and the
//! lane never grows. Order between different events is the order they first arrived in.

use std::collections::VecDeque;
use std::hash::Hash;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// The events waiting, oldest first, and what wakes the thread that handles them.
type Waiting<K, P> = (Mutex<VecDeque<(K, P)>>, Condvar);

/// A lane of events, each handled by one thread, one at a time, in order; an event whose key is
/// already waiting replaces the waiting one in its place.
pub(crate) struct Lane<K, P> {
    waiting: Arc<Waiting<K, P>>,
}

impl<K: Eq + Hash + Clone + Send + 'static, P: Send + 'static> Lane<K, P> {
    /// A lane whose events are handled with `handle` on a thread named `name`.
    pub(crate) fn new(name: &str, handle: impl Fn(P) + Send + 'static) -> Self {
        let waiting: Arc<Waiting<K, P>> = Arc::default();
        let worker = Arc::clone(&waiting);
        let spawned = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let (queue, wake) = &*worker;
                loop {
                    let next = {
                        let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
                        loop {
                            if let Some(next) = queue.pop_front() {
                                break next;
                            }
                            queue = wake.wait(queue).unwrap_or_else(PoisonError::into_inner);
                        }
                    };
                    handle(next.1);
                }
            });
        if let Err(err) = spawned {
            tracing::warn!("the window events' thread did not start: {err}");
        }
        Self { waiting }
    }

    /// Hands `event` over and returns at once, whatever the handler is doing.
    pub(crate) fn post(&self, key: K, event: P) {
        let (queue, wake) = &*self.waiting;
        let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
        match queue.iter_mut().find(|(waiting, _)| *waiting == key) {
            Some(waiting) => waiting.1 = event,
            None => queue.push_back((key, event)),
        }
        drop(queue);
        wake.notify_one();
    }
}

#[cfg(test)]
mod tests;
