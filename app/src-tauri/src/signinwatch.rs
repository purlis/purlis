// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
//! **What watches a profile's sign-in** in a shell tab (#2035, #2049), on this machine or on a
//! pool's runner: one rule for both (#4009, #4223). The harness is asked again until it says the
//! profile is signed in or the person leaves that sign-in, when its Ask goes. A harness that
//! cannot tell says nothing of a sign-in the person may still be finishing, so its Ask stays.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How often a profile is asked about again after its Sign in, so its Ask leaves the Inbox once
/// it is signed in; how long at that pace; and how often after that, for a shell left open.
const WATCHED_EVERY: Duration = Duration::from_secs(5);
const WATCHED_FOR: Duration = Duration::from_secs(15 * 60);
const WATCHED_LATER_EVERY: Duration = Duration::from_secs(60);

/// What a watcher watches: the open project that pressed Sign in, by the address of its `Held`
/// (which the watcher's own `Weak` keeps from being dealt again, so a reopened project has a
/// watcher of its own), the pool, or `None` for this machine, and the profile.
type Watched = (usize, Option<String>, String);

/// The watchers asking now, each with the shell tabs its Sign ins typed into: one each, however
/// often Sign in is pressed.
static WATCHED: Mutex<Option<HashMap<Watched, Vec<u32>>>> = Mutex::new(None);

/// Tells the windows of the open projects at some roots that their Asks changed.
pub type Told = Box<dyn FnOnce(&[PathBuf]) + Send>;

/// The project a watcher was started for, as the watcher needs it.
pub struct Project {
    /// [`Watched`]'s first part.
    pub id: usize,
    /// Its root.
    pub root: PathBuf,
    /// Whether shell tab `session` is still open in it.
    pub shell_open: Box<dyn Fn(u32) -> bool + Send>,
    /// Whether that project is still open: one closed is not the person leaving a sign-in.
    pub project_open: Box<dyn Fn() -> bool + Send>,
    /// [`Told`]: every project whose wait went, not only this one (#4205).
    pub told: Told,
}

/// The sign-in a watcher watches.
pub struct SignIn {
    /// The pool it is on, or `None` for this machine.
    pub pool: Option<String>,
    /// The profile's name.
    pub name: String,
    /// Whether the profile is still there to sign in: on this machine, and on the pool. One taken
    /// away ends the watch, so no look keeps it on a runner again.
    pub there: Box<dyn Fn() -> bool + Send>,
    /// Whether the harness says the profile is signed in now. Asked on the watcher's thread.
    pub signed_in: Box<dyn FnMut() -> bool + Send>,
    /// Lets the waits for it go: the projects whose wait went, as far as it can say.
    pub let_go: Box<dyn Fn(Whose<'_>) -> Vec<PathBuf> + Send>,
}

/// Whose waits a sign-in lets go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose<'a> {
    /// Every project's: the profile is signed in, whoever asked.
    Everywhere,
    /// The project's at this root, raised before the moment its shell was last seen open: only
    /// its person left, and a start refused since then still waits (#4203).
    InTheProject(&'a Path, Instant),
}

/// What ended a watcher's asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watch {
    /// The harness says the profile is signed in.
    SignedIn,
    /// The person closed every shell it was signing in from, and it is not signed in.
    ShellsClosed,
}

/// Whose Asks an ended watch lets go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsksGo {
    /// Every project's: the profile is signed in, whoever asked.
    Everywhere,
    /// The project's that pressed Sign in: only its person left.
    InTheProject,
    /// None: a project closing leaves its Ask for when it opens again.
    None,
}

fn asks_go(ended: Watch, project_open: bool) -> AsksGo {
    match ended {
        Watch::SignedIn => AsksGo::Everywhere,
        Watch::ShellsClosed if project_open => AsksGo::InTheProject,
        Watch::ShellsClosed => AsksGo::None,
    }
}

/// **Asks again every few seconds whether `sign_in` is done**, after its Sign in in shell tab
/// `session`, until the harness says so or the person closes the shell, when its Ask goes
/// ([`asks_go`]). On a thread of its own, one per open project, pool and profile: a second Sign
/// in while one asks starts none, and adds its shell.
pub fn watch(sign_in: SignIn, session: u32, project: Project) {
    let key = (project.id, sign_in.pool.clone(), sign_in.name.clone());
    if !watched_now(|watched| claimed(watched, &key, session)) {
        return;
    }
    std::thread::spawn(move || {
        let Project {
            root,
            shell_open,
            project_open,
            told,
            ..
        } = project;
        let SignIn {
            there,
            mut signed_in,
            let_go,
            ..
        } = sign_in;
        let (ended, last_open) = watched_until(
            Pace {
                every: WATCHED_EVERY,
                for_: WATCHED_FOR,
                later_every: WATCHED_LATER_EVERY,
            },
            || there() && signed_in(),
            || {
                // Asked with no lock held: the answer reads the project, and may be the last to
                // hold it.
                let shells = watched_now(|watched| watched.get(&key).cloned().unwrap_or_default());
                let closed = closed_of(shells, there(), &shell_open);
                watched_now(|watched| shells_left(watched, &key, &closed))
            },
        );
        // A watcher whose shells closed gave up its claim as it found them closed, so a Sign in
        // after that has a watcher of its own, which this one must not take away.
        if ended != Watch::ShellsClosed {
            watched_now(|watched| watched.remove(&key));
        }
        let open = project_open();
        let went = match asks_go(ended, open) {
            AsksGo::Everywhere => let_go(Whose::Everywhere),
            AsksGo::InTheProject => let_go(Whose::InTheProject(&root, last_open)),
            AsksGo::None => Vec::new(),
        };
        told(&to_tell(went, &root, open));
    });
}

/// Which of `shells` count as closed: all, with the profile no longer `there`.
fn closed_of(shells: Vec<u32>, there: bool, shell_open: impl Fn(u32) -> bool) -> Vec<u32> {
    shells
        .into_iter()
        .filter(|shell| !there || !shell_open(*shell))
        .collect()
}

/// Whose windows an ended watch tells: every project whose wait `went` (#4205), and its own while
/// open, which may be drawing a wait another let go. A closed one is told only through a wait
/// that went, which it cannot have unless it reopened.
fn to_tell(mut went: Vec<PathBuf>, root: &Path, project_open: bool) -> Vec<PathBuf> {
    if project_open && !went.iter().any(|one| one == root) {
        went.push(root.to_path_buf());
    }
    went
}

fn watched_now<T>(with: impl FnOnce(&mut HashMap<Watched, Vec<u32>>) -> T) -> T {
    with(
        WATCHED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(HashMap::new),
    )
}

/// Whether `key` is claimed for a watcher now, in `watched`: only where none watches it. Where
/// one does, `session` is one more shell it watches.
fn claimed(watched: &mut HashMap<Watched, Vec<u32>>, key: &Watched, session: u32) -> bool {
    match watched.get_mut(key) {
        Some(shells) => {
            shells.push(session);
            false
        }
        None => {
            watched.insert(key.clone(), vec![session]);
            true
        }
    }
}

/// Whether any of `key`'s shells is still open, with those in `closed` dropped. With none left
/// its claim goes in the same step, so a Sign in now starts a watcher rather than joining this
/// one.
fn shells_left(watched: &mut HashMap<Watched, Vec<u32>>, key: &Watched, closed: &[u32]) -> bool {
    let Some(shells) = watched.get_mut(key) else {
        return false;
    };
    shells.retain(|session| !closed.contains(session));
    if shells.is_empty() {
        watched.remove(key);
        return false;
    }
    true
}

/// How a watcher paces its asking.
#[derive(Debug, Clone, Copy)]
struct Pace {
    /// How often it looks at the shells, and asks the harness at first.
    every: Duration,
    /// How long it asks at that pace.
    for_: Duration,
    /// How often it asks the harness after that.
    later_every: Duration,
}

/// Asks until the harness says signed in or `shells_open` says the person has left. The harness
/// is asked first, so a sign-in finished just before the shell closed counts as one. No time
/// ends it, only slows the asking: a shell is closed sooner or later, and an Ask left with
/// nothing watching could no longer go by itself. How it ended, and when a shell was last seen
/// open: the moment before that look, or as the watch began.
fn watched_until(
    pace: Pace,
    mut signed_in: impl FnMut() -> bool,
    mut shells_open: impl FnMut() -> bool,
) -> (Watch, Instant) {
    let started = Instant::now();
    let mut last_open = started;
    let mut next_ask = started;
    loop {
        std::thread::sleep(pace.every);
        let now = Instant::now();
        if now >= next_ask {
            if signed_in() {
                return (Watch::SignedIn, last_open);
            }
            if now.duration_since(started) >= pace.for_ {
                next_ask = now + pace.later_every;
            }
        }
        let looked = Instant::now();
        if !shells_open() {
            return (Watch::ShellsClosed, last_open);
        }
        last_open = looked;
    }
}

#[cfg(test)]
mod tests;
