// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grading a draft from its page** (#2780; ADR 0105 as amended 2026-11-02): the window's half of
//! `purlis_core::skills::grades::draft`. The plan, the running page and the scorecard are a
//! skill's grade's, so a draft is planned, consented to and held to its limits the same way; only
//! what is graded differs. [`skill_draft_grade`] says what the draft's page shows beside Accept,
//! which no state of a grade ever blocks.

use std::sync::atomic::Ordering;

use serde::Serialize;
use specta::Type;
use tauri::Manager as _;

use purlis_core::skills::grades::{self, Graded};
use purlis_core::skills::verbs::drafts;

use super::{
    Begin, ByGrader, DraftBegun, GradeArms, GradeGrader, GradeRow, RunningStep, SkillGradePlan,
    SkillGradeStep, begin, chats_on, evals_can_run, flag_on, grader_picked, grader_takes,
    graders_here, plan_by, plan_holds, running, set_cap, tests_said, ticked,
};
use crate::planes::{PlaneId, Planes};

/// **What a draft's grade measured of one kind of fixture**: each arm's rate in points, and the
/// draft's difference from what it is weighed against, which the core leaves out where that arm
/// did not run.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct DraftScore {
    /// Its outcome evals, rather than its trigger tests.
    pub evals: bool,
    pub arms: GradeArms,
    pub delta: Option<f64>,
}

impl DraftScore {
    /// What `graded` measured: its trigger tests', and its outcome evals', each where the draft's
    /// arm scored any.
    fn all_of(graded: &Graded) -> Vec<DraftScore> {
        let points = |delta: Option<f64>| delta.map(|delta| (delta * 100.0).round());
        [
            DraftScore {
                evals: false,
                arms: GradeArms::of(graded),
                delta: points(graded.report.delta),
            },
            DraftScore {
                evals: true,
                arms: GradeArms::by(graded, |score| score.outcome_pass_rate),
                delta: points(graded.report.outcome_delta),
            },
        ]
        .into_iter()
        .filter(|score| score.arms.with.is_some())
        .collect()
    }
}

/// **What a draft's page says of its grade.**
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillDraftGrade {
    /// It changes only the skill's tests, so it gets no grade.
    pub only_evals: bool,
    /// What a grade of it would run, counted, and why its tests were not read, where they were
    /// not.
    pub tests: u32,
    pub evals: u32,
    pub tests_refused: Option<String>,
    /// Its newest grade, what that measured, the chats its plan ran, and whether the draft
    /// changed since.
    pub latest: Option<GradeRow>,
    pub scores: Vec<DraftScore>,
    pub asked: u32,
    pub changed: bool,
    /// The grade of it running now, at its latest step.
    pub running: Option<SkillGradeStep>,
}

impl SkillDraftGrade {
    fn of(said: grades::draft::Said, running: Option<SkillGradeStep>) -> Self {
        let (tests, evals, tests_refused) = tests_said(said.tests);
        let latest = said.latest.as_deref();
        SkillDraftGrade {
            only_evals: said.only_evals,
            tests,
            evals,
            tests_refused,
            latest: latest.map(GradeRow::of),
            scores: latest.map_or_else(Vec::new, DraftScore::all_of),
            asked: latest
                .and_then(|row| row.draft.as_ref())
                .map_or(0, |mark| mark.chats),
            changed: said.changed,
            running,
        }
    }
}

/// The latest step of the grade of the draft `draft` running in `plane` now, where one is.
fn step_of(plane: &PlaneId, draft: &str) -> Option<SkillGradeStep> {
    running()
        .values()
        .filter(|one| one.plane == *plane && one.draft.as_deref() == Some(draft))
        .find_map(|one| match &one.step {
            RunningStep::Skill(step) => Some(step.clone()),
            RunningStep::Router(_) => None,
        })
}

/// **Stops the grade of the draft `draft`, where one runs**: a draft accepted or discarded has no
/// page to show it, and nothing would read what it goes on spending for.
pub(crate) fn stop(plane: &PlaneId, draft: &str) {
    for one in running().values() {
        if one.plane == *plane && one.draft.as_deref() == Some(draft) {
            one.stop.store(true, Ordering::SeqCst);
        }
    }
}

/// **What the page of the draft `draft` says of its grade.**
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_draft_grade(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    draft: String,
) -> Result<SkillDraftGrade, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a draft's grade", move || {
        flag_on()?;
        let one = drafts::one(held.root(), &draft)?;
        Ok(SkillDraftGrade::of(
            grades::draft::of(held.root(), &one),
            step_of(&plane, &draft),
        ))
    })
    .await
}

/// The plan of the draft `draft` of the project at `root`, as the window draws it.
fn plan_of_draft(
    root: &std::path::Path,
    plane: &PlaneId,
    draft: &str,
    (profile, model): (Option<String>, Option<String>),
    evals: &[String],
    (offered, grader): (Vec<GradeGrader>, Option<&str>),
) -> Result<SkillGradePlan, String> {
    let one = drafts::one(root, draft)?;
    let on = chats_on(root, profile, model);
    let planned = grades::draft::planned(root, &one, on.profile.as_deref(), &ticked(evals))?;
    let mut plan = plan_by(
        &planned,
        step_of(plane, draft).map(|step| step.grade),
        on,
        offered,
        grader,
    );
    plan.draft = true;
    Ok(plan)
}

/// **What grading the draft `draft` would run**, before anything runs.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_draft_grade_plan(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    draft: String,
    profile: Option<String>,
    model: Option<String>,
    evals: Vec<String>,
    grader: Option<String>,
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reading what a grade runs", move || {
        flag_on()?;
        plan_of_draft(
            held.root(),
            &plane,
            &draft,
            (profile, model),
            &evals,
            (graders_here(&executor, held.root()), grader.as_deref()),
        )
    })
    .await
}

/// **Sets the token cap a grade in the project is held to**, as `skill_grade_set_cap` does: the
/// answer is the plan of the draft `draft` again, with it.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_draft_grade_set_cap(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    draft: String,
    profile: Option<String>,
    model: Option<String>,
    evals: Vec<String>,
    grader: Option<String>,
    tokens: f64,
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("setting a grade's cap", move || {
        flag_on()?;
        set_cap(held.root(), tokens)?;
        plan_of_draft(
            held.root(),
            &plane,
            &draft,
            (profile, model),
            &evals,
            (graders_here(&executor, held.root()), grader.as_deref()),
        )
    })
    .await
}

/// **Grades the draft `draft`**, on the plan the person pressed Start on: see
/// `skill_grade_start`. The answer is the grade's id.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_draft_grade_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    draft: String,
    profile: Option<String>,
    model: Option<String>,
    evals: Vec<String>,
    grader: Option<String>,
    consent: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    let executor = app.state::<crate::views::Views>().executor();
    crate::off_the_window("starting a grade", move || {
        flag_on()?;
        let one = drafts::one(held.root(), &draft)?;
        let on = chats_on(held.root(), profile.clone(), model.clone());
        let planned = grades::draft::planned(held.root(), &one, None, &ticked(&evals))?;
        let picked = grader_picked(&graders_here(&executor, held.root()), grader.as_deref())?;
        grader_takes(&planned, picked.is_some())?;
        evals_can_run(&planned, on.evals_refused.as_ref())?;
        plan_holds(
            &planned,
            &on,
            picked.as_ref(),
            &consent,
            &one.name,
            "nothing was started",
        )?;
        let chats = planned.plan.map_or(0, |plan| plan.chats);
        let mark = grades::draft::Mark::of(&one, chats);
        begin(
            app,
            held,
            Begin {
                plane,
                level: one.level,
                skill: one.name,
                profile,
                model,
                after_edit: false,
                grader: picked.map(|one| ByGrader {
                    id: one.id,
                    executor,
                }),
                follower: None,
                draft: Some(DraftBegun { id: draft, mark }),
            },
            planned,
        )
    })
    .await
}

#[cfg(test)]
mod tests;
