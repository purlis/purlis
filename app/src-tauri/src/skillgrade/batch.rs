// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grade, for several skills at once** (#5356, the operator's ruling of 2026-11-05): the
//! window's half of `purlis_core::skills::grades::batch`, which the Library's Select mode asks.
//!
//! - **The estimate comes first** ([`skill_grades_plan`]): each skill's plan as Grade would show
//!   it, on the project's default profile and model, its trigger tests alone, each with what its
//!   last grade spent worked out for this one; those with nothing to go on named; and those left
//!   out, each with why (a skill with no tests among them).
//! - **A few run at a time** ([`skill_grades_start`]): [`AT_ONCE`] grades together, each a
//!   grade of its own, started as a press of Grade starts one, through the one check a press
//!   takes, on the consent its plan showed. Each is held to its own limit besides.
//! - **The person's limit holds them all**: once what they spent together reaches it, none
//!   starts and those running are stopped, which scores and records what they ran. A grade that
//!   ended is kept. Stop ([`skill_grades_stop`]) does the same at once.
//! - **The window is told each step** ([`EVENT`]), and a page opened while it runs asks where it
//!   is ([`skill_grades_running`]). While a batch runs, no other grade starts in the project.
//!
//! A batch is held in memory alone: a quit ends it, and each of its grades is recorded as a quit
//! leaves any grade (`grades::underway`). It is no store of its own.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::Emitter as _;

use purlis_core::skills::grades::batch::{self as core, Batch, Ending, NotRun, State};

use super::{
    Begin, GradeEnded, GradeEstimate, InBatch, SkillGradePlan, SkillGradeStep, begin, chats_on,
    grades, plan_by, plan_holds, running,
};
use crate::planes::{Held, PlaneId, Planes};
use crate::skillchange::SkillLevel;
use purlis_core::skills::verbs::Level;

/// The event the window is told each step of a batch by.
pub const EVENT: &str = "skill-grades";

/// **How many of a batch's grades run together.**
pub const AT_ONCE: u32 = 3;

/// What Select's Grade says of a project already grading several skills.
const A_BATCH_RUNS: &str = "Several skills are being graded in this project: stop them, or wait \
     for them to end.";

/// **A skill a batch grades, as its estimate shows it**: its own plan's chats, limit and consent,
/// and what its last grade spent, worked out for this one, where there is one.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct BatchSkill {
    pub level: SkillLevel,
    pub name: String,
    pub chats: u32,
    /// The most its own grade may spend, in dollars: its plan's limit.
    pub limit_usd: f64,
    pub estimate: Option<GradeEstimate>,
    /// What pressing Start consents to for it: handed back, and a grade on another plan is
    /// refused.
    pub consent: String,
    /// **The stopped grade it resumes** (#5718), where the batch resumes one: its chats are those
    /// left, and its estimate what they would spend.
    pub resume: Option<String>,
}

/// **A skill a batch leaves out**, and why, in a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct BatchLeftOut {
    pub level: SkillLevel,
    pub name: String,
    pub why: String,
}

/// **What grading several skills would run and spend**, before anything runs.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradesPlan {
    pub skills: Vec<BatchSkill>,
    pub left_out: Vec<BatchLeftOut>,
    /// The estimates known, added, how many there are, and the names of those with none.
    pub known_usd: f64,
    pub known: u32,
    pub unknown: Vec<String>,
    /// Each grade's own limit, added: the most the batch could spend whatever limit is set.
    pub most_usd: f64,
    pub at_once: u32,
    /// The profile the chats run on, and why no grade can run on it, where none can.
    pub profile: Option<String>,
    pub refused: Option<String>,
    /// Several skills of this project being graded already, where they are.
    pub running: Option<String>,
}

/// **Why `plan` leaves its skill out of a batch**, or `None` where a batch grades it.
fn left_out_why(plan: &SkillGradePlan) -> Option<String> {
    if let Some(why) = &plan.tests_refused {
        return Some(format!("purlis could not read its trigger tests: {why}"));
    }
    if plan.tests.is_empty() {
        return Some(if plan.evals.is_empty() {
            "It has no tests.".to_owned()
        } else {
            "It has outcome evals but no trigger tests, and outcome evals run only from its own \
             Grades tab."
                .to_owned()
        });
    }
    if plan.running.is_some() {
        return Some("A grade of it is running already.".to_owned());
    }
    if plan.consent.is_none() {
        return Some("purlis could not say what its grade would run.".to_owned());
    }
    None
}

impl SkillGradesPlan {
    /// The estimate of grading each copy of `plans`, in their order, each its plan or why it was
    /// not read.
    pub(crate) fn of(plans: Vec<(SkillLevel, String, Result<SkillGradePlan, String>)>) -> Self {
        let mut skills = Vec::new();
        let mut left_out = Vec::new();
        let mut profile = None;
        let mut refused = None;
        for (level, name, plan) in plans {
            let plan = match plan {
                Ok(plan) => plan,
                Err(why) => {
                    left_out.push(BatchLeftOut { level, name, why });
                    continue;
                }
            };
            profile = profile.or_else(|| plan.profile.clone());
            refused = refused.or_else(|| plan.refused.clone());
            if let Some(why) = left_out_why(&plan) {
                left_out.push(BatchLeftOut { level, name, why });
                continue;
            }
            skills.push(BatchSkill {
                level,
                name,
                chats: plan.chats,
                limit_usd: plan.cost_cap_usd,
                estimate: plan.estimate.clone(),
                consent: plan.consent.clone().unwrap_or_default(),
                resume: None,
            });
        }
        Self::of_skills(skills, left_out, profile, refused)
    }

    /// **The estimate of grading `skills`**, leaving `left_out` out, its chats on `profile` or
    /// refused there with `refused`: their estimates added, and each one's own limit.
    fn of_skills(
        skills: Vec<BatchSkill>,
        left_out: Vec<BatchLeftOut>,
        profile: Option<String>,
        refused: Option<String>,
    ) -> Self {
        let each: Vec<Option<f64>> = skills
            .iter()
            .map(|one| one.estimate.as_ref().and_then(|estimate| estimate.cost_usd))
            .collect();
        let estimate = core::estimate(&each);
        Self {
            unknown: skills
                .iter()
                .zip(&each)
                .filter(|(_, cost)| cost.is_none())
                .map(|(one, _)| one.name.clone())
                .collect(),
            most_usd: skills.iter().map(|one| one.limit_usd).sum(),
            known_usd: estimate.known_usd,
            known: estimate.known,
            skills,
            left_out,
            at_once: AT_ONCE,
            profile,
            refused,
            running: None,
        }
    }
}

/// **A skill a batch is asked to grade**: its copy, and the consent its estimate showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BatchPick {
    pub level: SkillLevel,
    pub name: String,
    pub consent: String,
    /// **The stopped grade it resumes** (#5718), where Select's Resume picked it: only its
    /// unfinished cases run.
    #[serde(default)]
    pub resume: Option<String>,
}

/// **Where a skill of a batch is**, as the window is told it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum BatchState {
    Waiting,
    Running,
    /// Its grade ended: its `ended` says how.
    Ended,
    /// It was never graded, and why, in a sentence.
    NotRun {
        why: String,
    },
}

impl From<&State> for BatchState {
    fn from(state: &State) -> Self {
        match state {
            State::Waiting => Self::Waiting,
            State::Running => Self::Running,
            State::Ended => Self::Ended,
            State::NotRun { why } => Self::NotRun {
                why: match why {
                    NotRun::Limit => "The limit was reached first.".to_owned(),
                    NotRun::Stopped => "Stopped before it started.".to_owned(),
                    NotRun::SpendNotReported => "A chat did not say what it spent, so the limit \
                                                 could not hold."
                        .to_owned(),
                    NotRun::Refused(why) => why.clone(),
                },
            },
        }
    }
}

/// **How a batch ended**, as the window is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum BatchEnded {
    Whole,
    Limit,
    Stopped,
    SpendNotReported,
}

impl From<Ending> for BatchEnded {
    fn from(ending: Ending) -> Self {
        match ending {
            Ending::Whole => Self::Whole,
            Ending::Limit => Self::Limit,
            Ending::Stopped => Self::Stopped,
            Ending::SpendNotReported => Self::SpendNotReported,
        }
    }
}

/// **One skill of a batch, at a step**: where it is, its grade's id once it started, how many of
/// its chats ran of how many, what they spent, and how it ended.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct BatchSkillStep {
    pub level: SkillLevel,
    pub name: String,
    pub state: BatchState,
    pub grade: Option<String>,
    pub ran: u32,
    pub of: u32,
    pub cost_usd: Option<f64>,
    pub ended: Option<GradeEnded>,
}

/// **One step of a batch**, as [`EVENT`] tells the window.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradesStep {
    pub plane: PlaneId,
    pub batch: String,
    pub limit_usd: f64,
    pub spent_usd: f64,
    pub skills: Vec<BatchSkillStep>,
    /// How it ended, once it has.
    pub ended: Option<BatchEnded>,
}

/// What a batch's own skill holds beside the core's state.
struct Of {
    pick: BatchPick,
    grade: Option<String>,
    ran: u32,
    of: u32,
    cost_usd: Option<f64>,
    ended: Option<GradeEnded>,
}

/// **A batch being driven**: the core's decisions, and what each skill's grade said.
pub(crate) struct Driving {
    id: String,
    plane: PlaneId,
    pub(crate) batch: Batch,
    skills: Vec<Of>,
}

impl Driving {
    pub(crate) fn new(id: String, plane: PlaneId, picks: Vec<BatchPick>, limit_usd: f64) -> Self {
        Self {
            batch: Batch::new(picks.len(), limit_usd, AT_ONCE as usize),
            skills: picks
                .into_iter()
                .map(|pick| Of {
                    pick,
                    grade: None,
                    ran: 0,
                    of: 0,
                    cost_usd: None,
                    ended: None,
                })
                .collect(),
            id,
            plane,
        }
    }

    /// The skill at `at` began as the grade `grade`.
    pub(crate) fn begun(&mut self, at: usize, grade: String) {
        if let Some(one) = self.skills.get_mut(at) {
            one.grade = Some(grade);
        }
    }

    /// **The grade of the skill at `at` said a step**: `ran` of `of` chats, `cost_usd` spent so
    /// far, and how it ended, once it has. `true` where the batch's grades running are to be
    /// stopped now.
    pub(crate) fn told(
        &mut self,
        at: usize,
        ran: u32,
        of: u32,
        cost_usd: Option<f64>,
        ended: Option<GradeEnded>,
    ) -> bool {
        let Some(one) = self.skills.get_mut(at) else {
            return false;
        };
        // An end that says nothing of what ran (a fault in purlis) keeps what its steps said.
        (one.ran, one.of, one.cost_usd) = (ran.max(one.ran), of, cost_usd.or(one.cost_usd));
        let stop = self.batch.spent(at, ran, cost_usd);
        if let Some(ended) = ended {
            one.ended = Some(ended);
            self.batch.ended(at, one.cost_usd.unwrap_or_default());
        }
        stop
    }

    /// Where it is now, as the window is told it.
    pub(crate) fn step(&self) -> SkillGradesStep {
        SkillGradesStep {
            plane: self.plane.clone(),
            batch: self.id.clone(),
            limit_usd: self.batch.limit_usd(),
            spent_usd: self.batch.spent_usd(),
            skills: self
                .skills
                .iter()
                .zip(self.batch.states())
                .map(|(one, state)| BatchSkillStep {
                    level: one.pick.level.clone(),
                    name: one.pick.name.clone(),
                    state: state.into(),
                    grade: one.grade.clone(),
                    ran: one.ran,
                    of: one.of,
                    cost_usd: one.cost_usd,
                    ended: one.ended,
                })
                .collect(),
            ended: self.batch.ending().map(Into::into),
        }
    }
}

/// What a batch's driver hears.
enum Heard {
    /// A step of the grade of the skill at its place.
    Step(usize, Box<SkillGradeStep>),
    /// The person pressed Stop.
    Stop,
}

/// **A batch running**: where, what tells its driver, and its latest step.
struct Live {
    plane: PlaneId,
    tell: Sender<Heard>,
    step: SkillGradesStep,
}

/// **The batches running in this app**, by id: one per project at a time.
static BATCHES: Mutex<Option<HashMap<String, Live>>> = Mutex::new(None);

fn batches<T>(with: impl FnOnce(&mut HashMap<String, Live>) -> T) -> T {
    let mut all = BATCHES.lock().unwrap_or_else(PoisonError::into_inner);
    with(all.get_or_insert_with(HashMap::new))
}

/// **Why no other grade starts in `plane`**, where a batch runs there: what [`begin`] refuses
/// a grade of no batch with.
pub(crate) fn holds(plane: &PlaneId) -> Option<&'static str> {
    batches(|all| all.values().any(|one| one.plane == *plane)).then_some(A_BATCH_RUNS)
}

/// The batch running in `plane`, at its latest step.
fn running_in(plane: &PlaneId) -> Option<SkillGradesStep> {
    batches(|all| {
        all.values()
            .find(|one| one.plane == *plane)
            .map(|one| one.step.clone())
    })
}

/// **Stops every grade of the batch `id` that runs**: each scores and records what it ran.
fn stop_its_grades(id: &str) {
    for one in running().values() {
        if one.batch.as_deref() == Some(id) {
            one.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// **What grading the copies `skills` would run and spend**, before anything runs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades_plan(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    skills: Vec<(SkillLevel, String)>,
) -> Result<SkillGradesPlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what grading several skills runs", move || {
        let root = held.root();
        let plans = skills
            .into_iter()
            .map(|(level, name)| {
                let on = chats_on(root, None, None);
                // On the profile its chats run on, so its estimate says whether its last grade
                // ran there too.
                let estimated_on = on.profile.clone();
                let plan = grades::planned_with(
                    root,
                    &(&level).into(),
                    &name,
                    estimated_on.as_deref(),
                    &[],
                )
                .map(|planned| {
                    plan_by(
                        &planned,
                        super::running_of(&plane, &name),
                        on,
                        Vec::new(),
                        None,
                    )
                });
                (level, name, plan)
            })
            .collect();
        let mut plan = SkillGradesPlan::of(plans);
        plan.running = holds(&plane).map(str::to_owned);
        Ok(plan)
    })
    .await
}

/// **What resuming the stopped grades `grades` together would run and spend** (#5718, the
/// operator's ruling 6): each stopped skill's chats left and what they would spend, under one new
/// limit the person sets; a grade that cannot be resumed now is left out with why.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades_resume_plan(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    grades: Vec<String>,
) -> Result<SkillGradesPlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what resuming several grades runs", move || {
        flag_on()?;
        let root = held.root();
        let mut skills = Vec::new();
        let mut left_out = Vec::new();
        let mut profile = None;
        for grade in grades {
            let Some(stopped) = purlis_core::skills::grades::resumable::of(root, &grade) else {
                continue;
            };
            let level = stopped
                .level
                .as_deref()
                .and_then(purlis_core::skills::grades::level_of_recorded)
                .map_or(SkillLevel::Project { local: false }, |level| {
                    (&level).into()
                });
            let shown = super::resumed::plan_of(root, &plane, &stopped);
            let why = shown.refused.clone().or_else(|| {
                shown
                    .waits_until
                    .as_ref()
                    .map(|_| "It waits until its usage limit resets.".to_owned())
            });
            if let Some(why) = why.or_else(|| {
                shown
                    .running
                    .as_ref()
                    .map(|_| "A grade of it is running already.".to_owned())
            }) {
                left_out.push(BatchLeftOut {
                    level,
                    name: stopped.skill.clone(),
                    why,
                });
                continue;
            }
            profile = profile.or_else(|| shown.profile.clone());
            skills.push(BatchSkill {
                level,
                name: stopped.skill.clone(),
                chats: shown.left,
                limit_usd: shown.limit_usd,
                estimate: Some(GradeEstimate {
                    tokens: shown.tokens_left,
                    cost_usd: shown.usd_left,
                    chats_then: shown.left,
                    ts: stopped.ts.clone(),
                    profile: shown.profile.clone(),
                    harness: stopped.deciding.program.clone(),
                    model: shown.model.clone(),
                    same_profile: Some(true),
                    interrupted: false,
                }),
                consent: shown.consent.clone().unwrap_or_default(),
                resume: Some(stopped.grade.clone()),
            });
        }
        let mut plan = SkillGradesPlan::of_skills(skills, left_out, profile, None);
        plan.running = holds(&plane).map(str::to_owned);
        Ok(plan)
    })
    .await
}

/// **Grades the copies `skills`**, a few at a time, under `limit_usd` (see the module). The
/// answer is the batch's id. Refused while any grade runs in the project.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    skills: Vec<BatchPick>,
    limit_usd: f64,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting to grade several skills", move || {
        if !(limit_usd.is_finite() && limit_usd > 0.0) {
            return Err("Set a limit above $0.00: nothing was started.".to_owned());
        }
        if skills.is_empty() {
            return Err("No skill was picked to grade: nothing was started.".to_owned());
        }
        let id = purlis_core::reopen::mint();
        let (tell, heard) = channel();
        let driving = Driving::new(id.clone(), plane.clone(), skills, limit_usd);
        // The grades running are read while the batches are held, always in that order: a grade
        // that starts alone after this sees the batch, and its own start refuses it.
        batches(|all| {
            if all.values().any(|one| one.plane == plane) {
                return Err(A_BATCH_RUNS.to_owned());
            }
            if running().values().any(|one| one.plane == plane) {
                return Err(super::A_GRADE_RUNS.to_owned());
            }
            all.insert(
                id.clone(),
                Live {
                    plane: plane.clone(),
                    tell: tell.clone(),
                    step: driving.step(),
                },
            );
            Ok(())
        })?;
        std::thread::Builder::new()
            .name("purlis-grades".to_owned())
            .spawn({
                let id = id.clone();
                move || {
                    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drive(app, held, driving, tell, heard)
                    }));
                    // A fault in the driver leaves no batch holding the project: its grades
                    // stop and end as grades do, and the Library can start others.
                    if ran.is_err() {
                        tracing::warn!("purlis: the batch of grades {id} ended on a fault");
                        batches(|all| all.remove(&id));
                        stop_its_grades(&id);
                    }
                }
            })
            .map_err(|why| {
                batches(|all| all.remove(&id));
                format!("The grades did not start: {why}")
            })?;
        Ok(id)
    })
    .await
}

/// **Stops the batch `batch`**: none of its skills starts, and its grades running stop.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades_stop(plane: PlaneId, batch: String) -> Result<(), String> {
    batches(|all| match all.get(&batch) {
        Some(one) if one.plane == plane => {
            let _ = one.tell.send(Heard::Stop);
            Ok(())
        }
        _ => Err("Those grades are not running any more.".to_owned()),
    })
}

/// **The batch running in `plane`**, at its latest step, where one runs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades_running(
    plane: PlaneId,
) -> Result<Option<SkillGradesStep>, String> {
    Ok(running_in(&plane))
}

/// **Starts the skill at `at` of a batch**: its plan read again and held to the consent its
/// estimate showed, then begun as a press of Grade begins one, its steps told to the driver.
fn start_one(
    app: &tauri::AppHandle,
    held: &Arc<Held>,
    driving: &Driving,
    at: usize,
    tell: &Sender<Heard>,
) -> Result<String, String> {
    let pick = &driving.skills[at].pick;
    let in_batch = {
        let tell = tell.clone();
        InBatch {
            id: driving.id.clone(),
            told: Box::new(move |step: &SkillGradeStep| {
                let _ = tell.send(Heard::Step(at, Box::new(step.clone())));
            }),
        }
    };
    // A stopped grade it resumes runs only what it left, on its own plan (#5718).
    if let Some(grade) = &pick.resume {
        return super::resumed::begin_resume(
            app.clone(),
            Arc::clone(held),
            driving.plane.clone(),
            grade,
            &pick.consent,
            Some(in_batch),
        );
    }
    let root = held.root();
    let level: Level = (&pick.level).into();
    let on = chats_on(root, None, None);
    let planned = grades::planned_with(root, &level, &pick.name, None, &[])?;
    plan_holds(
        &planned,
        &on,
        None,
        &pick.consent,
        &pick.name,
        "it was not started",
    )?;
    begin(
        app.clone(),
        Arc::clone(held),
        Begin {
            plane: driving.plane.clone(),
            level,
            skill: pick.name.clone(),
            profile: None,
            model: None,
            after_edit: false,
            grader: None,
            follower: None,
            draft: None,
            batch: Some(in_batch),
            resume: None,
        },
        planned,
    )
}

/// **Drives a batch to its end**: starts what it may, hears each grade's steps, stops its
/// grades when it says so, and tells the window each step.
fn drive(
    app: tauri::AppHandle,
    held: Arc<Held>,
    mut driving: Driving,
    tell: Sender<Heard>,
    heard: Receiver<Heard>,
) {
    let told = |driving: &Driving| {
        let step = driving.step();
        batches(|all| {
            if let Some(one) = all.get_mut(&driving.id) {
                one.step = step.clone();
            }
        });
        let _ = app.emit(EVENT, step);
    };
    loop {
        while let Some(at) = driving.batch.start_next() {
            match start_one(&app, &held, &driving, at, &tell) {
                Ok(grade) => driving.begun(at, grade),
                Err(why) => driving.batch.refused(at, why),
            }
        }
        if driving.batch.ending().is_some() {
            break;
        }
        told(&driving);
        let stop = match heard.recv() {
            Ok(Heard::Step(at, step)) => {
                driving.told(at, step.ran, step.of, step.cost_usd, step.ended)
            }
            Ok(Heard::Stop) => {
                driving.batch.stop();
                true
            }
            // Every sender is held here: never.
            Err(_) => break,
        };
        if stop {
            stop_its_grades(&driving.id);
        }
    }
    batches(|all| all.remove(&driving.id));
    told(&driving);
}

#[cfg(test)]
mod tests;
