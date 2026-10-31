// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a running chat's skills were when it started** (SK-7, #2770; ADR 0099 §5).
//!
//! A running chat keeps the skill set it started with, except what its harness takes in by
//! itself ([`Reload`]). The window keeps a [`Stamp`] of each chat's set at its start, and when the
//! project's skills change it takes a new one: where the two differ, the chat's tab says its skills
//! changed and offers a restart on the same conversation.

use std::path::PathBuf;

use crate::SkillSet;

/// What a running chat of a harness takes in by itself when the project's skills change, through
/// the route purlis hands them by (ADR 0099 §5). Whatever else changes waits for a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reload {
    /// Nothing: a skill added, removed, renamed, moved or edited waits for the restart.
    Nothing,
    /// A listed skill's `SKILL.md`, which the chat reads from where it lies each time the skill
    /// applies: an edit to it is read, and every other change waits for the restart.
    Bodies,
    /// Everything: the chat asks for its skill set afresh each time it finds or loads a skill,
    /// so no change waits for a restart.
    Everything,
}

/// One chat's plane skills as they were at a moment, by name. Two stamps differ exactly where a
/// chat started at the first would miss something of the second.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamp(Vec<Stamped>);

/// One skill in a [`Stamp`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Stamped {
    name: String,
    /// Its `SKILL.md`.
    path: PathBuf,
    description: String,
    /// Its `SKILL.md`'s text, where the chat's harness would not read an edit to it by itself.
    body: Option<Vec<u8>>,
}

impl Stamp {
    /// The stamp of `set`'s plane skills for a chat whose harness takes in `reload` by itself.
    /// A `SKILL.md` that cannot be read is stamped as no text, so it reads as changed once it can.
    pub fn of(set: &SkillSet, reload: Reload) -> Stamp {
        if reload == Reload::Everything {
            return Stamp::default();
        }
        let mut rows: Vec<_> = set
            .from_the_plane()
            .map(|effective| {
                let body = match reload {
                    Reload::Bodies | Reload::Everything => None,
                    Reload::Nothing => {
                        Some(std::fs::read(&effective.skill.path).unwrap_or_default())
                    }
                };
                Stamped {
                    name: effective.skill.name.clone(),
                    path: effective.skill.path.clone(),
                    description: effective.skill.description.clone(),
                    body,
                }
            })
            .collect();
        // The index a router hands in place of what it hid (SK-15): a change to what is hidden
        // changes it: what it names, and what it says to use instead, but not its text (#3245).
        // The harness lists the index by its fixed description, and a hidden skill is loaded as
        // it is now (`skill_load`), so a hidden skill's description edit is nothing a running
        // chat misses.
        if let Some(index) = set.index() {
            rows.push(Stamped {
                name: index.name,
                path: PathBuf::new(),
                description: named_by_the_index(set),
                body: None,
            });
        }
        rows.sort();
        Stamp(rows)
    }

    /// **What changed from this stamp to `now`, by skill name** (#3694, S8): the skills a chat
    /// started at this one lacks, the ones it has that are gone, and the ones in both that
    /// changed (moved, described anew, or edited where its harness would not read the edit). Each
    /// list in name order, each name once.
    pub fn changed_to(&self, now: &Stamp) -> SkillsChanged {
        let names = |stamp: &Stamp| -> std::collections::BTreeSet<String> {
            stamp.0.iter().map(|row| row.name.clone()).collect()
        };
        let rows_of = |stamp: &Stamp, name: &str| -> Vec<Stamped> {
            stamp
                .0
                .iter()
                .filter(|row| row.name == name)
                .cloned()
                .collect()
        };
        let (then, now_names) = (names(self), names(now));
        SkillsChanged {
            added: now_names.difference(&then).cloned().collect(),
            gone: then.difference(&now_names).cloned().collect(),
            edited: then
                .intersection(&now_names)
                .filter(|name| rows_of(self, name) != rows_of(now, name))
                .cloned()
                .collect(),
        }
    }
}

/// **What changed in a running chat's skills since it started, by name** ([`Stamp::changed_to`]):
/// what the Skills view marks when its chat's skills changed (#3694).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillsChanged {
    /// Skills a chat started now would get that it does not have.
    pub added: Vec<String>,
    /// Skills it has that a chat started now would not get.
    pub gone: Vec<String>,
    /// Skills it has whose file, description or (where its harness does not read an edit by
    /// itself) text changed.
    pub edited: Vec<String>,
}

impl SkillsChanged {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.gone.is_empty() && self.edited.is_empty()
    }
}

/// What `set`'s index names, one line each, by name: every skill a router hid, and where one is
/// deprecated, that it is and what it names instead.
fn named_by_the_index(set: &SkillSet) -> String {
    let mut named: Vec<String> = set
        .effective
        .iter()
        .filter(|effective| set.hidden_by_router(&effective.skill.name).is_some())
        .map(|effective| match &effective.deprecated {
            None => effective.skill.name.clone(),
            Some(deprecated) => match &deprecated.replacement {
                None => format!("{}\tdeprecated", effective.skill.name),
                Some(instead) => format!("{}\tdeprecated\t{instead}", effective.skill.name),
            },
        })
        .collect();
    named.sort();
    named.join("\n")
}

#[cfg(test)]
mod tests;
