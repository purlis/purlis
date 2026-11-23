// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The object cache a held push's base is read from** (ADR 0128, amended 2026-11-23): one
//! repository of the app's own per forge repo, so that a push fetches what changed since the last
//! one and not the repo's whole history again.
//!
//! A quarantine ([`super::Quarantine`]) used to fetch the base and its history into an empty
//! repository on every push. On a large repo that took longer than the 120 s a network call is
//! given ([`git::NETWORK`]), and every push was refused. Now:
//!
//! 1. The base (and the advertised tips, where a thin pack needs them) are fetched into **this
//!    cache**, incrementally: its refs tell the forge what it holds. A first fill is bounded by
//!    [`FILL`], apart from the per-call limit.
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
//! chat can write the cache: it is in a folder of the app's own, and a chat names no path in it.
//!
//! **Concurrency.** One lock per cache in the app (the app is one per user); git's own locks
//! guard the rest. A push that arrives while the cache is being filled for the first time is
//! refused at once with what is happening, not left to time out; once it has been filled, a push
//! waits for the update in progress, which is short.
//!
//! **Size.** `git gc --auto` after an update; a cache past [`MOST`] is made again, but only when
//! no quarantine is using it, since a held push's quarantine reads through it until the ask ends.
//!
//! **Tier** (ADR 0069): Machine, device-bound, **rebuildable**: nothing in it is not on the forge.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError, TryLockError};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::worktree::git;

/// How long one fetch into the cache may take: the first fill of a large repo's history. Apart
/// from [`git::NETWORK`], which holds every other call.
pub const FILL: Duration = Duration::from_secs(15 * 60);

/// The size a cache may reach before it is made again (10 GiB).
pub const MOST: u64 = 10 * 1024 * 1024 * 1024;

/// The base refs kept in a cache: one is made per distinct base, so the oldest are dropped.
const BASES_KEPT: usize = 32;

/// The file that says a fetch into the cache has finished once.
const FILLED: &str = "purlis-filled";

/// How long `git gc --auto` may take.
const GC: Duration = Duration::from_secs(300);

static CACHES: LazyLock<Mutex<HashMap<PathBuf, Arc<ObjectCache>>>> = LazyLock::new(Mutex::default);

/// **A repository of the app's own that holds a forge repo's objects**, shared by every push to it.
pub struct ObjectCache {
    path: PathBuf,
    label: String,
    lock: Mutex<()>,
    leases: AtomicUsize,
}

/// A quarantine's claim on a cache: while one is held the cache is not made again.
pub struct Lease(Arc<ObjectCache>);

impl Drop for Lease {
    fn drop(&mut self) {
        self.0.leases.fetch_sub(1, Ordering::AcqRel);
    }
}

impl ObjectCache {
    /// The cache of `repo` at `host` under `parent` (a folder of the app's own). Nothing is made
    /// on disk until the first [`Self::update`]. The folder is named by a hash, so no name a forge
    /// or a chat gives reaches a path.
    pub fn of(parent: &Path, host: &str, repo: &str) -> Arc<ObjectCache> {
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
                    lock: Mutex::new(()),
                    leases: AtomicUsize::new(0),
                })
            })
            .clone()
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

    fn filled(&self) -> bool {
        self.path.join(".git").join(FILLED).is_file()
    }

    /// Runs `fetch` (which fetches into the repository it is given) under the cache's lock, and
    /// tidies the cache after it. **Refused at once** where the cache is being filled for the
    /// first time by another push.
    pub fn update(&self, fetch: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
        let _held = match self.lock.try_lock() {
            Ok(held) => held,
            Err(TryLockError::Poisoned(held)) => held.into_inner(),
            Err(TryLockError::WouldBlock) if !self.filled() => {
                return Err(format!(
                    "purlis is still reading {} from the forge for the first time; try the push again in a few minutes",
                    self.label
                ));
            }
            Err(TryLockError::WouldBlock) => {
                self.lock.lock().unwrap_or_else(PoisonError::into_inner)
            }
        };
        self.make_ready()?;
        fetch(&self.path)?;
        std::fs::write(self.path.join(".git").join(FILLED), b"")
            .map_err(|why| format!("purlis could not write its object cache ({why})"))?;
        self.trim_bases();
        // Detached by git itself where it runs; a failure is not the push's.
        let _ = git::run(&self.path, &["gc", "--auto", "--quiet"], GC);
        Ok(())
    }

    /// The repository exists, and is made again if it has outgrown [`MOST`] while no quarantine
    /// is using it.
    fn make_ready(&self) -> Result<(), String> {
        let exists = self.path.join(".git").is_dir();
        // The quarantine being opened holds one lease already.
        if exists && size(&self.objects()) > MOST && self.leases.load(Ordering::Acquire) <= 1 {
            std::fs::remove_dir_all(&self.path).map_err(|why| why.to_string())?;
        } else if exists {
            return Ok(());
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

    /// Keeps the newest [`BASES_KEPT`] of the base refs, by commit date. A ref holds objects
    /// alive and tells the forge what the cache has, so the few newest are what is worth keeping.
    fn trim_bases(&self) {
        let Ok(listed) = git::run(
            &self.path,
            &[
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)",
                "refs/quarantine/base",
            ],
            git::READ,
        ) else {
            return;
        };
        for name in listed.out.lines().skip(BASES_KEPT) {
            let _ = git::run(&self.path, &["update-ref", "-d", name], git::READ);
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
