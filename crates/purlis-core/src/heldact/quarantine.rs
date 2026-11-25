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
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::ci::GitTrees;
use super::objectcache::{self, Fetch, Lease, ObjectCache, PushCache, Stage};
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
    /// and each fetch is given no more than what is left of it (and at least one second), so a run
    /// ends at it and not a whole fetch past it.
    pub deadline: Duration,
    /// The most the quarantine's object store may grow by during one run, checked after each chunk.
    pub growth: u64,
    /// The tips named in one `git fetch`.
    pub per_fetch: usize,
}

impl FetchLimits {
    /// Four minutes in all (two fetches' worth of [`git::NETWORK`]), and half a gibibyte of objects the base did not
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
    /// The forge repo's object cache as this push uses it: what a push fetches into, so that it is
    /// sent only what is new, within the push's one deadline ([`super::objectcache::PUSH`]). `None`
    /// fetches the whole base into the quarantine, on the per-call limit alone.
    pub cache: Option<PushCache>,
}

impl FromForge {
    /// [`BaseObjects::fetch_all`] within `limits`, each `git fetch` on [`git::NETWORK`] at most.
    pub fn fetch_all_within(
        &self,
        into: &Path,
        tips: &[HeadSha],
        limits: &FetchLimits,
    ) -> Result<(), String> {
        self.fetch_all_bounded(into, tips, limits, git::NETWORK)
    }

    /// [`BaseObjects::fetch_all`] within `limits`, each `git fetch` taking `bound` at most (and
    /// never past what is left of `limits.deadline`).
    ///
    /// A tip the forge no longer has (a branch deleted since it was listed) fails its whole `git
    /// fetch`, so that chunk is taken again one tip at a time and the tip is skipped: a pack that
    /// needed it is refused by git afterwards, so nothing is let through by skipping. A run that
    /// fetched no tip at all is an error.
    fn fetch_all_bounded(
        &self,
        into: &Path,
        tips: &[HeadSha],
        limits: &FetchLimits,
        bound: Duration,
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
            match fetch_into_refs(
                &self.url,
                self.helper.as_deref(),
                into,
                chunk,
                (
                    "tips",
                    "git could not fetch the advertised tips from the forge",
                ),
                left(started, limits, bound),
                None,
            ) {
                Ok(()) => fetched_any = true,
                Err(why) => {
                    first_failure.get_or_insert(why);
                    for tip in chunk {
                        check_deadline(started, limits)?;
                        if fetch_into_refs(
                            &self.url,
                            self.helper.as_deref(),
                            into,
                            std::slice::from_ref(tip),
                            (
                                "tips",
                                "git could not fetch the advertised tip from the forge",
                            ),
                            left(started, limits, bound),
                            None,
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

/// How long the next `git fetch` of a run may take: `bound`, and no more than is left of the run.
fn left(started: Instant, limits: &FetchLimits, bound: Duration) -> Duration {
    limits
        .deadline
        .saturating_sub(started.elapsed())
        .min(bound)
        .max(Duration::from_secs(1))
}

/// One `git fetch` of `tips`, each into a private ref of the repository at `into`. A commit
/// fetched by its id alone lands only in `FETCH_HEAD`, which git does not negotiate from; a ref
/// does, so every later fetch tells the forge what the repository holds already (`have` lines) and
/// is sent only what is new. The name is the id, so a tip fetched twice is one ref. It may take
/// `bound`.
fn fetch_into_refs(
    url: &str,
    helper: Option<&str>,
    into: &Path,
    tips: &[HeadSha],
    (ref_dir, otherwise): (&str, &str),
    bound: Duration,
    stage: Option<Stage>,
) -> Result<(), String> {
    fetch_specs(
        url,
        helper,
        into,
        &ref_specs(tips, ref_dir),
        otherwise,
        bound,
        (stage, None),
    )
}

fn ref_specs(tips: &[HeadSha], ref_dir: &str) -> Vec<String> {
    tips.iter()
        .map(|tip| format!("+{0}:refs/quarantine/{ref_dir}/{0}", tip.as_str()))
        .collect()
}

/// One `git fetch` of `specs`, in the `stage` of a first fill where there is one. With `watch`,
/// git's progress lines are read as they are written (a fill's: [`ObjectCache::sink`]). git is
/// always run with `--progress`: its writing is what the stall check reads (that it wrote, never
/// what), so a fetch silent for [`objectcache::STALL`] is ended as stuck, and one that is moving is
/// never cut for being slow. A failure is told by git's `fatal:` or `error:` line ([`first_line`]),
/// never by a progress line.
fn fetch_specs(
    url: &str,
    helper: Option<&str>,
    into: &Path,
    specs: &[String],
    otherwise: &str,
    bound: Duration,
    (stage, watch): (Option<Stage>, Option<git::Watch>),
) -> Result<(), String> {
    let staged = stage.map(|stage| stage.args(into)).unwrap_or_default();
    let mut args = vec!["fetch", "--progress", "--no-tags"];
    args.extend(staged.iter().map(String::as_str));
    args.push("--");
    args.push(url);
    args.extend(specs.iter().map(String::as_str));
    let run = git::run_network_stalling(into, helper, &args, bound, objectcache::STALL, watch)
        .map_err(|why| why.to_string())?;
    if run.ok() {
        Ok(())
    } else {
        Err(first_line(&run.err, otherwise))
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
        const OTHERWISE: &str = "git could not fetch the base from the forge";
        let Some(cache) = &self.cache else {
            return fetch_into_refs(
                &self.url,
                self.helper.as_deref(),
                into,
                std::slice::from_ref(base),
                ("base", OTHERWISE),
                git::NETWORK,
                None,
            );
        };
        let (url, helper, tip) = (self.url.clone(), self.helper.clone(), base.clone());
        let sink = cache.cache().sink();
        // A first fill is staged and resumes, of the default branch as the early start's is, and
        // the base is fetched on top of it; a filled cache is topped up with the base alone.
        let filling = cache.cache().clone();
        let fetch: Fetch = Arc::new(move |dir, bound| {
            if !filling.is_filled() {
                stage_default_branch(&url, helper.as_deref(), &filling, dir, bound, &sink)?;
            }
            fetch_specs(
                &url,
                helper.as_deref(),
                dir,
                &ref_specs(std::slice::from_ref(&tip), "base"),
                OTHERWISE,
                bound,
                (None, Some(sink.clone())),
            )
        });
        // A base the cache holds already is read through it while a fill goes on.
        cache.update_having(fetch, &[base.as_str().to_owned()])?;
        cache.cache().keep_base(base.as_str());
        Ok(())
    }

    fn advertised(&self, into: &Path) -> Option<Vec<String>> {
        if self.url.contains('@') {
            return None;
        }
        // Inside the push's one deadline where there is a cache.
        let bound = self.cache.as_ref().map_or(git::NETWORK, |cache| {
            cache.left().min(git::NETWORK).max(Duration::from_secs(1))
        });
        let run = git::run_network_within(
            into,
            self.helper.as_deref(),
            &["ls-remote", "--heads", "--", &self.url],
            bound,
        )
        .ok()?;
        crate::diffscan::tips(run)
    }

    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        let Some(cache) = &self.cache else {
            return self.fetch_all_within(into, tips, &FetchLimits::DEFAULT);
        };
        let (url, helper) = (self.url.clone(), self.helper.clone());
        let wanted = tips.to_vec();
        let fetch: Fetch = Arc::new(move |dir, bound| {
            let alone = FromForge {
                url: url.clone(),
                helper: helper.clone(),
                cache: None,
            };
            let limits = FetchLimits {
                deadline: bound,
                ..FetchLimits::DEFAULT
            };
            alone.fetch_all_bounded(dir, &wanted, &limits, bound)
        });
        // Tips the cache holds already are read through it while a fill goes on.
        let ids: Vec<String> = tips.iter().map(|tip| tip.as_str().to_owned()).collect();
        cache.update_having(fetch, &ids)?;
        let ids: Vec<String> = tips.iter().map(|tip| tip.as_str().to_owned()).collect();
        cache.cache().keep_tips(&ids);
        Ok(())
    }

    fn cache(&self) -> Option<Arc<ObjectCache>> {
        self.cache.as_ref().map(|cache| cache.cache().clone())
    }
}

impl FromForge {
    /// **Starts the object cache's first fill now** (#7532), as a chat opens on the repo, so that a
    /// push does not meet an empty cache: the default branch with its history, on the cache's own
    /// thread and within [`super::FILL`]. Nothing where there is no cache or it is filled or
    /// filling already. It answers at once.
    pub fn warm(&self) {
        let Some(cache) = &self.cache else { return };
        if self.url.contains('@') {
            return;
        }
        let (url, helper) = (self.url.clone(), self.helper.clone());
        let sink = cache.cache().sink();
        let filling = cache.cache().clone();
        let fetch: Fetch = Arc::new(move |dir, bound| {
            stage_default_branch(&url, helper.as_deref(), &filling, dir, bound, &sink)
        });
        cache.cache().start_fill(fetch);
    }
}

/// **A first fill's stages** ([`objectcache::fill_in_stages`]) of the forge's default branch: the
/// target of the early start and of a push alike ([`objectcache::FIRST_FILL_TARGET`]), so that a
/// fill either began is resumed by the other at the stage it reached. Each stage tells the cache
/// ([`ObjectCache::stage_began`]) so that the figure and the part it reports are the stage's own.
fn stage_default_branch(
    url: &str,
    helper: Option<&str>,
    cache: &ObjectCache,
    dir: &Path,
    bound: Duration,
    sink: &git::Watch,
) -> Result<(), String> {
    objectcache::fill_in_stages(
        dir,
        objectcache::FIRST_FILL_TARGET,
        &objectcache::FIRST_FILL,
        |stage| {
            cache.stage_began(stage);
            fetch_specs(
                url,
                helper,
                dir,
                &["+HEAD:refs/quarantine/default".to_owned()],
                "git could not read the forge's default branch",
                bound,
                (Some(stage), Some(sink.clone())),
            )
        },
    )
}

enum Refused {
    ForWantOfObjects(String),
    /// git took the pack in, and the pushed tip is still not here: the pack sent nothing of it,
    /// as git sends when the forge has every object already (an empty pack).
    TipNotHere,
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
        quarantine.has(base).map_err(|why| missing(base, why))?;
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
        self.has(extra).map_err(|why| missing(extra, why))
    }

    /// **Takes in the pack the chat pushed**, whose tip is `tip`: every object hashed and checked by
    /// git, completed against the base where the pack is thin, and refused where the tip is not a
    /// commit whose every object is here.
    pub fn take_in(&self, pack: Vec<u8>, tip: &HeadSha, base: &HeadSha) -> Result<(), String> {
        self.read_in(pack, tip, base)
            .map_err(|refused| match refused {
                Refused::ForWantOfObjects(why) | Refused::Otherwise(why) => why,
                Refused::TipNotHere => format!("the quarantine has no commit {}", short(tip)),
            })
    }

    fn read_in(&self, pack: Vec<u8>, tip: &HeadSha, base: &HeadSha) -> Result<(), Refused> {
        let sent = object_count(&pack);
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
        // **Only a tip that is an object of the pushed pack is taken here.** Asking the quarantine
        // (`cat-file -e`) reads through the object cache as an alternate, which holds more than the
        // advertised history: old base tips since force-pushed away, and the objects of dropped
        // refs, until gc. Any other tip is Refused::TipNotHere, which the caller answers with the
        // reachability test; a failure to look is not that.
        if !self
            .in_the_pack(&done.out, sent, tip)
            .map_err(Refused::Otherwise)?
        {
            return Err(Refused::TipNotHere);
        }
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

    /// Whether `tip` is one of the `sent` objects the pack itself held. `index_pack_said` is what
    /// `index-pack --stdin` printed (`pack\t<name>`); the pack's index lists its objects by offset,
    /// and the ones `--fix-thin` appended (a delta's base, read through the cache) come after the
    /// pack's own, so they are left out. Not found, or not readable, is "no": it fails closed.
    fn in_the_pack(
        &self,
        index_pack_said: &[u8],
        sent: usize,
        tip: &HeadSha,
    ) -> Result<bool, String> {
        let said = String::from_utf8_lossy(index_pack_said);
        let Some(name) = said
            .lines()
            .find_map(|line| line.strip_prefix("pack\t"))
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            return Ok(false);
        };
        if sent == 0 {
            return Ok(false);
        }
        let idx = self
            .path
            .join(".git")
            .join("objects")
            .join("pack")
            .join(format!("pack-{name}.idx"));
        let idx = std::fs::read(idx).map_err(|why| why.to_string())?;
        let listed = git::run_with_input(&self.path, &["show-index"], idx, git::READ)
            .map_err(|why| why.to_string())?;
        if listed.code != Some(0) {
            return Err("git could not read the pushed pack's index".to_owned());
        }
        let mut objects: Vec<(u64, String)> = String::from_utf8_lossy(&listed.out)
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let offset = fields.next()?.parse().ok()?;
                Some((offset, fields.next()?.to_ascii_lowercase()))
            })
            .collect();
        objects.sort_unstable();
        Ok(objects
            .iter()
            .take(sent)
            .any(|(_, id)| id == &tip.as_str().to_ascii_lowercase()))
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
    ///
    /// **A push may also send no new objects** because the repo
    /// has them all: a fast-forward to an integration branch's tip, or a branch made at a commit
    /// the repo already has, is an empty pack. When git took the pack in and `tip` is still not
    /// here, the repo's advertised branch tips are fetched with their history (the same tips, at
    /// most [`KNOWN_AT_MOST`], that a thin pack uses) and `tip` is accepted **only if it is
    /// reachable from one of them**. Whether "the forge has the commit" is no test: a forge
    /// resolves a commit of any fork, and a dangling one (force-pushed away), by its id, and such
    /// a commit is in none of this repo's refs. The fetched objects get the connectivity walk
    /// from `base` (every object of the range is here), not git's per-object check a pack gets, but
    /// they are the repo's own history. The caller's scans still run on `tip` against `base`.
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
            Err(Refused::TipNotHere) => return self.take_in_from_refs(tip, base, from),
            Err(Refused::ForWantOfObjects(why)) => why,
        };
        let (tips, cut) = self.advertised_tips(from);
        if tips.is_empty() {
            return Err(first);
        }
        from.fetch_all(&self.path, &tips)?;
        match self.read_in(pack.to_vec(), tip, base) {
            Ok(()) => Ok(()),
            // A pack of other objects, with the tip in none of them.
            Err(Refused::TipNotHere) => self.reachable_from(&tips, cut, tip, base),
            Err(Refused::ForWantOfObjects(why) | Refused::Otherwise(why)) => Err(why),
        }
    }

    /// The advertised branch tips: distinct, full ids, at most [`KNOWN_AT_MOST`]; empty where the
    /// forge did not answer. The flag is whether more were advertised than were kept.
    fn advertised_tips(&self, from: &dyn BaseObjects) -> (Vec<HeadSha>, bool) {
        let Some(advertised) = from.advertised(&self.path) else {
            return (Vec::new(), false);
        };
        let mut seen = std::collections::HashSet::new();
        let distinct: Vec<&String> = advertised
            .iter()
            .filter(|id| seen.insert(id.to_ascii_lowercase()))
            .collect();
        let cut = distinct.len() > KNOWN_AT_MOST;
        let tips = distinct
            .into_iter()
            .take(KNOWN_AT_MOST)
            .filter_map(|id| HeadSha::try_from(id.as_str()).ok())
            .collect();
        (tips, cut)
    }

    /// The pushed tip is no object of the pack: it is taken only if it is in this repo's own
    /// advertised history.
    fn take_in_from_refs(
        &self,
        tip: &HeadSha,
        base: &HeadSha,
        from: &dyn BaseObjects,
    ) -> Result<(), String> {
        let (tips, cut) = self.advertised_tips(from);
        if tips.is_empty() {
            return Err(
                "the repo's branches could not be read from the forge, so the commit could not be \
                 checked against them; try again in a minute"
                    .to_owned(),
            );
        }
        from.fetch_all(&self.path, &tips)?;
        self.reachable_from(&tips, cut, tip, base)
    }

    /// `tip` is here, and reachable from one of `tips` (fetched), and the range from `base` is
    /// whole.
    fn reachable_from(
        &self,
        tips: &[HeadSha],
        cut: bool,
        tip: &HeadSha,
        base: &HeadSha,
    ) -> Result<(), String> {
        let unknown = || {
            if cut {
                // Not found among the first KNOWN_AT_MOST branches: it may be on one past them.
                format!(
                    "it names commit {}, which is not in the push, and this repo has more branches \
                     than the {KNOWN_AT_MOST} that can be checked, so the branch list was too long \
                     to check it against",
                    short(tip)
                )
            } else {
                format!(
                    "it names commit {}, which is neither in the push nor in a branch of this repo",
                    short(tip)
                )
            }
        };
        match self.has(tip) {
            Ok(()) => {}
            Err(None) => return Err(unknown()),
            Err(Some(why)) => return Err(why),
        }
        // Reachable from an advertised tip: nothing of `tip` is outside what they hold. A tip the
        // fetch skipped (a branch deleted since it was listed) is ignored, and `tip` is here
        // (checked above), so an ignored `tip` cannot pass for an empty answer.
        let mut args = vec!["rev-list", "-n1", "--ignore-missing", tip.as_str(), "--not"];
        args.extend(tips.iter().map(HeadSha::as_str));
        let outside = git::run(&self.path, &args, TAKING_IN).map_err(|why| why.to_string())?;
        if !outside.ok() || !outside.out.trim().is_empty() {
            return Err(unknown());
        }
        // The range from the base is whole.
        match self.walk(tip, base, "error") {
            Ok(walked) if walked.ok() => Ok(()),
            Ok(walked) => Err(first_line(
                &walked.err,
                "the commit's objects are not whole",
            )),
            Err(Refused::ForWantOfObjects(why) | Refused::Otherwise(why)) => Err(why),
            Err(Refused::TipNotHere) => Err(unknown()),
        }
    }

    /// `Ok` where `commit` is here; `Err(None)` where it is not, `Err(Some(why))` where git could
    /// not be asked.
    fn has(&self, commit: &HeadSha) -> Result<(), Option<String>> {
        let found = git::run(
            &self.path,
            &["cat-file", "-e", &format!("{}^{{commit}}", commit.as_str())],
            git::READ,
        )
        .map_err(|why| Some(why.to_string()))?;
        if found.ok() { Ok(()) } else { Err(None) }
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

/// The object count in a pack's header (`PACK`, a version, the count), 0 where there is no header.
fn object_count(pack: &[u8]) -> usize {
    pack.get(8..12)
        .filter(|_| pack.starts_with(b"PACK"))
        .map_or(0, |bytes| {
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
        })
}

fn short(commit: &HeadSha) -> &str {
    &commit.as_str()[..12]
}

/// The message of a `has` that found no commit, for the callers that return a string.
fn missing(commit: &HeadSha, why: Option<String>) -> String {
    why.unwrap_or_else(|| format!("the quarantine has no commit {}", short(commit)))
}

/// The first line of what git said, or `otherwise`. git's progress rewrites its line with `\r`, so
/// a line ends at either, and a progress line is no reason: git's own `fatal:` or `error:` leads.
fn first_line(said: &str, otherwise: &str) -> String {
    let lines = || {
        said.split(['\n', '\r'])
            .map(str::trim)
            .filter(|line| !line.is_empty())
    };
    lines()
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .or_else(|| lines().find(|line| !is_progress(line)))
        .unwrap_or(otherwise)
        .to_owned()
}

/// A line of git's progress report (`Receiving objects: 45% (…)`).
fn is_progress(line: &str) -> bool {
    [
        "Receiving objects",
        "Resolving deltas",
        "remote: Enumerating",
        "remote: Counting",
        "remote: Compressing",
        "remote: Total",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}
