// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's tab beyond its runs** (SK-19, #2782): each copy's history, with a diff and a
//! revert per version and that version's runs; its lifecycle state (deprecated, archived) and
//! owners; the suggestion to deprecate a skill no chat ran for 60 days; and the skills it names
//! and that name it. Each is `purlis_core::skills`' own (`history`, `lifecycle`, `graph`), which
//! `purlis skill` calls too. Behind the `skill-studio` flag until its finish line (#2786).

use serde::{Deserialize, Serialize};
use specta::Type;

use purlis_core::skills::verbs::Level;
use purlis_core::skills::{Deprecated, Lifecycle, history, lifecycle};

use crate::skillchange::{SkillChanged, SkillLevel, built_in, flag_on, heard};

/// A skill's lifecycle state, as its tab shows and sets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SkillLifecycle {
    /// In use.
    Active,
    /// Still works, is shown after the rest, and names the skill to use instead.
    Deprecated { replacement: Option<String> },
    /// Never loads, until it is restored.
    Archived,
}

impl From<Lifecycle> for SkillLifecycle {
    fn from(lifecycle: Lifecycle) -> SkillLifecycle {
        match lifecycle {
            Lifecycle::Active => SkillLifecycle::Active,
            Lifecycle::Deprecated(Deprecated { replacement }) => {
                SkillLifecycle::Deprecated { replacement }
            }
            Lifecycle::Archived => SkillLifecycle::Archived,
        }
    }
}

impl From<SkillLifecycle> for Lifecycle {
    fn from(lifecycle: SkillLifecycle) -> Lifecycle {
        match lifecycle {
            SkillLifecycle::Active => Lifecycle::Active,
            SkillLifecycle::Deprecated { replacement } => {
                Lifecycle::Deprecated(Deprecated { replacement })
            }
            SkillLifecycle::Archived => Lifecycle::Archived,
        }
    }
}

/// What one version's runs counted: signals, never a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct SkillVersionRuns {
    pub runs: u32,
    /// The runs whose end is recorded, which the counts are summed from.
    pub ended: u32,
    pub tool_calls: u32,
    pub tool_errors: u32,
    pub refusals: u32,
    pub asks: u32,
    pub ms: u32,
}

/// A version's grade, once grading lands (SK-16): always null for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct SkillGrade {
    pub passed: u32,
    pub fixtures: u32,
}

/// One version of a skill: one commit that changed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillVersion {
    /// Its commit: what its diff and its revert are asked by.
    pub commit: String,
    /// When it was committed, as a timestamp.
    pub at: String,
    pub author: String,
    pub subject: String,
    pub runs: SkillVersionRuns,
    pub grade: Option<SkillGrade>,
}

/// One copy of a skill's name, at one level of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillCopy {
    pub level: SkillLevel,
    /// Its level as a person reads it: `project`, `local workspace ops`.
    pub label: String,
    pub lifecycle: SkillLifecycle,
    /// Why its lifecycle state and owners could not be read, where they could not: it is then
    /// taken as in use with no owners.
    pub unread: Option<String>,
    /// Who looks after it, for people to know.
    pub owners: Vec<String>,
    /// How many days no chat ran it, where that is 60 or more: the tab suggests deprecating it.
    pub unused_days: Option<u32>,
    /// Its versions, newest first.
    pub versions: Vec<SkillVersion>,
}

/// A skill another skill names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUse {
    /// As the text writes it: `skills:deploy`, `purlis:secrets`.
    pub said: String,
    pub name: String,
    /// Whether it names one of the project's skills, not a built-in one.
    pub project: bool,
    /// Whether no skill has that name.
    pub broken: bool,
}

/// What a skill's tab shows beside its runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillPage {
    /// Every copy of the name at the project's levels, the project's first.
    pub copies: Vec<SkillCopy>,
    pub uses: Vec<SkillUse>,
    /// The project's skills that name it.
    pub used_by: Vec<String>,
    /// Whether the project holds more skills than the graph reads.
    pub uses_cut: bool,
}

/// One version's diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillDiff {
    pub text: String,
    /// Whether it was cut, being too long to show whole.
    pub cut: bool,
    /// A line for each line of it that holds a character that draws as nothing.
    pub unseen: Vec<String>,
}

fn small(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// [`skill_page`]'s answer for the project at `root`, at `now` (seconds since 1970).
fn page(root: &std::path::Path, name: &str, now: u64) -> SkillPage {
    let unused = lifecycle::unused(root, now);
    let copies = Level::every(root)
        .into_iter()
        .filter_map(|level| {
            let folder = level
                .dir(root)
                .and_then(|dir| purlis_core::skills::named(&dir, name))?;
            let about = purlis_core::skills::about_of(&folder);
            let versions = history::history(root, &level, name)
                .unwrap_or_default()
                .into_iter()
                .map(|version| SkillVersion {
                    at: chrono::DateTime::from_timestamp(i64::try_from(version.at).unwrap_or(0), 0)
                        .map(purlis_core::dispatch::stamp)
                        .unwrap_or_default(),
                    commit: version.commit,
                    author: version.author,
                    subject: version.subject,
                    runs: SkillVersionRuns {
                        runs: small(version.runs.runs),
                        ended: small(version.runs.ended),
                        tool_calls: small(version.runs.tool_calls),
                        tool_errors: small(version.runs.tool_errors),
                        refusals: small(version.runs.refusals),
                        asks: small(version.runs.asks),
                        ms: small(version.runs.ms),
                    },
                    grade: version.grade.map(|grade| SkillGrade {
                        passed: grade.passed,
                        fixtures: grade.fixtures,
                    }),
                })
                .collect();
            Some(SkillCopy {
                label: level.to_string(),
                unused_days: unused
                    .iter()
                    .find(|one| one.name == name && one.level == level)
                    .map(|one| small(one.days)),
                level: (&level).into(),
                lifecycle: about
                    .as_ref()
                    .map(|about| about.lifecycle.clone())
                    .unwrap_or_default()
                    .into(),
                owners: about
                    .as_ref()
                    .map(|about| about.owners.clone())
                    .unwrap_or_default(),
                unread: about.err(),
                versions,
            })
        })
        .collect();
    let built_in = built_in();
    let node = purlis_core::skills::graph::uses_of(root, built_in.as_deref(), name);
    SkillPage {
        copies,
        uses: node
            .uses
            .into_iter()
            .map(|one| SkillUse {
                said: one.said,
                name: one.name,
                project: one.project,
                broken: one.broken,
            })
            .collect(),
        used_by: node.used_by,
        uses_cut: node.cut,
    }
}

/// **What a skill's tab shows beside its runs**: each copy's history, lifecycle state and
/// owners, and the skills it names and that name it. Nothing while the flag is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_page(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    name: String,
) -> Result<SkillPage, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill's history", move || {
        flag_on()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|at| at.as_secs())
            .unwrap_or_default();
        Ok(page(held.root(), &name, now))
    })
    .await
}

/// What one version of a skill changed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_diff(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    commit: String,
) -> Result<SkillDiff, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the version's changes", move || {
        flag_on()?;
        history::diff(held.root(), &(&level).into(), &name, &commit).map(|diff| SkillDiff {
            text: diff.text,
            cut: diff.cut,
            unseen: diff.unseen,
        })
    })
    .await
}

/// **Puts a skill back as one of its versions has it**, as one commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_revert(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    commit: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reverting the skill", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            history::revert(cx, &(&level).into(), &name, &commit)
        })
    })
    .await
}

/// **Deprecates, archives or restores a skill**, as one commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_lifecycle(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    lifecycle: SkillLifecycle,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("changing the skill's state", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            lifecycle::set(cx, &(&level).into(), &name, &lifecycle.into())
        })
    })
    .await
}

/// **Sets who looks after a skill**, as one commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_owners(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    owners: Vec<String>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("changing the skill's owners", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            lifecycle::set_owners(cx, &(&level).into(), &name, &owners)
        })
    })
    .await
}
