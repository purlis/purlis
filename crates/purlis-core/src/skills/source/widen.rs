// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a project install's grant asks of the project skills it relies on** (#4172, ADR 0100
//! as amended 2026-10-31).
//!
//! A skill installed at the project level with a grant reaches the chats at the root and those
//! in each workspace the grant reaches. A skill it refers to that the project holds already is left as
//! it is: a second copy cannot be made at the same level. That skill reaches those chats only
//! where its own grant does, and the workspace has not turned it off, by the rule
//! [`purlis_skills::skill_set`] hands a chat its project skills by. So the install widens its
//! grant to each workspace the install's grant reaches and its own does not, and the review
//! says so first. A workspace that turned it off keeps it off: no grant turns a skill back on.

use std::path::{Path, PathBuf};

use crate::skills::verbs::{Level, Whose};
use crate::skills::{Grant, grant_of};

/// **A project skill an install refers to whose grant does not reach every workspace the
/// install's grant does**, or which one of those workspaces turned off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Widening {
    /// Its name.
    pub name: String,
    /// Its folder, in the project's shared skills.
    pub folder: PathBuf,
    /// Its grant now.
    pub granted: Grant,
    /// The grant the install widens it to, `None` where it reaches every workspace already.
    pub widened: Option<Grant>,
    /// The workspaces the install's grant reaches that turned it off, whose chats get the
    /// installed skill without it.
    pub turned_off: Vec<String>,
}

/// **What installing at `level` with `grant` asks of the project skills it refers to**: each of
/// `already`, the references the install leaves as they are ([`super::SkillReview::already`]),
/// that the project's
/// shared skills hold, whose grant does not reach a workspace `grant` reaches, or which such a
/// workspace turned off. None where the install is granted nothing beyond the root: anywhere but
/// the project's shared skills, with no grant, or with the root's.
pub fn widenings<'a>(
    plane: &Path,
    level: &Level,
    grant: Option<&Grant>,
    already: impl IntoIterator<Item = &'a str>,
) -> Vec<Widening> {
    let Some(grant) = granted_beyond_the_root(level, grant) else {
        return Vec::new();
    };
    let reached: Vec<String> = match grant {
        Grant::Only(names) => names.clone(),
        _ => crate::workspaces::Plane::open(plane)
            .workspaces()
            .unwrap_or_default(),
    };
    let off: Vec<(String, Vec<String>)> = reached
        .iter()
        .map(|ws| (ws.clone(), crate::skills::turned_off(plane, ws)))
        .collect();
    let mut out: Vec<Widening> = Vec::new();
    for name in already {
        if out.iter().any(|one| one.name == name) {
            continue;
        }
        let Some(folder) = held(plane, name) else {
            continue;
        };
        // One whose grant cannot be read reaches no workspace, as a chat reads it.
        let granted = grant_of(&folder).unwrap_or(Grant::Root);
        let widening = Widening {
            name: name.to_owned(),
            widened: widened(&granted, grant),
            turned_off: off
                .iter()
                .filter(|(_, names)| names.iter().any(|one| one == name))
                .map(|(ws, _)| ws.clone())
                .collect(),
            granted,
            folder,
        };
        if widening.widened.is_some() || !widening.turned_off.is_empty() {
            out.push(widening);
        }
    }
    out
}

/// The grant an install at `level` writes, where it reaches past the root: as
/// [`super::install`] grants what it installs.
pub(super) fn granted_beyond_the_root<'a>(
    level: &Level,
    grant: Option<&'a Grant>,
) -> Option<&'a Grant> {
    grant.filter(|grant| level.at == Whose::Project && !level.local && **grant != Grant::Root)
}

/// The folder of the project's shared skill `name`, which a shared install relies on.
fn held(plane: &Path, name: &str) -> Option<PathBuf> {
    crate::skills::named_at(plane, &Whose::Project, false, name)
}

/// `granted` widened to reach every workspace `install` reaches, or `None` where it does already.
fn widened(granted: &Grant, install: &Grant) -> Option<Grant> {
    match (granted, install) {
        (Grant::All, _) | (_, Grant::Root) => None,
        (_, Grant::All) => Some(Grant::All),
        (_, Grant::Only(names)) => {
            let mut missing: Vec<String> = Vec::new();
            for ws in names {
                if !granted.reaches(ws) && !missing.contains(ws) {
                    missing.push(ws.clone());
                }
            }
            if missing.is_empty() {
                return None;
            }
            let mut all = match granted {
                Grant::Only(had) => had.clone(),
                _ => Vec::new(),
            };
            all.extend(missing);
            Some(Grant::Only(all))
        }
    }
}

/// What `to` reaches that `from` does not, as a person is told it: every workspace, or the
/// workspaces' names.
pub fn added(from: &Grant, to: &Grant) -> String {
    match to {
        Grant::Only(names) => names
            .iter()
            .filter(|ws| !from.reaches(ws))
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
        Grant::All => "every workspace".to_owned(),
        Grant::Root => "the root".to_owned(),
    }
}

impl Widening {
    /// What each workspace that turned it off is told of a skill `installed` that refers to it.
    pub fn turned_off_said(&self, installed: &str) -> Vec<String> {
        self.turned_off
            .iter()
            .map(|ws| {
                format!(
                    "{ws} turned {} off, so its chats get {installed} without it",
                    self.name
                )
            })
            .collect()
    }
}
