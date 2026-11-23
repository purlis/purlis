// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The quarantine a held CI-file push is read in** (ADR 0128 D8.3): an object store the app made
//! itself, that **no chat can write**, holding the base from the forge and the pack the chat pushed,
//! every object of which git has hashed itself.
//!
//! A chat's clone is the chat's: it can overwrite a loose object so that a diff read from the clone
//! shows other lines than the commit the forge will receive. So the diff is never read from there.
//!
//! 1. A fresh **repository** (never checked out) under a folder of the app's own
//!    ([`Quarantine::open`]). Not a bare one: git's `safe.bareRepository` setting may refuse to find it.
//! 2. **The base, fetched from the forge** with the person's credential ([`BaseObjects`]): what a
//!    thin pack is a delta against, and what the diff is made against. Into the forge repo's
//!    [`ObjectCache`] where the source has one, which the quarantine names as an **alternate**, so
//!    a push fetches only what changed and the quarantine's own store holds the pushed pack alone
//!    (ADR 0128, amended 2026-11-23; why that is safe is in [`super::objectcache`]).
//! 3. **The chat's pack** taken in by `git index-pack --stdin --fix-thin --strict` ([`Quarantine::take_in`]):
//!    git hashes every object and checks its form, so an object is its content and nothing else, and
//!    a pack that leaves the tip without its objects is refused.
//! 4. [`Quarantine::trees`]: the only [`super::Trees`] and [`super::TipFiles`] that production reads
//!    a push through.
//!
//! **Tier** (ADR 0069): Machine, device-bound, **rebuildable**: a folder under the app's own cache
//! that holds nothing but what the forge and the pushed pack hold, removed with the [`Quarantine`]
//! that made it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use std::sync::Arc;

use super::ci::GitTrees;
use super::objectcache::{self, FILL, Lease, ObjectCache};
use crate::diffscan::KNOWN_AT_MOST;
use crate::mergehold::HeadSha;
use crate::worktree::git;

/// The tips fetched in one `git fetch`: an argument list that stays well inside a platform's limit.
const TIPS_PER_FETCH: usize = 256;

/// **What reading the forge's advertised tips may cost** ([`BaseObjects::fetch_all`], ADR 0128,
/// amended 2026-11-23): a chat can make a pack that needs them, so they are bounded and a run that
/// goes past either bound is refused, never cut short and trusted.
#[derive(Debug, Clone, Copy)]
pub struct FetchLimits {
    /// The most time all the fetches of one run may take together. Checked before each `git fetch`,
    /// so a run can end one fetch (at most [`git::NETWORK`]) past it.
    pub deadline: Duration,
    /// The most the quarantine's object store may grow by during one run, checked after each chunk.
    pub growth: u64,
    /// The tips named in one `git fetch`.
    pub per_fetch: usize,
}

impl FetchLimits {
    /// Two fetches' worth of [`git::NETWORK`], and half a gibibyte of objects the base did not
    /// have. With the tips fetched incrementally (each chunk is negotiated against the ones before
    /// it) a repository of a thousand branches takes a few chunks and well inside both.
    pub const DEFAULT: FetchLimits = FetchLimits {
        deadline: Duration::from_secs(240),
        growth: 512 * 1024 * 1024,
        per_fetch: TIPS_PER_FETCH,
    };
}

/// How long git takes a pack in and checks it.
const TAKING_IN: Duration = Duration::from_secs(300);

/// **Where the base's objects come from**: the forge. A test stands another repository in.
pub trait BaseObjects {
    /// Fetches commit `base`, with its history, into the repository at `into`.
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String>;

    /// **The branch tips the forge advertises now**, as full object ids, at most
    /// [`KNOWN_AT_MOST`] (ADR 0128, amended 2026-11-23): what a thin pack's base may be. `None`
    /// where the forge did not answer, and the default says nothing: a source that does not
    /// list leaves a thin pack on an integration branch refused, as before.
    fn advertised(&self, _into: &Path) -> Option<Vec<String>> {
        None
    }

    /// Fetches every commit of `tips`, with its history, into the repository at `into`, within
    /// [`FetchLimits::DEFAULT`]: a source refuses a run that would pass them.
    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        tips.iter().try_for_each(|tip| self.fetch(into, tip))
    }

    /// The object cache this source fetches into, which a quarantine reads through as an
    /// alternate. `None` where it fetches into the quarantine itself.
    fn cache(&self) -> Option<Arc<ObjectCache>> {
        None
    }
}

/// **The forge's own objects**, fetched over HTTPS with the person's credential and no other
/// ([`git::run_network`]'s one-credential rule): `url` is the repo's address without user
/// information, and `helper` the credential helper purlis's own binary provides
/// ([`crate::forge::gitcredential::helper_for_account`]). **The token is on no disk and in no
/// argument or variable**: the helper reads it from the keyring and answers git's pipe, and
/// answers only for the one account's host.
pub struct FromForge {
    pub url: String,
    pub helper: Option<String>,
    /// The forge repo's object cache: what a push fetches into, so that it is sent only what is
    /// new. `None` fetches the whole base into the quarantine, on the per-call limit alone.
    pub cache: Option<Arc<ObjectCache>>,
}

impl FromForge {
    /// One `git fetch` of `tips`, each into a private ref of the quarantine. A commit fetched by
    /// its id alone lands only in `FETCH_HEAD`, which git does not negotiate from; a ref does, so
    /// every later fetch tells the forge what the quarantine holds already (`have` lines) and is
    /// sent only what is new. The name is the id, so a tip fetched twice is one ref.
    fn fetch_into_refs(
        &self,
        into: &Path,
        tips: &[HeadSha],
        ref_dir: &str,
        otherwise: &str,
    ) -> Result<(), String> {
        let specs: Vec<String> = tips
            .iter()
            .map(|tip| format!("+{0}:refs/quarantine/{ref_dir}/{0}", tip.as_str()))
            .collect();
        let mut args = vec!["fetch", "--quiet", "--no-tags", "--", self.url.as_str()];
        args.extend(specs.iter().map(String::as_str));
        // A fetch into the cache may be the first, of a whole history: it has a bound of its own.
        let bound = if self.cache.is_some() {
            FILL
        } else {
            git::NETWORK
        };
        let run = git::run_network_within(into, self.helper.as_deref(), &args, bound)
            .map_err(|why| why.to_string())?;
        if run.ok() {
            Ok(())
        } else {
            Err(first_line(&run.err, otherwise))
        }
    }

    /// [`BaseObjects::fetch_all`] within `limits`.
    ///
    /// A tip the forge no longer has (a branch deleted since it was listed) fails its whole `git
    /// fetch`, so that chunk is taken again one tip at a time and the tip is skipped: a pack that
    /// needed it is refused by git afterwards, so nothing is let through by skipping. A run that
    /// fetched no tip at all is an error.
    pub fn fetch_all_within(
        &self,
        into: &Path,
        tips: &[HeadSha],
        limits: &FetchLimits,
    ) -> Result<(), String> {
        if self.url.contains('@') {
            return Err("a forge's address carries no user information".to_owned());
        }
        let started = Instant::now();
        let before = store_size(into);
        let mut fetched_any = false;
        let mut first_failure = None;
        for chunk in tips.chunks(limits.per_fetch.max(1)) {
            check_deadline(started, limits)?;
            match self.fetch_into_refs(
                into,
                chunk,
                "tips",
                "git could not fetch the advertised tips from the forge",
            ) {
                Ok(()) => fetched_any = true,
                Err(why) => {
                    first_failure.get_or_insert(why);
                    for tip in chunk {
                        check_deadline(started, limits)?;
                        if self
                            .fetch_into_refs(
                                into,
                                std::slice::from_ref(tip),
                                "tips",
                                "git could not fetch the advertised tip from the forge",
                            )
                            .is_ok()
                        {
                            fetched_any = true;
                        }
                    }
                }
            }
            if store_size(into).saturating_sub(before) > limits.growth {
                return Err(format!(
                    "the forge's branches hold more than {} MiB beyond the base, which is more than purlis reads for one push",
                    limits.growth / (1024 * 1024)
                ));
            }
        }
        match first_failure {
            Some(why) if !fetched_any => Err(why),
            _ => Ok(()),
        }
    }
}

fn check_deadline(started: Instant, limits: &FetchLimits) -> Result<(), String> {
    if started.elapsed() >= limits.deadline {
        Err(format!(
            "reading the forge's branches took longer than {} s, which is more than purlis allows for one push",
            limits.deadline.as_secs()
        ))
    } else {
        Ok(())
    }
}

/// The bytes under a repository's object store (links are not followed).
fn store_size(repository: &Path) -> u64 {
    objectcache::size(&repository.join(".git").join("objects"))
}

impl BaseObjects for FromForge {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        if self.url.contains('@') {
            return Err("a forge's address carries no user information".to_owned());
        }
        let fetch = |dir: &Path| {
            self.fetch_into_refs(
                dir,
                std::slice::from_ref(base),
                "base",
                "git could not fetch the base from the forge",
            )
        };
        match &self.cache {
            Some(cache) => cache.update(fetch),
            None => fetch(into),
        }
    }

    fn advertised(&self, into: &Path) -> Option<Vec<String>> {
        if self.url.contains('@') {
            return None;
        }
        let run = git::run_network(
            into,
            self.helper.as_deref(),
            &["ls-remote", "--heads", "--", &self.url],
        )
        .ok()?;
        crate::diffscan::tips(run)
    }

    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        let Some(cache) = &self.cache else {
            return self.fetch_all_within(into, tips, &FetchLimits::DEFAULT);
        };
        cache.update(|dir| self.fetch_all_within(dir, tips, &FetchLimits::DEFAULT))?;
        let ids: Vec<String> = tips.iter().map(|tip| tip.as_str().to_owned()).collect();
        cache.keep_tips(&ids);
        Ok(())
    }

    fn cache(&self) -> Option<Arc<ObjectCache>> {
        self.cache.clone()
    }
}

/// Why a pack was not taken in: for want of objects the forge may hold, or otherwise.
enum Refused {
    ForWantOfObjects(String),
    Otherwise(String),
}

/// **A repository of the app's own, with nothing checked out**, removed when this is dropped.
pub struct Quarantine {
    path: PathBuf,
    /// The cache this reads through, kept from being made again while this is held.
    _lease: Option<Lease>,
}

impl Quarantine {
    /// A fresh quarantine under `parent` (a folder of the app's own, which no chat can write), holding
    /// `base` and its history as `from` gives them.
    pub fn open(
        parent: &Path,
        base: &HeadSha,
        from: &dyn BaseObjects,
    ) -> Result<Quarantine, String> {
        std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
        let path = parent.join(format!("quarantine-{}", uuid::Uuid::new_v4()));
        let made = git::run(
            parent,
            &["init", "--quiet", &path.to_string_lossy()],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        if !made.ok() {
            return Err(first_line(&made.err, "git could not make the quarantine"));
        }
        let cache = from.cache();
        let quarantine = Quarantine {
            path,
            _lease: cache.as_ref().map(ObjectCache::lease),
        };
        if let Some(cache) = &cache {
            quarantine.read_through(cache)?;
        }
        from.fetch(&quarantine.path, base)?;
        quarantine.has(base)?;
        Ok(quarantine)
    }

    /// Names the cache's object store as this quarantine's alternate.
    fn read_through(&self, cache: &ObjectCache) -> Result<(), String> {
        let info = self.path.join(".git").join("objects").join("info");
        std::fs::create_dir_all(&info).map_err(|why| why.to_string())?;
        std::fs::write(
            info.join("alternates"),
            format!("{}\n", cache.objects().display()),
        )
        .map_err(|why| why.to_string())
    }

    /// **Also holds commit `extra`**, with its history, as `from` gives it: the default branch's
    /// tip, which a push to an existing branch is read against ([`crate::heldact::check_push_since`])
    /// and whose objects its pack leaves out because the forge has them.
    pub fn also_have(&self, from: &dyn BaseObjects, extra: &HeadSha) -> Result<(), String> {
        from.fetch(&self.path, extra)?;
        self.has(extra)
    }

    /// **Takes in the pack the chat pushed**, whose tip is `tip`: every object hashed and checked by
    /// git, completed against the base where the pack is thin, and refused where the tip is not a
    /// commit whose every object is here.
    pub fn take_in(&self, pack: Vec<u8>, tip: &HeadSha, base: &HeadSha) -> Result<(), String> {
        self.read_in(pack, tip, base)
            .map_err(|refused| match refused {
                Refused::ForWantOfObjects(why) | Refused::Otherwise(why) => why,
            })
    }

    fn read_in(&self, pack: Vec<u8>, tip: &HeadSha, base: &HeadSha) -> Result<(), Refused> {
        let done = git::run_with_input(
            &self.path,
            &["index-pack", "--stdin", "--fix-thin", "--strict"],
            pack,
            TAKING_IN,
        )
        .map_err(|why| Refused::Otherwise(why.to_string()))?;
        if done.code != Some(0) {
            let said = String::from_utf8_lossy(&done.err);
            let why = first_line(&said, "git did not take the pushed objects in");
            // git's own failures for a pack built on objects that are not here (`LC_ALL=C`): a
            // thin delta whose base is missing, or, under `--strict`, a commit or tree whose
            // parent or entry is missing (a small change carries no delta at all). A truncated
            // pack ("early EOF") or garbage ("pack signature mismatch") is another failure.
            return Err(
                if said.contains("unresolved delta")
                    || said.contains("did not receive expected object")
                {
                    Refused::ForWantOfObjects(why)
                } else {
                    Refused::Otherwise(why)
                },
            );
        }
        self.has(tip).map_err(Refused::Otherwise)?;
        // Every object of the push is here: a tip whose tree or blobs are missing has no diff.
        // `--quiet` lists nothing, so a pack of millions of objects needs no cap: only the exit
        // code is read.
        let walked = self.walk(tip, base, "error")?;
        if walked.ok() {
            return Ok(());
        }
        let why = first_line(&walked.err, "the pushed objects are not whole");
        // The same walk that lets a missing object pass fails no more: what failed was a missing
        // object (a pack built on a commit the forge holds leaves out what that commit has), not
        // an object git could not read.
        Err(if self.walk(tip, base, "allow-any")?.ok() {
            Refused::ForWantOfObjects(why)
        } else {
            Refused::Otherwise(why)
        })
    }

    fn walk(&self, tip: &HeadSha, base: &HeadSha, missing: &str) -> Result<git::Run, Refused> {
        git::run(
            &self.path,
            &[
                "rev-list",
                "--objects",
                "--quiet",
                &format!("--missing={missing}"),
                tip.as_str(),
                "--not",
                base.as_str(),
            ],
            TAKING_IN,
        )
        .map_err(|why| Refused::Otherwise(why.to_string()))
    }

    /// **[`Self::take_in`], where a thin pack may be built on any branch the forge advertises**
    /// (ADR 0128, amended 2026-11-23): a task branch cut from an integration branch is a pack
    /// whose deltas are against that branch's objects, which the quarantine does not hold. **Only
    /// when git says the pack is short of objects** (a delta whose base is not here, or a tip whose
    /// walk meets a missing object) are the forge's advertised branch tips, at most
    /// [`KNOWN_AT_MOST`] and nothing a chat names, fetched with their history from `from`, and the
    /// pack tried once more. A pack git cannot read at all (truncated, garbage, a bad object) is
    /// refused at once, with no fetch. **The pack is verified either way**: git hashes and checks
    /// every object, and `tip` must be a commit whose objects are all here. What the objects
    /// are read against is the caller's: a pack built on an integration branch is judged against
    /// the default branch's tip like any other.
    pub fn take_in_known(
        &self,
        pack: &[u8],
        tip: &HeadSha,
        base: &HeadSha,
        from: &dyn BaseObjects,
    ) -> Result<(), String> {
        let first = match self.read_in(pack.to_vec(), tip, base) {
            Ok(()) => return Ok(()),
            Err(Refused::Otherwise(why)) => return Err(why),
            Err(Refused::ForWantOfObjects(why)) => why,
        };
        let Some(advertised) = from.advertised(&self.path) else {
            return Err(first);
        };
        let mut seen = std::collections::HashSet::new();
        let tips: Vec<HeadSha> = advertised
            .iter()
            .filter(|id| seen.insert(id.to_ascii_lowercase()))
            .take(KNOWN_AT_MOST)
            .filter_map(|id| HeadSha::try_from(id.as_str()).ok())
            .collect();
        if tips.is_empty() {
            return Err(first);
        }
        from.fetch_all(&self.path, &tips)?;
        self.take_in(pack.to_vec(), tip, base)
    }

    fn has(&self, commit: &HeadSha) -> Result<(), String> {
        let found = git::run(
            &self.path,
            &["cat-file", "-e", &format!("{}^{{commit}}", commit.as_str())],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        if found.ok() {
            Ok(())
        } else {
            Err(format!(
                "the quarantine has no commit {}",
                &commit.as_str()[..12]
            ))
        }
    }

    /// **The trees of the quarantine**, and no other repository's.
    pub fn trees(&self) -> GitTrees {
        GitTrees::of_trusted_store(&self.path)
    }
}

/// **Removes every quarantine under `parent`**, left by an app that ended without dropping them: a
/// real folder named `quarantine-*`, never a link. The app calls it at start (the one-per-user lock
/// means no other app owns a live one) and at exit.
pub fn sweep(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let named = entry
            .file_name()
            .to_string_lossy()
            .starts_with("quarantine-");
        if named && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl Drop for Quarantine {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn first_line(said: &str, otherwise: &str) -> String {
    said.trim()
        .lines()
        .next()
        .filter(|line| !line.is_empty())
        .unwrap_or(otherwise)
        .to_owned()
}
