// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A sealed set: a list of answers, each sealed on its own** (ticket T1.1-73, #6968; rulings
//! Q306 and Q308b), for the answers this machine keeps as a list: the dispatch grants, the
//! acceptances of the project's grants, and the person's nevers.
//!
//! # What holds an entry in place
//!
//! - **Its own seal** ([`super::seal`]), over the set's store, the entry's kind, the plane's
//!   anchor ([`crate::sandbox::local::anchor_key`]), its text and its `generation`: the set's
//!   counter when the app made it. An entry edited by hand or by a chat, or copied from another
//!   clone, has a seal that does not hold.
//! - **The live list**: the seals of the entries in force, beside them in the file, with the
//!   set's `generation`, which rises on every write, a removal included. **The app anchors the
//!   seal of the counter and the sorted live list in the keyring** ([`super::seal::keep`]), out
//!   of every chat's reach. That anchor is the sealed counter and digest of the live set.
//!
//! An entry counts only where its own seal holds **and** its seal is in a live list whose
//! anchor holds ([`Set::verify`]). So a revoked entry put back from a copy is in no live list,
//! and an older copy of the whole set does not match the anchor and reads as broken.
//!
//! # No re-seal in bulk
//!
//! [`Set::rewrite`] starts from what verifies, never from what is on disk: what a chat put
//! there, or what an older build wrote, is never sealed by a later write of the app's. An entry
//! is sealed only where the person answered for it, one at a time.
//!
//! # Where it is weaker
//!
//! As the seal is ([`super::seal`]): only the app verifies, and in any other process nothing
//! holds. On Linux and Windows any of the person's programs can read the keyring, so there a set
//! stops a hand edit and a sandboxed chat only.

use serde::{Deserialize, Serialize};

/// **Whether sealed sets decide in this process**: in purlis's app, outside a test build of
/// another crate (`fence::FENCED`). Anywhere else the answers they seal are read as written, as
/// before ticket T1.1-73, and nothing is sealed: no other process judges a dispatch for a chat.
pub fn decides() -> bool {
    #[cfg(test)]
    {
        crate::enforcement::in_the_app()
    }
    #[cfg(not(test))]
    {
        super::keyhold::is_the_app() && !crate::fence::FENCED
    }
}

/// Whether this process can seal and verify a set: [`super::seal::can_seal`].
pub fn can_seal() -> bool {
    super::seal::can_seal()
}

/// One sealed entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// What sort of answer it is, within its store (`mine`, `seen`, `never`).
    pub kind: String,
    /// The answer, as the store spells it.
    pub text: String,
    /// The set's counter when the app sealed it.
    #[serde(default)]
    pub generation: u64,
    /// Its seal, in hex.
    #[serde(default)]
    pub mac: String,
}

/// A sealed set, as it is kept beside the answers it seals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    /// One past the last on every write the app made.
    #[serde(default)]
    pub generation: u64,
    /// The seals of the entries in force, as the app last wrote them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live: Vec<String>,
    /// The entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
}

/// What [`Set::verify`] found of a set as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Its anchor holds: each entry is judged on its own.
    Holds,
    /// The app never anchored one here, and nothing is in it: a fresh install.
    Fresh,
    /// One was anchored and this is not it (removed, an older copy put back, a live seal taken
    /// out): nothing in it counts.
    Broken,
    /// Nothing was ever anchored, and entries are here, or a counter only the app's writes
    /// raise: written by something other than the app, by a build from before the set, or
    /// anchored once under a path the plane is no longer reached by. Nothing in it counts; a store of denies imports
    /// them once ([`crate::dispatchnever`]).
    Unanchored,
    /// This process cannot verify (it is not the app, or the key cannot be read): nothing
    /// counts.
    Unverifiable,
}

/// What [`Set::verify`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    /// How the set stands as a whole.
    pub standing: Standing,
    /// The entries that count, each as kind and text.
    pub holding: Vec<(String, String)>,
    /// The entries that do not, each as kind and text: shown read-only, never counted.
    pub dropped: Vec<(String, String)>,
    /// How many seals of the live list name no entry that holds: a sealed answer taken out.
    pub missing: usize,
}

impl Verified {
    /// Whether `kind` `text` counts.
    pub fn holds(&self, kind: &str, text: &str) -> bool {
        self.holding.iter().any(|(k, t)| k == kind && t == text)
    }

    /// The texts of `kind` that count.
    pub fn of_kind(&self, kind: &str) -> Vec<String> {
        self.holding
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, text)| text.clone())
            .collect()
    }
}

/// The seal of one entry of `store`, or none where this process cannot seal.
fn mac_of(store: &str, clone: &str, kind: &str, text: &str, generation: u64) -> Option<String> {
    super::seal::sealed(
        &format!("{store}/{kind}"),
        clone,
        &format!("{generation}\n{text}"),
    )
}

/// Whether `entry`'s own seal holds for `store` and `clone`.
fn entry_holds(store: &str, clone: &str, entry: &Entry) -> bool {
    super::seal::holds(
        &entry.mac,
        &format!("{store}/{}", entry.kind),
        clone,
        &format!("{}\n{}", entry.generation, entry.text),
    )
}

/// The keyring account kind a set of `store` is anchored under.
fn anchor_kind(store: &str) -> String {
    format!("{store}-live")
}

impl Set {
    /// The text the anchor seals: the counter, then the live list sorted.
    fn live_text(generation: u64, live: &[String]) -> String {
        let mut sorted = live.to_vec();
        sorted.sort();
        format!("{generation}\n{}", sorted.join("\n"))
    }

    /// Whether nothing is in it.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.live.is_empty()
    }

    /// **What counts of this set**, the set of `store` for the plane anchored `clone`.
    pub fn verify(&self, store: &str, clone: &str) -> Verified {
        let all = || {
            self.entries
                .iter()
                .map(|entry| (entry.kind.clone(), entry.text.clone()))
                .collect::<Vec<_>>()
        };
        let none = |standing| Verified {
            standing,
            holding: Vec::new(),
            dropped: all(),
            missing: 0,
        };
        if !super::seal::can_seal() {
            return none(Standing::Unverifiable);
        }
        match super::seal::kept_holds(
            &anchor_kind(store),
            clone,
            &Self::live_text(self.generation, &self.live),
        ) {
            // A set the app wrote at least once (its counter past nought) was anchored then: no
            // anchor now is one taken away, or a plane reached by another path, never a fresh
            // install (the review of #7113).
            None if self.is_empty() => none(Standing::Fresh), // MUTATION(5)
            None => none(Standing::Unanchored),
            Some(false) => none(Standing::Broken),
            Some(true) => {
                let (mut holding, mut dropped) = (Vec::new(), Vec::new());
                let mut found = Vec::new();
                for entry in &self.entries {
                    let pair = (entry.kind.clone(), entry.text.clone());
                    if self.live.contains(&entry.mac)
                        && entry_holds(store, clone, entry)
                        && !holding.contains(&pair)
                    {
                        found.push(entry.mac.clone());
                        holding.push(pair);
                    } else {
                        dropped.push(pair);
                    }
                }
                let missing = self.live.iter().filter(|mac| !found.contains(mac)).count();
                Verified {
                    standing: Standing::Holds,
                    holding,
                    dropped,
                    missing,
                }
            }
        }
    }

    /// **Rewrites the set from what verifies**: each entry that counts and `keep` keeps, then
    /// each of `add` not already among them, sealed at a counter one past the last, and the new
    /// live list anchored in the keyring **before** the caller writes the set back, so a set
    /// that never reaches the disk, or one put back from before, reads as broken. Refused where
    /// this process cannot seal: nothing is changed then.
    ///
    /// Never a re-seal of what is on disk: an entry that does not verify is dropped here.
    pub fn rewrite(
        &mut self,
        store: &str,
        clone: &str,
        keep: impl Fn(&str, &str) -> bool,
        add: &[(&str, &str)],
    ) -> std::io::Result<()> {
        if !super::seal::can_seal() {
            return Err(std::io::Error::other(
                "only purlis's app can seal an answer, and it could not here, so nothing was \
                 changed",
            ));
        }
        let verified = self.verify(store, clone);
        let generation = self.generation.saturating_add(1);
        let mut entries: Vec<Entry> = self
            .entries
            .iter()
            .filter(|entry| {
                verified.holds(&entry.kind, &entry.text)
                    && self.live.contains(&entry.mac)
                    && entry_holds(store, clone, entry)
                    && keep(&entry.kind, &entry.text)
            })
            .cloned()
            .collect();
        entries.dedup_by(|a, b| a.kind == b.kind && a.text == b.text);
        for (kind, text) in add {
            if entries
                .iter()
                .any(|one| one.kind == *kind && one.text == *text)
            {
                continue;
            }
            let mac = mac_of(store, clone, kind, text, generation)
                .ok_or_else(|| std::io::Error::other("purlis could not seal the answer"))?;
            entries.push(Entry {
                kind: (*kind).to_owned(),
                text: (*text).to_owned(),
                generation,
                mac,
            });
        }
        let live: Vec<String> = entries.iter().map(|entry| entry.mac.clone()).collect();
        if !super::seal::keep(
            &anchor_kind(store),
            clone,
            &Self::live_text(generation, &live),
        ) {
            return Err(std::io::Error::other(
                "purlis could not keep the answer's seal in the keyring, so nothing was changed",
            ));
        }
        *self = Self {
            generation,
            live,
            entries,
        };
        Ok(())
    }

    /// Whether an anchor of `store` is kept for `clone`, whether or not this process can read
    /// the key: the presence marker, where nothing else can be told.
    pub fn anchored(store: &str, clone: &str) -> bool {
        super::seal::kept_exists(&anchor_kind(store), clone)
    }
}

#[cfg(test)]
#[path = "sealed_set_tests.rs"]
mod tests;
