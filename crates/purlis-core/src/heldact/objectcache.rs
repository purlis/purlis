// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The object cache a held push's base is read from** (ADR 0128, amended 2026-11-23 and
//! 2026-11-24): one repository of the app's own per forge repo, so that a push fetches what
//! changed since the last one and not the repo's whole history again.
//!
//! A quarantine ([`super::Quarantine`]) used to fetch the base and its history into an empty
//! repository on every push. On a large repo that took longer than the 120 s a network call is
//! given ([`git::NETWORK`]), and every push was refused. Now:
//!
//! 1. The base (and the advertised tips, where a thin pack needs them) are fetched into **this
//!    cache**, incrementally: its refs tell the forge what it holds.
//! 2. The quarantine names the cache's object store as an **alternate**
//!    (`objects/info/alternates`), so its own store holds only the pushed pack.
//!
//! **Why an alternate is safe.** An object is its hash: git reads a pushed commit's objects by
//! walking the commit graph from the pushed tip down to the base (`rev-list --objects tip --not
//! base`), and every object so reached is read and scanned wherever it is stored. The cache can
//! hold more than the base does (other branches' objects, an older base), and a pack that leaves
//! out an object the cache holds now passes `index-pack --strict` where it was once fetched for;
//! that object is what the forge sent, hashed by git on the way in, and the walk still reads it
//! when the pushed tip reaches it. Nothing the walk reaches is skipped for being in the cache. No
//! chat can write the cache: it is in a folder of the app's own that the sandbox denies every chat
//! to read and to write, and a chat names no path in it.
//!
//! **Time.** A push is answered inside the listener's connection total, so **the first fill of a
//! cache never runs on the connection**: the push that finds the cache empty starts the fill on a
//! thread of the app's own, owned by the cache and bounded by [`FILL`], and is refused at once
//! with what is happening. A push to a filled cache tops it up within [`PUSH`], for all the
//! fetches of that push together; a top-up that does not finish in time is refused with that
//! reason and finished on the same thread, so the next push finds it done.
//!
//! **Concurrency.** One lock per cache in the app (the app is one per user); git's own locks
//! guard the rest. A push that arrives while a fill runs is refused at once, never left to time
//! out; a push that arrives during a top-up waits for it, within its own bound.
//!
//! **Size and repair.** `git gc --auto` after an update; a cache past its cap ([`MOST`]) is made
//! again, but only when no quarantine is using it, since a held push's quarantine reads through
//! it until the ask ends. A repo whose history alone is past the cap is refused with that reason
//! and left alone, not deleted and fetched again on every push. A cache that git does not
//! recognise as a repository is made again, when no quarantine is using it. A `tmp_pack_*` a
//! killed fetch left is removed before the next one.
//!
//! **Tier** (ADR 0069): Machine, device-bound, **rebuildable**: nothing in it is not on the forge.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::worktree::git;

/// How long a fill's fetch may take: the first fill of a large repo's history. It runs on a thread
/// of the app's own, never on a chat's connection. Apart from [`git::NETWORK`], which holds every
/// other call.
pub const FILL: Duration = Duration::from_secs(15 * 60);

/// **How long all the fetches of one push may take together**, well inside the listener's
/// connection total (10 minutes), so that every refusal's text reaches the chat and a verdict is
/// never written to a connection that has been dropped.
pub const PUSH: Duration = Duration::from_secs(240);

/// The size a cache may reach before it is made again (10 GiB).
pub const MOST: u64 = 10 * 1024 * 1024 * 1024;

/// The base refs kept in a cache: one is made per distinct base, so the least recently used are
/// dropped.
const BASES_KEPT: usize = 32;

/// The file that says a fetch into the cache has finished once. It holds the size of the object
/// store then, in bytes.
const FILLED: &str = "purlis-filled";

/// The file of the bases used last, newest first, one id to a line.
const BASES: &str = "purlis-bases";

/// How long `git gc --auto` may take.
const GC: Duration = Duration::from_secs(300);

/// How often a push waiting for a top-up looks again.
const LOOK_AGAIN: Duration = Duration::from_millis(50);

/// What fetches into a cache: the repository it is given, and how long it may take. It is owned,
/// so that a fill can go on after the push that began it has been answered.
pub type Fetch = Arc<dyn Fn(&Path, Duration) -> Result<(), String> + Send + Sync>;

static CACHES: LazyLock<Mutex<HashMap<PathBuf, Arc<ObjectCache>>>> = LazyLock::new(Mutex::default);

/// **A repository of the app's own that holds a forge repo's objects**, shared by every push to it.
pub struct ObjectCache {
    path: PathBuf,
    label: String,
    most: u64,
    lock: Mutex<()>,
    leases: AtomicUsize,
    /// A fill is running (or waiting for the lock) on the cache's own thread.
    filling: AtomicBool,
    /// A fill failed, or the cache was made again: the text says "again", not "for the first time".
    tried: AtomicBool,
    failure: Mutex<Option<String>>,
}

/// A quarantine's claim on a cache: while one is held the cache is not made again.
pub struct Lease(Arc<ObjectCache>);

impl Drop for Lease {
    fn drop(&mut self) {
        self.0.leases.fetch_sub(1, Ordering::AcqRel);
    }
}

/// **A cache as one push uses it**: every fetch of the push shares one deadline ([`PUSH`]).
#[derive(Clone)]
pub struct PushCache {
    cache: Arc<ObjectCache>,
    deadline: Instant,
}

impl PushCache {
    /// The cache itself.
    pub fn cache(&self) -> &Arc<ObjectCache> {
        &self.cache
    }

    /// What is left of this push's time.
    pub fn left(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// [`ObjectCache::update`] within what is left of this push's time.
    pub fn update(&self, fetch: Fetch) -> Result<(), String> {
        self.cache.update(self.deadline, fetch)
    }
}

impl ObjectCache {
    /// The cache of `repo` at `host` under `parent` (a folder of the app's own). Nothing is made
    /// on disk until the first [`Self::update`]. The folder is named by a hash, so no name a forge
    /// or a chat gives reaches a path.
    pub fn of(parent: &Path, host: &str, repo: &str) -> Arc<ObjectCache> {
        Self::of_capped(parent, host, repo, MOST)
    }

    /// [`Self::of`] with a cap of the caller's own instead of [`MOST`]: for a test.
    pub fn of_capped(parent: &Path, host: &str, repo: &str, most: u64) -> Arc<ObjectCache> {
        let key = format!(
            "{}\n{}",
            host.to_ascii_lowercase(),
            repo.to_ascii_lowercase()
        );
        let name: String = Sha256::digest(key.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let path = parent.join(name);
        let mut caches = CACHES.lock().unwrap_or_else(PoisonError::into_inner);
        caches
            .entry(path.clone())
            .or_insert_with(|| {
                Arc::new(ObjectCache {
                    path,
                    label: format!("{host}/{repo}"),
                    most,
                    lock: Mutex::new(()),
                    leases: AtomicUsize::new(0),
                    filling: AtomicBool::new(false),
                    tried: AtomicBool::new(false),
                    failure: Mutex::new(None),
                })
            })
            .clone()
    }

    /// This cache for one push, which has `within` for all its fetches.
    pub fn for_a_push(self: &Arc<Self>, within: Duration) -> PushCache {
        PushCache {
            cache: self.clone(),
            deadline: Instant::now() + within,
        }
    }

    /// The cache's repository.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The cache's object store: what a quarantine names as an alternate.
    pub fn objects(&self) -> PathBuf {
        self.path.join(".git").join("objects")
    }

    /// Claims the cache for a quarantine.
    pub fn lease(self: &Arc<Self>) -> Lease {
        self.leases.fetch_add(1, Ordering::AcqRel);
        Lease(self.clone())
    }

    /// Whether a fetch into the cache has finished once.
    pub fn is_filled(&self) -> bool {
        self.path.join(".git").join(FILLED).is_file()
    }

    /// Whether a fill is running now.
    pub fn is_filling(&self) -> bool {
        self.filling.load(Ordering::Acquire)
    }

    /// What a push is told while the cache is being filled.
    fn reading(&self) -> String {
        let when = if self.tried.load(Ordering::Acquire) {
            "again"
        } else {
            "for the first time"
        };
        let last = self
            .failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .map(|why| format!(" (the last read failed: {why})"))
            .unwrap_or_default();
        format!(
            "purlis is still reading {} from the forge {when}{last}; try the push again in a few minutes",
            self.label
        )
    }

    /// **Runs `fetch` into the cache and tidies it after**, for a push whose time ends at
    /// `deadline`.
    ///
    /// - A cache that is not filled is **filled on a thread of its own**, never on the push's
    ///   connection, and the push is **refused at once** with what is happening. The same where a
    ///   fill is already running.
    /// - A filled cache is topped up here, within what is left of the push's time. A top-up that
    ///   runs out of it is refused with that reason and finished on the cache's thread.
    pub fn update(self: &Arc<Self>, deadline: Instant, fetch: Fetch) -> Result<(), String> {
        if self.is_filling() {
            return Err(self.reading());
        }
        let held = self.lock_until(deadline)?;
        // The lock may have been waited for while a fill was started.
        if self.is_filling() {
            return Err(self.reading());
        }
        self.make_ready()?;
        if !self.is_filled() {
            self.fill_in_background(fetch);
            return Err(self.reading());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(self.timed_out(fetch));
        }
        if let Err(why) = fetch(&self.path, left) {
            return Err(if Instant::now() >= deadline {
                self.timed_out(fetch)
            } else {
                why
            });
        }
        self.tidy();
        drop(held);
        Ok(())
    }

    /// A top-up that outran its push's time: finished on the cache's own thread, and the push told.
    fn timed_out(self: &Arc<Self>, fetch: Fetch) -> String {
        self.tried.store(true, Ordering::Release);
        self.fill_in_background(fetch);
        format!(
            "purlis could not finish reading {} from the forge within {} s, and is reading on in the background; try the push again in a few minutes",
            self.label,
            PUSH.as_secs()
        )
    }

    /// The cache's lock, waited for until `deadline` without parking a thread on it.
    fn lock_until(&self, deadline: Instant) -> Result<MutexGuard<'_, ()>, String> {
        loop {
            match self.lock.try_lock() {
                Ok(held) => return Ok(held),
                Err(TryLockError::Poisoned(held)) => return Ok(held.into_inner()),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "purlis waited {} s for another push's read of {} and gave up; try the push again in a moment",
                            PUSH.as_secs(),
                            self.label
                        ));
                    }
                    std::thread::sleep(LOOK_AGAIN);
                }
            }
        }
    }

    /// Starts the fill on a thread of the app's own: it takes the lock, makes the cache ready,
    /// fetches within [`FILL`] and says it is filled. A failure leaves the cache **not filled**.
    fn fill_in_background(self: &Arc<Self>, fetch: Fetch) {
        if self.filling.swap(true, Ordering::AcqRel) {
            return;
        }
        let cache = self.clone();
        let started = std::thread::Builder::new()
            .name("object-cache-fill".to_owned())
            .spawn(move || {
                let done = {
                    let _held = cache.lock.lock().unwrap_or_else(PoisonError::into_inner);
                    cache
                        .make_ready()
                        .and_then(|()| fetch(&cache.path, FILL))
                        .and_then(|()| cache.mark_filled())
                        .inspect(|()| cache.tidy())
                };
                match done {
                    Ok(()) => {
                        *cache
                            .failure
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner) = None;
                    }
                    Err(why) => {
                        cache.tried.store(true, Ordering::Release);
                        let _ = std::fs::remove_file(cache.path.join(".git").join(FILLED));
                        *cache
                            .failure
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner) = Some(why);
                    }
                }
                cache.filling.store(false, Ordering::Release);
            });
        if started.is_err() {
            self.filling.store(false, Ordering::Release);
        }
    }

    fn mark_filled(&self) -> Result<(), String> {
        std::fs::write(
            self.path.join(".git").join(FILLED),
            size(&self.objects()).to_string(),
        )
        .map_err(|why| format!("purlis could not write its object cache ({why})"))
    }

    /// The size the object store had when the last fill finished.
    fn filled_at(&self) -> Option<u64> {
        std::fs::read_to_string(self.path.join(".git").join(FILLED))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// After an update: the least recently used bases go, and git tidies its own store.
    fn tidy(&self) {
        // Detached by git itself where it runs; a failure is not the push's.
        let _ = git::run(&self.path, &["gc", "--auto", "--quiet"], GC);
    }

    /// Whether git takes the folder for the repository it was made as: the folder's own files are
    /// all there, and git, run in it, names `.git` as its repository (an empty `.git` is not one,
    /// and git would otherwise look for one in a folder above).
    fn is_sound(&self) -> bool {
        let dot = self.path.join(".git");
        if !dot.join("HEAD").is_file() || !dot.join("objects").is_dir() || !dot.join("refs").is_dir()
        {
            return false;
        }
        git::run(&self.path, &["rev-parse", "--git-dir"], git::READ)
            .is_ok_and(|run| run.ok() && run.out.trim() == ".git")
    }

    /// Removes what a killed fetch left in the object store.
    fn remove_leftovers(&self) {
        let Ok(entries) = std::fs::read_dir(self.objects().join("pack")) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("tmp_pack_") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// The repository exists and git takes it for one, and is made again where it is damaged or
    /// has outgrown its cap while no quarantine is using it. A repo whose history alone is past the
    /// cap is refused, not made again.
    fn make_ready(&self) -> Result<(), String> {
        let exists = self.path.exists();
        if exists && self.filled_at().is_some_and(|filled| filled > self.most) {
            return Err(format!(
                "{} holds more history than the {} GiB purlis keeps for one repo, so it cannot read it for a push",
                self.label,
                self.most / (1024 * 1024 * 1024)
            ));
        }
        // The quarantine being opened holds one lease already.
        let alone = self.leases.load(Ordering::Acquire) <= 1;
        let sound = exists && self.is_sound();
        let too_big = sound && size(&self.objects()) > self.most;
        if sound && !(too_big && alone) {
            self.remove_leftovers();
            return Ok(());
        }
        if exists && !alone {
            return Err(format!(
                "purlis's object cache for {} is damaged and a push is reading it; try the push again in a moment",
                self.label
            ));
        }
        if exists {
            std::fs::remove_dir_all(&self.path).map_err(|why| why.to_string())?;
            self.tried.store(true, Ordering::Release);
        }
        std::fs::create_dir_all(&self.path).map_err(|why| why.to_string())?;
        let made = git::run(
            &self.path,
            &["init", "--quiet", &self.path.to_string_lossy()],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        if made.ok() {
            Ok(())
        } else {
            Err("git could not make purlis's object cache".to_owned())
        }
    }

    /// **Records that `base` was just used** and drops the base refs used least recently, past the
    /// [`BASES_KEPT`] newest. The base just fetched is always kept, and the order is of use (a file
    /// of ids, newest first), not of commit date: an old base a push is read against now is the
    /// newest use. A ref holds objects alive and tells the forge what the cache has.
    pub fn keep_base(&self, base: &str) {
        let list = self.path.join(".git").join(BASES);
        let base = base.to_ascii_lowercase();
        let mut recent: Vec<String> = std::fs::read_to_string(&list)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .filter(|id| *id != base)
            .collect();
        recent.insert(0, base);
        recent.truncate(BASES_KEPT);
        let _ = std::fs::write(&list, recent.join("\n"));
        let Ok(listed) = git::run(
            &self.path,
            &["for-each-ref", "--format=%(refname)", "refs/quarantine/base"],
            git::READ,
        ) else {
            return;
        };
        for name in listed.out.lines() {
            let id = name.rsplit('/').next().unwrap_or_default();
            if !recent.iter().any(|kept| kept.eq_ignore_ascii_case(id)) {
                let _ = git::run(&self.path, &["update-ref", "-d", name], git::READ);
            }
        }
    }

    /// Drops the tip refs the forge no longer advertises: `keep` are the ones it does.
    pub fn keep_tips(&self, keep: &[String]) {
        let Ok(listed) = git::run(
            &self.path,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/quarantine/tips",
            ],
            git::READ,
        ) else {
            return;
        };
        for name in listed.out.lines() {
            let id = name.rsplit('/').next().unwrap_or_default();
            if !keep.iter().any(|tip| tip.eq_ignore_ascii_case(id)) {
                let _ = git::run(&self.path, &["update-ref", "-d", name], git::READ);
            }
        }
    }
}

/// The bytes under `dir` (links are not followed).
pub(super) fn size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => size(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}
