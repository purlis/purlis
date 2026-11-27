// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's prerequisites** (ADR 0130 §1 to §3): the conditions a project must meet before
//! its chats can work, which only the person can satisfy, in the window.
//!
//! The core defines each one, decides when it is open and sets their order ([`Kind`]); the window
//! never decides one itself. It ships with **one, the policy one** ([`Policy`]): open while this
//! machine's acceptance of the project's `[policy]` is **Broken** or **Waiting**.
//!
//! **The app's process holds the list**, per project ([`Kept`]), and is told the whole ordered
//! list, never a delta, each time it changes, after the new list is kept: so a reader that asks
//! in the handler gets the same list. The list is in memory only and evaluated again on a
//! project's open; it is no store (ADR 0069).
//!
//! **It is evaluated again** ([`Kept::evaluate`]) on a project's open, on a change to any file
//! [`watched`] names (the plane root's `HEAD`, the settings file, the local settings file, this
//! machine's policy file, the acceptance record and the never-dispatch list), on a new seal key,
//! and after every answer ([`Kept::accept`], [`Kept::accept_with_new_key`]). Where its watch
//! misses a file, at a read too, at most once a second ([`Kept::lapsed`]).
//!
//! **While one is open, the core holds the chats the person starts in that project**
//! ([`Kept::hold`], ADR 0130 §5): a chat they start from the window or the CLI, a pool or runner
//! chat among them, waits until they answer. Every other start ([`Starter`]) starts at once,
//! under the policy in force ([`enforcement::in_force`]), as running chats keep running under it.
//! The window only shows the hold and releases it, through an answer.
//!
//! **The answer that grants nothing** ([`Kept::defer`], ADR 0130 §6) is kept in this list's
//! memory, against the digest and reasons it was shown for, and never written anywhere: it ends
//! with the app's run, and an entry whose digest or reasons differ from every answer kept is
//! open again, while one back at a digest and reasons already answered this run is not.
//! Meanwhile the entry reads [`State::Deferred`], which holds no start. It grants nothing: the
//! policy in force is what it was.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use crate::enforcement::{self, Standing};

/// **What a prerequisite is about**, in the core's fixed order: a list holds its entries in the
/// order these are declared, whichever was evaluated first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The project's `[policy]`, not accepted on this machine ([`Policy`]).
    Policy,
    /// A stand-in for a second kind, for a test of the order: only the policy one ships.
    #[cfg(test)]
    StandIn,
}

/// **Where an open prerequisite stands this run.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Unanswered: it waits for the person.
    Open,
    /// Answered for this run with the answer that grants nothing (ADR 0130 §6), against the
    /// digest and reason it was shown for.
    Deferred,
}

/// **Why the policy prerequisite is open**: one or both, Broken first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// An acceptance was kept on this machine, and what is here now is not it.
    Broken,
    /// The committed policy is looser than what this machine accepted: than the shipped
    /// `watch` where nothing was kept, or where what was kept is broken.
    Waiting,
}

/// **The policy prerequisite** (ADR 0130 §3), as one read of the project's policy found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Open, or deferred for this run.
    pub state: State,
    /// Broken, Waiting, or both, in that order.
    pub reasons: Vec<Reason>,
    /// The digest an Accept must name ([`enforcement::press`]).
    pub digest: String,
    /// Whether Accept must first replace a seal key another program made (#7714).
    pub needs_new_key: bool,
    /// Whether the acceptance broke because purlis replaced its seal key on this machine since
    /// it was sealed (#7714), never because something edited it.
    pub after_new_key: bool,
    /// The read it was found in: what waits, as [`enforcement::standing`] gives it, which the
    /// window lists.
    pub standing: Standing,
}

/// **One prerequisite of a project.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prerequisite {
    /// The project's `[policy]`, not accepted on this machine.
    Policy(Box<Policy>),
    /// A stand-in for a second kind, for a test of the order.
    #[cfg(test)]
    StandIn,
}

impl Prerequisite {
    /// What it is about.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Policy(_) => Kind::Policy,
            #[cfg(test)]
            Self::StandIn => Kind::StandIn,
        }
    }

    /// Where it stands this run.
    pub fn state(&self) -> State {
        match self {
            Self::Policy(policy) => policy.state,
            #[cfg(test)]
            Self::StandIn => State::Open,
        }
    }
}

/// **A project's prerequisites**, in the core's order ([`Kind`]): none where it needs nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    entries: Vec<Prerequisite>,
}

impl List {
    /// The list of `entries`, put in the core's order.
    fn of(mut entries: Vec<Prerequisite>) -> Self {
        entries.sort_by_key(Prerequisite::kind);
        Self { entries }
    }

    /// Each prerequisite, in the core's order.
    pub fn entries(&self) -> &[Prerequisite] {
        &self.entries
    }

    /// Whether the project needs nothing of the person.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// **Whether it holds the person's starts**: an entry is open, unanswered this run.
    pub fn holds(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.state() == State::Open)
    }

    /// This list, each entry deferred whose digest and reasons an answer kept (`answers`) was
    /// shown for, once `answers` has noted each pair it holds as held this run.
    fn deferred_by(mut self, answers: &mut Answers) -> Self {
        for entry in &mut self.entries {
            match entry {
                Prerequisite::Policy(policy) => {
                    let pair = (policy.digest.clone(), policy.reasons.clone());
                    if answers.held(pair) {
                        policy.state = State::Deferred;
                    }
                }
                #[cfg(test)]
                Prerequisite::StandIn => {}
            }
        }
        self
    }
}

/// **Who starts a chat**, as an open prerequisite holds it (ADR 0130 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Starter {
    /// **The person**: a chat they start from the window or the CLI, a session they resume,
    /// and a pool or runner chat they start, which waits as a local one does. It waits.
    Person,
    /// **The app, on its own**: a chat put back at a project's open, started again in its
    /// place, or woken for its schedule or its loop. It starts at once.
    App,
    /// **A running chat**: a task it dispatches, or a chat it hands work to, which starts under
    /// the policy in force as a dispatched task does. It starts at once.
    AChat,
}

impl Starter {
    /// Who made a start made as `made` says, from the app's own record of it.
    pub fn of(made: &crate::sandbox::Made) -> Self {
        use crate::sandbox::Made;
        match made {
            Made::Window | Made::Resume => Self::Person,
            Made::Relaunch => Self::App,
            Made::Asked { .. } => Self::AChat,
            // A start that does not say how it was made is never the person's own: it waits on
            // no answer of theirs, and runs under the policy in force.
            Made::Unnamed => Self::App,
        }
    }

    /// Whether its start waits while a prerequisite is open: the person's alone.
    pub fn waits(self) -> bool {
        self == Self::Person
    }
}

/// **How a held start ended** ([`Kept::hold`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// Nothing held it: its starter does not wait, or nothing was open.
    AtOnce,
    /// It waited, and an answer released it: it starts now, under the policy in force.
    Released,
    /// It waited, and its caller stopped it before an answer (the person's Cancel). `why` is
    /// what is still open, in the core's words ([`waiting_for`]).
    Stopped { why: String },
    /// It waited as long as its caller gave it, and nothing answered. `why` is what is still
    /// open, in the core's words ([`waiting_for`]).
    RanOut { why: String },
}

/// **What the window showed when the person gave the answer that grants nothing**
/// ([`Kept::defer`]): the answer is kept against it, never against what is there by the time it
/// lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    /// The policy prerequisite, with the digest and the reasons it was shown with.
    Policy {
        digest: String,
        reasons: Vec<Reason>,
    },
}

/// A policy entry as it can be answered: its digest and its reasons.
type Answered = (String, Vec<Reason>);

/// **The digest and reasons pairs the list has held this run**, oldest first, each with whether
/// the answer that grants nothing was given for it ([`Kept::defer`]). An answer is kept only for
/// a pair the list held, so what a link sends that nobody was shown is dropped, and the pairs
/// are bounded ([`ANSWERS_KEPT_AT_MOST`]): past that the oldest goes, and is asked again.
#[derive(Debug, Default)]
struct Answers(VecDeque<(Answered, bool)>);

impl Answers {
    /// Notes `pair` as held this run: whether it was answered.
    fn held(&mut self, pair: Answered) -> bool {
        if let Some((_, answered)) = self.0.iter().find(|(held, _)| *held == pair) {
            return *answered;
        }
        if self.0.len() >= ANSWERS_KEPT_AT_MOST {
            self.0.pop_front();
        }
        self.0.push_back((pair, false));
        false
    }

    /// Answers `pair`, where the list held it this run: whether it did.
    fn answer(&mut self, pair: &Answered) -> bool {
        self.0
            .iter_mut()
            .find(|(held, _)| held == pair)
            .map(|(_, answered)| *answered = true)
            .is_some()
    }

    /// How many pairs are kept.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// **What a held start waits on**, in the core's words: each open entry of `list`, for the
/// project at `root`, by its folder's name. What a held start is told, and says where it stops.
pub fn waiting_for(root: &Path, list: &List) -> String {
    let project = root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    list.entries()
        .iter()
        .filter(|entry| entry.state() == State::Open)
        .map(|entry| match entry {
            Prerequisite::Policy(_) => {
                format!("{project}'s policy needs your answer in the purlis window on this machine")
            }
            #[cfg(test)]
            Prerequisite::StandIn => format!("{project} needs a stand-in's answer"),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// **The most digest and reasons pairs a list keeps for a run** ([`Kept::defer`]): far past
/// the policies one project goes through in a run, so an old answer goes only after many.
const ANSWERS_KEPT_AT_MOST: usize = 32;

/// How long a held start waits at most between looks at whether its caller stopped it.
const LOOKS_EVERY: Duration = Duration::from_millis(100);

#[cfg(test)]
thread_local! {
    /// Whether a test's stand-in for a second kind is open on this thread.
    static STAND_IN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **The prerequisites of the project at `root`**, evaluated now. Only the app, which alone can
/// verify what the person accepted, evaluates them: every other reader asks it.
pub fn evaluate(root: &Path) -> List {
    let mut entries = Vec::new();
    // Evaluated before the policy one, so a test sees the order is the list's, not this.
    #[cfg(test)]
    if STAND_IN.with(std::cell::Cell::get) {
        entries.push(Prerequisite::StandIn);
    }
    entries.extend(policy(root).map(|policy| Prerequisite::Policy(Box::new(policy))));
    List::of(entries)
}

/// **The policy prerequisite of the project at `root`**, where it is open (ADR 0130 §3).
///
/// **Broken** where this machine kept an acceptance that does not hold now. **Waiting** where
/// the committed policy is looser than what was accepted, or than the shipped `watch` where
/// nothing was kept; under Broken, looser than the shipped `watch` alone, never weighed against
/// the record that does not hold. Never alone, with nothing looser than `watch`, is none: the
/// shipped default is no loosening the person must weigh. All of one read
/// ([`enforcement::standing`]).
pub fn policy(root: &Path) -> Option<Policy> {
    let standing = enforcement::standing(root);
    let waiting = if standing.broken {
        standing.loosens_the_shipped
    } else {
        standing.waits()
    };
    let reasons: Vec<Reason> = [
        standing.broken.then_some(Reason::Broken),
        waiting.then_some(Reason::Waiting),
    ]
    .into_iter()
    .flatten()
    .collect();
    if reasons.is_empty() {
        return None;
    }
    Some(Policy {
        state: State::Open,
        reasons,
        digest: standing.digest.clone(),
        needs_new_key: standing.key_foreign,
        after_new_key: standing.broken && standing.new_key,
        standing,
    })
}

/// **The files whose change evaluates the list again**, for the project at `root`: the plane
/// root's `HEAD` (the file itself, its reflog, the branch it names and the packed refs), the
/// settings file and the local settings file under every name purlis reads them by, this
/// machine's policy file, the acceptance record and the never-dispatch list. A seal key is no file: the press that
/// makes one evaluates every project again. Where a file is not there, its change is its
/// coming.
pub fn watched(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS]
        .iter()
        .flat_map(|name| {
            name.spellings()
                .map(|file| root.join(file))
                .collect::<Vec<_>>()
        })
        .collect();
    let git = git_dir(root);
    files.extend(["HEAD", "packed-refs", "logs/HEAD"].map(|file| git.join(file)));
    if let Some(branch) = std::fs::read_to_string(git.join("HEAD"))
        .ok()
        .and_then(|head| {
            head.strip_prefix("ref: ")
                .map(|named| named.trim().to_owned())
        })
        .filter(|named| named.starts_with("refs/") && !named.contains(".."))
    {
        files.push(git.join(branch));
    }
    files.push(PathBuf::from(crate::sandbox::policy::MACHINE_FILE));
    // What the policy prerequisite carries of the never-dispatch list, which its own press
    // writes outside the list.
    files.push(crate::dispatchnever::path(root));
    let record = crate::sandbox::local::path(root);
    if !record.as_os_str().is_empty() {
        files.push(record);
    }
    files
}

/// The plane root's git folder: `.git`, or where a `.git` file points (`gitdir: …`).
fn git_dir(root: &Path) -> PathBuf {
    let dot_git = root.join(".git");
    std::fs::read_to_string(&dot_git)
        .ok()
        .and_then(|text| {
            text.strip_prefix("gitdir: ")
                .map(|pointed| root.join(pointed.trim()))
        })
        .unwrap_or(dot_git)
}

/// How long a list evaluated while its watch lapsed answers the reads after it: a window asking
/// at each render evaluates the policy at most this often.
const LAPSED_FRESH_FOR: Duration = Duration::from_secs(1);

/// Told a project's whole list each time it changes, after it is kept.
pub type Told = Arc<dyn Fn(&List) + Send + Sync + 'static>;

/// **The list the app holds for one project**, and who is told when it changes.
pub struct Kept {
    /// The list as last evaluated, and when; `None` before the first evaluation.
    list: Mutex<Option<(List, Instant)>>,
    /// Woken each time a list is kept: what a held start waits on ([`Self::hold`]).
    kept_again: Condvar,
    /// **The answers that grant nothing given this run** ([`Self::defer`]), each against what
    /// it was shown for, among the pairs the list held. In memory only: a restart is when the
    /// person is asked again.
    answered: Mutex<Answers>,
    /// Held across an evaluation, its keeping and its telling, so two evaluations at once are
    /// told in the order they were kept, and the last one told is the one kept.
    evaluating: Mutex<()>,
    /// The thread holding [`Self::evaluating`], so a read in the handler it tells is known.
    evaluator: Mutex<Option<ThreadId>>,
    /// How long a list evaluated while lapsed answers the reads after it ([`LAPSED_FRESH_FOR`]).
    fresh_for: Duration,
    /// Whether its watch misses a file ([`Self::lapsed`]).
    lapsed: AtomicBool,
    told: Told,
}

impl Kept {
    /// A list not evaluated yet, whose changes are told to `told`. `told` must not evaluate
    /// this list again; it may read it ([`Self::current`]).
    pub fn new(told: Told) -> Self {
        Self {
            list: Mutex::new(None),
            kept_again: Condvar::new(),
            answered: Mutex::new(Answers::default()),
            evaluating: Mutex::new(()),
            evaluator: Mutex::new(None),
            fresh_for: LAPSED_FRESH_FOR,
            lapsed: AtomicBool::new(false),
            told,
        }
    }

    /// This list, a lapsed read reusing an evaluation younger than `fresh_for`.
    #[cfg(test)]
    fn fresh_for(mut self, fresh_for: Duration) -> Self {
        self.fresh_for = fresh_for;
        self
    }

    /// **Evaluates the list for the project at `root` again**, keeps it, and then, where it
    /// differs from the list kept before (or none was), tells it whole.
    pub fn evaluate(&self, root: &Path) -> List {
        let evaluating = self
            .evaluating
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.evaluate_holding(evaluating, root)
    }

    /// [`Self::evaluate`], with its lock `_evaluating` taken.
    fn evaluate_holding(&self, _evaluating: MutexGuard<'_, ()>, root: &Path) -> List {
        *self.evaluator() = Some(std::thread::current().id());
        let now = evaluate(root)
            .deferred_by(&mut self.answered.lock().unwrap_or_else(PoisonError::into_inner));
        let changed = {
            let mut kept = self.list.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = kept.as_ref().map(|(list, _)| list) != Some(&now);
            *kept = Some((now.clone(), Instant::now()));
            changed
        };
        self.kept_again.notify_all();
        // After it is kept, and outside the lock a read takes.
        if changed {
            (self.told)(&now);
        }
        *self.evaluator() = None;
        now
    }

    /// Which thread is evaluating, where one is.
    fn evaluator(&self) -> MutexGuard<'_, Option<ThreadId>> {
        self.evaluator
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The list kept, and when it was evaluated.
    fn kept(&self) -> Option<(List, Instant)> {
        self.list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// **The list as kept**, evaluated first where it never was, and again where its watch
    /// lapsed ([`Self::lapsed`]) and what is kept is older than a second: a window asking at
    /// each render evaluates it no more often than that. A lapsed read while an evaluation is
    /// under way on another thread waits for it, and gets what it kept. A read in the handler
    /// an evaluation tells gets the list that evaluation kept: evaluating again there would
    /// wait on itself.
    pub fn current(&self, root: &Path) -> List {
        match self.kept() {
            None => self.evaluate(root),
            Some((kept, _)) if !self.lapsed.load(Ordering::SeqCst) => kept,
            Some((kept, _)) if *self.evaluator() == Some(std::thread::current().id()) => kept,
            Some(_) => {
                let evaluating = self
                    .evaluating
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                match self.kept() {
                    Some((kept, at)) if at.elapsed() < self.fresh_for => kept,
                    _ => self.evaluate_holding(evaluating, root),
                }
            }
        }
    }

    /// **Whether the watch that evaluates this list again misses a file it is evaluated from**
    /// (a folder the platform would not watch, or no watch at all): then the list kept is no
    /// answer, and a read ([`Self::current`]) evaluates it again, at most once a second, until
    /// the watch is whole.
    pub fn lapsed(&self, lapsed: bool) {
        self.lapsed.store(lapsed, Ordering::SeqCst);
    }

    /// **The person's Accept** ([`enforcement::press`]), then the list evaluated again, whatever
    /// the press came to: a press refused because the file changed since it was shown is a
    /// list that changed too.
    pub fn accept(&self, root: &Path, shown: &str) -> std::io::Result<List> {
        let pressed = enforcement::press(root, shown);
        let list = self.evaluate(root);
        pressed.map(|()| list)
    }

    /// **The answer that grants nothing** ("Work in strict mode" under Broken, "Keep my current
    /// rules" under Waiting; ADR 0130 §6), kept against what was `shown`, then the list
    /// evaluated again: an entry shown so reads [`State::Deferred`] and holds no start, while
    /// one whose digest or reasons moved since is open still. An answer for a pair the list
    /// has not held this run is kept nowhere: a linked page may send it, and nobody was shown
    /// it. It grants nothing and writes nothing: the policy in force is what it was.
    pub fn defer(&self, root: &Path, shown: Shown) -> List {
        let Shown::Policy { digest, reasons } = shown;
        let pair = (digest, reasons);
        if !self
            .answered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .answer(&pair)
        {
            tracing::info!(
                "purlis: an answer for a policy {} did not hold this run was not kept",
                root.display()
            );
        }
        self.evaluate(root)
    }

    /// **Holds a start `starter` makes in the project at `root` while a prerequisite is open**
    /// (ADR 0130 §5), until an answer releases it ([`Self::defer`], [`Self::accept`]).
    ///
    /// A start the person does not make ([`Starter::waits`]) is never held, and neither is one
    /// where nothing is open: both are [`Held::AtOnce`]. A held one is told once, at first, what
    /// it waits on (`waiting`, [`waiting_for`]), and ends [`Held::Released`] once no entry is
    /// open, [`Held::Stopped`] once `stopped` says so, or [`Held::RanOut`] after `within`, where
    /// its caller gives it one. This never changes the policy in force, which it starts under.
    pub fn hold(
        &self,
        root: &Path,
        starter: Starter,
        within: Option<Duration>,
        stopped: &dyn Fn() -> bool,
        waiting: impl FnOnce(&str),
    ) -> Held {
        if !starter.waits() {
            return Held::AtOnce;
        }
        let by = within.map(|within| Instant::now() + within);
        let mut waiting = Some(waiting);
        loop {
            let list = self.current(root);
            if !list.holds() {
                return if waiting.is_some() {
                    Held::AtOnce
                } else {
                    Held::Released
                };
            }
            let why = waiting_for(root, &list);
            if let Some(say) = waiting.take() {
                say(&why);
            }
            if stopped() {
                return Held::Stopped { why };
            }
            let now = Instant::now();
            let left = match by {
                Some(by) if now >= by => return Held::RanOut { why },
                Some(by) => (by - now).min(LOOKS_EVERY),
                None => LOOKS_EVERY,
            };
            // Woken by the next list kept, or after a moment to look at `stopped` again: a list
            // kept between the read above and this wait is read at the next look.
            let kept = self.list.lock().unwrap_or_else(PoisonError::into_inner);
            let _ = self
                .kept_again
                .wait_timeout(kept, left)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// **The person's Make a new key and accept** ([`enforcement::press_with_new_key`]), then
    /// this list evaluated again. A new key breaks every other project accepted on this
    /// machine: the app evaluates each of theirs again too.
    pub fn accept_with_new_key(&self, root: &Path, shown: &str) -> std::io::Result<List> {
        let pressed = enforcement::press_with_new_key(root, shown);
        let list = self.evaluate(root);
        pressed.map(|()| list)
    }
}

#[cfg(test)]
mod tests;
