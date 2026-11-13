// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Extension skill sources in the studio** (#3092, #3089; ADR 0100 §8, as amended 2026-11-08):
//! each extension this machine approved, the project has on, and that declares `skill-source`,
//! listed beside the git sources, browsed, reviewed, installed and updated through the same
//! pages. Every question goes through the executor's gate and the extension's sandbox
//! (`purlis_core::skills::studio`).
//!
//! **What a review read is held here, in memory**, and the install or update the person accepts
//! copies exactly that: the extension is never asked again between the review and the copy, and
//! the core refuses where what is accepted is not what is held. Nothing held is written anywhere,
//! and it goes with the app.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use specta::Type;

use purlis_core::extension::capability::Capability;
use purlis_core::skills::source::extension::{self as ext, Held};
use purlis_core::skills::source::{Accepted, AcceptedSkill, Origin, Pin, Revision};
use purlis_core::skills::studio::{Studio, with_studio};

use super::{
    InstallReview, InstallSetReview, OfferedSkill, Reviewed, SkillRename, SourceBrowse,
    SourceCommit, UpdateReview, esc, update_shown,
};
use crate::skillchange::{SkillChanged, SkillGrant, SkillLevel, flag_on, heard_audited};

/// **An extension's skill source**, as the studio lists one: the extension's id, the name it
/// gives itself, and how many skills are installed from it here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ExtensionSkillSource {
    pub id: String,
    pub label: String,
    pub installed: u32,
}

/// What a review held: an install's, by the project and extension, or an update's, by the
/// project, level and skill.
#[derive(Debug, Clone, PartialEq, Eq)]
enum For {
    Install {
        plane: PathBuf,
        id: String,
    },
    Update {
        plane: PathBuf,
        level: String,
        name: String,
    },
}

/// The most reviews held at once: each is one skill set's files, most a few KiB. Review updates
/// holds one for each skill from an extension it shows (#5760), so it is room for a page of them.
const MOST_HELD: usize = 32;

/// **What each open review read**, newest last: one for each install or update a review waits on.
static HELD: Mutex<Vec<(For, Held)>> = Mutex::new(Vec::new());

/// Holds `held` for `what`, in place of what an earlier review of it read.
fn hold(what: For, held: Held) {
    let mut all = HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    all.retain(|(one, _)| *one != what);
    all.push((what, held));
    let over = all.len().saturating_sub(MOST_HELD);
    all.drain(..over);
}

/// What the last review of `what` read, where one is held.
fn held(what: &For) -> Result<Held, String> {
    HELD.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .find(|(one, _)| one == what)
        .map(|(_, held)| held.clone())
        .ok_or_else(|| "That review is no longer open: review it again.".to_owned())
}

/// Lets go of what the review of `what` read: its change was made, and the same accept sent
/// again needs a new review, never the old bytes over what changed since.
fn release(what: &For) {
    HELD.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .retain(|(one, _)| one != what);
}

/// **Refused unless the extension `id` is still a skill source here**: approved on this machine,
/// on in the project, and declaring `skill-source`. What its review read is copied only while
/// the person's consent to it stands; asking it nothing.
fn still_a_source(
    executor: &purlis_core::executor::Executor,
    root: &Path,
    id: &str,
) -> Result<(), String> {
    let on = with_studio(executor, root, |studio| {
        studio.is_some_and(|studio| {
            studio
                .declaring(Capability::SkillSource)
                .iter()
                .any(|one| one.id == id)
        })
    });
    if on {
        Ok(())
    } else {
        Err(
            "That extension is no longer approved or on here, so nothing was changed. Turn it on \
             in Extensions, then review it again."
                .to_owned(),
        )
    }
}

/// What was accepted, at the extension's revision.
fn accepted_at_revision(reviewed: &Reviewed) -> Result<Accepted, String> {
    Ok(Accepted {
        pin: Pin::Revision(
            Revision::parse(&reviewed.commit)
                .ok_or_else(|| "That review is not one purlis gave: review it again.".to_owned())?,
        ),
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

/// `with` the studio of the project at `root`, or why no extension can be asked here.
fn studied<T>(
    executor: &purlis_core::executor::Executor,
    root: &Path,
    with: impl FnOnce(Studio<'_>) -> Result<T, String>,
) -> Result<T, String> {
    with_studio(executor, root, |studio| match studio {
        Some(studio) => with(studio),
        None => Err(
            "purlis could not read its extensions here, so the source was not asked. Open \
             Extensions to check them, then try again."
                .into(),
        ),
    })
}

impl From<&ext::Review> for InstallReview {
    fn from(review: &ext::Review) -> Self {
        InstallReview {
            commit: (&Pin::Revision(review.revision.clone())).into(),
            skills: review.skills.iter().map(Into::into).collect(),
            accepted: (&review.accepted()).into(),
        }
    }
}

/// **The extensions' skill sources** (#3092): each extension this machine approved, the project
/// has on, and that declares `skill-source`, in id order. Read from the record of extensions and
/// the lockfiles: no extension is asked.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_sources(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<ExtensionSkillSource>, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reading the extensions' skill sources", move || {
        flag_on()?;
        let (locked, _) = purlis_core::skills::source::lock::every(held.root());
        Ok(with_studio(&executor, held.root(), |studio| {
            studio
                .map(|studio| studio.declaring(Capability::SkillSource))
                .unwrap_or_default()
        })
        .into_iter()
        .map(|one| ExtensionSkillSource {
            installed: purlis_core::skills::source::checked::installed_from(
                &locked,
                &Origin::Extension(one.id.clone()),
            ),
            label: esc(&one.name),
            id: esc(&one.id),
        })
        .collect())
    })
    .await
}

/// **Browses an extension's skill source** at the revision it names now: its listing, and each
/// skill's files read for its marks (#5760).
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_source_browse(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: String,
) -> Result<SourceBrowse, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("browsing the skill source", move || {
        flag_on()?;
        let browse = studied(&executor, held.root(), |studio| {
            ext::browse(studio, held.root(), &id)
        })?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |at| at.as_secs());
        Ok(SourceBrowse {
            commit: SourceCommit::from(&Pin::Revision(browse.revision.clone())),
            skills: browse
                .skills
                .into_iter()
                .map(|one| OfferedSkill {
                    name: esc(&one.skill.name),
                    description: esc(&one.skill.description),
                    folder: esc(&one.skill.folder),
                    installed: one.installed.iter().map(ToString::to_string).collect(),
                    // purlis's own words, read to browse as a git source's are and let go of
                    // (#5760): the install copies what its own review reads.
                    marks: one.marks,
                    refused: one.refused.as_ref().map(Into::into),
                })
                .collect(),
            not_skills: Vec::new(),
            checked: super::seconds(now),
        })
    })
    .await
}

/// **The install review** of `names` from an extension's source into `level`: the git source's
/// review, and what it read held for the install.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_install_review(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: String,
    names: Vec<String>,
    level: SkillLevel,
    without: Vec<String>,
) -> Result<InstallReview, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reviewing the install", move || {
        flag_on()?;
        let root = held.root();
        let at = (&level).into();
        let (review, read) = studied(&executor, root, |studio| {
            let (mut review, read) = ext::review(studio, root, &id, &names, &at, &without)?;
            studio.lint_reviewed(&mut review.skills, &at);
            Ok((review, read))
        })?;
        hold(
            For::Install {
                plane: root.to_path_buf(),
                id,
            },
            read,
        );
        Ok(InstallReview::from(&review))
    })
    .await
}

/// **The review of installing `names` from an extension's source together** (Install all…): one
/// review, those named that the level holds already left out (#4153), what it read held for the
/// install.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_install_set_review(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: String,
    names: Vec<String>,
    level: SkillLevel,
    without: Vec<String>,
) -> Result<InstallSetReview, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reviewing the install", move || {
        flag_on()?;
        let root = held.root();
        let at = (&level).into();
        let (set, read) = studied(&executor, root, |studio| {
            let (mut set, read) = ext::review_set(studio, root, &id, &names, &at, &without)?;
            studio.lint_reviewed(&mut set.review.skills, &at);
            Ok((set, read))
        })?;
        hold(
            For::Install {
                plane: root.to_path_buf(),
                id,
            },
            read,
        );
        Ok(InstallSetReview::Review {
            review: (&set.review).into(),
            there: set.there.iter().map(|one| esc(one)).collect(),
        })
    })
    .await
}

/// **Installs what was reviewed** from an extension's source at `level`: the bytes its review
/// read, never asked of the extension again.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_extension_install(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: String,
    level: SkillLevel,
    reviewed: Reviewed,
    grant: Option<SkillGrant>,
    renames: Vec<SkillRename>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("installing the skill", move || {
        flag_on()?;
        let root = held.root();
        still_a_source(&executor, root, &id)?;
        let what = For::Install {
            plane: root.to_path_buf(),
            id,
        };
        let read = self::held(&what)?;
        let accepted = accepted_at_revision(&reviewed)?;
        let grant: Option<purlis_core::skills::Grant> = grant.map(Into::into);
        let changed = heard_audited(&held, |cx| {
            ext::install(
                cx,
                &read,
                &(&level).into(),
                &accepted,
                grant.as_ref(),
                &renames
                    .iter()
                    .map(|one| (one.from.clone(), one.to.clone()))
                    .collect(),
            )
        })?;
        release(&what);
        Ok(changed)
    })
    .await
}

/// **What the update of `name` at `level` copies from, as one of several accepted at once**
/// (#5760): for a skill from an extension's source, what its own review read, while the extension
/// is still a source here; `None` for a git source's. Refused where its review is not held.
pub(crate) fn held_for_a_set(
    executor: &purlis_core::executor::Executor,
    root: &Path,
    level: &purlis_core::skills::verbs::Level,
    name: &str,
) -> Result<Option<Held>, String> {
    let (locked, _) = purlis_core::skills::source::lock::every(root);
    let from_an_extension = locked
        .iter()
        .any(|(_, one)| one.is(level, name) && matches!(one.source, Origin::Extension(_)));
    if !from_an_extension {
        return Ok(None);
    }
    let read = held(&For::Update {
        plane: root.to_path_buf(),
        level: level.to_string(),
        name: name.to_owned(),
    })
    .map_err(|_| format!("The review of {name} is no longer open: review the updates again."))?;
    still_a_source(executor, root, read.extension())?;
    Ok(Some(read))
}

/// Lets go of what the reviews of the updates of `names` (each at its level) read, once the set
/// that copied them is made.
pub(crate) fn release_set(root: &Path, names: &[(purlis_core::skills::verbs::Level, String)]) {
    for (level, name) in names {
        release(&For::Update {
            plane: root.to_path_buf(),
            level: level.to_string(),
            name: name.clone(),
        });
    }
}

/// **The review of updating `name` at `level`**, a skill from an extension's source, to the
/// revision its source names now; what it read held for the update.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_update_review(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<UpdateReview, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reviewing the update", move || {
        flag_on()?;
        let root = held.root();
        let at = (&level).into();
        let (update, read) = studied(&executor, root, |studio| {
            let (mut update, read) = ext::update_review(studio, root, &at, &name)?;
            studio.lint_reviewed(update.skills_mut(), &at);
            Ok((update, read))
        })?;
        hold(
            For::Update {
                plane: root.to_path_buf(),
                level: at.to_string(),
                name,
            },
            read,
        );
        Ok(update_shown(&update))
    })
    .await
}

/// **Accepts an update** of a skill from an extension's source that was reviewed: from what its
/// review read, the pin moved to the revision, as one commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_extension_update(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    reviewed: Reviewed,
    grant: Option<SkillGrant>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("updating the skill", move || {
        flag_on()?;
        let root = held.root();
        let at: purlis_core::skills::verbs::Level = (&level).into();
        let what = For::Update {
            plane: root.to_path_buf(),
            level: at.to_string(),
            name: name.clone(),
        };
        let read = self::held(&what)?;
        still_a_source(&executor, root, read.extension())?;
        let accepted = accepted_at_revision(&reviewed)?;
        let grant: Option<purlis_core::skills::Grant> = grant.map(Into::into);
        let changed = heard_audited(&held, |cx| {
            ext::update(cx, &read, &at, &name, &accepted, grant.as_ref())
        })?;
        release(&what);
        Ok(changed)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_review_is_found_by_what_it_is_for_and_the_oldest_goes_past_the_most() {
        let for_ = |n: usize| For::Install {
            plane: PathBuf::from(format!("/held-test-{n}")),
            id: "acme".into(),
        };
        let one = Held::of("acme", "r1", &[]);
        for n in 0..=MOST_HELD {
            hold(for_(n), one.clone());
        }
        assert!(held(&for_(0)).is_err(), "the oldest went");
        assert_eq!(held(&for_(MOST_HELD)).unwrap(), one);
        // A second review of the same install takes the first one's place.
        let two = Held::of("acme", "r2", &[]);
        hold(for_(MOST_HELD), two.clone());
        assert_eq!(held(&for_(MOST_HELD)).unwrap(), two);
        // Once its change is made, the same accept needs a new review.
        release(&for_(MOST_HELD));
        assert!(held(&for_(MOST_HELD)).is_err());
    }

    #[test]
    fn what_was_accepted_is_read_as_the_extensions_revision_or_refused() {
        let reviewed = |commit: &str| Reviewed {
            commit: commit.into(),
            skills: Vec::new(),
        };
        assert_eq!(
            accepted_at_revision(&reviewed("v1.2+b")).unwrap().pin,
            Pin::Revision(Revision::parse("v1.2+b").unwrap())
        );
        assert!(accepted_at_revision(&reviewed("../x")).is_err());
    }
}
