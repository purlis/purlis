// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each open project's prerequisites, as the app holds them** (ADR 0130 §2): the list
//! `purlis_core::prerequisites` evaluates, kept per project on its [`crate::planes::Held`], and
//! the one payload every reader gets it in ([`Prerequisites`]): the [`EVENT`] the window
//! subscribes to, the read any reader asks ([`project_prerequisites`]), and the hook socket's
//! answer to a chat's `purlis` ([`purlis_core::hookwire::Ask::Prerequisites`]).
//!
//! **The event goes to every window**, not only the project's: a project not in front shows a
//! dot on its tab, and answering in one window closes the modal in every window. Each window
//! filters on the plane, as it does `plane-changed`.
//!
//! **What evaluates it again** ([`Watch`]): the project's open, a change to any file the core
//! names ([`purlis_core::prerequisites::watched`]), every answer, which the policy commands make
//! through the held list (`policypress`), and, for every other project held, any Accept, which
//! may make or replace the seal key. Where a folder is not watched, each ask too.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

use purlis_core::prerequisites as core;

use crate::planes::{PlaneId, Planes};
use crate::policypress::PolicyState;

/// The event every window is sent when a project's list changes.
pub const EVENT: &str = "prerequisites-changed";

/// **A project's whole list of prerequisites, in the core's order**: what [`EVENT`] carries,
/// what [`project_prerequisites`] answers, and what the hook socket hands a chat's `purlis`.
/// Never a delta.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Prerequisites {
    /// The project it is of.
    pub plane: PlaneId,
    /// Each prerequisite, open or deferred, in the core's order: none where the project needs
    /// nothing of the person.
    pub list: Vec<Prerequisite>,
}

/// **One prerequisite**, by its kind.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Prerequisite {
    /// The project's `[policy]` is not accepted on this machine (ADR 0130 §3).
    Policy(PolicyPrerequisite),
}

/// Where a prerequisite stands this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PrerequisiteState {
    /// Unanswered: it waits for the person.
    Open,
    /// Answered for this run with the answer that grants nothing.
    Deferred,
}

/// Why the policy prerequisite is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    /// An acceptance was kept on this machine, and what is here now is not it: the project's
    /// chats work in strict mode until the person answers.
    Broken,
    /// The committed policy loosens what this machine accepted, or the shipped `watch` where
    /// nothing holds.
    Waiting,
}

/// **The policy prerequisite**: one read of the project's policy, so the reasons, the rows and
/// the digest the window presses are all of the same file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyPrerequisite {
    /// Open, or deferred for this run.
    pub state: PrerequisiteState,
    /// Broken, Waiting, or both, Broken first.
    pub reasons: Vec<PolicyReason>,
    /// The digest an Accept must name.
    pub digest: String,
    /// Whether Accept must first replace a seal key another program made (#7714): the press is
    /// Make a new key and accept.
    pub needs_new_key: bool,
    /// Whether the acceptance broke because purlis replaced its seal key on this machine
    /// (#7714), never because something edited it.
    pub after_new_key: bool,
    /// The project's policy as it stands, from the same read: the waiting rows among it.
    pub policy: PolicyState,
}

impl Prerequisites {
    /// The list `list` of `plane`, as every reader is given it.
    pub fn of(plane: PlaneId, list: &core::List) -> Self {
        Self {
            plane,
            list: list.entries().iter().map(Prerequisite::of).collect(),
        }
    }
}

impl Prerequisite {
    fn of(entry: &core::Prerequisite) -> Self {
        match entry {
            core::Prerequisite::Policy(policy) => Self::Policy(PolicyPrerequisite {
                state: match policy.state {
                    core::State::Open => PrerequisiteState::Open,
                    core::State::Deferred => PrerequisiteState::Deferred,
                },
                reasons: policy
                    .reasons
                    .iter()
                    .map(|reason| match reason {
                        core::Reason::Broken => PolicyReason::Broken,
                        core::Reason::Waiting => PolicyReason::Waiting,
                    })
                    .collect(),
                digest: policy.digest.clone(),
                needs_new_key: policy.needs_new_key,
                after_new_key: policy.after_new_key,
                policy: crate::policypress::state_of_standing(policy.standing.clone()),
            }),
        }
    }
}

/// Told each project's whole list each time it changes.
pub type Teller = Arc<dyn Fn(Prerequisites) + Send + Sync + 'static>;

/// **What tells every window of `app`** each project's list, whichever project it shows: each
/// window filters on the plane.
pub fn to_every_window<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Teller {
    use tauri::Emitter;
    Arc::new(move |told: Prerequisites| {
        let _ = app.emit(EVENT, told);
    })
}

/// **The project's prerequisites now**: what a window that opens, or one that missed an event,
/// asks. Evaluated first where it never was. Off the window's thread.
#[tauri::command]
#[specta::specta]
pub async fn project_prerequisites(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Prerequisites, String> {
    let held = planes.held(&plane)?;
    let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
    crate::off_the_window(READING, move || {
        Ok(Prerequisites::of(plane, &kept.current(&root)))
    })
    .await
}

/// **What the window showed when the person gave the answer that grants nothing**: the entry as
/// it was drawn, so the answer is kept against what the person saw, never against what is there
/// by the time it lands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShownPrerequisite {
    /// The policy prerequisite, with the digest and the reasons it was shown with.
    Policy {
        digest: String,
        reasons: Vec<PolicyReason>,
    },
}

impl ShownPrerequisite {
    fn into_core(self) -> core::Shown {
        match self {
            Self::Policy { digest, reasons } => core::Shown::Policy {
                digest,
                reasons: reasons
                    .into_iter()
                    .map(|reason| match reason {
                        PolicyReason::Broken => core::Reason::Broken,
                        PolicyReason::Waiting => core::Reason::Waiting,
                    })
                    .collect(),
            },
        }
    }
}

/// **The answer that grants nothing**: "Work in strict mode" under Broken, "Keep my current
/// rules" under Waiting (ADR 0130 §6). Kept in the app's memory for this run, against what was
/// `shown`, and never written anywhere; the entry then reads deferred, a start the person made
/// in the project and held on it starts, under the policy in force, and every window is told the
/// new list. It grants nothing, so a linked page may give it too; Accept stays the window's.
#[tauri::command]
#[specta::specta]
pub async fn defer_project_prerequisite(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: ShownPrerequisite,
) -> Result<Prerequisites, String> {
    let held = planes.held(&plane)?;
    let (root, kept) = (held.root().to_path_buf(), held.prerequisites());
    crate::off_the_window(READING, move || Ok(deferred(plane, &root, &kept, shown))).await
}

/// [`defer_project_prerequisite`] on the project `plane` at `root`, whose list `kept` holds.
fn deferred(
    plane: PlaneId,
    root: &Path,
    kept: &core::Kept,
    shown: ShownPrerequisite,
) -> Prerequisites {
    Prerequisites::of(plane, &kept.defer(root, shown.into_core()))
}

/// **Holds a start the person makes (`made`) in the project `held` holds, while a prerequisite
/// is open** (ADR 0130 §5), until they answer in the window. Every other start passes at once,
/// under the policy in force: the core judges which ([`core::Starter::of`]). Made before the
/// start is readied, so a start an Accept released is readied under what was accepted. The
/// person's Cancel on a start in the project (`cancel`, [`crate::hostedsessions`]) ends the
/// wait, and the start is refused, saying what is still open.
pub(crate) fn hold_start(
    held: &crate::planes::Held,
    made: &purlis_core::sandbox::Made,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let root = held.root();
    match held.prerequisites().hold(
        root,
        core::Starter::of(made),
        None,
        &|| cancel.load(Ordering::SeqCst),
        |why| {
            tracing::info!(
                "purlis: a chat started in {} waits for the person: {why}",
                root.display()
            );
        },
    ) {
        core::Held::AtOnce | core::Held::Released => Ok(()),
        core::Held::Stopped { why } | core::Held::RanOut { why } => {
            Err(format!("purlis did not start the chat: {why}."))
        }
    }
}

/// [`hold_start`], ended by the person's Cancel on any start in the project: for a start of the
/// window's that has no Cancel of its own.
pub(crate) fn hold_a_start(
    held: &crate::planes::Held,
    made: &purlis_core::sandbox::Made,
) -> Result<(), String> {
    hold_start(
        held,
        made,
        &crate::hostedsessions::a_cancellable_start(held.root()),
    )
}

/// What the commands here name their work, off the window's thread.
const READING: &str = "reading what the project needs from you";

/// How long a burst of writes to the watched files is folded before the list is evaluated.
const QUIET_FOR: Duration = Duration::from_millis(150);

/// The longest a burst is folded, under writes without pause.
const LONGEST: Duration = Duration::from_secs(1);

/// The most paths a burst holds before it is everything.
const MOST_PATHS: usize = 256;

/// The watcher a project's list is watched on: the platform's own. In this crate's tests on
/// macOS, notify's poller, as the plane watch's ([`crate::planewatch::Platform`]): FSEvents gives
/// no bound on when it delivers.
#[cfg(not(all(test, target_os = "macos")))]
pub type Platform = notify::RecommendedWatcher;
#[cfg(all(test, target_os = "macos"))]
pub type Platform = notify::PollWatcher;

/// **The watch on what a project's list is evaluated from** ([`core::watched`]). Each folder
/// holding one of the files is watched, non-recursively; where a folder is not there, the
/// nearest one above it is, so its coming is heard. After every burst that names one of the
/// files, or a folder on the way to one, or that is everything, the folders are listed and
/// followed again (a checkout names another branch's file) and then the list is evaluated again.
/// A folder the platform would not watch is said (`lapsed`), so the list is read at each ask
/// until it is watched. Dropping it stops it.
pub struct Watch<W: notify::Watcher = Platform> {
    _watcher: Arc<Mutex<W>>,
}

impl Watch {
    /// Watches what the list of the project at `root` is evaluated from, and calls
    /// `evaluate` after each change, and once at the start: the project's open. Tells `lapsed`,
    /// at the start and before each evaluation, whether a folder it needs is not watched.
    pub fn start(
        root: &Path,
        evaluate: impl Fn() + Send + 'static,
        lapsed: impl Fn(bool) + Send + 'static,
    ) -> notify::Result<Self> {
        let root = root.to_path_buf();
        Self::start_on(
            move || core::watched(&root),
            evaluate,
            lapsed,
            notify::Config::default(),
        )
    }
}

impl<W: notify::Watcher + Send + 'static> Watch<W> {
    /// [`Watch::start`], of the files `files` lists, on a watcher of the caller's choosing,
    /// made with `config`.
    fn start_on(
        files: impl Fn() -> Vec<PathBuf> + Send + 'static,
        evaluate: impl Fn() + Send + 'static,
        lapsed: impl Fn(bool) + Send + 'static,
        config: notify::Config,
    ) -> notify::Result<Self> {
        let (sent, events) = mpsc::channel();
        let mut watcher = W::new(crate::watchset::sender(sent), config)?;
        let mut watched = HashSet::new();
        let mut named = files();
        // A folder the platform would not watch is said, so the list is read at each ask,
        // and tried again after the next burst.
        lapsed(!crate::watchset::follow(&mut watcher, &mut watched, folders_of(&named)).is_empty());
        let watcher = Arc::new(Mutex::new(watcher));
        let following = Arc::downgrade(&watcher);
        std::thread::Builder::new()
            .name("purlis-prerequisites-watch".into())
            .spawn(move || {
                evaluate();
                for burst in
                    crate::watchset::trailing_bursts(events, QUIET_FOR, LONGEST, MOST_PATHS)
                {
                    if !(burst.everything
                        || burst
                            .paths
                            .iter()
                            .any(|path| concerns(&named, &watched, &burst, path)))
                    {
                        continue;
                    }
                    let Some(watcher) = following.upgrade() else {
                        return;
                    };
                    // Followed again before it is evaluated: a file the change brought (a
                    // checkout names another branch's) is watched before it is read, so a write
                    // to it after the read is heard.
                    named = files();
                    let unwatched = {
                        let mut watcher = watcher.lock().unwrap_or_else(PoisonError::into_inner);
                        // A followed folder that went is forgotten, so one made again in its
                        // place is followed again (#7885's review, F7).
                        for folder in gone(&watched, &burst) {
                            crate::watchset::forget(&mut *watcher, &mut watched, &folder);
                        }
                        crate::watchset::follow(&mut *watcher, &mut watched, folders_of(&named))
                    };
                    lapsed(!unwatched.is_empty());
                    evaluate();
                }
            })
            .map_err(notify::Error::io)?;
        Ok(Self { _watcher: watcher })
    }
}

/// Whether a change at `path` concerns one of `files`: it is one, or a folder on the way to
/// one, made, removed or renamed. A folder the watch follows (`watched`) that is still there
/// concerns nothing: its own change is only that something in it changed, and a change to one
/// of the files names the file (a poller sees a folder's time move with any write beside the
/// file). One that went ([`gone`]) concerns. The platform may name a path by its real spelling
/// (`/private/var` for `/var` on macOS), so each file is asked by both; `watched` and the paths
/// a burst names are both real.
fn concerns(
    files: &[PathBuf],
    watched: &HashSet<PathBuf>,
    burst: &crate::watchset::Burst,
    path: &Path,
) -> bool {
    if watched.contains(path) && !went(burst, path) {
        return false;
    }
    files
        .iter()
        .any(|file| file.starts_with(path) || resolved(file).starts_with(path))
}

/// The folders the watch follows (`watched`) that `burst` says went: a removal named it, or it
/// is no longer a folder.
fn gone(watched: &HashSet<PathBuf>, burst: &crate::watchset::Burst) -> Vec<PathBuf> {
    burst
        .paths
        .iter()
        .filter(|path| watched.contains(*path) && went(burst, path))
        .cloned()
        .collect()
}

/// Whether the folder at `path` went: `burst` names its removal, or it is no longer a folder.
fn went(burst: &crate::watchset::Burst, path: &Path) -> bool {
    burst.removed.contains(path) || !path.is_dir()
}

/// `file` with the nearest folder above it that is there resolved, links and all.
fn resolved(file: &Path) -> PathBuf {
    file.ancestors()
        .skip(1)
        .find_map(|folder| {
            let real = std::fs::canonicalize(folder).ok()?;
            let rest = file.strip_prefix(folder).ok()?;
            Some(real.join(rest))
        })
        .unwrap_or_else(|| file.to_path_buf())
}

/// The folders that hold `files`, each the nearest one that is there, by its real path: a
/// non-recursive watch on macOS keeps only the events of the folder as the platform spells it.
fn folders_of(files: &[PathBuf]) -> HashSet<PathBuf> {
    files
        .iter()
        .filter_map(|file| file.ancestors().skip(1).find(|folder| folder.is_dir()))
        .map(|folder| std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf()))
        .collect()
}

#[cfg(test)]
mod tests;
