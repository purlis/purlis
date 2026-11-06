// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Several updates accepted at once** (#5354): every one checked before the first is written,
//! then written as **one commit, all or nothing**, with one Undo. A chat keeps the copy it started
//! with until then, as it does for one update.
//!
//! **What a review leaves unticked stays out until it is ticked on its own** (operator's ruling of
//! 2026-11-05): each update is accepted with the kinds of new capability the person ticked it
//! with ([`Gained`]), and one that gains a kind it was not accepted with is refused here, whatever
//! the window drew, so nothing new runs that no one looked at.

use std::collections::BTreeSet;

use super::super::lock::{self, Kept, Locked, Lockfile};
use super::super::review::Accepted;
use super::super::widen;
use super::{Gained, Planned, Wrote, plan, write_planned};
use crate::counts::counted;
use crate::skills::Grant;
use crate::skills::verbs::{self, Ctx, Level};

/// **One update of a set, as it was accepted**: the skill, what its review was accepted as, the
/// grant the review showed, and each kind of new capability the person ticked it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Updating {
    pub level: Level,
    pub name: String,
    pub accepted: Accepted,
    pub grant: Option<Grant>,
    pub allowed: Vec<Gained>,
}

/// One lockfile as a set's update found it and as it left it.
struct LockMoved {
    kept: Kept,
    before: Lockfile,
    after: Lockfile,
}

/// **What a set's update did, and what its Undo takes back**: held by whoever offers the Undo,
/// never written anywhere.
pub struct Undo {
    wrote: Vec<Wrote>,
    locks: Vec<LockMoved>,
    /// Each skill updated, as its lockfile entry read after the update.
    updated: Vec<Locked>,
}

impl Undo {
    /// How many skills it updated.
    pub fn count(&self) -> usize {
        self.updated.len()
    }
}

impl std::fmt::Debug for Undo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Undo")
            .field(
                "updated",
                &self.updated.iter().map(|one| &one.name).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// The kinds `gained` holds that `allowed` does not, in words.
fn not_allowed(gained: &[Gained], allowed: &[Gained]) -> Vec<Gained> {
    gained
        .iter()
        .filter(|one| !allowed.contains(one))
        .copied()
        .collect()
}

/// Takes back every write of `wrote`, newest first: what could not be, in words.
fn take_back_all(wrote: &[Wrote]) -> Vec<String> {
    wrote.iter().rev().filter_map(Wrote::take_back).collect()
}

/// Writes each of `locks` back as it was before: what could not be, in words.
fn locks_back(plane: &std::path::Path, locks: &[LockMoved]) -> Vec<String> {
    let mut left = Vec::new();
    for moved in locks.iter().rev() {
        let now = lock::read(plane, moved.kept).map(|read| read.was);
        let back =
            now.and_then(|was| lock::write(plane, moved.kept, was.as_deref(), &moved.before));
        if let Err(why) = back {
            left.push(format!("{} ({why})", moved.kept.file()));
        }
    }
    left
}

/// The sentence a set that was taken back says, with what could not be.
fn none_updated(why: &str, left: &[String]) -> String {
    if left.is_empty() {
        format!("{why}, so no skill was updated")
    } else {
        format!("{why}, and {} could not be put back", left.join(", "))
    }
}

/// Each lockfile the planned updates move, read, and written with their entries.
fn moved_locks(
    plane: &std::path::Path,
    planned: &[Planned],
) -> Result<Vec<(LockMoved, Option<String>)>, String> {
    let mut out = Vec::new();
    for kept in [Kept::Shared, Kept::Local] {
        let mine: Vec<&Planned> = planned
            .iter()
            .filter(|one| Kept::of(&one.level) == kept)
            .collect();
        if mine.is_empty() {
            continue;
        }
        let held = lock::read(plane, kept)?;
        let mut after = held.lock.clone();
        for one in &mine {
            let at = after
                .skills
                .iter()
                .position(|entry| entry.is(&one.level, &one.name))
                .ok_or_else(|| {
                    format!(
                        "{} is no longer recorded as installed, so no skill was updated",
                        one.name
                    )
                })?;
            after.skills[at] = one.entry();
            after.skills.extend(one.brought_entries());
        }
        out.push((
            LockMoved {
                kept,
                before: held.lock,
                after,
            },
            held.was,
        ));
    }
    Ok(out)
}

/// **Accepts every update of `set` as one change**: each checked first ([`plan`]) and refused
/// whole where one gains a kind of capability it was not accepted with, or two would bring along
/// the same skill; then each written, the lockfiles moved and one commit made of what git keeps.
/// Whatever fails after the first write takes every one back. Answers what its Undo takes back.
pub fn update_set(cx: &mut Ctx<'_>, set: &[Updating]) -> Result<Undo, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if set.is_empty() {
        return Err("no skill was named to update".to_owned());
    }
    let mut seen = BTreeSet::new();
    for one in set {
        if !seen.insert((one.level.to_string(), one.name.clone())) {
            return Err(format!(
                "{} is named twice, so no skill was updated",
                one.name
            ));
        }
    }
    // Every one is checked before the first is written.
    let mut planned = Vec::new();
    for one in set {
        let checked = plan(cx, &one.level, &one.name, &one.accepted, one.grant.as_ref())
            .map_err(|why| format!("{}: {why}", one.name))?;
        let missing = not_allowed(&checked.gained, &one.allowed);
        if !missing.is_empty() {
            return Err(format!(
                "{} newly adds what it was not accepted with ({}), so no skill was updated: \
                 open it and tick it on its own",
                one.name,
                missing
                    .iter()
                    .map(|kind| kind.said())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        planned.push(checked);
    }
    let mut landing = BTreeSet::new();
    for one in &planned {
        for name in one.brought_names() {
            if !landing.insert((one.level.to_string(), name.clone())) {
                return Err(format!(
                    "two of these bring along {name}, so no skill was updated: update them one \
                     at a time"
                ));
            }
        }
    }
    let locks = moved_locks(plane, &planned)?;
    let mut wrote: Vec<Wrote> = Vec::new();
    for one in &planned {
        match write_planned(plane, one) {
            Ok(done) => wrote.push(done),
            Err(why) => {
                let left = take_back_all(&wrote);
                return Err(none_updated(&format!("{}: {why}", one.name), &left));
            }
        }
    }
    let mut moved: Vec<LockMoved> = Vec::new();
    for (lock_moved, was) in locks {
        if let Err(why) = lock::write(plane, lock_moved.kept, was.as_deref(), &lock_moved.after) {
            let mut left = locks_back(plane, &moved);
            left.extend(take_back_all(&wrote));
            return Err(none_updated(&why, &left));
        }
        moved.push(lock_moved);
    }
    let mut paths: Vec<String> = planned
        .iter()
        .zip(&wrote)
        .filter(|(one, _)| Kept::of(&one.level) == Kept::Shared)
        .flat_map(|(_, done)| done.paths.iter().cloned())
        .collect();
    let count = planned.len();
    if !paths.is_empty() {
        paths.push(lock::FILE.to_owned());
        let commits_now = crate::planegit::paths_unsaved(plane, &paths).is_none();
        let before = verbs::head(plane);
        let names: Vec<&str> = planned.iter().map(|one| one.name.as_str()).collect();
        cx.commit(
            &paths,
            &format!("skill update {count}\n\n{}", names.join("\n")),
        );
        if commits_now && verbs::head(plane) == before {
            let mut left = locks_back(plane, &moved);
            left.extend(take_back_all(&wrote));
            return Err(none_updated("its commit could not be made", &left));
        }
    }
    for one in &planned {
        // Each skill it brought is a new one: the grades of one that had its name before are none
        // of its own (#4104).
        for skill in one.brought_names() {
            verbs::earlier_dropped(cx, &one.level, &skill);
        }
        widen::said_done(cx, one.widenings(), "what was updated");
    }
    cx.done(format!(
        "{} updated; Undo puts them back as they were",
        counted(count, "skill", "skills")
    ));
    Ok(Undo {
        updated: planned.iter().map(Planned::entry).collect(),
        wrote,
        locks: moved,
    })
}

/// Why a set's Undo did not take it back whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotUndone {
    /// Refused before anything was touched: a skill or a grant changed since. The Undo can be
    /// offered again once that is put right.
    Refused(String),
    /// Taken back but for what the sentence names: it is not offered again.
    Partly(String),
}

impl std::fmt::Display for NotUndone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(why) | Self::Partly(why) => f.write_str(why),
        }
    }
}

/// Each lockfile entry the update changed or added, put back as it was in the lockfile as it is
/// now: every entry it did not touch, made since or not, stays. What could not be, in words.
fn entries_back(plane: &std::path::Path, locks: &[LockMoved]) -> Vec<String> {
    let mut left = Vec::new();
    for moved in locks {
        let result = lock::read(plane, moved.kept).and_then(|held| {
            let mut now = held.lock.clone();
            for entry in &moved.after.skills {
                let was = moved
                    .before
                    .skills
                    .iter()
                    .find(|old| old.is(&entry.level.0, &entry.name));
                if was == Some(entry) {
                    continue;
                }
                now.skills
                    .retain(|one| !one.is(&entry.level.0, &entry.name));
                if let Some(was) = was {
                    now.skills.push(was.clone());
                }
            }
            lock::write(plane, moved.kept, held.was.as_deref(), &now)
        });
        if let Err(why) = result {
            left.push(format!("{} ({why})", moved.kept.file()));
        }
    }
    left
}

/// **Takes back a set's update** (`undo`): each copy as it was, the skills it brought along
/// gone, the grants it widened as they were and the pins back, as one commit, every lockfile
/// entry it did not touch left as it is now. Refused whole, with nothing touched, where any skill
/// it updated or brought along, or any grant it widened, changed since.
pub fn undo_update_set(cx: &mut Ctx<'_>, undo: &Undo) -> Result<(), NotUndone> {
    let plane = cx.plane;
    crate::settings::writable(plane).map_err(NotUndone::Refused)?;
    let (now, _) = lock::every(plane);
    let touched = undo.locks.iter().flat_map(|moved| {
        moved
            .after
            .skills
            .iter()
            .filter(|entry| !moved.before.skills.contains(entry))
    });
    for after in touched {
        let same = now.iter().any(|(_, one)| one == after);
        if !same || super::edited_since_install(plane, after) {
            return Err(NotUndone::Refused(format!(
                "{} changed since it was updated, so nothing was undone",
                after.name
            )));
        }
    }
    if !undo.wrote.iter().all(Wrote::grants_as_left) {
        return Err(NotUndone::Refused(
            "a grant the update widened changed since, so nothing was undone".to_owned(),
        ));
    }
    let mut left = take_back_all(&undo.wrote);
    left.extend(entries_back(plane, &undo.locks));
    // Only a shared level's copies are committed.
    let mut paths: Vec<String> = undo
        .updated
        .iter()
        .zip(&undo.wrote)
        .filter(|(one, _)| Kept::of(&one.level.0) == Kept::Shared)
        .flat_map(|(_, done)| done.paths.iter().cloned())
        .collect();
    let count = undo.count();
    if !paths.is_empty() {
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill update undo {count}"));
    }
    if !left.is_empty() {
        return Err(NotUndone::Partly(format!(
            "the update was undone but for {}",
            left.join(", ")
        )));
    }
    cx.done(format!(
        "{} back as they were",
        counted(count, "skill", "skills")
    ));
    Ok(())
}
