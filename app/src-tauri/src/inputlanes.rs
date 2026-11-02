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
//! runs at once, so nothing in a lane waits on itself. **What types into a chat waits for the
//! lane no longer than [`A_TURN_WAITS_AT_MOST`]** ([`Lanes::in_turn_within`]): most of it is
//! asked on the window's thread, and a lane stuck behind a slow host would freeze the window
//! for as long. Past it, it is refused and never typed.
//!
//! **A key the host refuses is said, and the keys queued behind it are not sent** (the
//! [`Refused`] told, once for the send that failed): the person typed them after what failed,
//! and half of what they typed is worse than none. A resize and a view's close are never
//! dropped ([`Lanes::send_always`]): a view left open is one the host goes on feeding. The lane
//! tries keys again once it has emptied. A turn always runs, as its caller waits for its answer.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::planes::PlaneId;

/// The event the window is sent when what a pane sent its chat was refused.
pub const REFUSED: &str = "chat-input-refused";

/// Why a chat whose runner is reconnecting (#2496) is sent nothing: it fails at once, rather
/// than holding its lane behind a link that is not there.
pub const RECONNECTING: &str = "this chat's runner is reconnecting";

/// The longest what types into a chat waits for the sends before it in the chat's lane
/// ([`Lanes::in_turn_within`]), on the window's thread for most of it: a lane that has not
/// emptied by then is stuck on a host that is not answering.
pub const A_TURN_WAITS_AT_MOST: Duration = Duration::from_secs(1);

/// Why what was to be typed into a chat was not, when its lane had not emptied in time.
pub const LANE_STILL_BUSY: &str = "this chat's host has not yet taken what was sent to it \
     before, so this was not typed. Try again once the chat answers";

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
    /// A send nobody waits on: what it refuses is told. One that is `droppable` (a key, a
    /// paste) is dropped behind one that failed; a resize or a view's close never is.
    Send {
        work: Box<dyn FnOnce() -> Result<(), String> + Send>,
        droppable: bool,
    },
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
    /// Whether a send failed since the lane was last empty: the keys behind it are not sent.
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

    /// Sends `work` to chat `session` after everything sent to it before, and returns at once:
    /// what the person typed, dropped if a send before it in the lane failed.
    pub fn send(&self, session: u32, work: impl FnOnce() -> Result<(), String> + Send + 'static) {
        self.enqueue(session, Box::new(work), true);
    }

    /// [`Self::send`] for what must reach the host even behind a send that failed: a resize, a
    /// view's close (review of #4821). A view never closed is one the host keeps feeding.
    pub fn send_always(
        &self,
        session: u32,
        work: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        self.enqueue(session, Box::new(work), false);
    }

    fn enqueue(
        &self,
        session: u32,
        work: Box<dyn FnOnce() -> Result<(), String> + Send>,
        droppable: bool,
    ) {
        if self.holds(session) {
            if let Err(why) = work() {
                (self.refused)(session, why);
            }
            return;
        }
        let mut lanes = self.lock();
        let lane = lanes.entry(session).or_default();
        lane.jobs.push_back(Job::Send { work, droppable });
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
        self.take_turn(session, None, work)
            .expect("a turn with no deadline is always given")
    }

    /// [`Self::in_turn`], waiting for the lane no longer than `within`: past it, `work` never
    /// runs and the answer is [`LANE_STILL_BUSY`]. What types into a chat from the window's
    /// thread, which draws nothing while it waits (review of #4821).
    pub fn in_turn_within<T>(
        &self,
        session: u32,
        within: Duration,
        work: impl FnOnce() -> T,
    ) -> Result<T, String> {
        self.take_turn(session, Some(within), work)
            .ok_or_else(|| LANE_STILL_BUSY.to_owned())
    }

    /// Runs `work` in chat `session`'s turn, or gives up once `within` has passed with the
    /// lane still busy, answering none. With no `within`, it never gives up.
    fn take_turn<T>(
        &self,
        session: u32,
        within: Option<Duration>,
        work: impl FnOnce() -> T,
    ) -> Option<T> {
        if self.holds(session) {
            return Some(work());
        }
        let mut lanes = self.lock();
        let lane = lanes.entry(session).or_default();
        if !lane.busy {
            // Nobody has it: this caller does, and hands it on as it goes, a panic too, or
            // the lane would stay busy and every key after it would wait for good.
            lane.busy = true;
            drop(lanes);
            struct HandOn<'a>(&'a Lanes, u32);
            impl Drop for HandOn<'_> {
                fn drop(&mut self) {
                    self.0.hand_on(self.1);
                }
            }
            let _hand_on = HandOn(self, session);
            return Some(self.holding(session, work));
        }
        let (turn, my_turn) = std::sync::mpsc::sync_channel::<()>(1);
        let (done, done_with) = std::sync::mpsc::sync_channel::<()>(1);
        lane.jobs.push_back(Job::Turn(Box::new(move || {
            if turn.send(()).is_ok() {
                let _ = done_with.recv();
            }
        })));
        drop(lanes);
        // The worker ends only by handing the turn over, so this always comes, in time or
        // not. A caller that gave up has dropped `my_turn` and `done`, and the worker, finding
        // either gone, goes on with the lane.
        match within {
            None => {
                let _ = my_turn.recv();
            }
            Some(within) => {
                if my_turn.recv_timeout(within).is_err() {
                    return None;
                }
            }
        }
        let answer = self.holding(session, work);
        let _ = done.send(());
        Some(answer)
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
                    Job::Send { .. } => {}
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
                Job::Send {
                    droppable: true, ..
                } if failing => {}
                Job::Send { work, .. } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Lanes that tell each refusal on `told`.
    fn lanes() -> (Lanes, mpsc::Receiver<(u32, String)>) {
        let (tell, told) = mpsc::channel();
        let tell = Mutex::new(tell);
        let lanes = Lanes::new(Arc::new(move |session, why| {
            let _ = tell.lock().expect("the teller").send((session, why));
        }));
        (lanes, told)
    }

    /// A send that waits for `go` before it answers `answer`, so what is sent after it queues.
    fn held_until(
        go: mpsc::Receiver<()>,
        answer: Result<(), String>,
    ) -> impl FnOnce() -> Result<(), String> + Send + 'static {
        move || {
            let _ = go.recv_timeout(Duration::from_secs(10));
            answer
        }
    }

    /// Waits until chat `session`'s lane is let go: everything in it has run or been dropped.
    fn emptied(lanes: &Lanes, session: u32) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while lanes.lock().contains_key(&session) {
            assert!(
                std::time::Instant::now() < deadline,
                "the lane never emptied"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_views_close_and_a_resize_behind_a_refused_key_still_reach_the_host() {
        // Review of #4821: every send behind a refused one was dropped, a view's close too, and
        // a view never closed is a view the host keeps open and paced for good (#4810).
        let (lanes, told) = lanes();
        let (go, wait) = mpsc::channel();
        let ran = Arc::new(Mutex::new(Vec::new()));
        lanes.send(7, held_until(wait, Err("refused".to_owned())));
        let key = Arc::clone(&ran);
        lanes.send(7, move || {
            key.lock().expect("ran").push("key");
            Ok(())
        });
        let resize = Arc::clone(&ran);
        lanes.send_always(7, move || {
            resize.lock().expect("ran").push("resize");
            Ok(())
        });
        let close = Arc::clone(&ran);
        lanes.send_always(7, move || {
            close.lock().expect("ran").push("unwatch");
            Ok(())
        });
        go.send(()).expect("the first send waits");
        emptied(&lanes, 7);

        assert_eq!(*ran.lock().expect("ran"), vec!["resize", "unwatch"]);
        assert_eq!(told.try_recv(), Ok((7, "refused".to_owned())));
        assert!(told.try_recv().is_err(), "told more than once");
    }

    #[test]
    fn a_turn_behind_a_lane_that_does_not_empty_is_refused_by_its_deadline() {
        // Review of #4821: a stop or a smart close on the window's thread waited behind the
        // whole lane, so a host stuck on the pane's keys froze the window for as long.
        let (lanes, _told) = lanes();
        let (go, wait) = mpsc::channel();
        lanes.send(3, held_until(wait, Ok(())));
        let ran = Arc::new(Mutex::new(false));
        let mine = Arc::clone(&ran);
        let started = std::time::Instant::now();

        let answer = lanes.in_turn_within(3, Duration::from_millis(100), move || {
            *mine.lock().expect("ran") = true;
        });

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "waited {:?}",
            started.elapsed()
        );
        assert!(
            answer.is_err(),
            "the turn ran behind a lane that never emptied"
        );
        go.send(()).expect("the first send waits");
        emptied(&lanes, 3);
        assert!(!*ran.lock().expect("ran"), "a turn given up on still ran");

        // And the lane goes on: the next send is sent.
        let (sent, heard) = mpsc::channel();
        lanes.send(3, move || {
            let _ = sent.send(());
            Ok(())
        });
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("the lane goes on after a turn was given up on");
    }

    #[test]
    fn a_turn_that_panics_lets_its_chats_lane_go() {
        // Review of #4821: a turn taken on a free lane that panicked left it busy for good, so
        // every later key to that chat queued and was never sent.
        let (lanes, _told) = lanes();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            lanes.in_turn(5, || panic!("a turn that fails"));
        }));
        assert!(panicked.is_err());

        let (sent, heard) = mpsc::channel();
        lanes.send(5, move || {
            let _ = sent.send(());
            Ok(())
        });
        heard
            .recv_timeout(Duration::from_secs(5))
            .expect("a key after a turn that panicked is sent");
    }
}
