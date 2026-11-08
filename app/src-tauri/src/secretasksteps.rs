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
//! **A host that stops answering holds a bounded number of steps** (#5618). The writer waits on
//! the session host's answer to each write, and the queue to it holds at most [`MOST_QUEUED`]
//! steps: one told past that is not written, and takes with it every step of its ask not written
//! yet, those queued before it and those told after, so nothing is written without its raise.
//! Telling never waits, since the asks tell under their lock. The asks so dropped are remembered
//! for the plane's life, one number each.
//!
//! Best effort, as before: a step still waiting as the app quits is not written, and the steps of
//! an ask raised then and answered after a restart are written without a raise. The chat's own
//! trace keeps the whole of every ask's life whatever the log holds.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::eventlog::Entry;
use purlis_core::eventlog::secretasks::Step;

use crate::hooks::Events;

/// The most steps that wait for the event log to open; one past it is not written.
const MOST_WAITING: usize = 512;

/// The most steps told and not yet taken by the writer, which waits on each write: a session
/// host that stops answering holds no more than these (#5618).
const MOST_QUEUED: usize = 512;

/// Asks one of whose steps was not written: their later steps are not either. Shared by the
/// teller, which drops a step the queue has no room for, and the writer.
type Unraised = Arc<Mutex<HashSet<u32>>>;

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
    Written(std::sync::mpsc::Sender<()>),
}

/// **A plane's Secret asks' steps, written in order off the asks' lock** (module docs). Its
/// thread starts with the first [`Self::teller`], and ends once the plane and its asks are gone.
#[derive(Default)]
pub struct Steps {
    sending: Mutex<Option<SyncSender<Told>>>,
    unraised: Unraised,
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
        let unraised = Arc::clone(&self.unraised);
        Arc::new(move |chat, step| {
            let Some(send) = &send else {
                return;
            };
            let told = send.try_send(Told::Step {
                plane: plane.clone(),
                chat,
                step,
            });
            // Its writer gone, the plane is closing: nothing is left to write it.
            if let Err(TrySendError::Full(_)) = told {
                if let Some(ask) = ask_of(step) {
                    unraised
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .insert(ask);
                }
                tracing::info!(
                    "purlis: a secret ask's {} was not recorded (the session host is not \
                     answering, and {MOST_QUEUED} steps wait on it)",
                    step.kind()
                );
            }
        })
    }

    /// The event log has opened: the steps waiting on it are written. Not told where the queue
    /// is full: the writer writes what waits after each step it takes anyway.
    pub fn opened(&self) {
        if let Some(send) = self
            .sending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let _ = send.try_send(Told::Opened);
        }
    }

    /// Waits until every step told so far is written, or waits on a log not open.
    #[cfg(test)]
    pub fn written(&self) {
        let (done, wait) = std::sync::mpsc::channel();
        let send = self
            .sending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if send.is_some_and(|send| send.send(Told::Written(done)).is_ok()) {
            let _ = wait.recv();
        }
    }

    /// The writer's sender, started the first time. None where its thread could not start, and
    /// the steps go unwritten, which is how a log that is not open treated them before.
    fn sender(&self, events: &Arc<Mutex<Option<Events>>>) -> Option<SyncSender<Told>> {
        let mut sending = self.sending.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(send) = sending.as_ref() {
            return Some(send.clone());
        }
        let (send, told) = sync_channel(MOST_QUEUED);
        let events = Arc::clone(events);
        let unraised = Arc::clone(&self.unraised);
        let started = std::thread::Builder::new()
            .name("purlis-secret-ask-steps".to_owned())
            .spawn(move || write(&told, &events, &unraised));
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
fn write(told: &Receiver<Told>, events: &Mutex<Option<Events>>, unraised: &Unraised) {
    let mut waiting: VecDeque<(PathBuf, Option<u32>, Step)> = VecDeque::new();
    for one in told {
        match one {
            Told::Step { plane, chat, step } => {
                if waiting.len() >= MOST_WAITING {
                    if let Some(ask) = ask_of(step) {
                        unraised
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .insert(ask);
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
                write_waiting(&mut waiting, unraised, events);
                let _ = done.send(());
                continue;
            }
        }
        write_waiting(&mut waiting, unraised, events);
    }
}

/// Writes what waits, in order, where the log is open.
fn write_waiting(
    waiting: &mut VecDeque<(PathBuf, Option<u32>, Step)>,
    unraised: &Unraised,
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
        let dropped = |ask: u32| {
            unraised
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(&ask)
        };
        if ask.is_some_and(dropped) {
            continue;
        }
        let written = open.record(Entry::SecretAsk {
            plane,
            number,
            step,
        });
        if let Err(why) = written {
            if let (Some(ask), Step::Raised { .. }) = (ask, step) {
                unraised
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(ask);
            }
            tracing::info!(
                "purlis: a secret ask's {} was not recorded ({why})",
                step.kind()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::Duration;

    /// A session host that takes the first write and then stops answering until `release` is
    /// sent, as one stalled in the middle of a write does; every write it takes is listed.
    /// The host, word that it stalled, the release, and every step it took.
    type Stalled = (Events, Receiver<()>, Sender<()>, Arc<Mutex<Vec<Step>>>);

    fn a_stalled_host() -> Stalled {
        let written: Arc<Mutex<Vec<Step>>> = Arc::default();
        let (entered, stalled) = sync_channel(1);
        let (release, released) = channel::<()>();
        let released = Mutex::new(released);
        let listed = Arc::clone(&written);
        let events = Events::in_the_host(move |entry| {
            if let Entry::SecretAsk { step, .. } = entry {
                let first = {
                    let mut listed = listed.lock().unwrap_or_else(PoisonError::into_inner);
                    listed.push(step);
                    listed.len() == 1
                };
                if first {
                    let _ = entered.try_send(());
                    let _ = released
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .recv();
                }
            }
            Ok(())
        });
        (events, stalled, release, written)
    }

    fn raised(ask: u32) -> Step {
        Step::Raised {
            ask,
            declared: false,
        }
    }

    #[test]
    fn a_session_host_stalled_mid_write_holds_a_bounded_number_of_steps_and_a_dropped_raise_takes_its_ask_s_later_steps()
     {
        let (events, stalled, release, written) = a_stalled_host();
        let events = Arc::new(Mutex::new(Some(events)));
        let steps = Steps::default();
        let tell = steps.teller(Path::new("/plane"), &events);
        tell(Some(1), raised(0));
        stalled
            .recv_timeout(Duration::from_secs(10))
            .expect("the host takes the first write and stalls");

        // Told under the asks' lock while the host stalls: each returns at once.
        let told = 20 * MOST_WAITING as u32;
        for ask in 1..=told {
            tell(Some(1), raised(ask));
        }
        // The last ask's raise was dropped: its fill is not written without it.
        tell(
            Some(1),
            Step::Declined {
                ask: told,
                waiting: 0,
            },
        );
        release.send(()).expect("the host is there");
        steps.written();

        let written = written.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(
            written.len() <= MOST_QUEUED + 2,
            "{} steps were held for a stalled host",
            written.len()
        );
        assert!(
            written.len() > 1,
            "the steps queued before it filled are written"
        );
        assert!(
            !written.iter().any(|step| ask_of(*step) == Some(told)),
            "a raise not written takes its ask's later steps with it: {:?}",
            written.last()
        );
    }
}
