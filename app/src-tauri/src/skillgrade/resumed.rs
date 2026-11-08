// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Resuming a stopped grade** (#5718, the operator's rulings of 2026-11-08): the window's half
//! of `purlis_core::skills::grades::resumable`.
//!
//! - **Offered beside Grade again** on a stopped grade's scorecard, only while nothing that
//!   decides its result changed; otherwise the plan says what did ([`skill_grade_resume_plan`]).
//! - **Planned and confirmed like any grade**: the chats left, what they would spend at what its
//!   chats spent each, and a new cap counting only this run's spend.
//! - **Never by itself**: a usage limit's reset is said, and Resume is refused before it; the
//!   Skill studio's Needs you says when it can run ([`needs`]).
//! - **A draft's grade resumes too**, against the draft as it is now, and a batch's stopped
//!   grades resume under one limit (`super::batch`).

use serde::Serialize;
use specta::Type;

use purlis_core::skills::grades::{self, Planned, resumable};
use purlis_core::skills::verbs::Level;
use purlis_core::skills::verbs::drafts;

use super::{Begin, DraftBegun, On, begin, chats_on, running_of, terms, ticked_ids};
use crate::planes::{PlaneId, Planes};

/// **What resuming a stopped grade would run, as the window shows it before Resume is pressed.**
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradeResumePlan {
    pub grade: String,
    pub skill: String,
    /// How it stopped, in a sentence (`Stopped at the token cap`).
    pub ended: String,
    /// **Why Resume is not offered**, where it is not: what changed since it stopped, or why what
    /// it graded cannot be read now. Grade again is then the one way on.
    pub refused: Option<String>,
    /// **When the program's usage limit resets**, an RFC 3339 time, while it has not: Resume
    /// waits for it.
    pub waits_until: Option<String>,
    /// The chats left, and what they would spend at what its chats spent each, where they said.
    pub left: u32,
    pub tokens_left: Option<f64>,
    pub usd_left: Option<f64>,
    /// The cap and the dollar limit a resume is held to, which count only its own spend.
    pub token_cap: f64,
    pub cap_set_here: bool,
    pub limit_usd: f64,
    /// What the stopped grade spent, which the resumed one's scorecard adds to its own.
    pub earlier_tokens: Option<f64>,
    pub earlier_usd: Option<f64>,
    /// The profile and the model its chats run on, as the stopped grade's did.
    pub profile: Option<String>,
    pub model: Option<String>,
    /// What pressing Resume consents to; `None` where it is refused.
    pub consent: Option<String>,
    /// A grade of the skill running now, which Resume waits for.
    pub running: Option<String>,
}

/// **What a resume of a stopped grade reads now**: the plan of its copy (or its draft) with the
/// evals it ticked, what its chats would run on, and what decides its result now.
struct Now {
    planned: Planned,
    on: On,
    deciding: resumable::Deciding,
    level: Level,
    draft: Option<drafts::Draft>,
}

/// [`Now`] of `stopped` in the project at `root`, or why what it graded cannot be read.
fn read_now(root: &std::path::Path, stopped: &resumable::Stopped) -> Result<Now, String> {
    let ticked = ticked_ids(&stopped.request);
    let on = chats_on(
        root,
        stopped.deciding.profile.clone(),
        stopped.deciding.model.clone(),
    );
    let (planned, level, draft) = match &stopped.draft_of {
        Some(id) => {
            let draft = drafts::one(root, id)?;
            let planned = grades::draft::planned(root, &draft, on.profile.as_deref(), &ticked)?;
            (planned, draft.level.clone(), Some(draft))
        }
        None => {
            let level = stopped
                .level
                .as_deref()
                .and_then(grades::level_of_recorded)
                .ok_or("the copy it graded is not found here")?;
            let planned =
                grades::planned_with(root, &level, &stopped.skill, on.profile.as_deref(), &ticked)?;
            (planned, level, None)
        }
    };
    let (Some(request), Some(copy)) = (&planned.request, &planned.copy) else {
        return Err("it has no trigger tests or outcome evals to grade now".to_owned());
    };
    let deciding = resumable::Deciding::of(
        request,
        copy,
        resumable::On {
            profile: on.profile.as_deref(),
            program: on.harness.as_deref(),
            model: on.priced.model.as_deref().or(on.model.as_deref()),
        },
        None,
    );
    Ok(Now {
        planned,
        on,
        deciding,
        level,
        draft,
    })
}

/// **Why `stopped` cannot be resumed on `now`**, where it cannot: what changed, or a request of
/// another shape than what it ran (which a change of its tests or settings says first).
fn refused(stopped: &resumable::Stopped, now: &Now) -> Option<String> {
    if let Some(said) = resumable::changed_said(&stopped.deciding.changed(&now.deciding)) {
        return Some(said);
    }
    let request = now.planned.request.as_ref()?;
    stopped.kept.fits(request).err().map(|_| {
        "Resume is not offered: its tests changed since it stopped. Grade again runs it whole."
            .to_owned()
    })
}

/// `resets_at` while it is still ahead of `now`.
fn still_ahead(resets_at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(resets_at?).ok()?;
    (at > now).then(|| resets_at.unwrap_or_default().to_owned())
}

/// **The plan of resuming `stopped`** in `plane` at `root`, as [`SkillGradeResumePlan`] says it.
pub(crate) fn plan_of(
    root: &std::path::Path,
    plane: &PlaneId,
    stopped: &resumable::Stopped,
) -> SkillGradeResumePlan {
    shown(
        stopped,
        read_now(root, stopped),
        running_of(plane, &stopped.skill),
        chrono::Utc::now(),
    )
}

/// [`plan_of`], from `now`, what was read of what `stopped` graded (or why it was not), with
/// `running` the grade of its skill running, at `at`.
fn shown(
    stopped: &resumable::Stopped,
    now: Result<Now, String>,
    running: Option<String>,
    at: chrono::DateTime<chrono::Utc>,
) -> SkillGradeResumePlan {
    let earlier = stopped.kept.earlier;
    let mut shown = SkillGradeResumePlan {
        grade: stopped.grade.clone(),
        skill: stopped.skill.clone(),
        ended: grades::ended_said_on(stopped.ended, stopped.deciding.program.as_deref()),
        refused: None,
        waits_until: still_ahead(stopped.resets_at.as_deref(), at),
        left: stopped.left(),
        tokens_left: None,
        usd_left: None,
        token_cap: 0.0,
        cap_set_here: false,
        limit_usd: 0.0,
        earlier_tokens: earlier.tokens.map(|tokens| tokens as f64),
        earlier_usd: earlier.usd,
        profile: stopped.deciding.profile.clone(),
        model: stopped.deciding.model.clone(),
        consent: None,
        running,
    };
    let now = match now {
        Ok(now) => now,
        Err(why) => {
            shown.refused = Some(format!(
                "Resume is not offered: {why}. Grade again runs it whole."
            ));
            return shown;
        }
    };
    let plan = resumable::plan(stopped, now.planned.cap);
    shown.tokens_left = plan.tokens_left.map(|tokens| tokens as f64);
    shown.usd_left = plan.usd_left;
    shown.token_cap = plan.cap.tokens as f64;
    shown.cap_set_here = plan.cap.from == grades::cap::From::Yours;
    shown.limit_usd = plan.cap.cost_usd(&now.on.priced);
    shown.refused = refused(stopped, &now);
    if shown.refused.is_none() {
        shown.consent = Some(resumable::consent_of(
            stopped,
            &now.deciding,
            &plan,
            &terms(&plan.cap, &now.on),
        ));
    }
    shown
}

/// **What resuming the stopped grade `grade` would run** (#5718): `None` where this machine keeps
/// no stopped grade of that id (it finished, was resumed, a newer grade of its copy began, or it
/// ran on another machine).
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_resume_plan(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    grade: String,
) -> Result<Option<SkillGradeResumePlan>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a resume would run", move || {
        Ok(
            resumable::of(held.root(), &grade)
                .map(|stopped| plan_of(held.root(), &plane, &stopped)),
        )
    })
    .await
}

/// **Resumes the stopped grade `grade`** (#5718), on the plan whose consent `consent` is: only its
/// unfinished cases run, held to a new cap of their own. The answer is the new grade's id.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_resume(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    grade: String,
    consent: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("resuming a grade", move || {
        begin_resume(app, held, plane, &grade, &consent, None)
    })
    .await
}

/// **Resumes the stopped grade `grade`** in `plane`, held, on the plan whose consent is `consent`,
/// alone or as one of the batch `batch`: refused where it cannot be resumed now, where it waits
/// for a usage limit's reset, or where its plan changed since it was shown.
pub(crate) fn begin_resume(
    app: tauri::AppHandle,
    held: std::sync::Arc<crate::planes::Held>,
    plane: PlaneId,
    grade: &str,
    consent: &str,
    batch: Option<super::InBatch>,
) -> Result<String, String> {
    let stopped = resumable::of(held.root(), grade).ok_or(
        "That grade cannot be resumed now: it was resumed or graded again, so nothing was \
         started.",
    )?;
    let shown = plan_of(held.root(), &plane, &stopped);
    if let Some(why) = shown.refused {
        return Err(why);
    }
    if let Some(at) = shown.waits_until {
        return Err(format!(
            "Resume waits until the usage limit resets ({at}), so nothing was started."
        ));
    }
    if shown.consent.as_deref() != Some(consent) {
        return Err(
            "What a resume would run changed since its plan was shown, so nothing was started. \
             Read the plan again."
                .to_owned(),
        );
    }
    let now = read_now(held.root(), &stopped)?;
    let draft = now.draft.as_ref().map(|draft| DraftBegun {
        id: stopped.draft_of.clone().unwrap_or_default(),
        mark: grades::draft::Mark::of(draft, now.planned.plan.map_or(0, |plan| plan.chats)),
    });
    begin(
        app,
        held,
        Begin {
            plane,
            level: now.level,
            skill: stopped.skill.clone(),
            profile: stopped.deciding.profile.clone(),
            model: stopped.deciding.model.clone(),
            after_edit: false,
            grader: None,
            follower: None,
            draft,
            batch,
            resume: Some(stopped),
        },
        now.planned,
    )
}

/// **The Skill studio's Needs you items of the stopped grades of the project at `root`** (#5718,
/// the operator's ruling 5): each that the person did not stop themselves, saying why it stopped
/// and when Resume can run. A press opens its scorecard, where Resume is; nothing resumes by
/// itself.
pub(crate) fn needs(root: &std::path::Path) -> Vec<crate::skillstudio::needs::NeedsYou> {
    needs_of(resumable::all(root), chrono::Utc::now())
}

/// [`needs`], of the stopped grades `all`, at `now`.
fn needs_of(
    all: Vec<resumable::Stopped>,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<crate::skillstudio::needs::NeedsYou> {
    use crate::skillstudio::needs::{NeedFix, NeedKind, NeedsYou};
    all.into_iter()
        .filter(|stopped| stopped.ended != purlis_core::skills::grade::GradeEnd::Cancelled)
        .map(|stopped| {
            let ended = grades::ended_said_on(stopped.ended, stopped.deciding.program.as_deref());
            let when = match still_ahead(stopped.resets_at.as_deref(), now) {
                Some(at) => format!("Resume can run after your limit resets at {}.", clock(&at)),
                None => "Resume can run now.".to_owned(),
            };
            NeedsYou {
                id: format!("resume:{}", stopped.grade),
                kind: NeedKind::Resume,
                title: stopped.skill.clone(),
                tag: stopped.draft.as_ref().map(|_| "its draft".to_owned()),
                reason: format!(
                    "{ended}. {when} It runs {} left and keeps what ran.",
                    match stopped.left() {
                        1 => "the 1 chat".to_owned(),
                        many => format!("the {many} chats"),
                    },
                ),
                fix: NeedFix::OpenGrade {
                    name: stopped.skill.clone(),
                    grade: stopped.grade.clone(),
                    draft: stopped.draft_of.clone(),
                },
                fix_label: "Open the grade".to_owned(),
                fix_does: "Opens its scorecard, where Resume shows its plan before anything runs."
                    .to_owned(),
            }
        })
        .collect()
}

/// The RFC 3339 time `at` on this machine's clock, as a person reads it: `14:00`, or with its day
/// where it is not today.
fn clock(at: &str) -> String {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(at) else {
        return at.to_owned();
    };
    let local = at.with_timezone(&chrono::Local);
    if local.date_naive() == chrono::Local::now().date_naive() {
        local.format("%H:%M").to_string()
    } else {
        local.format("%a %H:%M").to_string()
    }
}

#[cfg(test)]
mod tests;
