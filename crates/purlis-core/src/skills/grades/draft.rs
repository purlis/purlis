// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A draft's grade** (#2780; spec #2762 decision 43; ADR 0105 as amended 2026-11-02): what a
//! chat drafted, graded against the version it would replace before the person accepts it.
//!
//! - **What is graded is what Accept would write**: the project's files of the skill with the
//!   draft's over them, read once, without `evals/` ([`planned`]).
//! - **The current version is the `previous` arm**, read from the skill's folder, as it may hold
//!   what git does not; a new skill has none, and is graded against no skill alone.
//! - **Its tests are the draft's overlay**: a fixture the draft writes is the one that runs, and
//!   one it leaves is the folder's. A draft that changes only fixtures gets no grade
//!   ([`ONLY_EVALS`]), as both arms would run the same skill.
//! - **Its row is this machine's alone** (`Graded::draft`), marked with the draft and a digest of
//!   what it wrote, and stays out of the Grades tab, the estimate and `purlis skill grade`. Only
//!   its id, or [`of`], reads it. Accept never asks for it.

use std::path::Path;

use purlis_skills::grade;

use super::super::verbs::drafts::{Draft, Snapshot};
use super::super::verbs::{Level, find, rel};
use super::{
    Graded, Planned, Previous, SkillCopy, TriggerTests, Wanted, Which, copy_of, counted,
    planned_from, read_where,
};
use crate::skilluse::Content;

/// What a draft that changes only fixtures is told.
pub const ONLY_EVALS: &str = "This draft changes only the skill's tests, so there is nothing to grade against the current \
     version.";

/// **Which draft a grade graded**: what tells it from the skill's own grades.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mark {
    /// The draft's mint ([`Draft::mint`]): the next draft of the skill has another.
    pub id: String,
    /// What it wrote when it was graded ([`Draft::digest`]).
    pub digest: String,
}

impl Mark {
    /// The mark of `draft` as it is now.
    pub fn of(draft: &Draft) -> Mark {
        Mark {
            id: draft.mint.clone(),
            digest: draft.digest(),
        }
    }

    /// Whether a row read back marks a draft as purlis does: a plain id and a whole digest.
    pub(super) fn sound(&self) -> bool {
        super::plain(&self.id)
            && self.digest.len() == 64
            && self
                .digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

/// The fixture file at `path` as `draft` writes it.
fn drafted<'a>(draft: &'a Draft, path: &str) -> Option<&'a str> {
    draft
        .files
        .iter()
        .find(|file| file.path == path)
        .map(|file| file.after.as_str())
}

/// **The fixtures a grade of `draft` runs**: the folder's, with those the draft writes over them.
fn fixtures_of(root: &Path, draft: &Draft) -> Result<grade::Fixtures, String> {
    let folder = if draft.new {
        format!("{}/{}", draft.level.rel().unwrap_or_default(), draft.name)
    } else {
        rel(root, &find(root, &draft.level, &draft.name)?)
    };
    Ok(grade::fixtures_over(
        grade::fixtures_of(root, Path::new(&folder), &draft.name),
        drafted(draft, &grade::triggers_path()),
        drafted(draft, &grade::outcomes_path()),
        &draft.name,
    ))
}

/// **The copy a grade of `draft` runs, read once**: the project's files with the draft's over
/// them as the skill, and the project's as the version it is compared with, where there is one.
/// `inputs` are read from the draft where it writes them, else from the folder.
fn copy_of_draft(root: &Path, draft: &Draft, inputs: &[String]) -> Result<SkillCopy, String> {
    let current = if draft.new {
        None
    } else {
        Some(copy_of(root, &draft.level, &draft.name, inputs)?)
    };
    let mut files = current
        .as_ref()
        .map(|copy| copy.snapshot.files.clone())
        .unwrap_or_default();
    for file in &draft.files {
        if file.path.split('/').next() != Some(grade::EVALS_DIR) {
            files.insert(file.path.clone(), file.after.clone());
        }
    }
    let mut read = current
        .as_ref()
        .map(|copy| copy.inputs.clone())
        .unwrap_or_default();
    for path in inputs {
        if let Some(text) = drafted(draft, path) {
            read.insert(path.clone(), text.to_owned());
        }
    }
    Ok(SkillCopy {
        snapshot: Snapshot {
            name: draft.name.clone(),
            files,
        },
        content: Content::Uncommitted,
        commit: None,
        previous: current.map(|copy| Previous {
            commit: copy.commit.unwrap_or_default(),
            tree: copy.content.word().to_owned(),
            subject: String::new(),
            snapshot: copy.snapshot,
        }),
        previous_refused: None,
        inputs: read,
    })
}

/// **The plan of grading `draft`** in the project at `root`, its chats on `profile` where it is
/// known, with the outcome evals `ticked` on: see the module. Refused for a draft that changes
/// only fixtures, and for a new skill with no instructions yet.
pub fn planned(
    root: &Path,
    draft: &Draft,
    profile: Option<&str>,
    ticked: &[u64],
) -> Result<Planned, String> {
    if draft.only_evals() {
        return Err(ONLY_EVALS.to_owned());
    }
    if draft.new && draft.instructions().is_none() {
        return Err(format!(
            "The draft of {} has no instructions yet, so a grade has nothing to run.",
            draft.name
        ));
    }
    planned_from(
        root,
        &draft.level,
        &draft.name,
        profile,
        ticked,
        fixtures_of(root, draft)?,
        |inputs| copy_of_draft(root, draft, inputs),
    )
}

/// **What a draft's page says of its grade.**
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    /// It changes only fixtures: no grade.
    pub only_evals: bool,
    /// What a grade of it would run, counted, or why not.
    pub tests: Result<TriggerTests, String>,
    /// Its newest grade, where it has one.
    pub latest: Option<Box<Graded>>,
    /// That grade graded what it wrote before it last changed.
    pub changed: bool,
}

/// **What is known of `draft`'s grade** in the project at `root`.
pub fn of(root: &Path, draft: &Draft) -> Said {
    let latest = latest_of(root, &draft.level, &draft.name, &draft.mint);
    let digest = draft.digest();
    Said {
        only_evals: draft.only_evals(),
        tests: fixtures_of(root, draft).map(|fixtures| counted(&fixtures)),
        changed: latest
            .as_ref()
            .and_then(|row| row.draft.as_ref())
            .is_some_and(|mark| mark.digest != digest),
        latest: latest.map(Box::new),
    }
}

/// The newest grade of the draft minted `mint` of the copy of `name` at `level`.
fn latest_of(root: &Path, level: &Level, name: &str, mint: &str) -> Option<Graded> {
    let wanted = Wanted {
        name,
        level: Some(level.rel()),
        trees: None,
        until: None,
        grades: &[],
    };
    read_where(
        root,
        std::slice::from_ref(&wanted),
        Which::OfDraft(mint),
        chrono::Utc::now(),
    )
    .grades
    .into_iter()
    .next()
}

#[cfg(test)]
mod tests;
