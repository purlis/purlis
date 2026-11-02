// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each open chat's skill stamp as it would be now, kept** (#4738).
//!
//! `chats_plane_updated` is asked on every tab mount, and each ask read every open chat's skill
//! folders again ([`super::Chats::skills_changed`]). What a chat would get now changes only when
//! the plane's instructions do, and the plane's watch says when they do (`planes::applying`), so
//! each stamp is read once and kept until then.
//!
//! Nothing is kept while the plane is not wholly watched: with no watch to tell of a change, every
//! ask reads again, as it always did. Nor for a Try it's scratch chat, whose skills come from its
//! try's folder in the app's own state, which the plane's watch does not tell of.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use purlis_core::harness::Harness;
use purlis_core::hoststart::ChatSkills;
use purlis_core::skills::Stamp;

/// What a chat's stamp now is read from: its workspace, its persona, its harness and where its
/// start read its plane skills. A chat started again on any other is read again.
pub(super) type ReadFrom = (Option<String>, Option<String>, Harness, ChatSkills);

/// The kept stamps, by session.
#[derive(Debug, Default)]
pub struct SkillsNow {
    kept: Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    /// Whether the plane's watch is live, which is what tells of a change.
    watched: bool,
    /// How many times the stamps were forgotten: a read that began before a forget does not keep
    /// what it read, which may be from before the change.
    forgotten: u64,
    stamps: HashMap<u32, (ReadFrom, Stamp)>,
}

impl SkillsNow {
    /// Whether the plane's watch can be trusted to tell every change now (`Held::trusted`, the
    /// sidebar model's rule): while it is, a stamp is kept until it says the instructions
    /// changed; while it is not, nothing is kept and every ask reads again.
    pub fn watched(&self, whole: bool) {
        let mut kept = self.lock();
        kept.watched = whole;
        if !whole {
            kept.forgotten += 1;
            kept.stamps.clear();
        }
    }

    /// The plane's instructions changed, or what changed is not known: every stamp is read
    /// again at the next ask.
    pub fn forget(&self) {
        let mut kept = self.lock();
        kept.forgotten += 1;
        kept.stamps.clear();
    }

    /// How many times the stamps were forgotten.
    #[cfg(test)]
    pub fn forgotten(&self) -> u64 {
        self.lock().forgotten
    }

    /// The stamp now of each chat in `chats`, by `read` where none is kept for what it is read
    /// from. `read` runs with no lock held. What is kept afterwards is these chats' alone, so a
    /// closed chat's goes with it.
    pub(super) fn of(
        &self,
        chats: Vec<(u32, ReadFrom)>,
        read: impl Fn(&ReadFrom) -> Stamp,
    ) -> Vec<(u32, Stamp)> {
        let (watched, forgotten, found) = {
            let kept = self.lock();
            let found: Vec<(u32, ReadFrom, Option<Stamp>)> = chats
                .into_iter()
                .map(|(session, from)| {
                    let stamp = kept
                        .stamps
                        .get(&session)
                        .filter(|(was, _)| *was == from)
                        .map(|(_, stamp)| stamp.clone());
                    (session, from, stamp)
                })
                .collect();
            (kept.watched, kept.forgotten, found)
        };
        let now: Vec<(u32, ReadFrom, Stamp)> = found
            .into_iter()
            .map(|(session, from, stamp)| {
                let stamp = stamp.unwrap_or_else(|| read(&from));
                (session, from, stamp)
            })
            .collect();
        let mut kept = self.lock();
        if watched && kept.forgotten == forgotten {
            kept.stamps = now
                .iter()
                .filter(|(_, from, _)| !tried(from))
                .map(|(session, from, stamp)| (*session, (from.clone(), stamp.clone())))
                .collect();
        }
        drop(kept);
        now.into_iter()
            .map(|(session, _, stamp)| (session, stamp))
            .collect()
    }

    /// The stamps, through a poisoned lock too: a panic elsewhere costs a read, never a mark.
    fn lock(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A Try it's scratch chat, whose skills are its try's folder, never kept.
fn tried(from: &ReadFrom) -> bool {
    from.3.on && from.3.tried
}
