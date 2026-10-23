// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill sources in the studio** (SK-10, #2773; ADR 0100): add a git repository of skills,
//! browse it, review an install summary first, install it pinned, and review each newer commit
//! as a diff before it is applied. Each verb is `purlis_core::skills::source`'s own, which
//! `purlis skill install | update` calls too. Behind the `skill-studio` flag until its finish line
//! (#2786).
//!
//! The window is handed what it shows, and hands back only what the review answered (the commit
//! and each skill's digest), so an install is of exactly the bytes the person read.

use serde::{Deserialize, Serialize};
use specta::Type;

use purlis_core::skills::review;
use purlis_core::skills::source::{self, Accepted, Source, VaultEntry};
use purlis_core::skills::tree;

use crate::skillchange::{SkillChanged, SkillGrant, SkillLevel, flag_on, heard};

/// A skill source as the studio shows and adds one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSource {
    pub url: String,
    pub branch: Option<String>,
    pub folder: Option<String>,
    /// The vault and the entry in it a private source signs in with; never the token.
    pub vault: Option<String>,
    pub key: Option<String>,
    /// Kept on this machine, not shared with the project.
    pub local: bool,
    /// As a person reads it.
    pub label: String,
}

impl SkillSource {
    fn core(&self) -> Source {
        Source {
            url: self.url.trim().to_owned(),
            branch: self.branch.clone().filter(|b| !b.trim().is_empty()),
            folder: self.folder.clone().filter(|f| !f.trim().is_empty()),
            sign_in: match (&self.vault, &self.key) {
                (Some(vault), Some(key)) if !vault.is_empty() && !key.is_empty() => {
                    Some(VaultEntry {
                        vault: vault.clone(),
                        key: key.clone(),
                        user: None,
                    })
                }
                _ => None,
            },
        }
    }

    fn of(source: Source, local: bool) -> SkillSource {
        SkillSource {
            label: source.to_string(),
            url: source.url,
            branch: source.branch,
            folder: source.folder,
            vault: source.sign_in.as_ref().map(|s| s.vault.clone()),
            key: source.sign_in.map(|s| s.key),
            local,
        }
    }
}

/// One skill a source offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct OfferedSkill {
    pub name: String,
    pub description: String,
    pub folder: String,
    /// The levels it is installed at from this source, as a person reads them.
    pub installed: Vec<String>,
}

/// What a source offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SourceBrowse {
    pub commit: String,
    pub skills: Vec<OfferedSkill>,
    /// Each folder that is not a whole skill, and why.
    pub not_skills: Vec<NotASkill>,
}

/// A folder of a source that is not a whole skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct NotASkill {
    pub folder: String,
    pub why: String,
}

/// A script the summary lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewScript {
    pub path: String,
    pub runs: String,
    /// Why it is marked, as a person reads it; empty for one that is not.
    pub risks: Vec<String>,
}

/// A network host the summary lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewHost {
    pub host: String,
    pub files: Vec<String>,
    pub risky: bool,
}

/// A `!` command, or a write outside the workspace: both always marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewLine {
    pub path: String,
    pub line: u32,
    /// The command, or where it writes.
    pub text: String,
}

/// What a skill does beyond its instructions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewSummary {
    pub scripts: Vec<ReviewScript>,
    pub hosts: Vec<ReviewHost>,
    pub commands: Vec<ReviewLine>,
    pub outside: Vec<ReviewLine>,
    pub risky: bool,
}

impl From<&review::Summary> for ReviewSummary {
    fn from(summary: &review::Summary) -> Self {
        let line = |path: &str, line: usize, text: &str| ReviewLine {
            path: path.to_owned(),
            line: u32::try_from(line).unwrap_or(u32::MAX),
            text: text.to_owned(),
        };
        ReviewSummary {
            scripts: summary
                .scripts
                .iter()
                .map(|one| ReviewScript {
                    path: one.path.clone(),
                    runs: one.runs.clone(),
                    risks: one.risks.iter().map(ToString::to_string).collect(),
                })
                .collect(),
            hosts: summary
                .hosts
                .iter()
                .map(|one| ReviewHost {
                    host: one.host.clone(),
                    files: one.files.clone(),
                    risky: one.risky,
                })
                .collect(),
            commands: summary
                .commands
                .iter()
                .map(|one| line(&one.path, one.line, &one.command))
                .collect(),
            outside: summary
                .outside
                .iter()
                .map(|one| line(&one.path, one.line, &one.target))
                .collect(),
            risky: summary.risky(),
        }
    }
}

/// One file the review shows, one click away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewFile {
    pub path: String,
    pub size: u32,
    pub executable: bool,
    pub text: Option<String>,
    pub risky: bool,
}

/// An entry of a skill's folder that is not installed, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewLeftOut {
    pub path: String,
    pub why: String,
}

/// One skill of an install review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewSkill {
    pub name: String,
    pub description: String,
    pub folder: String,
    pub brought_by: Option<String>,
    pub summary: ReviewSummary,
    pub files: Vec<ReviewFile>,
    pub left_out: Vec<ReviewLeftOut>,
    pub lint: Vec<String>,
    pub grade: Option<String>,
    pub digest: String,
}

fn left_shown(left: &[tree::LeftOut]) -> Vec<ReviewLeftOut> {
    left.iter()
        .map(|one| ReviewLeftOut {
            path: one.path.clone(),
            why: one.why.to_string(),
        })
        .collect()
}

impl From<&source::SkillReview> for ReviewSkill {
    fn from(skill: &source::SkillReview) -> Self {
        ReviewSkill {
            name: skill.name.clone(),
            description: skill.description.clone(),
            folder: skill.folder.clone(),
            brought_by: skill.brought_by.clone(),
            summary: (&skill.summary).into(),
            files: skill
                .files
                .iter()
                .map(|file| ReviewFile {
                    path: file.path.clone(),
                    size: u32::try_from(file.size).unwrap_or(u32::MAX),
                    executable: file.executable,
                    text: file.text.clone(),
                    risky: file.risky,
                })
                .collect(),
            left_out: left_shown(&skill.left_out),
            lint: skill.lint.clone(),
            grade: skill.grade.clone(),
            digest: skill.digest.clone(),
        }
    }
}

/// **An install review**, summary first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct InstallReview {
    pub commit: String,
    pub skills: Vec<ReviewSkill>,
}

/// One newer commit offered for an installed skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdateOffer {
    pub name: String,
    pub level: SkillLevel,
    pub label: String,
    pub url: String,
    pub pinned: String,
    pub newer: String,
    pub edited: bool,
}

/// An installed skill whose source could not be asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdateUnasked {
    pub name: String,
    pub label: String,
    pub why: String,
}

/// The updates there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdates {
    pub offers: Vec<SkillUpdateOffer>,
    pub unasked: Vec<SkillUpdateUnasked>,
}

/// One file's change in an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateFile {
    pub path: String,
    /// `added`, `removed` or `changed`.
    pub change: String,
    pub diff: String,
}

/// **An update's review**: the summary's changes, then each file's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateReview {
    pub name: String,
    pub pinned: String,
    pub newer: String,
    pub from_pin: bool,
    pub added: ReviewSummary,
    pub removed: ReviewSummary,
    pub files: Vec<UpdateFile>,
    pub edited: Vec<String>,
    pub skill: ReviewSkill,
}

/// What the window hands back to install or update: the commit and each skill's digest, as the
/// review answered them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Reviewed {
    pub commit: String,
    pub skills: Vec<ReviewedSkill>,
}

/// One skill of [`Reviewed`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReviewedSkill {
    pub name: String,
    pub digest: String,
}

impl From<&Reviewed> for Accepted {
    fn from(reviewed: &Reviewed) -> Accepted {
        Accepted {
            commit: reviewed.commit.clone(),
            skills: reviewed
                .skills
                .iter()
                .map(|one| (one.name.clone(), one.digest.clone()))
                .collect(),
        }
    }
}

/// The project's skill sources.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_sources(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<SkillSource>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill sources", move || {
        flag_on()?;
        Ok(source::sources(held.root())
            .into_iter()
            .map(|one| SkillSource::of(one.source, one.local))
            .collect())
    })
    .await
}

/// **Adds a skill source**, shared with the project or kept on this machine.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_add(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: SkillSource,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("adding the skill source", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            source::add(cx, &source.core(), source.local)
        })
    })
    .await
}

/// **Takes a skill source out** of the project's. Skills installed from it stay.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_remove(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: SkillSource,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("taking the skill source out", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            source::remove(cx, &source.url, source.branch.as_deref())
        })
    })
    .await
}

/// **Browses a skill source** at its newest commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_browse(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: SkillSource,
) -> Result<SourceBrowse, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("browsing the skill source", move || {
        flag_on()?;
        let browse = source::browse(held.root(), &source.core())?;
        Ok(SourceBrowse {
            commit: browse.commit,
            skills: browse
                .skills
                .into_iter()
                .map(|one| OfferedSkill {
                    name: one.skill.name,
                    description: one.skill.description,
                    folder: one.skill.folder,
                    installed: one.installed.iter().map(ToString::to_string).collect(),
                })
                .collect(),
            not_skills: browse
                .not_skills
                .into_iter()
                .map(|(folder, why)| NotASkill { folder, why })
                .collect(),
        })
    })
    .await
}

/// **The install review** of `names` from a source, and the skills they refer to.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_install_review(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: SkillSource,
    names: Vec<String>,
) -> Result<InstallReview, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reviewing the install", move || {
        flag_on()?;
        let review = source::review(held.root(), &source.core(), &names)?;
        Ok(InstallReview {
            commit: review.commit,
            skills: review.skills.iter().map(Into::into).collect(),
        })
    })
    .await
}

/// **Installs what was reviewed** at `level`, with `grant` where it is a project skill.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_install(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: SkillSource,
    level: SkillLevel,
    reviewed: Reviewed,
    grant: Option<SkillGrant>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("installing the skill", move || {
        flag_on()?;
        let grant: Option<purlis_core::skills::Grant> = grant.map(Into::into);
        heard(held.root(), |cx| {
            source::install(
                cx,
                &source.core(),
                &(&level).into(),
                &(&reviewed).into(),
                grant.as_ref(),
            )
        })
    })
    .await
}

/// **The updates there are**: each source asked for its newest commit, nothing fetched.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_updates(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<SkillUpdates, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("asking the skill sources", move || {
        flag_on()?;
        let (offers, unasked) = source::updates(held.root());
        Ok(SkillUpdates {
            offers: offers
                .into_iter()
                .map(|one| SkillUpdateOffer {
                    label: one.level.to_string(),
                    level: (&one.level).into(),
                    name: one.name,
                    url: one.url,
                    pinned: one.pinned,
                    newer: one.newer,
                    edited: one.edited,
                })
                .collect(),
            unasked: unasked
                .into_iter()
                .map(|one| SkillUpdateUnasked {
                    label: one.level.to_string(),
                    name: one.name,
                    why: one.why,
                })
                .collect(),
        })
    })
    .await
}

/// **An update's review**: the installed skill `name` at `level` against its source's newest
/// commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_update_review(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<UpdateReview, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reviewing the update", move || {
        flag_on()?;
        let update = source::update_review(held.root(), &(&level).into(), &name)?;
        let changed = |change: source::Changed| match change {
            source::Changed::Added => "added",
            source::Changed::Removed => "removed",
            source::Changed::Changed => "changed",
        };
        Ok(UpdateReview {
            name: update.name.clone(),
            pinned: update.pinned.clone(),
            newer: update.newer.clone(),
            from_pin: update.from_pin,
            added: (&update.changes.added).into(),
            removed: (&update.changes.removed).into(),
            files: update
                .files
                .iter()
                .map(|file| UpdateFile {
                    path: file.path.clone(),
                    change: changed(file.change).to_owned(),
                    diff: file.diff.clone(),
                })
                .collect(),
            edited: update.edited.clone(),
            skill: (&update.review).into(),
        })
    })
    .await
}

/// **Accepts an update** that was reviewed: the pin moves, as one commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_update(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    reviewed: Reviewed,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("updating the skill", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            source::update(cx, &(&level).into(), &name, &(&reviewed).into())
        })
    })
    .await
}
