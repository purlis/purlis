// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Publish a skill to a source, and export it, from the window** (SK-18, #2781;
//! `prototype/skill-publish`): the review the page draws, the publish its button makes, and the
//! export its dialog makes. Every verb is the core's (`skills::source::publish`, `outgoing`).
//!
//! **No chat reaches these**: they are commands of the window's alone, which asks the forge as
//! the person in the window, through their purlis account for the host and no other helper. A
//! publish is handed back what the person accepted and refused where any of it moved.

use std::path::{Path, PathBuf};

use purlis_core::skills::source::publish::{self, Accepted, OnTheForge, PublishReview, Published};
use purlis_core::skills::source::{self, Outgoing, Purpose, Stop};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_plugin_dialog::DialogExt as _;

use crate::skillchange::{SkillLevel, flag_on};
use crate::skillsources::SkillSource;

/// A file's or a skill's text as the window shows it: every invisible character escaped.
fn esc(text: &str) -> String {
    purlis_core::skills::review::escaped(text)
}

/// One file that goes.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct OutgoingFile {
    /// Its path, under its skill's folder.
    pub path: String,
    pub bytes: u32,
    pub runs: bool,
    /// Whether it is of a skill the named one refers to.
    pub comes_along: bool,
}

/// One file that stays behind.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct BehindFile {
    pub path: String,
    pub why: String,
}

/// One thing that stops it, as `where` and what it holds, never the value itself.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct OutgoingStop {
    pub place: String,
    pub said: String,
}

/// One grade the description holds.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct PublishGrade {
    pub skill: String,
    pub when: String,
    pub version: String,
    pub harness: String,
    pub with_it: String,
    pub without_it: String,
    pub runs: u32,
    pub ended: String,
}

/// **What the publish review page draws.**
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct PublishReviewView {
    /// "pull request" or "merge request".
    pub request: String,
    pub forge: String,
    pub base: String,
    pub commit_short: String,
    pub branch: String,
    pub title: String,
    pub author: String,
    pub files: Vec<OutgoingFile>,
    pub behind: Vec<BehindFile>,
    pub stops: Vec<OutgoingStop>,
    /// Where the skill named is this machine's alone, the level that says so.
    pub local: bool,
    pub grades: Vec<PublishGrade>,
    pub accepted: PublishAccepted,
}

/// What a yes hands back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct PublishAccepted {
    pub base: String,
    pub commit: String,
    pub digest: String,
}

/// **How a publish ended.**
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PublishDone {
    Opened { url: String },
    Already { url: String },
    CannotPush { why: String },
}

/// **What the export dialog says.**
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct ExportReview {
    pub skills: Vec<String>,
    pub behind: Vec<BehindFile>,
    pub stops: Vec<OutgoingStop>,
}

fn files_of(out: &Outgoing) -> Vec<OutgoingFile> {
    let first = out.skills.first().map(|skill| skill.name.clone());
    out.skills
        .iter()
        .flat_map(|skill| {
            let along = Some(&skill.name) != first.as_ref();
            skill.files.iter().map(move |file| OutgoingFile {
                path: esc(&format!("{}/{}", skill.name, file.path())),
                bytes: u32::try_from(file.bytes.len()).unwrap_or(u32::MAX),
                runs: file.executable,
                comes_along: along,
            })
        })
        .collect()
}

fn behind_of(out: &Outgoing) -> Vec<BehindFile> {
    out.skills
        .iter()
        .flat_map(|skill| {
            skill.left.iter().map(move |left| BehindFile {
                path: esc(&format!("{}/{}", skill.name, left.path)),
                why: left.why.to_owned(),
            })
        })
        .collect()
}

fn stops_of(out: &Outgoing) -> Vec<OutgoingStop> {
    out.stops
        .iter()
        .filter_map(|stop| match stop {
            Stop::Found {
                skill,
                path,
                line,
                kind,
                masked,
            } => Some(OutgoingStop {
                place: esc(&format!("{skill}/{path}:{line}")),
                said: esc(&format!("Holds {kind}: {masked}")),
            }),
            Stop::Local { .. } => None,
        })
        .collect()
}

fn view_of(review: &PublishReview, forge: &OnTheForge) -> PublishReviewView {
    let accepted = review.accepted();
    PublishReviewView {
        request: forge.request_noun().to_owned(),
        forge: forge.display().to_owned(),
        base: esc(&review.base),
        commit_short: review.commit.short().to_owned(),
        branch: esc(&review.branch),
        title: esc(&review.title),
        author: esc(&format!("{}, {}", review.author.0, review.author.1)),
        files: files_of(&review.outgoing),
        behind: behind_of(&review.outgoing),
        stops: stops_of(&review.outgoing),
        local: review
            .outgoing
            .stops
            .iter()
            .any(|stop| matches!(stop, Stop::Local { .. })),
        grades: review
            .grades
            .iter()
            .map(|one| PublishGrade {
                skill: one.skill.clone(),
                when: one.when.clone(),
                version: one.version.to_owned(),
                harness: one.harness.clone(),
                with_it: one.with_it.clone(),
                without_it: one.without_it.clone(),
                runs: u32::from(one.runs),
                ended: one.ended.to_owned(),
            })
            .collect(),
        accepted: PublishAccepted {
            base: accepted.base,
            commit: accepted.commit.to_string(),
            digest: accepted.digest,
        },
    }
}

/// **The review of publishing `name` at `level` to `source`.** Nothing goes anywhere.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_publish_review(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    source: SkillSource,
) -> Result<PublishReviewView, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reviewing the publish", move || {
        flag_on()?;
        crate::forgeaccounts::window_git(|| {
            let core = source.core();
            let forge = OnTheForge::for_the_window(held.root(), &core)?;
            let built_in = crate::skillchange::built_in();
            let review = publish::review(
                held.root(),
                built_in.as_deref(),
                &(&level).into(),
                &name,
                &core,
            )?;
            Ok(view_of(&review, &forge))
        })
    })
    .await
}

/// **Sends what the person accepted**, and only that.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_publish(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    source: SkillSource,
    accepted: PublishAccepted,
) -> Result<PublishDone, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("publishing the skill", move || {
        flag_on()?;
        let commit = source::Commit::parse(&accepted.commit)
            .ok_or("The review named no commit. Review it again")?;
        let accepted = Accepted {
            base: accepted.base,
            commit,
            digest: accepted.digest,
        };
        crate::forgeaccounts::window_git(|| {
            let core = source.core();
            let forge = OnTheForge::for_the_window(held.root(), &core)?;
            let built_in = crate::skillchange::built_in();
            let done = publish::publish(
                held.root(),
                built_in.as_deref(),
                &(&level).into(),
                &name,
                &core,
                &accepted,
                &forge,
            )?;
            Ok(match done {
                Published::Opened { url, .. } => PublishDone::Opened { url },
                Published::Already { url } => PublishDone::Already { url },
                Published::PushRefused { why } => PublishDone::CannotPush { why: esc(&why) },
            })
        })
    })
    .await
}

/// **What an export of `name` at `level` would write.**
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_export_review(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<ExportReview, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what an export would write", move || {
        flag_on()?;
        let built_in = crate::skillchange::built_in();
        let out = source::gather(
            held.root(),
            built_in.as_deref(),
            &(&level).into(),
            &name,
            Purpose::Export,
        )?;
        Ok(ExportReview {
            skills: out.skills.iter().map(|skill| esc(&skill.name)).collect(),
            behind: behind_of(&out),
            stops: stops_of(&out),
        })
    })
    .await
}

/// Where to write it, as the person picks in the system's save dialog: `None` where they did not.
async fn save_to(app: &tauri::AppHandle, suggested: String) -> Result<Option<PathBuf>, String> {
    let (chose, chosen) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_file_name(suggested)
        .save_file(move |picked| {
            // The window may have gone while the dialog was up; then there is nobody to tell.
            let _ = chose.send(picked);
        });
    tauri::async_runtime::spawn_blocking(move || chosen.recv().ok().flatten())
        .await
        .map_err(|err| format!("the save dialog did not finish: {err}"))?
        .map(|picked| {
            picked
                .into_path()
                .map_err(|err| format!("that place cannot be written: {err}"))
        })
        .transpose()
}

/// `gather` and write, read again at the moment of writing.
fn exported(
    root: &Path,
    level: &SkillLevel,
    name: &str,
    to: &Path,
    zip: bool,
) -> Result<(), String> {
    let built_in = crate::skillchange::built_in();
    let out = source::gather(
        root,
        built_in.as_deref(),
        &level.into(),
        name,
        Purpose::Export,
    )?;
    if zip {
        source::export_zip(&out, to)
    } else {
        source::export_folder(&out, to)
    }
}

/// **Asks where, and writes the export**: the place written, or `None` where the person chose
/// none. Nothing is written over anything.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_export(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    zip: bool,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    flag_on()?;
    let suggested = if zip {
        format!("{name}.zip")
    } else {
        name.clone()
    };
    let Some(to) = save_to(&app, suggested).await? else {
        return Ok(None);
    };
    crate::off_the_window("exporting the skill", move || {
        exported(held.root(), &level, &name, &to, zip)?;
        Ok(Some(to.display().to_string()))
    })
    .await
}
