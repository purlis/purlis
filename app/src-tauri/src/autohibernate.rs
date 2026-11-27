// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Over the hot target, the longest-rested chat is hibernated for the person** (SC-4g, #7636;
//! spec #1391, *When the app acts*; ADR 0082 as amended 2026-11-25, the gate; ADR 0076 as
//! amended the same day, the rested chat and its looked-at clock).
//!
//! Three pieces, each its own:
//!
//! - [`LookedAt`], each project's **looked-at clock**, in memory. A chat on screen in a window
//!   that has not lost the focus is being looked at; its clock starts when it leaves the screen
//!   (its last pane goes, its tab goes behind, its window is minimised or hidden) or its window
//!   loses the focus, and a key the person types in it sets it to now. **It reads only
//!   [`Instant`]**, which on macOS (`mach_absolute_time`) does not advance while the Mac
//!   sleeps, so a night asleep rests no chat and a wall-clock jump cannot make one rested. The
//!   app's start, and so the project's opening, sets every clock to now.
//! - [`choose`], **the gate's one decision**, pure: under the target, nothing and nothing armed,
//!   however long a chat has rested; over it, the longest-rested chat whose time is up, or else
//!   the one deadline at which the first one's will be.
//! - [`Gate`], which reads the device's hot count and each chat's rest **only on the events that
//!   change them** ([`Gate::poke`]) and at its **one deadline**. There is no periodic sweep: with
//!   nothing that can become rested, nothing is armed. It hibernates one chat at a time, with
//!   cause `idle`, through the person's own act ([`crate::planes::Held::hibernate_chat_idle`]),
//!   which reads the predicate and the count again under the chat's latch, and reads the count
//!   again after each, so it stops at the target.
//!
//! **The gate picks only chats the act would accept now** ([`purlis_core::rested::by_hand`] as
//! well as [`purlis_core::rested::rested`]): until SC-4j (#7639), a chat that is a task or has
//! a task of its own is never picked. A chat the act still refuses is skipped, the next goes,
//! and the refused one is not tried again until a fact read of it changes, so no refusal
//! retried for ever starves the chats behind it.
//!
//! **Never a runner's or a pool's chat**: [`purlis_core::rested`] refuses one, and none counts
//! toward this device's hot count, whose memory it does not use.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use purlis_core::rested::Facts;

use crate::planes::{Held, PlaneId};

/// **How long a chat nobody looks at rests before the app may hibernate it**: the spec's
/// default, 30 minutes. SC-4h (#7637) makes it each project's `[chats] hibernate_after`.
pub const AFTER: Duration = Duration::from_secs(30 * 60);

/// What tells the time to the looked-at clock and the gate: **an [`Instant`], never the wall
/// clock's time**, so nothing a wall clock does can rest a chat. The app's is
/// [`Instant::now`]; a test holds its own ([`Held`]-clock in the tests).
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// The machine's monotonic clock, which stops while a Mac sleeps.
#[derive(Debug, Default)]
pub struct Monotonic;

impl Clock for Monotonic {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A clock a test holds still and moves on by hand: nothing here ever sleeps.
#[cfg(test)]
#[derive(Debug)]
pub struct HeldClock(Mutex<Instant>);

#[cfg(test)]
impl HeldClock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Instant::now())))
    }

    pub fn advance(&self, by: Duration) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) += by;
    }
}

#[cfg(test)]
impl Clock for HeldClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// **One project's looked-at clock**, by chat. A chat it has not read before rests from that
/// first read, never earlier ([`Self::rests_since`]): so the app's start, and the project's
/// opening in it, set every clock to now.
#[derive(Debug, Default)]
pub struct LookedAt {
    looks: Mutex<Looks>,
}

#[derive(Debug, Default)]
struct Looks {
    /// When each chat was last looked at or typed in.
    since: HashMap<u32, Instant>,
    /// The chats being looked at now: on screen in a window that has not lost the focus.
    looking: HashSet<u32>,
    /// The windows (`0` the project window) that said they lost the focus and have not had it
    /// back. A window that never said is taken as focused, so a chat on screen as the app
    /// opens is looked at until its window says otherwise.
    unfocused: HashSet<u32>,
}

impl LookedAt {
    fn looks(&self) -> MutexGuard<'_, Looks> {
        self.looks.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **What is on screen now**: `on_screen`, each chat with the window whose front tab shows
    /// it. A chat on screen in a window that has not lost the focus is being looked at; one
    /// that was and is not any more starts its clock now. Answers whether any chat's look
    /// changed. Closing one of two panes showing a chat leaves it on screen, and so leaves its
    /// look.
    pub fn on_screen(&self, now: Instant, on_screen: impl IntoIterator<Item = (u32, u32)>) -> bool {
        let mut looks = self.looks();
        let looking: HashSet<u32> = on_screen
            .into_iter()
            .filter(|(_, window)| !looks.unfocused.contains(window))
            .map(|(chat, _)| chat)
            .collect();
        if looking == looks.looking {
            return false;
        }
        let left: Vec<u32> = looks.looking.difference(&looking).copied().collect();
        for chat in left {
            looks.since.insert(chat, now);
        }
        looks.looking = looking;
        true
    }

    /// Window `window` gained or lost the focus. What is on screen is said again after it
    /// ([`Self::on_screen`]), which starts the clocks of the chats it showed.
    pub fn focused(&self, window: u32, focused: bool) {
        let mut looks = self.looks();
        if focused {
            looks.unfocused.remove(&window);
        } else {
            looks.unfocused.insert(window);
        }
    }

    /// The person typed in chat `session`: a look at it.
    pub fn typed(&self, session: u32, now: Instant) {
        self.looks().since.insert(session, now);
    }

    /// **Since when chat `session` has rested**: `None` while it is being looked at. A chat
    /// this clock has not seen before starts resting now, as an app's start does: never
    /// earlier than the clock first knew of it.
    pub fn rests_since(&self, session: u32, now: Instant) -> Option<Instant> {
        let mut looks = self.looks();
        if looks.looking.contains(&session) {
            return None;
        }
        Some(*looks.since.entry(session).or_insert(now))
    }

    /// Whether chat `session` has been looked at or typed in within `after`: what
    /// [`purlis_core::rested::Facts::looked_at`] is told.
    pub fn looked_at_within(&self, session: u32, now: Instant, after: Duration) -> bool {
        self.rests_since(session, now)
            .is_none_or(|since| now.saturating_duration_since(since) < after)
    }

    /// Chat `session` is gone, or was woken under another number.
    pub fn forget(&self, session: u32) {
        let mut looks = self.looks();
        looks.since.remove(&session);
        looks.looking.remove(&session);
    }
}

/// **A chat the gate may hibernate**, as it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<K> {
    pub chat: K,
    /// Since when nobody has looked at it.
    pub since: Instant,
}

/// What the gate does, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice<K> {
    /// Nothing, and nothing armed: under the target, or with nothing that can become rested.
    Nothing,
    /// Hibernate this chat now: it has rested longest of those whose time is up.
    Hibernate(K),
    /// Nothing is rested yet: the one deadline, when the first one will be.
    ArmFor(Instant),
}

/// **The gate's one decision** (ADR 0082 as amended 2026-11-25): with `hot` chats hot on this
/// device and a target of `target`, among `candidates` (the chats rested in every fact but
/// time), at `now`, resting `after` before they may go.
pub fn choose<K: Clone>(
    hot: u32,
    target: u32,
    candidates: &[Candidate<K>],
    now: Instant,
    after: Duration,
) -> Choice<K> {
    if hot <= target {
        return Choice::Nothing;
    }
    let Some(longest) = candidates.iter().min_by_key(|candidate| candidate.since) else {
        return Choice::Nothing;
    };
    let due = longest.since + after;
    if due <= now {
        Choice::Hibernate(longest.chat.clone())
    } else {
        Choice::ArmFor(due)
    }
}

/// A chat, by its project and number.
pub type Chat = (PlaneId, u32);

/// What reads the device's hot target, or `None` while its class is not known.
pub type Target = Arc<dyn Fn() -> Option<u32> + Send + Sync>;

/// What one read of the gate did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    Nothing,
    Hibernated(Chat),
    /// The act refused it: skipped until a fact of it changes.
    Refused(Chat),
    Armed(Instant),
}

/// **The gate**, one for the device.
pub struct Gate {
    clock: Arc<dyn Clock>,
    target: Mutex<Option<Target>>,
    projects: Mutex<Weak<Mutex<HashMap<PlaneId, Arc<Held>>>>>,
    /// The one deadline, where one is armed.
    deadline: Mutex<Option<Instant>>,
    /// Something it reads changed since it last read.
    poked: Mutex<bool>,
    woken: Condvar,
    /// The chats the act refused, with the facts it was refused on: not tried again while they
    /// read the same.
    refused: Mutex<HashMap<Chat, Facts>>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Gate {
    /// A gate reading `clock`, with no target yet: it hibernates nothing until it is told how
    /// to read one ([`Self::reads_its_target`]).
    pub fn on(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            target: Mutex::new(None),
            projects: Mutex::new(Weak::new()),
            deadline: Mutex::new(None),
            poked: Mutex::new(false),
            woken: Condvar::new(),
            refused: Mutex::new(HashMap::new()),
        }
    }

    /// What it tells the time by.
    pub fn now(&self) -> Instant {
        self.clock.now()
    }

    /// The projects it reads: the registry's own.
    pub fn reads_projects(&self, projects: Weak<Mutex<HashMap<PlaneId, Arc<Held>>>>) {
        *lock(&self.projects) = projects;
    }

    /// How it reads the device's hot target (SC-4f, #7635).
    pub fn reads_its_target(&self, target: Target) {
        *lock(&self.target) = Some(target);
        self.poke();
    }

    /// The deadline armed, where one is.
    pub fn armed(&self) -> Option<Instant> {
        *lock(&self.deadline)
    }

    /// **Something it reads may have changed**: a turn ended or began, a chat opened, closed,
    /// came on screen or left it, a window gained or lost the focus, a key was typed. Only
    /// marked here, never read on the caller's thread, which may hold a lock the read takes:
    /// the gate's own thread reads ([`Self::start`]).
    pub fn poke(&self) {
        *lock(&self.poked) = true;
        self.woken.notify_all();
    }

    /// **Starts its thread**: it waits for a poke or its deadline, and reads, one chat at a
    /// time, until a read hibernates nothing.
    pub fn start(self: &Arc<Self>) {
        let gate = Arc::downgrade(self);
        let _ = std::thread::Builder::new()
            .name("purlis-hibernate-gate".into())
            .spawn(move || {
                loop {
                    let Some(gate) = gate.upgrade() else { return };
                    gate.wait();
                    while matches!(gate.read(), Read::Hibernated(_) | Read::Refused(_)) {}
                }
            });
    }

    /// Waits for a poke, or for the deadline to pass: with none armed, for a poke alone.
    fn wait(&self) {
        let mut poked = lock(&self.poked);
        while !*poked {
            let Some(deadline) = *lock(&self.deadline) else {
                poked = self
                    .woken
                    .wait(poked)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let left = deadline.saturating_duration_since(self.clock.now());
            if left.is_zero() {
                return;
            }
            poked = self
                .woken
                .wait_timeout(poked, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        *poked = false;
    }

    fn projects(&self) -> Vec<Arc<Held>> {
        lock(&self.projects)
            .upgrade()
            .map(|open| lock(&open).values().map(Arc::clone).collect())
            .unwrap_or_default()
    }

    /// **How many chats are hot on this device**: every project's ([`hot_in`]).
    pub fn hot(&self) -> u32 {
        self.projects().iter().map(|held| hot_in(held)).sum()
    }

    /// Whether the device has more hot chats than its target now.
    pub fn over(&self) -> bool {
        let target = lock(&self.target).clone();
        target
            .and_then(|target| target())
            .is_some_and(|target| self.hot() > target)
    }

    /// **Reads once**, and does what [`choose`] says: hibernates one chat, or arms its one
    /// deadline, or neither.
    pub fn read(&self) -> Read {
        let target = lock(&self.target).clone().and_then(|target| target());
        let projects = self.projects();
        let hot = projects.iter().map(|held| hot_in(held)).sum();
        let Some(target) = target.filter(|target| hot > *target) else {
            *lock(&self.deadline) = None;
            return Read::Nothing;
        };
        let now = self.clock.now();
        let mut candidates = Vec::new();
        let mut facts_of = HashMap::new();
        for held in &projects {
            held.look_again(now);
            for session in held.chats().sessions_open() {
                let facts = crate::hibernation::facts_of(held, session, false);
                if purlis_core::rested::rested(&facts).is_err()
                    || purlis_core::rested::by_hand(&facts).is_err()
                {
                    continue;
                }
                let chat = (held.plane_id().clone(), session);
                if lock(&self.refused).get(&chat) == Some(&facts) {
                    continue;
                }
                let Some(since) = held.looked().rests_since(session, now) else {
                    continue;
                };
                facts_of.insert(chat.clone(), facts);
                candidates.push(Candidate { chat, since });
            }
        }
        // Only the facts of chats still open are kept: a closed one cannot come back.
        lock(&self.refused).retain(|chat, _| {
            projects
                .iter()
                .any(|held| held.plane_id() == &chat.0 && held.chats().sessions_open().contains(&chat.1))
        });
        match choose(hot, target, &candidates, now, AFTER) {
            Choice::Nothing => {
                *lock(&self.deadline) = None;
                Read::Nothing
            }
            Choice::ArmFor(deadline) => {
                *lock(&self.deadline) = Some(deadline);
                Read::Armed(deadline)
            }
            Choice::Hibernate(chat) => {
                *lock(&self.deadline) = None;
                let Some(held) = projects.iter().find(|held| held.plane_id() == &chat.0) else {
                    return Read::Nothing;
                };
                match held.hibernate_chat_idle(chat.1, &|| self.over()) {
                    Ok(()) => {
                        held.tell_hibernated(chat.1);
                        Read::Hibernated(chat)
                    }
                    // A reason of the core's: skipped while its facts read the same.
                    Err(crate::hibernation::Refused::Not(why)) => {
                        tracing::info!(
                            "purlis: chat {} was not hibernated by itself ({why:?})",
                            chat.1
                        );
                        if let Some(facts) = facts_of.remove(&chat) {
                            lock(&self.refused).insert(chat.clone(), facts);
                        }
                        Read::Refused(chat)
                    }
                    // The act could not be done now (under way already, being stopped, or the
                    // device at its target): read again on the next poke.
                    Err(crate::hibernation::Refused::Said(said)) => {
                        tracing::info!(
                            "purlis: chat {} was not hibernated by itself ({said})",
                            chat.1
                        );
                        Read::Nothing
                    }
                }
            }
        }
    }
}

/// **How many of `held`'s chats are hot**: open, with a harness, run on this machine, and with a
/// program (starting, working, at a turn's end, asking or paused); not one hibernated.
pub fn hot_in(held: &Held) -> u32 {
    let board = held.board();
    let chats = held.chats();
    let hot = chats
        .sessions_open()
        .into_iter()
        .filter(|&session| {
            chats.harness(session).is_some()
                && held.hibernated().of(session).is_none()
                && chats.runner_of(session).is_none()
                && chats.pool_of(session).is_none()
                && !matches!(
                    board.glance(session).state,
                    purlis_core::state::State::Done | purlis_core::state::State::Failed
                )
        })
        .count();
    u32::try_from(hot).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
