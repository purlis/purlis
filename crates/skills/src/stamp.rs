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
        // changes it.
        if let Some(index) = set.index() {
            rows.push(Stamped {
                name: crate::INDEX.to_owned(),
                path: PathBuf::new(),
                description: index.text,
                body: None,
            });
        }
        rows.sort();
        Stamp(rows)
    }
}

#[cfg(test)]
mod tests;
