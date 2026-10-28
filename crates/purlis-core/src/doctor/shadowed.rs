// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `shadowed docs`: a project keeping its own copy of something purlis ships (#994).
//!
//! The failure that produced Python's check: a project carried its own `setup` skill telling
//! people to authenticate over SSH, months after the rule became HTTPS tokens only and the
//! guard began refusing exactly that. Every copy looked wired, and nothing compared any of them
//! to the program they described.
//!
//! **What purlis ships**: its documentation pages ([`crate::docsrc::topics`], compiled into
//! this binary and read with `purlis docs show <topic>`), and the skills its Claude Code plugin
//! carries ([`SHIPPED_SKILLS`]). A shadow is a project's `docs/<topic>.md` for one of those
//! topics, or a `.claude/skills/<name>/SKILL.md` for one of those skills.
//!
//! **It reports and never resolves.** A copy is not wrong by being a copy; it is unwatched, and
//! that is the whole finding. A project may override purlis's guidance on purpose, which is
//! the operator's call, so the hint never says delete. Reads directory listings only.

use std::path::Path;

use super::{Doctor, Row};
use crate::counts::plural;

const NAME: &str = "shadowed docs";

/// The skills purlis's Claude Code plugin ships, by directory name: the bundle's
/// `plugin/skills/<name>/SKILL.md` (held to that directory by a test).
pub(crate) const SHIPPED_SKILLS: [&str; 11] = crate::skills::SHIPPED;

/// What a project at `root` keeps of purlis's own: the shipped skills it has a copy of, and
/// the documentation topics, each sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Shadowed {
    pub skills: Vec<String>,
    pub docs: Vec<String>,
}

/// [`Shadowed`] for the project at `root`, or nothing for purlis's own repository, whose
/// `docs/` and plugin skills ARE the pages and skills it ships.
pub(crate) fn shadowed(root: &Path) -> std::io::Result<Shadowed> {
    if is_purlis_checkout(root) {
        return Ok(Shadowed::default());
    }
    let mut skills: Vec<String> = Vec::new();
    match std::fs::read_dir(root.join(".claude/skills")) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if SHIPPED_SKILLS.contains(&name.as_str())
                    && entry.path().join("SKILL.md").is_file()
                {
                    skills.push(name);
                }
            }
        }
        Err(e) if crate::memstore::is_absent(&e) => {}
        Err(e) => return Err(e),
    }
    let topics = crate::docsrc::topics();
    let mut docs: Vec<String> = Vec::new();
    match std::fs::read_dir(root.join("docs")) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(topic) = name.strip_suffix(".md")
                    && topics.contains(&topic)
                    && entry.path().is_file()
                {
                    docs.push(topic.to_owned());
                }
            }
        }
        Err(e) if crate::memstore::is_absent(&e) => {}
        Err(e) => return Err(e),
    }
    skills.sort();
    docs.sort();
    Ok(Shadowed { skills, docs })
}

/// Whether `root` is purlis's own repository, asked of a file only it has.
fn is_purlis_checkout(root: &Path) -> bool {
    root.join("crates/purlis-core/build.rs").is_file()
}

pub(super) fn shadowed_docs(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(NAME, "no control plane found");
    }
    match shadowed(&d.root) {
        Ok(hit) => said(&hit),
        Err(e) => Row::not_checked(NAME, super::first_line(&e.to_string())),
    }
}

fn said(hit: &Shadowed) -> Row {
    let mut parts: Vec<String> = Vec::new();
    if !hit.skills.is_empty() {
        parts.push(format!(
            "{} {}",
            plural(hit.skills.len(), "skill", "skills"),
            hit.skills.join(", ")
        ));
    }
    if !hit.docs.is_empty() {
        let pages: Vec<String> = hit.docs.iter().map(|d| format!("docs/{d}.md")).collect();
        parts.push(pages.join(", "));
    }
    if parts.is_empty() {
        return Row::ok(NAME, "none — purlis's own knowledge is not duplicated here");
    }
    Row::warn(
        NAME,
        format!("{} duplicate what purlis ships", parts.join(" · ")),
        "A local copy is read instead of purlis's and is compared with nothing, so it drifts \
         both ways unwatched: behind on what purlis learned since, ahead with what purlis never \
         received. Read the shipped one with `purlis docs show <topic>`, and keep yours only \
         where it says something purlis's does not.",
    )
}

#[cfg(test)]
mod tests;
