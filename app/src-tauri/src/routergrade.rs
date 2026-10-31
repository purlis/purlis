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
//! - **It is held as under way while it runs** (#3616), as a skill's is (`grades::underway`): a
//!   crash, a fault or a quit before it ends leaves it to the project's next start to record as
//!   interrupted.
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
    GradeEnded, GradeEstimate, GradeLeftOut, GradeOutcome, GradePrice, GradeSpent, InChats, On,
    Running, ScoredTest, chats_on, held_to_its_end, not_kept, record_interrupted, running,
    scored_tests, set_cap, terms,
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
    /// The profile its chats run on (the one picked, else the project's default), the sandbox
    /// they run in there, or why no grade can run on it.
    pub profile: Option<String>,
    pub sandbox: Option<String>,
    pub refused: Option<String>,
    /// The models its harness offers for a grade, and the one picked (#3618, as SK-60's).
    pub models: Vec<String>,
    pub model: Option<String>,
    /// **What it should spend, from the router's last grade in this scope** (#3618), as a skill's
    /// plan works it out (#3273): null where it has none.
    pub estimate: Option<GradeEstimate>,
    /// Whether that grade ran on the library this one runs on: a chat is handed the whole
    /// library, so one of another may spend otherwise, and the plan says so. Null where there
    /// is no estimate.
    pub estimate_same_library: Option<bool>,
    /// A grade running in this project now, where one is: a skill's or a router's.
    pub running: Option<String>,
    /// What pressing Grade consents to: what it grades, and the profile, model, cap and dollar
    /// limit this plan shows (`grades::consent_on`).
    pub consent: Option<String>,
}

/// **What pressing Grade on `planned`, its chats `on`, consents to**: what it grades, and the
/// profile, model, token cap and dollar limit the plan shows, as a skill's plan's (#3618).
fn consent_shown(planned: &RouterPlanned, on: &On) -> Option<String> {
    planned
        .consent
        .as_deref()
        .map(|consent| grades::consent_on(consent, &terms(&planned.cap, on)))
}

impl RouterGradePlan {
    fn of(
        planned: &RouterPlanned,
        running: Option<String>,
        on: On,
        estimate: Option<routers::RouterEstimate>,
    ) -> Self {
        let consent = consent_shown(planned, &on);
        let On {
            profile,
            checked,
            priced,
            models,
            model,
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
            models,
            model,
            estimate: estimate.as_ref().map(|one| (&one.estimate).into()),
            estimate_same_library: estimate.map(|one| one.same_library),
            running,
            consent,
        }
    }
}

/// **The plan of grading the router `scope` chose**, its chats on `profile` and `model` (the
/// project's default and the profile's own where none is picked), with its estimate.
fn plan_of(
    root: &std::path::Path,
    plane: &PlaneId,
    scope: &Scope,
    profile: Option<String>,
    model: Option<String>,
) -> Result<RouterGradePlan, String> {
    let router = chosen(root, scope)?;
    let planned = routers::planned(
        root,
        scope.workspace.as_deref(),
        scope.persona.as_deref(),
        &router,
    )?;
    let on = chats_on(root, profile, model);
    let estimate = routers::estimate(root, &planned, on.profile.as_deref());
    Ok(RouterGradePlan::of(
        &planned,
        running_in(plane),
        on,
        estimate,
    ))
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
    profile: Option<String>,
    model: Option<String>,
) -> Result<RouterGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a router's grade runs", move || {
        flag_on()?;
        plan_of(
            held.root(),
            &plane,
            &scope(workspace, persona)?,
            profile,
            model,
        )
    })
    .await
}

/// **Sets the token cap a grade in the project is held to**, on this machine, as a skill's plan
/// sets it (#3618; the operator's ruling of 2026-10-26): one cap for every grade of the project.
/// The answer is the router's plan again, with it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn router_grade_set_cap(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    profile: Option<String>,
    model: Option<String>,
    tokens: f64,
) -> Result<RouterGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("setting a grade's cap", move || {
        flag_on()?;
        let scope = scope(workspace, persona)?;
        set_cap(held.root(), tokens)?;
        plan_of(held.root(), &plane, &scope, profile, model)
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

/// **Grades the router the studio's workspace or persona chose**, on `profile` (the project's
/// default where none is picked) and `model` (the profile's own where none is, #3618): its chats
/// run one at a time behind the window, which is told each step ([`EVENT`]). The answer is the
/// grade's id. Refused while a grade runs in the project, and where the plan the person pressed
/// Grade on no longer holds: what it grades, the profile, the model, the cap or the limit.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn router_grade_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    profile: Option<String>,
    model: Option<String>,
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
        // What runs is what the plan showed: its library and tests, on the profile, model and
        // limits it showed (#3618).
        let on = chats_on(&root, profile, model);
        if consent_shown(&planned, &on).as_deref() != Some(consent.as_str()) {
            return Err(
                "The router's plan changed since it was shown (its library or trigger tests, the \
                 profile or model its chats run on, or its limits), so nothing was started: read \
                 the plan again."
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
        let On {
            profile,
            checked,
            priced,
            model,
            ..
        } = on;
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
                    level: None,
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
        // Held as under way while it runs (#3616): what it would be recorded as were purlis to
        // stop under it now. Its lock is held while the grade runs (#3466).
        let under_way = RouterGraded {
            v: routers::VERSION,
            grade: grade.clone(),
            router: router.clone(),
            workspace: planned.workspace.clone(),
            persona: planned.persona.clone(),
            commit: planned.commit.clone(),
            library: planned.copies.iter().map(RecordedCopy::of).collect(),
            harness: None,
            profile: Some(profile.clone()),
            ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
            runs: grading::RUNS,
            report: routers::interrupted_report(
                &router,
                cap,
                limits.cost_usd,
                limits.priced.clone(),
            ),
            spent: Spent::default(),
            model: None,
        };
        let local = routers::library_is_local(&root, &planned.copies);
        let marker = match grades::underway::began_router(&root, local, &under_way) {
            Ok(marker) => Some(marker),
            Err(why) => {
                tracing::warn!(
                    "purlis: router grade {grade} is not held as under way ({why}): a crash \
                     before it ends loses it"
                );
                None
            }
        };
        let held_at = root.clone();
        std::thread::Builder::new()
            .name("purlis-router-grade".to_owned())
            .spawn(move || {
                let marker = std::cell::RefCell::new(marker);
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
                        // Each chat's screen, for the router's scorecard (#3617).
                        keeps: Some(kept_of(&planned)),
                        profile: &profile,
                        stop: &stop,
                        spent: Spent::default(),
                        tell: &|spent, _asked| tell(spent.chats, spent.tokens, None, None),
                        // Said in its marker as each chat ends (#3582, #3616).
                        on_spent: &|spent| {
                            if let Some(marker) = marker.borrow_mut().as_mut()
                                && let Err(why) = marker.spent(spent)
                            {
                                tracing::warn!(
                                    "purlis: router grade {grade} under way does not say what \
                                     it spent ({why})"
                                );
                            }
                        },
                        grade: &grade,
                        asked: 0,
                        asked_for: 0,
                        harness: None,
                        plugins_off: None,
                        model: model.as_deref(),
                        reported: Vec::new(),
                    };
                    let report = engine::grade(&request, &mut chats, Some(limits.clone()));
                    let graded = RouterGraded {
                        harness: chats.harness.take(),
                        ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
                        spent: chats.spent,
                        model: grades::reported_model(chats.reported.iter().map(String::as_str)),
                        report,
                        ..under_way.clone()
                    };
                    let recorded = routers::record(
                        &root,
                        &planned.copies,
                        &graded,
                        &purlis_core::machine::this_log_name(),
                    );
                    grades::underway::ended(&root, &grade);
                    // The chats of the router's older grades in this scope go: its newest keep
                    // theirs (#3617).
                    grades::kept::sweep_of(&root, &kept_of(&planned));
                    drop(marker.borrow_mut().take());
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
                    // Recorded as interrupted, unless the fault came once it was recorded whole.
                    // Its lock let go of first: a start reads a held lock as a grade that runs.
                    drop(marker.borrow_mut().take());
                    let said = if record_interrupted(&root).contains(&grade) {
                        "A fault in purlis ended the router's grade before it was scored, so it \
                         is recorded as interrupted: grade it again."
                    } else {
                        "A fault in purlis ended the router's grade once it was recorded."
                    };
                    tell(0, None, Some(GradeEnded::Failed), Some(said.to_owned()));
                });
            })
            .map_err(|why| {
                running().remove(&id);
                grades::underway::ended(&held_at, &id);
                format!("The grade could not start ({why}).")
            })?;
        Ok(id)
    })
    .await
}

/// **Whose chats a router's grade keeps** (#3617): the router's, in the scope that chose it, whose
/// grades are read by that scope.
fn kept_of(planned: &RouterPlanned) -> grades::kept::Of {
    grades::kept::Of::Router {
        router: planned.router.clone(),
        workspace: planned.workspace.clone(),
        persona: planned.persona.clone(),
    }
}

/// **One skill of a router's grade, on its scorecard** (#3617): each of its trigger tests in both
/// arms, with each chat one press away where this machine kept it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct RouterScoredSkill {
    pub skill: String,
    pub tests: Vec<ScoredTest>,
}

/// **Each arm's pass rate over every skill of a router's grade**, in points from 0 to 100, where
/// the arm ran: the scorecard's totals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
pub struct RouterGradeArms {
    pub router: Option<f64>,
    pub built_in: Option<f64>,
}

/// **A router's grade's scorecard** (#3617), as a skill's is (SK-59): how it ended, its totals,
/// then each skill's trigger tests in both arms.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct RouterGradeScorecard {
    pub grade: String,
    pub router: String,
    /// When it ran, and the library it ran on: its commit, or that it held changes not
    /// committed.
    pub graded: String,
    pub library: String,
    /// What its chats ran on, and how it ended, in the record's words.
    pub ran_on: String,
    pub ended: String,
    /// The router's line of it, as the Routing page says it.
    pub said: String,
    pub outcome: GradeOutcome,
    pub arms: RouterGradeArms,
    pub spent: GradeSpent,
    /// How many of its chats asked the person something and were ended there.
    pub asked: u32,
    pub skills: Vec<RouterScoredSkill>,
}

impl RouterGradeScorecard {
    /// `graded`'s scorecard, each chat `kept` says whether this machine kept.
    fn of(graded: &RouterGraded, kept: impl Fn(&str) -> bool) -> Self {
        use purlis_core::skills::studio;
        let points = |arm: Arm| {
            let (passed, of) = graded.report.passed_in(arm);
            (of > 0).then(|| (f64::from(passed) * 100.0 / f64::from(of)).round())
        };
        let harness = graded
            .harness
            .as_deref()
            .map(purlis_core::skilluse::harness_title)
            .unwrap_or_else(|| "Not recorded".to_owned());
        RouterGradeScorecard {
            grade: graded.grade.clone(),
            router: graded.router.clone(),
            graded: studio::said_at(&graded.ts),
            library: match &graded.commit {
                Some(commit) => format!("the library at {}", commit.get(..12).unwrap_or(commit)),
                None => "a library with changes not committed".to_owned(),
            },
            ran_on: match &graded.model {
                Some(model) => format!("{harness} · {model}"),
                None => harness,
            },
            ended: grades::ended_said(graded.report.ended).to_owned(),
            said: graded.said(),
            outcome: graded.report.ended.into(),
            arms: RouterGradeArms {
                router: points(Arm::Router),
                built_in: points(Arm::BuiltInRouter),
            },
            spent: GradeSpent {
                chats: graded.spent.chats,
                tokens: graded.spent.tokens.map(|tokens| tokens as f64),
                cost_usd: graded.spent.cost_usd,
                limit_usd: graded.report.cost_cap_usd,
            },
            asked: graded.report.asked(),
            skills: graded
                .report
                .skills
                .iter()
                .map(|part| RouterScoredSkill {
                    skill: part.skill.clone(),
                    tests: scored_tests(&part.report.triggers, &kept),
                })
                .collect(),
        }
    }
}

/// The grade `grade` of the router the scope chose, as its record holds it, and whose chats it
/// keeps.
fn router_grade_of(
    root: &std::path::Path,
    scope: &Scope,
    grade: &str,
) -> Result<(RouterGraded, grades::kept::Of), String> {
    let router = chosen(root, scope)?;
    let graded = routers::grades_of(
        root,
        &router,
        scope.workspace.as_deref(),
        scope.persona.as_deref(),
    )
    .into_iter()
    .find(|one| one.grade == grade)
    .ok_or_else(|| "That grade of the router is not in the record any more.".to_owned())?;
    let of = grades::kept::Of::Router {
        router,
        workspace: scope.workspace.clone(),
        persona: scope.persona.clone(),
    };
    Ok((graded, of))
}

/// **The scorecard of the grade `grade` of the router the studio's workspace or persona chose**
/// (#3617), as the routers' record holds it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn router_grade_scorecard(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    grade: String,
) -> Result<RouterGradeScorecard, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a router's grade", move || {
        flag_on()?;
        let root = held.root();
        let (graded, of) = router_grade_of(root, &scope(workspace, persona)?, &grade)?;
        Ok(RouterGradeScorecard::of(&graded, |chat| {
            grades::kept::kept_of(root, &of, &grade, chat).is_some()
        }))
    })
    .await
}

/// **What chat `chat` of the router's grade `grade` showed as it ended** (#3617): its screen and
/// scrollback as the escape sequences that draw them, for a page that shows it read-only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn router_grade_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: Option<String>,
    persona: Option<String>,
    grade: String,
    chat: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a router's grade's chat", move || {
        flag_on()?;
        let root = held.root();
        let (_, of) = router_grade_of(root, &scope(workspace, persona)?, &grade)?;
        grades::kept::kept_of(root, &of, &grade, &chat).ok_or_else(|| not_kept("the router's"))
    })
    .await
}

#[cfg(test)]
mod tests;
