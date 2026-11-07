// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A plane's Secret asks' steps, on their way to the event log** (#5078, #5509, ADR 0109).
//!
//! The asks tell each step while they hold their lock, so the steps of one ask are told in the
//! order of its life (#5220). Writing a step can be a round trip to the session host, which holds
//! the log while chats run there, so it is never made under that lock: a step told is queued,
//! in that order, and one thread of the plane's writes the queue out.
//!
//! **No answer is written without its raise.** A step told before the event log is open waits
//! for it, in order, and is written as it opens: the first seconds of the app no longer lose a
//! raise and keep its answer. A raise that is not written after all — the queue full, or the log
//! refused it — takes the ask's later steps with it.
//!
//! Best effort, as before: a step still waiting as the app quits is not written, and the steps of
//! an ask raised then and answered after a restart are written without a raise. The chat's own
//! trace keeps the whole of every ask's life whatever the log holds.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::eventlog::Entry;
use purlis_core::eventlog::secretasks::Step;

use crate::hooks::Events;

/// The most steps that wait for the event log to open; one past it is not written.
const MOST_WAITING: usize = 512;

/// What the plane's writer is told.
enum Told {
    /// A step of a Secret ask of chat `chat`'s, in the project at `plane`.
    Step {
        plane: PathBuf,
        chat: Option<u32>,
        step: Step,
    },
    /// The event log has opened: what waits on it is written.
    Opened,
    /// Answered once everything told before it is written, or waits on a log not open.
    #[cfg(test)]
    Written(Sender<()>),
}

/// **A plane's Secret asks' steps, written in order off the asks' lock** (module docs). Its
/// thread starts with the first [`Self::teller`], and ends once the plane and its asks are gone.
#[derive(Default)]
pub struct Steps {
    sending: Mutex<Option<Sender<Told>>>,
}

impl Steps {
    /// **What the asks of the project at `plane` tell their steps to** (`Asks::telling`): each
    /// queued, to be written into `events` once it is open.
    pub fn teller(
        &self,
        plane: &Path,
        events: &Arc<Mutex<Option<Events>>>,
    ) -> purlis_core::secrets::ask::Telling {
        let send = self.sender(events);
        let plane = plane.to_path_buf();
        Arc::new(move |chat, step| {
            if let Some(send) = &send {
                // Its writer gone, the plane is closing: nothing is left to write it.
                let _ = send.send(Told::Step {
                    plane: plane.clone(),
                    chat,
                    step,
                });
            }
        })
    }

    /// The event log has opened: the steps waiting on it are written.
    pub fn opened(&self) {
        self.tell(Told::Opened);
    }

    /// Waits until every step told so far is written, or waits on a log not open.
    #[cfg(test)]
    pub fn written(&self) {
        let (done, wait) = channel();
        if self.tell(Told::Written(done)) {
            let _ = wait.recv();
        }
    }

    /// Tells the writer, where it runs; whether it does.
    fn tell(&self, told: Told) -> bool {
        self.sending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|send| send.send(told).is_ok())
    }

    /// The writer's sender, started the first time. None where its thread could not start, and
    /// the steps go unwritten, which is how a log that is not open treated them before.
    fn sender(&self, events: &Arc<Mutex<Option<Events>>>) -> Option<Sender<Told>> {
        let mut sending = self.sending.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(send) = sending.as_ref() {
            return Some(send.clone());
        }
        let (send, told) = channel();
        let events = Arc::clone(events);
        let started = std::thread::Builder::new()
            .name("purlis-secret-ask-steps".to_owned())
            .spawn(move || write(&told, &events));
        if let Err(why) = started {
            tracing::warn!("purlis: secret asks' steps will not be recorded ({why})");
            return None;
        }
        *sending = Some(send.clone());
        Some(send)
    }
}

/// The ask a step is of, where it is one ask's.
fn ask_of(step: Step) -> Option<u32> {
    match step {
        Step::Raised { ask, .. }
        | Step::Joined { ask, .. }
        | Step::Filled { ask, .. }
        | Step::Declined { ask, .. } => Some(ask),
        Step::Refused { .. } => None,
    }
}

/// The writer's thread: each step in the order it was told, held while the log is not open.
fn write(told: &Receiver<Told>, events: &Mutex<Option<Events>>) {
    let mut waiting: VecDeque<(PathBuf, Option<u32>, Step)> = VecDeque::new();
    // Asks whose raise was not written: their later steps are not either.
    let mut unraised: HashSet<u32> = HashSet::new();
    for one in told {
        match one {
            Told::Step { plane, chat, step } => {
                if waiting.len() >= MOST_WAITING {
                    if let Some(ask) = ask_of(step) {
                        unraised.insert(ask);
                    }
                    tracing::info!(
                        "purlis: a secret ask's {} was not recorded (too many wait for the \
                         event log to open)",
                        step.kind()
                    );
                } else {
                    waiting.push_back((plane, chat, step));
                }
            }
            Told::Opened => {}
            #[cfg(test)]
            Told::Written(done) => {
                write_waiting(&mut waiting, &mut unraised, events);
                let _ = done.send(());
                continue;
            }
        }
        write_waiting(&mut waiting, &mut unraised, events);
    }
}

/// Writes what waits, in order, where the log is open.
fn write_waiting(
    waiting: &mut VecDeque<(PathBuf, Option<u32>, Step)>,
    unraised: &mut HashSet<u32>,
    events: &Mutex<Option<Events>>,
) {
    let open = events
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let Some(open) = open else {
        return;
    };
    while let Some((plane, number, step)) = waiting.pop_front() {
        let ask = ask_of(step);
        if ask.is_some_and(|ask| unraised.contains(&ask)) {
            continue;
        }
        let written = open.record(Entry::SecretAsk {
            plane,
            number,
            step,
        });
        if let Err(why) = written {
            if let (Some(ask), Step::Raised { .. }) = (ask, step) {
                unraised.insert(ask);
            }
            tracing::info!(
                "purlis: a secret ask's {} was not recorded ({why})",
                step.kind()
            );
        }
    }
}
