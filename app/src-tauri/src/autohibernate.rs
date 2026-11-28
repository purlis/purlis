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
//! a task of its own is never picked. A chat the act still refuses is passed over for the rest
//! of the pass and the next goes. One the act could not do for it ([`Refused::Said`]) is not
//! tried again until it begins a turn, the device goes to its target, or [`AFTER`] has passed
//! since, whichever comes first: so no refusal retried for ever starves the chats behind it,
//! and none keeps a chat awake for ever either.
//!
//! [`Refused::Said`]: crate::hibernation::Refused::Said
//!
//! **Never a runner's or a pool's chat**: [`purlis_core::rested`] refuses one, and none counts
//! toward this device's hot count, whose memory it does not use.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

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

/// **One project's looked-at clock**, by chat. Every chat open is put on it as it is seen open
/// ([`Self::open`]), at every read of the gate, under the target as over it, and a chat it has
/// not read before rests from that first read, never earlier ([`Self::rests_since`]): so the
/// app's start, and the project's opening in it, set every clock to now.
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

    /// The person typed in chat `session`: a look at it. Answers whether that changed what the
    /// gate reads: not for a chat already being looked at, whose rest has not begun.
    pub fn typed(&self, session: u32, now: Instant) -> bool {
        let mut looks = self.looks();
        looks.since.insert(session, now);
        !looks.looking.contains(&session)
    }

    /// **The chats open now**, `open`: each one the clock has not seen before starts resting
    /// `now`, so the app's start, a project's opening, a chat's opening or restore and a
    /// woken chat's new number each set its clock to now, read under the target as over it;
    /// one no longer open is forgotten.
    pub fn open(&self, now: Instant, open: &[u32]) {
        let mut looks = self.looks();
        looks.since.retain(|session, _| open.contains(session));
        looks.looking.retain(|session| open.contains(session));
        for &session in open {
            looks.since.entry(session).or_insert(now);
        }
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

/// The projects the app holds: the registry's own map ([`crate::planes::Planes`]).
pub type Projects = Mutex<HashMap<PlaneId, Arc<Held>>>;

/// What reads the device's hot target, or `None` while its class is not known.
pub type Target = Arc<dyn Fn() -> Option<u32> + Send + Sync>;

/// What one read of the gate did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    Nothing,
    Hibernated(Chat),
    /// The act refused it: passed over for the rest of this pass, and, where the act could not
    /// be done, kept refused ([`Refusal`]).
    Refused(Chat),
    Armed(Instant),
}

/// **What the gate's thread waits on**: a poke, the one deadline, or the gate's end. The thread
/// holds this, and the gate only weakly, so it ends once the gate goes ([`Gate::start`]).
struct Signal {
    clock: Arc<dyn Clock>,
    /// The one deadline, where one is armed.
    deadline: Mutex<Option<Instant>>,
    woken: Mutex<Woken>,
    wake: Condvar,
}

#[derive(Debug, Default)]
struct Woken {
    /// Something the gate reads changed since it last read.
    poked: bool,
    /// The gate is gone.
    stopped: bool,
}

impl Signal {
    fn tell(&self, how: impl FnOnce(&mut Woken)) {
        how(&mut lock(&self.woken));
        self.wake.notify_all();
    }

    /// **Waits for a poke, or for the deadline to pass**: with none armed, for a poke alone.
    /// Answers `false` once the gate is gone.
    fn wait(&self) -> bool {
        let mut woken = lock(&self.woken);
        loop {
            if woken.stopped {
                return false;
            }
            if woken.poked {
                woken.poked = false;
                return true;
            }
            let Some(deadline) = *lock(&self.deadline) else {
                woken = self
                    .wake
                    .wait(woken)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            let left = deadline.saturating_duration_since(self.clock.now());
            if left.is_zero() {
                return true;
            }
            woken = self
                .wake
                .wait_timeout(woken, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// **A chat the act could not hibernate** ([`crate::hibernation::Refused::Said`]): not tried
/// again while it has begun no turn since (its board's count of turns, which only goes up), the
/// device stays over its target, and [`AFTER`] has not passed since `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Refusal {
    /// How many turns it had begun as the act was tried.
    turns: u32,
    /// When it was refused.
    at: Instant,
}

impl Refusal {
    /// When it lapses, though nothing else changed.
    fn lapses(&self) -> Instant {
        self.at + AFTER
    }
}

/// **The gate**, one for the device.
pub struct Gate {
    signal: Arc<Signal>,
    target: Mutex<Option<Target>>,
    projects: Mutex<Weak<Projects>>,
    /// The chats the act could not hibernate, kept refused ([`Refusal`]).
    refused: Mutex<HashMap<Chat, Refusal>>,
    /// The chats refused in this pass ([`Self::pass`]): not tried again in it.
    passed_over: Mutex<HashSet<Chat>>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field("deadline", &self.signal.deadline)
            .finish_non_exhaustive()
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        self.signal.tell(|woken| woken.stopped = true);
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
            signal: Arc::new(Signal {
                clock,
                deadline: Mutex::new(None),
                woken: Mutex::new(Woken::default()),
                wake: Condvar::new(),
            }),
            target: Mutex::new(None),
            projects: Mutex::new(Weak::new()),
            refused: Mutex::new(HashMap::new()),
            passed_over: Mutex::new(HashSet::new()),
        }
    }

    /// What it tells the time by.
    pub fn now(&self) -> Instant {
        self.signal.clock.now()
    }

    /// The projects it reads: the registry's own.
    pub fn reads_projects(&self, projects: Weak<Projects>) {
        *lock(&self.projects) = projects;
    }

    /// How it reads the device's hot target (SC-4f, #7635).
    pub fn reads_its_target(&self, target: Target) {
        *lock(&self.target) = Some(target);
        self.poke();
    }

    /// The deadline armed, where one is.
    #[cfg(test)]
    pub fn armed(&self) -> Option<Instant> {
        *lock(&self.signal.deadline)
    }

    /// Takes its deadline away, as a test that holds the clock past it while the thread waits.
    #[cfg(test)]
    pub fn disarm(&self) {
        self.arm(None);
    }

    fn arm(&self, deadline: Option<Instant>) {
        *lock(&self.signal.deadline) = deadline;
    }

    /// **Something it reads may have changed**: a turn ended or began, a chat opened, closed,
    /// came on screen or left it, a window gained or lost the focus, a key was typed, the
    /// device's class was read. Only marked here, never read on the caller's thread, which may
    /// hold a lock the read takes: the gate's own thread reads ([`Self::start`]).
    pub fn poke(&self) {
        self.signal.tell(|woken| woken.poked = true);
    }

    /// **Starts its thread**: it waits for a poke or its deadline, and makes a pass
    /// ([`Self::pass`]). It holds the gate only while it reads, and ends once the gate goes.
    /// Answers why, where the thread could not be started.
    pub fn start(self: &Arc<Self>) -> std::io::Result<std::thread::JoinHandle<()>> {
        let gate = Arc::downgrade(self);
        let signal = Arc::clone(&self.signal);
        std::thread::Builder::new()
            .name("purlis-hibernate-gate".into())
            .spawn(move || {
                while signal.wait() {
                    let Some(gate) = gate.upgrade() else { return };
                    gate.pass();
                }
            })
    }

    /// **One pass**: reads, one chat at a time, until a read neither hibernates nor is refused.
    /// A chat refused in it is passed over for the rest of it.
    pub fn pass(&self) {
        lock(&self.passed_over).clear();
        while matches!(self.read(), Read::Hibernated(_) | Read::Refused(_)) {}
    }

    fn projects(&self) -> Vec<Arc<Held>> {
        lock(&self.projects)
            .upgrade()
            .map(|open| lock(&open).values().map(Arc::clone).collect())
            .unwrap_or_default()
    }

    /// **How many chats are hot on this device**: every project's
    /// ([`crate::diagnostics::hot_in`], what Diagnostics counts).
    pub fn hot(&self) -> u32 {
        self.projects()
            .iter()
            .map(|held| crate::diagnostics::hot_in(held))
            .sum()
    }

    /// Whether the device has more hot chats than its target now.
    pub fn over(&self) -> bool {
        let target = lock(&self.target).clone();
        target
            .and_then(|target| target())
            .is_some_and(|target| self.hot() > target)
    }

    /// **Reads once**, and does what [`choose`] says: hibernates one chat, or arms its one
    /// deadline, or neither. Every chat open is put on its project's clock first, under the
    /// target as over it, so the one rested longest is the one whose clock started first. The
    /// one deadline is the earliest of the first rest to be up and the first refusal to lapse.
    pub fn read(&self) -> Read {
        let projects = self.projects();
        let now = self.now();
        for held in &projects {
            held.look_again(now);
        }
        let target = lock(&self.target).clone().and_then(|target| target());
        let hot = projects
            .iter()
            .map(|held| crate::diagnostics::hot_in(held))
            .sum();
        let Some(target) = target.filter(|target| hot > *target) else {
            // At or under the target, every refusal lets go: over it again, each is tried anew.
            lock(&self.refused).clear();
            self.arm(None);
            return Read::Nothing;
        };
        // Only the refusals of chats still open, and not lapsed, are kept.
        lock(&self.refused).retain(|chat, refusal| {
            refusal.lapses() > now
                && projects.iter().any(|held| {
                    held.plane_id() == &chat.0 && held.chats().sessions_open().contains(&chat.1)
                })
        });
        let mut candidates = Vec::new();
        let mut lapses: Option<Instant> = None;
        for held in &projects {
            for session in held.chats().sessions_open() {
                let facts = crate::hibernation::facts_of(held, session, false);
                if purlis_core::rested::rested(&facts).is_err()
                    || purlis_core::rested::by_hand(&facts).is_err()
                {
                    continue;
                }
                let Some(since) = held.looked().rests_since(session, now) else {
                    continue;
                };
                let chat = (held.plane_id().clone(), session);
                // Read before the pass's own skip, so one refused in this pass arms its lapse.
                if let Some(refusal) = self.still_refused(&chat, turns_of(held, session)) {
                    lapses = Some(lapses.map_or(refusal.lapses(), |at| at.min(refusal.lapses())));
                    continue;
                }
                if lock(&self.passed_over).contains(&chat) {
                    continue;
                }
                candidates.push(Candidate { chat, since });
            }
        }
        let armed = |deadline: Option<Instant>| {
            self.arm(deadline);
            deadline.map_or(Read::Nothing, Read::Armed)
        };
        match choose(hot, target, &candidates, now, AFTER) {
            Choice::Nothing => armed(lapses),
            Choice::ArmFor(due) => armed(Some(lapses.map_or(due, |at| at.min(due)))),
            Choice::Hibernate(chat) => {
                self.arm(None);
                let Some(held) = projects.iter().find(|held| held.plane_id() == &chat.0) else {
                    return Read::Nothing;
                };
                let turns = turns_of(held, chat.1);
                match held.hibernate_chat_idle(chat.1, &|| self.over()) {
                    Ok(()) => {
                        held.tell_hibernated(chat.1);
                        Read::Hibernated(chat)
                    }
                    Err(why) => self.refused(chat, turns, &why),
                }
            }
        }
    }

    /// Chat `chat`'s refusal, while it holds: it lets go once the chat has begun a turn since.
    fn still_refused(&self, chat: &Chat, turns: u32) -> Option<Refusal> {
        let mut refused = lock(&self.refused);
        let refusal = *refused.get(chat)?;
        if refusal.turns != turns {
            refused.remove(chat);
            return None;
        }
        Some(refusal)
    }

    /// **The act refused `chat`**, which had begun `turns` turns as it was tried, for `why`. At
    /// the target, or with the app quitting, the pass ends. Any other refusal passes it over for
    /// the rest of the pass, and the next goes. One the chat's facts gave
    /// ([`crate::hibernation::Refused::Not`]) is not kept: the facts are read again at every
    /// read, and pick it only once they allow it. One the act could not do
    /// ([`crate::hibernation::Refused::Said`]) is kept ([`Refusal`]), so it is not tried again
    /// on each keystroke while nothing changed.
    pub(crate) fn refused(
        &self,
        chat: Chat,
        turns: u32,
        why: &crate::hibernation::Refused,
    ) -> Read {
        use crate::hibernation::Refused;
        tracing::info!(
            "purlis: chat {} was not hibernated by itself ({why:?})",
            chat.1
        );
        if let Refused::Said(said) = why
            && (said == crate::hibernation::AT_THE_TARGET
                || said == crate::planes::QUIT_AS_IT_HIBERNATED)
        {
            return Read::Nothing;
        }
        lock(&self.passed_over).insert(chat.clone());
        if let Refused::Said(_) = why {
            let at = self.now();
            lock(&self.refused).insert(chat.clone(), Refusal { turns, at });
        }
        Read::Refused(chat)
    }
}

/// How many turns chat `session` has begun this run: a count that only goes up, so a turn
/// begun and ended between two reads still shows.
fn turns_of(held: &Held, session: u32) -> u32 {
    held.board().glance(session).turns
}

#[cfg(test)]
mod tests;
