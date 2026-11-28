// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What every place a chat runs shares: its scrollback, what an ended view is sent, why a start
//! is refused while the kill switch is thrown, and where a chat's hooks report.
//!
//! **A chat runs only in the session host** (#2005, ADR 0068, Ruled 3): the window's own way of
//! running one is a stand-in that only a test build has ([`in_this_process`]).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use purlis_core::hookwire::ChatTokens;

#[cfg(test)]
mod in_this_process;
#[cfg(test)]
pub use in_this_process::Sessions;

/// How many lines of history each session keeps, in the window or in the session host. The pane
/// showing it is told, so its own terminal keeps the same.
pub const SCROLLBACK: u32 = purlis_core::engine::SCROLLBACK;

/// What a pane is sent once its view has closed, so a screen that will never change again
/// does not look like one that might. A pane whose own view closed writes it and is gone.
pub(crate) const ENDED: &str = "\r\n\x1b[2m— the program has ended —\x1b[0m\r\n";

/// Why a start was refused while the kill switch is thrown (OV-1).
pub const STOPPED: &str = "Every chat purlis started is stopped. Re-arm from the title bar to \
                           start chats again; a new shell still opens.";

/// What a session is told, so a hook running inside it can find its way back.
///
/// A hook is a descendant of the session's own program, so it inherits this and has to look
/// nothing up: no plane read, no file, no discovery. That is most of why the call costs 1.8 ms.
///
/// **Read only by the in-process stand-in a test build has** (#2005): a chat in the session host
/// is issued its token by the host, which owns the project's hook channel (#2000).
#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Reporting {
    pub socket: PathBuf,
    /// The chats' tokens the socket checks every line against: each chat is issued its own
    /// here as it starts, and it is forgotten when the chat closes.
    pub tokens: Arc<ChatTokens>,
}

/// The operator's shell, or a plain one where the environment does not name it.
pub fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
}

/// Bytes turned into text, holding back the start of a character split across two reads for
/// the read that finishes it. Bytes that are no character at all become `U+FFFD`. A pane's
/// terminal draws nothing for them instead: xterm's decoder skips them (#4745).
///
/// For what reads a view's output as text (a behaviour's pane, a grade's screen). A pane in the
/// window is never handed text: it is sent the bytes, and its terminal decodes them (#4745).
#[derive(Default)]
pub(crate) struct Utf8Stream {
    waiting: Vec<u8>,
}

impl Utf8Stream {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> String {
        let mut rest = std::mem::take(&mut self.waiting);
        rest.extend_from_slice(bytes);
        let mut text = String::new();
        let mut from = 0;
        loop {
            match std::str::from_utf8(&rest[from..]) {
                Ok(whole) => {
                    text.push_str(whole);
                    return text;
                }
                Err(err) => {
                    let good = from + err.valid_up_to();
                    text.push_str(std::str::from_utf8(&rest[from..good]).unwrap_or_default());
                    match err.error_len() {
                        // No character at all: one replacement, as a terminal shows it.
                        Some(wrong) => {
                            text.push(char::REPLACEMENT_CHARACTER);
                            from = good + wrong;
                        }
                        // The start of one: it waits for the read that finishes it.
                        None => {
                            self.waiting = rest[good..].to_vec();
                            return text;
                        }
                    }
                }
            }
        }
    }
}

/// Runs every job on a thread of its own, all at once, and answers with what each returned once
/// all of them have. A job whose thread could not be started runs here instead, so none is
/// skipped. Named for what the threads are called in a sample or a crash report.
pub(crate) fn all_at_once<T: Send + 'static, J: FnOnce() -> T + Send + 'static>(
    name: &str,
    jobs: impl IntoIterator<Item = J>,
) -> Vec<T> {
    let mut done = Vec::new();
    let mut running = Vec::new();
    for job in jobs {
        let job = std::sync::Arc::new(Mutex::new(Some(job)));
        let theirs = std::sync::Arc::clone(&job);
        let started = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || lock(&theirs).take().map(|job| job()));
        match started {
            Ok(thread) => running.push(thread),
            Err(_) => done.extend(lock(&job).take().map(|job| job())),
        }
    }
    done.extend(
        running
            .into_iter()
            .filter_map(|thread| thread.join().ok().flatten()),
    );
    done
}

/// Whether a process is still there and not a zombie, as the operating system sees it.
#[cfg(test)]
pub(crate) fn alive(pid: u32) -> bool {
    purlis_core::forklock::output(std::process::Command::new("ps").args([
        "-o",
        "stat=",
        "-p",
        &pid.to_string(),
    ]))
    .map(|out| {
        out.status.success()
            && !String::from_utf8_lossy(&out.stdout)
                .trim_start()
                .starts_with('Z')
    })
    .unwrap_or(false)
}

/// A poisoned lock only means another thread panicked while holding it; what is inside is
/// still the sessions that are running.
fn lock<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
