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
//!    thin pack is a delta against, and what the diff is made against.
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
use std::time::Duration;

use super::ci::GitTrees;
use crate::diffscan::KNOWN_AT_MOST;
use crate::mergehold::HeadSha;
use crate::worktree::git;

/// The tips fetched in one `git fetch`: an argument list that stays well inside a platform's limit.
const TIPS_PER_FETCH: usize = 256;

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

    /// Fetches every commit of `tips`, with its history, into the repository at `into`.
    fn fetch_all(&self, into: &Path, tips: &[HeadSha]) -> Result<(), String> {
        tips.iter().try_for_each(|tip| self.fetch(into, tip))
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
}

impl BaseObjects for FromForge {
    fn fetch(&self, into: &Path, base: &HeadSha) -> Result<(), String> {
        if self.url.contains('@') {
            return Err("a forge's address carries no user information".to_owned());
        }
        let run = git::run_network(
            into,
            self.helper.as_deref(),
            &["fetch", "--quiet", "--no-tags", &self.url, base.as_str()],
        )
        .map_err(|why| why.to_string())?;
        if run.ok() {
            Ok(())
        } else {
            Err(first_line(
                &run.err,
                "git could not fetch the base from the forge",
            ))
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
        if self.url.contains('@') {
            return Err("a forge's address carries no user information".to_owned());
        }
        for chunk in tips.chunks(TIPS_PER_FETCH) {
            let mut args = vec!["fetch", "--quiet", "--no-tags", self.url.as_str()];
            args.extend(chunk.iter().map(HeadSha::as_str));
            let run = git::run_network(into, self.helper.as_deref(), &args)
                .map_err(|why| why.to_string())?;
            if !run.ok() {
                return Err(first_line(
                    &run.err,
                    "git could not fetch the advertised tips from the forge",
                ));
            }
        }
        Ok(())
    }
}

/// **A repository of the app's own, with nothing checked out**, removed when this is dropped.
pub struct Quarantine {
    path: PathBuf,
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
        let quarantine = Quarantine { path };
        from.fetch(&quarantine.path, base)?;
        quarantine.has(base)?;
        Ok(quarantine)
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
        let done = git::run_with_input(
            &self.path,
            &["index-pack", "--stdin", "--fix-thin", "--strict"],
            pack,
            TAKING_IN,
        )
        .map_err(|why| why.to_string())?;
        if done.code != Some(0) {
            return Err(first_line(
                &String::from_utf8_lossy(&done.err),
                "git did not take the pushed objects in",
            ));
        }
        self.has(tip)?;
        // Every object of the push is here: a tip whose tree or blobs are missing has no diff.
        // `--quiet` lists nothing, so a pack of millions of objects needs no cap: only the exit
        // code is read.
        let walked = git::run(
            &self.path,
            &[
                "rev-list",
                "--objects",
                "--quiet",
                "--missing=error",
                tip.as_str(),
                "--not",
                base.as_str(),
            ],
            TAKING_IN,
        )
        .map_err(|why| why.to_string())?;
        if walked.ok() {
            Ok(())
        } else {
            Err(first_line(&walked.err, "the pushed objects are not whole"))
        }
    }

    /// **[`Self::take_in`], where a thin pack may be built on any branch the forge advertises**
    /// (ADR 0128, amended 2026-11-23): a task branch cut from an integration branch is a pack
    /// whose deltas are against that branch's objects, which the quarantine does not hold. When
    /// the pack is refused for want of them, the forge's advertised branch tips, at most
    /// [`KNOWN_AT_MOST`] and nothing a chat names, are fetched with their history from `from`,
    /// and the pack is tried once more. **The pack is verified either way**: git hashes and checks
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
        let first = match self.take_in(pack.to_vec(), tip, base) {
            Ok(()) => return Ok(()),
            Err(first) => first,
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
