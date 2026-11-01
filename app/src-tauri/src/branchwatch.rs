// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Tells a window when anything in a branch it shows the changes of moves (FM-4, #1107): an
//! agent's first write into a folder nobody opened is heard, and the branch's change markers
//! are read again without a refresh.
//!
//! **Not the tree's watch** (`filewatch.rs`), which watches exactly the folders a window has
//! open so it can read each again. This one listens to the whole branch, and says only *which
//! branch* moved: the window reads its status again.
//!
//! **What the whole branch costs depends on the platform**, so it is watched two ways
//! (`purlis_core::files::Root`):
//! - where one watch covers a tree (FSEvents on macOS, `ReadDirectoryChangesW` on Windows), the
//!   branch's folder, recursively. Adding folders one at a time there restarts the stream each
//!   time, measured at about 6 ms a folder on macOS, so a branch of 2,000 folders would take 12
//!   seconds to watch; one recursive watch takes milliseconds.
//! - where a recursive watch is one per folder (inotify), the folders git knows, each
//!   non-recursively, so `node_modules/` and `target/` cost nothing. A folder created in one is
//!   added when it appears.
//!
//! **A move that cannot change the status is not told**: one only inside what git ignores (a
//! build writing `target/`), or inside git's own folder. Asked once per branch per burst
//! (`Root::matters`), of the bounded reader (`purlis_core::files::Reader`, D-88h), and
//! **never on the watch's own thread**: each branch's check runs on a worker of its own, one at
//! a time, with what moved meanwhile asked next. A branch whose check hangs (an ignore file that
//! is a FIFO) holds only its own worker until the reader's deadline, which counts as "it
//! matters", and every other branch's markers keep moving.
//!
//! **A branch the reader gave no answer for is said, and asked again** (#1745). Finding a
//! branch asks the bounded reader, which may stay busy past every retry, pause the branch, or
//! give no answer within its deadline. Such a branch is not listened to yet, and the window is
//! told so through the folder watch's list (`filewatch.rs`, `files-unwatched`): the explorer says
//! its changes are not shown yet. It is asked for again once the pause is over, or about a reader
//! deadline later, on a thread of its own for as long as the window keeps the set that named it;
//! once found it is listened to and told as moved, so the window reads its status again —
//! whatever changed before its watch held was heard by nothing, and the window's own read of a
//! newly open branch is made when the set is answered, not when the watch holds.
//!
//! **The cockpit's refs are heard too** (#1152). A commit that writes no file in the branch's
//! folder — `git commit --amend --no-edit`, a commit of what was staged — moves only git's own
//! folder, which the checks above pass over. So for the branch a window's sidebar is focused on
//! ([`BranchWatch::focus`]), and only that one, the folders holding its refs
//! (`purlis_core::files::Root::refs`) are watched one by one, and a burst naming one of those
//! files tells the window as a move in the folder would. A burst naming the cockpit's `HEAD` (a
//! checkout in its folder) has its refs found again, off the watch's thread, so the branch
//! checked out now is the one whose ref is watched; once it is, the window is told the cockpit
//! moved, since a commit made before that watch held was heard by nothing (#2463). A ref that
//! moved touches no shared save standing (`purlis_core::planegit::touch`): the standing's own
//! stamp holds the clone's `HEAD` and refs (`purlis_core::standings::Stamp`), so its next read
//! has the move anyway (D-1152-6).
//!
//! **A move in a repo skill's folder is told apart** (#4377): [`SKILLS_MOVED`], for the Skills view,
//! which reads a whole set again on it and so must not on an agent's every other write. It is told
//! from the burst itself, on the watch's thread, since it reads nothing: before git is asked, as a
//! repo may ignore the folder its harness still reads skills from, and before a check that hangs.
//! A branch watched folder by folder (inotify) has no watch on a folder git ignores, so there such
//! a folder's moves are not heard, as they are not by the rest of this watch.
//!
//! **Every burst is told by what it named** ([`crate::watchset::bursts`], #1139): a file made
//! and removed inside one is still a move. A burst that is everything — the platform lost
//! track, a watcher error, more paths than a burst holds — moves every branch listened to, and
//! may have made a folder, so a branch watched folder by folder is listed again.
//!
//! **The one watch per repo** (FD-11, #651). A clone whose save standing is shared
//! (`purlis_core::reposave::shared_standing`: auto-save's look, the Saving rows) asks to be
//! watched here too ([`BranchWatch::want`]), so the explorer's markers and the standing hear
//! one watch and one check per burst, not one each. A move that matters touches the
//! standing (`purlis_core::planegit::touch`) as well as telling the windows, and while the
//! clone's whole tree is watched its standing is *covered*: read again on what moved, and
//! otherwise only on its long backstop — about one read in twenty minutes for a monorepo
//! nobody touches (`purlis_core::standings::WATCHED_SHARE`). Its plane let go of, it is let
//! go of too.
//!
//! **Only a watch of the whole tree covers** (D-FD11h, fail closed): FSEvents and
//! `ReadDirectoryChangesW`, [`How::Whole`]. A clone watched folder by folder (inotify) is never
//! covered, and keeps the standing's clock: a folder deleted and made again, a folder made
//! between a listing and its watch, or folders that could not be listed ([`How::Unread`]) all
//! leave edits no watch reports. A burst that says the platform lost track or erred
//! ([`crate::watchset::Burst::lost`]) uncovers at once every clone whose tree it concerns, and
//! watches each again: the trees the rescan's named paths lie in or above, or every tree when
//! it named none, or one in none of them ([`lost_trees`]); a clone is covered again only once
//! that watch has been made, and its standing is read again then. A run of losses (a build
//! overflowing inotify's queue) watches the trees again at most once in
//! [`WATCHED_AGAIN_EVERY`] (#874).
//!
//! **A watcher that cannot be made is not made again at once** (#874): a clone the standing asks
//! for, and a branch found late, wait [`REFUSED_FOR`] before trying again, as for a clone the
//! reader will not find. A window's own set tries at once.
//!
//! **Nothing here walks a tree.** Nothing keeps a file-id cache, and links are not followed: a
//! link an agent puts in its branch to the operator's home is never read through.
//! On inotify, the folders a branch is listed again with as folders appear are listed at most
//! once a second, and the app watches at most a quarter of the user's inotify watches.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak, mpsc};
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use purlis_core::files::{Branch, Reader, Root};
use purlis_core::standings::Wanted;

use crate::planes::{PlaneId, Planes};

/// The event a window is sent.
pub(crate) const CHANGED: &str = "branch-changed";

/// The event a window is sent when a repo skill's folder in a branch moved, with what
/// [`CHANGED`] carries.
pub(crate) const SKILLS_MOVED: &str = "repo-skills-moved";

/// How long a burst is folded for, as the tree's watch folds it.
const QUIET_FOR: Duration = Duration::from_millis(250);

/// The most moved paths one burst holds, across every branch. Past it the burst is everything
/// and every branch is read again; kept well above what one check asks
/// (`purlis_core::files::ASKED`), so a burst inside git's own folder or what it ignores is
/// sorted rather than told.
const MOST_PATHS: usize = 4096;

/// The most branches one window listens to at once.
pub const BRANCHES: usize = 32;

/// A branch, as the window names it: the plane, the workspace, the repo and the piece (none for
/// the repo's own folder).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WatchedBranch {
    pub plane: PlaneId,
    pub workspace: String,
    pub repo: String,
    pub piece: Option<String>,
}

/// What `branch-changed` carries: the branches, of those this window listens to, that moved.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct BranchChanged {
    pub branches: Vec<WatchedBranch>,
}

/// Told which window's branches moved.
pub type Told = Arc<dyn Fn(&str, Vec<WatchedBranch>) + Send + Sync + 'static>;

/// How one branch is listened to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// Its folder, recursively.
    Whole,
    /// These folders, each non-recursively, refreshed as folders appear.
    Folders(Vec<PathBuf>),
    /// Folder by folder, but the folders git knows could not be listed (the reader refused or
    /// ran out of time): only the branch's own folder, non-recursively, until a listing works.
    Unread,
}

/// The platform's way: [`How::Whole`] where one watch covers a tree.
pub fn how(root: &Root, reader: &Reader) -> How {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        How::Whole
    } else {
        listed(root, reader)
    }
}

/// [`How::Folders`] as the reader lists them now, or [`How::Unread`].
fn listed(root: &Root, reader: &Reader) -> How {
    root.folders(reader).map_or(How::Unread, How::Folders)
}

/// One branch a window listens to.
#[derive(Clone)]
struct Listened {
    branch: WatchedBranch,
    root: Root,
    how: How,
}

struct Inner<W: notify::Watcher> {
    watcher: Option<W>,
    /// Told which window's branches moved a repo skill ([`SKILLS_MOVED`]).
    skills_told: Option<Told>,
    by_window: HashMap<String, Vec<Listened>>,
    newest: HashMap<String, u64>,
    watched: HashMap<PathBuf, RecursiveMode>,
    /// When each branch's folders were last listed again, by its folder, and whether a listing
    /// is already waiting for its turn: at most one a second (`RELISTED_EVERY`).
    relisted: HashMap<PathBuf, (Instant, bool)>,
    /// The most folders watched one by one, app-wide (`inotify_budget`).
    budget: usize,
    /// Each branch's check of what moved, by its folder: what is waiting to be asked, whether a
    /// folder appeared among it, and whether a worker is asking now.
    checking: HashMap<PathBuf, Check>,
    /// The clones listened to for their shared standing (FD-11), by the folder the standing
    /// names them by.
    kept: HashMap<PathBuf, Kept>,
    /// The clones being found, by that folder: asked once however often the standing asks.
    finding: HashSet<PathBuf>,
    /// The clones whose whole tree is watched now, which the standing has been told it covers.
    covering: HashSet<PathBuf>,
    /// How many times each plane has been let go of: a clone found for a plane let go of
    /// while it was being found is not kept.
    let_go: HashMap<PathBuf, u64>,
    /// Clones the reader would not find, and until when they are not asked for again.
    refused: HashMap<PathBuf, Instant>,
    /// Each window's cockpit (FM-5): the one branch whose refs are watched too (#1152).
    cockpits: HashMap<String, WatchedBranch>,
    /// The cockpits' folders whose refs are being found again after their `HEAD` moved, by
    /// folder, and whether it moved again meanwhile ([`find_refs_again`]).
    refinding: HashMap<PathBuf, bool>,
    /// For a test: each re-find of refs waits for this to open before it reads ([`Latch`]).
    #[cfg(test)]
    refind_held: Option<Latch>,
    /// Until when no watcher is made again on the watch's own account, after one could not be
    /// made (#874): a clone the standing asks for, and a branch found late, wait it out.
    unmade_until: Option<Instant>,
    /// When the kept clones' trees were last watched again after the platform lost track, and
    /// whether another is waiting (#874, L11).
    rewatching: Rewatching,
    /// The kept clones' trees a loss was heard in since they were last watched: none of them is
    /// covered until it is watched again (D-FD11h).
    stale: HashSet<PathBuf>,
}

/// How often, at most, the kept clones' trees are watched again after the platform lost track
/// (#874, L11). Under a build that overflows inotify's queue a loss is heard every burst, four a
/// second; each re-watch of a whole tree costs a stream restart. Every loss still uncovers the
/// clones at once, so a standing only waits on its own clock meanwhile.
const WATCHED_AGAIN_EVERY: Duration = Duration::from_secs(2);

/// When the trees were last watched again after a loss, and whether a re-watch is waiting for
/// [`WATCHED_AGAIN_EVERY`] to be over.
#[derive(Debug, Default)]
struct Rewatching {
    last: Option<Instant>,
    waiting: bool,
}

/// What a loss heard at some moment asks of the trees.
#[derive(Debug, PartialEq, Eq)]
enum Again {
    /// Watch them again now.
    Now,
    /// Watch them again after this long, once, for every loss heard until then.
    After(Duration),
    /// A re-watch is waiting already, and takes this loss too.
    Waiting,
}

impl Rewatching {
    /// A loss heard at `now`, with re-watches at most `every` apart.
    fn lost(&mut self, now: Instant, every: Duration) -> Again {
        if self.waiting {
            return Again::Waiting;
        }
        match self.last {
            Some(last) if now.duration_since(last) < every => {
                self.waiting = true;
                Again::After(every - now.duration_since(last))
            }
            _ => {
                self.last = Some(now);
                Again::Now
            }
        }
    }

    /// The waiting re-watch was made at `now`.
    fn done(&mut self, now: Instant) {
        self.last = Some(now);
        self.waiting = false;
    }
}

/// How long a clone the reader would not find is left alone before it is asked for again: the
/// standing asks on every read, and each ask would start a reader.
const REFUSED_FOR: Duration = Duration::from_secs(60);

impl<W: notify::Watcher> Inner<W> {
    /// Every branch and clone listened to, with how: the windows' and the standing's.
    fn listened(&self) -> impl Iterator<Item = (&Root, &How)> {
        self.by_window
            .values()
            .flatten()
            .map(|one| (&one.root, &one.how))
            .chain(self.kept.values().map(|kept| (&kept.root, &kept.how)))
    }

    /// Whether a watcher runs, made with `start` now when none does (#874, L9). One that could
    /// not be made is not made again for [`REFUSED_FOR`]: a clone the standing asks for on
    /// every read, or each branch found late, would otherwise try on every ask. A window's own
    /// set does not wait: it makes one in [`BranchWatch::set_from`], as the person asked.
    fn started(&mut self, start: impl FnOnce() -> Result<W, String>) -> bool {
        if self.watcher.is_some() {
            return true;
        }
        let now = Instant::now();
        if self.unmade_until.is_some_and(|until| now < until) {
            return false;
        }
        match start() {
            Ok(watcher) => {
                self.watcher = Some(watcher);
                self.unmade_until = None;
                true
            }
            Err(_) => {
                self.unmade_until = Some(now + REFUSED_FOR);
                false
            }
        }
    }
}

/// A clone listened to for its shared standing.
#[derive(Clone)]
struct Kept {
    plane: PathBuf,
    root: Root,
    how: How,
}

/// One branch's check of what moved in it.
#[derive(Default)]
struct Check {
    moved: Vec<PathBuf>,
    made_folder: bool,
    running: bool,
}

/// How often a branch's folders are listed again as folders appear in it, at most: an agent
/// making folders without pause lists them once a second, not on every burst.
const RELISTED_EVERY: Duration = Duration::from_secs(1);

/// How many folders the app watches one by one, at most, across every window and branch.
///
/// On inotify each one is a watch from the user's whole budget
/// (`/proc/sys/fs/inotify/max_user_watches`, 8,192 on older kernels), which the operator's
/// editor and every other watcher share: charter takes a quarter of it. Elsewhere a branch is
/// watched whole, and this does not bind.
fn inotify_budget() -> usize {
    if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
            .ok()
            .and_then(|text| text.trim().parse::<usize>().ok())
            .map_or(2048, |max| (max / 4).max(256))
    } else {
        usize::MAX
    }
}

/// Every window's listened branches. Managed by the app; dropping it stops every watch.
pub struct BranchWatch<W: notify::Watcher = notify::RecommendedWatcher> {
    inner: Arc<Mutex<Inner<W>>>,
    told: Told,
    reader: Reader,
    config: notify::Config,
    tickets: std::sync::atomic::AtomicU64,
    /// How a clone kept for its standing is listened to: [`how`], the platform's way.
    how_of: fn(&Root, &Reader) -> How,
}

impl BranchWatch {
    /// A watch that tells `told`, on the platform's own watcher.
    pub fn new(told: Told, reader: Reader) -> Self {
        Self::with_config(told, reader, notify::Config::default())
    }
}

impl<W: notify::Watcher + Send + 'static> BranchWatch<W> {
    fn with_config(told: Told, reader: Reader, config: notify::Config) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                watcher: None,
                skills_told: None,
                by_window: HashMap::new(),
                newest: HashMap::new(),
                watched: HashMap::new(),
                relisted: HashMap::new(),
                budget: inotify_budget(),
                checking: HashMap::new(),
                kept: HashMap::new(),
                finding: HashSet::new(),
                covering: HashSet::new(),
                let_go: HashMap::new(),
                refused: HashMap::new(),
                cockpits: HashMap::new(),
                refinding: HashMap::new(),
                #[cfg(test)]
                refind_held: None,
                unmade_until: None,
                rewatching: Rewatching::default(),
                stale: HashSet::new(),
            })),
            told,
            reader,
            config,
            tickets: std::sync::atomic::AtomicU64::new(0),
            how_of: how,
        }
    }

    /// The same watch, telling `told` of each move in a repo skill's folder ([`SKILLS_MOVED`]).
    pub fn telling_skills(self, told: Told) -> Self {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .skills_told = Some(told);
        self
    }

    /// The same watch, whose re-finds of refs wait for `latch` to open: for a test to order a
    /// re-find after what the watch's own thread tells.
    #[cfg(test)]
    fn holding_refinds(self, latch: Latch) -> Self {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refind_held = Some(latch);
        self
    }

    /// The same watch, listening to the clones kept for their standing `how_of`'s way: for a
    /// test to try either way on any platform.
    #[cfg(test)]
    fn keeping_clones(mut self, how_of: fn(&Root, &Reader) -> How) -> Self {
        self.how_of = how_of;
        self
    }

    /// A number for a set a window asks for, larger than every one before it.
    pub fn ticket(&self) -> u64 {
        self.tickets
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1)
    }

    /// `window` now listens to `branches`, each with its resolved folder and how; unless it
    /// has since asked for a newer set. Whether the set was taken.
    pub fn set_from(
        &self,
        window: &str,
        ticket: u64,
        branches: Vec<(WatchedBranch, Root, How)>,
    ) -> Result<bool, String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let newest = inner.newest.entry(window.to_string()).or_default();
        if ticket < *newest {
            return Ok(false);
        }
        *newest = ticket;
        let listened: Vec<Listened> = branches
            .into_iter()
            .map(|(branch, root, how)| Listened { branch, root, how })
            .collect();
        if listened.is_empty() {
            inner.by_window.remove(window);
        } else {
            inner.by_window.insert(window.to_string(), listened);
        }
        if inner.watcher.is_none() && !inner.by_window.is_empty() {
            inner.watcher = Some(self.start()?);
        }
        inner.follow();
        Ok(true)
    }

    /// Whether `ticket` is still the newest set `window` asked for.
    fn wants(&self, window: &str, ticket: u64) -> bool {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.newest.get(window) == Some(&ticket)
    }

    /// Branches of `window`'s set `ticket` the reader answered for late (#1745): listened to
    /// now, and told to the window as moved, once their watch holds. Whatever changed in them
    /// before was heard by nothing, and the window read their status when the set was answered,
    /// so it reads it again. Whether the set still holds: it does not once the window has asked
    /// for a newer one, and then nothing is listened to or told.
    fn found_late(
        &self,
        window: &str,
        ticket: u64,
        found: Vec<(WatchedBranch, Root, How)>,
    ) -> bool {
        let moved: Vec<WatchedBranch> = found.iter().map(|(branch, ..)| branch.clone()).collect();
        {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if inner.newest.get(window) != Some(&ticket) {
                return false;
            }
            inner
                .by_window
                .entry(window.to_string())
                .or_default()
                .extend(found.into_iter().map(|(branch, root, how)| Listened {
                    branch,
                    root,
                    how,
                }));
            // No watcher could be made, now or lately: nothing is listened to, as a set that
            // could not start one answers; the window's next set tries again.
            if !inner.started(|| self.start()) {
                return true;
            }
            inner.follow();
        }
        // Outside the lock, as the watch's own thread tells. What moved before it held was heard
        // by nothing, a repo skill's folder among it.
        if !moved.is_empty() {
            let skills_told = self
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .skills_told
                .clone();
            if let Some(skills_told) = skills_told {
                skills_told(window, moved.clone());
            }
            (self.told)(window, moved);
        }
        true
    }

    /// `window` is gone: none of its branches is listened to for it any more.
    pub fn forget(&self, window: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.newest.remove(window);
        let had_cockpit = inner.cockpits.remove(window).is_some();
        if inner.by_window.remove(window).is_some() || had_cockpit {
            inner.follow();
        }
    }

    /// `window`'s sidebar is focused on `cockpit` now, or on the whole workspace (FM-5). While
    /// the window listens to that branch, its refs are watched too ([`Root::refs`], #1152), so
    /// a commit that writes no file in its folder still moves the cockpit's count. Only the
    /// cockpit's: one branch a window, a few folders each, whatever the window listens to.
    pub fn focus(&self, window: &str, cockpit: Option<WatchedBranch>) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let moved = match cockpit {
            Some(branch) => {
                inner.cockpits.insert(window.to_string(), branch.clone()) != Some(branch)
            }
            None => inner.cockpits.remove(window).is_some(),
        };
        if moved {
            inner.follow();
        }
    }

    /// Listens to the clone `wanted` names for its shared standing (FD-11), and covers the
    /// standing once its whole tree is watched. Answers at once: the clone is found by the
    /// bounded reader on a thread of its own, once however often it is asked for.
    pub fn want(&self, wanted: &Wanted) {
        let let_go = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if inner
                .refused
                .get(&wanted.path)
                .is_some_and(|until| Instant::now() < *until)
            {
                return;
            }
            if inner.kept.contains_key(&wanted.path) || !inner.finding.insert(wanted.path.clone()) {
                return;
            }
            inner.let_go.get(&wanted.plane).copied().unwrap_or_default()
        };
        let handle = Arc::downgrade(&self.inner);
        let reader = self.reader.clone();
        let told = Arc::clone(&self.told);
        let config = self.config;
        let how_of = self.how_of;
        let wanted = wanted.clone();
        let _ = std::thread::Builder::new()
            .name("charter-standing-watch".into())
            .spawn(move || {
                let found = purlis_core::files::root(
                    &reader,
                    &wanted.plane,
                    Branch::repo(&wanted.workspace, &wanted.repo),
                )
                .map(|root| {
                    let how = how_of(&root, &reader);
                    (root, how)
                });
                let Some(inner) = handle.upgrade() else {
                    return;
                };
                let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                held.finding.remove(&wanted.path);
                // Its plane was let go of while it was being found.
                if held.let_go.get(&wanted.plane).copied().unwrap_or_default() != let_go {
                    return;
                }
                // A clone the reader will not find is not watched, and not asked for again for a
                // while; the standing keeps its clock.
                let Ok((root, how)) = found else {
                    held.refused
                        .insert(wanted.path.clone(), Instant::now() + REFUSED_FOR);
                    return;
                };
                held.kept.insert(
                    wanted.path.clone(),
                    Kept {
                        plane: wanted.plane.clone(),
                        root,
                        how,
                    },
                );
                // No watcher could be made: the clone is left alone for a while, as one the
                // reader will not find is, rather than asked for on the standing's next read.
                if !held.started(|| start::<W>(&inner, &reader, &told, config)) {
                    held.kept.remove(&wanted.path);
                    held.refused
                        .insert(wanted.path.clone(), Instant::now() + REFUSED_FOR);
                    return;
                }
                held.follow();
            });
    }

    /// The plane at `plane` is let go of: none of its clones is listened to for its standing any
    /// more, and none is covered.
    pub fn let_go_of_plane(&self, plane: &std::path::Path) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        *inner.let_go.entry(plane.to_path_buf()).or_default() += 1;
        inner.refused.retain(|at, _| !at.starts_with(plane));
        let before = inner.kept.len();
        inner.kept.retain(|_, kept| kept.plane != plane);
        if inner.kept.len() != before {
            inner.follow();
        }
    }

    /// What is watched now, for a test to see a folder added as it appears: the poller the
    /// tests run on macOS sees a write in a new folder from its parent's listing, so hearing
    /// one there would not show the new folder is watched.
    #[cfg(test)]
    fn watching(&self) -> Vec<PathBuf> {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.watched.keys().cloned().collect()
    }

    /// The reader this watch asks: for a branch's folders when it is resolved.
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    /// The platform's watcher, and a thread that folds what it reports into bursts and hands
    /// each branch that something moved in to its worker. The thread ends when the watcher is
    /// dropped, which drops the sending half it reads.
    fn start(&self) -> Result<W, String> {
        start::<W>(&self.inner, &self.reader, &self.told, self.config)
    }
}

/// [`BranchWatch::start`], for a caller that holds only the watch's insides.
fn start<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    reader: &Reader,
    told: &Told,
    config: notify::Config,
) -> Result<W, String> {
    let (sent, events) = mpsc::channel();
    let handle: Weak<Mutex<Inner<W>>> = Arc::downgrade(inner);
    let told = Arc::clone(told);
    let reader = reader.clone();
    std::thread::Builder::new()
        .name("charter-branch-watch".into())
        .spawn(move || {
            for burst in crate::watchset::bursts(events, QUIET_FOR, MOST_PATHS) {
                let Some(inner) = handle.upgrade() else {
                    return;
                };
                heard(&inner, &reader, &told, &burst);
            }
        })
        .map_err(|e| format!("purlis could not watch the branch: {e}"))?;
    // No file-id cache, so nothing walks the tree under a watch — a link in the branch to the
    // operator's home is never followed and read (R2) — and links are not followed.
    W::new(
        crate::watchset::sender(sent),
        config.with_follow_symlinks(false),
    )
    .map_err(|e| format!("purlis could not watch the branch: {e}"))
}

/// Hands each branch that `burst` moved something in to its worker. Nothing is read here: this
/// thread goes straight back to the platform's events.
fn heard<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    reader: &Reader,
    told: &Told,
    burst: &crate::watchset::Burst,
) {
    let (roots, refs_moved, heads_moved) = {
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        if burst.lost {
            lost(inner, &mut held, burst.lost_only_in.as_ref());
        }
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let roots: Vec<Root> = held
            .listened()
            .map(|(root, _)| root)
            .filter(|root| seen.insert(root.path().to_path_buf()))
            .cloned()
            .collect();
        // A burst that is everything moves every branch below, the cockpit's among them.
        let refs_moved = if burst.everything {
            HashMap::new()
        } else {
            refs_moved(held.cockpits_listened(), &burst.paths)
        };
        // A cockpit whose HEAD moved may have another branch checked out now, whose ref is not
        // watched yet; one the platform lost track of may have too.
        let heads_moved: Vec<Root> = held
            .cockpit_roots()
            .filter(|root| burst.everything || head_moved(root.refs(), &burst.paths))
            .cloned()
            .collect();
        (roots, refs_moved, heads_moved)
    };
    for root in heads_moved {
        find_refs_again(inner, reader, told, root);
    }
    // The cockpit's branch moved in git's own folder only: nothing in it is the status's to
    // sort (`Root::matters` passes over git's folder), so it is told as it is.
    for (window, branches) in refs_moved {
        told(&window, branches);
    }
    for root in roots {
        // Everything moved the branch's folder itself, which always matters (`Root::matters`).
        let inside: Vec<PathBuf> = if burst.everything {
            vec![root.path().to_path_buf()]
        } else {
            burst
                .paths
                .iter()
                .filter(|path| path.starts_with(root.path()))
                .cloned()
                .collect()
        };
        if inside.is_empty() {
            continue;
        }
        if inside.iter().any(|path| {
            path.strip_prefix(root.path())
                .is_ok_and(purlis_core::harness_plugin::moves_a_repo_skill)
        }) {
            let (skills_told, by_window) = {
                let held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                (
                    held.skills_told.clone(),
                    windows_listening(&held, root.path()),
                )
            };
            if let Some(skills_told) = skills_told {
                for (window, branches) in by_window {
                    skills_told(&window, branches);
                }
            }
        }
        check(inner, reader, told, root, inside, burst.made_folder);
    }
}

/// The platform lost track, or its watcher erred (#874, L11): the kept clones the loss concerns
/// ([`lost_trees`] of `only_in`, the paths it named) are uncovered at once, and their trees
/// are watched again now, or once [`WATCHED_AGAIN_EVERY`] is over when trees were watched
/// again less than that ago, on a thread of its own; one re-watch takes every loss heard while
/// it waits.
fn lost<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    held: &mut Inner<W>,
    only_in: Option<&HashSet<PathBuf>>,
) {
    held.uncover(only_in);
    match held.rewatching.lost(Instant::now(), WATCHED_AGAIN_EVERY) {
        Again::Now => held.watch_again(),
        Again::Waiting => {}
        Again::After(wait) => {
            let handle = Arc::downgrade(inner);
            let started = std::thread::Builder::new()
                .name("purlis-branch-rewatch".into())
                .spawn(move || {
                    std::thread::sleep(wait);
                    let Some(inner) = handle.upgrade() else {
                        return;
                    };
                    let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                    held.rewatching.done(Instant::now());
                    held.watch_again();
                });
            // No thread: watched again now rather than left uncovered until the next loss.
            if started.is_err() {
                held.rewatching.done(Instant::now());
                held.watch_again();
            }
        }
    }
}

/// Hands what moved in `root` to its branch's worker: started now when none is asking, else
/// asked next. One worker per branch at a time, so a hung check holds one thread, not one per
/// burst.
fn check<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    reader: &Reader,
    told: &Told,
    root: Root,
    moved: Vec<PathBuf>,
    made_folder: bool,
) {
    {
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        let waiting = held.checking.entry(root.path().to_path_buf()).or_default();
        // Past what one check looks at, the rest only says "more": the burst matters anyway.
        let room = (purlis_core::files::ASKED + 1).saturating_sub(waiting.moved.len());
        waiting.moved.extend(moved.into_iter().take(room));
        waiting.made_folder |= made_folder;
        if waiting.running {
            return;
        }
        waiting.running = true;
    }
    let handle = Arc::downgrade(inner);
    let reader = reader.clone();
    let told = Arc::clone(told);
    std::thread::spawn(move || {
        loop {
            let Some(inner) = handle.upgrade() else {
                return;
            };
            let (moved, made_folder) = {
                let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(waiting) = held.checking.get_mut(root.path()) else {
                    return;
                };
                if waiting.moved.is_empty() {
                    held.checking.remove(root.path());
                    return;
                }
                (
                    std::mem::take(&mut waiting.moved),
                    std::mem::take(&mut waiting.made_folder),
                )
            };
            // Outside the lock: this is the read that can take until the reader's deadline.
            if !root.matters(&reader, &moved) {
                continue;
            }
            // The clone's shared standing reads git again at its next ask (FD-11), whoever
            // listens: the explorer's branch of a clone is the clone the Saving row reads.
            let standings: Vec<PathBuf> = {
                let held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                held.kept
                    .iter()
                    .filter(|(_, kept)| kept.root.path() == root.path())
                    .map(|(at, _)| at.clone())
                    .collect()
            };
            purlis_core::planegit::touch(root.path());
            for at in standings {
                purlis_core::planegit::touch(&at);
            }
            if made_folder {
                relist_soon(&inner, &reader, root.clone());
            }
            let by_window = {
                let held = inner.lock().unwrap_or_else(PoisonError::into_inner);
                windows_listening(&held, root.path())
            };
            for (window, branches) in by_window {
                told(&window, branches);
            }
        }
    });
}

/// The branches at `path` each window listens to.
fn windows_listening<W: notify::Watcher>(
    held: &Inner<W>,
    path: &Path,
) -> HashMap<String, Vec<WatchedBranch>> {
    let mut by_window: HashMap<String, Vec<WatchedBranch>> = HashMap::new();
    for (window, listened) in &held.by_window {
        for one in listened.iter().filter(|one| one.root.path() == path) {
            let told = by_window.entry(window.clone()).or_default();
            if !told.contains(&one.branch) {
                told.push(one.branch.clone());
            }
        }
    }
    by_window
}

/// Lists `root`'s folders again, now or — when it was listed less than [`RELISTED_EVERY`] ago —
/// once that has passed, on a thread of its own; never twice in that time. Only for a branch
/// watched folder by folder.
fn relist_soon<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    reader: &Reader,
    root: Root,
) {
    let wait = {
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        let one_by_one = held.listened().any(|(one, how)| {
            one.path() == root.path() && matches!(how, How::Folders(_) | How::Unread)
        });
        if !one_by_one {
            return;
        }
        let now = Instant::now();
        let at = root.path().to_path_buf();
        match held.relisted.get(&at).copied() {
            Some((_, true)) => return,
            Some((last, false)) if now.duration_since(last) < RELISTED_EVERY => {
                held.relisted.insert(at, (last, true));
                Some(RELISTED_EVERY - now.duration_since(last))
            }
            _ => {
                held.relisted.insert(at, (now, false));
                None
            }
        }
    };
    let handle = Arc::downgrade(inner);
    let reader = reader.clone();
    let run = move || {
        let now = listed(&root, &reader);
        let Some(inner) = handle.upgrade() else {
            return;
        };
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        held.relisted
            .insert(root.path().to_path_buf(), (Instant::now(), false));
        let held = &mut *held;
        let hows = held
            .by_window
            .values_mut()
            .flatten()
            .map(|one| (&one.root, &mut one.how))
            .chain(
                held.kept
                    .values_mut()
                    .map(|kept| (&kept.root, &mut kept.how)),
            );
        for (one, how) in hows {
            if one.path() == root.path() && matches!(how, How::Folders(_) | How::Unread) {
                *how = now.clone();
            }
        }
        held.follow();
    };
    match wait {
        None => run(),
        Some(wait) => {
            std::thread::spawn(move || {
                std::thread::sleep(wait);
                run();
            });
        }
    }
}

impl<W: notify::Watcher> Inner<W> {
    /// Each window's cockpit, while the window listens to it, with its refs: the branches whose
    /// refs are watched (#1152).
    fn cockpits_listened(&self) -> impl Iterator<Item = (&str, &WatchedBranch, &[PathBuf])> {
        self.by_window.iter().flat_map(|(window, listened)| {
            let cockpit = self.cockpits.get(window);
            listened
                .iter()
                .filter(move |one| Some(&one.branch) == cockpit)
                .map(move |one| (window.as_str(), &one.branch, one.root.refs()))
        })
    }
}

/// The folders to watch, each on its own, so the cockpits' refs are heard: the folder holding
/// each ref file, since git replaces a ref by renaming a new file over it.
fn ref_folders<'a>(
    cockpits: impl Iterator<Item = (&'a str, &'a WatchedBranch, &'a [PathBuf])>,
) -> HashSet<PathBuf> {
    cockpits
        .flat_map(|(_, _, refs)| refs.iter())
        .filter_map(|file| file.parent().map(PathBuf::from))
        .collect()
}

/// The windows whose cockpit's refs are among `moved`, each with that branch.
fn refs_moved<'a>(
    cockpits: impl Iterator<Item = (&'a str, &'a WatchedBranch, &'a [PathBuf])>,
    moved: &HashSet<PathBuf>,
) -> HashMap<String, Vec<WatchedBranch>> {
    let mut told: HashMap<String, Vec<WatchedBranch>> = HashMap::new();
    for (window, branch, refs) in cockpits {
        if refs.iter().any(|file| moved.contains(file)) {
            let branches = told.entry(window.to_string()).or_default();
            if !branches.contains(branch) {
                branches.push(branch.clone());
            }
        }
    }
    told
}

/// Whether `moved` names the `HEAD` among a cockpit's `refs`: its folder may have another branch
/// checked out now.
fn head_moved(refs: &[PathBuf], moved: &HashSet<PathBuf>) -> bool {
    refs.iter()
        .any(|file| file.file_name() == Some(std::ffi::OsStr::new("HEAD")) && moved.contains(file))
}

impl<W: notify::Watcher> Inner<W> {
    /// Each cockpit's folder, once, as the windows listening to it resolved it.
    fn cockpit_roots(&self) -> impl Iterator<Item = &Root> {
        let mut seen: HashSet<&Path> = HashSet::new();
        self.by_window
            .iter()
            .flat_map(|(window, listened)| {
                let cockpit = self.cockpits.get(window);
                listened
                    .iter()
                    .filter(move |one| Some(&one.branch) == cockpit)
            })
            .map(|one| &one.root)
            .filter(move |root| seen.insert(root.path()))
    }

    /// The windows whose cockpit, while they listen to it, is the folder `at`, each with that
    /// branch.
    fn cockpits_in(&self, at: &Path) -> HashMap<String, Vec<WatchedBranch>> {
        let mut told: HashMap<String, Vec<WatchedBranch>> = HashMap::new();
        for (window, listened) in &self.by_window {
            let cockpit = self.cockpits.get(window);
            for one in listened {
                if Some(&one.branch) == cockpit && one.root.path() == at {
                    let branches = told.entry(window.clone()).or_default();
                    if !branches.contains(&one.branch) {
                        branches.push(one.branch.clone());
                    }
                }
            }
        }
        told
    }
}

/// Finds `root`'s refs again, by the bounded reader on a thread of its own, after its `HEAD`
/// moved (#1152): a checkout in the cockpit's folder puts another branch there, whose ref is
/// then watched in place of the old one's, and the cockpit is told as moved once that watch
/// holds ([`refind`], #2463). Once per folder at a time; a move heard meanwhile finds them once
/// more when this one is done. A folder the reader no longer finds keeps what it had: the
/// window's next set says what it listens to.
fn find_refs_again<W: notify::Watcher + Send + 'static>(
    inner: &Arc<Mutex<Inner<W>>>,
    reader: &Reader,
    told: &Told,
    root: Root,
) {
    #[cfg(test)]
    let held_by: Option<Latch>;
    {
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        match held.refinding.get_mut(root.path()) {
            Some(again) => {
                *again = true;
                return;
            }
            None => {
                held.refinding.insert(root.path().to_path_buf(), false);
            }
        }
        #[cfg(test)]
        {
            held_by = held.refind_held.clone();
        }
    }
    let handle = Arc::downgrade(inner);
    let reader = reader.clone();
    let told = Arc::clone(told);
    let at = root.path().to_path_buf();
    let started = std::thread::Builder::new()
        .name("purlis-cockpit-refs".into())
        .spawn({
            let at = at.clone();
            move || {
                #[cfg(test)]
                if let Some(latch) = held_by {
                    latch.wait();
                }
                refind(&handle, &at, &told, || past_busy(|| root.again(&reader)))
            }
        });
    // No thread: the next move asks again.
    if started.is_err() {
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        held.refinding.remove(&at);
    }
}

/// A gate a test opens once: whoever waits on it waits until then.
#[cfg(test)]
#[derive(Clone, Default)]
struct Latch(Arc<(Mutex<bool>, std::sync::Condvar)>);

#[cfg(test)]
impl Latch {
    fn open(&self) {
        *self.0.0.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.0.1.notify_all();
    }

    fn wait(&self) {
        let mut open = self.0.0.lock().unwrap_or_else(PoisonError::into_inner);
        while !*open {
            open = self.0.1.wait(open).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// [`find_refs_again`]'s thread: `again` finds the refs of the cockpit folder `at` once more,
/// and every listened branch with that folder takes them; again while a move was heard
/// meanwhile. A read that panics lets go of the folder ([`Refinding`]), so the next move heard
/// finds them again.
///
/// **Refs that changed are told as moved once they are watched** (#2463). Between the checkout
/// and this watch, the branch checked out now has a ref nothing watches: a commit made right
/// after `git checkout -b` moves only that file, and a platform reports nothing from before a
/// watch (FSEvents also restarts its stream to add one). So each window whose cockpit is in
/// `at` is told it moved, after the watch is made, and reads its status again.
fn refind<W: notify::Watcher>(
    handle: &Weak<Mutex<Inner<W>>>,
    at: &Path,
    told: &Told,
    mut again: impl FnMut() -> Option<Root>,
) {
    let _letting_go = Refinding { handle, at };
    loop {
        // Outside the lock: this is the read that can take until the reader's deadline.
        let found = again();
        let Some(inner) = handle.upgrade() else {
            return;
        };
        let mut held = inner.lock().unwrap_or_else(PoisonError::into_inner);
        let mut moved: HashMap<String, Vec<WatchedBranch>> = HashMap::new();
        if let Some(found) = found.filter(|found| found.path() == at) {
            let mut changed = false;
            for one in held.by_window.values_mut().flatten() {
                if one.root.path() == at && one.root.refs() != found.refs() {
                    one.root = found.clone();
                    changed = true;
                }
            }
            if changed {
                held.follow();
                moved = held.cockpits_in(at);
            }
        }
        let more = held.refinding.get(at) == Some(&true);
        if more {
            held.refinding.insert(at.to_path_buf(), false);
        } else {
            held.refinding.remove(at);
        }
        // Outside the lock, as the watch's own thread tells.
        drop(held);
        for (window, branches) in moved {
            told(&window, branches);
        }
        if !more {
            return;
        }
    }
}

/// Lets go of a cockpit folder being re-found when its thread panics (train 49's review): left
/// marked, its later `HEAD` moves would only set the "again" flag of a thread that is gone, and
/// a checkout there would never be followed. A thread that ends as it should has let go
/// already, under the lock, and this does nothing then: a re-find started after that is not
/// let go of.
struct Refinding<'a, W: notify::Watcher> {
    handle: &'a Weak<Mutex<Inner<W>>>,
    at: &'a Path,
}

impl<W: notify::Watcher> Drop for Refinding<'_, W> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        if let Some(inner) = self.handle.upgrade() {
            inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .refinding
                .remove(self.at);
        }
    }
}

impl<W: notify::Watcher> Inner<W> {
    /// Watches what every window's branches need, and stops watching what none needs.
    fn follow(&mut self) {
        let mut wanted: HashMap<PathBuf, RecursiveMode> = HashMap::new();
        let mut one_by_one: Vec<(usize, PathBuf)> = Vec::new();
        for (root, how) in self.listened() {
            match how {
                How::Whole => {
                    wanted.insert(root.path().to_path_buf(), RecursiveMode::Recursive);
                }
                How::Unread => one_by_one.push((0, root.path().to_path_buf())),
                How::Folders(folders) => {
                    for folder in folders {
                        let depth = folder
                            .strip_prefix(root.path())
                            .map_or(usize::MAX, |below| below.components().count());
                        one_by_one.push((depth, folder.clone()));
                    }
                }
            }
        }
        // The cockpit's refs (#1152): the folders holding them, each on its own. A few a
        // window, so outside the share below, which bounds what a branch's tree can cost.
        for folder in ref_folders(self.cockpits_listened()) {
            wanted.entry(folder).or_insert(RecursiveMode::NonRecursive);
        }
        // Within the app's share of the platform's watches, the shallowest first: past it, a
        // deep folder is heard only when something above it moves.
        one_by_one.sort();
        one_by_one.dedup_by(|a, b| a.1 == b.1);
        for (_, folder) in one_by_one.into_iter().take(self.budget) {
            wanted.entry(folder).or_insert(RecursiveMode::NonRecursive);
        }
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        settle(watcher, &mut self.watched, wanted);
        self.cover();
    }

    /// Tells the shared standing which kept clones are watched whole now (FD-11): only those
    /// a recursive watch holds (D-FD11h). A clone that starts or stops being covered is read
    /// again at its next ask.
    fn cover(&mut self) {
        let whole = |kept: &Kept| {
            kept.how == How::Whole
                && !self.stale.contains(kept.root.path())
                && self.watched.get(kept.root.path()) == Some(&RecursiveMode::Recursive)
        };
        let now: HashSet<PathBuf> = self
            .kept
            .iter()
            .filter(|(_, kept)| whole(kept))
            .map(|(at, _)| at.clone())
            .collect();
        for gone in self.covering.difference(&now) {
            purlis_core::reposave::cover(gone, false);
        }
        for new in now.difference(&self.covering) {
            purlis_core::reposave::cover(new, true);
        }
        self.covering = now;
    }

    /// The platform lost track, or its watcher erred: what it said about the watched trees
    /// may be incomplete. Every kept clone whose tree the loss concerns ([`lost_trees`] of
    /// `only_in`, the paths it named) is uncovered, and stays so until its tree is watched
    /// again ([`Inner::watch_again`]): only a watch made since covers it (D-FD11h).
    fn uncover(&mut self, only_in: Option<&HashSet<PathBuf>>) {
        let whole: Vec<PathBuf> = self
            .kept
            .values()
            .filter(|kept| kept.how == How::Whole)
            .map(|kept| kept.root.path().to_path_buf())
            .collect();
        for tree in lost_trees(&whole, only_in) {
            if self.covering.remove(&tree) {
                purlis_core::reposave::cover(&tree, false);
            }
            self.stale.insert(tree);
        }
    }

    /// Every kept clone's tree a loss uncovered ([`Inner::uncover`]) is watched again, and
    /// covered once more where that watch is made.
    fn watch_again(&mut self) {
        let whole: Vec<PathBuf> = self.stale.drain().collect();
        if let Some(watcher) = self.watcher.as_mut() {
            for root in whole {
                if self.watched.contains_key(&root) {
                    unwatch(watcher, &mut self.watched, &root);
                }
            }
        }
        self.follow();
    }
}

/// The trees of `trees` a loss that named `only_in` concerns: those a named path lies in or
/// above. Every tree when the loss named no path, or named one in none of them: a path spelled
/// otherwise than the tree it is in must not leave that tree trusted (fail closed).
fn lost_trees(trees: &[PathBuf], only_in: Option<&HashSet<PathBuf>>) -> Vec<PathBuf> {
    let Some(named) = only_in else {
        return trees.to_vec();
    };
    let related = |tree: &PathBuf, path: &PathBuf| path.starts_with(tree) || tree.starts_with(path);
    if named
        .iter()
        .any(|path| !trees.iter().any(|tree| related(tree, path)))
    {
        return trees.to_vec();
    }
    trees
        .iter()
        .filter(|tree| named.iter().any(|path| related(tree, path)))
        .cloned()
        .collect()
}

/// Watches `wanted`, each its way, and stops watching what `watched` holds that it does not
/// want. `watched` is what the platform watches now, kept in step.
fn settle<W: notify::Watcher>(
    watcher: &mut W,
    watched: &mut HashMap<PathBuf, RecursiveMode>,
    wanted: HashMap<PathBuf, RecursiveMode>,
) {
    let stale: Vec<PathBuf> = watched
        .iter()
        .filter(|(path, mode)| wanted.get(*path) != Some(*mode))
        .map(|(path, _)| path.clone())
        .collect();
    for path in stale {
        if watched.contains_key(&path) {
            unwatch(watcher, watched, &path);
        }
    }
    for (path, mode) in wanted {
        if watched.contains_key(&path) {
            continue;
        }
        match watcher.watch(&path, mode) {
            // A folder swapped for a link before its watch was added has the link's target
            // watched: inotify follows a link it is handed, and notify never asks it not to
            // (#1130). Looked at again once watched, a folder that is a link now is let go of.
            Ok(()) if is_a_link(&path) => {
                let _ = watcher.unwatch(&path);
            }
            Ok(()) => {
                watched.insert(path, mode);
            }
            // The platform's watches are spent: what is watched already keeps working,
            // and nothing more is asked of it.
            Err(e) if matches!(e.kind, notify::ErrorKind::MaxFilesWatch) => break,
            Err(_) => {}
        }
    }
}

/// Whether `path` is a link, read without following it.
fn is_a_link(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Stops watching `path`, and forgets it in `watched`.
///
/// **A whole tree takes the folders under it with it** (#1152). inotify keeps one watch per
/// folder, and notify, unwatching a recursive watch, removes every folder under it: a ref
/// folder a cockpit still wants (`.git/refs/heads`, watched on its own) goes too, though
/// `watched` still lists it, so nothing would watch it again and a commit that writes no file
/// would be heard by no one. Each folder watched on its own under the tree is let go of as
/// well, and forgotten, so the next [`settle`] watches it again where it is still wanted. On
/// a platform that keeps them apart the same steps cost one unwatch and one watch each.
fn unwatch<W: notify::Watcher>(
    watcher: &mut W,
    watched: &mut HashMap<PathBuf, RecursiveMode>,
    path: &std::path::Path,
) {
    let _ = watcher.unwatch(path);
    if watched.remove(path) != Some(RecursiveMode::Recursive) {
        return;
    }
    let under: Vec<PathBuf> = watched
        .iter()
        .filter(|(below, mode)| **mode == RecursiveMode::NonRecursive && below.starts_with(path))
        .map(|(below, _)| below.clone())
        .collect();
    for below in under {
        let _ = watcher.unwatch(&below);
        watched.remove(&below);
    }
}

/// Listens to every branch named, so the window hears when anything in one moves that its
/// changes could show (FM-4). A branch that does not resolve is not listened to. The set
/// replaces the window's last one.
// Each branch is resolved by `purlis_core::files::root`, with the tree's own checks; the window
// never names a directory. Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn branch_watch(
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    branches: Vec<WatchedBranch>,
) -> Result<(), String> {
    use tauri::Manager as _;
    let ticket = window.state::<BranchWatch>().ticket();
    let reader = window.state::<BranchWatch>().reader().clone();
    let mut asked: Vec<(PathBuf, WatchedBranch)> = Vec::new();
    let mut seen: HashSet<WatchedBranch> = HashSet::new();
    for branch in branches {
        if asked.len() >= BRANCHES || !seen.insert(branch.clone()) {
            continue;
        }
        let Ok(held) = planes.held(&branch.plane) else {
            continue;
        };
        asked.push((held.root().to_path_buf(), branch));
    }
    // Off the thread that draws (SC-2): finding a branch, and on inotify its folders, asks the
    // bounded reader.
    let (resolved, later) = tauri::async_runtime::spawn_blocking(move || resolve(&reader, asked))
        .await
        .map_err(|err| format!("watching the branch did not finish: {err}"))?;
    let label = window.label().to_string();
    if !window
        .state::<BranchWatch>()
        .set_from(&label, ticket, resolved)?
    {
        return Ok(());
    }
    say_unlistened(&window, &label, ticket, unlistened(&later));
    if !later.is_empty() {
        let window = window.clone();
        // No thread: the branches stay said as unwatched, and the window's next set asks again.
        let _ = std::thread::Builder::new()
            .name("purlis-branch-later".into())
            .spawn(move || {
                let watch = window.state::<BranchWatch>();
                let reader = watch.reader().clone();
                listen_later(
                    || watch.wants(&label, ticket),
                    later,
                    |one| found(&reader, &one.plane, &one.branch),
                    |branch, answer| match answer {
                        Some((root, how)) => {
                            watch.found_late(&label, ticket, vec![(branch, root, how)])
                        }
                        // Refused: not listened to, as a refusal at the set is not.
                        None => watch.wants(&label, ticket),
                    },
                    |still| say_unlistened(&window, &label, ticket, still),
                );
            });
    }
    Ok(())
}

/// Tells the window labelled `label` which branches of its set `ticket` are not listened to
/// yet (#1745), through the folder watch's list ([`crate::filewatch::UNWATCHED`]): the explorer
/// says "Changes on disk are not shown yet" over every open folder of such a branch, as it does
/// for a branch the folder watch could not find (#1727).
fn say_unlistened(window: &tauri::Window, label: &str, ticket: u64, branches: Vec<WatchedBranch>) {
    use tauri::Manager as _;
    if let Some(files) = window.try_state::<crate::filewatch::FileWatch>() {
        files.branches_unlistened(label, ticket, branches, |to, list| {
            crate::filewatch::say_unwatched(window, to, list);
        });
    }
}

/// The branches of `later`, in its order.
fn unlistened(later: &[Later]) -> Vec<WatchedBranch> {
    later.iter().map(|one| one.branch.clone()).collect()
}

/// Each branch's folder and how it is listened to, a branch that does not resolve left out;
/// and the branches the reader gave no answer for, to ask for again later.
///
/// Each branch is asked on its own (#1130), so one whose read hangs until its deadline does not
/// hold back watching the window's others. [`BRANCHES`] bounds the threads, and the reader's own
/// app-wide gate how many children run at once. A branch whose ask found that gate full is asked
/// again ([`past_busy`]), and one the reader still gave no answer for — busy past every retry,
/// paused, or past its deadline — is [`Later`] (#1745): the window asks for its set only when the
/// set changes, so a branch left out for that would go unwatched with nothing said.
fn resolve(
    reader: &Reader,
    asked: Vec<(PathBuf, WatchedBranch)>,
) -> (Vec<(WatchedBranch, Root, How)>, Vec<Later>) {
    let each = each_apart(asked, |(plane, branch)| {
        let answer = found(reader, &plane, &branch);
        Some((plane, branch, answer))
    });
    let mut resolved = Vec::new();
    let mut later = Vec::new();
    for (plane, branch, answer) in each {
        match answer {
            Asked::Answered(Some((root, how))) => resolved.push((branch, root, how)),
            Asked::Answered(None) => {}
            Asked::Later(after) => later.push(Later {
                plane,
                branch,
                due: Instant::now() + after,
            }),
        }
    }
    (resolved, later)
}

/// `branch` of the project at `plane`, found by the bounded reader, with how it is listened to.
fn found(reader: &Reader, plane: &Path, branch: &WatchedBranch) -> Asked<(Root, How)> {
    let named = crate::piecefiles::branch(&branch.workspace, &branch.repo, &branch.piece);
    asked_or_later(|| purlis_core::files::root(reader, plane, named)).map(|root| {
        let how = how(&root, reader);
        (root, how)
    })
}

/// One branch of a window's set the reader gave no answer for (#1745), asked for again at
/// `due`.
struct Later {
    plane: PathBuf,
    branch: WatchedBranch,
    due: Instant,
}

/// How long after a read that gave no answer, and was not a pause, it is asked again: about
/// one reader deadline. A branch that keeps hanging is paused by the reader after its second
/// strike, and is then asked again when that pause is over.
pub(crate) const AGAIN_AFTER: Duration = Duration::from_secs(30);

/// How often a thread waiting to ask for a branch again looks at whether the window still
/// wants it: a window that moves on lets go of the thread within this.
pub(crate) const LOOKED_EVERY: Duration = Duration::from_secs(1);

/// Asks for each of `later` again once it is due, by `find`, for as long as `wanted`; hands
/// each the reader answered for to `found` (`None` when it refused), and stops once that says
/// the set no longer holds. Says the branches still not answered for by `said` each time that
/// list changes (#1745).
fn listen_later<T>(
    wanted: impl Fn() -> bool,
    mut later: Vec<Later>,
    find: impl Fn(&Later) -> Asked<T>,
    mut found: impl FnMut(WatchedBranch, Option<T>) -> bool,
    mut said: impl FnMut(Vec<WatchedBranch>),
) {
    while !later.is_empty() && wanted() {
        let now = Instant::now();
        let Some(next) = later.iter().position(|one| one.due <= now) else {
            let first = later.iter().map(|one| one.due).min().unwrap_or(now);
            std::thread::sleep(first.saturating_duration_since(now).min(LOOKED_EVERY));
            continue;
        };
        let one = later.remove(next);
        match find(&one) {
            Asked::Later(after) => later.insert(
                next,
                Later {
                    due: Instant::now() + after,
                    ..one
                },
            ),
            Asked::Answered(answer) => {
                if !found(one.branch, answer) {
                    return;
                }
                said(unlistened(&later));
            }
        }
    }
}

/// What a watch's automatic read came to: a read nobody asks for again, so one the reader gave
/// no answer for is asked again later rather than gone without (#1727, #1745).
#[derive(Debug)]
pub(crate) enum Asked<T> {
    /// The reader answered: what it found, or nothing when it refused.
    Answered(Option<T>),
    /// The reader gave no answer: ask again after this long.
    Later(Duration),
}

impl<T> Asked<T> {
    /// What was found, made into `U`.
    pub(crate) fn map<U>(self, into: impl FnOnce(T) -> U) -> Asked<U> {
        match self {
            Self::Answered(answer) => Asked::Answered(answer.map(into)),
            Self::Later(after) => Asked::Later(after),
        }
    }
}

/// What `ask` came to, asked again while it finds every reader place taken ([`past_busy`]):
/// [`Asked::Later`] once the reader's pause is over when it paused the branch, else after
/// [`AGAIN_AFTER`] when it gave no answer (busy past every retry, or past its bounds); and
/// answered with nothing when it refused.
pub(crate) fn asked_or_later<T>(
    ask: impl FnMut() -> Result<T, purlis_core::files::Refused>,
) -> Asked<T> {
    match asked_past_busy(BUSY_TRIES, ask) {
        Ok(found) => Asked::Answered(Some(found)),
        Err(why) => match Reader::paused_for(&why) {
            Some(left) => Asked::Later(left),
            None if no_answer(&why) => Asked::Later(AGAIN_AFTER),
            None => Asked::Answered(None),
        },
    }
}

/// Whether the reader gave no answer, rather than answering with a refusal
/// ([`purlis_core::files::READ_FAILED`]).
fn no_answer(why: &purlis_core::files::Refused) -> bool {
    matches!(why, purlis_core::files::Refused::Read(said)
        if said.starts_with(purlis_core::files::READ_FAILED))
}

/// How often a branch is asked while the reader's gate stays full: enough for every branch a
/// window may watch to have had its turn, [`purlis_core::files::AT_ONCE`] at a time, and once
/// more.
pub(crate) const BUSY_TRIES: usize = BRANCHES.div_ceil(purlis_core::files::AT_ONCE) + 1;

/// What `ask` answers, asked again while it finds every reader place taken, at most
/// [`BUSY_TRIES`] times; nothing once it fails otherwise.
///
/// The app's one retry for a read the bounded reader's gate may answer busy (#1189): an
/// automatic read nobody will ask for again, such as a watch finding its folder, uses it rather
/// than going without.
pub(crate) fn past_busy<T>(
    ask: impl FnMut() -> Result<T, purlis_core::files::Refused>,
) -> Option<T> {
    asked_past_busy(BUSY_TRIES, ask).ok()
}

/// What `ask` answers, asked again while it finds every reader place taken, `tries` times in
/// all (at least once); otherwise its last refusal. [`past_busy`] for a read someone waits on
/// and should be told about: each busy answer has already waited the reader's deadline.
pub(crate) fn asked_past_busy<T>(
    tries: usize,
    mut ask: impl FnMut() -> Result<T, purlis_core::files::Refused>,
) -> Result<T, purlis_core::files::Refused> {
    let mut left = tries.max(1);
    loop {
        left -= 1;
        match ask() {
            Err(why) if left > 0 && purlis_core::files::was_busy(&why) => {}
            answered => return answered,
        }
    }
}

/// `each` of `asked`, every one on a thread of its own, answered in `asked`'s order; one that
/// answers nothing, or whose thread panicked, is left out.
fn each_apart<T: Send, R: Send>(asked: Vec<T>, each: impl Fn(T) -> Option<R> + Sync) -> Vec<R> {
    let each = &each;
    std::thread::scope(|scope| {
        let running: Vec<_> = asked
            .into_iter()
            .map(|one| scope.spawn(move || each(one)))
            .collect();
        running
            .into_iter()
            .filter_map(|thread| thread.join().ok().flatten())
            .collect()
    })
}

#[cfg(test)]
mod tests;
