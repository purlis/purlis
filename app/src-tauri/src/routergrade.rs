// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A router's grade, where the router is chosen** (SK-15e, #3233; ADR 0102 §8): the window's
//! half of `purlis_core::skills::grades::routers`.
//!
//! - **The cost comes first** ([`router_grade_plan`]): what grading the router chosen by the
//!   studio's workspace or persona runs, on the library a chat there is handed, before anything
//!   runs.
//! - **It runs real chats**, as a skill's grade does (`skillgrade::InChats`): each of the
//!   library's trigger tests with the router in place and with the built-in router, every chat a
//!   grade's chat (`curation::start_graded`). In the router's arm, a chat whose start fell back on
//!   the built-in router is no run, and the grade says why.
//! - **One grade runs in a project at a time**, a skill's or a router's: they share
//!   `skillgrade`'s record of the grades running, and its Stop.
//! - **The grade goes to its record** with the library's commit, and the studio's router line says
//!   it ([`crate::skillstudio::StudioRouter::grade`]).
//!
//! Behind the `skill-studio` flag until its finish line (#2786).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use specta::Type;
use tauri::Emitter as _;

use purlis_core::skills::grade::routers::{self as engine, LibraryChats};
use purlis_core::skills::grade::{self as grading, Arm};
use purlis_core::skills::grades::routers::{self, RecordedCopy, RouterGraded, RouterPlanned};
use purlis_core::skills::grades::{self, Spent};

use crate::planes::{PlaneId, Planes};
use crate::skillchange::flag_on;
use crate::skillgrade::{
    GradeEnded, GradeLeftOut, GradePrice, InChats, Running, chats_on, held_to_its_end, running,
};

/// The event the window is told each step of a router's grade by.
pub const EVENT: &str = "router-grade";

/// **One skill of the library a router's grade grades**, as its plan shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct RouterGradeSkill {
    pub skill: String,
    /// Its trigger tests that run.
    pub tests: u32,
    pub left_out: Vec<GradeLeftOut>,
    /// Why its trigger tests could not be read, where they were not.
    pub refused: Option<String>,
}

/// **What grading a router would run, before anything runs** (decision 22).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct RouterGradePlan {
    pub router: String,
    /// The skills each chat is handed: the library a chat of this workspace or persona gets.
    pub library: u32,
    pub skills: Vec<RouterGradeSkill>,
    /// The chats it runs in all, each one turn, and how many times each test runs in each arm.
    pub chats: u32,
    pub runs: u32,
    pub chat_minutes: u32,
    /// The commit the library is, where every copy is committed: what the grade is recorded on.
    pub commit: Option<String>,
    pub token_cap: f64,
    pub cap_set_here: bool,
    pub cost_cap_usd: f64,
    pub priced: GradePrice,
    /// The profile its chats run on (the project's default), the sandbox they run in there, or
    /// why no grade can run on it.
    pub profile: Option<String>,
    pub sandbox: Option<String>,
    pub refused: Option<String>,
    /// A grade running in this project now, where one is: a skill's or a router's.
    pub running: Option<String>,
    pub consent: Option<String>,
}

impl RouterGradePlan {
    fn of(planned: &RouterPlanned, running: Option<String>, on: crate::skillgrade::On) -> Self {
        let crate::skillgrade::On {
            profile,
            checked,
            priced,
            ..
        } = on;
        RouterGradePlan {
            router: planned.router.clone(),
            library: u32::try_from(planned.library).unwrap_or(u32::MAX),
            skills: planned
                .skills
                .iter()
                .map(|one| RouterGradeSkill {
                    skill: one.skill.clone(),
                    tests: u32::try_from(one.tests).unwrap_or(u32::MAX),
                    left_out: one
                        .left_out
                        .iter()
                        .map(|left| GradeLeftOut {
                            at: u32::try_from(left.at).unwrap_or(u32::MAX),
                            why: left.why.clone(),
                        })
                        .collect(),
                    refused: one.refused.clone(),
                })
                .collect(),
            chats: planned.plan.map(|plan| plan.chats).unwrap_or_default(),
            runs: u32::from(grading::RUNS),
            chat_minutes: u32::try_from(grades::CHAT_TIME.as_secs() / 60).unwrap_or(u32::MAX),
            commit: planned.commit.clone(),
            token_cap: planned.cap.tokens as f64,
            cap_set_here: planned.cap.from == grades::cap::From::Yours,
            cost_cap_usd: planned.cap.cost_usd(&priced),
            priced: (&priced).into(),
            profile,
            refused: checked.as_ref().err().cloned(),
            sandbox: checked.ok(),
            running,
            consent: planned.consent.clone(),
        }
    }
}

/// **One step of a router's grade**, as the window is told it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct RouterGradeStep {
    pub plane: PlaneId,
    pub router: String,
    pub grade: String,
    pub ran: u32,
    pub of: u32,
    pub tokens: Option<f64>,
    pub cap: f64,
    pub ended: Option<GradeEnded>,
    /// What the window says of its end, where it says more than how it ended.
    pub said: Option<String>,
}

/// The grade running in `plane` now, a skill's or a router's, by id.
fn running_in(plane: &PlaneId) -> Option<String> {
    running()
        .iter()
        .find(|(_, one)| one.plane == *plane)
        .map(|(id, _)| id.clone())
}

/// The workspace and persona a studio is filtered to, each a plain name or none.
fn scope(workspace: Option<String>, persona: Option<String>) -> Result<Scope, String> {
    let workspace_ok = workspace
        .as_deref()
        .is_none_or(purlis_core::contain::workspace_name_ok);
    let persona_ok = persona
        .as_deref()
        .is_none_or(purlis_core::personas::valid_name);
    if !workspace_ok || !persona_ok {
        return Err("That is not a workspace or a persona of this project.".to_owned());
    }
    Ok(Scope { workspace, persona })
}

struct Scope {
    workspace: Option<String>,
    persona: Option<String>,
}

/// The router the scope chose, or why it chose none a grade can grade.
fn chosen(root: &std::path::Path, scope: &Scope) -> Result<String, String> {
    use purlis_core::skills::router::{Chosen, chosen};
    match chosen(root, scope.workspace.as_deref(), scope.persona.as_deref()) {
        Ok(Chosen::Extension(id)) => Ok(id),
        Ok(Chosen::BuiltIn) => Err(
            "Chats here are routed by the built-in router, so there is no other router to grade."
                .to_owned(),
        ),
        Err(why) => Err(format!("{why}.")),
    }
}

/// **What grading the router the studio's workspace or persona chose would run**, before
/// anything runs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn router_grade_plan(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
) -> Result<RouterGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a router's grade runs", move || {
        flag_on()?;
        let scope = scope(workspace, persona)?;
        let router = chosen(held.root(), &scope)?;
        let planned = routers::planned(
            held.root(),
            scope.workspace.as_deref(),
            scope.persona.as_deref(),
            &router,
        )?;
        let on = chats_on(held.root(), None, None);
        Ok(RouterGradePlan::of(&planned, running_in(&plane), on))
    })
    .await
}

/// **This app's host for a router's grade**: a skill's grade's host, told which skill's tests
/// run next.
impl LibraryChats for InChats<'_> {
    fn grading(&mut self, skill: &str) {
        skill.clone_into(&mut self.skill);
    }
}

/// **Grades the router the studio's workspace or persona chose**, on the project's default
/// profile: its chats run one at a time behind the window, which is told each step ([`EVENT`]).
/// The answer is the grade's id. Refused while a grade runs in the project, and where the plan
/// the person pressed Grade on no longer holds.
#[tauri::command]
#[specta::specta]
pub(crate) async fn router_grade_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    consent: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting a router's grade", move || {
        flag_on()?;
        let root = held.root().to_path_buf();
        let scope = scope(workspace, persona)?;
        let router = chosen(&root, &scope)?;
        let planned = routers::planned(
            &root,
            scope.workspace.as_deref(),
            scope.persona.as_deref(),
            &router,
        )?;
        if planned.consent.as_deref() != Some(consent.as_str()) {
            return Err(
                "The router's library or its trigger tests changed since the plan was shown, so \
                 nothing was started: read the plan again."
                    .to_owned(),
            );
        }
        let Some(request) = planned.request.clone() else {
            return Err(
                "No skill of this library has trigger tests to grade the router on, so nothing \
                 was started."
                    .to_owned(),
            );
        };
        let crate::skillgrade::On {
            profile,
            checked,
            priced,
            ..
        } = chats_on(&root, None, None);
        checked?;
        let profile = profile.ok_or("The grade did not start: no profile was found.")?;
        let id = purlis_core::reopen::mint();
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut all = running();
            if all.values().any(|one| one.plane == plane) {
                return Err(
                    "A grade is running in this project already: stop it, or wait for it to \
                     end."
                        .to_owned(),
                );
            }
            all.insert(
                id.clone(),
                Running {
                    plane: plane.clone(),
                    // No skill's: a router's grade is stopped by its id alone.
                    skill: String::new(),
                    stop: Arc::clone(&stop),
                    after_edit: None,
                    step: None,
                },
            );
        }
        let of = engine::plan(&request).chats;
        let cap = planned.cap.tokens;
        let limits = grading::Limits {
            tokens: cap,
            cost_usd: Some(planned.cap.cost_usd(&priced)),
            priced: Some(priced),
            unpriced_usd_per_million: Some(grades::price::DEAREST_USD_PER_MILLION),
        };
        let grade = id.clone();
        std::thread::Builder::new()
            .name("purlis-router-grade".to_owned())
            .spawn(move || {
                let tell = |ran: u32,
                            tokens: Option<u64>,
                            ended: Option<GradeEnded>,
                            said: Option<String>| {
                    let _ = app.emit(
                        EVENT,
                        RouterGradeStep {
                            plane: plane.clone(),
                            router: router.clone(),
                            grade: grade.clone(),
                            ran,
                            of,
                            tokens: tokens.map(|tokens| tokens as f64),
                            cap: cap as f64,
                            ended,
                            said,
                        },
                    );
                };
                tell(0, None, None, None);
                let run = || {
                    let snapshots: Vec<_> = planned
                        .copies
                        .iter()
                        .map(|one| one.copy.snapshot.clone())
                        .collect();
                    let lay_out = |arm: Arm| {
                        let routed_by = match arm {
                            Arm::Router => Some(router.as_str()),
                            Arm::BuiltInRouter => None,
                            _ => return Err("a router's grade has no such arm".to_owned()),
                        };
                        purlis_core::skills::verbs::drafts::lay_out_library_arm(
                            &root,
                            &snapshots,
                            routed_by,
                            &planned.pins,
                        )
                    };
                    let mut chats = InChats {
                        held: &held,
                        lay_out: &lay_out,
                        cost_left: limits.cost_usd,
                        unpriced_at: limits.unpriced_usd_per_million,
                        skill: String::new(),
                        label: format!("Grade the router {router}"),
                        // The scorecard of a router's grade is its line in the studio.
                        keeps_screens: false,
                        profile: &profile,
                        stop: &stop,
                        spent: Spent::default(),
                        tell: &|spent, _asked| tell(spent.chats, spent.tokens, None, None),
                        // No grade under way is held for a router's (#3616).
                        on_spent: &|_| {},
                        grade: &grade,
                        asked: 0,
                        harness: None,
                        plugins_off: None,
                        model: None,
                        reported: Vec::new(),
                    };
                    let report = engine::grade(&request, &mut chats, Some(limits.clone()));
                    let graded = RouterGraded {
                        v: routers::VERSION,
                        grade: grade.clone(),
                        router: router.clone(),
                        workspace: planned.workspace.clone(),
                        persona: planned.persona.clone(),
                        commit: planned.commit.clone(),
                        library: planned.copies.iter().map(RecordedCopy::of).collect(),
                        harness: chats.harness.take(),
                        profile: Some(profile.clone()),
                        ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
                        runs: grading::RUNS,
                        spent: chats.spent,
                        model: grades::reported_model(chats.reported.iter().map(String::as_str)),
                        report,
                    };
                    let recorded = routers::record(
                        &root,
                        &planned.copies,
                        &graded,
                        &purlis_core::machine::this_log_name(),
                    );
                    running().remove(&grade);
                    let said = match (&graded.report.not_started, recorded) {
                        (Some(why), _) => Some(why.clone()),
                        (None, None) => Some(
                            "The grade ran, but this project could not be written, so it was not \
                             recorded."
                                .to_owned(),
                        ),
                        (None, Some(_)) => Some(graded.said()),
                    };
                    tell(
                        graded.spent.chats,
                        graded.report.spent_tokens,
                        Some(graded.report.ended.into()),
                        said,
                    );
                };
                held_to_its_end(&grade, run, || {
                    tell(
                        0,
                        None,
                        Some(GradeEnded::Failed),
                        Some("A fault in purlis ended the router's grade before it was scored: grade it again.".to_owned()),
                    );
                });
            })
            .map_err(|why| {
                running().remove(&id);
                format!("The grade could not start ({why}).")
            })?;
        Ok(id)
    })
    .await
}

#[cfg(test)]
mod tests;
