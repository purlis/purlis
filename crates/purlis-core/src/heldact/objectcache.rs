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
//! 1. The base (and the advertised tips, where a thin pack needs them or a push sends no new
//!    objects) are fetched into **this cache**, incrementally: its refs tell the forge what it
//!    holds. **Only this repo's own advertised history is ever fetched**: never a commit a chat
//!    names, which a forge resolves by id even where it is a fork's or a dangling one, so a push's
//!    tip is never fetched, kept as a base, or let by for being in the cache (a tip not in the
//!    pack is accepted only where it is reachable from an advertised tip).
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
//! thread of the app's own, owned by the cache and ended only by [`STALL`] (a fetch that has written
//! nothing for that long) or the generous upper bound [`FILL`], and is refused at once with what
//! is happening. A push to a filled cache tops it up within [`PUSH`], for all the
//! fetches of that push together; a top-up that does not finish in time is refused with that
//! reason and finished on the same thread, so the next push finds it done.
//!
//! **A first fill that finishes** (#7587). A killed fetch leaves only a `tmp_pack_*`, which git
//! cannot resume, so the fill is **staged** ([`fill_in_stages`]): the base is fetched at a depth of
//! 200 commits, then 5000, then in full, and each stage that completes leaves real objects and a
//! ref that the next stage (and a fill begun again after a restart) negotiates from, so an
//! interrupted fill loses the stage it was in and not the whole. The stages are of **one target,
//! the forge's default branch** ([`FIRST_FILL_TARGET`]), for the early start and for a push alike,
//! so a fill begun by either is resumed by the other; a push's base is fetched on top of it. The
//! progress is a file in the cache's `.git`, and a cache holds a shallow history only until the
//! last stage has made it whole: **it is marked filled, and a push reads it, only then**, so
//! [`ObjectCache::holds`] is `false` while `.git/shallow` exists (a ref git wrote after a shallow
//! fetch says nothing of the history behind it). (`--filter=blob:none` is no option: the gate
//! reads the base's CI files and the blobs of the pushed range.)
//!
//! **Progress.** A fill's `git fetch --progress` is read as it runs ([`Progress`],
//! [`ObjectCache::sink`]); the refusal a push gets names the objects and bytes received so far and
//! an estimate of what is left, and the figure is logged at info level every [`LOG_EVERY`]. A
//! staged fill's figure is its stage's alone (dropped as each begins, [`ObjectCache::stage_began`]),
//! the refusal says which part of three it is in, and only the last part ([`Stage::Rest`]) gives
//! an estimate: the earlier parts are too short for a rate to say anything.
//!
//! **A fill does not block a push it need not block** ([`ObjectCache::update_having`]): where the
//! objects a push is read against ([`ObjectCache::holds`]) are in the cache already and
//! reachable from a ref a finished fetch wrote, the push goes on while the fill continues. A push
//! whose base is not there, or while the cache is still shallow, is refused, with the progress.
//! **Fail closed:** a push is let on only for commits git finds reachable from those refs, and
//! every other answer is a refusal.
//!
//! **Early start.** [`ObjectCache::start_fill`] starts a fill before any push, when a chat opens
//! on the repo, within the same bounds ([`FILL`], [`MOST`]).
//!
//! **Concurrency.** One lock per cache in the app (the app is one per user); git's own locks
//! guard the rest. A push that arrives while a fill runs is refused at once, never left to time
//! out; a push that arrives during a top-up waits for it, within its own bound.
//!
//! **Size and repair.** `git gc --auto` after an update; a cache past its cap ([`MOST`]) is made
//! again, but only when no quarantine is using it, since a held push's quarantine reads through
//! it until the ask ends. A repo whose history alone is past the cap is made again once, and
//! refused with that reason and left alone after that, not deleted and fetched again on every push.
//! The size a fill finished at is written for a first fill or a rebuild only, never after a top-up. A cache that git does not
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

/// **The most a fill's fetch may take**, however much it is moving: the upper bound under
/// [`STALL`], for the first fill of a large repo's history. It runs on a thread of the app's own,
/// never on a chat's connection. Apart from [`git::NETWORK`], which holds every other call.
pub const FILL: Duration = Duration::from_secs(2 * 60 * 60);

/// **How long a fetch into the cache may write nothing** (its progress, which git writes as it
/// receives and as it resolves deltas) before it is ended as stuck. A fetch that is moving is
/// never cut for being slow.
pub const STALL: Duration = Duration::from_secs(3 * 60);

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

/// **What a first fill stages**: the forge's default branch, whoever begins the fill (a chat
/// opening on the repo, or a push), so that a fill one began and a stall cut is resumed by the
/// other at the stage it reached. A push's base is fetched after it, as any top-up is.
pub const FIRST_FILL_TARGET: &str = "default-branch";

/// The file of the stage a first fill has reached: the target it fetches, a space, and how many of
/// its stages have finished.
const STAGE: &str = "purlis-fill-stage";

/// The file of the bases used last, newest first, one id to a line.
const BASES: &str = "purlis-bases";

/// How long `git gc --auto` may take.
const GC: Duration = Duration::from_secs(300);

/// How often a push waiting for a top-up looks again.
const LOOK_AGAIN: Duration = Duration::from_millis(50);

/// How often a running fill's progress is logged.
pub const LOG_EVERY: Duration = Duration::from_secs(30);

/// **How far a fill has got**, from `git fetch --progress`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    /// Objects received so far, and in all.
    pub received: u64,
    pub total: u64,
    /// Bytes received so far.
    pub bytes: u64,
    /// Once every object is received git resolves deltas: how many of how many.
    pub resolving: Option<(u64, u64)>,
}

/// Reads `git fetch --progress`'s stderr as it comes in chunks: its lines end in `\r` (a status
/// line rewritten in place) or `\n`, and a chunk may end in the middle of one.
#[derive(Default)]
pub struct ProgressReader {
    carry: String,
    last: Progress,
}

impl ProgressReader {
    /// Takes in `chunk`; the progress after its last whole line, where it moved.
    pub fn feed(&mut self, chunk: &str) -> Option<Progress> {
        self.carry.push_str(chunk);
        let mut moved = false;
        while let Some(end) = self.carry.find(['\r', '\n']) {
            let line: String = self.carry.drain(..=end).collect();
            if let Some(seen) = parse_progress_line(&line) {
                self.last = match seen {
                    Seen::Receiving {
                        received,
                        total,
                        bytes,
                    } => Progress {
                        received,
                        total,
                        bytes,
                        resolving: None,
                    },
                    Seen::Resolving(done, of) => Progress {
                        resolving: Some((done, of)),
                        ..self.last
                    },
                };
                moved = true;
            }
        }
        // A line git never ends (a stream that is not git's progress) is not kept without bound.
        if self.carry.len() > MOST_CARRIED {
            self.carry.clear();
        }
        moved.then_some(self.last)
    }
}

/// The most of an unfinished line a [`ProgressReader`] keeps (64 KiB): git's own lines are far
/// shorter.
const MOST_CARRIED: usize = 64 * 1024;

enum Seen {
    Receiving {
        received: u64,
        total: u64,
        bytes: u64,
    },
    Resolving(u64, u64),
}

/// One line of git's progress: `Receiving objects:  45% (4500/10000), 12.00 MiB | 3.00 MiB/s` (the
/// byte figure is absent on the first lines, and the last ends `, done.`) or
/// `Resolving deltas:  10% (10/100)`. Anything else is `None`.
fn parse_progress_line(line: &str) -> Option<Seen> {
    // Git's own lines start with the keyword (after the `\r` of the line before it); a line that
    // merely contains it, such as `remote: Receiving objects: ...`, is the forge's to say.
    let line = line.trim_start_matches('\r');
    // A pack of fewer than `transfer.unpackLimit` objects is unpacked, not indexed.
    let counted = line
        .strip_prefix("Receiving objects:")
        .or_else(|| line.strip_prefix("Unpacking objects:"));
    let (receiving, rest) = if let Some(rest) = counted {
        (true, rest)
    } else {
        (false, line.strip_prefix("Resolving deltas:")?)
    };
    let open = rest.find('(')?;
    let close = open + rest[open..].find(')')?;
    let (done, of) = rest[open + 1..close].split_once('/')?;
    let (done, of): (u64, u64) = (done.trim().parse().ok()?, of.trim().parse().ok()?);
    if !receiving {
        return Some(Seen::Resolving(done, of));
    }
    let after = rest[close + 1..].trim_start_matches([',', ' ']);
    let size = after.split(['|', ',']).next().unwrap_or_default();
    Some(Seen::Receiving {
        received: done,
        total: of,
        bytes: parse_size(size).unwrap_or(0),
    })
}

/// `12.00 MiB`, `340 bytes`, `1.50 GiB` as bytes.
fn parse_size(text: &str) -> Option<u64> {
    let mut parts = text.split_whitespace();
    let number: f64 = parts.next()?.parse().ok()?;
    let unit: f64 = match parts.next()? {
        "bytes" | "byte" | "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * unit) as u64)
}

/// A time as a person says it: "a minute", "12 minutes", "2 hours".
fn about(left: Duration) -> String {
    let minutes = left.as_secs().div_ceil(60).max(1);
    match minutes {
        1 => "a minute".to_owned(),
        2..=89 => format!("{minutes} minutes"),
        _ => format!("{} hours", minutes.div_ceil(60)),
    }
}

/// How long a fill that has read `seen.received` of `seen.total` objects in `elapsed` has still to
/// run, at the rate so far. Never more than what is left of [`FILL`], the most a fill runs: a
/// figure that has not moved, or a total git (or a forge) made up, cannot promise more than that,
/// and cannot overflow a [`Duration`].
fn estimate(seen: &Progress, elapsed: Duration) -> Duration {
    let cap = FILL.saturating_sub(elapsed);
    let rest = elapsed.as_secs_f64() * (seen.total - seen.received.min(seen.total)) as f64
        / seen.received.max(1) as f64;
    Duration::try_from_secs_f64(rest).map_or(cap, |rest| rest.min(cap))
}

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
    /// The cache was made again once because its history alone is past the cap: a second time it
    /// is refused, not made again.
    rebuilt_for_history: AtomicBool,
    failure: Mutex<Option<String>>,
    /// How far the running fill has got, and when it began.
    progress: Mutex<Option<(Progress, Instant)>>,
    /// The stage of a first fill that is running: which part of how many, and what it asks for.
    part: Mutex<Option<(usize, usize, Stage)>>,
    /// The fill's own `git fetch` is running, under the cache's lock: only then is what the sink
    /// hears the fill's progress. A top-up run inline for a push shares the sink, and is not it.
    fetching: AtomicBool,
}

/// Resets [`ObjectCache::filling`] when the cache's thread ends, however it ends: a panic or an
/// early return must not leave every later push refused as "still reading".
struct Filling(Arc<ObjectCache>);

impl Drop for Filling {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.tried.store(true, Ordering::Release);
            *self
                .0
                .failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner) =
                Some("the read stopped unexpectedly".to_owned());
        }
        // What a fill read is no word on the next one, however this one ended.
        self.0.fetching.store(false, Ordering::Release);
        self.0.clear_progress();
        self.0.filling.store(false, Ordering::Release);
    }
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

    /// [`ObjectCache::update_having`] within what is left of this push's time.
    pub fn update_having(&self, fetch: Fetch, have: &[String]) -> Result<(), String> {
        self.cache.update_having(self.deadline, fetch, have)
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
                    rebuilt_for_history: AtomicBool::new(false),
                    failure: Mutex::new(None),
                    progress: Mutex::new(None),
                    part: Mutex::new(None),
                    fetching: AtomicBool::new(false),
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

    /// Whether the cache's history is cut short (a staged first fill's early stages leave it so).
    fn is_shallow(&self) -> bool {
        self.path.join(".git").join("shallow").exists()
    }

    /// Whether a fill is running now.
    pub fn is_filling(&self) -> bool {
        self.filling.load(Ordering::Acquire)
    }

    /// What a push is told while the cache is being filled: how far it has got where git has said.
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
        let progress = *self.progress.lock().unwrap_or_else(PoisonError::into_inner);
        // Only a first fill has parts: a top-up that runs the last stage's fetch is not "part 3".
        let part = (*self.part.lock().unwrap_or_else(PoisonError::into_inner))
            .filter(|_| !self.is_filled());
        let of_three = part
            .map(|(at, of, _)| format!("part {at} of {of}"))
            .unwrap_or_default();
        let Some((seen, began)) = progress.filter(|(seen, _)| seen.total > 0) else {
            let at = if of_three.is_empty() {
                String::new()
            } else {
                format!(" ({of_three})")
            };
            return format!(
                "purlis is still reading {} from the forge {when}{last}{at}; an interrupted read resumes where it stopped, so try the push again in a few minutes",
                self.label
            );
        };
        let megabytes = seen.bytes as f64 / 1_000_000.0;
        let wait = match seen.resolving {
            Some((done, of)) => format!(
                "all {} objects are in and git is resolving deltas ({done} of {of}); try again in a minute or two",
                seen.total
            ),
            None if seen.received == 0 => {
                format!("{} objects to read; try again in a few minutes", seen.total)
            }
            // Only the last part is long enough for a rate to promise anything.
            None if part.is_some_and(|(_, _, stage)| stage != Stage::Rest) => format!(
                "{} of {} objects ({megabytes:.0} MB); try again in a few minutes",
                seen.received, seen.total
            ),
            None => format!(
                "{} of {} objects ({megabytes:.0} MB); try again in about {}",
                seen.received,
                seen.total,
                about(estimate(&seen, began.elapsed()))
            ),
        };
        let wait = if of_three.is_empty() {
            wait
        } else {
            format!("{of_three}, {wait}")
        };
        format!(
            "purlis is reading {} {when}{last}: {wait} (an interrupted read resumes where it stopped)",
            self.label
        )
    }

    /// What reads a fetch's `--progress` lines into this cache's state and, every [`LOG_EVERY`],
    /// into the log. Given to the fill's `git fetch` ([`git::run_network_stalling`]).
    pub fn sink(self: &Arc<Self>) -> git::Watch {
        let cache = self.clone();
        let reader = Mutex::new(ProgressReader::default());
        let logged = Mutex::new(Instant::now());
        Arc::new(move |chunk: &str| {
            let moved = reader
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .feed(chunk);
            let Some(seen) = moved else { return };
            if !cache.fetching.load(Ordering::Acquire) {
                return;
            }
            let mut state = cache
                .progress
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let began = state.map_or_else(Instant::now, |(_, began)| began);
            *state = Some((seen, began));
            drop(state);
            let mut logged = logged.lock().unwrap_or_else(PoisonError::into_inner);
            if logged.elapsed() >= LOG_EVERY {
                *logged = Instant::now();
                tracing::info!(
                    repo = %cache.label,
                    received = seen.received,
                    total = seen.total,
                    bytes = seen.bytes,
                    "reading a forge repo into the object cache"
                );
            }
        })
    }

    fn clear_progress(&self) {
        *self.progress.lock().unwrap_or_else(PoisonError::into_inner) = None;
        *self.part.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// **A stage of a first fill is beginning**: the last stage's figure and start are not this
    /// one's (its estimate and its "resolving deltas" are no word on it), so they are dropped, and
    /// the refusal says which part of [`FIRST_FILL`] is running.
    pub fn stage_began(&self, stage: Stage) {
        *self.progress.lock().unwrap_or_else(PoisonError::into_inner) = None;
        *self.part.lock().unwrap_or_else(PoisonError::into_inner) = FIRST_FILL
            .iter()
            .position(|each| *each == stage)
            .map(|at| (at + 1, FIRST_FILL.len(), stage));
    }

    /// The figure the running fill is at, where git has said one.
    pub fn progress(&self) -> Option<Progress> {
        self.progress
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .map(|(seen, _)| seen)
    }

    /// **Whether every commit of `ids` is in the cache and reachable from a ref a finished fetch
    /// wrote** (`refs/quarantine/*`): git writes a ref only after the fetch's connectivity check,
    /// so a commit under one has its whole history here, whatever a fill still running adds.
    ///
    /// **Never while the cache is shallow** (`.git/shallow` exists): a staged first fill's early
    /// stages leave refs whose history stops at a boundary, so that premise is false until the
    /// last stage has unshallowed it. The file is the fact itself, where [`Self::is_filled`] is
    /// only the fill's word that it ended. Not an id, nothing to ask, or a git that could not be
    /// asked is `false`: it fails closed.
    pub fn holds(&self, ids: &[String]) -> bool {
        if ids.is_empty() || !self.path.join(".git").is_dir() || self.is_shallow() {
            return false;
        }
        ids.iter().all(|id| {
            if id.len() < 40 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                return false;
            }
            let here = git::run(
                &self.path,
                &["cat-file", "-e", &format!("{id}^{{commit}}")],
                git::READ,
            )
            .is_ok_and(|run| run.ok());
            here && git::run(
                &self.path,
                &["rev-list", "-n1", id, "--not", "--glob=refs/quarantine/*"],
                git::READ,
            )
            .is_ok_and(|run| run.ok() && run.out.trim().is_empty())
        })
    }

    /// Starts a fill now, on the cache's own thread, unless the cache is filled or being filled:
    /// for a chat that opens on the repo, before any push. Bounded as every fill is ([`FILL`], and
    /// [`MOST`] by the cache's care). A thread that cannot start leaves the first push to start it.
    pub fn start_fill(self: &Arc<Self>, fetch: Fetch) {
        if self.is_filling() || self.is_filled() {
            return;
        }
        let _ = self.fill_in_background(fetch);
    }

    /// A refusal for a push whose `have` the cache does not hold: `Ok` where it does, because a
    /// fill running in the background is no reason to refuse a push that needs none of it.
    fn refuse_unless_held(&self, have: &[String]) -> Result<(), String> {
        if self.holds(have) {
            Ok(())
        } else {
            Err(self.reading())
        }
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
        self.update_having(deadline, fetch, &[])
    }

    /// [`Self::update`] for a push that is read against the commits `have` (full ids): **where the
    /// cache holds them all** ([`Self::holds`]) a fill that is running, or one that is only just
    /// started, or a top-up that outran the push's time, is no refusal, and the push goes on
    /// while the cache goes on filling. With none, or where one is not held, it is [`Self::update`].
    pub fn update_having(
        self: &Arc<Self>,
        deadline: Instant,
        fetch: Fetch,
        have: &[String],
    ) -> Result<(), String> {
        if self.is_filling() {
            return self.refuse_unless_held(have);
        }
        let held = self.lock_until(deadline)?;
        // The lock may have been waited for while a fill was started.
        if self.is_filling() {
            return self.refuse_unless_held(have);
        }
        // The quarantine being opened holds one lease already.
        self.make_ready(1)?;
        if !self.is_filled() {
            self.fill_in_background(fetch)?;
            return self.refuse_unless_held(have);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            let refusal = self.timed_out(fetch);
            return self.holds(have).then_some(()).ok_or(refusal);
        }
        if let Err(why) = fetch(&self.path, left) {
            if Instant::now() >= deadline {
                let refusal = self.timed_out(fetch);
                return self.holds(have).then_some(()).ok_or(refusal);
            }
            return Err(self.scrub(why));
        }
        drop(held);
        self.tidy_later();
        Ok(())
    }

    /// What a push may be told of a failed fetch: git's first line names the cache's folder, a path
    /// of the app's own that no chat is told.
    fn scrub(&self, why: String) -> String {
        let named = |path: &Path| why.contains(path.to_string_lossy().as_ref());
        if named(&self.path) || self.path.canonicalize().is_ok_and(|real| named(&real)) {
            format!("git could not update purlis's copy of {}", self.label)
        } else {
            why
        }
    }

    /// `git gc --auto` on a thread of the cache's own, after the push has its verdict: it takes the
    /// lock, so the next push waits for it only within its own bound.
    fn tidy_later(self: &Arc<Self>) {
        let cache = self.clone();
        let _ = std::thread::Builder::new()
            .name("object-cache-tidy".to_owned())
            .spawn(move || {
                let _held = cache.lock.lock().unwrap_or_else(PoisonError::into_inner);
                cache.tidy();
            });
    }

    /// A top-up that outran its push's time: finished on the cache's own thread, and the push told.
    fn timed_out(self: &Arc<Self>, fetch: Fetch) -> String {
        self.tried.store(true, Ordering::Release);
        if let Err(why) = self.fill_in_background(fetch) {
            return why;
        }
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
    /// fetches within [`FILL`] and, for a first fill or a rebuild, says it is filled (a top-up
    /// never rewrites that: the size it holds is what the cap is judged against). A failure leaves
    /// the cache **not filled**. A thread that cannot start is the push's refusal, with its reason.
    fn fill_in_background(self: &Arc<Self>, fetch: Fetch) -> Result<(), String> {
        if self.filling.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        // The last fill's figures and start are not this one's, and a push refused before it says
        // anything must not be told them.
        self.clear_progress();
        // Moved into the thread; dropped with the closure where the thread does not start.
        let ending = Filling(self.clone());
        let cache = self.clone();
        let started = std::thread::Builder::new()
            .name("object-cache-fill".to_owned())
            .spawn(move || {
                let _ending = ending;
                let done = {
                    let _held = cache.lock.lock().unwrap_or_else(PoisonError::into_inner);
                    // No lease of this thread's own: one held is another push's quarantine.
                    cache.make_ready(0).and_then(|()| {
                        let first = !cache.is_filled();
                        cache.fetching.store(true, Ordering::Release);
                        let fetched = fetch(&cache.path, FILL);
                        cache.fetching.store(false, Ordering::Release);
                        fetched.map_err(|why| cache.scrub(why))?;
                        if first {
                            cache.mark_filled()?;
                        }
                        cache.tidy();
                        Ok(())
                    })
                };
                match done {
                    Ok(()) => {
                        *cache.failure.lock().unwrap_or_else(PoisonError::into_inner) = None;
                    }
                    Err(why) => {
                        cache.tried.store(true, Ordering::Release);
                        // Only a first fill that failed leaves the cache not filled: a top-up that
                        // failed leaves what was filled filled (and a staged first fill's progress
                        // is kept, to resume from).
                        *cache.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(why);
                    }
                }
            });
        match started {
            Ok(_) => Ok(()),
            Err(why) => {
                self.tried.store(true, Ordering::Release);
                Err(format!(
                    "purlis could not start reading {} from the forge ({why}); try the push again in a few minutes",
                    self.label
                ))
            }
        }
    }

    /// Never while the cache is shallow, whichever fetch got it here (a push's tips are fetched
    /// with no depth, and git leaves a boundary such a fetch did not move).
    fn mark_filled(&self) -> Result<(), String> {
        if self.is_shallow() {
            return Err(format!(
                "purlis has read only the newest part of {}'s history so far and reads on; try the push again in a few minutes",
                self.label
            ));
        }
        let _ = std::fs::remove_file(self.path.join(".git").join(STAGE));
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
        if !dot.join("HEAD").is_file()
            || !dot.join("objects").is_dir()
            || !dot.join("refs").is_dir()
        {
            return false;
        }
        git::run(&self.path, &["rev-parse", "--git-dir"], git::READ)
            .is_ok_and(|run| run.ok() && run.out.trim() == ".git")
    }

    /// Removes what a killed fetch or git left: `tmp_pack_*` in the object store and stale `*.lock`
    /// files of the refs, the index and `shallow`. Called with the cache's lock held, when no git of
    /// ours is running in it, so a lock still there is a killed git's. (A `gc` that git detached may
    /// still be running, as for the `tmp_pack_*` of F6 in the ADR amendment.)
    fn remove_leftovers(&self) {
        if let Ok(entries) = std::fs::read_dir(self.objects().join("pack")) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with("tmp_pack_") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let dot = self.path.join(".git");
        for lock in ["packed-refs.lock", "index.lock", "shallow.lock"] {
            let _ = std::fs::remove_file(dot.join(lock));
        }
        remove_locks(&dot.join("refs"));
    }

    /// The repository exists and git takes it for one, and is made again where it is damaged or
    /// has outgrown its cap while no quarantine is using it. `own` is the leases the caller holds
    /// itself (a push's quarantine, one; the cache's own thread, none): any more are another push's.
    /// A repo whose history alone is past the cap is made again once, and refused after that.
    fn make_ready(&self, own: usize) -> Result<(), String> {
        let exists = self.path.exists();
        if exists {
            // Before the size is measured, so a killed fetch's leftovers do not count.
            self.remove_leftovers();
        }
        let alone = self.leases.load(Ordering::Acquire) <= own;
        let sound = exists && self.is_sound();
        let too_big = sound && size(&self.objects()) > self.most;
        if sound && too_big && alone {
            let history = self.filled_at().is_some_and(|filled| filled > self.most);
            if history && self.rebuilt_for_history.swap(true, Ordering::AcqRel) {
                return Err(format!(
                    "{} holds more history than the {} GiB purlis keeps for one repo, so it cannot read it for a push",
                    self.label,
                    self.most / (1024 * 1024 * 1024)
                ));
            }
        }
        if sound && !(too_big && alone) {
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
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/quarantine/base",
            ],
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

/// **One stage of a first fill**: how much of the target's history a fetch asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The newest `n` commits: a shallow fetch, whose objects stay when a later stage is killed.
    Depth(u32),
    /// All the rest: the history made whole (`--unshallow` where the cache is shallow).
    Rest,
}

impl Stage {
    /// The `git fetch` arguments of this stage in the repository at `into`.
    pub fn args(self, into: &Path) -> Vec<String> {
        match self {
            Stage::Depth(n) => vec![format!("--depth={n}")],
            // `--unshallow` on a complete repository is an error, so only where it is shallow.
            Stage::Rest if into.join(".git").join("shallow").is_file() => {
                vec!["--unshallow".to_owned()]
            }
            Stage::Rest => Vec::new(),
        }
    }
}

/// The stages of a first fill: the newest 200 commits (a minute on a large repo), the newest 5000,
/// then everything. A killed stage loses its own fetch alone.
pub const FIRST_FILL: [Stage; 3] = [Stage::Depth(200), Stage::Depth(5000), Stage::Rest];

/// **Runs a first fill's fetch in stages, from the one it had reached** (#7587), and plainly once
/// the cache is filled. `target` names what is fetched (the base's id): a fill begun again for
/// another target forgets the partial one's refs and shallow boundaries, which no push has read,
/// so that no ref of the cache is left with a history cut short (the fills of a forge repo all use
/// [`FIRST_FILL_TARGET`], so that a fill is resumed whoever begins it). `fetch` runs one stage; the first
/// that fails ends the fill with its reason, and the stages before it stay done.
///
/// The file that remembers the stage lives in the cache's `.git`, so it goes with the cache when
/// the cache is made again.
pub fn fill_in_stages(
    dir: &Path,
    target: &str,
    stages: &[Stage],
    mut fetch: impl FnMut(Stage) -> Result<(), String>,
) -> Result<(), String> {
    let dot = dir.join(".git");
    if dot.join(FILLED).is_file() {
        return fetch(Stage::Rest);
    }
    let record = dot.join(STAGE);
    let target = target.to_ascii_lowercase();
    let remembered = std::fs::read_to_string(&record).unwrap_or_default();
    let done = match remembered.trim().rsplit_once(' ') {
        Some((was, count)) if was == target => count.parse::<usize>().unwrap_or(0),
        Some(_) => {
            forget_partial(dir);
            0
        }
        None => 0,
    };
    for (index, stage) in stages.iter().enumerate().skip(done) {
        fetch(*stage)?;
        std::fs::write(&record, format!("{target} {}", index + 1))
            .map_err(|why| format!("purlis could not write its object cache ({why})"))?;
    }
    Ok(())
}

/// Drops what a first fill for another target left: its refs (none of which a push has read:
/// [`ObjectCache::holds`] is `false` while the cache is shallow, and it is marked filled only once
/// it is not) and the shallow boundaries that cut their histories short.
fn forget_partial(dir: &Path) {
    if let Ok(listed) = git::run(
        dir,
        &["for-each-ref", "--format=%(refname)", "refs/quarantine"],
        git::READ,
    ) {
        for name in listed.out.lines() {
            let _ = git::run(dir, &["update-ref", "-d", name], git::READ);
        }
    }
    let _ = std::fs::remove_file(dir.join(".git").join("shallow"));
}

/// Removes every `*.lock` file under `dir` (links are not followed).
fn remove_locks(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            remove_locks(&entry.path());
        } else if kind.is_file() && entry.file_name().to_string_lossy().ends_with(".lock") {
            let _ = std::fs::remove_file(entry.path());
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

#[cfg(test)]
mod tests;
