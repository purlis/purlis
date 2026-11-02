// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill source an extension adds** (the SkillSource capability, ADR 0101 §1; #2777): one
//! that is not git, such as a registry or an artifact store, answered by an extension that
//! declares `skill-source`.
//!
//! purlis asks it what skills it has at one revision ([`browse`]), and for one skill's files at
//! that revision; the review is purlis's own, the one the git source's install shows
//! ([`super::review::reviews_of`]): every file, the summary first, and the skills it refers to
//! brought along. The extension answers inside its sandbox, and its answer is read failing closed
//! within one skill's limits ([`crate::extension::studio::files_of`]). It never writes the
//! project: it hands purlis files, and purlis alone copies what the person accepted.

use purlis_skills::tree::LeftOut;

use super::fetch::{Found, NotASkill, SourceFile};
use super::review::{SkillReview, reviews_of};
use crate::extension::studio::{self as wire, Ask};
use crate::skills::studio::Studio;

/// What an extension's source offers, at one revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Browse {
    /// The extension, by id.
    pub extension: String,
    /// What it names what it holds by, now.
    pub revision: String,
    pub skills: Vec<Found>,
}

/// **The review of installing `names` from an extension's source**, at the revision it answered.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Review {
    pub extension: String,
    pub revision: String,
    pub skills: Vec<SkillReview>,
}

/// An extension's source, asked at one revision.
struct Asked<'a> {
    studio: Studio<'a>,
    id: &'a str,
    offered: wire::Offered,
}

impl Asked<'_> {
    /// `id`'s source, asked what it offers now.
    fn of<'a>(studio: Studio<'a>, id: &'a str) -> Result<Asked<'a>, String> {
        let doc = studio.ask(id, &Ask::Skills)?;
        let offered = wire::offered_of(id, &doc)?;
        Ok(Asked {
            studio,
            id,
            offered,
        })
    }
}

impl super::SkillSource for Asked<'_> {
    fn shown(&self) -> String {
        format!("the skill source '{}'", self.id)
    }

    fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String> {
        Ok((
            self.offered
                .skills
                .iter()
                .map(|skill| Found {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    folder: skill.name.clone(),
                    dir: vec![skill.name.clone()],
                })
                .collect(),
            Vec::new(),
        ))
    }

    fn files(&self, skill: &Found) -> Result<(Vec<SourceFile>, Vec<LeftOut>), String> {
        let doc = self.studio.ask(
            self.id,
            &Ask::Files {
                skill: &skill.name,
                revision: &self.offered.revision,
            },
        )?;
        let files = wire::files_of(self.id, &self.offered.revision, &doc)?;
        let total: usize = files.iter().map(|file| file.text.len()).sum();
        if total as u64 > super::MOST_SKILL_BYTES {
            return Err(format!(
                "{} holds more than {} MiB, the most a skill purlis installs may",
                skill.name,
                super::MOST_SKILL_BYTES >> 20
            ));
        }
        Ok((
            files
                .into_iter()
                .map(|file| SourceFile {
                    parts: file.path.split('/').map(str::to_owned).collect(),
                    executable: file.executable,
                    bytes: file.text.into_bytes(),
                })
                .collect(),
            Vec::new(),
        ))
    }
}

/// **What the extension `id`'s source offers now.**
pub fn browse(studio: Studio<'_>, id: &str) -> Result<Browse, String> {
    let asked = Asked::of(studio, id)?;
    let (skills, _) = super::SkillSource::skills(&asked)?;
    Ok(Browse {
        extension: id.to_owned(),
        revision: asked.offered.revision.clone(),
        skills,
    })
}

/// **The review of installing `names` from the extension `id`'s source**, and every skill they
/// refer to that it holds and is not met already: the same review the git source's install
/// shows. No level is chosen for it yet, so only a name of purlis's own meets a reference (#4155).
pub fn review(studio: Studio<'_>, id: &str, names: &[String]) -> Result<Review, String> {
    let asked = Asked::of(studio, id)?;
    let skills = reviews_of(&asked, names, &crate::skills::name_reserved)?;
    Ok(Review {
        extension: id.to_owned(),
        revision: asked.offered.revision.clone(),
        skills,
    })
}
