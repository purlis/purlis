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
//! heard that could change it: any event in the folder, or in the folder git keeps its index
//! in; and **the watch failing closed**, as `watchset::sender` makes every watcher: an error or
//! a lost track is a rescan, and a rescan moves every folder. An answer read while an event
//! arrived is not kept. Events reach a reader late, so before a kept answer is served the
//! watch is flushed: a file is written into each watched folder and its event waited for, and
//! events of one watch come in order, so everything before it has been heard. A flush that
//! times out moves every folder. A restart keeps nothing.
//!
//! Only a successful read is kept; a refusal is read again every time.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

/// What one folder's read said: the count of uncommitted files, or why it could not be read.
pub type Read = (Option<u32>, Option<String>);

/// One folder to answer for.
#[derive(Debug, Clone)]
pub struct Folder {
    pub piece: String,
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

/// How long a flush waits for its file's event before every folder counts as moved.
const FLUSH_WITHIN: Duration = Duration::from_secs(2);

/// The name every flush file starts with. An event naming one moves no folder.
const FLUSH_PREFIX: &str = ".purlis-flush-";

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
    /// Whether this folder is covered by a watch that is up.
    watched: bool,
    kept: Option<Read>,
    /// A read that was not kept (a refusal, or no watch to vouch for it), given to the next ask
    /// once and then dropped, so the ask after it reads again.
    once: Option<Read>,
    /// Waiting for a worker, or being read.
    reading: bool,
    /// A read finished that no ask has yet taken as news.
    fresh: bool,
}

#[derive(Default)]
struct Shared {
    entries: HashMap<PathBuf, Entry>,
    queue: VecDeque<PathBuf>,
    workers: usize,
    /// Flush files whose event has come.
    flushed: HashSet<PathBuf>,
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
            let inside = path.starts_with(folder)
                || entry.gitdir.as_deref().is_some_and(|g| path.starts_with(g));
            if inside {
                entry.heard += 1;
                entry.kept = None;
            }
        }
    }
}

struct Watching {
    watcher: Box<dyn Watcher + Send>,
    dirs: HashSet<PathBuf>,
}

/// Makes the watcher whose events go to a sender: the platform's, or a test's.
pub type Make = fn(std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>>;

fn platform(events: std::sync::mpsc::Sender<notify::Event>) -> Option<Box<dyn Watcher + Send>> {
    let watcher = notify::recommended_watcher(crate::watchset::sender(events)).ok()?;
    Some(Box::new(watcher))
}

struct Inner {
    make: Make,
    read: Box<dyn Fn(&Path) -> Read + Send + Sync>,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    watching: Mutex<Option<Watching>>,
    reads: AtomicUsize,
}

#[derive(Clone)]
pub struct StateCache(Arc<Inner>);

/// Sequence for flush file names, across caches.
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
        loop {
            let mut queued = false;
            for (n, key) in keys.iter().enumerate() {
                let Some(entry) = guard.entries.get_mut(key) else {
                    continue;
                };
                // What a read finished since the last ask told is news, kept or not.
                let news = std::mem::take(&mut entry.fresh);
                if said[n].is_none() {
                    if let Some(read) = entry.kept.clone().or_else(|| entry.once.take()) {
                        said[n] = Some(read);
                    } else if !entry.reading {
                        entry.reading = true;
                        guard.queue.push_back(key.clone());
                        queued = true;
                    }
                }
                if news && said[n].is_some() && first_news.is_none() {
                    first_news = Some(Instant::now());
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
            guard = finished
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
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
                let Some(entry) = guard.entries.get(&key) else {
                    continue;
                };
                (key, entry.path.clone(), entry.heard)
            };
            self.reads.fetch_add(1, Ordering::Relaxed);
            let said = (self.read)(&path);
            let mut guard = lock(shared);
            let Some(entry) = guard.entries.get_mut(&key) else {
                continue;
            };
            if entry.heard != heard {
                // Something moved while git read: the answer may already be wrong, so it is
                // read again, and never shown.
                guard.queue.push_back(key);
            } else {
                entry.reading = false;
                entry.fresh = true;
                if entry.watched && said.1.is_none() {
                    entry.kept = Some(said);
                } else {
                    entry.once = Some(said);
                }
            }
            finished.notify_all();
        }
    }

    /// Makes an entry for each folder and watches what it lives in, before any read of it.
    /// A watch that cannot be had leaves its folders unkept: read every time. Whether any
    /// entry is new: what the platform still has to say of before the watch began is heard, by
    /// a flush, before a read is stamped.
    fn watch(&self, folders: &[Folder], keys: &[PathBuf]) -> bool {
        let mut added = false;
        let mut watching = lock(&self.watching);
        if watching.is_none() {
            *watching = self.start();
        }
        let mut shared = lock(&self.shared.0);
        for (folder, key) in folders.iter().zip(keys) {
            if shared.entries.contains_key(key) {
                continue;
            }
            added = true;
            let gitdir = gitdir_of(key);
            let mut dirs: Vec<PathBuf> = key.parent().map(Path::to_path_buf).into_iter().collect();
            if let Some(common) = gitdir.as_deref().and_then(common_dir) {
                dirs.push(common);
            }
            let up = watching.as_mut().is_some_and(|w| {
                dirs.into_iter().all(|dir| {
                    w.dirs.contains(&dir)
                        || w.watcher.watch(&dir, RecursiveMode::Recursive).is_ok()
                            && w.dirs.insert(dir)
                })
            });
            shared.entries.insert(
                key.clone(),
                Entry {
                    path: folder.path.clone(),
                    gitdir,
                    watched: up,
                    ..Entry::default()
                },
            );
        }
        added
    }

    fn start(&self) -> Option<Watching> {
        let (events, heard) = std::sync::mpsc::channel();
        let watcher = (self.make)(events)?;
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("purlis-state-watch".into())
            .spawn(move || hear(&heard, &shared))
            .ok()?;
        Some(Watching {
            watcher,
            dirs: HashSet::new(),
        })
    }

    /// Waits until everything that happened before now has been heard, by writing a file into
    /// each watched folder and waiting for its event: a watch's events come in order. Failing
    /// that in time, every folder counts as moved.
    fn flush(&self, added: bool) {
        let dirs: Vec<PathBuf> = {
            let watching = lock(&self.watching);
            let shared = lock(&self.shared.0);
            if !added && !shared.entries.values().any(|e| e.kept.is_some()) {
                return;
            }
            watching
                .as_ref()
                .map(|w| w.dirs.iter().cloned().collect())
                .unwrap_or_default()
        };
        let files: Vec<PathBuf> = dirs
            .iter()
            .map(|dir| {
                let n = FLUSHES.fetch_add(1, Ordering::Relaxed);
                dir.join(format!("{FLUSH_PREFIX}{}-{n}", std::process::id()))
            })
            .collect();
        let written: Vec<&PathBuf> = files
            .iter()
            .filter(|file| std::fs::write(file, b"").is_ok())
            .collect();
        let (shared, heard) = &*self.shared;
        let deadline = Instant::now() + FLUSH_WITHIN;
        let mut guard = lock(shared);
        let mut ok = written.len() == files.len();
        while ok && !written.iter().all(|f| guard.flushed.contains(*f)) {
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
        for file in &files {
            guard.flushed.remove(file);
        }
        if !ok {
            guard.moved_all();
        }
        drop(guard);
        for file in written {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// Folds each event into what moved. A rescan, or an event naming no path, moves every folder.
fn hear(events: &Receiver<notify::Event>, shared: &(Mutex<Shared>, Condvar)) {
    while let Ok(event) = events.recv() {
        let mut state = lock(&shared.0);
        if event.need_rescan() || event.paths.is_empty() {
            state.moved_all();
            continue;
        }
        for path in &event.paths {
            let flush = path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(FLUSH_PREFIX));
            if flush {
                state.flushed.insert(path.clone());
            } else {
                state.moved(path);
            }
        }
        shared.1.notify_all();
    }
}

/// A linked worktree's `.git` is a file naming its folder inside the repo's `.git/worktrees`.
fn gitdir_of(folder: &Path) -> Option<PathBuf> {
    let said = std::fs::read_to_string(folder.join(".git")).ok()?;
    let dir = PathBuf::from(said.strip_prefix("gitdir:")?.trim());
    dir.canonicalize().ok()
}

/// The repo's own `.git`, which a linked worktree's folder sits in two levels down.
fn common_dir(gitdir: &Path) -> Option<PathBuf> {
    let worktrees = gitdir.parent()?;
    (worktrees.file_name()? == "worktrees").then(|| worktrees.parent().map(Path::to_path_buf))?
}
