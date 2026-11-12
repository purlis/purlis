// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Tells the window when a plane changes on disk under it (charter-app#264).
//!
//! `charter ws todo done` in a terminal deletes a todo's file, and the window's Todos panel
//! went on drawing it for hours: the window read a workspace once, when it was focused, and
//! nothing ever said the plane had moved. The plane is a directory that the CLI, other chats
//! and the operator's editor all write, so a cache of it can only be kept honest by being told.
//!
//! # What is watched, and why it is not the whole plane
//!
//! The directories the panels and the sidebar read, each **non-recursively**:
//!
//! - the plane root (`charter.toml`, which names the default persona, and `workspaces/`
//!   itself appearing);
//! - `workspaces/`, so a workspace made or deleted elsewhere is seen and watched in turn;
//! - each `workspaces/<ws>/` (`workspace.json`, a clone arriving, `todos/` being created);
//! - each plain folder in a workspace, one that is not a clone, a store or dotted and holds no
//!   more than [`PLAIN_FOLDER_MOST`] entries, so a `git init` in it or a `.git` moved into it
//!   is heard as the workspace's change (#1160);
//! - each `workspaces/<ws>/todos/`, which is the bug, and its `archive/`, the done todos
//!   (ADR 0106);
//! - each `workspaces/<ws>/memory/` and `workspaces/<ws>/sessions/`, and the root's
//!   `sessions/`: the Memory and Sessions panels' stores, so a memory an agent saves or a
//!   session record a smart close writes reaches the panel without some other change having
//!   to happen first (FD-10);
//! - `personas/`, and each `personas/<name>/`, whose `persona.md` a chat reads at its start,
//!   and each `personas/<name>/memory/`, which the Personas panel counts;
//! - `.claude/` and `.claude/agents/`, the harness settings and sub-agents a chat reads at its
//!   start — with the root's `CLAUDE.md`, what the window marks a chat for when it changes under
//!   it (charter#369, [`purlis_core::instructions`]).
//!
//! Not the plane recursively, because a workspace holds its clones: a recursive inotify watch
//! would put one watch on every directory of every clone's `node_modules/` and `target/`, and
//! run the machine out of watches. The set is re-read after every batch, so a workspace made
//! in a terminal is watched from then on.
//!
//! # One stream on macOS (#5499)
//!
//! On macOS the set is what is REPORTED, not what is watched ([`Mode`]). FSEvents streams a
//! subtree as cheaply as a folder, and every folder added to a stream restarts it, so a watch
//! of each folder cost O(F²) to set up: 376.8 s at 1,000 workspaces (#5486). There the watch
//! is one recursive stream on the root, and the stream's events are kept to the set
//! ([`Listened`]) in-process, before they are folded, so a clone's build is not a burst.
//!
//! # One event per burst
//!
//! The platform's events are folded into bursts ([`crate::watchset::trailing_bursts`]) — `git pull`
//! landing ten todos, an editor's write-then-rename. A burst closes once [`QUIET_FOR`] passes with
//! no event, and at most [`LONGEST`] after it began, so agents writing without pause are told about
//! once a second (#4741). A burst is one `plane-changed` carrying the plane
//! **and what changed in it**: each path, and what it is part of
//! ([`purlis_core::planechange`]) — a todo of `alpha`, a memory of `steward`, the root's
//! session records. A panel reads again only on a change its answer is made of (FD-10), so a
//! memory an agent saves no longer makes the sidebar list every workspace's todos once more.
//!
//! A burst this cannot place — a path outside the plane, an event notify could not name a path
//! for, a rescan, more paths than a burst holds — is told as `changes: null`, "anything may have moved", and every reader
//! reads again, as each one did before there were kinds. **At most one such burst is told each
//! [`UNKNOWN_EVERY`]** for a plane: under a build's load FSEvents reports a rescan about once a
//! second, and the ones inside that gap of the one told are told once at its end.
//!
//! **An access is not a change.** notify's inotify backend watches `IN_OPEN`, and reading a
//! todo opens it — so a window that re-read on every event would re-read because it re-read,
//! forever. [`crate::watchset::matters`] is where that loop is cut.
//!
//! **A file made and removed inside one burst is still told** (#1139). The debouncer this used
//! to fold through took a removal that arrived while the creation was still queued as the file
//! never having been there, and told neither — but a read the window made on another change
//! may already have drawn that file.
//!
//! # When a folder cannot be watched (#756, #5499)
//!
//! The sidebar's model and the panels answer from what the watch told them (FD-10b), so a
//! change nothing tells is one the window never draws. A watch the platform refused (inotify
//! out of watches) is such a folder, and the watch **looks at that folder itself**
//! ([`Windows`]), and at no other:
//!
//! - **while a window is shown**, every [`SWEEP_EVERY`], it reads the entries of each folder
//!   the platform would not watch (one `read_dir` and one `lstat` each, no file is opened) and
//!   tells what differs from its last look;
//! - **when a window is focused**, it looks at once, so coming back to the window after a
//!   terminal wrote the project shows the terminal's change. A storm of focus changes is one
//!   look per [`QUIET_FOR`];
//! - **while every window is hidden or minimised** it does not look at all (SC-18, #1392); the
//!   events it hears meanwhile are kept, and the first look after a focus compares against the
//!   look before the windows went;
//! - each change the look had to tell is counted for the doctor's `file events` row
//!   ([`Windows::missed`]).
//!
//! A look tells the changed entries as a burst of their own, placed as the events' are, so the
//! readers that hold the changed answers read again and nothing else does. A folder entry is
//! compared by its being there, not its time: a clone's own writes move its folder's time, and
//! no non-recursive watch tells those.
//!
//! **A folder the platform watches is never looked at.** Until #5499 every watched folder was,
//! every five seconds, for FSEvents' late events (#577: 4 to 15 seconds late on a busy Mac,
//! with no flag): at 1,000 workspaces that was some 200,000 `lstat`s a look, and an idle
//! window's CPU was never flat. A late event is late, not lost. A stream that does lose events
//! says so (`MustScanSubDirs`, a dropped-events flag, inotify's overflow): that burst is
//! everything, and every reader reads again once — the one full reconcile.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, LazyLock, Mutex, OnceLock, PoisonError, Weak, mpsc};
use std::time::{Duration, Instant, SystemTime};

use purlis_core::planechange::{self, Answer, Change, Kind};
use purlis_core::workspaces::Plane;

use crate::planes::PlaneId;
use crate::watchset::Burst;

/// The event the window is sent.
pub(crate) const CHANGED: &str = "plane-changed";

/// What `plane-changed` carries: which plane moved, and what moved in it. Every window filters
/// on the plane, as it filters `chat-moved`, because the app holds several planes and emits on
/// the app.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PlaneChanged {
    pub plane: PlaneId,
    /// Each changed path and what it is part of, or `null` when what changed is not known —
    /// a batch this could not place, or auto-save having moved the tree — and every reader
    /// reads again. Auto-save that left the tree as it was says so as one change of kind
    /// `git`, with no path.
    pub changes: Option<Vec<PlaneChange>>,
    /// The answers these changes concern, each once, or `null` — every answer — when what
    /// changed is not known. A reader names the answer it holds and reads again only when it
    /// is here: which answer a change concerns is the core's question
    /// ([`purlis_core::planechange::answers`]), never the window's.
    pub answers: Option<Vec<PlaneAnswer>>,
}

impl PlaneChanged {
    /// What the window is told about `plane` when `changes` moved in it.
    pub fn of(plane: PlaneId, changes: What) -> Self {
        let answers = planechange::answers(changes.as_deref())
            .map(|answers| answers.into_iter().map(PlaneAnswer::from).collect());
        Self {
            plane,
            changes: changes.map(|changes| changes.into_iter().map(PlaneChange::from).collect()),
            answers,
        }
    }
}

/// One answer the window reads from the plane ([`purlis_core::planechange::Answer`],
/// mirrored for the bindings).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "answer", rename_all = "camelCase")]
pub enum PlaneAnswer {
    /// `plane_sidebar`.
    Sidebar,
    /// `workspace_panels` for `workspace`, or for every workspace when it is `null`.
    Panels { workspace: Option<String> },
    /// `plane_root_panels`.
    RootPanels,
    /// `chats_plane_updated`: the instructions a chat read at its start.
    Instructions,
    /// `curation_offers`: the curation actions offered on each subject.
    Curations,
    /// What the project has on, and its theme.
    Settings,
    /// The git standings: the alerts and the Saving rows.
    Git,
    /// The view tabs.
    Views,
}

impl From<Answer> for PlaneAnswer {
    fn from(answer: Answer) -> Self {
        match answer {
            Answer::Sidebar => Self::Sidebar,
            Answer::Panels { workspace } => Self::Panels { workspace },
            Answer::RootPanels => Self::RootPanels,
            Answer::Instructions => Self::Instructions,
            Answer::Curations => Self::Curations,
            Answer::Settings => Self::Settings,
            Answer::Git => Self::Git,
            Answer::Views => Self::Views,
        }
    }
}

/// What one changed path is part of, as the window's readers divide the plane
/// ([`purlis_core::planechange::Kind`], which this mirrors for the bindings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Project,
    Harness,
    Workspace,
    Todos,
    Artifacts,
    Memory,
    Sessions,
    Persona,
    Skills,
    Git,
    Chats,
}

/// One changed path ([`purlis_core::planechange::Change`], mirrored for the bindings).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PlaneChange {
    pub kind: ChangeKind,
    /// The workspace it is in, where it is in one.
    pub workspace: Option<String>,
    /// The persona it belongs to, where it belongs to one (`_shared` for the shared store).
    pub persona: Option<String>,
    /// Relative to the plane root, `/` between its parts.
    pub path: String,
}

impl From<Change> for PlaneChange {
    fn from(change: Change) -> Self {
        Self {
            kind: match change.kind {
                Kind::Project => ChangeKind::Project,
                Kind::Harness => ChangeKind::Harness,
                Kind::Workspace => ChangeKind::Workspace,
                Kind::Todos => ChangeKind::Todos,
                Kind::Artifacts => ChangeKind::Artifacts,
                Kind::Memory => ChangeKind::Memory,
                Kind::Sessions => ChangeKind::Sessions,
                Kind::Persona => ChangeKind::Persona,
                Kind::Skills => ChangeKind::Skills,
                Kind::Git => ChangeKind::Git,
                Kind::Chats => ChangeKind::Chats,
            },
            workspace: change.workspace,
            persona: change.persona,
            path: change.path,
        }
    }
}

/// What a plane change says moved: the changes, or `None` for "not known, read everything".
pub type What = Option<Vec<Change>>;

/// Told whenever a plane changes on disk, whichever plane it is, and what changed in it.
pub type Changed = Arc<dyn Fn(PlaneId, What) + Send + Sync + 'static>;

/// How long a burst waits for the plane to go quiet. Short enough that a todo closed in a
/// terminal is off the panel within the second #264 asks for; long enough that a `git pull` is
/// one read, not ten.
const QUIET_FOR: Duration = Duration::from_millis(250);

/// The longest a burst stays open while writes keep coming (#4741): agents writing the plane
/// without pause are one `plane-changed` a second, not one each [`QUIET_FOR`].
const LONGEST: Duration = Duration::from_secs(1);

/// The most changed paths one burst holds. Past it the burst is told as "anything may have
/// moved": a flood costs a fixed amount of memory, and every reader reads again. Scaled with
/// [`LONGEST`] (256 a quarter second before #4741), so steady writes are no likelier to become
/// everything for a burst lasting up to a second.
const MOST_PATHS: usize = 1024;

/// The least time between two `changes: null` of one plane (felt speed round 2). Under a
/// build's load FSEvents drops events, and each rescan it reports was a burst that made every
/// reader read again and every artifact tab ask for its page: one a second, while the build
/// ran. A rescan inside this of the one told is owed, and told once at its end.
const UNKNOWN_EVERY: Duration = Duration::from_secs(5);

/// **When a burst whose changes are not known is told** ([`UNKNOWN_EVERY`]).
#[derive(Debug, Default)]
struct Unknowns {
    /// When the last one was told.
    told: Option<Instant>,
    /// Whether one is owed, to be told at the end of the gap.
    owed: bool,
}

/// What to do with a burst whose changes are not known.
#[derive(Debug, PartialEq, Eq)]
enum Unknown {
    /// Tell it now.
    Now,
    /// Tell it after this long, once: nothing was owed before it.
    After(Duration),
    /// It is folded into the one already owed.
    Folded,
}

impl Unknowns {
    /// A burst whose changes are not known was heard at `now`.
    fn heard(&mut self, now: Instant) -> Unknown {
        if self.owed {
            return Unknown::Folded;
        }
        match self.told {
            Some(told) if now.saturating_duration_since(told) < UNKNOWN_EVERY => {
                self.owed = true;
                Unknown::After((told + UNKNOWN_EVERY).saturating_duration_since(now))
            }
            _ => {
                self.told = Some(now);
                Unknown::Now
            }
        }
    }

    /// The one owed is told at `now`: whether one was.
    fn due(&mut self, now: Instant) -> bool {
        if !std::mem::take(&mut self.owed) {
            return false;
        }
        self.told = Some(now);
        true
    }
}

/// The watcher and the directories it is watching, behind one lock: the set is re-read on the
/// watch's own thread after every burst.
struct Inner<W: notify::Watcher> {
    watcher: Option<W>,
    mode: Mode,
    /// What the platform was asked to watch: each folder, or on [`Mode::OneStream`] the root.
    watched: HashSet<PathBuf>,
    /// The folders whose changes are told ([`wanted`]): on [`Mode::OneStream`], what the
    /// stream's events are kept to.
    listened: Listened,
    /// The folders the platform would not watch at the last follow, which are still there.
    unwatched: HashSet<PathBuf>,
    /// Whether any folder went unwatched since [`Watch::standing`] last said the watch was
    /// whole.
    lapsed: bool,
    rewatch: Rewatch,
}

/// How far a plane's watch can be trusted to tell everything the panels and the sidebar read
/// ([`Watch::standing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Every folder is watched, and has been since this was last asked.
    Whole,
    /// Every folder is watched now, but some went unwatched since this was last asked: what
    /// was held from the watch's word over that gap has to be read again.
    WholeAgain,
    /// Some folder the platform would not watch, which the watch's own look tells instead
    /// (#5499), every [`SWEEP_EVERY`] while a window is shown.
    Partial,
}

/// The watcher the app's plane watch runs on: the platform's own (FSEvents, inotify). In this
/// crate's tests on macOS, notify's poller, looking every 50 ms: FSEvents gives no bound on
/// when it delivers (#577, #756), and a test of a held plane waits on it.
#[cfg(not(all(test, target_os = "macos")))]
pub type Platform = notify::RecommendedWatcher;
#[cfg(all(test, target_os = "macos"))]
pub type Platform = notify::PollWatcher;

/// How a watch hears the folders it reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// One recursive watch on the root, its events kept to the folders the panels and the
    /// sidebar read, here (#5499). macOS: FSEvents watches a subtree as cheaply as a folder,
    /// and every folder added to a stream restarts it.
    OneStream,
    /// One watch for each folder, none recursive. Linux: an inotify watch is one directory,
    /// and a recursive one would watch every clone's build trees.
    PerFolder,
}

impl Mode {
    /// The mode the app runs on this platform. **In this crate's tests, a watch per folder
    /// everywhere**: on macOS they run on notify's poller ([`Platform`]), not FSEvents, and
    /// what a held plane does with a folder the platform refuses is what they check. One
    /// stream is tested on its own, in this module.
    const fn here() -> Self {
        if cfg!(all(target_os = "macos", not(test))) {
            Self::OneStream
        } else {
            Self::PerFolder
        }
    }
}

/// How [`Platform`] is configured.
fn platform_config() -> notify::Config {
    let config = notify::Config::default();
    #[cfg(all(test, target_os = "macos"))]
    let config = config.with_poll_interval(Duration::from_millis(50));
    config
}

/// The least time between two drops of every watch ([`Rewatch`]).
const REWATCH_EVERY: Duration = Duration::from_secs(1);

/// When a burst that was everything drops every watch and watches the set again.
///
/// **Only on inotify**, whose watch goes with its directory: a directory removed and made
/// again inside a burst too big to say which paths it held would otherwise never be watched
/// again. FSEvents and the others watch a path, not a directory, and dropping every watch there
/// restarts the stream once per folder for nothing, so they only follow the set again.
/// **And at most once a [`REWATCH_EVERY`]**: a drop owed sooner is made on the first burst
/// after it.
#[derive(Debug, Default)]
struct Rewatch {
    owed: bool,
    last: Option<Instant>,
}

impl Rewatch {
    /// Whether to drop every watch now, at `at`, after a burst on `kind` that was or was not
    /// `everything`.
    fn now(&mut self, kind: notify::WatcherKind, everything: bool, at: Instant) -> bool {
        if everything && kind == notify::WatcherKind::Inotify {
            self.owed = true;
        }
        if !self.owed
            || self
                .last
                .is_some_and(|last| at.saturating_duration_since(last) < REWATCH_EVERY)
        {
            return false;
        }
        self.owed = false;
        self.last = Some(at);
        true
    }
}

/// How often a watch looks at the folders the platform would not watch, while a window is
/// shown (#756, #5499). Five seconds keeps a change there within the "few seconds" #756 asks
/// for; only those folders are read, so a whole watch looks at nothing. Nothing looks while
/// every window is hidden.
pub(crate) const SWEEP_EVERY: Duration = Duration::from_secs(5);

/// How long a change the look had to tell keeps the doctor's `file events` row a warning.
pub(crate) const LATELY: Duration = Duration::from_secs(600);

/// The app's windows as the plane watches' looks follow them (#756): whether one is shown, and
/// each time one was focused. One for the app ([`windows`]); a test makes its own.
pub struct Windows {
    state: Mutex<Shown>,
    wake: Condvar,
    /// Whether any window is shown now, asked from a look's own thread. Until it is set, every
    /// window counts as shown.
    shown: OnceLock<Box<dyn Fn() -> bool + Send + Sync>>,
    /// Each watched root: how many watches watch it, and the changes their looks had to tell.
    /// Counted, so a project's watch started again before the old one is dropped is not
    /// forgotten when the old one goes.
    missed: Mutex<HashMap<PathBuf, (usize, Missed)>>,
}

#[derive(Debug, Default)]
struct Shown {
    /// Every window was found hidden or minimised at the last look: nothing looks until one is
    /// focused.
    hidden: bool,
    /// How many times a window was focused.
    focused: u64,
}

/// What a watch's looks had to tell because no event did, since the watch started.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Missed {
    /// How many changes.
    pub count: u64,
    /// When the last was told.
    pub last: Option<Instant>,
}

/// Why a look's thread woke.
enum Wake {
    /// Its watch is gone.
    Gone,
    /// A window was focused.
    Focused,
    /// Its time to look came.
    Look,
}

static WINDOWS: LazyLock<Arc<Windows>> = LazyLock::new(Windows::new);

/// The app's [`Windows`], which every plane the app opens is watched with.
pub fn windows() -> &'static Arc<Windows> {
    &WINDOWS
}

/// A window's focus changed (`lib.rs`'s `WindowEvent::Focused` arm, #756): the first time, the
/// app's [`Windows`] learn how to ask whether any window is shown — visible and not minimised —
/// and each time a window gains focus, every plane watch looks now and goes on looking.
pub fn window_focused(app: &tauri::AppHandle, focused: bool) {
    let watches = windows();
    if watches.shown.get().is_none() {
        let app = app.clone();
        watches.shown_when(move || {
            use tauri::Manager as _;
            app.windows().values().any(|window| {
                window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
            })
        });
    }
    if focused {
        watches.focused();
    }
}

impl Windows {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Shown::default()),
            wake: Condvar::new(),
            shown: OnceLock::new(),
            missed: Mutex::new(HashMap::new()),
        })
    }

    /// How a look asks whether any window is shown. The first one given is kept.
    pub fn shown_when(&self, shown: impl Fn() -> bool + Send + Sync + 'static) {
        let _ = self.shown.set(Box::new(shown));
    }

    /// A window was focused: every watch looks now, and goes on looking.
    pub fn focused(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.hidden = false;
        state.focused += 1;
        drop(state);
        self.wake.notify_all();
    }

    /// What the looks of the watch on `root` had to tell, or `None` when nothing watches it.
    pub fn missed(&self, root: &Path) -> Option<Missed> {
        self.missed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(root)
            .map(|(_, missed)| *missed)
    }

    fn any_shown(&self) -> bool {
        self.shown.get().is_none_or(|shown| shown())
    }

    fn hide(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .hidden = true;
    }

    fn focused_count(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .focused
    }

    /// Waits for a focus past `seen`, for `every` to pass while a window is shown, or for
    /// `alive` to say no. `alive` is asked under this lock, and a watch's drop takes it before
    /// it wakes the looks, so a drop between the asking and the waiting is never slept through.
    fn wait(&self, seen: u64, every: Duration, alive: &dyn Fn() -> bool) -> Wake {
        let until = Instant::now() + every;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if !alive() {
                return Wake::Gone;
            }
            if state.focused != seen {
                return Wake::Focused;
            }
            if state.hidden {
                state = self
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Wake::Look;
            }
            state = self
                .wake
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Wakes every look's thread to see whether its watch is still there.
    fn wake_all(&self) {
        drop(self.state.lock().unwrap_or_else(PoisonError::into_inner));
        self.wake.notify_all();
    }

    fn watching(&self, root: &Path) {
        self.missed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(root.to_path_buf())
            .or_default()
            .0 += 1;
    }

    fn not_watching(&self, root: &Path) {
        let mut missed = self.missed.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((watches, _)) = missed.get_mut(root) {
            *watches = watches.saturating_sub(1);
            if *watches == 0 {
                missed.remove(root);
            }
        }
    }

    fn told_missed(&self, root: &Path, count: usize, at: Instant) {
        if let Some(missed) = self
            .missed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(root)
            .map(|(_, missed)| missed)
        {
            missed.count += count as u64;
            missed.last = Some(at);
        }
    }
}

/// One entry of a watched folder, as a look compares it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stamp {
    /// A folder: compared by its being there (see the module's header).
    Folder,
    /// Anything else, by its time and length, as `lstat` gives them.
    Other {
        modified: Option<SystemTime>,
        len: u64,
    },
}

/// What one look saw: each watched folder it could read, and its entries.
type Look = HashMap<PathBuf, HashMap<OsString, Stamp>>;

/// The entries of each of `dirs`. A folder that cannot be read is left out, and compared with
/// nothing: its going is seen in the folder it was in.
fn look<'a>(dirs: impl IntoIterator<Item = &'a PathBuf>) -> Look {
    dirs.into_iter()
        .filter_map(|dir| {
            let entries = std::fs::read_dir(dir).ok()?;
            let entries = entries
                .flatten()
                .filter_map(|entry| {
                    let found = std::fs::symlink_metadata(entry.path()).ok()?;
                    let stamp = if found.file_type().is_dir() {
                        Stamp::Folder
                    } else {
                        Stamp::Other {
                            modified: found.modified().ok(),
                            len: found.len(),
                        }
                    };
                    Some((entry.file_name(), stamp))
                })
                .collect();
            Some((dir.clone(), entries))
        })
        .collect()
}

/// What moved between two looks, in the folders both read: the paths that differ, and among
/// them the ones that went.
fn differ(before: &Look, now: &Look) -> (HashSet<PathBuf>, HashSet<PathBuf>) {
    let mut changed = HashSet::new();
    let mut removed = HashSet::new();
    for (dir, entries) in now {
        let Some(was) = before.get(dir) else {
            continue;
        };
        for (name, stamp) in entries {
            if was.get(name) != Some(stamp) {
                changed.insert(dir.join(name));
            }
        }
        for name in was.keys().filter(|name| !entries.contains_key(*name)) {
            let path = dir.join(name);
            removed.insert(path.clone());
            changed.insert(path);
        }
    }
    (changed, removed)
}

/// One plane's watch. Dropping it stops it.
///
/// `W` is where the changes come from: the platform's own watcher (FSEvents, inotify) in the
/// app. The tests on macOS choose notify's poller instead, because FSEvents gives no bound on
/// when it delivers (#577): its one daemon serves the whole machine, and on a Mac busy writing
/// build trees it was measured handing a stream a change 4 to 15 seconds late, and a stream
/// nothing at all for minutes. What this module decides — fold a burst, follow a new workspace,
/// fall silent when dropped — is the same whichever watcher feeds it.
pub struct Watch<W: notify::Watcher = Platform> {
    inner: Arc<Mutex<Inner<W>>>,
    /// What its looks follow ([`Windows`]), and the root it is counted under there.
    windows: Arc<Windows>,
    root: PathBuf,
    /// Whether it covers the plane's git standing ([`covering`]).
    covers: bool,
}

impl Watch {
    /// Starts watching `root` and tells `changed` about `plane` whenever it moves, looking for
    /// itself as the app's windows are shown and focused ([`windows`]).
    pub fn start(plane: PlaneId, root: &Path, changed: Changed) -> notify::Result<Self> {
        Self::start_with(
            plane,
            root,
            changed,
            platform_config(),
            Arc::clone(windows()),
            SWEEP_EVERY,
            Mode::here(),
        )
    }
}

/// Every watch the session host runs (#2002): no window is beside it, so its watches never
/// look for themselves and cost nothing at rest; their events alone keep its saver's standing
/// current, and the standing is read again by itself after [`purlis_core::planegit::SHARED_FOR`].
static IN_THE_HOST: LazyLock<Arc<Windows>> = LazyLock::new(|| {
    let windows = Windows::new();
    windows.shown_when(|| false);
    windows
});

impl Watch {
    /// Starts watching `root` in the session host, for its saver and its project index
    /// (`crate::hostsaving`): what moved marks the plane's git standing to be read again, and is
    /// told to `changed`, which hands it to the index (ADR 0079 §4). No window is told.
    pub fn in_the_host(root: &Path, changed: Changed) -> notify::Result<Self> {
        Self::start_with(
            crate::planes::PlaneId::saved_in_the_host(root),
            root,
            changed,
            platform_config(),
            Arc::clone(&IN_THE_HOST),
            SWEEP_EVERY,
            Mode::here(),
        )
    }
}

impl<W: notify::Watcher> Watch<W> {
    /// Whether this watch tells everything the panels and the sidebar read, and whether it
    /// has since it was last asked. `WholeAgain` is said once: asking again says `Whole`.
    pub fn standing(&self) -> Standing {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if !inner.unwatched.is_empty() {
            Standing::Partial
        } else if std::mem::take(&mut inner.lapsed) {
            Standing::WholeAgain
        } else {
            Standing::Whole
        }
    }
}

impl<W: notify::Watcher + Send + 'static> Watch<W> {
    /// [`Watch::start`] on a watcher of the caller's choosing, configured by `config`, its looks
    /// following `windows` every `every` while one is shown.
    fn start_with(
        plane: PlaneId,
        root: &Path,
        changed: Changed,
        config: notify::Config,
        windows: Arc<Windows>,
        every: Duration,
        mode: Mode,
    ) -> notify::Result<Self> {
        // The platform reports paths as the disk spells them, which a root opened through a
        // link (`/var` for `/private/var` on macOS) is not.
        let spelled = root.canonicalize().ok();
        let listened = Listened {
            folders: Arc::default(),
            root: root.to_path_buf(),
            spelled: spelled.clone().filter(|spelled| spelled != root),
        };
        let (sent, events) = mpsc::channel();
        let mut send = crate::watchset::sender(sent);
        let keep = listened.clone();
        let tree = GitTree {
            root: root.to_path_buf(),
            spelled: listened.spelled.clone(),
        };
        let heard = move |event: notify::Result<notify::Event>| {
            // One stream hears the whole tree (#5505): a move git may show makes the plane's
            // standing read again, before the event is kept to what the panels read.
            if mode == Mode::OneStream && tree.may_show(&event) {
                purlis_core::planegit::tree_moved(&tree.root);
            }
            let event = match event {
                Ok(event) if mode == Mode::OneStream => match keep.kept(event) {
                    Some(event) => Ok(event),
                    None => return,
                },
                other => other,
            };
            notify::EventHandler::handle_event(&mut send, event);
        };
        let watcher = W::new(heard, config)?;
        let inner = Arc::new(Mutex::new(Inner {
            watcher: Some(watcher),
            mode,
            watched: HashSet::new(),
            listened: listened.clone(),
            unwatched: HashSet::new(),
            lapsed: false,
            rewatch: Rewatch::default(),
        }));
        let first = {
            let mut inner = inner.lock().unwrap_or_else(PoisonError::into_inner);
            // On one stream, only the stream: the folders its events are kept to are listed on
            // the watch's thread, below. At 1,000 workspaces that listing is some 5,000 folders
            // and 12,000 calls to the disk, which a project's open would otherwise wait for
            // (#5672).
            match mode {
                Mode::PerFolder => inner.follow(root),
                Mode::OneStream => inner.stream(root),
            }
            // The root is where a workspace or a persona arriving is heard: without it the
            // watch could never become whole, so it is no watch.
            if inner.unwatched.contains(root) {
                return Err(notify::Error::generic(&format!(
                    "the platform would not watch {}",
                    root.display()
                )));
            }
            // The first look at the folders it could not watch is taken now, so a change in
            // one from here on is one the next look finds.
            look(&inner.unwatched)
        };
        let handle: Weak<Mutex<Inner<W>>> = Arc::downgrade(&inner);
        let at = root.to_path_buf();
        let unknowns: Arc<Mutex<Unknowns>> = Arc::default();
        let tell = {
            let handle = handle.clone();
            let at = at.clone();
            move |burst: Burst| {
                let what = what_changed(&at, spelled.as_deref(), &burst);
                // The set first, so a workspace made in this burst is watched before the window
                // reads it, and a change inside it straight after is not missed.
                //
                // **And nothing at all once the watch is dropped.** Dropping it ends this
                // thread, but a burst already gathered still arrives here, and a plane that
                // has been closed must not be reported.
                {
                    let Some(inner) = handle.upgrade() else {
                        return;
                    };
                    let mut inner = inner.lock().unwrap_or_else(PoisonError::into_inner);
                    if inner.watcher.is_none() {
                        return;
                    }
                    // What went is not known, so on inotify nothing watched can be trusted to
                    // still be.
                    if inner
                        .rewatch
                        .now(W::kind(), burst.everything, Instant::now())
                    {
                        inner.forget_all();
                    }
                    for path in &burst.removed {
                        inner.forget(path);
                    }
                    inner.follow(&at);
                }
                if what.is_none() {
                    let asked = unknowns
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .heard(Instant::now());
                    match asked {
                        Unknown::Now => {}
                        Unknown::Folded => return,
                        Unknown::After(wait) => {
                            let handle = handle.clone();
                            let folded = Arc::clone(&unknowns);
                            let changed = Arc::clone(&changed);
                            let (plane, at) = (plane.clone(), at.clone());
                            let owed = move || {
                                std::thread::sleep(wait);
                                let alive = handle.upgrade().is_some_and(|inner| {
                                    inner
                                        .lock()
                                        .unwrap_or_else(PoisonError::into_inner)
                                        .watcher
                                        .is_some()
                                });
                                let due = folded
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner)
                                    .due(Instant::now());
                                if alive && due {
                                    purlis_core::planegit::touch(&at);
                                    changed(plane, None);
                                }
                            };
                            if let Err(why) = std::thread::Builder::new()
                                .name("charter-plane-owed".into())
                                .spawn(owed)
                            {
                                tracing::warn!(
                                    "purlis: a plane's change not known is told now ({why})"
                                );
                                unknowns
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner)
                                    .due(Instant::now());
                            } else {
                                return;
                            }
                        }
                    }
                }
                // Before the window hears it, so the reads it makes on `plane-changed` are of
                // the plane as it is now, not the shared standings from before (FD-11).
                purlis_core::planegit::touch(&at);
                changed(plane.clone(), what);
            }
        };
        let tell: Arc<dyn Fn(Burst) + Send + Sync> = Arc::new(tell);
        {
            let tell = Arc::clone(&tell);
            let listing = (mode == Mode::OneStream).then(|| (listened.clone(), at.clone()));
            std::thread::Builder::new()
                .name("charter-plane-watch".into())
                .spawn(move || {
                    // Before any burst, whose own follow lists them again.
                    if let Some((listened, root)) = listing {
                        listened.set(wanted(&root));
                    }
                    for burst in
                        crate::watchset::trailing_bursts(events, QUIET_FOR, LONGEST, MOST_PATHS)
                    {
                        tell(burst);
                    }
                })
                .map_err(notify::Error::io)?;
        }
        if mode == Mode::OneStream {
            covering(root, true);
        }
        windows.watching(root);
        // The focus count the first look stands at, read here and not on the look's thread: a
        // focus between this start and that thread's first read would otherwise be counted as
        // already seen, and wait out a whole `every` (#1731).
        let seen = windows.focused_count();
        let looking = Looking {
            inner: handle,
            windows: Arc::clone(&windows),
            root: at,
            every,
            tell,
        };
        let started = std::thread::Builder::new()
            .name("charter-plane-look".into())
            .spawn(move || looking.run(first, seen));
        if let Err(why) = started {
            windows.not_watching(root);
            return Err(notify::Error::io(why));
        }
        Ok(Self {
            inner,
            windows,
            root: root.to_path_buf(),
            covers: mode == Mode::OneStream,
        })
    }
}

/// A watch's own looks (#756, the module's header), on a thread of their own.
struct Looking<W: notify::Watcher> {
    inner: Weak<Mutex<Inner<W>>>,
    windows: Arc<Windows>,
    root: PathBuf,
    every: Duration,
    tell: Arc<dyn Fn(Burst) + Send + Sync>,
}

impl<W: notify::Watcher> Looking<W> {
    /// Looks until the watch is dropped, from `before`, the look taken as it started, and
    /// `seen`, the focus count as it started.
    fn run(self, mut before: Look, mut seen: u64) {
        loop {
            match self.windows.wait(seen, self.every, &|| self.alive()) {
                Wake::Gone => return,
                Wake::Focused => {
                    // A storm of focus changes is one look: the ones inside this wait are
                    // folded into it.
                    std::thread::sleep(QUIET_FOR);
                    seen = self.windows.focused_count();
                }
                Wake::Look => {
                    // Asked outside every lock: the answer may come from the window's thread.
                    if !self.windows.any_shown() {
                        self.windows.hide();
                        continue;
                    }
                }
            }
            let Some(refused) = self.refused() else {
                return;
            };
            let now = look(&refused);
            let (missed, removed) = differ(&before, &now);
            if !missed.is_empty() {
                self.windows
                    .told_missed(&self.root, missed.len(), Instant::now());
                (self.tell)(Burst {
                    paths: missed,
                    removed,
                    ..Burst::default()
                });
            }
            before = self.merged(now);
        }
    }

    /// Whether the watch is still there.
    fn alive(&self) -> bool {
        self.inner.upgrade().is_some_and(|inner| {
            inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .watcher
                .is_some()
        })
    }

    /// The folders the platform would not watch at the last follow, or `None` once the watch
    /// is dropped.
    fn refused(&self) -> Option<HashSet<PathBuf>> {
        let inner = self.inner.upgrade()?;
        let inner = inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.watcher.as_ref()?;
        Some(inner.unwatched.clone())
    }

    /// `now`, and a first look at each folder the platform refused since (a workspace made
    /// once the machine was out of watches): what the next look compares against.
    fn merged(&self, mut now: Look) -> Look {
        let Some(refused) = self.refused() else {
            return now;
        };
        let new: Vec<PathBuf> = refused
            .into_iter()
            .filter(|dir| !now.contains_key(dir))
            .collect();
        now.extend(look(&new));
        now
    }
}

impl<W: notify::Watcher> Drop for Watch<W> {
    fn drop(&mut self) {
        // Taken out under the lock and dropped outside it, so the watch's thread, which takes
        // the same lock, never waits on a drop. An empty slot is also what tells a burst
        // already in flight that nobody is listening any more.
        let watcher = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watcher
            .take();
        drop(watcher);
        if self.covers {
            covering(&self.root, false);
        }
        self.windows.not_watching(&self.root);
        // And its looks stop, whether a window is shown or not.
        self.windows.wake_all();
    }
}

impl<W: notify::Watcher> Inner<W> {
    /// Watches what the plane has now, and stops watching what it no longer has.
    fn follow(&mut self, root: &Path) {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        let wanted = wanted(root);
        match self.mode {
            Mode::PerFolder => {
                self.unwatched = crate::watchset::follow(watcher, &mut self.watched, wanted);
                if !self.unwatched.is_empty() {
                    self.lapsed = true;
                }
            }
            Mode::OneStream => {
                self.stream(root);
                self.listened.set(wanted);
            }
        }
    }

    /// The one recursive watch of `root`, while it is a folder ([`Mode::OneStream`]): all the
    /// platform is asked for. Which of its folders' events are kept is [`Listened`]'s.
    fn stream(&mut self, root: &Path) {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        let stream = if a_dir(root) {
            HashSet::from([root.to_path_buf()])
        } else {
            HashSet::new()
        };
        self.unwatched = crate::watchset::follow_as(
            watcher,
            &mut self.watched,
            stream,
            notify::RecursiveMode::Recursive,
        );
        if !self.unwatched.is_empty() {
            self.lapsed = true;
        }
    }

    fn forget(&mut self, path: &Path) {
        if let Some(watcher) = self.watcher.as_mut() {
            crate::watchset::forget(watcher, &mut self.watched, path);
        }
    }

    fn forget_all(&mut self) {
        for path in std::mem::take(&mut self.watched) {
            if let Some(watcher) = self.watcher.as_mut() {
                let _ = watcher.unwatch(&path);
            }
        }
    }
}

/// How many watches on [`Mode::OneStream`] cover each plane's git standing: counted, so a
/// project's watch started again before the old one is dropped keeps it covered.
static COVERING: LazyLock<Mutex<HashMap<PathBuf, usize>>> = LazyLock::new(Mutex::default);

/// One more (`true`) or one fewer watch on one stream hears all of `root`'s tree. While any
/// does, the plane's shared git standing is read again on what moved, not on a clock (#5505,
/// `purlis_core::planegit::cover`). A watch on [`Mode::PerFolder`] never covers: it hears only
/// the folders the panels read, and an edit deeper down only the clock finds (D-FD11h).
fn covering(root: &Path, more: bool) {
    let mut counts = COVERING.lock().unwrap_or_else(PoisonError::into_inner);
    let count = counts.entry(root.to_path_buf()).or_default();
    let was = *count;
    *count = if more { was + 1 } else { was.saturating_sub(1) };
    let now = *count;
    if now == 0 {
        counts.remove(root);
    }
    drop(counts);
    if (was == 0) != (now == 0) {
        purlis_core::planegit::cover(root, now > 0);
    }
}

/// The plane's working tree as one stream hears it, for its git standing (#5505).
struct GitTree {
    root: PathBuf,
    spelled: Option<PathBuf>,
}

impl GitTree {
    /// Whether `event` may move what `git status` says of the plane: anything but a move only
    /// in git's own folder (the standing's stamp holds those), in the state purlis keeps in
    /// the plane (`names::STATE_DIR`, ignored by every plane), or inside a clone (a
    /// repository of its own, which the project's status never looks into). A loss, an
    /// error or an event naming no path may be anything.
    fn may_show(&self, event: &notify::Result<notify::Event>) -> bool {
        let Ok(event) = event else {
            return true;
        };
        if event.need_rescan() || event.paths.is_empty() {
            return true;
        }
        event.paths.iter().any(|path| self.shows(path))
    }

    fn shows(&self, path: &Path) -> bool {
        let below = path.strip_prefix(&self.root).ok().or_else(|| {
            self.spelled
                .as_deref()
                .and_then(|spelled| path.strip_prefix(spelled).ok())
        });
        let Some(below) = below else {
            return false;
        };
        let parts: Vec<&std::ffi::OsStr> = below.iter().collect();
        match parts.as_slice() {
            [first, ..]
                if first.to_str().is_some_and(|first| {
                    first == ".git" || purlis_core::names::STATE_DIR.recognises(first)
                }) =>
            {
                false
            }
            [workspaces, workspace, folder, _, ..] if *workspaces == "workspaces" => !self
                .root
                .join(workspaces)
                .join(workspace)
                .join(folder)
                .join(".git")
                .exists(),
            _ => true,
        }
    }
}

/// The folders a watch on [`Mode::OneStream`] tells the changes of, shared with the handler the
/// platform calls: one recursive stream hears the whole project, clones and their build trees
/// with it, and only what a watch of each of these folders would have reported is kept.
///
/// **Until the folders are first listed, an event waits for them** (#5672): the stream is started
/// before they are, and an event it hears then is kept or dropped as it would be once they are,
/// never told unfiltered and never lost.
#[derive(Clone)]
struct Listened {
    /// `None` until first listed.
    folders: Arc<(Mutex<Option<HashSet<PathBuf>>>, Condvar)>,
    root: PathBuf,
    /// The root as the disk spells it, where that is another spelling.
    spelled: Option<PathBuf>,
}

impl Listened {
    fn set(&self, folders: HashSet<PathBuf>) {
        let (listed, ready) = &*self.folders;
        *listed.lock().unwrap_or_else(PoisonError::into_inner) = Some(folders);
        ready.notify_all();
    }

    /// `event` with only the paths a watch of each listened folder would have reported: the
    /// folder itself, and each entry directly in it. `None` when that leaves none. **A loss is
    /// always kept**, and so is an event naming no path: what they lost may be anywhere.
    fn kept(&self, mut event: notify::Event) -> Option<notify::Event> {
        if event.need_rescan() || event.paths.is_empty() {
            return Some(event);
        }
        let (listed, ready) = &*self.folders;
        let listed = ready
            .wait_while(
                listed.lock().unwrap_or_else(PoisonError::into_inner),
                |folders| folders.is_none(),
            )
            .unwrap_or_else(PoisonError::into_inner);
        let Some(folders) = listed.as_ref() else {
            return Some(event);
        };
        event.paths.retain(|path| {
            let path = self.respelled(path);
            folders.contains(path.as_ref())
                || path.parent().is_some_and(|folder| folders.contains(folder))
        });
        (!event.paths.is_empty()).then_some(event)
    }

    /// `path` under the root as it was given, where the disk spelled it another way.
    fn respelled<'a>(&self, path: &'a Path) -> std::borrow::Cow<'a, Path> {
        match self
            .spelled
            .as_deref()
            .and_then(|spelled| path.strip_prefix(spelled).ok())
        {
            Some(below) => std::borrow::Cow::Owned(self.root.join(below)),
            None => std::borrow::Cow::Borrowed(path),
        }
    }
}

/// What a burst changed, each path placed by [`planechange`] against the root as it was given
/// or, failing that, as the disk spells it — or `None` when any one of them cannot be placed
/// or the burst is everything: an event named no path, notify asked for a rescan (it dropped
/// events it cannot name), or there were more paths than a burst holds.
fn what_changed(root: &Path, spelled: Option<&Path>, burst: &Burst) -> What {
    if burst.everything {
        return None;
    }
    let paths = || burst.paths.iter().map(PathBuf::as_path);
    planechange::of_batch(root, paths())
        .or_else(|| spelled.and_then(|spelled| planechange::of_batch(spelled, paths())))
}

/// The most entries a plain workspace folder may hold and still be watched (#1160): a folder
/// the platform refuses is looked at entry by entry, so a big folder that is not a clone (a
/// download or build folder) would cost every look that much. Such a folder becoming a clone is seen
/// as it was before #1160: not until something else re-reads the workspace.
const PLAIN_FOLDER_MOST: usize = 1024;

/// Whether `folder` holds at most [`PLAIN_FOLDER_MOST`] entries, counting no further.
fn small(folder: &Path) -> bool {
    std::fs::read_dir(folder)
        .is_ok_and(|entries| entries.take(PLAIN_FOLDER_MOST + 1).count() <= PLAIN_FOLDER_MOST)
}

/// Whether `path` is a folder, a link to one not followed.
fn a_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_dir())
}

/// The directories the panels and the sidebar read, as they are on disk now. See the module's
/// header for why these and not the plane recursively.
///
/// **A link is not followed.** charter refuses a store that is a link out of the plane, and a
/// watch through one would be charter listening to a directory it will not read.
fn wanted(root: &Path) -> HashSet<PathBuf> {
    let mut wanted = HashSet::new();
    if a_dir(root) {
        wanted.insert(root.to_path_buf());
    }
    let personas = root.join("personas");
    if a_dir(&personas) {
        for name in std::fs::read_dir(&personas).into_iter().flatten().flatten() {
            let persona = name.path();
            if a_dir(&persona) {
                let memory = persona.join("memory");
                if a_dir(&memory) {
                    wanted.insert(memory);
                }
                wanted.insert(persona);
            }
        }
        wanted.insert(personas);
    }
    let records = root.join("sessions");
    if a_dir(&records) {
        wanted.insert(records);
    }
    for harness in [".claude", ".claude/agents"] {
        let dir = root.join(harness);
        if a_dir(&dir) {
            wanted.insert(dir);
        }
    }
    let workspaces = root.join("workspaces");
    if !a_dir(&workspaces) {
        return wanted;
    }
    wanted.insert(workspaces);
    let plane = Plane::open(root);
    for name in plane.workspaces().unwrap_or_default() {
        let Ok(workspace) = plane.workspace(&name) else {
            continue;
        };
        let dir = workspace.dir().to_path_buf();
        if !a_dir(&dir) {
            continue;
        }
        const STORES: [&str; 3] = ["todos", "memory", "sessions"];
        for store in STORES {
            let store = dir.join(store);
            if a_dir(&store) {
                wanted.insert(store);
            }
        }
        // And the done todos (ADR 0106), which the Todos view's Done tab lists: a done todo
        // deleted in a terminal, or one a sync brought, changes nothing in `todos/` itself.
        let done = dir.join("todos").join(purlis_core::memstore::ARCHIVE);
        if a_dir(&done) {
            wanted.insert(done);
        }
        // A plain folder, on its own: the one change it can make that the workspace's listing
        // never shows is becoming a clone (#1160). A clone is not watched (see the header), and
        // neither is a plain folder holding more than [`PLAIN_FOLDER_MOST`] entries, which
        // every look would stat one by one.
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name();
            let plain = name
                .to_str()
                .is_some_and(|name| !name.starts_with('.') && !STORES.contains(&name));
            let folder = entry.path();
            if plain && a_dir(&folder) && !folder.join(".git").exists() && small(&folder) {
                wanted.insert(folder);
            }
        }
        artifacts(&dir, &mut wanted);
        wanted.insert(dir);
    }
    wanted
}

/// **A workspace's artifacts** (ADR 0122, #5955): `artifacts/` itself, which holds the pins,
/// and each pinned folder with every folder in it, so an open artifact hears its template and
/// its data change. Only what is pinned: an artifact folder is small (it is served whole), and
/// one nobody pinned is not read by anything. A link is not followed, as nowhere here.
fn artifacts(workspace: &Path, wanted: &mut HashSet<PathBuf>) {
    let dir = workspace.join(purlis_core::artifact::DIR);
    if !a_dir(&dir) {
        return;
    }
    let pins = purlis_core::artifact::read(workspace)
        .map(|read| read.pins)
        .unwrap_or_default();
    for pin in &pins {
        let folder = dir.join(&pin.folder);
        if a_dir(&folder) {
            folders_in(&folder, ARTIFACT_DEPTH, wanted);
        }
    }
    wanted.insert(dir);
}

/// How deep a pinned artifact's folders are watched: as deep as one is served.
const ARTIFACT_DEPTH: usize = 16;

/// `folder` and every folder in it, `depth` deep, none dotted and none through a link.
fn folders_in(folder: &Path, depth: usize, wanted: &mut HashSet<PathBuf>) {
    if depth > 0 {
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let inner = entry.path();
            let dotted = entry.file_name().to_string_lossy().starts_with('.');
            if !dotted && a_dir(&inner) {
                folders_in(&inner, depth - 1, wanted);
            }
        }
    }
    wanted.insert(folder.to_path_buf());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// How long a test waits to be told before it fails. Only a failure waits this long; a
    /// pass comes back in a quarter of a second.
    const PATIENCE: Duration = Duration::from_secs(10);

    fn plane_with_todos(todos: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a scratch plane");
        let store = dir.path().join("workspaces/alpha/todos");
        std::fs::create_dir_all(&store).expect("a todo store");
        std::fs::create_dir_all(dir.path().join("personas")).expect("personas");
        for slug in todos {
            std::fs::write(store.join(format!("{slug}.md")), format!("# {slug}\n")).expect("todo");
        }
        dir
    }

    fn id(root: &Path) -> PlaneId {
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id")
    }

    /// Where the tests' changes come from (#577). On macOS, notify's poller: FSEvents has no
    /// bound on when it delivers. On a busy Mac it handed a stream a change seconds to minutes
    /// late, and handed it the writes that made the plane seconds after the stream began, so
    /// no deadline made these tests pass there; a longer one only waited longer for the same
    /// flake. Elsewhere the platform's own watcher, as the app runs it: inotify is where an
    /// access used to loop.
    #[cfg(target_os = "macos")]
    type Source = notify::PollWatcher;
    #[cfg(not(target_os = "macos"))]
    type Source = notify::RecommendedWatcher;

    /// How often the poller looks; inotify ignores it. Well inside [`QUIET_FOR`], so what the
    /// window waits on is the debounce, not the look.
    const LOOK_EVERY: Duration = Duration::from_millis(50);

    /// A watch on `root` fed by the tests' [`Source`], and the channel it tells. Anything done
    /// to the plane from here on is told. Nothing done before it is — a promise of the poller
    /// and of inotify, NOT of FSEvents, which the app runs on macOS (see [`Source`]).
    fn watching(root: &Path) -> (Watch<Source>, mpsc::Receiver<PlaneId>) {
        watching_on::<Source>(root)
    }

    /// [`watching`] on a watcher of the test's choosing.
    fn watching_on<W: notify::Watcher + Send + 'static>(
        root: &Path,
    ) -> (Watch<W>, mpsc::Receiver<PlaneId>) {
        let (watch, told) = telling_on::<W>(root);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for (plane, _) in told {
                if tx.send(plane).is_err() {
                    return;
                }
            }
        });
        (watch, rx)
    }

    /// [`watching`], told what changed as well as which plane.
    fn telling(root: &Path) -> (Watch<Source>, mpsc::Receiver<(PlaneId, What)>) {
        telling_on::<Source>(root)
    }

    fn telling_on<W: notify::Watcher + Send + 'static>(
        root: &Path,
    ) -> (Watch<W>, mpsc::Receiver<(PlaneId, What)>) {
        looking_on::<W>(root, &Windows::new(), NEVER)
    }

    /// Longer than any test: a watch whose looks a test does not ask about looks only when its
    /// own windows are focused, which nothing does.
    const NEVER: Duration = Duration::from_secs(3600);

    /// [`telling_on`], its looks following `windows` every `every`.
    fn looking_on<W: notify::Watcher + Send + 'static>(
        root: &Path,
        windows: &Arc<Windows>,
        every: Duration,
    ) -> (Watch<W>, mpsc::Receiver<(PlaneId, What)>) {
        looking_in::<W>(root, windows, every, Mode::PerFolder)
    }

    /// [`looking_on`], hearing the plane in `mode`.
    fn looking_in<W: notify::Watcher + Send + 'static>(
        root: &Path,
        windows: &Arc<Windows>,
        every: Duration,
        mode: Mode,
    ) -> (Watch<W>, mpsc::Receiver<(PlaneId, What)>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let watch = Watch::<W>::start_with(
            id(root),
            root,
            Arc::new(move |plane, what| {
                let _ = tx.lock().expect("sender").send((plane, what));
            }),
            notify::Config::default().with_poll_interval(LOOK_EVERY),
            Arc::clone(windows),
            every,
            mode,
        )
        .expect("a watch");
        (watch, rx)
    }

    #[test]
    fn a_todo_file_removed_under_a_watched_plane_is_told_within_the_second() {
        a_removed_todo_is_told_within_the_second_on::<Source>();
    }

    /// The same on the watcher the app runs on macOS. Ignored because FSEvents gives no bound
    /// on when it delivers, so on a busy Mac this fails however long it waits; until #756 the
    /// "about a second" of #264 is checked on Linux's inotify only.
    /// `cargo test -p purlis-app planewatch -- --ignored` runs it.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "FSEvents has no delivery bound on a busy Mac (#577, #756)"]
    fn on_fsevents_a_todo_file_removed_under_a_watched_plane_is_told_within_the_second() {
        a_removed_todo_is_told_within_the_second_on::<notify::RecommendedWatcher>();
    }

    fn a_removed_todo_is_told_within_the_second_on<W: notify::Watcher + Send + 'static>() {
        let plane = plane_with_todos(&["m8-1", "m8-2"]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = watching_on::<W>(&root);

        let slugs = || {
            let read = crate::panels::of(&root, "alpha").expect("the panels read");
            let read = serde_json::to_value(read).expect("serialisable");
            read["todos"]
                .as_array()
                .expect("todos")
                .iter()
                .map(|todo| todo["slug"].as_str().expect("a slug").to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(slugs(), ["m8-1", "m8-2"]);

        let started = Instant::now();
        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");

        let plane = told.recv_timeout(PATIENCE).expect("told the plane changed");
        // #264's "within about a second": the debounce is a quarter of one, and two is the
        // margin a loaded runner gets.
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "told after {:?}",
            started.elapsed()
        );
        assert_eq!(plane, id(&root));
        // And what the window reads when told — the same command the Todos panel draws from —
        // no longer has the row.
        assert_eq!(slugs(), ["m8-2"]);
    }

    #[test]
    fn a_burst_of_changes_is_folded_rather_than_told_per_write() {
        // On the platform the test plays (#1139): a busy machine spread ten writes past the
        // debouncer's quarter-second tick and the poller's look, and they were told three
        // times. A burst now runs a quarter of a second from its first event, so what decides
        // is how the watch folds what it is handed, and that is what is tested.
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind};
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = watching_on::<Raw>(&root);

        for n in 0..10 {
            raw(
                EventKind::Create(CreateKind::File),
                &root.join(format!("workspaces/alpha/todos/t{n}.md")),
            );
        }

        told.recv_timeout(PATIENCE).expect("told");
        assert!(
            told.recv_timeout(QUIET_FOR * 4).is_err(),
            "a burst of ten writes was told more than once"
        );
    }

    #[test]
    fn a_todo_made_and_removed_inside_one_burst_is_still_told() {
        // The window may have drawn the todo on a read some other change set off; a watch that
        // took the removal as cancelling the creation would never tell it to draw it gone.
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind, RemoveKind};
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = telling_on::<Raw>(&root);
        let todo = root.join("workspaces/alpha/todos/brief.md");

        raw(EventKind::Create(CreateKind::File), &todo);
        raw(EventKind::Remove(RemoveKind::File), &todo);

        let change = told_about(&told, "workspaces/alpha/todos/brief.md");
        assert_eq!(change.kind, Kind::Todos);
    }

    #[test]
    fn a_todo_in_a_workspace_made_after_the_watch_began_is_still_told() {
        // The workspace comes from a terminal while the window is open: `workspaces/` moves,
        // the set is re-read, and the new workspace's store is watched from then on.
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = watching(&root);

        std::fs::create_dir_all(root.join("workspaces/beta/todos")).expect("a new workspace");
        told.recv_timeout(PATIENCE)
            .expect("told about the workspace");
        // The batch that told it also followed `beta/` and `beta/todos/`, before telling; a
        // directory is watched from its own contents onward, so nothing else is pending here.

        std::fs::write(root.join("workspaces/beta/todos/new.md"), "# new\n").expect("a todo");
        told.recv_timeout(PATIENCE)
            .expect("told about a todo in the new workspace");
    }

    #[test]
    fn a_folder_the_platform_will_not_watch_leaves_the_watch_partial_until_it_is_watched() {
        // A watch that dropped a folder's registration and went on as if it watched the whole
        // plane served panels that never moved again (the FD-10 review). The watch says it is
        // partial, so its readers read the disk; once it watches everything again it says so
        // once, so they rebuild what they held over the gap.
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        let todos = root.join("workspaces/alpha/todos");
        crate::watchset::refuse::refuse(&todos);
        let (watch, told) = watching(&root);
        assert_eq!(watch.standing(), Standing::Partial);
        assert_eq!(
            watch.standing(),
            Standing::Partial,
            "still partial when asked again"
        );

        crate::watchset::refuse::allow(&todos);
        std::fs::create_dir_all(root.join("workspaces/beta")).expect("a change it is told");
        told.recv_timeout(PATIENCE).expect("told about beta");

        assert_eq!(watch.standing(), Standing::WholeAgain);
        assert_eq!(watch.standing(), Standing::Whole);
    }

    #[test]
    fn a_root_the_platform_will_not_watch_is_no_watch_at_all() {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        crate::watchset::refuse::refuse(&root);
        let started = Watch::<Source>::start_with(
            id(&root),
            &root,
            Arc::new(|_, _| {}),
            notify::Config::default().with_poll_interval(LOOK_EVERY),
            Windows::new(),
            NEVER,
            Mode::PerFolder,
        );
        assert!(
            started.is_err(),
            "a watch that cannot watch the root started"
        );
    }

    /// `git` in `dir`, through charter's hardened runner, never signing.
    fn git(dir: &Path, args: &[&str]) {
        let argv: Vec<&str> = ["-c", "commit.gpgsign=false"]
            .into_iter()
            .chain(args.iter().copied())
            .collect();
        let done = purlis_core::worktree::git::run(dir, &argv, purlis_core::worktree::git::READ)
            .expect("git runs in a test");
        assert!(done.ok(), "git {args:?}: {done:?}");
    }

    #[test]
    fn a_change_the_watch_tells_is_in_the_next_shared_standing() {
        // FD-11: the window reads the plane again on `plane-changed`, and what it reads is the
        // standing every poller shares, so the watch makes it current before it tells.
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        git(&root, &["init", "-q", "-b", "main", "."]);
        git(&root, &["config", "user.email", "t@example.invalid"]);
        git(&root, &["config", "user.name", "t"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "one"]);
        assert!(
            purlis_core::planegit::shared_standing(&root)
                .changed
                .is_empty()
        );
        let (_watch, told) = watching(&root);

        std::fs::write(root.join("workspaces/alpha/todos/new.md"), "# new\n").expect("a todo");
        told.recv_timeout(PATIENCE).expect("told about the todo");

        assert_eq!(
            purlis_core::planegit::shared_standing(&root).changed,
            vec!["workspaces/alpha/todos/new.md".to_owned()]
        );
    }

    #[test]
    fn a_plane_nobody_touches_is_never_told_about_its_own_making() {
        // What the tests used to sleep for: FSEvents hands a stream the writes that made the
        // plane BEFORE the stream began, as late as its daemon gets to them. A watch that reports
        // only what moved after it started is one a test can act against at once.
        a_plane_nobody_touches_is_quiet_on::<Source>();
    }

    /// The same on the watcher the app runs on macOS, where it does NOT hold: FSEvents told
    /// a stream about the plane's making seconds after it began. Ignored until #756 decides
    /// what the app does about slow file events. `-- --ignored` runs it.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "FSEvents replays the plane's making late on a busy Mac (#577, #756)"]
    fn on_fsevents_a_plane_nobody_touches_is_never_told_about_its_own_making() {
        a_plane_nobody_touches_is_quiet_on::<notify::RecommendedWatcher>();
    }

    fn a_plane_nobody_touches_is_quiet_on<W: notify::Watcher + Send + 'static>() {
        let plane = plane_with_todos(&["m8-1", "m8-2"]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = watching_on::<W>(&root);
        assert!(told.recv_timeout(QUIET_FOR * 8).is_err());
    }

    #[test]
    fn nothing_is_told_once_the_watch_is_dropped() {
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        let (watch, told) = watching(&root);
        drop(watch);

        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");
        assert!(told.recv_timeout(QUIET_FOR * 4).is_err());
    }

    #[test]
    fn reading_a_todo_is_not_a_change() {
        // The loop inotify would otherwise close: the window reads because it was told, and
        // the read opens the files it was told about.
        use crate::watchset::matters;
        use notify::EventKind;
        use notify::event::{
            AccessKind, AccessMode, CreateKind, MetadataKind, ModifyKind, RemoveKind,
        };
        assert!(!matters(&EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))));
        assert!(!matters(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(!matters(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(matters(&EventKind::Remove(RemoveKind::File)));
        assert!(matters(&EventKind::Create(CreateKind::File)));
    }

    /// Waits for a batch naming `path`, and returns what it said that path is.
    fn told_about(told: &mpsc::Receiver<(PlaneId, What)>, path: &str) -> Change {
        let until = Instant::now() + PATIENCE;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            let (_, what) = told.recv_timeout(left).expect("told the plane changed");
            // A batch the watcher could not place is told as `None`, and is not this test's
            // business: the change it waits for comes in a batch of its own or the next one.
            let Some(changes) = what else {
                continue;
            };
            if let Some(found) = changes.into_iter().find(|change| change.path == path) {
                return found;
            }
        }
        panic!("never told about {path}");
    }

    /// A burst of the changes the platform reported on `paths`.
    fn burst(paths: &[&Path]) -> Burst {
        Burst {
            paths: paths.iter().map(|path| path.to_path_buf()).collect(),
            ..Burst::default()
        }
    }

    #[test]
    fn a_burst_not_known_inside_the_gap_of_the_one_told_is_told_once_at_its_end() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut unknowns = Unknowns::default();
        assert_eq!(unknowns.heard(at(0)), Unknown::Now);
        assert_eq!(
            unknowns.heard(at(1_000)),
            Unknown::After(Duration::from_millis(4_000))
        );
        assert_eq!(unknowns.heard(at(2_000)), Unknown::Folded);
        assert_eq!(unknowns.heard(at(4_900)), Unknown::Folded);
        assert!(
            unknowns.due(at(5_000)),
            "the one owed is told at the gap's end"
        );
        assert!(!unknowns.due(at(5_000)), "and only once");
        // The one told at the end starts the next gap.
        assert_eq!(
            unknowns.heard(at(6_000)),
            Unknown::After(Duration::from_millis(4_000))
        );
        assert!(unknowns.due(at(10_000)));
        assert_eq!(unknowns.heard(at(15_000)), Unknown::Now);
    }

    #[test]
    fn every_watch_is_dropped_on_inotify_only_and_at_most_once_a_second() {
        use notify::WatcherKind::{Inotify, PollWatcher};
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);

        let mut elsewhere = Rewatch::default();
        assert!(!elsewhere.now(PollWatcher, true, at(0)));

        let mut inotify = Rewatch::default();
        assert!(
            !inotify.now(Inotify, false, at(0)),
            "nothing was everything"
        );
        assert!(inotify.now(Inotify, true, at(0)));
        assert!(!inotify.now(Inotify, true, at(300)), "too soon: owed");
        assert!(!inotify.now(Inotify, false, at(900)), "still too soon");
        assert!(inotify.now(Inotify, false, at(1000)), "the owed drop, made");
        assert!(!inotify.now(Inotify, false, at(3000)), "nothing owed");
    }

    #[test]
    fn a_burst_is_told_as_unknown_when_anything_may_have_moved() {
        let root = Path::new("/home/dev/plane");
        let todo = root.join("workspaces/alpha/todos/a.md");
        let named = what_changed(root, None, &burst(&[&todo])).expect("placed");
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].kind, Kind::Todos);

        let everything = Burst {
            everything: true,
            ..Burst::default()
        };
        assert_eq!(what_changed(root, None, &everything), None);
    }

    #[test]
    fn a_root_opened_through_a_link_places_paths_the_disk_spells_its_own_way() {
        // macOS hands FSEvents paths as `/private/var/...` for a root opened as `/var/...`.
        let plane = plane_with_todos(&[]);
        let real = plane.path().canonicalize().expect("canonical");
        let links = tempfile::tempdir().expect("a place for the link");
        let link = links.path().join("plane");
        std::os::unix::fs::symlink(&real, &link).expect("a link to the plane");
        let todo = real.join("workspaces/alpha/todos/a.md");
        let written = burst(&[&todo]);

        assert_eq!(what_changed(&link, None, &written), None);
        let placed = what_changed(&link, Some(&real), &written).expect("placed");
        assert_eq!(placed[0].path, "workspaces/alpha/todos/a.md");
        assert_eq!(placed[0].workspace.as_deref(), Some("alpha"));
    }

    #[test]
    fn a_memory_an_agent_writes_in_a_workspace_is_told_as_that_workspaces_memory() {
        // The stale-data gap: `memory/` was not watched, so a memory an agent saved did not
        // reach the Memory panel until some other change happened to arrive.
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("workspaces/alpha/memory")).expect("a journal");
        let (_watch, told) = telling(&root);

        std::fs::write(root.join("workspaces/alpha/memory/note.md"), "# note\n").expect("a memory");

        let change = told_about(&told, "workspaces/alpha/memory/note.md");
        assert_eq!(change.kind, Kind::Memory);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));
    }

    #[test]
    fn a_session_record_and_a_persona_memory_are_told_by_what_they_are() {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("sessions")).expect("the root's records");
        std::fs::create_dir_all(root.join("workspaces/alpha/sessions")).expect("records");
        std::fs::create_dir_all(root.join("personas/steward/memory")).expect("a persona");
        let (_watch, told) = telling(&root);

        std::fs::write(root.join("sessions/r.md"), "# r\n").expect("a root record");
        let change = told_about(&told, "sessions/r.md");
        assert_eq!(change.kind, Kind::Sessions);
        assert_eq!(change.workspace, None);

        std::fs::write(root.join("workspaces/alpha/sessions/r.md"), "# r\n").expect("a record");
        let change = told_about(&told, "workspaces/alpha/sessions/r.md");
        assert_eq!(change.kind, Kind::Sessions);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));

        std::fs::write(root.join("personas/steward/memory/m.md"), "# m\n").expect("a memory");
        let change = told_about(&told, "personas/steward/memory/m.md");
        assert_eq!(change.kind, Kind::Memory);
        assert_eq!(change.persona.as_deref(), Some("steward"));
    }

    #[test]
    fn a_todo_closed_is_told_as_a_todo_of_its_workspace() {
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        let (_watch, told) = telling(&root);

        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");

        let change = told_about(&told, "workspaces/alpha/todos/m8-1.md");
        assert_eq!(change.kind, Kind::Todos);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));
    }

    #[test]
    fn the_watched_set_is_the_stores_the_panels_read_and_not_the_clones() {
        let plane = plane_with_todos(&[]);
        let root = plane.path();
        std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).expect("a clone");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc/src")).expect("its source");
        std::fs::create_dir_all(root.join("workspaces/.worktrees")).expect("charter's own");
        std::fs::create_dir_all(root.join("personas/steward/memory")).expect("a persona");
        std::fs::create_dir_all(root.join(".claude/agents")).expect("sub-agents");
        std::fs::create_dir_all(root.join(".claude/skills/x")).expect("a skill");
        std::fs::create_dir_all(root.join("workspaces/alpha/memory/archive")).expect("a journal");
        std::fs::create_dir_all(root.join("workspaces/alpha/sessions")).expect("records");
        std::fs::create_dir_all(root.join("sessions")).expect("the root's records");

        let mut got: Vec<_> = wanted(root)
            .into_iter()
            .map(|path| {
                path.strip_prefix(root)
                    .expect("inside")
                    .display()
                    .to_string()
            })
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                "",
                ".claude",
                ".claude/agents",
                "personas",
                "personas/steward",
                "personas/steward/memory",
                "sessions",
                "workspaces",
                "workspaces/alpha",
                "workspaces/alpha/memory",
                "workspaces/alpha/sessions",
                "workspaces/alpha/todos"
            ]
        );
    }

    #[test]
    fn a_plain_folder_in_a_workspace_is_watched_and_a_clone_a_store_or_a_dotted_one_is_not() {
        // A folder that is not a clone yet can become one (`git init`, a `.git` moved in), and
        // only a watch on the folder itself hears that (#1160).
        let plane = plane_with_todos(&[]);
        let root = plane.path();
        let alpha = root.join("workspaces/alpha");
        std::fs::create_dir_all(alpha.join("notes/deeper")).expect("a plain folder");
        std::fs::create_dir_all(alpha.join("svc/.git")).expect("a clone");
        std::fs::create_dir_all(alpha.join("tree")).expect("a worktree");
        std::fs::write(alpha.join("tree/.git"), "gitdir: elsewhere\n").expect("its .git file");
        std::fs::create_dir_all(alpha.join(".cache")).expect("a dotted folder");
        std::fs::write(alpha.join("workspace.json"), "{}\n").expect("a file");
        #[cfg(unix)]
        std::os::unix::fs::symlink(alpha.join("notes"), alpha.join("linked")).expect("a link");

        let got = wanted(root);
        assert!(got.contains(&alpha.join("notes")), "{got:?}");
        for not in [
            "svc",
            "tree",
            ".cache",
            "notes/deeper",
            "linked",
            "workspace.json",
        ] {
            assert!(!got.contains(&alpha.join(not)), "{not} is watched: {got:?}");
        }
    }

    /// `plane` with artifact `backlog` pinned in workspace alpha, and `draft` beside it, not.
    fn with_artifacts(plane: &Path) -> PathBuf {
        let at = plane.join("workspaces/alpha/artifacts");
        for folder in ["backlog/data/weeks", "backlog/css", "draft/data"] {
            std::fs::create_dir_all(at.join(folder)).expect("an artifact's folder");
        }
        std::fs::write(at.join("backlog/index.html"), "<h1>12</h1>").expect("a page");
        std::fs::write(
            at.join("pins.toml"),
            "version = 1\npin = [\n  { id = \"a00000000001\", folder = \"backlog\", name = \"Backlog\", icon = \"chart\", order = 1 },\n]\n",
        )
        .expect("the pins");
        at
    }

    #[test]
    fn a_pinned_artifact_s_folders_are_watched_and_an_unpinned_one_s_are_not() {
        let plane = plane_with_todos(&[]);
        let at = with_artifacts(plane.path());
        let got = wanted(plane.path());
        for folder in [
            "",
            "backlog",
            "backlog/data",
            "backlog/data/weeks",
            "backlog/css",
        ] {
            assert!(
                got.contains(&at.join(folder)),
                "{folder} is not watched: {got:?}"
            );
        }
        for not in ["draft", "draft/data"] {
            assert!(!got.contains(&at.join(not)), "{not} is watched: {got:?}");
        }
    }

    #[test]
    fn an_artifact_s_data_written_is_told_as_its_workspace_s_artifacts() {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        let at = with_artifacts(&root);
        let (_watch, told) = telling(&root);

        std::fs::write(at.join("backlog/data/weeks/45.json"), "[]").expect("the data");

        let change = told_about(
            &told,
            "workspaces/alpha/artifacts/backlog/data/weeks/45.json",
        );
        assert_eq!(change.kind, Kind::Artifacts);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));
    }

    #[test]
    fn the_done_todos_are_watched_where_there_are_any() {
        let plane = plane_with_todos(&[]);
        let root = plane.path();
        let done = root.join("workspaces/alpha/todos/archive");
        assert!(!wanted(root).contains(&done));

        std::fs::create_dir_all(&done).expect("a done todo's folder");

        assert!(wanted(root).contains(&done));
    }

    #[test]
    fn a_plain_folder_too_big_to_look_at_every_sweep_is_not_watched() {
        let plane = plane_with_todos(&[]);
        let root = plane.path();
        let alpha = root.join("workspaces/alpha");
        for (folder, count) in [("fits", PLAIN_FOLDER_MOST), ("big", PLAIN_FOLDER_MOST + 1)] {
            std::fs::create_dir_all(alpha.join(folder)).expect("a plain folder");
            for n in 0..count {
                std::fs::write(alpha.join(folder).join(n.to_string()), "").expect("an entry");
            }
        }

        let got = wanted(root);
        assert!(got.contains(&alpha.join("fits")), "{got:?}");
        assert!(!got.contains(&alpha.join("big")), "{got:?}");
    }

    #[test]
    fn a_folder_in_a_workspace_that_becomes_a_clone_is_told_as_that_workspaces_change() {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc")).expect("a plain folder");
        let (_watch, told) = telling(&root);

        std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).expect("git init");

        let change = told_about(&told, "workspaces/alpha/svc/.git");
        assert_eq!(change.kind, Kind::Workspace);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));
    }

    #[test]
    fn the_window_is_told_which_answers_a_change_concerns_and_every_one_when_it_is_not_known() {
        let plane = id(Path::new("/home/dev/plane"));
        let memory = planechange::classify(
            Path::new("/home/dev/plane"),
            Path::new("/home/dev/plane/workspaces/alpha/memory/m.md"),
        )
        .expect("placed");
        let told = PlaneChanged::of(plane.clone(), Some(vec![memory]));
        assert_eq!(
            told.answers,
            Some(vec![
                PlaneAnswer::Panels {
                    workspace: Some("alpha".to_owned())
                },
                PlaneAnswer::Views
            ])
        );
        assert_eq!(
            serde_json::to_value(&told.answers).expect("serialisable"),
            serde_json::json!([{"answer": "panels", "workspace": "alpha"}, {"answer": "views"}])
        );

        let unknown = PlaneChanged::of(plane, None);
        assert_eq!(unknown.changes, None);
        assert_eq!(unknown.answers, None);
    }

    // #5499: on one stream. A per-folder watch on FSEvents restarted the stream for every
    // folder it added, which took 376.8 s at the scale spec's size (#5486). One stream is one
    // watch whatever the size, and the folders it reports are kept to here.

    /// The `relative` paths of every change `told` names until it has been quiet for
    /// [`QUIET_FOR`] × 4, or `None` once one batch was not known (everything).
    fn told_paths(told: &mpsc::Receiver<(PlaneId, What)>) -> Option<Vec<String>> {
        let mut paths = Vec::new();
        while let Ok((_, what)) = told.recv_timeout(QUIET_FOR * 4) {
            paths.extend(what?.into_iter().map(|change| change.path));
        }
        paths.sort();
        Some(paths)
    }

    /// A plane that is a repository with one commit, at its real path.
    fn plane_in_git() -> (tempfile::TempDir, PathBuf) {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        git(&root, &["init", "-q", "-b", "main", "."]);
        git(&root, &["config", "user.email", "t@example.invalid"]);
        git(&root, &["config", "user.name", "t"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "one"]);
        (plane, root)
    }

    #[test]
    fn on_one_stream_an_edit_anywhere_in_the_tree_is_in_the_next_shared_standing() {
        // #5505: one stream hears the whole project, so a move deep in a folder no panel reads
        // still makes the shared git standing read again, and nothing waits on a clock.
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind};
        let (_plane, root) = plane_in_git();
        let (watch, _told) = looking_in::<Raw>(&root, &Windows::new(), NEVER, Mode::OneStream);
        assert!(purlis_core::planegit::covered(&root));
        assert!(
            purlis_core::planegit::shared_standing(&root)
                .changed
                .is_empty()
        );

        let deep = root.join("docs/guides/deep.md");
        std::fs::create_dir_all(deep.parent().unwrap()).expect("a folder");
        std::fs::write(&deep, "deep").expect("a file");
        raw(EventKind::Create(CreateKind::File), &deep);

        assert_eq!(
            purlis_core::planegit::shared_standing(&root).changed,
            vec!["docs/guides/deep.md".to_owned()]
        );
        drop(watch);
        assert!(!purlis_core::planegit::covered(&root));
    }

    #[test]
    fn on_one_stream_a_move_in_gits_own_folder_or_a_clone_does_not_make_the_standing_read_again() {
        // The standing's own stamp holds git's files, and a clone's moves are never in the
        // project's status: a build in a clone costs the project no `status`.
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind};
        let (_plane, root) = plane_in_git();
        let clone = root.join("workspaces/alpha/thing");
        std::fs::create_dir_all(clone.join(".git")).expect("a clone");
        let (_watch, _told) = looking_in::<Raw>(&root, &Windows::new(), NEVER, Mode::OneStream);
        purlis_core::planegit::shared_standing(&root);

        for moved in [
            clone.join("target/debug/out.o"),
            root.join(".git/fsmonitor--daemon/cookies/1"),
            root.join(".purlis/app/spool/1"),
        ] {
            raw(EventKind::Create(CreateKind::File), &moved);
        }

        // Still the answer read before: written into the tree behind the watch's back, a new
        // file is not seen while nothing it hears says the tree moved.
        std::fs::write(root.join("unheard.md"), "x").expect("a file");
        assert!(
            purlis_core::planegit::shared_standing(&root)
                .changed
                .is_empty()
        );
    }

    #[test]
    fn on_one_stream_the_whole_project_is_one_recursive_watch_on_its_root() {
        use crate::watchset::raw::{self, Raw, raw};
        use notify::event::{CreateKind, EventKind};
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("workspaces/alpha/memory")).expect("a journal");
        let (_watch, told) = looking_in::<Raw>(&root, &Windows::new(), NEVER, Mode::OneStream);

        std::fs::create_dir_all(root.join("workspaces/beta/todos")).expect("a new workspace");
        raw(
            EventKind::Create(CreateKind::Folder),
            &root.join("workspaces/beta"),
        );
        told_about(&told, "workspaces/beta");

        assert_eq!(raw::watched_recursively(&root), 1);
        assert_eq!(
            raw::watched(&root),
            1,
            "the root was watched more than once"
        );
        for folder in [
            "workspaces",
            "workspaces/alpha",
            "workspaces/alpha/todos",
            "workspaces/alpha/memory",
            "workspaces/beta",
            "workspaces/beta/todos",
        ] {
            let folder = root.join(folder);
            assert_eq!(
                raw::watched(&folder),
                0,
                "{} was watched on its own",
                folder.display()
            );
        }
    }

    #[test]
    fn on_one_stream_a_change_inside_a_clone_is_not_told_and_a_memory_beside_it_is() {
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind};
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).expect("a clone");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc/target")).expect("its build");
        std::fs::create_dir_all(root.join("workspaces/alpha/memory")).expect("a journal");
        let (_watch, told) = looking_in::<Raw>(&root, &Windows::new(), NEVER, Mode::OneStream);

        for built in ["svc/target/debug.rlib", "svc/src/lib.rs", "svc/.git/index"] {
            raw(
                EventKind::Create(CreateKind::File),
                &root.join("workspaces/alpha").join(built),
            );
        }
        assert!(
            told.recv_timeout(QUIET_FOR * 4).is_err(),
            "a clone's own writes were told"
        );

        raw(
            EventKind::Create(CreateKind::File),
            &root.join("workspaces/alpha/svc/target/again.rlib"),
        );
        raw(
            EventKind::Create(CreateKind::File),
            &root.join("workspaces/alpha/memory/note.md"),
        );
        assert_eq!(
            told_paths(&told),
            Some(vec!["workspaces/alpha/memory/note.md".to_owned()])
        );
    }

    #[test]
    fn on_one_stream_dropped_events_are_one_reconcile_however_many_the_platform_reports() {
        use crate::watchset::raw::{Raw, event};
        use notify::event::{EventKind, Flag};
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        let windows = Windows::new();
        let (_watch, told) = looking_in::<Raw>(&root, &windows, SOON, Mode::OneStream);

        for _ in 0..3 {
            event(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan));
        }
        let (_, what) = told.recv_timeout(PATIENCE).expect("told to reconcile");
        assert_eq!(what, None, "a reconcile reads everything again");
        assert!(
            told.recv_timeout(TOLD_BY_NOW).is_err(),
            "reconciled more than once"
        );
    }

    #[test]
    fn on_one_stream_a_rescan_soon_after_one_told_is_told_once_the_gap_is_over() {
        use crate::watchset::raw::{Raw, event};
        use notify::event::{EventKind, Flag};
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        let windows = Windows::new();
        let (_watch, told) = looking_in::<Raw>(&root, &windows, SOON, Mode::OneStream);

        event(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan));
        let first = told
            .recv_timeout(PATIENCE)
            .expect("the first is told at once");
        assert_eq!(first.1, None);
        let heard = Instant::now();
        // Two more, each a burst of its own, both well inside the gap.
        for _ in 0..2 {
            std::thread::sleep(LONGEST);
            event(notify::Event::new(EventKind::Other).set_flag(Flag::Rescan));
        }
        let (_, what) = told
            .recv_timeout(UNKNOWN_EVERY + PATIENCE)
            .expect("the ones folded are told");
        assert_eq!(what, None);
        assert!(
            heard.elapsed() >= UNKNOWN_EVERY,
            "told {:?} after the first, inside the gap",
            heard.elapsed()
        );
        assert!(
            told.recv_timeout(TOLD_BY_NOW).is_err(),
            "the ones folded were told more than once"
        );
    }

    #[test]
    fn on_one_stream_a_memory_a_session_record_and_a_persona_memory_reach_their_panels() {
        // On the watcher the tests play the platform with (#577): a recursive watch of the
        // poller on macOS, of inotify on Linux.
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        for store in [
            "sessions",
            "workspaces/alpha/memory",
            "workspaces/alpha/sessions",
            "personas/steward/memory",
            "personas/_shared/memory",
        ] {
            std::fs::create_dir_all(root.join(store)).expect("a store");
        }
        let (_watch, told) = looking_in::<Source>(&root, &Windows::new(), NEVER, Mode::OneStream);

        for (path, kind) in [
            ("workspaces/alpha/memory/note.md", Kind::Memory),
            ("workspaces/alpha/sessions/r.md", Kind::Sessions),
            ("sessions/r.md", Kind::Sessions),
            ("personas/steward/memory/m.md", Kind::Memory),
            ("personas/_shared/memory/s.md", Kind::Memory),
        ] {
            std::fs::write(root.join(path), "# written\n").expect("a write");
            assert_eq!(told_about(&told, path).kind, kind, "{path}");
        }
    }

    // #756, then #5499: the watch's own looks. They once read every watched folder every five
    // seconds, for FSEvents' late events; at the scale spec's size that was some 200,000
    // `lstat`s a look. Now a folder the platform watches is never looked at, and the looks keep
    // to the folders it refused. Each test's watch is fed by `Raw`, which reports nothing the
    // test does not send, so a write on disk is one whose event never came.

    /// How often the looks of a test that waits on them look.
    const SOON: Duration = Duration::from_millis(100);

    /// Long enough for a look that was going to tell to have told.
    const TOLD_BY_NOW: Duration = Duration::from_secs(3);

    /// A plane whose `alpha` todos the platform refuses to watch, with `todos` in them.
    fn plane_refusing_alpha_todos(todos: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let plane = plane_with_todos(todos);
        let root = plane.path().canonicalize().expect("canonical");
        crate::watchset::refuse::refuse(&root.join("workspaces/alpha/todos"));
        (plane, root)
    }

    #[test]
    fn a_folder_the_platform_watches_is_never_looked_at_on_either_mode() {
        use crate::watchset::raw::Raw;
        for mode in [Mode::PerFolder, Mode::OneStream] {
            let plane = plane_with_todos(&["m8-1"]);
            let root = plane.path().canonicalize().expect("canonical");
            let windows = Windows::new();
            let (_watch, told) = looking_in::<Raw>(&root, &windows, SOON, mode);
            // Past a look or two, so a sweep would have something to compare against.
            std::thread::sleep(SOON * 3);

            std::fs::write(root.join("workspaces/alpha/todos/m8-1.md"), "# moved\n")
                .expect("a write");
            std::fs::write(root.join("workspaces/alpha/todos/m8-2.md"), "# new\n").expect("a todo");
            windows.focused();

            assert!(
                told.recv_timeout(TOLD_BY_NOW).is_err(),
                "{mode:?}: a look swept a watched folder"
            );
            assert_eq!(windows.missed(&root).map(|missed| missed.count), Some(0));
        }
    }

    #[test]
    fn only_the_folder_the_platform_refused_is_looked_at_on_a_timer() {
        // inotify out of watches (#5499): the refused folder falls back to a look of its own,
        // and the rest of the project goes on being heard, not looked at.
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1"]);
        std::fs::create_dir_all(root.join("workspaces/alpha/memory")).expect("a journal");
        let windows = Windows::new();
        let (watch, told) = looking_on::<Raw>(&root, &windows, SOON);
        assert_eq!(watch.standing(), Standing::Partial);
        std::thread::sleep(SOON * 3);

        std::fs::write(root.join("workspaces/alpha/memory/note.md"), "# note\n").expect("a memory");
        std::fs::write(root.join("workspaces/alpha/todos/m8-2.md"), "# new\n").expect("a todo");

        let change = told_about(&told, "workspaces/alpha/todos/m8-2.md");
        assert_eq!(change.kind, Kind::Todos);
        assert!(
            told.recv_timeout(TOLD_BY_NOW).is_err(),
            "a look told more than the refused folder"
        );
        assert_eq!(windows.missed(&root).map(|missed| missed.count), Some(1));
    }

    #[test]
    fn a_write_in_a_refused_folder_is_told_when_a_window_is_focused() {
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1"]);
        let windows = Windows::new();
        let (_watch, told) = looking_on::<Raw>(&root, &windows, NEVER);

        std::fs::write(
            root.join("workspaces/alpha/todos/m8-1.md"),
            "# m8-1, written in a terminal\n",
        )
        .expect("a write");
        assert!(
            told.recv_timeout(QUIET_FOR * 4).is_err(),
            "told with no event and no focus"
        );
        windows.focused();

        let change = told_about(&told, "workspaces/alpha/todos/m8-1.md");
        assert_eq!(change.kind, Kind::Todos);
        assert_eq!(change.workspace.as_deref(), Some("alpha"));
        assert_eq!(windows.missed(&root).map(|missed| missed.count), Some(1));
    }

    #[test]
    fn a_removal_in_a_refused_folder_is_told_by_the_next_look() {
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1", "m8-2"]);
        let windows = Windows::new();
        let (_watch, told) = looking_on::<Raw>(&root, &windows, SOON);

        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");

        let change = told_about(&told, "workspaces/alpha/todos/m8-1.md");
        assert_eq!(change.kind, Kind::Todos);
        let missed = windows.missed(&root).expect("watched");
        assert_eq!(missed.count, 1);
        assert!(missed.last.is_some());
    }

    #[test]
    fn nothing_is_looked_at_while_every_window_is_hidden_and_a_focus_looks_at_once() {
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1"]);
        let windows = Windows::new();
        windows.shown_when(|| false);
        let (_watch, told) = looking_on::<Raw>(&root, &windows, SOON);
        // The first look's turn finds every window hidden, and stops looking.
        std::thread::sleep(SOON * 3);

        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");
        assert!(
            told.recv_timeout(TOLD_BY_NOW).is_err(),
            "looked while every window was hidden"
        );
        assert_eq!(windows.missed(&root).map(|missed| missed.count), Some(0));

        windows.focused();
        told_about(&told, "workspaces/alpha/todos/m8-1.md");
    }

    #[test]
    fn a_storm_of_focus_changes_is_one_look() {
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1"]);
        let windows = Windows::new();
        let (_watch, told) = looking_on::<Raw>(&root, &windows, NEVER);

        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");
        for _ in 0..20 {
            windows.focused();
        }

        told_about(&told, "workspaces/alpha/todos/m8-1.md");
        assert!(
            told.recv_timeout(TOLD_BY_NOW).is_err(),
            "told more than once"
        );
        assert_eq!(windows.missed(&root).map(|missed| missed.count), Some(1));
    }

    #[test]
    fn a_workspace_a_look_found_in_a_refused_folder_is_watched_from_then_on() {
        use crate::watchset::raw::Raw;
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        crate::watchset::refuse::refuse(&root.join("workspaces"));
        let windows = Windows::new();
        let (_watch, told) = looking_on::<Raw>(&root, &windows, SOON);

        std::fs::create_dir_all(root.join("workspaces/beta/todos")).expect("a new workspace");
        told_about(&told, "workspaces/beta");

        assert!(
            crate::watchset::raw::watched(&root.join("workspaces/beta/todos")) > 0,
            "the new workspace's todos are not watched"
        );
    }

    #[test]
    fn a_dropped_watch_is_no_longer_counted_and_its_looks_tell_nothing() {
        use crate::watchset::raw::Raw;
        let (_plane, root) = plane_refusing_alpha_todos(&["m8-1"]);
        let windows = Windows::new();
        let (watch, told) = looking_on::<Raw>(&root, &windows, SOON);
        assert_eq!(windows.missed(&root), Some(Missed::default()));

        drop(watch);
        std::fs::remove_file(root.join("workspaces/alpha/todos/m8-1.md")).expect("closed");
        windows.focused();

        assert_eq!(windows.missed(&root), None);
        assert!(told.recv_timeout(TOLD_BY_NOW).is_err());
    }

    /// A project watched again before its old watch is dropped is still counted once the old
    /// one goes: the doctor's row does not say it is not watched.
    #[test]
    fn a_watch_started_again_is_counted_after_the_old_one_is_dropped() {
        use crate::watchset::raw::Raw;
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        let windows = Windows::new();
        let (old, _) = looking_on::<Raw>(&root, &windows, NEVER);
        let (_new, _) = looking_on::<Raw>(&root, &windows, NEVER);

        drop(old);

        assert_eq!(windows.missed(&root), Some(Missed::default()));
    }

    #[test]
    fn a_look_compares_a_folder_by_its_being_there_and_a_file_by_its_time_and_length() {
        // A clone's own writes move its folder's time, and no non-recursive watch tells them.
        let plane = plane_with_todos(&["m8-1"]);
        let root = plane.path().canonicalize().expect("canonical");
        std::fs::create_dir_all(root.join("workspaces/alpha/svc")).expect("a clone");
        let dirs = [
            root.join("workspaces/alpha"),
            root.join("workspaces/alpha/todos"),
        ];
        let before = look(&dirs);

        std::fs::write(root.join("workspaces/alpha/svc/README.md"), "busy\n").expect("a write");
        std::fs::write(root.join("workspaces/alpha/todos/m8-1.md"), "# changed\n").expect("a todo");
        std::fs::write(root.join("workspaces/alpha/todos/m8-2.md"), "# new\n").expect("a todo");
        let (changed, removed) = differ(&before, &look(&dirs));

        let todos = root.join("workspaces/alpha/todos");
        assert_eq!(
            changed,
            HashSet::from([todos.join("m8-1.md"), todos.join("m8-2.md")])
        );
        assert!(removed.is_empty());

        let before = look(&dirs);
        std::fs::remove_file(todos.join("m8-2.md")).expect("closed");
        let (changed, removed) = differ(&before, &look(&dirs));
        assert_eq!(changed, HashSet::from([todos.join("m8-2.md")]));
        assert_eq!(removed, HashSet::from([todos.join("m8-2.md")]));
    }

    /// The cost of one look on a large project, for the PR (#756, SC-18). Not a check: a
    /// number. `cargo test -p purlis-app --lib planewatch::tests::what_a_look_costs -- --ignored
    /// --nocapture` prints it.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn what_a_look_costs() {
        let plane = plane_with_todos(&[]);
        let root = plane.path().canonicalize().expect("canonical");
        for ws in 0..30 {
            for store in ["todos", "memory", "sessions"] {
                let dir = root.join(format!("workspaces/w{ws}/{store}"));
                std::fs::create_dir_all(&dir).expect("a store");
                for n in 0..20 {
                    std::fs::write(dir.join(format!("{n}.md")), "# x\n").expect("a file");
                }
            }
        }
        let dirs: Vec<PathBuf> = wanted(&root).into_iter().collect();
        let entries: usize = look(&dirs).values().map(HashMap::len).sum();
        let rounds = 200;
        let started = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(look(&dirs));
        }
        println!(
            "one look: {} folders, {entries} entries, {:?}",
            dirs.len(),
            started.elapsed() / rounds
        );
    }
}
