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
//! - at most [`at_once`] chats start at a time, across every project ([`Gate`]), and a start
//!   hands its place back once its line is composed, before it waits for the session host to
//!   answer ([`hand_back_here`]), and at most [`ON_THE_HOST_PER_PLACE`] times as many wait on
//!   their host at once, so Reopen all with many chats still wakes them a few at a time;
//! - a shell goes before a harness ([`Rank::heavy`]); then the chat in front of the project in
//!   front goes first, then each project's chat in front, then the rest in the order they were
//!   recorded ([`Rank`]);
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
/// never fewer than three and never more than four (each start is a harness, its sandbox and
/// its proxy coming up). One project takes all of them but one ([`Gate::enter`]), so three is
/// the fewest where one slow start is neither its own project's other chats' nor another
/// project's.
pub(crate) fn at_once() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(3)
        .clamp(3, 4)
}

/// How many starts wait on their session host at once for each place of the gate. A start
/// hands its place back once its line is composed (felt speed round 2, H4), so a quick shell is
/// not stuck behind a program coming up; what then keeps the programs coming up together few is
/// this bound, and not the places. Twice them: the host phase is mostly waiting, but Reopen all
/// with many chats (story 28) must still wake them a few at a time, never all together.
pub(crate) const ON_THE_HOST_PER_PLACE: usize = 2;

/// Where a start stands among the ones waiting: the least goes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rank {
    /// What its project starts next is a harness, and not a shell (felt speed round 2, H4). A
    /// shell starts in a fraction of a second and a harness's start in seconds, so a shell
    /// waiting goes before every harness, even one of the project in front.
    pub heavy: bool,
    /// Its project is not the one in front.
    pub behind: bool,
    /// Its project's chat in front is not among the ones still to start.
    pub hidden: bool,
}

/// Lets at most a few starts run at once, the one that matters most first.
pub(crate) struct Gate {
    most: usize,
    /// The most starts let in and not ended at once, those waiting on their host among them:
    /// [`ON_THE_HOST_PER_PLACE`] for each place.
    most_started: usize,
    lends_after: Duration,
    state: Mutex<State>,
    moved: Condvar,
}

#[derive(Default)]
struct State {
    /// The places taken, by the turn that took each, when, and for which project.
    held: Vec<(u64, Instant, PathBuf)>,
    /// Every start let in that has not ended, by its turn, when it went in, and for which
    /// project: one that has handed its place back is still here while it waits on its host.
    started: Vec<(u64, Instant, PathBuf)>,
    /// In a test: the most places held at once, and the most starts let in and not ended.
    #[cfg(test)]
    most_seen: (usize, usize),
    /// Who waits, by turn, where each last said it stands, and for which project.
    waiting: Vec<(u64, Rank, PathBuf)>,
    turns: u64,
}

impl State {
    /// How many places `project`'s starts hold that are not lent yet.
    fn holding(&self, project: &Path, now: Instant, lends_after: Duration) -> usize {
        of_project(&self.held, project, now, lends_after)
    }

    /// How many of `project`'s starts are let in and not ended, nor lent yet.
    fn started_by(&self, project: &Path, now: Instant, lends_after: Duration) -> usize {
        of_project(&self.started, project, now, lends_after)
    }
}

/// How many of `list`'s entries are `project`'s and not lent yet.
fn of_project(
    list: &[(u64, Instant, PathBuf)],
    project: &Path,
    now: Instant,
    lends_after: Duration,
) -> usize {
    list.iter()
        .filter(|(_, since, of)| of == project && now.duration_since(*since) < lends_after)
        .count()
}

/// A place at the gate, given back when it is dropped, or earlier by [`Self::hand_back`]; and a
/// start among the ones let in, until it is dropped.
pub(crate) struct Place {
    gate: Arc<Gate>,
    turn: u64,
}

impl Place {
    /// What the gate's places are for is done: another start may go in beside this one, which
    /// still counts among the starts let in, waiting on its host, until it is dropped.
    fn hand_back(&self) {
        let mut state = self.gate.lock();
        state.held.retain(|(turn, ..)| *turn != self.turn);
        drop(state);
        self.gate.moved.notify_all();
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.held.retain(|(turn, ..)| *turn != self.turn);
        state.started.retain(|(turn, ..)| *turn != self.turn);
        drop(state);
        self.gate.moved.notify_all();
    }
}

impl Gate {
    pub(crate) fn letting(most: usize, lends_after: Duration) -> Self {
        Self {
            most: most.max(1),
            most_started: most.max(1) * ON_THE_HOST_PER_PLACE,
            lends_after,
            state: Mutex::default(),
            moved: Condvar::new(),
        }
    }

    /// The one gate every project's launch goes through. In a test each project has a gate
    /// of its own, so a start one test holds takes no place from another test's chats.
    pub(crate) fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<Gate>> = OnceLock::new();
        #[cfg(test)]
        if let Some(gate) = ONE_GATE_IN_THIS_TEST.with_borrow(Clone::clone) {
            return gate;
        }
        if cfg!(test) {
            return Arc::new(Self::letting(at_once(), LENDS_AFTER));
        }
        Arc::clone(SHARED.get_or_init(|| Arc::new(Self::letting(at_once(), LENDS_AFTER))))
    }

    /// In a test: the most places held at once (the starts in what the gate is for), and the
    /// most starts let in and not ended at once (those waiting on their host among them).
    #[cfg(test)]
    pub(crate) fn most_seen(&self) -> (usize, usize) {
        self.lock().most_seen
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The most places one project's starts hold at once: all but one, so a project whose starts
    /// hang leaves a place for every other project's chats (the operator's report of
    /// 2026-11-03). Never fewer than one.
    fn most_for_one(&self) -> usize {
        self.most.saturating_sub(1).max(1)
    }

    /// The most starts of one project let in and not ended at once: all but the room
    /// [`Self::most_for_one`] leaves the other projects, so a project whose programs hang as
    /// they come up still leaves them one, and a gate of one place leaves none, as its places do.
    fn most_started_for_one(&self) -> usize {
        self.most_started - (self.most - self.most_for_one())
    }

    /// Waits for a place for a start of the project at `project`. `rank` is asked each time this
    /// looks, so a project brought to the front while its chats wait moves ahead. Among equals,
    /// the one that asked first goes first, of the projects that hold fewer places than
    /// [`Self::most_for_one`] and have fewer starts let in than [`Self::most_started_for_one`].
    pub(crate) fn enter(self: &Arc<Self>, project: &Path, rank: &dyn Fn() -> Rank) -> Place {
        let mut state = self.lock();
        state.turns += 1;
        let turn = state.turns;
        state.waiting.push((turn, rank(), project.to_path_buf()));
        loop {
            let now = Instant::now();
            let stands = rank();
            if let Some(mine) = state.waiting.iter_mut().find(|(one, ..)| *one == turn) {
                mine.1 = stands;
            }
            // A place held longer than a start should take is lent: it is not counted.
            let taken = state
                .held
                .iter()
                .filter(|(_, since, _)| now.duration_since(*since) < self.lends_after)
                .count();
            // And so is a start that has waited on its host that long.
            let started = state
                .started
                .iter()
                .filter(|(_, since, _)| now.duration_since(*since) < self.lends_after)
                .count();
            let first = state
                .waiting
                .iter()
                .filter(|(.., of)| {
                    state.holding(of, now, self.lends_after) < self.most_for_one()
                        && state.started_by(of, now, self.lends_after) < self.most_started_for_one()
                })
                .map(|(one, stands, _)| (*stands, *one))
                .min()
                .is_some_and(|(_, one)| one == turn);
            if taken < self.most && started < self.most_started && first {
                state.waiting.retain(|(one, ..)| *one != turn);
                state.held.push((turn, now, project.to_path_buf()));
                state.started.push((turn, now, project.to_path_buf()));
                #[cfg(test)]
                {
                    let (held, started) = (state.held.len(), state.started.len());
                    let most = &mut state.most_seen;
                    *most = (most.0.max(held), most.1.max(started));
                }
                drop(state);
                // Whoever is next looks at once, where another place is free.
                self.moved.notify_all();
                return Place {
                    gate: Arc::clone(self),
                    turn,
                };
            }
            state = self
                .moved
                .wait_timeout(state, LOOKS_AGAIN)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

thread_local! {
    /// The place the start on this thread holds, until it hands it back ([`hand_back_here`]).
    static HELD_HERE: std::cell::RefCell<Option<Place>> = const { std::cell::RefCell::new(None) };
}

/// The start about to be made on this thread holds `place` until [`hand_back_here`], called by
/// the start once its line is composed, and is among the starts let in until [`done_here`],
/// called by its caller after it.
pub(crate) fn hold_here(place: Place) {
    HELD_HERE.set(Some(place));
}

/// The start on this thread is done with what the gate's places are for, and hands its place
/// back (felt speed round 2, H4): its profile, its sandbox and its proxy are ready and its line
/// is composed, and what is left is waiting for the session host to answer. A start that held
/// its place through that wait kept a launch's later chats queued behind programs coming up: a
/// shell that needed 0.2 s waited 11 s. It still counts among the starts waiting on their host
/// ([`ON_THE_HOST_PER_PLACE`]) until [`done_here`]. Nothing where this thread holds no place.
pub(crate) fn hand_back_here() {
    HELD_HERE.with_borrow(|held| {
        if let Some(place) = held {
            place.hand_back();
        }
    });
}

/// The start on this thread has ended, however far it got: its place, where it still holds
/// one, is given back, and it is no longer among the starts waiting on their host.
pub(crate) fn done_here() {
    drop(HELD_HERE.take());
}

#[cfg(test)]
thread_local! {
    /// The gate every project opened on this thread shares, as the app's do, where a test
    /// said so ([`one_gate_in_this_test`]).
    static ONE_GATE_IN_THIS_TEST: std::cell::RefCell<Option<Arc<Gate>>> =
        const { std::cell::RefCell::new(None) };
}

/// In a test: every project opened on this thread from here starts its chats through `gate`,
/// one gate for all of them as in the app, and not one each.
#[cfg(test)]
pub(crate) fn one_gate_in_this_test(gate: Arc<Gate>) {
    ONE_GATE_IN_THIS_TEST.with_borrow_mut(|one| *one = Some(gate));
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
        heavy: true,
        behind: false,
        hidden: false,
    };
    const REST: Rank = Rank {
        heavy: true,
        behind: false,
        hidden: true,
    };
    /// A shell, of a project behind the one in front.
    const SHELL_BEHIND: Rank = Rank {
        heavy: false,
        behind: true,
        hidden: true,
    };

    #[test]
    fn no_more_start_at_once_than_the_gate_lets() {
        let gate = Arc::new(Gate::letting(2, Duration::from_secs(60)));
        let (inside, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for each in 0..8 {
                let (gate, inside, most) = (&gate, &inside, &most);
                scope.spawn(move || {
                    // A project each: one project alone takes all places but one.
                    let project = PathBuf::from(format!("/restoring/{each}"));
                    let _place = gate.enter(&project, &|| REST);
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
        let gate = Arc::new(Gate::letting(1, Duration::from_secs(60)));
        let order = Mutex::new(Vec::new());
        let held = gate.enter(Path::new("/restoring/one"), &|| REST);
        std::thread::scope(|scope| {
            for name in ["rest 1", "rest 2"] {
                let (gate, order) = (&gate, &order);
                scope.spawn(move || {
                    let _place = gate.enter(Path::new("/restoring/one"), &|| REST);
                    order.lock().unwrap().push(name);
                });
                // Each has asked before the next does.
                while gate.lock().waiting.len() < order_len(name) {
                    std::thread::yield_now();
                }
            }
            let (gate, order) = (&gate, &order);
            scope.spawn(move || {
                let _place = gate.enter(Path::new("/restoring/one"), &|| FRONT);
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

    /// The operator's report of 2026-11-03: one project of three came back. A project whose
    /// starts hang takes every place it can, and another project's chats then waited until
    /// each was lent; one place is always left for another project.
    #[test]
    fn a_project_never_holds_every_place_while_another_waits() {
        let gate = Arc::new(Gate::letting(2, Duration::from_secs(60)));
        let (stuck, other) = (Path::new("/restoring/stuck"), Path::new("/restoring/other"));
        // The stuck project's first start hangs, and holds its place.
        let _hung = gate.enter(stuck, &|| REST);
        let order = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            // Its next start asks first, and the other project's after it.
            let (gate, order) = (&gate, &order);
            scope.spawn(move || {
                let _place = gate.enter(stuck, &|| REST);
                order.lock().unwrap().push("stuck");
            });
            while gate.lock().waiting.is_empty() {
                std::thread::yield_now();
            }
            scope.spawn(move || {
                let _place = gate.enter(other, &|| REST);
                order.lock().unwrap().push("other");
            });
            // The other project's start goes in, past the one that asked first.
            let began = Instant::now();
            while order.lock().unwrap().is_empty() {
                assert!(began.elapsed() < Duration::from_secs(5), "nobody went in");
                std::thread::yield_now();
            }
            assert_eq!(*order.lock().unwrap(), ["other"]);
            drop(_hung);
        });
        assert_eq!(*order.lock().unwrap(), ["other", "stuck"]);
    }

    /// Projects open one after another, so the first has asked for every place before the next
    /// asks for any: it takes all but one, and the last waits for a project that comes later.
    #[test]
    fn a_project_takes_all_places_but_one() {
        let gate = Arc::new(Gate::letting(3, Duration::from_secs(60)));
        let (one, later) = (Path::new("/restoring/first"), Path::new("/restoring/later"));
        let _held: Vec<_> = (0..2).map(|_| gate.enter(one, &|| REST)).collect();
        let began = Instant::now();
        let _later = gate.enter(later, &|| REST);
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "the place left was taken"
        );
        // And one place of a gate of one is the project's: nothing is ever left unused.
        let alone = Arc::new(Gate::letting(1, Duration::from_secs(60)));
        drop(alone.enter(one, &|| REST));
    }

    #[test]
    fn a_start_that_takes_too_long_lends_its_place() {
        let gate = Arc::new(Gate::letting(1, Duration::from_millis(50)));
        let _hung = gate.enter(Path::new("/restoring/one"), &|| REST);
        let began = Instant::now();
        // The one place is held and never given back, and the next still gets in.
        let _next = gate.enter(Path::new("/restoring/one"), &|| REST);
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

    /// Felt speed round 2, H4: the three places of a gate were held by starts waiting on the
    /// session host, and a fourth waited until one was lent, 20 s on. A start parked in its
    /// host phase has handed its place back, and the next goes in at once.
    #[test]
    fn three_starts_parked_on_the_host_leave_a_place_for_a_fourth() {
        let gate = Arc::new(Gate::letting(3, Duration::from_secs(60)));
        let parked = std::sync::Barrier::new(4);
        let host_answers = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for each in 0..3 {
                let (gate, parked, host_answers) = (&gate, &parked, &host_answers);
                scope.spawn(move || {
                    let project = PathBuf::from(format!("/restoring/parked/{each}"));
                    hold_here(gate.enter(&project, &|| REST));
                    // Its line is composed: what is left is the host's answer.
                    hand_back_here();
                    parked.wait();
                    host_answers.wait();
                });
            }
            parked.wait();
            let (gate, (sent, got)) = (&gate, std::sync::mpsc::channel());
            scope.spawn(move || {
                let _place = gate.enter(Path::new("/restoring/parked/fourth"), &|| REST);
                let _ = sent.send(());
            });
            let fourth = got.recv_timeout(Duration::from_secs(5));
            host_answers.wait();
            assert!(fourth.is_ok(), "the fourth start found no place");
        });
    }

    /// Story 28 (Reopen all with many chats): a start that has handed its place back still
    /// waits on its host, and those waits are bounded too, at twice the gate's places, so a
    /// launch of many projects never brings every program up together.
    #[test]
    fn no_more_starts_wait_on_their_host_at_once_than_twice_the_places() {
        let gate = Arc::new(Gate::letting(2, Duration::from_secs(60)));
        let parked = AtomicUsize::new(0);
        let answered = std::sync::RwLock::new(());
        let host_answers = answered.write().unwrap();
        std::thread::scope(|scope| {
            for each in 0..8 {
                let (gate, parked, answered) = (&gate, &parked, &answered);
                scope.spawn(move || {
                    // A project each, so no project's own bound is what holds them.
                    let project = PathBuf::from(format!("/restoring/on-host/{each}"));
                    hold_here(gate.enter(&project, &|| REST));
                    // Its line is composed: it waits on the host until the test lets it go.
                    hand_back_here();
                    parked.fetch_add(1, Ordering::SeqCst);
                    drop(answered.read().unwrap());
                    // The start is over: what it still holds goes with the thread.
                });
            }
            let began = Instant::now();
            while parked.load(Ordering::SeqCst) < 4 {
                assert!(began.elapsed() < Duration::from_secs(5), "too few went in");
                std::thread::yield_now();
            }
            // Long enough for the others to go in, had a place been left for them.
            std::thread::sleep(Duration::from_millis(300));
            let most = parked.load(Ordering::SeqCst);
            drop(host_answers);
            assert_eq!(most, 4, "starts waiting on their host at once");
        });
        assert_eq!(
            parked.load(Ordering::SeqCst),
            8,
            "every start went in, in the end"
        );
    }

    /// The operator's report of 2026-11-03 again, past the hand-back: a project whose programs
    /// hang as they come up has handed every place back, and still leaves room for another
    /// project's starts among the ones waiting on their host.
    #[test]
    fn a_project_hung_on_its_host_never_takes_every_wait_while_another_asks() {
        let gate = Arc::new(Gate::letting(2, Duration::from_secs(60)));
        let (stuck, other) = (Path::new("/restoring/hung"), Path::new("/restoring/after"));
        // Three of the four waits on the host, each with its place handed back.
        let hung: Vec<Place> = (0..3)
            .map(|_| {
                let place = gate.enter(stuck, &|| REST);
                place.hand_back();
                place
            })
            .collect();
        let began = Instant::now();
        let (sent, got) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let gate = &gate;
            // The stuck project's next start asks first, and waits.
            let fourth = scope.spawn(move || drop(gate.enter(stuck, &|| REST)));
            while gate.lock().waiting.is_empty() {
                assert!(!fourth.is_finished(), "the stuck project's fourth went in");
                std::thread::yield_now();
            }
            scope.spawn(move || {
                let _place = gate.enter(other, &|| REST);
                let _ = sent.send(());
            });
            let other_went_in = got.recv_timeout(Duration::from_secs(5));
            let still_waiting = gate.lock().waiting.len();
            drop(hung);
            assert!(
                other_went_in.is_ok(),
                "the other project's start found no room"
            );
            assert_eq!(
                still_waiting, 1,
                "the stuck project's next start still waits"
            );
        });
        assert!(began.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_place_handed_back_is_not_given_back_twice() {
        let gate = Arc::new(Gate::letting(2, Duration::from_secs(60)));
        let other = gate.enter(Path::new("/restoring/twice/a"), &|| REST);
        hold_here(gate.enter(Path::new("/restoring/twice/b"), &|| REST));
        hand_back_here();
        hand_back_here();
        assert_eq!(
            gate.lock().held.len(),
            1,
            "the other start still holds its place"
        );
        assert_eq!(
            gate.lock().started.len(),
            2,
            "the start handed back still waits on its host"
        );
        done_here();
        done_here();
        assert_eq!(
            gate.lock().started.len(),
            1,
            "the other start has not ended"
        );
        drop(other);
        assert!(gate.lock().held.is_empty());
        assert!(gate.lock().started.is_empty());
    }

    /// Felt speed round 2, H4: a shell queued behind harnesses waited for every one of them.
    #[test]
    fn a_shell_goes_before_the_harnesses_that_asked_first_even_in_front() {
        let gate = Arc::new(Gate::letting(1, Duration::from_secs(60)));
        let order = Mutex::new(Vec::new());
        let held = gate.enter(Path::new("/restoring/shell/front"), &|| FRONT);
        std::thread::scope(|scope| {
            let (gate, order) = (&gate, &order);
            scope.spawn(move || {
                let _place = gate.enter(Path::new("/restoring/shell/front"), &|| FRONT);
                order.lock().unwrap().push("harness in front");
            });
            while gate.lock().waiting.is_empty() {
                std::thread::yield_now();
            }
            scope.spawn(move || {
                let _place = gate.enter(Path::new("/restoring/shell/behind"), &|| SHELL_BEHIND);
                order.lock().unwrap().push("shell behind");
            });
            while gate.lock().waiting.len() < 2 {
                std::thread::yield_now();
            }
            drop(held);
        });
        assert_eq!(*order.lock().unwrap(), ["shell behind", "harness in front"]);
    }
}
