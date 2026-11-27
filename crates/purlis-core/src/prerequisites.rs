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
//! machine's policy file and the acceptance record), on a new seal key, and after every answer
//! ([`Kept::accept`], [`Kept::accept_with_new_key`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
}

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
/// machine's policy file and the acceptance record. A seal key is no file: the press that
/// makes one evaluates every project again. Where a file is not there, its change is its
/// coming.
pub fn watched(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = [crate::names::PLANE_MANIFEST, crate::names::LOCAL_SETTINGS]
        .iter()
        .flat_map(|name| name.spellings().map(|file| root.join(file)).collect::<Vec<_>>())
        .collect();
    let git = git_dir(root);
    files.extend(["HEAD", "packed-refs", "logs/HEAD"].map(|file| git.join(file)));
    if let Some(branch) = std::fs::read_to_string(git.join("HEAD"))
        .ok()
        .and_then(|head| head.strip_prefix("ref: ").map(|named| named.trim().to_owned()))
        .filter(|named| named.starts_with("refs/") && !named.contains(".."))
    {
        files.push(git.join(branch));
    }
    files.push(PathBuf::from(crate::sandbox::policy::MACHINE_FILE));
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

/// Told a project's whole list each time it changes, after it is kept.
pub type Told = Arc<dyn Fn(&List) + Send + Sync + 'static>;

/// **The list the app holds for one project**, and who is told when it changes.
pub struct Kept {
    /// The list as last evaluated; `None` before the first evaluation.
    list: Mutex<Option<List>>,
    /// Held across an evaluation, its keeping and its telling, so two evaluations at once are
    /// told in the order they were kept, and the last one told is the one kept.
    evaluating: Mutex<()>,
    told: Told,
}

impl Kept {
    /// A list not evaluated yet, whose changes are told to `told`. `told` must not evaluate
    /// this list again; it may read it ([`Self::current`]).
    pub fn new(told: Told) -> Self {
        Self {
            list: Mutex::new(None),
            evaluating: Mutex::new(()),
            told,
        }
    }

    /// **Evaluates the list for the project at `root` again**, keeps it, and then, where it
    /// differs from the list kept before (or none was), tells it whole.
    pub fn evaluate(&self, root: &Path) -> List {
        let _evaluating = self
            .evaluating
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let now = evaluate(root);
        let changed = {
            let mut kept = self.list.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = kept.as_ref() != Some(&now);
            *kept = Some(now.clone());
            changed
        };
        // After it is kept, and outside the lock a read takes.
        if changed {
            (self.told)(&now);
        }
        now
    }

    /// **The list as kept**, evaluated first where it never was.
    pub fn current(&self, root: &Path) -> List {
        let kept = self
            .list
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        kept.unwrap_or_else(|| self.evaluate(root))
    }

    /// **The person's Accept** ([`enforcement::press`]), then the list evaluated again, whatever
    /// the press came to: a press refused because the file changed since it was shown is a
    /// list that changed too.
    pub fn accept(&self, root: &Path, shown: &str) -> std::io::Result<List> {
        let pressed = enforcement::press(root, shown);
        let list = self.evaluate(root);
        pressed.map(|()| list)
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
