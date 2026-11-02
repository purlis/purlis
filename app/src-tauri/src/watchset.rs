// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the app's file watchers share (`planewatch.rs`, `filewatch.rs`, `branchwatch.rs`,
//! `killswitch.rs`, `themewatch.rs`):
//! which events are changes, folding the platform's events into bursts, and keeping a
//! watcher's watches equal to a wanted set of folders, each watched non-recursively.
//!
//! **Every watcher folds the platform's own events, here, and none uses a debouncer**
//! (#1138, #1139). `notify-debouncer-full` takes a file removed while its creation is still
//! being folded as never having been there and reports neither, so a short-lived file — one an
//! agent writes and removes, a stop marker taken away at once — went unheard. And it debounces
//! each path on its own clock, ticked a quarter of its timeout apart, so writes spread over
//! more than a tick by a busy machine came out as several batches rather than one. A burst
//! here is every change from its first event until [`bursts`]' `quiet_for` after it, whatever
//! its paths did in between.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify::event::{CreateKind, EventKind, MetadataKind, ModifyKind};

/// Whether an event is a change. Everything but an access is: reading a file changes nothing,
/// and it is exactly what the window does when it is told. An access time moving is the same
/// read seen from inotify's `IN_ATTRIB` under `relatime`. Without this, a window that re-read on
/// every event would re-read because it re-read, forever.
pub(crate) fn matters(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime))
    )
}

/// The handler a platform watcher is made with: every event that is a change, sent on to
/// [`bursts`].
///
/// **An error fails closed**: the watcher could not say what changed, so it is sent on as the
/// platform losing track (a rescan), and its burst is everything. Dropped, as the debouncer
/// before it dropped one, it was a change nobody was ever told of.
pub(crate) fn sender(events: Sender<notify::Event>) -> impl notify::EventHandler {
    move |event: notify::Result<notify::Event>| {
        let event = match event {
            Ok(event) if matters(&event.kind) => event,
            Ok(_) => return,
            Err(_) => notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan),
        };
        // The receiving half is gone only once its watch is.
        let _ = events.send(event);
    }
}

/// One burst of changes, folded.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Burst {
    /// Every path a change named. Empty when [`Burst::everything`].
    pub paths: HashSet<PathBuf>,
    /// The paths among them a removal named.
    pub removed: HashSet<PathBuf>,
    /// Folders, or files, each standing for every path in it the burst named past what it
    /// holds of one tree (#4740): the entry at the top of that tree the path lies in, or the
    /// tree's own folder for the tree itself. Only a burst folded by tree ([`bursts_by_tree`])
    /// has any, and none when [`Burst::everything`].
    pub crowded: HashSet<PathBuf>,
    /// Anything may have changed: the platform lost track, an event named no path, or the
    /// burst named more paths than it holds (of those in no tree, or of crowded entries).
    pub everything: bool,
    /// A folder may have been made: a creation the platform named as a folder's, or as
    /// anything's, or a burst that is everything.
    pub made_folder: bool,
    /// The platform lost track or its watcher erred (a rescan, or an event naming no path),
    /// rather than the burst holding more paths than it can: what is watched may no longer be
    /// heard, and a watch that vouched for a whole tree no longer does (FD-11).
    pub lost: bool,
    /// Where the platform lost track, when every loss of the burst named where (#874, L11):
    /// the paths its rescans named, at most as many as the burst holds. `None` when nothing
    /// was lost, or when a loss named no path, the watcher erred, or the losses named more
    /// paths than that: it may have lost track anywhere. The burst is everything all the same,
    /// for what a watch reads; only a watch's own looking again may keep to these paths.
    pub lost_only_in: Option<HashSet<PathBuf>>,
}

impl Burst {
    /// Folds `event` in, holding at most `most` paths, as [`bursts`] does.
    #[cfg(test)]
    fn add(&mut self, event: notify::Event, most: usize) {
        self.add_holding(event, &mut Holding::new(most, |_: &Path| None));
    }

    /// Folds `event` in as `holding` says: past what it holds of a tree, a path is held by
    /// its entry at the top of the tree ([`Burst::crowded`]); past what it holds in all, the
    /// burst counts as everything, so a flood costs a fixed amount of memory and the reader
    /// reads a little more than it needed to rather than missing a change.
    fn add_holding<T: FnMut(&Path) -> Option<PathBuf>>(
        &mut self,
        event: notify::Event,
        holding: &mut Holding<T>,
    ) {
        let most = holding.most;
        if event.need_rescan() || event.paths.is_empty() {
            self.lost_in(event.paths, most);
            return self.is_everything();
        }
        if self.everything {
            // Already everything, by overflow or a loss.
            return;
        }
        let removed = matches!(event.kind, EventKind::Remove(_));
        self.made_folder |= matches!(
            event.kind,
            EventKind::Create(CreateKind::Folder | CreateKind::Any)
        );
        for path in event.paths {
            if !self.paths.contains(&path) {
                match holding.place(&path, self.paths.len()) {
                    Place::Named => {}
                    Place::Crowded(entry) => {
                        if self.crowded.len() >= most && !self.crowded.contains(&entry) {
                            return self.is_everything();
                        }
                        self.crowded.insert(entry);
                        continue;
                    }
                    Place::Full => return self.is_everything(),
                }
            }
            if removed {
                self.removed.insert(path.clone());
            }
            self.paths.insert(path);
        }
    }

    /// A loss that named `paths`, none when it named none: kept where every loss named its
    /// paths and they fit in `most`, else the loss may be anywhere.
    fn lost_in(&mut self, paths: Vec<PathBuf>, most: usize) {
        let first = !self.lost;
        self.lost = true;
        if paths.is_empty() {
            self.lost_only_in = None;
            return;
        }
        let only_in = if first {
            self.lost_only_in.insert(HashSet::new())
        } else if let Some(only_in) = self.lost_only_in.as_mut() {
            only_in
        } else {
            return;
        };
        only_in.extend(paths);
        if only_in.len() > most {
            self.lost_only_in = None;
        }
    }

    fn is_everything(&mut self) {
        self.everything = true;
        self.made_folder = true;
        self.paths = HashSet::new();
        self.removed = HashSet::new();
        self.crowded = HashSet::new();
    }
}

/// How many trees' share of paths a burst holds in all (the review of #4740): past
/// `most` × this many paths in trees, a tree's further paths are held by the entry at its top
/// they lie in, as a full tree's are. So a burst holds a fixed number of paths however many
/// trees are listened to, rather than `most` for each of them.
const TREES_IN_ALL: usize = 4;

/// How many paths one burst holds: at most `most` of each tree's, by the tree `tree` puts a
/// path in, `most` × [`TREES_IN_ALL`] of all trees', and at most `most` of those it puts in
/// none.
struct Holding<T> {
    most: usize,
    tree: T,
    /// How many paths each tree has named so far.
    held: HashMap<PathBuf, usize>,
    /// How many of the burst's paths are held for a tree.
    in_trees: usize,
}

/// Where a path the burst has not named yet goes.
enum Place {
    /// Among the burst's paths.
    Named,
    /// Held by this entry at the top of its tree, which is full, or all trees are.
    Crowded(PathBuf),
    /// Nowhere: the burst holds no more paths outside a tree.
    Full,
}

impl<T: FnMut(&Path) -> Option<PathBuf>> Holding<T> {
    fn new(most: usize, tree: T) -> Self {
        Self {
            most,
            tree,
            held: HashMap::new(),
            in_trees: 0,
        }
    }

    /// Where `path` goes, in a burst naming `named` paths already.
    fn place(&mut self, path: &Path, named: usize) -> Place {
        let Some(tree) = (self.tree)(path) else {
            return if named - self.in_trees < self.most {
                Place::Named
            } else {
                Place::Full
            };
        };
        let held = self.held.entry(tree.clone()).or_default();
        if *held < self.most && self.in_trees < self.most * TREES_IN_ALL {
            *held += 1;
            self.in_trees += 1;
            return Place::Named;
        }
        Place::Crowded(top_of(&tree, path))
    }
}

/// The entry at the top of `tree` that `path` lies in, or `tree` itself.
pub(crate) fn top_of(tree: &Path, path: &Path) -> PathBuf {
    match path
        .strip_prefix(tree)
        .ok()
        .and_then(|below| below.components().next())
    {
        Some(first) => tree.join(first),
        None => tree.to_path_buf(),
    }
}

/// The changes `events` carries, one [`Burst`] at a time: each from its first event until
/// `quiet_for` after it, holding at most `most` paths. It ends when the watcher sending them is
/// dropped, and a burst cut short by that is not handed on: nobody is listening for it.
///
/// **The deadline is checked before every receive**, so a flood still ends its burst on time:
/// `recv_timeout` with no time left answers with what is queued and never times out, and a
/// burst that ended only on a quiet moment never ended under an agent writing without pause.
pub(crate) fn bursts(
    events: Receiver<notify::Event>,
    quiet_for: Duration,
    most: usize,
) -> impl Iterator<Item = Burst> {
    std::iter::from_fn(move || {
        let first = events.recv().ok()?;
        rest_of_burst(&events, first, quiet_for, most)
    })
}

/// [`bursts`] for a watch of several trees (#4740): `tree` names the tree a path lies in, or
/// none, and the burst holds at most `most` paths of each tree and `most` × [`TREES_IN_ALL`]
/// of all trees together. Past either, a tree's paths are held by the entry at its top they lie
/// in ([`Burst::crowded`]), so one tree's flood leaves what the others named as it was. Past `most` paths in no tree, or `most` crowded entries,
/// the burst is everything, as [`bursts`]' is. Its timing is [`bursts`]'.
pub(crate) fn bursts_by_tree(
    events: Receiver<notify::Event>,
    quiet_for: Duration,
    most: usize,
    mut tree: impl FnMut(&Path) -> Option<PathBuf>,
) -> impl Iterator<Item = Burst> {
    std::iter::from_fn(move || {
        let first = events.recv().ok()?;
        fold(
            &events,
            first,
            quiet_for,
            &mut Holding::new(most, &mut tree),
        )
    })
}

/// [`bursts`], and `None` each time `idle` passes with no event at all: for a watch that also
/// looks for itself now and then, so a watch the platform has quietly lost makes it slow rather
/// than blind. It ends as [`bursts`] does.
pub(crate) fn bursts_or_idle(
    events: Receiver<notify::Event>,
    quiet_for: Duration,
    most: usize,
    idle: Duration,
) -> impl Iterator<Item = Option<Burst>> {
    std::iter::from_fn(move || match events.recv_timeout(idle) {
        Ok(first) => rest_of_burst(&events, first, quiet_for, most).map(Some),
        Err(RecvTimeoutError::Timeout) => Some(None),
        Err(RecvTimeoutError::Disconnected) => None,
    })
}

/// The burst `first` begins: everything received until `quiet_for` after it, or `None` when the
/// watcher is dropped before then.
pub(crate) fn rest_of_burst(
    events: &Receiver<notify::Event>,
    first: notify::Event,
    quiet_for: Duration,
    most: usize,
) -> Option<Burst> {
    fold(
        events,
        first,
        quiet_for,
        &mut Holding::new(most, |_: &Path| None),
    )
}

/// [`rest_of_burst`], holding paths as `holding` says.
fn fold<T: FnMut(&Path) -> Option<PathBuf>>(
    events: &Receiver<notify::Event>,
    first: notify::Event,
    quiet_for: Duration,
    holding: &mut Holding<T>,
) -> Option<Burst> {
    let mut burst = Burst::default();
    burst.add_holding(first, holding);
    let until = Instant::now() + quiet_for;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Some(burst);
        }
        match events.recv_timeout(left) {
            Ok(more) => burst.add_holding(more, holding),
            Err(RecvTimeoutError::Timeout) => return Some(burst),
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// Watches every folder of `wanted`, non-recursively, and stops watching the ones in `watched`
/// it no longer names; `watched` is kept as what is watched now.
///
/// **Answers the folders of `wanted` the platform would not watch**, which are still there:
/// a watcher that drops one and goes on as if it watched the whole set serves what it held
/// for that folder for ever (the FD-10 review). A caller that trusts its watch to tell it
/// everything treats a non-empty answer as not watching. The next call tries them again.
#[must_use = "a folder the platform would not watch is one nothing hears about"]
pub(crate) fn follow(
    watcher: &mut impl notify::Watcher,
    watched: &mut HashSet<PathBuf>,
    wanted: HashSet<PathBuf>,
) -> HashSet<PathBuf> {
    let stale: Vec<PathBuf> = watched.difference(&wanted).cloned().collect();
    for path in stale {
        // An error is a watch the platform has already dropped with its directory.
        let _ = watcher.unwatch(&path);
        watched.remove(&path);
    }
    let mut unwatched = HashSet::new();
    for path in wanted {
        if watched.contains(&path) {
            continue;
        }
        match watch_one(watcher, &path) {
            Ok(()) => {
                watched.insert(path);
            }
            // A directory that went between the listing and here is simply not watched; the
            // next batch lists again.
            Err(_) if !is_a_dir(&path) => {}
            Err(why) => {
                tracing::warn!("purlis: {} is not watched ({why})", path.display());
                unwatched.insert(path);
            }
        }
    }
    unwatched
}

/// Whether `path` is a directory itself, not a link to one.
fn is_a_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_dir())
}

/// One folder watched, or why not — refused here in this crate's tests when a test says so
/// ([`refuse`]).
fn watch_one(watcher: &mut impl notify::Watcher, path: &Path) -> notify::Result<()> {
    #[cfg(test)]
    if refuse::refused(path) {
        return Err(notify::Error::generic("refused by the test"));
    }
    watcher.watch(path, RecursiveMode::NonRecursive)
}

/// The platform refusing a watch, played by the tests: a folder named here is never watched,
/// whichever watcher is asked. Paths are each test's own, under a directory of its own, so
/// tests running at once do not see each other's.
#[cfg(test)]
pub(crate) mod refuse {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, PoisonError};

    static REFUSED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    /// `path` is refused from now on.
    pub(crate) fn refuse(path: &Path) {
        REFUSED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.to_path_buf());
    }

    /// `path` is watched again when next asked.
    pub(crate) fn allow(path: &Path) {
        REFUSED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|refused| refused != path);
    }

    pub(super) fn refused(path: &Path) -> bool {
        REFUSED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|refused| refused == path)
    }
}

/// Stops watching `path`, if it was watched: it is gone, and so is its watch (inotify drops it
/// on its own). A directory removed and made again has to be watched again, so it must not
/// still be counted as watched.
pub(crate) fn forget(
    watcher: &mut impl notify::Watcher,
    watched: &mut HashSet<PathBuf>,
    path: &Path,
) {
    if watched.remove(path) {
        let _ = watcher.unwatch(path);
    }
}

/// A watcher the tests play the platform for.
#[cfg(test)]
pub(crate) mod raw {
    use std::path::Path;

    thread_local! {
        /// What [`Raw`] was handed on this test's thread.
        static RAW: std::cell::RefCell<Option<Box<dyn notify::EventHandler>>> =
            const { std::cell::RefCell::new(None) };
    }

    /// A watcher that reports exactly the events the test sends ([`raw`], [`event`]), one at
    /// a time as inotify does: no poller's snapshot and no FSEvents latency between the test
    /// and what the watch makes of them. Made on the test's own thread.
    pub(crate) struct Raw;

    /// Every path a [`Raw`] was asked to watch, from any thread: each test's are its own,
    /// under a directory of its own.
    static WATCHES: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

    /// Paths a [`Raw`] refuses to watch, as a platform out of watches or a folder gone would.
    static REFUSED: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

    /// From now on, a [`Raw`] refuses to watch `path`.
    pub(crate) fn refuse(path: &Path) {
        REFUSED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.to_path_buf());
    }

    /// How many times a [`Raw`] has been asked to watch `path`.
    pub(crate) fn watched(path: &Path) -> usize {
        WATCHES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|watched| *watched == path)
            .count()
    }

    impl notify::Watcher for Raw {
        fn new<F: notify::EventHandler>(handler: F, _: notify::Config) -> notify::Result<Self> {
            RAW.with(|raw| *raw.borrow_mut() = Some(Box::new(handler)));
            Ok(Raw)
        }
        fn watch(&mut self, path: &Path, _: notify::RecursiveMode) -> notify::Result<()> {
            if REFUSED
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .any(|refused| refused == path)
            {
                return Err(notify::Error::path_not_found());
            }
            WATCHES
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(path.to_path_buf());
            Ok(())
        }
        fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
            Ok(())
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::NullWatcher
        }
    }

    /// The platform reports `kind` on `path`.
    pub(crate) fn raw(kind: notify::EventKind, path: &Path) {
        event(notify::Event::new(kind).add_path(path.to_path_buf()));
    }

    /// The platform reports `event`.
    pub(crate) fn event(event: notify::Event) {
        RAW.with(|raw| {
            raw.borrow_mut()
                .as_mut()
                .expect("the watch has started")
                .handle_event(Ok(event));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode, Flag, RemoveKind};
    use std::sync::mpsc;

    fn on(kind: EventKind, path: &str) -> notify::Event {
        notify::Event::new(kind).add_path(PathBuf::from(path))
    }

    #[test]
    fn a_burst_folds_every_change_queued_inside_it_into_one() {
        let (tx, rx) = mpsc::channel();
        for n in 0..10 {
            tx.send(on(
                EventKind::Create(CreateKind::File),
                &format!("/p/t{n}.md"),
            ))
            .unwrap();
        }
        let mut bursts = bursts(rx, Duration::from_millis(500), 256);

        let burst = bursts.next().expect("a burst");
        assert_eq!(burst.paths.len(), 10);
        assert!(!burst.everything);
        drop(tx);
        assert_eq!(bursts.next(), None, "a dropped watcher ends the bursts");
    }

    #[test]
    fn a_file_made_and_removed_inside_one_burst_is_still_a_change_and_a_removal() {
        let (tx, rx) = mpsc::channel();
        tx.send(on(EventKind::Create(CreateKind::File), "/p/marker"))
            .unwrap();
        tx.send(on(EventKind::Remove(RemoveKind::File), "/p/marker"))
            .unwrap();

        let burst = bursts(rx, Duration::from_millis(100), 256)
            .next()
            .expect("a burst");
        assert_eq!(burst.paths, HashSet::from([PathBuf::from("/p/marker")]));
        assert_eq!(burst.removed, HashSet::from([PathBuf::from("/p/marker")]));
    }

    #[test]
    fn a_burst_ends_on_time_under_a_flood_that_never_pauses() {
        let (tx, rx) = mpsc::channel();
        let flood = std::thread::spawn(move || {
            let mut n: u64 = 0;
            while tx
                .send(on(EventKind::Create(CreateKind::File), &format!("/p/f{n}")))
                .is_ok()
            {
                n += 1;
            }
        });
        let started = Instant::now();
        let burst = bursts(rx, Duration::from_millis(100), 256)
            .next()
            .expect("a burst");
        let took = started.elapsed();
        assert!(burst.everything, "a flood past the cap is everything");
        assert!(took < Duration::from_secs(5), "the burst took {took:?}");
        // Dropping the receiver ends the flood.
        flood.join().unwrap();
    }

    #[test]
    fn a_burst_past_its_cap_is_everything_and_holds_no_paths() {
        let mut burst = Burst::default();
        for n in 0..20 {
            burst.add(
                on(EventKind::Remove(RemoveKind::File), &format!("/p/{n}")),
                8,
            );
        }
        assert!(burst.everything);
        assert!(burst.paths.is_empty() && burst.removed.is_empty());
    }

    #[test]
    fn a_tree_past_its_cap_is_held_by_its_top_entries_and_leaves_the_others_named() {
        let tree = |path: &Path| {
            ["/a", "/b"]
                .into_iter()
                .map(PathBuf::from)
                .find(|tree| path.starts_with(tree))
        };
        let mut holding = Holding::new(8, tree);
        let mut burst = Burst::default();
        for n in 0..20 {
            let path = format!("/a/target/debug/{n}");
            burst.add_holding(on(EventKind::Create(CreateKind::File), &path), &mut holding);
        }
        burst.add_holding(
            on(EventKind::Create(CreateKind::File), "/a/src/lib.rs"),
            &mut holding,
        );
        burst.add_holding(on(EventKind::Create(CreateKind::File), "/a"), &mut holding);
        burst.add_holding(
            on(EventKind::Create(CreateKind::File), "/b/src/lib.rs"),
            &mut holding,
        );

        assert!(!burst.everything);
        assert_eq!(
            burst
                .paths
                .iter()
                .filter(|path| path.starts_with("/a"))
                .count(),
            8
        );
        assert!(burst.paths.contains(Path::new("/b/src/lib.rs")));
        assert_eq!(burst.crowded, set(&["/a/target", "/a/src", "/a"]));
    }

    /// The review of #4817: each tree under its own cap, many trees at once, must not make a
    /// burst hold a cap's worth of paths per tree.
    #[test]
    fn many_trees_each_under_their_cap_hold_no_more_paths_in_all_than_the_ceiling() {
        let mut holding = Holding::new(8, |path: &Path| {
            path.ancestors()
                .find(|above| above.parent() == Some(Path::new("/")))
                .map(Path::to_path_buf)
        });
        let mut burst = Burst::default();
        for tree in 0..10 {
            for n in 0..8 {
                let path = format!("/t{tree}/src/{n}");
                burst.add_holding(on(EventKind::Create(CreateKind::File), &path), &mut holding);
            }
        }

        assert!(!burst.everything);
        assert_eq!(burst.paths.len(), 8 * TREES_IN_ALL);
        // Every tree past the ceiling is held by its top entry, as a full tree's paths are.
        let past: HashSet<PathBuf> = (TREES_IN_ALL..10)
            .map(|tree| PathBuf::from(format!("/t{tree}/src")))
            .collect();
        assert_eq!(burst.crowded, past);
    }

    #[test]
    fn past_its_cap_outside_every_tree_a_burst_folded_by_tree_is_everything() {
        let mut holding = Holding::new(8, |path: &Path| {
            path.starts_with("/a").then(|| PathBuf::from("/a"))
        });
        let mut burst = Burst::default();
        for n in 0..20 {
            let path = format!("/elsewhere/{n}");
            burst.add_holding(on(EventKind::Create(CreateKind::File), &path), &mut holding);
        }
        assert!(burst.everything);
        assert!(burst.paths.is_empty() && burst.crowded.is_empty());
    }

    #[test]
    fn a_folder_made_or_anything_made_or_everything_may_have_made_a_folder() {
        let mut file = Burst::default();
        file.add(on(EventKind::Create(CreateKind::File), "/p/a"), 8);
        file.add(on(EventKind::Remove(RemoveKind::Folder), "/p/b"), 8);
        assert!(!file.made_folder);

        for kind in [CreateKind::Folder, CreateKind::Any] {
            let mut made = Burst::default();
            made.add(on(EventKind::Create(kind), "/p/new"), 8);
            assert!(made.made_folder, "{kind:?}");
        }

        let mut rescan = Burst::default();
        rescan.add(
            notify::Event::new(EventKind::Other).set_flag(Flag::Rescan),
            8,
        );
        assert!(rescan.made_folder);
    }

    #[test]
    fn a_lost_track_or_a_pathless_event_is_everything() {
        let mut rescan = Burst::default();
        rescan.add(
            notify::Event::new(EventKind::Other).set_flag(Flag::Rescan),
            8,
        );
        assert!(rescan.everything);

        let mut pathless = Burst::default();
        pathless.add(notify::Event::new(EventKind::Any), 8);
        assert!(pathless.everything);
    }

    fn rescan(paths: &[&str]) -> notify::Event {
        paths.iter().fold(
            notify::Event::new(EventKind::Other).set_flag(Flag::Rescan),
            |event, path| event.add_path(PathBuf::from(path)),
        )
    }

    fn set(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    /// #874 (L11): a rescan that names where the platform lost track keeps those paths, so a
    /// watch can look again there only; the burst is still everything for what it reads.
    #[test]
    fn a_rescan_keeps_the_paths_it_names() {
        let mut burst = Burst::default();
        burst.add(on(EventKind::Create(CreateKind::File), "/p/a/new"), 8);
        burst.add(rescan(&["/p/b"]), 8);
        burst.add(rescan(&["/p/c/d"]), 8);
        // A change after the loss, into a burst that is everything already.
        burst.add(on(EventKind::Create(CreateKind::File), "/p/e"), 8);

        assert!(burst.lost && burst.everything);
        assert_eq!(burst.lost_only_in, Some(set(&["/p/b", "/p/c/d"])));
    }

    /// #874 (L11), failing closed: a loss that names no path, a watcher's error, or more named
    /// paths than a burst holds, may be anywhere.
    #[test]
    fn a_loss_that_names_no_path_or_too_many_is_anywhere() {
        for losses in [
            vec![rescan(&["/p/b"]), rescan(&[])],
            vec![rescan(&[]), rescan(&["/p/b"])],
            vec![rescan(&["/p/1", "/p/2", "/p/3"])],
            vec![rescan(&["/p/1", "/p/2"]), rescan(&["/p/3"])],
        ] {
            let mut burst = Burst::default();
            for loss in losses.clone() {
                burst.add(loss, 2);
            }
            assert!(burst.lost, "{losses:?}");
            assert_eq!(burst.lost_only_in, None, "{losses:?}");
        }

        let mut overflowed = Burst::default();
        for n in 0..3 {
            overflowed.add(
                on(EventKind::Create(CreateKind::File), &format!("/p/{n}")),
                2,
            );
        }
        assert!(overflowed.everything && !overflowed.lost);
        assert_eq!(overflowed.lost_only_in, None);
    }

    #[test]
    fn a_read_never_reaches_a_burst() {
        let (tx, rx) = mpsc::channel();
        let mut handler = sender(tx);
        notify::EventHandler::handle_event(
            &mut handler,
            Ok(on(
                EventKind::Access(AccessKind::Open(AccessMode::Read)),
                "/p/a",
            )),
        );
        notify::EventHandler::handle_event(
            &mut handler,
            Ok(on(EventKind::Create(CreateKind::File), "/p/b")),
        );
        assert_eq!(
            rx.try_recv().expect("the change").paths,
            [PathBuf::from("/p/b")]
        );
        assert!(rx.try_recv().is_err(), "the read was sent on");
    }

    #[test]
    fn a_watcher_error_fails_closed_into_a_burst_that_is_everything() {
        let (tx, rx) = mpsc::channel();
        let mut handler = sender(tx);
        notify::EventHandler::handle_event(
            &mut handler,
            Err(notify::Error::generic("the watch was lost")),
        );
        let burst = bursts(rx, Duration::from_millis(50), 8)
            .next()
            .expect("the error was sent on");
        assert!(burst.everything);
    }

    #[test]
    fn an_idle_watch_is_said_to_be_idle_and_a_burst_still_comes_through() {
        let (tx, rx) = mpsc::channel();
        let mut bursts =
            bursts_or_idle(rx, Duration::from_millis(50), 8, Duration::from_millis(50));
        assert_eq!(bursts.next(), Some(None), "no event, so idle");
        tx.send(on(EventKind::Create(CreateKind::File), "/p/a"))
            .unwrap();
        let burst = bursts.next().expect("not ended").expect("a burst");
        assert_eq!(burst.paths, HashSet::from([PathBuf::from("/p/a")]));
        drop(tx);
        assert_eq!(bursts.next(), None, "a dropped watcher ends them");
    }
}
