// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the Skill studio offers when it opens** (SK-22, #2785; spec #2762 decision 42; ADR 0100
//! §1 as amended 2026-10-24): the studio lists every skill it finds, and beside them it offers
//! what a person can start from. One copy of that choice, which the window draws.
//!
//! - **The three starts** — make one with a chat, browse skill sources, start blank — while the
//!   project has no skill of its own at any level (its own, a workspace's or a persona's). The
//!   built-in skills and the skills a chat program finds itself are listed still, but they are
//!   not the project's, so they do not make it a studio with something in it.
//! - **The offered sources** ([`offered_here`]): the first-party catalogue and the well-known
//!   skill sources ([`offered`], the committed list `first_open/offered.toml`), less every one the
//!   project already browses. Taking one up is SK-10's Add source, filled in and asked first, and
//!   an install from it is still its own review: an offer never adds or installs anything. They
//!   are read apart from [`first_open`], with the project's sources, because reading those asks
//!   git whether this machine's settings are ignored: the studio's own read stays on the disk.
//! - **The move SK-21 left for here**: a project whose skills are still in the folder only some
//!   chat programs read (`.claude/skills`) is offered `purlis doctor --fix project-skills`.

use std::path::Path;

use serde::Deserialize;

use super::source::{Remote, Source, sources};
use super::{Origin, StudioRow};
use crate::doctor::fix::FixId;

/// The committed list of offered sources.
const OFFERED: &str = include_str!("first_open/offered.toml");

/// The most entries of the old skills folder read to count its skills.
const MOST_ENTRIES: usize = 1_000;

/// Whose list a source is offered from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Offering {
    /// purlis's own first-party catalogue (PE-44).
    Catalogue,
    /// A collection someone else keeps that people use widely.
    WellKnown,
}

/// **A source the studio offers with one click**: the source, its name, and the one line that
/// says why it is on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedSource {
    pub offering: Offering,
    pub name: String,
    pub source: Source,
    pub why: String,
}

/// One table of the list, every key it may hold and no other.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    offering: Offering,
    name: String,
    url: String,
    #[serde(default)]
    branch: Option<String>,
    folder: String,
    why: String,
}

/// The list's file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    #[serde(default)]
    source: Vec<Written>,
}

/// `text`, read as the list: every source, or the sentence that refuses them all. A source must
/// be one a team's committed file could name: an `https://` URL with a plain folder.
fn read(text: &str) -> Result<Vec<OfferedSource>, String> {
    let list: List = toml::from_str(text).map_err(|e| format!("the list is not read: {e}"))?;
    list.source
        .into_iter()
        .map(|one| {
            let source = Source {
                remote: Remote {
                    url: one.url,
                    branch: one.branch,
                },
                folder: Some(one.folder),
                sign_in: None,
            };
            source.check()?;
            source.remote.check(true)?;
            if one.name.trim().is_empty() || one.why.trim().is_empty() {
                return Err(format!("{source} is offered with no name or no reason"));
            }
            Ok(OfferedSource {
                offering: one.offering,
                name: one.name,
                source,
                why: one.why,
            })
        })
        .collect()
}

/// **Every source the studio offers**, the first-party catalogue's first, in the list's order.
/// None where the list cannot all be read: the build's own test reads it first.
pub(crate) fn offered() -> Vec<OfferedSource> {
    let mut all = read(OFFERED).unwrap_or_else(|why| {
        tracing::warn!(target: "purlis::skills", "no skill source is offered: {why}");
        Vec::new()
    });
    all.sort_by_key(|one| one.offering != Offering::Catalogue);
    all
}

/// **What the studio offers when it opens.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstOpen {
    /// Whether the project has no skill of its own, so the three starts are offered.
    pub starts: bool,
    /// Where the project's skills are still in the old folder: how many, and the fix that moves
    /// them.
    pub unmoved: Option<Unmoved>,
}

/// The project's skills still in the folder only some chat programs read, and the doctor's fix
/// that moves them into the studio's (SK-21, #2784).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unmoved {
    pub skills: usize,
    pub fix: FixId,
}

/// **What the studio of the project at `plane` offers**, with `rows` the skills it lists. It
/// reads the disk alone.
pub fn first_open(plane: &Path, rows: &[StudioRow]) -> FirstOpen {
    let owned = rows.iter().any(|row| {
        matches!(
            row.origin,
            Origin::Project | Origin::Workspace | Origin::Persona
        )
    });
    FirstOpen {
        starts: !owned,
        unmoved: unmoved(plane),
    }
}

/// **The sources the studio offers the project at `plane`**: [`offered`], less every source it
/// browses already. None where the project's sources cannot be read: the studio's sources section
/// says why, and an add would be refused.
pub fn offered_here(plane: &Path) -> Vec<OfferedSource> {
    let Ok(listed) = sources(plane) else {
        return Vec::new();
    };
    offered()
        .into_iter()
        .filter(|one| {
            !listed
                .iter()
                .any(|listed| same_place(&listed.source.remote, &one.source.remote))
        })
        .collect()
}

/// Whether `a` and `b` are one repository's same branch, however the URL ends.
fn same_place(a: &Remote, b: &Remote) -> bool {
    let plain = |url: &str| {
        let url = url.trim_end_matches('/');
        url.strip_suffix(".git").unwrap_or(url).to_ascii_lowercase()
    };
    plain(&a.url) == plain(&b.url) && a.branch == b.branch
}

/// The project's skills still in the old folder, where it has not moved them: each folder there
/// that holds a `SKILL.md`, itself and not through a link, counting at most [`MOST_ENTRIES`].
fn unmoved(plane: &Path) -> Option<Unmoved> {
    if crate::compat::requires(plane, super::PROJECT_SKILLS_FEATURE) {
        return None;
    }
    let old = plane.join(crate::guest::RETIRED_MIRROR);
    if !crate::leftovers::is_plain_dir(&old) {
        return None;
    }
    let skills = std::fs::read_dir(&old)
        .ok()?
        .take(MOST_ENTRIES)
        .filter_map(Result::ok)
        .filter(|entry| {
            let folder = entry.path();
            crate::leftovers::is_plain_dir(&folder)
                && std::fs::symlink_metadata(folder.join(purlis_skills::FILE))
                    .is_ok_and(|meta| meta.is_file())
        })
        .count();
    (skills > 0).then_some(Unmoved {
        skills,
        fix: FixId::ProjectSkills,
    })
}

#[cfg(test)]
mod tests;
