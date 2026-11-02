// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grade, in a skill's tab** (SK-16, #2779; spec #2762 decisions 22 and 43; ADR 0101 §5,
//! ADR 0105): the window's half of `purlis_core::skills::grades` and of the built-in runner.
//!
//! - **The cost comes first** ([`skill_grade_plan`]): what a grade of the copy runs, its trigger
//!   tests as the chats will read them, and what the copy's last grade spent per chat. Nothing
//!   runs until the person presses Grade on that plan.
//! - **A grade runs real chats** ([`skill_grade_start`]): the built-in runner asks for each one,
//!   and this host lays out a throwaway piece for its arm (the copy without its fixtures, or no
//!   skill), starts a chat there on the trigger test's query on the profile the person chose,
//!   under the project's sandbox, and watches it: the skill runs that start in it, and the end of
//!   its turn. It ends the chat as soon as the skill under test starts, since a trigger test learns
//!   nothing more from it, at the end of its turn, or at [`CHAT_TIME`]. One chat runs at a time.
//! - **A grade's chats are drawn nowhere** (`purlis_core::reopen::Chat::graded`): no tab, no row,
//!   and a relaunch never starts one again. The window is told the grade's progress instead
//!   ([`EVENT`]), and the person can stop it ([`skill_grade_stop`]).
//! - **The grade goes to the eval record** when it ends, whole or stopped, tied to the copy's
//!   version, and the skill's Grades panel and history show it.
//!
//! Behind the `skill-studio` flag until its finish line (#2786).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use purlis_session_protocol::session;
use serde::Serialize;
use specta::Type;
use tauri::{Emitter as _, Manager as _};

use purlis_core::executor::Executor;
use purlis_core::skills::grade::{
    self as grading, Arm, ChatEnd, ChatKind, ChatRan, Chats, GradeEnd, RunChat,
};
use purlis_core::skills::grades::{self, Graded, Planned, Spent};
use purlis_core::skills::runs::StartedBy;
use purlis_core::skills::verbs::Level;

use crate::curation::ChatTyped;
use crate::planes::{Held, PlaneId, Planes};
use crate::skillchange::{SkillLevel, flag_on};
use crate::skillrun::Watched;

/// The event the window is told each step of a grade by.
pub const EVENT: &str = "skill-grade";

/// The longest one grade's chat runs before purlis ends it.
pub const CHAT_TIME: Duration = Duration::from_secs(5 * 60);

/// **How long a grade's chat has to get as far as its session's start** before purlis takes it for
/// one that waits on something no hook tells of, such as Claude Code asking whether to trust its
/// folder. It has spent nothing by then.
const A_CHAT_BEGINS_WITHIN: Duration = Duration::from_secs(120);

/// What a chat that never began is told in the grade.
const NEVER_BEGAN: &str = "its chat did not get as far as starting: Claude Code may be waiting for \
     you to trust this project's folder, so open a chat here once and accept it";

/// How often a chat being watched is asked whether the person stopped the grade.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// **How long an outcome eval's or a judge's chat is waited on for what its turn ended saying**
/// once its turn ended (SK-55): its hook tells it on a line of its own, just before the turn's
/// end, and the two lines may be heard in either order.
const ANSWER_COMES_WITHIN: Duration = Duration::from_secs(2);

/// The size a grade's chat runs at: it is drawn nowhere.
const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 120,
    rows: 40,
};

/// One trigger test, as the plan shows it before anything runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradeQuery {
    /// The query as the chat will read it, each character no reader would see marked.
    pub query: String,
    pub should_trigger: bool,
    /// Whether it holds a character no reader would see.
    pub hides: bool,
}

/// **A trigger test a grade leaves out** (`grades::LeftOut`): its place in the file, from 0, and
/// why, as a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradeLeftOut {
    pub at: u32,
    pub why: String,
}

/// **One outcome eval, as the plan shows it before anything runs** (SK-55): its id, its first
/// message as the chat will read it, each character no reader would see marked, its input files,
/// and how many expectations its judge checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradeEval {
    pub id: String,
    pub prompt: String,
    pub hides: bool,
    pub files: Vec<String>,
    pub expectations: u32,
    /// Whether the person ticked it (off until they do), and the chats ticking it adds.
    pub on: bool,
    pub chats: u32,
}

/// **An outcome eval a grade leaves out** (`grades::LeftOutEval`): its id, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradeEvalLeftOut {
    pub id: String,
    pub why: String,
}

/// What a grade's chats spent before, worked out for this one (`grades::Estimate`): both
/// figures are `None` where that grade's chats did not say what they spent.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GradeEstimate {
    pub tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    /// The chats the grade it is worked out from asked for, ran or not: none where none ran.
    pub chats_then: u32,
    /// That grade: when it ran, and the profile and the harness its chats ran on.
    pub ts: String,
    pub profile: Option<String>,
    pub harness: Option<String>,
    /// The model its chats reported (SK-60): a grade on another model spends otherwise.
    pub model: Option<String>,
    /// Whether it ran on the profile this grade runs on, or `None` where that is not known.
    pub same_profile: Option<bool>,
    /// Whether that grade was interrupted before it was scored (#3602): the plan says so.
    pub interrupted: bool,
}

/// **The price a grade's dollar limit is worked out at** (`grades::price`), as the plan says it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GradePrice {
    /// Dollars a million tokens.
    pub usd_per_million: f64,
    /// Whether it is the price of the model the chats run on, rather than the dearest known.
    pub of_the_model: bool,
    /// The model the chats run on, where the profile says one.
    pub model: Option<String>,
}

impl From<&grading::Priced> for GradePrice {
    fn from(priced: &grading::Priced) -> GradePrice {
        GradePrice {
            usd_per_million: priced.usd_per_million,
            of_the_model: priced.from == grading::PricedFrom::Model,
            model: priced.model.clone(),
        }
    }
}

/// **What grading a copy would run, before anything runs** (decision 22).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradePlan {
    /// The trigger tests it runs, in the file's order.
    pub tests: Vec<GradeQuery>,
    /// The chats it runs in all, each one turn, and how many times each test runs in each arm.
    pub chats: u32,
    pub runs: u32,
    /// **The chats each grade after an edit may run** (#4035): with its previous version's arm
    /// counted where its first saved edit brings one (`after_edit::chats_to_accept`).
    pub after_edit_chats: u32,
    /// Why the trigger tests could not be read, where they were not.
    pub tests_refused: Option<String>,
    /// The trigger tests left out, each with why: a query no chat can be started on.
    pub left_out: Vec<GradeLeftOut>,
    /// **The outcome evals it runs** (SK-55), each in each arm and run, and the judge chats that
    /// grade them, counted in [`Self::chats`]; the ones it leaves out, each with why; and why the
    /// file could not be read, where it was not.
    pub evals: Vec<GradeEval>,
    pub outcome_evals: u32,
    pub judge_chats: u32,
    pub outcomes_left_out: Vec<GradeEvalLeftOut>,
    pub outcomes_refused: Option<String>,
    pub estimate: Option<GradeEstimate>,
    /// What the grade record left out as it was read: rows skipped, files not read.
    pub record_said: Option<String>,
    /// The grade of this skill running now, where one is.
    pub running: Option<String>,
    /// The tokens a grade may spend before it stops, and whether this machine set it.
    pub token_cap: f64,
    pub cap_set_here: bool,
    /// The dollar limit a grade is held to besides (the operator's ruling G12, #3469): the cap's
    /// tokens at `priced` (`grades::cap::Cap::cost_usd`).
    pub cost_cap_usd: f64,
    pub priced: GradePrice,
    /// The longest one of its chats runs, in minutes: the plan's "up to".
    pub chat_minutes: u32,
    /// The profile its chats run on, and the sandbox they run in there, by its name, or why no
    /// grade can run on it (the operator's ruling of 2026-10-26).
    pub profile: Option<String>,
    /// **The models its chats can be picked to run on** (SK-60, #3240), as the profile's harness
    /// offers them, and the one picked: `None` is the profile's own model.
    pub models: Vec<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub refused: Option<String>,
    /// What pressing Grade consents to: handed back with the press, and a grade on another plan
    /// is refused.
    pub consent: Option<String>,
    /// **The version it compares with too** (SK-57, #3237), where the skill has one: its commit
    /// shortened and its subject; and why one is not compared, where it did not read.
    pub previous: Option<GradePrevious>,
    pub previous_refused: Option<String>,
    /// **The Grader extensions it can be graded by besides purlis's own** (#3236), the one picked
    /// (`None` is purlis's own), and why the one picked is not on offer now, where it is not.
    pub graders: Vec<GradeGrader>,
    pub grader: Option<GradeGrader>,
    pub grader_refused: Option<String>,
    /// **Whether it grades a draft** (#2780): its `previous` is then the current version, read
    /// from the folder, which may be no commit.
    pub draft: bool,
}

/// **The version a grade compares with, as its plan says it** (SK-57).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradePrevious {
    pub commit: String,
    pub subject: String,
}

/// **Where a grade's chats run**, as [`chats_on`] reads it.
pub(crate) struct On {
    /// The profile, where one was found.
    pub(crate) profile: Option<String>,
    /// The sandbox's name there, or why no grade can run on it.
    pub(crate) checked: Result<String, String>,
    /// The price its dollar limit is worked out at.
    pub(crate) priced: grading::Priced,
    /// The models its harness offers for a grade, and the one picked (SK-60, #3240).
    pub(crate) models: Vec<String>,
    pub(crate) model: Option<String>,
    /// Why outcome evals cannot run there, where they cannot (`grades::outcome_refused`).
    pub(crate) evals_refused: Option<String>,
}

/// **Whether a grade's chats can run on `profile`** (the project's default where none) and on
/// `model` (the profile's own where none) in the project at `root`: the profile's name and the
/// sandbox's, or why not (R1 and R2 of the operator's ruling of 2026-10-26); the price its
/// dollar limit is worked out at there (`grades::priced`), the dearest where no chat could start;
/// and the models its harness offers (`grades::models_for`).
pub(crate) fn chats_on(
    root: &std::path::Path,
    profile: Option<String>,
    model: Option<String>,
) -> On {
    let profile = match profile {
        Some(profile) => profile,
        None => match crate::curation::default_profile(root, "no grade can run") {
            Ok((profile, _)) => profile,
            Err(why) => {
                return On {
                    profile: None,
                    checked: Err(why),
                    priced: grades::price::of_model(None),
                    models: Vec::new(),
                    model,
                    evals_refused: None,
                };
            }
        },
    };
    let start = purlis_core::start::Start {
        profile: Some(profile.clone()),
        name: "grade".to_owned(),
        cwd: Some(root.to_path_buf()),
        made: purlis_core::sandbox::Made::Window,
        ..Default::default()
    };
    // The models are offered from the profile's own start, so a model that is refused leaves the
    // others still on offer; that start is the one asked again only where a model is picked.
    let own = grades::ready_to_grade(&start, root);
    let models: Vec<String> = own
        .as_ref()
        .map(|ready| {
            grades::models_for(ready)
                .iter()
                .map(|model| (*model).to_owned())
                .collect()
        })
        .unwrap_or_default();
    let ready = match model.as_deref() {
        None => own,
        Some(picked) => grades::ready_to_grade_on(&start, root, Some(picked)),
    };
    let evals_refused = ready.as_ref().ok().and_then(grades::outcome_refused);
    let priced = ready
        .as_ref()
        .map_or_else(|_| grades::price::of_model(None), grades::priced);
    let checked = ready.map(|ready| {
        ready
            .sandbox
            .as_ref()
            .map_or(
                purlis_core::sandbox::Level::Full,
                purlis_core::sandbox::Applied::level,
            )
            .title()
            .to_owned()
    });
    On {
        profile: Some(profile),
        checked,
        priced,
        models,
        model,
        evals_refused,
    }
}

/// **What pressing Grade on `planned`, its chats `on`, consents to**: what it grades, and the
/// profile, model, token cap and dollar limit the plan shows (`grades::consent_on`). `None`
/// where there is nothing to grade.
fn consent_shown(planned: &Planned, on: &On) -> Option<String> {
    consent_by(planned, on, None)
}

/// **What pressing Grade consents to, the plan naming `grader`** (#3236): `consent_shown` with
/// the pick in it, so a grade on another Grader than the plan named is refused. `None` is
/// purlis's own runner.
fn consent_by(planned: &Planned, on: &On, grader: Option<&GradeGrader>) -> Option<String> {
    planned.consent.as_deref().map(|consent| {
        grades::consent_on(
            consent,
            &grades::Terms {
                grader: grader.map(|one| (one.id.as_str(), one.title.as_str(), &*one.hosts)),
                ..terms_of(planned, on)
            },
        )
    })
}

/// **The terms a grade of `planned` would start on, on `on`**: the profile, the model its chats
/// run on (their line's, the price's, where it names one, so a change of the profile's own model
/// is a change too; else the one picked), the cap and the dollar limit at that model's price.
fn terms_of<'a>(planned: &Planned, on: &'a On) -> grades::Terms<'a> {
    terms(&planned.cap, on)
}

/// **The terms a grade held to `cap` would start on, on `on`** ([`terms_of`]): a skill's grade's
/// or a router's (#3618).
pub(crate) fn terms<'a>(cap: &grades::cap::Cap, on: &'a On) -> grades::Terms<'a> {
    grades::Terms {
        profile: on.profile.as_deref(),
        model: on.priced.model.as_deref().or(on.model.as_deref()),
        token_cap: cap.tokens,
        limit_usd: cap.cost_usd(&on.priced),
        grader: None,
    }
}

/// **A Grader extension a plan can name** (#3236): its id, which is what says whose program it
/// is, and the title it gives itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GradeGrader {
    pub id: String,
    pub title: String,
    /// The hosts its program can send what it is handed to: the plan says each.
    pub hosts: Vec<String>,
}

/// **The Grader extensions the project at `root` can grade with now**: each this machine approved
/// and the project has on.
fn graders_here(executor: &Executor, root: &std::path::Path) -> Vec<GradeGrader> {
    purlis_core::skills::studio::with_studio(executor, root, |studio| {
        studio.map_or_else(Vec::new, |studio| {
            studio
                .graders()
                .into_iter()
                .map(|one| GradeGrader {
                    id: one.id,
                    title: one.title,
                    hosts: one.hosts,
                })
                .collect()
        })
    })
}

/// **The Grader `picked` names among `offered`**, or the sentence that says it is not on offer
/// now. `None` picked is purlis's own runner.
fn grader_picked(
    offered: &[GradeGrader],
    picked: Option<&str>,
) -> Result<Option<GradeGrader>, String> {
    let Some(picked) = picked else {
        return Ok(None);
    };
    offered
        .iter()
        .find(|one| one.id == picked)
        .cloned()
        .map(Some)
        .ok_or_else(|| {
            format!(
                "The Grader '{picked}' cannot grade here now: it was removed, turned off in this \
                 project, or is no longer approved on this machine."
            )
        })
}

/// **The plan of `planned` on `on`, with the Graders it lists and the one picked** (#3236): where
/// the pick is not on offer the plan says why and consents to nothing.
fn plan_by(
    planned: &Planned,
    running: Option<String>,
    on: On,
    offered: Vec<GradeGrader>,
    picked: Option<&str>,
) -> SkillGradePlan {
    let pick = grader_picked(&offered, picked);
    let consent = match &pick {
        Ok(grader) => consent_by(planned, &on, grader.as_ref()),
        Err(_) => None,
    };
    let evals_refused = on.evals_refused.clone();
    let mut plan = SkillGradePlan::of(planned, running, on);
    plan.consent = consent;
    plan.graders = offered;
    match pick {
        Ok(grader) => plan.grader = grader,
        Err(why) => plan.grader_refused = Some(why),
    }
    if let Err(why) = grader_takes(planned, plan.grader.is_some()) {
        plan.consent = None;
        plan.grader_refused = Some(why);
    }
    if let Err(why) = evals_can_run(planned, evals_refused.as_ref()) {
        plan.consent = None;
        plan.refused = Some(why);
    }
    plan
}

/// **Whether `planned`'s ticked outcome evals can run**, given why they cannot where they cannot.
pub(crate) fn evals_can_run(planned: &Planned, refused: Option<&String>) -> Result<(), String> {
    let ticked = planned
        .request
        .as_ref()
        .is_some_and(|request| !request.outcomes.is_empty());
    match refused {
        Some(why) if ticked => Err(why.clone()),
        _ => Ok(()),
    }
}

/// **Whether a grade of `planned` can run with a Grader, where one is picked**: a Grader scores
/// trigger tests alone, so ticked outcome evals and a Grader are not one grade (SK-55).
fn grader_takes(planned: &Planned, by_a_grader: bool) -> Result<(), String> {
    let ticked = planned
        .request
        .as_ref()
        .is_some_and(|request| !request.outcomes.is_empty());
    if by_a_grader && ticked {
        return Err(
            "A Grader scores trigger tests alone: untick the outcome evals, or grade with \
             purlis's own runner."
                .to_owned(),
        );
    }
    Ok(())
}

impl From<&grades::Estimate> for GradeEstimate {
    fn from(estimate: &grades::Estimate) -> Self {
        GradeEstimate {
            tokens: estimate.tokens.map(|tokens| tokens as f64),
            cost_usd: estimate.cost_usd,
            chats_then: estimate.chats_then,
            ts: estimate.ts.clone(),
            profile: estimate.profile.clone(),
            harness: estimate
                .harness
                .as_deref()
                .map(purlis_core::skilluse::harness_title),
            model: estimate.model.clone(),
            same_profile: estimate.same_profile,
            interrupted: estimate.interrupted,
        }
    }
}

/// **Whether a press with `consent` was made on the plan a grade of `planned`, on `on`, would
/// start on now**, or the sentence that refuses it: what it grades, the profile, the model, the
/// cap or the limit changed since the plan was shown, and the window shows the new one. `so`
/// says what did not happen: a grade's start, and once #3238 lands, turning grading after each
/// edit on.
fn plan_holds(
    planned: &Planned,
    on: &On,
    grader: Option<&GradeGrader>,
    consent: &str,
    skill: &str,
    so: &str,
) -> Result<(), String> {
    if consent_by(planned, on, grader).as_deref() == Some(consent) {
        return Ok(());
    }
    Err(format!(
        "The plan of {skill} changed since it was shown (what it grades, the profile or model \
         its chats run on, its limits, or the Grader that scores it), so {so}: read the new plan and press again."
    ))
}

impl SkillGradePlan {
    fn of(planned: &Planned, running: Option<String>, on: On) -> Self {
        let tests = planned
            .request
            .as_ref()
            .map(|request| {
                request
                    .triggers
                    .iter()
                    .enumerate()
                    .map(|(at, test)| GradeQuery {
                        query: purlis_unicode::marked(&test.query, purlis_unicode::Breaks::Marked),
                        should_trigger: test.should_trigger,
                        hides: planned.unseen.contains(&at),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let consent = consent_shown(planned, &on);
        let On {
            profile,
            checked: sandbox,
            priced,
            models,
            model,
            ..
        } = on;
        SkillGradePlan {
            tests,
            chats: planned.plan.map(|plan| plan.chats).unwrap_or_default(),
            runs: u32::from(grading::RUNS),
            after_edit_chats: planned.after_edit_chats.unwrap_or_default(),
            tests_refused: planned
                .fixtures
                .triggers
                .refused()
                .map(purlis_core::skills::grade::Refused::in_window),
            left_out: planned
                .left_out
                .iter()
                .map(|left| GradeLeftOut {
                    at: u32::try_from(left.at).unwrap_or(u32::MAX),
                    why: left.why.clone(),
                })
                .collect(),
            evals: planned
                .offered
                .iter()
                .map(|eval| {
                    let prompt = grading::outcomes::outcome_prompt(eval);
                    GradeEval {
                        id: eval.id.to_string(),
                        hides: purlis_unicode::hides(&prompt, purlis_unicode::Breaks::Kept)
                            || eval
                                .expectations
                                .iter()
                                .chain(&eval.expected_output)
                                .chain(
                                    eval.files
                                        .iter()
                                        .filter_map(|path| planned.copy.as_ref()?.inputs.get(path)),
                                )
                                .any(|text| {
                                    purlis_unicode::hides(text, purlis_unicode::Breaks::Kept)
                                }),
                        prompt: purlis_unicode::marked(&prompt, purlis_unicode::Breaks::Marked),
                        files: eval.files.clone(),
                        expectations: u32::try_from(eval.expectations.len()).unwrap_or(u32::MAX),
                        on: planned.request.as_ref().is_some_and(|request| {
                            request.outcomes.iter().any(|one| one.id == eval.id)
                        }),
                        chats: planned.eval_chats,
                    }
                })
                .collect(),
            outcome_evals: planned
                .plan
                .map(|plan| plan.outcome_evals)
                .unwrap_or_default(),
            judge_chats: planned
                .plan
                .map(|plan| plan.judge_chats)
                .unwrap_or_default(),
            outcomes_left_out: planned
                .outcomes_left_out
                .iter()
                .map(|left| GradeEvalLeftOut {
                    id: left.id.to_string(),
                    why: left.why.clone(),
                })
                .collect(),
            outcomes_refused: planned
                .fixtures
                .outcomes
                .refused()
                .map(purlis_core::skills::grade::Refused::in_window),
            estimate: planned.estimate.as_ref().map(GradeEstimate::from),
            record_said: planned.record_said.clone(),
            running,
            token_cap: planned.cap.tokens as f64,
            cost_cap_usd: planned.cap.cost_usd(&priced),
            priced: (&priced).into(),
            cap_set_here: planned.cap.from == grades::cap::From::Yours,
            chat_minutes: u32::try_from(grades::CHAT_TIME.as_secs() / 60).unwrap_or(u32::MAX),
            profile,
            models,
            model,
            refused: sandbox.as_ref().err().cloned(),
            sandbox: sandbox.ok(),
            consent,
            previous: planned.previous.as_ref().map(|previous| GradePrevious {
                commit: purlis_core::skills::history::short(&previous.commit).to_owned(),
                subject: previous.subject.clone(),
            }),
            previous_refused: planned.previous_refused.clone(),
            graders: Vec::new(),
            grader: None,
            grader_refused: None,
            draft: false,
        }
    }
}

/// How a grade ended, as the window is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum GradeEnded {
    Whole,
    Stopped,
    ChatsDidNotStart,
    /// It reached its token cap.
    OverCap,
    /// It reached its dollar limit (#3469).
    OverLimit,
    /// A chat did not say what it spent, so the cap could not be held.
    SpendNotReported,
    /// A fault in purlis ended it before it was scored (#3353): recorded as interrupted.
    Failed,
}

impl From<GradeEnd> for GradeEnded {
    fn from(end: GradeEnd) -> GradeEnded {
        match end {
            GradeEnd::Whole => GradeEnded::Whole,
            GradeEnd::Cancelled => GradeEnded::Stopped,
            GradeEnd::ChatsDidNotStart => GradeEnded::ChatsDidNotStart,
            GradeEnd::OverCap => GradeEnded::OverCap,
            GradeEnd::OverLimit => GradeEnded::OverLimit,
            GradeEnd::SpendNotReported => GradeEnded::SpendNotReported,
            // Recorded at a start for a grade purlis stopped under, never told as one ends; and a
            // Grader extension's question that ended its grade (#3236), told with its sentence.
            GradeEnd::Interrupted | GradeEnd::GraderFailed => GradeEnded::Failed,
        }
    }
}

/// **One step of a grade**, as [`EVENT`] tells the window: how many of its chats have run, and,
/// once it is over, how it ended and what it says.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradeStep {
    pub plane: PlaneId,
    pub skill: String,
    /// The copy it grades, by its level (#3978): a same-named copy at another level is another
    /// skill, whose tab a step of this one moves nothing on.
    pub level: SkillLevel,
    pub grade: String,
    pub ran: u32,
    pub of: u32,
    /// What its chats have spent so far, where they said, and the cap it is held to.
    pub tokens: Option<f64>,
    /// What its chats have cost so far, in dollars, where they said (S6): the running page's
    /// spend against the grade's dollar limit.
    pub cost_usd: Option<f64>,
    /// The dollar limit it is held to, where it has one.
    pub limit_usd: Option<f64>,
    pub cap: f64,
    /// How many of its chats asked the person something and were ended there (SK-59, #3239):
    /// said in the grade, as none of them is a run.
    pub asked: u32,
    /// How it ended, once it has.
    pub ended: Option<GradeEnded>,
    /// What the window says of its end: why its chats did not start, or that it was not
    /// recorded.
    pub said: Option<String>,
    /// **The draft it grades, by its id** (#2780): a step of this one moves nothing on the
    /// skill's Grades tab.
    pub draft: Option<String>,
}

/// One grade running, and its way to be stopped.
pub(crate) struct Running {
    pub(crate) plane: PlaneId,
    /// The skill it grades, by its name; empty for a router's grade (SK-15e), which no skill's
    /// name is.
    pub(crate) skill: String,
    /// The copy it grades, by its level: same-named copies at two levels are two skills
    /// (#3353). `None` for a router's grade.
    pub(crate) level: Option<Level>,
    pub(crate) stop: Arc<AtomicBool>,
    /// The copy a saved edit of which started it (SK-58), by its opt-in's key (level and
    /// name): the next save of that copy stops it. `None` for a grade pressed by hand.
    pub(crate) after_edit: Option<String>,
    /// The draft it grades, by its id (#2780): none of the skill's own pages shows it.
    pub(crate) draft: Option<String>,
    /// Its latest step told to the window: what a page that opens while it runs starts from
    /// (#3928, #4222).
    pub(crate) step: RunningStep,
}

/// The latest step of a grade running: a skill's or a router's, as the grade is (#4240).
pub(crate) enum RunningStep {
    Skill(SkillGradeStep),
    Router(crate::routergrade::RouterGradeStep),
}

/// **The grades running in this app**, by id: one per project at a time.
static RUNNING: std::sync::LazyLock<Mutex<HashMap<String, Running>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// The grades running now, held.
pub(crate) fn running() -> std::sync::MutexGuard<'static, HashMap<String, Running>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// **Runs the grade `grade`'s `body`, and the grade is not running after it, whatever happened
/// in it** (#3353): a panic in it is handed to `failed` once the grade is no longer running, so
/// a grade never stays running with nothing left to end it. That catch works in dev and test
/// builds alone: every shipped build aborts on a panic (`panic = "abort"` in the release
/// profile), so there the app ends with it, and the project's next start recording the grade as
/// interrupted (`grades::underway`) is the floor, by decision (#3443).
pub(crate) fn held_to_its_end(grade: &str, body: impl FnOnce(), failed: impl FnOnce()) {
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
    running().remove(grade);
    if ran.is_err() {
        tracing::warn!("purlis: grade {grade} ended on a fault in purlis before it was scored");
        failed();
    }
}

/// What the window says of a grade a fault in purlis ended once it was recorded.
const FAILED_ONCE_RECORDED: &str = "A fault in purlis ended the grade once it was recorded: its \
     scorecard is in Grades below.";

/// What the window says of a grade a fault in purlis ended.
const FAILED: &str = "A fault in purlis ended the grade before it was scored, so it is recorded \
     as interrupted: grade it again.";

/// **Whether this app runs the grade `grade` now**: what a project's start asks before it
/// records a grade under way of this process as interrupted (`grades::underway::interrupted`).
pub(crate) fn runs_here(grade: &str) -> bool {
    running().contains_key(grade)
}

/// **Records the grades under way in the project at `root` that nothing runs, as interrupted**
/// (#3353): what a project's start does, and a grade a fault ended.
pub(crate) fn record_interrupted(root: &std::path::Path) -> Vec<String> {
    let recorded =
        grades::underway::interrupted(root, purlis_core::machine::this_log_name, runs_here);
    for grade in &recorded {
        tracing::info!(
            "purlis: grade {grade} was under way when purlis stopped: recorded as interrupted"
        );
    }
    recorded
}

/// The grade of `skill` running in `plane` now, by id: never a draft's, which is the draft's own
/// page's to show.
fn running_of(plane: &PlaneId, skill: &str) -> Option<String> {
    running()
        .iter()
        .find(|(_, one)| one.plane == *plane && one.skill == skill && one.draft.is_none())
        .map(|(id, _)| id.clone())
}

/// **The outcome evals the window says were ticked**, by id: none are, until the person ticks them.
pub(crate) fn ticked(evals: &[String]) -> Vec<u64> {
    evals.iter().filter_map(|id| id.parse().ok()).collect()
}

/// **What grading the copy of `skill` at `level` would run**, before anything runs.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_grade_plan(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
    profile: Option<String>,
    model: Option<String>,
    evals: Vec<String>,
    grader: Option<String>,
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reading what a grade runs", move || {
        flag_on()?;
        let on = chats_on(held.root(), profile, model);
        let planned = grades::planned_with(
            held.root(),
            &(&level).into(),
            &skill,
            on.profile.as_deref(),
            &ticked(&evals),
        )?;
        Ok(plan_by(
            &planned,
            running_of(&plane, &skill),
            on,
            graders_here(&executor, held.root()),
            grader.as_deref(),
        ))
    })
    .await
}

/// **Grades the copy of `skill` at `level`**, on `profile` (the project's default where none
/// is picked) and `model` (the profile's own where none is, SK-60): its chats run one at a time
/// behind the window, which is told each step ([`EVENT`]). The answer is the grade's id.
/// Refused while a grade runs in the project.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_grade_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
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
        let level: Level = (&level).into();
        let on = chats_on(held.root(), profile.clone(), model.clone());
        // The estimate is the plan's to show: a start needs none of it.
        let planned = grades::planned_with(held.root(), &level, &skill, None, &ticked(&evals))?;
        // What runs is what the plan showed (the review of #3229): the tests, the skill's files
        // and what its folder holds, read once here and laid out from this one copy, on the
        // profile, model and limits it showed.
        // And on the Grader it named: one no longer on offer starts nothing.
        let picked = grader_picked(&graders_here(&executor, held.root()), grader.as_deref())?;
        grader_takes(&planned, picked.is_some())?;
        evals_can_run(&planned, on.evals_refused.as_ref())?;
        plan_holds(
            &planned,
            &on,
            picked.as_ref(),
            &consent,
            &skill,
            "nothing was started",
        )?;
        begin(
            app,
            held,
            Begin {
                plane,
                level,
                skill,
                profile,
                model,
                after_edit: false,
                grader: picked.map(|one| ByGrader {
                    id: one.id,
                    executor,
                }),
                follower: None,
                draft: None,
            },
            planned,
        )
    })
    .await
}

/// What a terminal that asked for a grade is told of each of its steps.
pub(crate) type Follower = Box<dyn Fn(session::GradeStepTold) + Send + Sync>;

/// The app, for what its host asks of it outside a command: a grade a terminal asked for.
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// **The app takes the grades terminals ask for** (SK-61, #3241), from here on.
pub(crate) fn hears_terminals(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

/// The level a terminal named, as the window names it.
fn level_named(level: session::GradeLevel) -> SkillLevel {
    match level {
        session::GradeLevel::Project { local } => SkillLevel::Project { local },
        session::GradeLevel::Workspace { name, local } => SkillLevel::Workspace { name, local },
        session::GradeLevel::Persona { name, local } => SkillLevel::Persona { name, local },
    }
}

/// Dollars in the millionths the session protocol says them in.
fn micro_usd(usd: f64) -> u64 {
    (usd * 1_000_000.0).round() as u64
}

impl From<GradeEnded> for session::GradeEndTold {
    fn from(ended: GradeEnded) -> Self {
        match ended {
            GradeEnded::Whole => session::GradeEndTold::Whole,
            GradeEnded::Stopped => session::GradeEndTold::Stopped,
            GradeEnded::ChatsDidNotStart => session::GradeEndTold::ChatsDidNotStart,
            GradeEnded::OverCap => session::GradeEndTold::OverCap,
            GradeEnded::OverLimit => session::GradeEndTold::OverLimit,
            GradeEnded::SpendNotReported => session::GradeEndTold::SpendNotReported,
            GradeEnded::Failed => session::GradeEndTold::Failed,
        }
    }
}

impl From<&SkillGradeStep> for session::GradeStepTold {
    fn from(step: &SkillGradeStep) -> Self {
        session::GradeStepTold {
            grade: step.grade.clone(),
            ran: step.ran,
            of: step.of,
            tokens: step.tokens.map(|tokens| tokens as u64),
            cost_micro_usd: step.cost_usd.map(micro_usd),
            asked: step.asked,
            ended: step.ended.map(Into::into),
            said: step.said.clone(),
        }
    }
}

/// The plan a terminal is told, as a press would start it: the words of [`SkillGradePlan::of`].
fn plan_told(planned: &Planned, on: &On, running: Option<String>) -> session::GradePlanTold {
    let terms = terms_of(planned, on);
    session::GradePlanTold {
        profile: on.profile.clone(),
        model: terms.model.map(str::to_owned),
        sandbox: on.checked.as_ref().ok().cloned(),
        refused: on.checked.as_ref().err().cloned(),
        chats: planned.plan.map(|plan| plan.chats).unwrap_or_default(),
        token_cap: terms.token_cap,
        limit_micro_usd: micro_usd(terms.limit_usd),
        running,
        graded: planned.consent.clone(),
        consent: consent_shown(planned, on),
    }
}

/// **The window's answer to a grade a person asked for at a terminal** (SK-61, #3241): `project`'s
/// plan, or its start through the one check a press of Grade takes ([`plan_holds`]), on the plan's
/// cap and limit. `follower` hears each step. Named outcome evals are refused until #3235 runs them.
pub(crate) fn asked_at_a_terminal(
    project: &str,
    path: &str,
    ask: session::GradeAsk,
    follower: Follower,
) -> session::GradeAnswer {
    answering_a_terminal(project, path, ask, follower)
        .unwrap_or_else(|why| session::GradeAnswer::Refused { why })
}

fn answering_a_terminal(
    project: &str,
    path: &str,
    ask: session::GradeAsk,
    follower: Follower,
) -> Result<session::GradeAnswer, String> {
    flag_on()?;
    let app = APP.get().ok_or("The purlis window is still starting.")?;
    let planes = app.state::<Planes>();
    let plane = planes
        .open_now()
        .into_iter()
        .find(|plane| {
            purlis_core::scaffold::planefile::id_to_hold_by(plane.root())
                .is_ok_and(|id| id == project)
                && std::fs::canonicalize(plane.root()).is_ok_and(|root| root == Path::new(path))
        })
        .ok_or("This project is not open in the purlis window.")?;
    let held = planes.held(&plane)?;
    let of = match &ask {
        session::GradeAsk::Plan { of } | session::GradeAsk::Start { of, .. } => of.clone(),
    };
    if !of.evals.is_empty() {
        return Err(
            "Outcome evals do not run from a terminal yet (#3235), so nothing was started.".into(),
        );
    }
    let level: SkillLevel = level_named(of.level);
    let level: Level = (&level).into();
    let on = chats_on(held.root(), of.profile.clone(), of.model.clone());
    match ask {
        session::GradeAsk::Plan { .. } => {
            let planned = grades::planned(held.root(), &level, &of.skill, on.profile.as_deref())?;
            let running = running_of(&plane, &of.skill);
            Ok(session::GradeAnswer::Plan(Box::new(plan_told(
                &planned, &on, running,
            ))))
        }
        session::GradeAsk::Start { consent, .. } => {
            let planned = grades::planned(held.root(), &level, &of.skill, None)?;
            // A terminal's grade is purlis's own runner: its plan names no Grader.
            plan_holds(
                &planned,
                &on,
                None,
                &consent,
                &of.skill,
                "nothing was started",
            )?;
            let grade = begin(
                app.clone(),
                held,
                Begin {
                    plane,
                    level,
                    skill: of.skill,
                    profile: of.profile,
                    model: of.model,
                    after_edit: false,
                    grader: None,
                    follower: Some(follower),
                    draft: None,
                },
                planned,
            )?;
            Ok(session::GradeAnswer::Started { grade })
        }
    }
}

/// **Stops `grade`, whose terminal has gone**, where it still runs: nothing spends with nobody
/// following it.
pub(crate) fn stop_for_a_terminal(grade: &str) {
    if let Some(one) = running().get(grade) {
        one.stop.store(true, Ordering::SeqCst);
    }
}

/// **What a grade is started on**: where, of which copy, on which profile and model, and
/// whether a saved edit started it (SK-58) rather than a press of Grade.
pub(crate) struct Begin {
    pub plane: PlaneId,
    pub level: Level,
    pub skill: String,
    pub profile: Option<String>,
    pub model: Option<String>,
    pub after_edit: bool,
    /// The Grader extension the plan named (#3236); `None` is purlis's own runner.
    pub grader: Option<ByGrader>,
    /// The terminal that asked for it (SK-61, #3241), told each step the window is.
    pub follower: Option<Follower>,
    /// The draft it grades (#2780), where it grades one.
    pub draft: Option<DraftBegun>,
}

/// **The draft a grade grades**: its id, which the window finds it by, and the mark its row holds.
pub(crate) struct DraftBegun {
    pub id: String,
    pub mark: grades::draft::Mark,
}

/// **The Grader extension a grade runs through**, and what asks it.
pub(crate) struct ByGrader {
    pub id: String,
    pub executor: Arc<Executor>,
}

/// **`request` as a Grader extension is handed it**: what the plan shows it is handed, the trigger
/// tests. It can run no chat but theirs, so it has no use for the outcome evals.
fn handed_to_a_grader(request: &grading::GradeRequest) -> grading::GradeRequest {
    grading::GradeRequest {
        outcomes: Vec::new(),
        ..request.clone()
    }
}

/// **The report of `request`, graded by `by`** (purlis's own runner where none), its chats run by
/// `chats` and held to `limits`: the same host and the same limits either way.
fn graded_by(
    by: Option<&ByGrader>,
    root: &std::path::Path,
    request: &grading::GradeRequest,
    chats: &mut dyn Chats,
    limits: grading::Limits,
) -> grading::GradeReport {
    let Some(by) = by else {
        return grading::grade(request, chats, Some(limits));
    };
    let request = &handed_to_a_grader(request);
    purlis_core::skills::studio::with_studio(&by.executor, root, |studio| match studio {
        Some(studio) => studio.grade(&by.id, request, chats, Some(limits)),
        // Its plan listed it, so this machine's config home went since: still a report.
        None => purlis_core::extension::studio::grading::Grading::new(
            &by.id,
            request,
            chats,
            Some(limits),
        )
        .finished(Err("this machine has no config home now".to_owned())),
    })
}

/// **Starts a grade of `planned`**, the plan whose consent was checked or whose bounds the
/// person accepted (SK-58): its chats run one at a time behind the window, which is told each
/// step ([`EVENT`]). The answer is the grade's id. Refused while a grade runs in the project.
pub(crate) fn begin(
    app: tauri::AppHandle,
    held: Arc<Held>,
    begin: Begin,
    planned: Planned,
) -> Result<String, String> {
    let Begin {
        plane,
        level,
        skill,
        profile,
        model,
        after_edit,
        grader,
        follower,
        draft,
    } = begin;
    let root = held.root().to_path_buf();
    // The one read of the copy the consent was taken from: laid out and recorded as is.
    let (Some(copy), Some(request)) = (planned.copy, planned.request) else {
        return Err(format!(
            "{skill} has no trigger tests or outcome evals to grade, so nothing was started."
        ));
    };
    let On {
        profile,
        checked,
        priced,
        model,
        ..
    } = chats_on(&root, profile, model);
    checked?;
    let profile = profile.ok_or("The grade did not start: no profile was found.")?;
    let id = purlis_core::reopen::mint();
    let stop = Arc::new(AtomicBool::new(false));
    let of = grading::plan(&request).chats;
    // The cap the plan showed, read again as the grade starts: the one it is held to.
    let cap = planned.cap.tokens;
    let limits = grading::Limits {
        tokens: cap,
        cost_usd: Some(planned.cap.cost_usd(&priced)),
        priced: Some(priced),
        unpriced_usd_per_million: Some(grades::price::DEAREST_USD_PER_MILLION),
    };
    // **One step of it**, as the window is told it and as a page that opens while it runs reads
    // it (`skill_grades`).
    let step_of = {
        let (plane, skill, grade) = (plane.clone(), skill.clone(), id.clone());
        let copy_level = SkillLevel::from(&level);
        let limit_usd = limits.cost_usd;
        let draft = draft.as_ref().map(|one| one.id.clone());
        move |spent: Spent, asked: u32, ended: Option<GradeEnded>, said: Option<String>| {
            SkillGradeStep {
                plane: plane.clone(),
                skill: skill.clone(),
                level: copy_level.clone(),
                grade: grade.clone(),
                ran: spent.chats,
                of,
                tokens: spent.tokens.map(|tokens| tokens as f64),
                cost_usd: spent.cost_usd,
                limit_usd,
                cap: cap as f64,
                asked,
                ended,
                said,
                draft: draft.clone(),
            }
        }
    };
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
                skill: skill.clone(),
                level: Some(level.clone()),
                stop: Arc::clone(&stop),
                after_edit: after_edit
                    .then(|| grades::after_edit::key(&level, &skill))
                    .flatten(),
                draft: draft.as_ref().map(|one| one.id.clone()),
                step: RunningStep::Skill(step_of(Spent::default(), 0, None, None)),
            },
        );
    }
    let grade = id.clone();
    // A draft's chats are kept apart: its grades never push out the skill's own.
    let keeps = if draft.is_some() {
        grades::kept::of_draft(&skill)
    } else {
        grades::kept::Of::Skill(skill.clone())
    };
    // Held as under way while it runs (#3353): a crash, a fault or a quit before it ends
    // leaves it to the project's next start to record as interrupted.
    let under_way = Graded {
        v: grades::VERSION,
        grade: grade.clone(),
        skill: skill.clone(),
        level: grades::recorded_level(&level),
        tree: copy.content.word().to_owned(),
        commit: copy.commit.clone(),
        harness: None,
        profile: Some(profile.clone()),
        ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
        runs: request.runs,
        spent: Spent::default(),
        // A grade a crash ends is still the one the plan named.
        report: grading::GradeReport {
            grader: grader
                .as_ref()
                .map_or_else(|| grading::BUILT_IN.to_owned(), |by| by.id.clone()),
            ..grades::underway::interrupted_report(cap, limits.cost_usd, limits.priced.clone())
        },
        plugins_off: None,
        model: None,
        // A draft's current version may be no commit: the folder is what it was read from.
        previous: copy
            .previous
            .as_ref()
            .and_then(|previous| previous.commit.clone()),
        after_edit,
        draft: draft.map(|one| one.mark),
    };
    // Its lock is held while the grade runs (#3466): what tells a start it runs.
    let marker = match grades::underway::began(&root, &level, &under_way) {
        Ok(marker) => Some(marker),
        Err(why) => {
            tracing::warn!(
                "purlis: grade {grade} is not held as under way ({why}): a crash before it \
                 ends loses it"
            );
            None
        }
    };
    let held_at = root.clone();
    std::thread::Builder::new()
        .name("purlis-grade".to_owned())
        .spawn(move || {
            let marker = std::cell::RefCell::new(marker);
            let tell =
                |spent: Spent, asked: u32, ended: Option<GradeEnded>, said: Option<String>| {
                    let step = step_of(spent, asked, ended, said);
                    // Kept while it runs, so a page opened later starts from it; an end is told
                    // once it runs no more, and kept nowhere.
                    if let Some(one) = running().get_mut(&grade) {
                        one.step = RunningStep::Skill(step.clone());
                    }
                    if let Some(follower) = &follower {
                        follower((&step).into());
                    }
                    let _ = app.emit(EVENT, step);
                };
            tell(Spent::default(), 0, None, None);
            let run = || {
                // The arm's copy: the skill as read for the grade, as its previous version's
                // commit holds it (SK-57, read from git, never from the folder), or none.
                let lay_out = |arm: Arm| {
                    let skill = match arm {
                        Arm::With => Some(&copy.snapshot),
                        Arm::Previous => match copy.previous.as_ref() {
                            Some(previous) => Some(&previous.snapshot),
                            None => return Err("this skill has no previous version".to_owned()),
                        },
                        Arm::Without => None,
                        Arm::Router | Arm::BuiltInRouter => {
                            return Err("a skill's grade has no router's arm".to_owned());
                        }
                    };
                    purlis_core::skills::verbs::drafts::lay_out_arm(&root, skill)
                };
                let mut chats = InChats {
                    held: &held,
                    lay_out: &lay_out,
                    cost_left: limits.cost_usd,
                    unpriced_at: limits.unpriced_usd_per_million,
                    skill: skill.clone(),
                    label: format!("Grade {skill}"),
                    keeps: Some(keeps.clone()),
                    profile: &profile,
                    stop: &stop,
                    spent: Spent::default(),
                    tell: &|spent, asked| tell(spent, asked, None, None),
                    on_spent: &|spent| {
                        if let Some(marker) = marker.borrow_mut().as_mut()
                            && let Err(why) = marker.spent(spent)
                        {
                            tracing::warn!(
                                "purlis: grade {grade} under way does not say what it spent \
                                 ({why})"
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
                    inputs: &copy.inputs,
                    left: None,
                };
                let report =
                    graded_by(grader.as_ref(), &root, &request, &mut chats, limits.clone());
                let harness = chats.harness.take();
                let plugins_off = chats.plugins_off.take();
                let reported = grades::reported_model(chats.reported.iter().map(String::as_str));
                let spent = chats.spent;
                let graded = Graded {
                    harness,
                    plugins_off,
                    model: reported,
                    ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
                    spent,
                    report,
                    ..under_way.clone()
                };
                let recorded = grades::record(
                    &root,
                    &level,
                    &graded,
                    &purlis_core::machine::this_log_name(),
                );
                grades::underway::ended(&root, &grade);
                // The chats of the skill's older grades go: its newest keep theirs (SK-59).
                grades::kept::sweep_of(&root, &keeps);
                drop(marker.borrow_mut().take());
                running().remove(&grade);
                let said = match (&graded.report.not_started, recorded) {
                    // A Grader that ended its grade: the window says why, in its sentence.
                    (_, None) if graded.report.failed.is_some() => {
                        graded.report.failed.as_ref().map(|why| {
                            format!(
                                "{why} This project could not be written, so it was not recorded."
                            )
                        })
                    }
                    (_, Some(_)) if graded.report.failed.is_some() => graded.report.failed.clone(),
                    (Some(why), _) => Some(why.clone()),
                    (None, Some(_)) if graded.report.ended == GradeEnd::SpendNotReported => Some(
                        "This profile's chats do not say what they spend, so a grade on it cannot \
                         be held to its cap."
                            .to_owned(),
                    ),
                    (None, None) => Some(
                        "The grade ran, but this project could not be written, so it was not \
                         recorded."
                            .to_owned(),
                    ),
                    (None, Some(_)) => None,
                };
                tell(
                    Spent {
                        tokens: graded.report.spent_tokens,
                        ..graded.spent
                    },
                    graded.report.asked(),
                    Some(graded.report.ended.into()),
                    said,
                );
            };
            held_to_its_end(&grade, run, || {
                // Recorded as interrupted, unless the fault came once it was recorded whole.
                // Its lock let go of first: a start reads a held lock as a grade that runs.
                drop(marker.borrow_mut().take());
                let said = if record_interrupted(&root).contains(&grade) {
                    FAILED
                } else {
                    FAILED_ONCE_RECORDED
                };
                tell(
                    Spent::default(),
                    0,
                    Some(GradeEnded::Failed),
                    Some(said.to_owned()),
                );
            });
        })
        .map_err(|why| {
            running().remove(&id);
            grades::underway::ended(&held_at, &id);
            format!("The grade could not start ({why}).")
        })?;
    Ok(id)
}

/// **Sets the token cap a grade in the project is held to**, on this machine (#2779; the
/// operator's ruling of 2026-10-26): the person who pays for the chats sets it, and the plan shows
/// it before Grade is pressed. The answer is the plan of `skill` at `level` again, with it.
#[tauri::command]
#[specta::specta]
// A command's arguments are the window's call, named one by one, as the other commands' are.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn skill_grade_set_cap(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
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
        let on = chats_on(held.root(), profile, model);
        let planned = grades::planned_with(
            held.root(),
            &(&level).into(),
            &skill,
            on.profile.as_deref(),
            &ticked(&evals),
        )?;
        Ok(plan_by(
            &planned,
            running_of(&plane, &skill),
            on,
            graders_here(&executor, held.root()),
            grader.as_deref(),
        ))
    })
    .await
}

/// **Sets the token cap every grade in the project at `root` is held to, on this machine**: a
/// skill's and a router's (#3618), refused for what is not a whole number of tokens.
pub(crate) fn set_cap(root: &std::path::Path, tokens: f64) -> Result<(), String> {
    if !tokens.is_finite() || tokens.fract() != 0.0 || tokens < 0.0 {
        return Err("A grade's cap is a whole number of tokens, so nothing was changed.".into());
    }
    grades::cap::set_token_cap(root, tokens as u64).map(|_| ())
}

/// **Stops the grade `grade`**: the chat it runs now is ended, no other starts, and what ran is
/// scored and recorded.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_stop(plane: PlaneId, grade: String) -> Result<(), String> {
    match running().get(&grade) {
        Some(one) if one.plane == plane => {
            one.stop.store(true, Ordering::SeqCst);
            Ok(())
        }
        _ => Err("That grade is not running any more.".to_owned()),
    }
}

/// **One chat of a grade's trigger test**, as its scorecard offers it (SK-59, #3239): its id, and
/// whether this machine kept what it showed, which is what opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ScoredChat {
    pub id: String,
    /// Which of its test's runs in its arm it was, from 1.
    pub run: u32,
    pub kept: bool,
}

/// **A grade's arm, as its scorecard says it.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ScoredArm {
    With,
    Without,
    Previous,
    /// The library, routed by the router under grade (SK-15e).
    Router,
    /// The library, routed by the built-in router: a router's grade's baseline.
    BuiltInRouter,
}

impl From<Arm> for ScoredArm {
    fn from(arm: Arm) -> ScoredArm {
        match arm {
            Arm::With => ScoredArm::With,
            Arm::Without => ScoredArm::Without,
            Arm::Previous => ScoredArm::Previous,
            Arm::Router => ScoredArm::Router,
            Arm::BuiltInRouter => ScoredArm::BuiltInRouter,
        }
    }
}

/// **One trigger test in one arm of a grade**, as its scorecard shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ScoredRuns {
    pub arm: ScoredArm,
    /// The runs that ran, and of those the ones that started the skill.
    pub runs: u32,
    pub triggered: u32,
    /// Whether it passed there, where anything ran.
    pub pass: Option<bool>,
    /// Its chats that asked the person something and were ended there: no run.
    pub asked: u32,
    /// Each chat it ran, in run order, where one started.
    pub chats: Vec<ScoredChat>,
}

/// **One trigger test of a grade**, in each arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ScoredTest {
    /// The query as its chats read it, each character no reader would see marked.
    pub query: String,
    pub should_trigger: bool,
    pub arms: Vec<ScoredRuns>,
}

/// **A grade's scorecard** (SK-59, #3239; spec #2762 decision 43): the grade as its row in the
/// Grades panel says it, then each trigger test in each arm, with each of its chats one click
/// away where this machine kept it.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GradeScorecard {
    pub grade: String,
    pub skill: String,
    /// **The copy it graded, by its level**, where its row says one (#3978): a scorecard
    /// opened from where that copy is not listed (a retired tab comes back at the project's
    /// root) goes to the scope that lists it.
    pub level: Option<SkillLevel>,
    /// When it ran, the version it graded, what its chats ran on and how it ended, as its row
    /// says each.
    pub graded: String,
    pub version: String,
    pub ran_on: String,
    pub ended: String,
    /// The difference the skill made, in points, where both arms ran: against its previous
    /// version where the grade compared one (SK-57), else against no skill.
    pub delta: Option<f64>,
    /// **The version it compared with** (SK-57, #3237), its commit shortened, where it compared
    /// one; and the difference against no skill then, in points, where both arms ran.
    pub against: Option<String>,
    pub delta_without: Option<f64>,
    /// Whether a saved edit started it (SK-58, #3238), rather than a press of Grade.
    pub after_edit: bool,
    /// Whether it graded a draft (#2780): its arms are the draft and the current version.
    pub draft: bool,
    /// How many of its chats asked the person something and were ended there.
    pub asked: u32,
    pub tests: Vec<ScoredTest>,
    /// How it ended, its arms' pass rates and what it spent (S6): its end banner and totals.
    pub outcome: GradeOutcome,
    pub arms: GradeArms,
    pub spent: GradeSpent,
}

/// **How a grade ended, as its list marks it** (S6): graded whole, stopped by the person, stopped
/// at its limit (its token cap, its dollar limit, or a chat that did not say what it spent), or
/// interrupted (purlis stopped under it, or its chats did not start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum GradeOutcome {
    Graded,
    Stopped,
    AtLimit,
    Interrupted,
}

impl From<GradeEnd> for GradeOutcome {
    fn from(end: GradeEnd) -> GradeOutcome {
        match end {
            GradeEnd::Whole => GradeOutcome::Graded,
            GradeEnd::Cancelled => GradeOutcome::Stopped,
            GradeEnd::OverCap | GradeEnd::OverLimit | GradeEnd::SpendNotReported => {
                GradeOutcome::AtLimit
            }
            // A Grader that failed its question (#3236) left its grade with no scores of its own:
            // what ran is purlis's, and its row says why.
            GradeEnd::ChatsDidNotStart | GradeEnd::Interrupted | GradeEnd::GraderFailed => {
                GradeOutcome::Interrupted
            }
        }
    }
}

/// **Each arm's pass rate**, in points from 0 to 100, where the arm ran: the Grades tab's bars
/// and a scorecard's totals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
pub struct GradeArms {
    pub with: Option<f64>,
    pub without: Option<f64>,
    pub previous: Option<f64>,
}

/// **What a grade spent**: the chats that ran, their tokens and their cost where they said them,
/// and the dollar limit it was held to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
pub struct GradeSpent {
    pub chats: u32,
    pub tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    pub limit_usd: Option<f64>,
}

impl GradeArms {
    fn of(graded: &Graded) -> GradeArms {
        GradeArms::by(graded, |score| score.pass_rate)
    }

    /// Each arm's rate as `rate` reads it, in points.
    fn by(
        graded: &Graded,
        rate: impl Fn(&grading::ArmScore) -> Option<grading::Stat>,
    ) -> GradeArms {
        let points = |arm: Arm| {
            graded
                .report
                .arm(arm)
                .and_then(&rate)
                .map(|stat| (stat.mean * 100.0).round())
        };
        GradeArms {
            with: points(Arm::With),
            without: points(Arm::Without),
            previous: points(Arm::Previous),
        }
    }
}

impl GradeSpent {
    fn of(graded: &Graded) -> GradeSpent {
        GradeSpent {
            chats: graded.spent.chats,
            tokens: graded.spent.tokens.map(|tokens| tokens as f64),
            cost_usd: graded.spent.cost_usd,
            limit_usd: graded.report.cost_cap_usd,
        }
    }
}

/// **One grade in a skill's Grades tab** (S6): when it ran, the version, what it ran on, how it
/// ended, each arm's pass rate and what it spent; its id opens its scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GradeRow {
    pub grade: String,
    pub graded: String,
    pub version: String,
    pub ran_on: String,
    /// How it ended, in the record's words, and as its mark says it.
    pub ended: String,
    pub outcome: GradeOutcome,
    pub arms: GradeArms,
    pub spent: GradeSpent,
    pub after_edit: bool,
}

impl GradeRow {
    fn of(graded: &Graded) -> GradeRow {
        use purlis_core::skills::studio;
        GradeRow {
            grade: graded.grade.clone(),
            graded: studio::said_at(&graded.ts),
            version: studio::version_said(graded),
            ran_on: studio::ran_on_said(graded),
            ended: grades::ended_said(graded.report.ended).to_owned(),
            outcome: graded.report.ended.into(),
            arms: GradeArms::of(graded),
            spent: GradeSpent::of(graded),
            after_edit: graded.after_edit,
        }
    }
}

/// **A copy's grades, newest first** (S6), and what the record left out as it was read; and what
/// the Grades tab says besides, so it asks no plan (#3928): how many trigger tests a grade would
/// run, or why they were not read, and the grade of the skill running now, at its latest step.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGrades {
    pub grades: Vec<GradeRow>,
    pub said: Option<String>,
    pub tests: u32,
    /// The outcome evals a grade would run too (SK-55): a skill with these alone is graded.
    pub evals: u32,
    pub tests_refused: Option<String>,
    pub running: Option<SkillGradeStep>,
}

impl SkillGrades {
    /// The grades `read` of `skill` at `level` in `plane`, its trigger tests `tests` (or why
    /// they could not be counted), and the grade of that copy running now, where one is.
    fn of(
        read: &grades::Read,
        tests: Result<grades::TriggerTests, String>,
        plane: &PlaneId,
        level: &Level,
        skill: &str,
    ) -> Self {
        // A copy that cannot be found keeps its grades: only its tests are not counted.
        let (tests, evals, tests_refused) = tests_said(tests);
        SkillGrades {
            grades: read.grades.iter().map(GradeRow::of).collect(),
            said: read.said(),
            tests,
            evals,
            tests_refused,
            running: step_of_running(plane, level, skill),
        }
    }
}

/// **What a grade would run, as the window is told it**: its trigger tests and outcome evals,
/// counted, and why the tests were not read, where they were not.
fn tests_said(tests: Result<grades::TriggerTests, String>) -> (u32, u32, Option<String>) {
    match tests {
        Ok(tests) => (
            u32::try_from(tests.runs).unwrap_or(u32::MAX),
            u32::try_from(tests.evals).unwrap_or(u32::MAX),
            tests
                .refused
                .as_ref()
                .map(purlis_core::skills::grade::Refused::in_window),
        ),
        Err(why) => (0, 0, Some(why)),
    }
}

/// The latest step of the grade of the copy of `skill` at `level` running in `plane` now, where
/// one is.
fn step_of_running(plane: &PlaneId, level: &Level, skill: &str) -> Option<SkillGradeStep> {
    running()
        .values()
        .filter(|one| {
            one.plane == *plane
                && one.skill == skill
                && one.level.as_ref() == Some(level)
                && one.draft.is_none()
        })
        .find_map(|one| match &one.step {
            RunningStep::Skill(step) => Some(step.clone()),
            RunningStep::Router(_) => None,
        })
}

/// **The grades of the copy of `skill` at `level`** (S6), as the eval record holds them: what the
/// skill's Grades tab draws its bars and its list from, with whether it has trigger tests and the
/// grade of it running now (#3928), and what a running grade's page starts from.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grades(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
) -> Result<SkillGrades, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a skill's grades", move || {
        flag_on()?;
        let level = Level::from(&level);
        // With the grades from before a rename or a move (#3918).
        let read = grades::read_followed(held.root(), &level, &skill);
        let tests = grades::trigger_tests(held.root(), &level, &skill);
        Ok(SkillGrades::of(&read, tests, &plane, &level, &skill))
    })
    .await
}

/// **Each trigger test of a grade's report, in each arm** (SK-59): what a skill's scorecard and
/// each skill of a router's (#3617) show, each chat `kept` says whether this machine kept.
pub(crate) fn scored_tests(
    triggers: &[purlis_core::skills::grade::TriggerScore],
    kept: impl Fn(&str) -> bool,
) -> Vec<ScoredTest> {
    let mut tests: Vec<ScoredTest> = Vec::new();
    for score in triggers {
        let runs = ScoredRuns {
            arm: score.arm.into(),
            runs: score.runs,
            triggered: score.triggered,
            pass: score.pass,
            asked: score.asked,
            chats: score
                .chats
                .iter()
                .zip(1u32..)
                .filter_map(|(id, run)| {
                    id.as_ref().map(|id| ScoredChat {
                        id: id.clone(),
                        run,
                        kept: kept(id),
                    })
                })
                .collect(),
        };
        match tests
            .iter_mut()
            .find(|test| test_is(test, &score.query, score.should_trigger))
        {
            Some(test) => test.arms.push(runs),
            None => tests.push(ScoredTest {
                query: purlis_unicode::marked(&score.query, purlis_unicode::Breaks::Marked),
                should_trigger: score.should_trigger,
                arms: vec![runs],
            }),
        }
    }
    tests
}

impl GradeScorecard {
    /// `graded`'s scorecard, each chat `kept` says whether this machine kept.
    fn of(graded: &Graded, kept: impl Fn(&str) -> bool) -> GradeScorecard {
        use purlis_core::skills::studio;
        let tests = scored_tests(&graded.report.triggers, kept);
        GradeScorecard {
            grade: graded.grade.clone(),
            skill: graded.skill.clone(),
            level: graded
                .level
                .as_deref()
                .and_then(grades::level_of_recorded)
                .map(|level| (&level).into()),
            graded: studio::said_at(&graded.ts),
            version: studio::version_said(graded),
            ran_on: studio::ran_on_said(graded),
            // With a Grader's own sentence, where it ended its grade (#3236).
            ended: match &graded.report.failed {
                Some(why) => format!("{} {why}", grades::ended_said(graded.report.ended)),
                None => grades::ended_said(graded.report.ended).to_owned(),
            },
            delta: graded.report.delta.map(|delta| (delta * 100.0).round()),
            against: graded
                .compared_with()
                .map(|commit| purlis_core::skills::history::short(commit).to_owned()),
            delta_without: graded
                .report
                .arm(Arm::Previous)
                .and_then(|_| {
                    let mean = |arm: Arm| {
                        graded
                            .report
                            .arm(arm)
                            .and_then(|score| score.pass_rate)
                            .map(|stat| stat.mean)
                    };
                    mean(Arm::With).zip(mean(Arm::Without))
                })
                .map(|(with, without)| ((with - without) * 100.0).round()),
            after_edit: graded.after_edit,
            draft: graded.draft.is_some(),
            asked: graded.report.asked(),
            tests,
            outcome: graded.report.ended.into(),
            arms: GradeArms::of(graded),
            spent: GradeSpent::of(graded),
        }
    }
}

/// Whether `test` is the trigger test of `query` that should or should not trigger.
fn test_is(test: &ScoredTest, query: &str, should_trigger: bool) -> bool {
    test.should_trigger == should_trigger
        && test.query == purlis_unicode::marked(query, purlis_unicode::Breaks::Marked)
}

/// **The scorecard of `skill`'s grade `grade`** (SK-59, #3239), as the eval record holds it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_scorecard(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    skill: String,
    grade: String,
) -> Result<GradeScorecard, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a grade's scorecard", move || {
        flag_on()?;
        let root = held.root();
        let graded = grades::grade_of(root, &skill, &grade)
            .ok_or_else(|| format!("That grade of {skill} is not in the record any more."))?;
        Ok(GradeScorecard::of(&graded, |chat| {
            grades::kept::kept(root, &skill, &grade, chat).is_some()
        }))
    })
    .await
}

/// What a scorecard says of a chat this machine does not keep: one a teammate's purlis ran, one
/// past the newest grades `whose` ("the skill's", "the router's"), or one whose screen could not
/// be read as it ended.
pub(crate) fn not_kept(whose: &str) -> String {
    format!(
        "This chat is not kept on this machine. purlis keeps a grade's chats where the grade ran, \
         for {whose} {} newest grades, where their screen could be read as they ended.",
        grades::kept::KEPT_GRADES
    )
}

/// **What chat `chat` of `skill`'s grade `grade` showed as it ended** (SK-59, #3239): its screen
/// and scrollback as the escape sequences that draw them, for a tab that shows it read-only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_chat(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    skill: String,
    grade: String,
    chat: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a grade's chat", move || {
        flag_on()?;
        grades::kept::kept(held.root(), &skill, &grade, &chat)
            .ok_or_else(|| not_kept("the skill's"))
    })
    .await
}

/// **This app's host for the built-in runner**: each chat a grade asks for runs in the project,
/// as a grade's chat (`curation::start_graded`), one at a time.
pub(crate) struct InChats<'a> {
    pub(crate) held: &'a Arc<Held>,
    /// Lays out the throwaway piece of an arm's chat, from what the grade read once: the
    /// skill's copy, its previous version's, none, or a router's grade's library (SK-15e).
    pub(crate) lay_out: &'a dyn Fn(Arm) -> Result<purlis_core::skills::verbs::drafts::Try, String>,
    /// What the grade may still cost, where its chats say: a chat that passes it is ended.
    pub(crate) cost_left: Option<f64>,
    /// What a chat that says its tokens and no cost is counted at against [`Self::cost_left`],
    /// in dollars a million: the grade's own `Limits::unpriced_usd_per_million`, as the runner
    /// counts it.
    pub(crate) unpriced_at: Option<f64>,
    /// The skill whose tests run now: what a chat ends at, once it starts.
    pub(crate) skill: String,
    /// What each chat is labelled, in the session host's list.
    pub(crate) label: String,
    /// Whose scorecard what each chat showed is kept for (SK-59): the skill's, or a router's
    /// (#3617).
    pub(crate) keeps: Option<grades::kept::Of>,
    pub(crate) profile: &'a str,
    pub(crate) stop: &'a AtomicBool,
    /// What the chats that ran so far spent, and who is told each, with how many asked.
    pub(crate) spent: Spent,
    /// Told what the chats that ran so far spent, and how many asked, as each one ends.
    pub(crate) tell: &'a dyn Fn(Spent, u32),
    /// Who is told what the chats spent so far, and how many it asked for, ran or not
    /// (`Spent::asked_for`, #3602), as each chat ends: the grade's marker (#3582).
    pub(crate) on_spent: &'a dyn Fn(Spent),
    /// The chats the grade asked for so far, ran or not (#3602).
    pub(crate) asked_for: u32,
    /// The grade's id: what each chat's screen is kept under (SK-59).
    pub(crate) grade: &'a str,
    /// The chats that asked the person something and were ended there (SK-59).
    pub(crate) asked: u32,
    /// The harness the chats ran on, as the first that started said.
    pub(crate) harness: Option<String>,
    /// The plugins its chats were handed off (#3468): every one any chat that started was handed
    /// off, as each start reads the record again. `None` until one starts.
    pub(crate) plugins_off: Option<Vec<String>>,
    /// The model the person picked for its chats, or `None` for the profile's own (SK-60).
    pub(crate) model: Option<&'a str>,
    /// The model each chat's harness said it ran on, in the order they ran (SK-60): what the
    /// record keeps (`grades::reported_model`).
    pub(crate) reported: Vec<String>,
    /// **The input files the grade's outcome evals stage** (SK-55), as the grade read them once
    /// (`grades::SkillCopy::inputs`): none in a router's grade.
    pub(crate) inputs: &'a std::collections::BTreeMap<String, String>,
    /// **What the last outcome eval's chat left**, kept for its judge alone (SK-55): its id, its
    /// files as it ended and its final answer. Taken by the judge of that chat, and by no other.
    pub(crate) left: Option<Left>,
}

/// **What an outcome eval's chat left** (SK-55): its id, its piece's files as it ended
/// (`drafts::outcome_of`), and what its turn ended saying, where its harness said.
pub(crate) struct Left {
    chat: String,
    files: Vec<(String, String)>,
    answer: Option<String>,
}

impl Chats for InChats<'_> {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let ran = self.run_one(chat, tokens_left);
        self.spent = counted(self.spent, &ran);
        self.asked_for = self.asked_for.saturating_add(1);
        // Said in the grade's marker as each chat ends, so a grade purlis stops under is
        // recorded with what its chats spent (#3582) and how many it asked for (#3602), which
        // its estimate shares that spend over: written after the chat closed, never while one
        // opens. A chat that spent nothing is counted too, so it is said even then.
        (self.on_spent)(Spent {
            asked_for: Some(self.asked_for),
            ..self.spent
        });
        match (said_cost(&ran), ran.tokens) {
            (Some(cost), _) => {
                self.cost_left = self.cost_left.map(|left| left - cost);
            }
            // Tokens said and no cost: counted at the dearest price, as the runner counts them.
            (None, Some(tokens)) => {
                if let Some(at) = self.unpriced_at {
                    self.cost_left = self
                        .cost_left
                        .map(|left| left - tokens as f64 * at / 1_000_000.0);
                }
            }
            (None, None) => {}
        }
        if ran.end == ChatEnd::Asked {
            self.asked += 1;
        }
        if matches!(
            ran.end,
            ChatEnd::Turn
                | ChatEnd::TimedOut
                | ChatEnd::Asked
                | ChatEnd::ClosedEarly { .. }
                | ChatEnd::ReadPast
        ) {
            (self.tell)(self.spent, self.asked);
        }
        ran
    }
}

/// What the chat `ran` cost, as its harness said it, where it is a cost a reader could hold: as
/// the runner counts it, any other is no cost.
fn said_cost(ran: &ChatRan) -> Option<f64> {
    ran.cost_usd.filter(|cost| cost.is_finite() && *cost >= 0.0)
}

/// **What a grade's chats spent, `spent`, with the chat `ran` counted**: its tokens and its cost
/// where it said them, whatever ended it, as every chat that started counts toward the caps; and
/// the chat itself where it ran (a turn, or its time out), as the runner's arms count it. A cost
/// no reader could hold is none ([`said_cost`]), so nothing is counted past what the chats said.
fn counted(spent: Spent, ran: &ChatRan) -> Spent {
    let cost = said_cost(ran);
    Spent {
        chats: spent.chats + u32::from(matches!(ran.end, ChatEnd::Turn | ChatEnd::TimedOut)),
        tokens: match ran.tokens {
            Some(tokens) => Some(spent.tokens.unwrap_or(0).saturating_add(tokens)),
            None => spent.tokens,
        },
        cost_usd: match cost {
            Some(cost) => Some(spent.cost_usd.unwrap_or(0.0) + cost),
            None => spent.cost_usd,
        },
        asked_for: spent.asked_for,
    }
}

impl InChats<'_> {
    fn run_one(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let not_started = |why: String| ChatRan {
            chat: None,
            started: Vec::new(),
            tokens: None,
            cost_usd: None,
            ms: 0,
            end: ChatEnd::NotStarted { why },
            answer: None,
        };
        let root = self.held.root().to_path_buf();
        let mut guard_down = false;
        let laid = match &chat.kind {
            // A judge's piece holds what the chat it judges left, as purlis kept it, and its
            // task: nothing of the skill, and nothing else (SK-55).
            ChatKind::Judge { of, task } => {
                let Some(left) = self.left.take().filter(|left| left.chat == *of) else {
                    return not_started(
                        "purlis kept nothing of the chat it was to judge, so no judge could read                          it"
                        .to_owned(),
                    );
                };
                purlis_core::skills::verbs::drafts::lay_out_judge(
                    &root,
                    &left.files,
                    left.answer.as_deref(),
                    task,
                )
            }
            ChatKind::Outcome { files } => (self.lay_out)(chat.arm).and_then(|laid| {
                match purlis_core::skills::verbs::drafts::stage(&root, &laid, files, self.inputs) {
                    Ok(())
                        if !self.held.chats().binary().is_some_and(|binary| {
                            purlis_core::toolhooks::outcome_guard_answers(binary, &laid.piece)
                        }) =>
                    {
                        let _ = purlis_core::skills::verbs::drafts::end_try(&root, &laid.id);
                        guard_down = true;
                        Err(String::new())
                    }
                    Ok(()) => Ok(laid),
                    Err(why) => {
                        let _ = purlis_core::skills::verbs::drafts::end_try(&root, &laid.id);
                        Err(why)
                    }
                }
            }),
            ChatKind::Trigger => (self.lay_out)(chat.arm),
        };
        if guard_down {
            return ChatRan {
                end: ChatEnd::NotHeld {
                    why: "purlis could not hold this chat's writes to its folder, so it did not \
                          run and the eval failed."
                        .to_owned(),
                },
                ..not_started(String::new())
            };
        }
        let laid = match laid {
            Ok(laid) => laid,
            Err(why) => return not_started(why),
        };
        if matches!(chat.kind, ChatKind::Judge { .. })
            && !self.held.chats().binary().is_some_and(|binary| {
                purlis_core::toolhooks::judge_guard_answers(binary, &laid.piece)
            })
        {
            let _ = purlis_core::skills::verbs::drafts::end_try(&root, &laid.id);
            return not_started("purlis could not start a judge within its limits".to_owned());
        }
        // An outcome eval's chat and a judge's run their turn whole: only a trigger test's ends
        // once the skill under test starts.
        let whole_turn = !chat.kind.is_trigger();
        let watching = self.held.hooks().skill_runs();
        let mut told = None;
        let started = crate::curation::start_graded(
            self.held,
            ChatTyped {
                profile: self.profile.to_owned(),
                persona: None,
                cwd: laid.piece.clone(),
                label: self.label.clone(),
                prompt: chat.prompt.clone(),
                then_a_space: false,
                starts_runs: Some(StartedBy::Grade),
            },
            SIZE,
            self.model,
            |number| told = Some((number, watching.watch(number))),
        );
        let began = Instant::now();
        let started = match started {
            Ok(started) => started,
            Err(why) => {
                if let Some((number, _)) = told {
                    watching.unwatch(number);
                }
                let _ = purlis_core::skills::verbs::drafts::end_try(&root, &laid.id);
                return not_started(why);
            }
        };
        if self.harness.is_none() {
            self.harness = started
                .ready
                .harness
                .map(|harness| harness.name().to_owned());
        }
        // Each chat's start reads the record again, so what any of them had off is named.
        let off = self.plugins_off.get_or_insert_with(Vec::new);
        for id in grades::plugins_off(&started.ready) {
            if !off.contains(&id) {
                off.push(id);
            }
        }
        off.sort();
        let session = started.session;
        let id = self.held.chats().chat_at(session).and_then(|at| at.id);
        // A router's arm is a run of the router only where its start took the router's answer
        // (SK-15e): a chat that fell back on the built-in router measures that one instead, so
        // it is no run, and the grade says why. Its prompt was handed it at its start, so what
        // it spent before it was ended counts toward the grade's cap all the same.
        if chat.arm == Arm::Router
            && let Some(why) = router_did_not_route(self.held.chats(), session)
        {
            if let Some((number, _)) = told {
                watching.unwatch(number);
            }
            if let Err(close) = self.held.chats().close(session) {
                tracing::warn!("purlis: a grade's chat {session} did not close ({close})");
            }
            let spent = id
                .as_deref()
                .and_then(|id| purlis_core::usage::spent(&root, id));
            return ChatRan {
                chat: id,
                tokens: spent.and_then(|spent| {
                    spent
                        .input_tokens
                        .zip(spent.output_tokens)
                        .map(|(input, output)| input + output)
                }),
                cost_usd: spent.and_then(|spent| spent.cost_usd),
                ms: u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
                ..not_started(why)
            };
        }
        // What the chat has spent so far, as its harness last said: read each look, so a chat
        // that passes the grade's cap is ended in it.
        let spent_now = || {
            id.as_deref()
                .and_then(|id| purlis_core::usage::spent(&root, id))
                .and_then(|spent| {
                    spent
                        .input_tokens
                        .zip(spent.output_tokens)
                        .map(|(input, output)| input + output)
                })
        };
        // And what it has cost so far, where its harness says: the grade's second bound.
        let cost_now = || {
            id.as_deref()
                .and_then(|id| purlis_core::usage::spent(&root, id))
                .and_then(|spent| spent.cost_usd)
        };
        let mut skills: Vec<String> = Vec::new();
        let mut answer: Option<String> = None;
        let mut read_past = false;
        let mut has_begun = false;
        let judge = matches!(chat.kind, ChatKind::Judge { .. });
        let mut end = match &told {
            None => ChatEnd::TimedOut,
            Some((_, heard)) => loop {
                match heard.recv_timeout(LOOK_EVERY) {
                    Ok(Watched::Began) => has_begun = true,
                    Ok(Watched::Started(name)) => {
                        let this = purlis_core::skills::leaf_name(&name) == self.skill;
                        skills.push(name);
                        // A trigger test learns nothing more once the skill under test started.
                        if this && !whole_turn {
                            break ChatEnd::Turn;
                        }
                    }
                    Ok(Watched::Said(text)) => answer = Some(text),
                    // A judge is ended at once: nothing more of its turn is needed to fail it.
                    Ok(Watched::ReadPast) if judge => break ChatEnd::ReadPast,
                    Ok(Watched::RefusedOtherwise) if judge => {
                        break ChatEnd::ClosedEarly {
                            why: "purlis could not answer for what the judge read".to_owned(),
                        };
                    }
                    Ok(Watched::ReadPast | Watched::RefusedOtherwise) => {}
                    Ok(Watched::TurnEnded) => break ChatEnd::Turn,
                    // A grade's chat waits on nobody: ended at its first ask, with that end.
                    Ok(Watched::Asked) => break ChatEnd::Asked,
                    // A whole turn that closed before it ended left nothing to judge or be read:
                    // no run, never a score, so a crash is no verdict on the skill (SK-55).
                    Ok(Watched::Closed) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
                        if whole_turn =>
                    {
                        break ChatEnd::ClosedEarly {
                            why: "its chat closed before its turn ended".to_owned(),
                        };
                    }
                    Ok(Watched::Closed) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break ChatEnd::Turn;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                if !has_begun && began.elapsed() >= A_CHAT_BEGINS_WITHIN {
                    break ChatEnd::NotStarted {
                        why: NEVER_BEGAN.to_owned(),
                    };
                }
                // A program that ended with no hook to say so (signed out, say) tells nothing more.
                if !self.held.chats().sessions().running().contains(&session) {
                    break if whole_turn {
                        ChatEnd::ClosedEarly {
                            why: "its chat's program ended before its turn did".to_owned(),
                        }
                    } else {
                        ChatEnd::Turn
                    };
                }
                // After what was heard: a refusal queued as spend crosses a limit still counts.
                if self.cancelled() {
                    break ChatEnd::Cancelled;
                }
                if began.elapsed() >= CHAT_TIME {
                    break ChatEnd::TimedOut;
                }
                if let (Some(left), Some(now)) = (tokens_left, spent_now())
                    && now > left
                {
                    break ChatEnd::OverCap;
                }
                if let (Some(left), Some(now)) = (self.cost_left, cost_now())
                    && now > left
                {
                    break ChatEnd::OverLimit;
                }
            },
        };
        // What the turn ended saying, where it is heard just after the turn's end.
        if whole_turn
            && answer.is_none()
            && !read_past
            && matches!(end, ChatEnd::Turn)
            && let Some((_, heard)) = &told
        {
            let until = Instant::now() + ANSWER_COMES_WITHIN;
            while let Some(left) = until.checked_duration_since(Instant::now()) {
                match heard.recv_timeout(left) {
                    Ok(Watched::Said(text)) => {
                        answer = Some(text);
                        break;
                    }
                    Ok(Watched::ReadPast) if judge => {
                        read_past = true;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
        // A refusal heard behind its turn's end still fails it; a Stop or a limit stays what ended it.
        if judge && matches!(end, ChatEnd::Turn | ChatEnd::TimedOut) {
            if let Some((_, heard)) = &told {
                read_past |= heard.try_iter().any(|one| one == Watched::ReadPast);
            }
            if read_past {
                end = ChatEnd::ReadPast;
            }
        }
        if end == ChatEnd::ReadPast {
            answer = None;
        }
        // What an outcome eval's chat left, kept for its judge, read before its close takes
        // its piece (SK-55).
        if matches!(chat.kind, ChatKind::Outcome { .. })
            && matches!(end, ChatEnd::Turn | ChatEnd::TimedOut)
        {
            self.left = id.clone().map(|chat| Left {
                chat,
                files: purlis_core::skills::verbs::drafts::outcome_of(&root, &laid.piece),
                answer: answer.as_deref().map(|answer| {
                    purlis_core::skills::verbs::drafts::answer_as_its_judge_reads(
                        answer,
                        &laid.piece,
                    )
                }),
            });
        }
        let spent = id
            .as_deref()
            .and_then(|id| purlis_core::usage::spent(&root, id));
        // What its harness said it ran on, read before the close forgets it (SK-60).
        if let Some(model) = self.held.chats().model_of(session) {
            self.reported.push(model);
        }
        // What its terminal showed as it ended, kept to open from the scorecard (SK-59).
        if let (Some(chat), Some(of)) = (id.as_deref(), self.keeps.as_ref()) {
            match screen_of(self.held.chats().sessions(), session) {
                Some(screen) => {
                    if let Err(why) = grades::kept::keep_of(&root, of, self.grade, chat, &screen) {
                        tracing::warn!("purlis: a grade's chat {session} was not kept ({why})");
                    }
                }
                None => tracing::warn!("purlis: a grade's chat {session} showed nothing to keep"),
            }
        }
        watching.unwatch(session);
        if let Err(why) = self.held.chats().close(session) {
            tracing::warn!("purlis: a grade's chat {session} did not close ({why})");
        }
        ChatRan {
            chat: id,
            started: skills,
            tokens: spent.and_then(|spent| {
                spent
                    .input_tokens
                    .zip(spent.output_tokens)
                    .map(|(input, output)| input + output)
            }),
            cost_usd: spent.and_then(|spent| spent.cost_usd),
            ms: u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
            end,
            // A trigger test's last message is no part of its result, nor told to a Grader.
            answer: answer.filter(|_| whole_turn),
        }
    }
}

/// **Why chat `session` of a router's arm is no run of its router** (SK-15e): its start fell back
/// on the built-in router, in the words its routing says it, or asked no router at all. `None`
/// where the router's answer was taken.
fn router_did_not_route(chats: &crate::chats::Chats, session: u32) -> Option<String> {
    match chats
        .routings()
        .into_iter()
        .find(|one| one.session == session)
    {
        Some(one) if one.routed.record.fell_back.is_none() => None,
        Some(one) => Some(one.routed.said()),
        None => Some("its start asked no router, so it measured the built-in router".to_owned()),
    }
}

/// How long a grade's chat has to hand over its screen before it closes.
const A_SCREEN_TAKES_AT_MOST: Duration = Duration::from_secs(2);

/// How long a view of a grade's chat stays quiet before what it was sent is taken as the whole
/// screen: a host sends a snapshot in pieces, after a reset where the view is not its first.
const A_SCREEN_IS_WHOLE_AFTER: Duration = Duration::from_millis(300);

/// **What session `session` shows now, its scrollback with it**, as the escape sequences that draw
/// it into a blank terminal: what a view of it is sent first (`host::SessionHost::watch`), which
/// a host may send in pieces, so everything the view is sent until it is quiet for
/// [`A_SCREEN_IS_WHOLE_AFTER`], at most [`A_SCREEN_TAKES_AT_MOST`] and
/// `grades::kept::MOST_SCREEN_BYTES`. `None` where no view could be opened or nothing came.
fn screen_of(host: &dyn crate::host::SessionHost, session: u32) -> Option<String> {
    let (send, pieces) = std::sync::mpsc::channel();
    let view = host
        .watch(
            session,
            Box::new(move |text| {
                let _ = send.send(text);
            }),
        )
        .ok()?;
    let shown = gathered(&pieces, Instant::now());
    let _ = host.unwatch(session, view.view);
    shown
}

/// Everything `pieces` holds until it is quiet for [`A_SCREEN_IS_WHOLE_AFTER`] once something
/// was heard, at most [`A_SCREEN_TAKES_AT_MOST`] after `begun` and `grades::kept::MOST_SCREEN_BYTES`
/// long. `None` where nothing but empty pieces came.
fn gathered(pieces: &std::sync::mpsc::Receiver<String>, begun: Instant) -> Option<String> {
    let mut shown = String::new();
    let mut heard = false;
    loop {
        let left = A_SCREEN_TAKES_AT_MOST.saturating_sub(begun.elapsed());
        if left.is_zero() || shown.len() > grades::kept::MOST_SCREEN_BYTES {
            break;
        }
        let wait = if heard {
            left.min(A_SCREEN_IS_WHOLE_AFTER)
        } else {
            left
        };
        match pieces.recv_timeout(wait) {
            Ok(text) => {
                heard |= !text.is_empty();
                shown.push_str(&text);
            }
            Err(_) => break,
        }
    }
    heard.then_some(shown)
}

pub(crate) mod after_edit;
pub(crate) mod drafted;

#[cfg(test)]
mod tests;
