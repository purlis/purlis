// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A launch's chats come back in the background (the operator's report of 2026-10-13).
//!
//! Putting a record back starts one program per chat, and each start resolves a profile,
//! compiles a sandbox and waits for a terminal. A launch used to do that for every chat of
//! every project before the window could be used, so its wait grew with what was open. Now a
//! project's record is **registered** at once, which starts nothing, and its chats start
//! behind the window:
//!
//! - the window draws every chat's tab in its place from the first moment, as *restoring*
//!   (`Chats::restoring_now`), and each becomes the chat as it starts ([`Moved`]);
//! - at most [`at_once`] chats start at a time, across every project ([`Gate`]);
//! - the chat in front of the project in front goes first, then each project's chat in front,
//!   then the rest in the order they were recorded ([`Rank`]);
//! - a chat that does not start is listed as one that did not, as it always was, and the rest
//!   are not held up by it. One whose start takes very long lends its place ([`LENDS_AFTER`]).
//!
//! **Nothing about a start changes**: each chat goes through the one start every chat goes
//! through, with every check it makes, and none starts before them. Only when it starts moved.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// How long a start holds its place before another may start beside it. A program that hangs
/// as it starts would otherwise hold a place for the life of the app.
pub(crate) const LENDS_AFTER: Duration = Duration::from_secs(20);

/// How often a start that waits looks again: for a place lent, and for the project in front
/// having changed.
const LOOKS_AGAIN: Duration = Duration::from_millis(100);

/// How many chats start at once, across every project: as many as the machine has cores for,
/// never fewer than two (so one slow start is not everyone's) and never more than four (each
/// start is a harness, its sandbox and its proxy coming up).
pub(crate) fn at_once() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(2)
        .clamp(2, 4)
}

/// Where a start stands among the ones waiting: the least goes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rank {
    /// Its project is not the one in front.
    pub behind: bool,
    /// Its project's chat in front is not among the ones still to start.
    pub hidden: bool,
}

/// Lets at most a few starts run at once, the one that matters most first.
pub(crate) struct Gate {
    most: usize,
    lends_after: Duration,
    state: Mutex<State>,
    moved: Condvar,
}

#[derive(Default)]
struct State {
    /// The places taken, by the turn that took each, and when.
    held: Vec<(u64, Instant)>,
    /// Who waits, by turn, and where each last said it stands.
    waiting: Vec<(u64, Rank)>,
    turns: u64,
}

/// A place at the gate, given back when it is dropped.
pub(crate) struct Place<'a> {
    gate: &'a Gate,
    turn: u64,
}

impl Drop for Place<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.held.retain(|(turn, _)| *turn != self.turn);
        drop(state);
        self.gate.moved.notify_all();
    }
}

impl Gate {
    pub(crate) fn letting(most: usize, lends_after: Duration) -> Self {
        Self {
            most: most.max(1),
            lends_after,
            state: Mutex::default(),
            moved: Condvar::new(),
        }
    }

    /// The one gate every project's launch goes through. In a test each project has a gate
    /// of its own, so a start one test holds takes no place from another test's chats.
    pub(crate) fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<Gate>> = OnceLock::new();
        if cfg!(test) {
            return Arc::new(Self::letting(at_once(), LENDS_AFTER));
        }
        Arc::clone(SHARED.get_or_init(|| Arc::new(Self::letting(at_once(), LENDS_AFTER))))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Waits for a place. `rank` is asked each time this looks, so a project brought to the
    /// front while its chats wait moves ahead. Among equals, the one that asked first goes
    /// first.
    pub(crate) fn enter(&self, rank: &dyn Fn() -> Rank) -> Place<'_> {
        let mut state = self.lock();
        state.turns += 1;
        let turn = state.turns;
        state.waiting.push((turn, rank()));
        loop {
            let now = Instant::now();
            let stands = rank();
            if let Some(mine) = state.waiting.iter_mut().find(|(one, _)| *one == turn) {
                mine.1 = stands;
            }
            // A place held longer than a start should take is lent: it is not counted.
            let taken = state
                .held
                .iter()
                .filter(|(_, since)| now.duration_since(*since) < self.lends_after)
                .count();
            let first = state
                .waiting
                .iter()
                .map(|(one, stands)| (*stands, *one))
                .min()
                .is_some_and(|(_, one)| one == turn);
            if taken < self.most && first {
                state.waiting.retain(|(one, _)| *one != turn);
                state.held.push((turn, now));
                drop(state);
                // Whoever is next looks at once, where another place is free.
                self.moved.notify_all();
                return Place { gate: self, turn };
            }
            state = self
                .moved
                .wait_timeout(state, LOOKS_AGAIN)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// The project the main window has in front, as it last said (`window_holds_planes`): its
/// chats start before another project's.
static IN_FRONT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The window says which project it has in front, by its root, or that it has none.
pub(crate) fn project_in_front(root: Option<&Path>) {
    *IN_FRONT.lock().unwrap_or_else(PoisonError::into_inner) = root.map(Path::to_path_buf);
}

/// Whether another project than `root` is the one in front. With none said yet, none is
/// behind another.
pub(crate) fn is_behind(root: &Path) -> bool {
    IN_FRONT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_deref()
        .is_some_and(|front| front != root)
}

/// One chat of a launch started, or did not: what the window is told, so the tab it drew as
/// restoring becomes the chat, or goes and the chat is listed as one that did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Moved {
    /// The project's root.
    pub root: PathBuf,
    /// The number its tab was drawn under.
    pub number: u32,
    /// The session it started as, or none where it did not start.
    pub session: Option<u32>,
    /// How many of the project's chats are still to start.
    pub left: usize,
}

/// What hears each [`Moved`].
pub(crate) type Tell = Arc<dyn Fn(Moved) + Send + Sync>;

static TELL: Mutex<Option<Tell>> = Mutex::new(None);

/// Sets what is told as each chat of a launch starts: the app's own window.
pub(crate) fn telling(tell: Tell) {
    *TELL.lock().unwrap_or_else(PoisonError::into_inner) = Some(tell);
}

/// Tells the window, where one listens. Called with no lock of a project's held.
pub(crate) fn tell(moved: Moved) {
    let tell = TELL.lock().unwrap_or_else(PoisonError::into_inner).clone();
    if let Some(tell) = tell {
        tell(moved);
    }
}

/// The event the window hears a [`Moved`] as.
pub(crate) const RESTORED: &str = "chat-restored";

/// A [`Moved`] as the window hears it. It reads the project's chats again on each
/// (`opened_chats`), so one it misses is made up by the next.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ChatRestored {
    pub plane: crate::planes::PlaneId,
    /// The number its tab was drawn under while it was restoring.
    pub number: u32,
    /// The session it started as, or `null` where it did not: it is then among the chats that
    /// would not start.
    pub session: Option<u32>,
    /// How many of the project's chats are still to start.
    pub left: u32,
}

/// The names `tools/bench.mjs` reads a launch's restore by (its `restore` case): when the
/// window could be used, when the first chat had started, and when the last had.
pub(crate) const BENCH_USABLE: &str = "usable";
const BENCH_FIRST_CHAT: &str = "first-chat";
const BENCH_ALL_CHATS: &str = "all-chats";

/// Prints `charter-bench <what> <ms>` once per launch, and only where `tools/bench.mjs` asked
/// (`PURLIS_BENCH_LOG`, as `first_frame` does).
pub(crate) fn say_for_the_bench(what: &'static str, since_start: Duration) {
    static SAID: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    if purlis_core::envvar::var_os("PURLIS_BENCH_LOG").is_none() {
        return;
    }
    let mut said = SAID.lock().unwrap_or_else(PoisonError::into_inner);
    if said.contains(&what) {
        return;
    }
    said.push(what);
    println!("charter-bench {what} {}", since_start.as_millis());
}

/// What a [`Moved`] is to the bench: the first chat started, and the last one tried. A
/// launch of several projects says the last of the first project to finish, which the bench's
/// one-project case reads.
pub(crate) fn said_for_the_bench(moved: &Moved, since_start: Duration) {
    if moved.session.is_some() {
        say_for_the_bench(BENCH_FIRST_CHAT, since_start);
    }
    if moved.left == 0 {
        say_for_the_bench(BENCH_ALL_CHATS, since_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FRONT: Rank = Rank {
        behind: false,
        hidden: false,
    };
    const REST: Rank = Rank {
        behind: false,
        hidden: true,
    };

    #[test]
    fn no_more_start_at_once_than_the_gate_lets() {
        let gate = Gate::letting(2, Duration::from_secs(60));
        let (inside, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    let _place = gate.enter(&|| REST);
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    inside.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        assert_eq!(most.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_one_in_front_goes_before_the_ones_that_asked_earlier() {
        let gate = Gate::letting(1, Duration::from_secs(60));
        let order = Mutex::new(Vec::new());
        let held = gate.enter(&|| REST);
        std::thread::scope(|scope| {
            for name in ["rest 1", "rest 2"] {
                let (gate, order) = (&gate, &order);
                scope.spawn(move || {
                    let _place = gate.enter(&|| REST);
                    order.lock().unwrap().push(name);
                });
                // Each has asked before the next does.
                while gate.lock().waiting.len() < order_len(name) {
                    std::thread::yield_now();
                }
            }
            let (gate, order) = (&gate, &order);
            scope.spawn(move || {
                let _place = gate.enter(&|| FRONT);
                order.lock().unwrap().push("front");
            });
            while gate.lock().waiting.len() < 3 {
                std::thread::yield_now();
            }
            drop(held);
        });
        assert_eq!(*order.lock().unwrap(), ["front", "rest 1", "rest 2"]);
    }

    fn order_len(name: &str) -> usize {
        if name == "rest 1" { 1 } else { 2 }
    }

    #[test]
    fn a_start_that_takes_too_long_lends_its_place() {
        let gate = Gate::letting(1, Duration::from_millis(50));
        let _hung = gate.enter(&|| REST);
        let began = Instant::now();
        // The one place is held and never given back, and the next still gets in.
        let _next = gate.enter(&|| REST);
        assert!(began.elapsed() >= Duration::from_millis(50));
        assert!(began.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_project_is_behind_only_once_another_is_said_to_be_in_front() {
        // The one test that says which project is in front, so no other reads it changed.
        let (here, there) = (Path::new("/restoring/here"), Path::new("/restoring/there"));
        project_in_front(None);
        assert!(!is_behind(here));
        project_in_front(Some(there));
        assert!(is_behind(here));
        assert!(!is_behind(there));
        project_in_front(None);
    }
}
