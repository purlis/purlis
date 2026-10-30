// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill sources in the studio** (SK-10, #2773; ADR 0100): add a git repository of skills,
//! browse it, review an install summary first, install it pinned, confirm a source the committed
//! lockfile names before it is contacted, and review each newer commit as a diff before it is
//! applied. Each verb is `purlis_core::skills::source`'s own, which `purlis skill install |
//! update` calls too. Behind the `skill-studio` flag until its finish line (#2786).
//!
//! The window is handed what it shows, the core's own words and marks included (what is risky,
//! what is empty, each commit's short name, what accepting says), every field from a source
//! escaped by the core, and hands back only what the review answered as accepted, so an install
//! is of exactly the bytes the person read.

use serde::{Deserialize, Serialize};
use specta::Type;

use purlis_core::settings::Which;
use purlis_core::skills::review as summary;
use purlis_core::skills::source::{
    self, Accepted, AcceptedSkill, Commit, Remote, Source, VaultEntry,
};
use purlis_core::skills::tree;

use crate::skillchange::{SkillChanged, SkillGrant, SkillLevel, flag_on, heard};

/// A source's text as the window shows it: every invisible character escaped.
fn esc(text: &str) -> String {
    summary::escaped(text)
}

/// Which settings a source is kept in: the project's, shared with the team, or this machine's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKept {
    Shared,
    Local,
}

impl From<SourceKept> for Which {
    fn from(kept: SourceKept) -> Which {
        match kept {
            SourceKept::Shared => Which::Shared,
            SourceKept::Local => Which::Local,
        }
    }
}

impl From<Which> for SourceKept {
    fn from(which: Which) -> SourceKept {
        match which {
            Which::Shared => SourceKept::Shared,
            Which::Local => SourceKept::Local,
        }
    }
}

/// A repository and the branch read from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SourceRemote {
    pub url: String,
    pub branch: Option<String>,
    /// As a person reads it.
    pub label: String,
}

impl From<&Remote> for SourceRemote {
    fn from(remote: &Remote) -> Self {
        SourceRemote {
            url: remote.url.clone(),
            branch: remote.branch.clone(),
            label: esc(&remote.to_string()),
        }
    }
}

impl SourceRemote {
    fn core(&self) -> Remote {
        Remote {
            url: self.url.trim().to_owned(),
            branch: self.branch.clone().filter(|b| !b.trim().is_empty()),
        }
    }
}

/// A skill source as the studio shows and adds one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSource {
    pub remote: SourceRemote,
    pub folder: Option<String>,
    /// The vault and the entry in it a private source signs in with; never the token.
    pub vault: Option<String>,
    pub key: Option<String>,
    pub kept: SourceKept,
    /// As a person reads it.
    pub label: String,
}

impl SkillSource {
    fn core(&self) -> Source {
        Source {
            remote: self.remote.core(),
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

    fn of(source: Source, which: Which) -> SkillSource {
        SkillSource {
            label: esc(&source.to_string()),
            remote: (&source.remote).into(),
            folder: source.folder,
            vault: source.sign_in.as_ref().map(|s| s.vault.clone()),
            key: source.sign_in.map(|s| s.key),
            kept: which.into(),
        }
    }
}

/// A commit, and its short name as the core gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SourceCommit {
    pub id: String,
    pub short: String,
}

impl From<&Commit> for SourceCommit {
    fn from(commit: &Commit) -> Self {
        SourceCommit {
            id: commit.as_str().to_owned(),
            short: commit.short().to_owned(),
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
    /// What its files do beyond its instructions, a few words each, as the core marks them;
    /// `None` where they were not read to browse, and its review reads them.
    pub marks: Option<Vec<String>>,
}

/// A folder of a source that is not a skill purlis offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct NotASkill {
    pub folder: String,
    pub why: String,
}

/// What a source offers.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SourceBrowse {
    pub commit: SourceCommit,
    pub skills: Vec<OfferedSkill>,
    pub not_skills: Vec<NotASkill>,
    /// When it was read, in seconds since the epoch (#3840): what its page's lead says.
    pub checked: f64,
}

/// Why an item of the summary is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRisk {
    Network,
    RunsWhatItFetches,
    Elevates,
    Removes,
    Hidden,
    NotText,
}

impl From<summary::Risk> for ReviewRisk {
    fn from(risk: summary::Risk) -> Self {
        match risk {
            summary::Risk::Network => Self::Network,
            summary::Risk::RunsWhatItFetches => Self::RunsWhatItFetches,
            summary::Risk::Elevates => Self::Elevates,
            summary::Risk::Removes => Self::Removes,
            summary::Risk::Hidden => Self::Hidden,
            summary::Risk::NotText => Self::NotText,
        }
    }
}

/// A risk, and the core's words for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct RiskMark {
    pub kind: ReviewRisk,
    pub said: String,
}

/// A script the summary lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewScript {
    pub path: String,
    pub runs: String,
    /// Why it is marked; empty for one that is not.
    pub risks: Vec<RiskMark>,
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

/// The kind of a character no reader sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewInvisible {
    Tag,
    ZeroWidth,
    Blank,
    Bidi,
    Format,
}

impl From<summary::Invisible> for ReviewInvisible {
    fn from(kind: summary::Invisible) -> Self {
        match kind {
            summary::Invisible::Tag => Self::Tag,
            summary::Invisible::ZeroWidth => Self::ZeroWidth,
            summary::Invisible::Blank => Self::Blank,
            summary::Invisible::Bidi => Self::Bidi,
            summary::Invisible::Format => Self::Format,
        }
    }
}

/// A kind of invisible character, and the core's words for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct InvisibleMark {
    pub kind: ReviewInvisible,
    pub said: String,
}

/// A line holding characters no reader sees: always marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewUnseen {
    pub path: String,
    pub line: u32,
    pub kinds: Vec<InvisibleMark>,
}

/// What a skill does beyond its instructions, with the core's own marks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewSummary {
    pub scripts: Vec<ReviewScript>,
    pub hosts: Vec<ReviewHost>,
    pub commands: Vec<ReviewLine>,
    pub outside: Vec<ReviewLine>,
    pub unseen: Vec<ReviewUnseen>,
    /// Each file that is not text, so not shown.
    pub not_text: Vec<String>,
    /// How many of its items are marked (`Summary::marked`).
    pub marked: u32,
    /// Whether it lists nothing at all (`Summary::is_empty`).
    pub empty: bool,
}

fn line_of(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}

impl From<&summary::Summary> for ReviewSummary {
    fn from(said: &summary::Summary) -> Self {
        ReviewSummary {
            scripts: said
                .scripts
                .iter()
                .map(|one| ReviewScript {
                    path: esc(&one.path),
                    runs: esc(&one.runs),
                    risks: one
                        .risks
                        .iter()
                        .map(|risk| RiskMark {
                            kind: (*risk).into(),
                            said: risk.to_string(),
                        })
                        .collect(),
                })
                .collect(),
            hosts: said
                .hosts
                .iter()
                .map(|one| ReviewHost {
                    host: esc(&one.host),
                    files: one.files.iter().map(|f| esc(f)).collect(),
                    risky: one.risky,
                })
                .collect(),
            commands: said
                .commands
                .iter()
                .map(|one| ReviewLine {
                    path: esc(&one.path),
                    line: line_of(one.line),
                    text: esc(&one.command),
                })
                .collect(),
            outside: said
                .outside
                .iter()
                .map(|one| ReviewLine {
                    path: esc(&one.path),
                    line: line_of(one.line),
                    text: esc(&one.target),
                })
                .collect(),
            unseen: said
                .unseen
                .iter()
                .map(|one| ReviewUnseen {
                    path: esc(&one.path),
                    line: line_of(one.line),
                    kinds: one
                        .kinds
                        .iter()
                        .map(|kind| InvisibleMark {
                            kind: (*kind).into(),
                            said: kind.to_string(),
                        })
                        .collect(),
                })
                .collect(),
            not_text: said.not_text.iter().map(|f| esc(f)).collect(),
            marked: u32::try_from(said.marked()).unwrap_or(u32::MAX),
            empty: said.is_empty(),
        }
    }
}

/// One file the review shows, one click away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewFile {
    pub path: String,
    pub size: u32,
    pub executable: bool,
    /// Its text with every invisible character escaped; `None` for a file that is not text.
    pub text: Option<String>,
    pub risky: bool,
}

/// Why an entry of a skill's folder is not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum LeftOutWhy {
    Link,
    Submodule,
    Name,
    Case,
    TooLarge,
    Other,
}

impl From<tree::Why> for LeftOutWhy {
    fn from(why: tree::Why) -> Self {
        match why {
            tree::Why::Link => Self::Link,
            tree::Why::Submodule => Self::Submodule,
            tree::Why::Name => Self::Name,
            tree::Why::Case => Self::Case,
            tree::Why::TooLarge => Self::TooLarge,
            tree::Why::Other => Self::Other,
        }
    }
}

/// An entry of a skill's folder that is not installed, why, and the core's words for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ReviewLeftOut {
    pub path: String,
    pub why: LeftOutWhy,
    pub said: String,
}

/// One skill of a review.
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
}

impl From<&source::SkillReview> for ReviewSkill {
    fn from(skill: &source::SkillReview) -> Self {
        ReviewSkill {
            name: esc(&skill.name),
            description: esc(&skill.description),
            folder: esc(&skill.folder),
            brought_by: skill.brought_by.as_deref().map(esc),
            summary: (&skill.summary).into(),
            files: skill
                .files
                .iter()
                .map(|file| ReviewFile {
                    path: esc(&file.path),
                    size: u32::try_from(file.size).unwrap_or(u32::MAX),
                    executable: file.executable,
                    text: file.text.clone(),
                    risky: file.risky,
                })
                .collect(),
            left_out: skill
                .left_out
                .iter()
                .map(|one| ReviewLeftOut {
                    path: esc(&one.path),
                    why: one.why.into(),
                    said: one.why.to_string(),
                })
                .collect(),
            lint: skill.lint.iter().map(|l| esc(l)).collect(),
        }
    }
}

/// What the window hands back to install or update: what the review answered as accepted.
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

impl From<&Accepted> for Reviewed {
    fn from(accepted: &Accepted) -> Self {
        Reviewed {
            commit: accepted.commit.as_str().to_owned(),
            skills: accepted
                .skills
                .iter()
                .map(|one| ReviewedSkill {
                    name: one.name.clone(),
                    digest: one.digest.clone(),
                })
                .collect(),
        }
    }
}

impl TryFrom<&Reviewed> for Accepted {
    type Error = String;
    fn try_from(reviewed: &Reviewed) -> Result<Accepted, String> {
        Ok(Accepted {
            commit: Commit::parse(&reviewed.commit)
                .ok_or_else(|| "That review is not one purlis gave.".to_owned())?,
            skills: reviewed
                .skills
                .iter()
                .map(|one| AcceptedSkill {
                    name: one.name.clone(),
                    digest: one.digest.clone(),
                })
                .collect(),
        })
    }
}

/// **An install review**, summary first, and what accepting it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct InstallReview {
    pub commit: SourceCommit,
    pub skills: Vec<ReviewSkill>,
    pub accepted: Reviewed,
}

/// One newer commit offered for an installed skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdateOffer {
    pub name: String,
    pub level: SkillLevel,
    pub label: String,
    pub remote: SourceRemote,
    pub pinned: SourceCommit,
    pub newer: SourceCommit,
    pub edited: bool,
}

/// An installed skill whose source could not be asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdateUnasked {
    pub name: String,
    pub label: String,
    pub why: String,
}

/// A source the committed lockfile names that this machine has not confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSourceUnconfirmed {
    pub remote: SourceRemote,
    pub skills: Vec<String>,
}

/// The updates there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUpdates {
    pub offers: Vec<SkillUpdateOffer>,
    pub unasked: Vec<SkillUpdateUnasked>,
    pub unconfirmed: Vec<SkillSourceUnconfirmed>,
}

/// How a file changes in an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum FileChanged {
    Added,
    Removed,
    Changed,
}

impl From<source::Changed> for FileChanged {
    fn from(change: source::Changed) -> Self {
        match change {
            source::Changed::Added => Self::Added,
            source::Changed::Removed => Self::Removed,
            source::Changed::Changed => Self::Changed,
        }
    }
}

/// One file's change in an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateFile {
    pub path: String,
    pub change: FileChanged,
    /// Its lines' diff, every line escaped by the core.
    pub diff: String,
}

fn files_shown(files: &[source::FileChange]) -> Vec<UpdateFile> {
    files
        .iter()
        .map(|file| UpdateFile {
            path: esc(&file.path),
            change: file.change.into(),
            diff: file.diff.clone(),
        })
        .collect()
}

/// **An update's review**: the summary's changes, then each file's, the edits made here, and
/// the skills that come along.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateReview {
    pub name: String,
    pub pinned: SourceCommit,
    pub newer: SourceCommit,
    pub from_pin: bool,
    pub added: ReviewSummary,
    pub removed: ReviewSummary,
    pub files: Vec<UpdateFile>,
    pub edited: Vec<String>,
    pub local_edits: Vec<UpdateFile>,
    pub skill: ReviewSkill,
    pub brought: Vec<ReviewSkill>,
    /// Those of `brought` under a built-in skill's name (#3728).
    pub reserved: Vec<String>,
    /// Why it cannot be taken as it is, said before Accept update: it brings along a skill under
    /// a built-in skill's name (#3728).
    pub refused: Option<String>,
    pub accepted: Reviewed,
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
        Ok(source::sources(held.root())?
            .into_iter()
            .map(|one| SkillSource::of(one.source, one.which))
            .collect())
    })
    .await
}

/// **What the last check of a source found** (#3840): when it was, and how many skills it
/// offered then, as the core recorded it (`source::checked`), with how many are installed from it
/// now. Read with the source list, and never by asking a source.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SourceChecked {
    pub remote: SourceRemote,
    /// When it was last checked, in seconds since the epoch; `None` where nothing has yet.
    pub at: Option<f64>,
    /// How many skills it offered at its last browse; `None` where it was never browsed here.
    pub skills: Option<u32>,
    /// How many skills are installed from it, at any level, each once.
    pub installed: u32,
}

/// **When each of the project's sources was last checked, and its counts** (#3840): one for each
/// source [`skill_sources`] lists, read from what the last browse or updates check recorded and
/// from the lockfiles, so Sources opens without asking any source.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_sources_checked(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<SourceChecked>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading when the skill sources were checked", move || {
        flag_on()?;
        checked_shown(held.root())
    })
    .await
}

/// Seconds since the epoch as the window reads a number: exact for every second before the year
/// 285 million.
#[allow(clippy::cast_precision_loss)]
fn seconds(at: u64) -> f64 {
    at as f64
}

/// [`skill_sources_checked`] for the project at `root`.
fn checked_shown(root: &std::path::Path) -> Result<Vec<SourceChecked>, String> {
    let recorded = source::checked::read(root);
    let (locked, _) = source::lock::every(root);
    Ok(source::sources(root)?
        .into_iter()
        .map(|one| {
            let remote = one.source.remote;
            let last = recorded.iter().find(|at| at.remote == remote);
            SourceChecked {
                remote: (&remote).into(),
                at: last.map(|at| seconds(at.at)),
                skills: last.and_then(|at| at.skills),
                installed: source::checked::installed_from(&locked, &remote),
            }
        })
        .collect())
}

/// Whose list an offered source is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SourceOffering {
    /// purlis's own first-party catalogue.
    Catalogue,
    /// A collection someone else keeps that people use widely.
    WellKnown,
}

impl From<purlis_core::skills::first_open::Offering> for SourceOffering {
    fn from(offering: purlis_core::skills::first_open::Offering) -> Self {
        use purlis_core::skills::first_open::Offering;
        match offering {
            Offering::Catalogue => Self::Catalogue,
            Offering::WellKnown => Self::WellKnown,
        }
    }
}

/// **A skill source the studio offers** (SK-22, #2785): its name, why it is on the list, and the source Add
/// source starts filled in with, kept with the project unless the person picks this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct OfferedSource {
    pub offering: SourceOffering,
    pub name: String,
    pub why: String,
    pub source: SkillSource,
}

/// **The sources the studio offers** (SK-22, #2785; decision 42): the first-party catalogue and
/// the well-known skill sources the project does not browse, each with why it is on the list
/// (`purlis_core::skills::first_open::offered_here`). Read beside [`skill_sources`] and not with
/// the studio, since reading the project's sources asks git whether this machine's settings are
/// ignored.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_sources_offered(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<OfferedSource>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the offered skill sources", move || {
        flag_on()?;
        Ok(offered_shown(held.root()))
    })
    .await
}

/// The sources offered to the project at `root`, as the window draws them: each kept with the
/// project unless the person picks this machine in Add source.
fn offered_shown(root: &std::path::Path) -> Vec<OfferedSource> {
    purlis_core::skills::first_open::offered_here(root)
        .into_iter()
        .map(|one| OfferedSource {
            offering: one.offering.into(),
            name: esc(&one.name),
            why: esc(&one.why),
            source: SkillSource::of(one.source, Which::Shared),
        })
        .collect()
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
            source::add(cx, &source.core(), source.kept.into())
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
    remote: SourceRemote,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("taking the skill source out", move || {
        flag_on()?;
        heard(held.root(), |cx| source::remove(cx, &remote.core()))
    })
    .await
}

/// **Confirms a source the committed lockfile names** on this machine, so it is asked for
/// updates.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_confirm(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    remote: SourceRemote,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("confirming the skill source", move || {
        flag_on()?;
        heard(held.root(), |cx| source::confirm(cx, &remote.core()))
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
            commit: (&browse.commit).into(),
            skills: browse
                .skills
                .into_iter()
                .map(|one| OfferedSkill {
                    name: esc(&one.skill.name),
                    description: esc(&one.skill.description),
                    folder: esc(&one.skill.folder),
                    installed: one.installed.iter().map(ToString::to_string).collect(),
                    marks: one.marks,
                })
                .collect(),
            not_skills: browse
                .not_skills
                .into_iter()
                .map(|one| NotASkill {
                    folder: esc(&one.folder),
                    why: esc(&one.why),
                })
                .collect(),
            checked: seconds(browse.checked),
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
            commit: (&review.commit).into(),
            skills: review.skills.iter().map(Into::into).collect(),
            accepted: (&review.accepted()).into(),
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
        let accepted = Accepted::try_from(&reviewed)?;
        let grant: Option<purlis_core::skills::Grant> = grant.map(Into::into);
        heard(held.root(), |cx| {
            source::install(
                cx,
                &source.core(),
                &(&level).into(),
                &accepted,
                grant.as_ref(),
            )
        })
    })
    .await
}

/// **The updates there are**: each source this machine may contact asked for its newest commit,
/// nothing fetched.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_updates(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<SkillUpdates, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("asking the skill sources", move || {
        flag_on()?;
        let found = source::updates(held.root());
        Ok(SkillUpdates {
            offers: found
                .offers
                .iter()
                .map(|one| SkillUpdateOffer {
                    label: one.level.to_string(),
                    level: (&one.level).into(),
                    name: esc(&one.name),
                    remote: (&one.remote).into(),
                    pinned: (&one.pinned).into(),
                    newer: (&one.newer).into(),
                    edited: one.edited,
                })
                .collect(),
            unasked: found
                .unasked
                .iter()
                .map(|one| SkillUpdateUnasked {
                    label: one.level.to_string(),
                    name: esc(&one.name),
                    why: esc(&one.why),
                })
                .collect(),
            unconfirmed: found
                .unconfirmed
                .iter()
                .map(|one| SkillSourceUnconfirmed {
                    remote: (&one.remote).into(),
                    skills: one.skills.iter().map(|s| esc(s)).collect(),
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
        let built_in = crate::skillchange::built_in();
        let update =
            source::update_review(held.root(), built_in.as_deref(), &(&level).into(), &name)?;
        Ok(UpdateReview {
            name: esc(&update.name),
            pinned: (&update.pinned).into(),
            newer: (&update.newer).into(),
            from_pin: update.from_pin,
            added: (&update.changes.added).into(),
            removed: (&update.changes.removed).into(),
            files: files_shown(&update.files),
            edited: update.edited.iter().map(|e| esc(e)).collect(),
            local_edits: files_shown(&update.local_edits),
            skill: (&update.review).into(),
            brought: update.brought.iter().map(Into::into).collect(),
            reserved: update.reserved.iter().map(|n| esc(n)).collect(),
            refused: update.refused().map(|why| esc(&why)),
            accepted: (&update.accepted()).into(),
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
        let accepted = Accepted::try_from(&reviewed)?;
        heard(held.root(), |cx| {
            source::update(cx, &(&level).into(), &name, &accepted)
        })
    })
    .await
}

#[cfg(test)]
mod tests;
