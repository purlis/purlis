// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each chat's terminal, in order, and never on the window's thread** (#4734, ADR 0108).
//!
//! What a pane sends a chat (its keys, a paste, a mouse report, a resize, its view's open and
//! close) went to the session host on the window's thread and waited for the host's answer, so
//! that Tauri would keep its order (#891): a host slow to answer, or a link being made, froze
//! the window for up to 30 s. Now each chat has a lane. A send joins the back of its chat's lane
//! and returns at once; a worker of the lane's own sends each to the host in turn, and is gone
//! once the lane is empty. Chats do not wait on each other.
//!
//! **Everything else that types into a chat takes its turn in the same lane**
//! ([`Lanes::in_turn`]): a smart close's prompt, a stop's line, a reference, a curation prompt.
//! It waits for what was sent before it and runs on its own thread, as it did before, so a
//! prompt never lands in the middle of a paste. A turn taken by work already in the chat's lane
//! runs at once, so nothing in a lane waits on itself.
//!
//! **A send the host refuses is said, and what was queued behind it is not sent** (the
//! [`Refused`] told, once for the send that failed): the person typed it after what failed, and
//! half of what they typed is worse than none. The lane tries again with the next send once it
//! has emptied. A turn always runs, as its caller waits for its answer.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::planes::PlaneId;

/// The event the window is sent when what a pane sent its chat was refused.
pub const REFUSED: &str = "chat-input-refused";

/// Why a chat whose runner is reconnecting (#2496) is sent nothing: it fails at once, rather
/// than holding its lane behind a link that is not there.
pub const RECONNECTING: &str = "this chat's runner is reconnecting";

/// What a pane sent its chat that was refused, and why, as the Notice on its pane says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct InputRefused {
    pub plane: PlaneId,
    pub session: u32,
    pub why: String,
}

/// Told each refusal. The event carries its project.
pub type Teller = Arc<dyn Fn(InputRefused) + Send + Sync + 'static>;

/// Told each time a chat's lane refuses what was sent to it: the chat, and why.
pub type Refused = Arc<dyn Fn(u32, String) + Send + Sync + 'static>;

/// One chat's sends, waiting their turn.
enum Job {
    /// A send nobody waits on: what it refuses is told.
    Send(Box<dyn FnOnce() -> Result<(), String> + Send>),
    /// A caller waits for its turn ([`Lanes::in_turn`]): the worker hands it the lane, and
    /// waits until it hands it back.
    Turn(Box<dyn FnOnce() + Send>),
}

/// One chat's lane.
#[derive(Default)]
struct Lane {
    jobs: VecDeque<Job>,
    /// Whether someone has the lane: its worker, or a caller in its turn.
    busy: bool,
    /// Whether a send failed since the lane was last empty: what is behind it is not sent.
    failing: bool,
}

/// Every chat's lane in a project.
#[derive(Clone)]
pub struct Lanes {
    lanes: Arc<Mutex<HashMap<u32, Lane>>>,
    refused: Refused,
}

std::thread_local! {
    /// The lanes this thread has now, by the lanes' address and the chat: a turn taken inside
    /// one runs at once.
    static HOLDING: std::cell::RefCell<Vec<(usize, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Lanes {
    /// Lanes that tell `refused` of each send the host refused.
    pub fn new(refused: Refused) -> Self {
        Lanes {
            lanes: Arc::default(),
            refused,
        }
    }

    /// Sends `work` to chat `session` after everything sent to it before, and returns at once.
    pub fn send(&self, session: u32, work: impl FnOnce() -> Result<(), String> + Send + 'static) {
        if self.holds(session) {
            if let Err(why) = work() {
                (self.refused)(session, why);
            }
            return;
        }
        let mut lanes = self.lock();
        let lane = lanes.entry(session).or_default();
        lane.jobs.push_back(Job::Send(Box::new(work)));
        if lane.busy {
            return;
        }
        lane.busy = true;
        drop(lanes);
        self.drain(session);
    }

    /// Runs `work` on this thread once everything sent to chat `session` before it has been,
    /// and nothing after it runs until it has: what types into a chat and waits for the answer.
    pub fn in_turn<T>(&self, session: u32, work: impl FnOnce() -> T) -> T {
        if self.holds(session) {
            return work();
        }
        let mut lanes = self.lock();
        let lane = lanes.entry(session).or_default();
        if !lane.busy {
            // Nobody has it: this caller does, and hands it on as it goes.
            lane.busy = true;
            drop(lanes);
            let answer = self.holding(session, work);
            self.hand_on(session);
            return answer;
        }
        let (turn, my_turn) = std::sync::mpsc::sync_channel::<()>(1);
        let (done, done_with) = std::sync::mpsc::sync_channel::<()>(1);
        lane.jobs.push_back(Job::Turn(Box::new(move || {
            if turn.send(()).is_ok() {
                let _ = done_with.recv();
            }
        })));
        drop(lanes);
        // The worker ends only by handing the turn over, so this always comes.
        let _ = my_turn.recv();
        let answer = self.holding(session, work);
        let _ = done.send(());
        answer
    }

    /// A worker of chat `session`'s lane, which has it: sends what waits, in turn, and is gone
    /// once the lane is empty.
    fn drain(&self, session: u32) {
        let lanes = self.clone();
        let started = std::thread::Builder::new()
            .name(format!("purlis-input-{session}"))
            .spawn(move || lanes.work_through(session));
        if let Err(why) = started {
            // No thread to send on: what waits is refused, so nothing waits for ever.
            let left: Vec<Job> = {
                let mut all = self.lock();
                let lane = all.remove(&session).unwrap_or_default();
                lane.jobs.into_iter().collect()
            };
            for job in left {
                match job {
                    Job::Send(_) => {}
                    Job::Turn(turn) => turn(),
                }
            }
            (self.refused)(session, format!("purlis could not start sending it: {why}"));
        }
    }

    fn work_through(&self, session: u32) {
        loop {
            let (job, failing) = {
                let mut lanes = self.lock();
                let Some(lane) = lanes.get_mut(&session) else {
                    return;
                };
                match lane.jobs.pop_front() {
                    Some(job) => (job, lane.failing),
                    None => {
                        lanes.remove(&session);
                        return;
                    }
                }
            };
            match job {
                Job::Send(_) if failing => {}
                Job::Send(work) => {
                    let sent = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.holding(session, work)
                    }))
                    .unwrap_or_else(|_| Err("purlis failed as it sent it".to_owned()));
                    if let Err(why) = sent {
                        if let Some(lane) = self.lock().get_mut(&session) {
                            lane.failing = true;
                        }
                        (self.refused)(session, why);
                    }
                }
                Job::Turn(turn) => turn(),
            }
        }
    }

    /// A caller's turn is over: a worker takes what waited meanwhile, or the lane is let go.
    fn hand_on(&self, session: u32) {
        let mut lanes = self.lock();
        let waiting = lanes
            .get(&session)
            .is_some_and(|lane| !lane.jobs.is_empty());
        if !waiting {
            lanes.remove(&session);
            return;
        }
        drop(lanes);
        self.drain(session);
    }

    /// Runs `work` as the holder of chat `session`'s lane.
    fn holding<T>(&self, session: u32, work: impl FnOnce() -> T) -> T {
        struct Held(usize, u32);
        impl Drop for Held {
            fn drop(&mut self) {
                HOLDING.with(|holding| {
                    let mut holding = holding.borrow_mut();
                    if let Some(at) = holding.iter().rposition(|one| *one == (self.0, self.1)) {
                        holding.remove(at);
                    }
                });
            }
        }
        let key = self.key();
        HOLDING.with(|holding| holding.borrow_mut().push((key, session)));
        let _held = Held(key, session);
        work()
    }

    /// Whether this thread has chat `session`'s lane now.
    fn holds(&self, session: u32) -> bool {
        let key = self.key();
        HOLDING.with(|holding| holding.borrow().contains(&(key, session)))
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.lanes) as usize
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<u32, Lane>> {
        self.lanes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
