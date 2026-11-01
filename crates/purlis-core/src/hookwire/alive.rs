// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An asker told its ask is still being worked on** (#4080): a chat's brokered clone has no
//! total deadline, so the command waiting on it cannot hold one either. It waits as long as the
//! app keeps saying [`super::Answer::Alive`], and gives up only on a silence longer than the
//! clone's own stall limit.
//!
//! - **Asked for, never assumed.** Only an ask that says its asker reads these lines
//!   ([`super::GitAsk::alive`]) is sent them: an older command reads its first line as its
//!   answer.
//! - **Said when the work shows life, not on a clock.** Whatever runs the ask beats the pulse
//!   its thread holds ([`beat`]) when git says something, so a hung clone sends nothing and the
//!   asker's wait ends after it. At most one line every [`EVERY`].
//! - **Held per thread**, as git's isolation is ([`crate::worktree::git::within`]): a command
//!   that runs the work on threads of its own carries the pulse to them ([`held`], [`within`]).
//! - **Ended before the answer** ([`Pulse::end`]), so no line follows it on the connection.
//! - **Carried over the relay** (#4161): where the session host hears the line and the window
//!   runs the clone, the window holds a pulse that replies [`super::relay::Replied::Alive`]
//!   ([`Pulse::saying`]), and the host beats the pulse on the asker's connection for each.

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// The least time between two lines to one asker.
#[cfg(not(test))]
pub const EVERY: Duration = Duration::from_secs(10);
#[cfg(test)]
pub const EVERY: Duration = Duration::from_millis(50);

/// How long one line may take to be written: an asker that reads nothing for this long has gone.
const A_LINE_TAKES_AT_MOST: Duration = Duration::from_secs(2);

/// One asker's connection, told its ask is still being worked on.
pub struct Pulse {
    to: Mutex<Beating>,
}

struct Beating {
    /// What says it, which answers whether it could. `None` once ended, or once it could not.
    say: Option<Box<dyn FnMut() -> bool + Send>>,
    last: Option<Instant>,
}

impl Pulse {
    /// A pulse that writes its lines to `stream`.
    pub fn to(mut stream: impl Write + Send + 'static) -> Arc<Self> {
        Self::saying(move || {
            let Ok(mut line) = serde_json::to_vec(&super::Answer::Alive {}) else {
                return false;
            };
            line.push(b'\n');
            stream
                .write_all(&line)
                .and_then(|()| stream.flush())
                .is_ok()
        })
    }

    /// A pulse that says it by `say`, which answers whether it could.
    pub fn saying(say: impl FnMut() -> bool + Send + 'static) -> Arc<Self> {
        Arc::new(Self {
            to: Mutex::new(Beating {
                say: Some(Box::new(say)),
                last: None,
            }),
        })
    }

    /// Says the ask is alive, unless it was said less than [`EVERY`] ago.
    pub fn beat(&self) {
        let mut beating = self.to.lock().unwrap_or_else(PoisonError::into_inner);
        if beating.last.is_some_and(|last| last.elapsed() < EVERY) {
            return;
        }
        let Some(say) = beating.say.as_mut() else {
            return;
        };
        if say() {
            beating.last = Some(Instant::now());
        } else {
            beating.say = None;
        }
    }

    /// No more lines: the answer comes next.
    pub fn end(&self) {
        self.to.lock().unwrap_or_else(PoisonError::into_inner).say = None;
    }
}

/// A pulse writing to a clone of the asker's socket, its writes held to a short deadline.
#[cfg(unix)]
pub fn on(socket: &std::os::unix::net::UnixStream) -> Option<Arc<Pulse>> {
    let stream = socket.try_clone().ok()?;
    stream.set_write_timeout(Some(A_LINE_TAKES_AT_MOST)).ok()?;
    Some(Pulse::to(stream))
}

thread_local! {
    static HELD: std::cell::RefCell<Option<Arc<Pulse>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `then` with `pulse` held by this thread, or as it is with none.
pub fn within<T>(pulse: Option<&Arc<Pulse>>, then: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<Pulse>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            HELD.with(|held| *held.borrow_mut() = before);
        }
    }
    let Some(pulse) = pulse else {
        return then();
    };
    let before = HELD.with(|held| held.borrow_mut().replace(Arc::clone(pulse)));
    let _restore = Restore(before);
    then()
}

/// The pulse this thread holds, for work that carries it to threads of its own.
pub fn held() -> Option<Arc<Pulse>> {
    HELD.with(|held| held.borrow().clone())
}

/// Beats the pulse this thread holds, if it holds one.
pub fn beat() {
    if let Some(pulse) = held() {
        pulse.beat();
    }
}
