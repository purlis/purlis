// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The state of each branch folder, answered as each is read and kept until it moves (#5321, H16).
//!
//! `git status` in 133 folders took 44 to 47 seconds in one answer. Two things change that:
//! **an ask returns what is known as soon as something new is**, the rest still reading, so the
//! window draws the first folders within a status's time and asks again for the others (a
//! command that answers once works the same over the link as in the window, which a stream
//! would not); and **a folder's answer is kept**, until a watch says something in it moved, so
//! asking again reads only the folders that changed. Reads go on in the background, eight at
//! a time, whether or not an ask is still waiting for them.
//!
//! **Never clean when it has changes** (#1476). A kept answer stands only while nothing was
//! heard that could change it: any event in the folder or in the folder git keeps its index in,
//! and any event in the repo's common `.git` outside another worktree's own folder there (a ref,
//! `packed-refs`, `config`, `info/exclude`), which moves every folder of that repo. **The watch
//! fails closed**, as `watchset::sender` makes every watcher: an error or a lost track is a
//! rescan, and a rescan moves every folder; the platform's watch limit leaves every folder
//! unwatched; a watch added or removed moves every folder, because FSEvents restarts its stream
//! from now on each, and what happened while it did is never told. An answer read while an
//! event arrived is not kept. Events reach a reader late, so before a kept answer is served the
//! watch is flushed: a file is written in each folder's own gitdir (outside the work tree,
//! where an editor's or a dev server's watch never sees it, and git ignores it) and its event
//! waited for, and the
//! platform's events come in order, so everything before it has been heard. A flush that times out moves every folder. A restart keeps nothing.
//!
//! Only a successful read is kept; a refusal is read again every time. A folder that keeps
//! moving (a build writing `target/`) is read at most once every [`READ_EVERY`], and only while
//! someone asks: until a read hears nothing move, it is still being read, never clean.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use notify::event::{EventKind, ModifyKind};
use notify::{EventHandler, RecursiveMode, Watcher};

/// What one folder's read said: the count of uncommitted files, or why it could not be read.
pub type Read = (Option<u32>, Option<String>);

/// One folder to answer for.
#[derive(Debug, Clone)]
pub struct Folder {
    pub path: PathBuf,
}

/// How many folders are read at once: enough that a first ask fills in quickly, few enough
/// that a listing never floods the machine with `git status` runs.
const READ_AT_ONCE: usize = 8;

/// After the first new answer of an ask, how long it waits for more before returning, so a
/// burst of reads finishing together is one answer and not several.
const GATHER: Duration = Duration::from_millis(150);

/// The longest an ask waits for anything new before returning with every folder still reading.
const NOTHING_WITHIN: Duration = Duration::from_secs(15);

/// How long a flush waits for its marker's event before every folder counts as moved.
const FLUSH_WITHIN: Duration = Duration::from_secs(2);

/// The name every flush marker starts with. An event naming one moves no folder.
const FLUSH_PREFIX: &str = ".purlis-flush-";

/// The soonest a folder is read again after its last read began: a folder whose events keep
/// coming (a build in `target/`) is read once in this, not once per event.
const READ_EVERY: Duration = Duration::from_secs(2);

/// What an event says when the platform could watch no more folders (Linux's
/// `max_user_watches`).
const WATCH_LIMIT: &str = "purlis-statecache-watch-limit";

#[cfg(test)]
mod tests;

#[derive(Default)]
struct Entry {
    /// The folder as it is read.
    path: PathBuf,
    /// Counts what was heard in this folder; a read keeps its answer only if it did not move.
    heard: u64,
    /// The folder git keeps this one's index and HEAD in, when it is a linked worktree.
    gitdir: Option<PathBuf>,
    /// The repo's common `.git`, shared by every folder of the repo.
    common: Option<PathBuf>,
    /// Where a flush makes this folder's marker: its gitdir, out of the work tree.
    home: Option<PathBuf>,
    /// Whether this folder is covered by a watch that is up.
    watched: bool,
    kept: Option<Read>,
    /// A read that was not kept (a refusal, or no watch to vouch for it): given to every ask
    /// while the round of reads it came from is still going, so the window's asks for the rest
    /// do not read it again, and dropped by the first ask that begins with no read going.
    once: Option<Read>,
    /// Waiting for a worker, or being read.
    reading: bool,
    /// A read finished that no ask has yet taken as news.
    fresh: bool,
    /// When its last read began.
    began: Option<Instant>,
}

impl Entry {
    /// Whether a change at `path` may change what this folder's status says.
    fn moved_by(&self, folder: &Path, path: &Path) -> bool {
        if path.starts_with(folder) || self.gitdir.as_deref().is_some_and(|g| path.starts_with(g)) {
            return true;
        }
        let Some(rest) = self
            .common
            .as_deref()
            .and_then(|c| path.strip_prefix(c).ok())
        else {
            return false;
        };
        // Inside the common `.git`, only another linked worktree's own folder is that one's
        // alone: everything else there (refs, packed-refs, config, info/exclude) is the repo's.
        let mut parts = rest.components();
        let another_worktree =
            parts.next() == Some(Component::Normal("worktrees".as_ref())) && parts.next().is_some();
        !another_worktree
    }
}

#[derive(Default)]
struct Shared {
    entries: HashMap<PathBuf, Entry>,
    queue: VecDeque<PathBuf>,
    workers: usize,
    /// The folders a watch is up on.
    dirs: HashSet<PathBuf>,
    /// Where a flush makes its markers: each watched folder's home.
    homes: HashSet<PathBuf>,
    /// Flush markers made whose event has not yet come.
    pending: HashSet<PathBuf>,
    /// Flush markers whose event has come, until their flush takes them.
    flushed: HashSet<PathBuf>,
    /// The platform could watch no more: nothing is kept again until a restart.
    limited: bool,
}

impl Shared {
    fn moved_all(&mut self) {
        for entry in self.entries.values_mut() {
            entry.heard += 1;
            entry.kept = None;
        }
    }

    fn moved(&mut self, path: &Path) {
        for (folder, entry) in &mut self.entries {
            if entry.moved_by(folder, path) {
                entry.heard += 1;
                entry.kept = None;
            }
        }
    }

    /// The platform could not watch everything it was asked to: no folder is vouched for.
    fn limit(&mut self) {
        self.limited = true;
        for entry in self.entries.values_mut() {
            entry.watched = false;
        }
        self.moved_all();
    }
}

/// Makes the watcher whose events go to a sender: the platform's, or a test's.
pub type Make = fn(Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>>;

fn platform(events: Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    let watcher = notify::recommended_watcher(handler(events)).ok()?;
    Some(Box::new(watcher))
}

/// `watchset::sender`, and the platform's watch limit told apart from any other error, since
/// past it a folder made later is never watched at all.
fn handler(events: Sender<notify::Event>) -> impl EventHandler {
    let limit = events.clone();
    let mut send = crate::watchset::sender(events);
    move |event: notify::Result<notify::Event>| match event {
        Err(err) if matches!(err.kind, notify::ErrorKind::MaxFilesWatch) => {
            let _ = limit.send(notify::Event::new(EventKind::Other).set_info(WATCH_LIMIT));
        }
        event => send.handle_event(event),
    }
}

struct Inner {
    make: Make,
    read: Box<dyn Fn(&Path) -> Read + Send + Sync>,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    /// Held only to add or remove a watch, never with `shared`'s lock waited on inside it.
    watching: Mutex<Option<Box<dyn Watcher + Send>>>,
    reads: AtomicUsize,
}

#[derive(Clone)]
pub struct StateCache(Arc<Inner>);

/// Sequence for flush marker names, across caches.
static FLUSHES: AtomicU64 = AtomicU64::new(0);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One folder's answer so far: `None` while it is still being read.
pub type Said = Option<Read>;

impl StateCache {
    pub fn new(read: impl Fn(&Path) -> Read + Send + Sync + 'static) -> Self {
        Self::watched_by(platform, read)
    }

    pub fn watched_by(make: Make, read: impl Fn(&Path) -> Read + Send + Sync + 'static) -> Self {
        Self(Arc::new(Inner {
            make,
            read: Box::new(read),
            shared: Arc::new((Mutex::default(), Condvar::new())),
            watching: Mutex::new(None),
            reads: AtomicUsize::new(0),
        }))
    }

    /// How many folders have been read, kept or not.
    #[cfg(test)]
    fn read_count(&self) -> usize {
        self.0.reads.load(Ordering::Relaxed)
    }

    /// How many folders the cache holds an entry for.
    #[cfg(test)]
    fn known(&self) -> usize {
        lock(&self.0.shared.0).entries.len()
    }

    /// How many flush markers the cache still remembers.
    #[cfg(test)]
    fn markers_held(&self) -> usize {
        let shared = lock(&self.0.shared.0);
        shared.pending.len() + shared.flushed.len()
    }

    /// Anything may have changed, as a watch that lost track says.
    #[cfg(test)]
    fn lost_track(&self) {
        lock(&self.0.shared.0).moved_all();
    }

    /// Each of `folders`' answer as far as it is known, in order, `None` for one still being
    /// read. Returns when every folder is answered, or soon after the first new answer.
    pub fn states(&self, folders: &[Folder]) -> Vec<Said> {
        let inner = &self.0;
        let keys: Vec<PathBuf> = folders
            .iter()
            .map(|f| f.path.canonicalize().unwrap_or_else(|_| f.path.clone()))
            .collect();
        let added = inner.watch(folders, &keys);
        inner.flush(added);

        let mut said: Vec<Said> = vec![None; folders.len()];
        let mut first_news: Option<Instant> = None;
        let started = Instant::now();
        let (shared, finished) = &*inner.shared;
        let mut guard = lock(shared);
        if guard.workers == 0 && guard.queue.is_empty() {
            // A new round: what nothing vouched for is read again.
            for key in &keys {
                if let Some(entry) = guard.entries.get_mut(key) {
                    entry.once = None;
                }
            }
        }
        loop {
            let now = Instant::now();
            let mut queued = false;
            // The soonest a folder that moved too lately to read yet may be read.
            let mut due: Option<Instant> = None;
            for (n, key) in keys.iter().enumerate() {
                let Some(entry) = guard.entries.get_mut(key) else {
                    continue;
                };
                // What a read finished since the last ask told is news, kept or not.
                let news = std::mem::take(&mut entry.fresh);
                if said[n].is_none() {
                    if let Some(read) = entry.kept.clone().or_else(|| entry.once.clone()) {
                        said[n] = Some(read);
                    } else if !entry.reading {
                        let ready = entry.began.map_or(now, |began| began + READ_EVERY);
                        if ready <= now {
                            entry.reading = true;
                            guard.queue.push_back(key.clone());
                            queued = true;
                        } else {
                            due = Some(due.map_or(ready, |d| d.min(ready)));
                        }
                    }
                }
                if news && said[n].is_some() && first_news.is_none() {
                    first_news = Some(now);
                }
            }
            if queued {
                inner.start_workers(&mut guard);
            }
            if said.iter().all(Option::is_some) {
                return said;
            }
            let limit = match first_news {
                Some(at) => at + GATHER,
                None => started + NOTHING_WITHIN,
            };
            let left = limit.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return said;
            }
            let wake = due.map_or(left, |d| {
                left.min(d.saturating_duration_since(Instant::now()))
            });
            guard = finished
                .wait_timeout(guard, wake)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// Where git keeps a folder's state, as far as the watch needs it.
#[derive(Default)]
struct Located {
    gitdir: Option<PathBuf>,
    common: Option<PathBuf>,
}

impl Located {
    /// The folders to watch for this folder: itself, and the repo's common `.git` when that is
    /// elsewhere (a linked worktree's own gitdir lies inside it).
    /// Where a flush makes this folder's marker: its own gitdir (a main clone's is `.git`), so
    /// the work tree never sees one. A folder that is no repo has only itself, where no
    /// `git status` can be misled by it.
    fn home(&self, folder: &Path) -> PathBuf {
        self.gitdir
            .clone()
            .or_else(|| self.common.clone().filter(|c| c.starts_with(folder)))
            .unwrap_or_else(|| folder.to_path_buf())
    }

    fn roots(&self, folder: &Path) -> Vec<PathBuf> {
        let mut roots = vec![folder.to_path_buf()];
        roots.extend(
            self.common
                .iter()
                .filter(|c| !c.starts_with(folder))
                .cloned(),
        );
        roots
    }
}

impl Inner {
    /// Starts workers, up to `READ_AT_ONCE`, for what is queued.
    fn start_workers(self: &Arc<Self>, shared: &mut Shared) {
        while shared.workers < READ_AT_ONCE && shared.workers < shared.queue.len() {
            shared.workers += 1;
            let inner = Arc::clone(self);
            if std::thread::Builder::new()
                .name("purlis-state-read".into())
                .spawn(move || inner.work())
                .is_err()
            {
                shared.workers -= 1;
                break;
            }
        }
    }

    /// Reads what is queued until nothing is.
    fn work(&self) {
        let (shared, finished) = &*self.shared;
        loop {
            let (key, path, heard) = {
                let mut guard = lock(shared);
                let Some(key) = guard.queue.pop_front() else {
                    guard.workers -= 1;
                    return;
                };
                let Some(entry) = guard.entries.get_mut(&key) else {
                    continue;
                };
                entry.began = Some(Instant::now());
                (key, entry.path.clone(), entry.heard)
            };
            self.reads.fetch_add(1, Ordering::Relaxed);
            let said = (self.read)(&path);
            let mut guard = lock(shared);
            let Some(entry) = guard.entries.get_mut(&key) else {
                continue;
            };
            entry.reading = false;
            if entry.heard == heard {
                entry.fresh = true;
                if entry.watched && said.1.is_none() {
                    entry.kept = Some(said);
                } else {
                    entry.once = Some(said);
                }
            }
            // Otherwise something moved while git read: the answer may already be wrong, so
            // it is never shown, and the next ask reads it again, once `READ_EVERY` allows.
            finished.notify_all();
        }
    }

    /// Makes an entry for each new folder, watching the folder and its repo's `.git` before any
    /// read of it, and drops the entries (and the watches) of folders gone from their repo's
    /// listing. A watch that cannot be had leaves its folders unkept: read every time. Whether
    /// any entry is new: what the platform still has to say of before the watch began is heard,
    /// by a flush, before a read is stamped.
    ///
    /// The watches are added with only `watching` held, so the reads and asks going on meanwhile
    /// are not held up by a walk of a large folder.
    fn watch(&self, folders: &[Folder], keys: &[PathBuf]) -> bool {
        let new: Vec<usize> = {
            let shared = lock(&self.shared.0);
            (0..keys.len())
                .filter(|&n| !shared.entries.contains_key(&keys[n]))
                .collect()
        };
        let located: Vec<Option<Located>> = new.iter().map(|&n| locate(&keys[n])).collect();
        let listed: HashSet<&PathBuf> = keys.iter().collect();
        let gone: Vec<(PathBuf, Option<PathBuf>)> = {
            let shared = lock(&self.shared.0);
            let mut commons: HashSet<&PathBuf> = located
                .iter()
                .flatten()
                .filter_map(|l| l.common.as_ref())
                .collect();
            commons.extend(
                keys.iter()
                    .filter_map(|k| shared.entries.get(k)?.common.as_ref()),
            );
            shared
                .entries
                .iter()
                .filter(|(key, entry)| {
                    !listed.contains(key)
                        && entry.common.as_ref().is_some_and(|c| commons.contains(c))
                })
                .map(|(key, entry)| (key.clone(), entry.common.clone()))
                .collect()
        };
        if new.is_empty() && gone.is_empty() {
            return false;
        }

        let mut watching = lock(&self.watching);
        if watching.is_none() {
            *watching = self.start();
        }
        let (mut dirs, staying) = {
            let shared = lock(&self.shared.0);
            let gone: HashSet<&PathBuf> = gone.iter().map(|(key, _)| key).collect();
            let staying: Vec<Vec<PathBuf>> = shared
                .entries
                .iter()
                .filter(|(key, _)| !gone.contains(key))
                .map(|(key, entry)| {
                    let located = Located {
                        gitdir: None,
                        common: entry.common.clone(),
                    };
                    located.roots(key)
                })
                .collect();
            (shared.dirs.clone(), staying)
        };
        let mut changed = false;
        let mut up: Vec<bool> = Vec::with_capacity(new.len());
        for (&n, located) in new.iter().zip(&located) {
            let Some(located) = located else {
                // A `.git` naming a folder that cannot be found: nothing vouches for it.
                up.push(false);
                continue;
            };
            sweep(&located.home(&keys[n]));
            let all = watching.as_mut().is_some_and(|w| {
                located.roots(&keys[n]).into_iter().all(|dir| {
                    if dirs.contains(&dir) {
                        return true;
                    }
                    let ok = w.watch(&dir, RecursiveMode::Recursive).is_ok();
                    // Even a watch that failed may have restarted the platform's stream.
                    changed = true;
                    ok && dirs.insert(dir)
                })
            });
            up.push(all);
        }
        // A gone folder's watches, and its repo's when no folder still listed uses it.
        let mut keep: HashSet<PathBuf> = staying.into_iter().flatten().collect();
        for (&n, located) in new.iter().zip(&located) {
            keep.extend(located.iter().flat_map(|l| l.roots(&keys[n])));
        }
        for (key, common) in &gone {
            for dir in std::iter::once(key).chain(common) {
                if !keep.contains(dir) && dirs.remove(dir) {
                    if let Some(w) = watching.as_mut() {
                        let _ = w.unwatch(dir);
                    }
                    changed = true;
                }
            }
        }

        let mut shared = lock(&self.shared.0);
        shared.dirs = dirs;
        for (key, _) in &gone {
            shared.entries.remove(key);
        }
        for ((&n, located), up) in new.iter().zip(located).zip(up) {
            let home = located.as_ref().map(|l| l.home(&keys[n]));
            let located = located.unwrap_or_default();
            let watched = up && !shared.limited;
            shared.entries.entry(keys[n].clone()).or_insert(Entry {
                path: folders[n].path.clone(),
                home,
                gitdir: located.gitdir,
                common: located.common,
                watched,
                ..Entry::default()
            });
        }
        let homes = shared
            .entries
            .values()
            .filter(|e| e.watched)
            .filter_map(|e| e.home.clone())
            .collect();
        shared.homes = homes;
        if changed {
            // FSEvents starts its stream again from now for each watch added or removed: what
            // happened while it did is never told.
            shared.moved_all();
        }
        !new.is_empty()
    }

    fn start(&self) -> Option<Box<dyn Watcher + Send>> {
        let (events, heard) = std::sync::mpsc::channel();
        let watcher = (self.make)(events)?;
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("purlis-state-watch".into())
            .spawn(move || hear(&heard, &shared))
            .ok()?;
        Some(watcher)
    }

    /// Waits until everything that happened before now has been heard, by making a marker in
    /// each watched folder's gitdir and waiting for its event: the platform's events come in
    /// order (one FSEvents stream, one inotify queue). Failing that in time, every folder counts
    /// as moved.
    fn flush(&self, added: bool) {
        let (shared, heard) = &*self.shared;
        let markers: Vec<PathBuf> = {
            let mut guard = lock(shared);
            if !added && !guard.entries.values().any(|e| e.kept.is_some()) {
                return;
            }
            let markers: Vec<PathBuf> = guard
                .homes
                .iter()
                .map(|dir| {
                    let n = FLUSHES.fetch_add(1, Ordering::Relaxed);
                    dir.join(format!("{FLUSH_PREFIX}{}-{n}", std::process::id()))
                })
                .collect();
            guard.pending.extend(markers.iter().cloned());
            markers
        };
        let made: Vec<&PathBuf> = markers
            .iter()
            .filter(|marker| std::fs::write(marker, b"").is_ok())
            .collect();
        let deadline = Instant::now() + FLUSH_WITHIN;
        let mut guard = lock(shared);
        let mut ok = made.len() == markers.len();
        while ok && !made.iter().all(|m| guard.flushed.contains(*m)) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                ok = false;
                break;
            }
            guard = heard
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        for marker in &markers {
            guard.pending.remove(marker);
            guard.flushed.remove(marker);
        }
        if !ok {
            guard.moved_all();
        }
        drop(guard);
        for marker in made {
            let _ = std::fs::remove_file(marker);
        }
    }
}

/// Folds each event into what moved. A rescan, or an event naming no path, moves every folder.
fn hear(events: &Receiver<notify::Event>, shared: &(Mutex<Shared>, Condvar)) {
    while let Ok(event) = events.recv() {
        let mut state = lock(&shared.0);
        if event.info() == Some(WATCH_LIMIT) {
            state.limit();
            continue;
        }
        if event.need_rescan() || event.paths.is_empty() {
            state.moved_all();
            continue;
        }
        // A watched folder's or a marker home's own time changing only says something in it
        // was made or removed, which its own event names: the marker a flush made there would
        // otherwise move the folder.
        let own_time = matches!(event.kind, EventKind::Modify(ModifyKind::Metadata(_)));
        for path in &event.paths {
            let marker = path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(FLUSH_PREFIX));
            if marker {
                // Only a marker a flush still waits for is remembered, so none piles up.
                if state.pending.contains(path) {
                    state.flushed.insert(path.clone());
                }
            } else if !(own_time && (state.dirs.contains(path) || state.homes.contains(path))) {
                state.moved(path);
            }
        }
        shared.1.notify_all();
    }
}

/// Removes the flush markers a run that ended without removing its own left in `dir`.
fn sweep(dir: &Path) {
    let ours = format!("{FLUSH_PREFIX}{}-", std::process::id());
    let Ok(listing) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in listing.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(FLUSH_PREFIX) && !name.starts_with(&ours) {
            let path = entry.path();
            let _ = std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path));
        }
    }
}

/// Where git keeps `folder`'s state. A `.git` file naming a folder that cannot be found, or
/// that names none, is `None`: the folder is never vouched for. A folder with no `.git` has
/// nothing more than itself to watch (its read refuses, unless it is not a repo at all).
fn locate(folder: &Path) -> Option<Located> {
    let dot = folder.join(".git");
    let Ok(meta) = std::fs::metadata(&dot) else {
        return Some(Located::default());
    };
    if meta.is_dir() {
        return Some(Located {
            gitdir: None,
            common: Some(dot),
        });
    }
    let said = std::fs::read_to_string(&dot).ok()?;
    // Relative to the folder, as git reads it (`worktree.useRelativePaths`, git 2.48).
    let gitdir = folder
        .join(said.strip_prefix("gitdir:")?.trim())
        .canonicalize()
        .ok()?;
    // A linked worktree's gitdir names the repo's own `.git` in `commondir`; one without it
    // (a submodule's) is its own common dir.
    let common = match std::fs::read_to_string(gitdir.join("commondir")) {
        Ok(named) => gitdir.join(named.trim()).canonicalize().ok()?,
        Err(_) => gitdir.clone(),
    };
    Some(Located {
        gitdir: Some(gitdir),
        common: Some(common),
    })
}
