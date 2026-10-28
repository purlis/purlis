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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use tauri::Emitter as _;

use purlis_core::skills::grade::{
    self as grading, Arm, ChatEnd, ChatRan, Chats, GradeEnd, RunChat,
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

/// How often a chat being watched is asked whether the person stopped the grade.
const LOOK_EVERY: Duration = Duration::from_millis(500);

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

/// What a grade's chats spent before, worked out for this one (`grades::Estimate`): both
/// figures are `None` where that grade's chats did not say what they spent.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GradeEstimate {
    pub tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    /// The chats the grade it is worked out from asked for: none where none ran.
    pub chats_then: u32,
    /// That grade: when it ran, and the profile and the harness its chats ran on.
    pub ts: String,
    pub profile: Option<String>,
    pub harness: Option<String>,
    /// The model its chats reported (SK-60): a grade on another model spends otherwise.
    pub model: Option<String>,
    /// Whether it ran on the profile this grade runs on, or `None` where that is not known.
    pub same_profile: Option<bool>,
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
    /// Why the trigger tests could not be read, where they were not.
    pub tests_refused: Option<String>,
    /// The trigger tests left out, each with why: a query no chat can be started on.
    pub left_out: Vec<GradeLeftOut>,
    /// The outcome evals the skill has, shown and not run (#3235 grades them).
    pub outcome_evals: u32,
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
}

/// **Where a grade's chats run**, as [`chats_on`] reads it.
struct On {
    /// The profile, where one was found.
    profile: Option<String>,
    /// The sandbox's name there, or why no grade can run on it.
    checked: Result<String, String>,
    /// The price its dollar limit is worked out at.
    priced: grading::Priced,
    /// The models its harness offers for a grade, and the one picked (SK-60, #3240).
    models: Vec<String>,
    model: Option<String>,
}

/// **Whether a grade's chats can run on `profile`** (the project's default where none) and on
/// `model` (the profile's own where none) in the project at `root`: the profile's name and the
/// sandbox's, or why not (R1 and R2 of the operator's ruling of 2026-10-26); the price its
/// dollar limit is worked out at there (`grades::priced`), the dearest where no chat could start;
/// and the models its harness offers (`grades::models_for`).
fn chats_on(root: &std::path::Path, profile: Option<String>, model: Option<String>) -> On {
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
    }
}

/// **What pressing Grade on `planned`, its chats `on`, consents to**: what it grades, and the
/// profile, model, token cap and dollar limit the plan shows (`grades::consent_on`). `None`
/// where there is nothing to grade.
fn consent_shown(planned: &Planned, on: &On) -> Option<String> {
    planned.consent.as_deref().map(|consent| {
        grades::consent_on(
            consent,
            &grades::Terms {
                profile: on.profile.as_deref(),
                model: on.model.as_deref(),
                token_cap: planned.cap.tokens,
                limit_usd: planned.cap.cost_usd(&on.priced),
            },
        )
    })
}

/// **Whether a press with `consent` was made on the plan a grade of `planned`, on `on`, would
/// start on now**, or the sentence that refuses it: what it grades, the profile, the model, the
/// cap or the limit changed since the plan was shown, and the window shows the new one.
fn plan_holds(
    planned: &Planned,
    on: &On,
    consent: &str,
    skill: &str,
    so: &str,
) -> Result<(), String> {
    if consent_shown(planned, on).as_deref() == Some(consent) {
        return Ok(());
    }
    Err(format!(
        "The plan of {skill} changed since it was shown (what it grades, the profile or model \
         its chats run on, or its limits), so {so}: read the new plan and press again."
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
        } = on;
        SkillGradePlan {
            tests,
            chats: planned.plan.map(|plan| plan.chats).unwrap_or_default(),
            runs: u32::from(grading::RUNS),
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
            outcome_evals: planned
                .fixtures
                .outcomes
                .read()
                .map(|evals| u32::try_from(evals.len()).unwrap_or(u32::MAX))
                .unwrap_or_default(),
            outcomes_refused: planned
                .fixtures
                .outcomes
                .refused()
                .map(purlis_core::skills::grade::Refused::in_window),
            estimate: planned.estimate.as_ref().map(|estimate| GradeEstimate {
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
            }),
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
            // Recorded at a start for a grade purlis stopped under, never told as one ends.
            GradeEnd::Interrupted => GradeEnded::Failed,
        }
    }
}

/// **One step of a grade**, as [`EVENT`] tells the window: how many of its chats have run, and,
/// once it is over, how it ended and what it says.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SkillGradeStep {
    pub plane: PlaneId,
    pub skill: String,
    pub grade: String,
    pub ran: u32,
    pub of: u32,
    /// What its chats have spent so far, where they said, and the cap it is held to.
    pub tokens: Option<f64>,
    pub cap: f64,
    /// How many of its chats asked the person something and were ended there (SK-59, #3239):
    /// said in the grade, as none of them is a run.
    pub asked: u32,
    /// How it ended, once it has.
    pub ended: Option<GradeEnded>,
    /// What the window says of its end: why its chats did not start, or that it was not
    /// recorded.
    pub said: Option<String>,
}

/// One grade running, and its way to be stopped.
struct Running {
    plane: PlaneId,
    skill: String,
    stop: Arc<AtomicBool>,
}

/// **The grades running in this app**, by id: one per project at a time.
static RUNNING: std::sync::LazyLock<Mutex<HashMap<String, Running>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// The grades running now, held.
fn running() -> std::sync::MutexGuard<'static, HashMap<String, Running>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// **Runs the grade `grade`'s `body`, and the grade is not running after it, whatever happened
/// in it** (#3353): a panic in it is handed to `failed` once the grade is no longer running, so
/// a grade never stays running with nothing left to end it. That catch works in dev and test
/// builds alone: every shipped build aborts on a panic (`panic = "abort"` in the release
/// profile), so there the app ends with it, and the project's next start recording the grade as
/// interrupted (`grades::underway`) is the floor, by decision (#3443).
fn held_to_its_end(grade: &str, body: impl FnOnce(), failed: impl FnOnce()) {
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

/// The grade of `skill` running in `plane` now, by id.
fn running_of(plane: &PlaneId, skill: &str) -> Option<String> {
    running()
        .iter()
        .find(|(_, one)| one.plane == *plane && one.skill == skill)
        .map(|(id, _)| id.clone())
}

/// **What grading the copy of `skill` at `level` would run**, before anything runs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_plan(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
    profile: Option<String>,
    model: Option<String>,
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a grade runs", move || {
        flag_on()?;
        let on = chats_on(held.root(), profile, model);
        let planned =
            grades::planned(held.root(), &(&level).into(), &skill, on.profile.as_deref())?;
        Ok(SkillGradePlan::of(&planned, running_of(&plane, &skill), on))
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
    consent: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting a grade", move || {
        flag_on()?;
        let level: Level = (&level).into();
        let root = held.root().to_path_buf();
        let on = chats_on(&root, profile, model);
        // The estimate is the plan's to show: a start needs none of it.
        let planned = grades::planned(&root, &level, &skill, None)?;
        // What runs is what the plan showed (the review of #3229): the tests, the skill's files
        // and what its folder holds, read once here and laid out from this one copy, on the
        // profile, model and limits it showed.
        plan_holds(&planned, &on, &consent, &skill, "nothing was started")?;
        // The one read of the copy the consent was taken from: laid out and recorded as is.
        let (Some(copy), Some(request)) = (planned.copy, planned.request) else {
            return Err(format!(
                "{skill} has no trigger tests to grade, so nothing was started."
            ));
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
                    skill: skill.clone(),
                    stop: Arc::clone(&stop),
                },
            );
        }
        let of = grading::plan(&request).chats;
        // The cap the plan showed, read again as the grade starts: the one it is held to.
        let cap = planned.cap.tokens;
        let limits = grading::Limits {
            tokens: cap,
            cost_usd: Some(planned.cap.cost_usd(&priced)),
            priced: Some(priced),
            unpriced_usd_per_million: Some(grades::price::DEAREST_USD_PER_MILLION),
        };
        let grade = id.clone();
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
            report: grades::underway::interrupted_report(
                cap,
                limits.cost_usd,
                limits.priced.clone(),
            ),
            plugins_off: None,
            model: None,
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
                let tell = |ran: u32,
                            tokens: Option<u64>,
                            asked: u32,
                            ended: Option<GradeEnded>,
                            said: Option<String>| {
                    let _ = app.emit(
                        EVENT,
                        SkillGradeStep {
                            plane: plane.clone(),
                            skill: skill.clone(),
                            grade: grade.clone(),
                            ran,
                            of,
                            tokens: tokens.map(|tokens| tokens as f64),
                            cap: cap as f64,
                            asked,
                            ended,
                            said,
                        },
                    );
                };
                tell(0, None, 0, None, None);
                let run = || {
                    let mut chats = InChats {
                        held: &held,
                        snapshot: &copy.snapshot,
                        cost_left: limits.cost_usd,
                        unpriced_at: limits.unpriced_usd_per_million,
                        skill: &skill,
                        profile: &profile,
                        stop: &stop,
                        spent: Spent::default(),
                        tell: &|ran, tokens, asked| tell(ran, tokens, asked, None, None),
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
                        harness: None,
                        plugins_off: None,
                        model: model.as_deref(),
                        reported: Vec::new(),
                    };
                    let report = grading::grade(&request, &mut chats, Some(limits.clone()));
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
                    grades::kept::sweep(&root, &skill);
                    drop(marker.borrow_mut().take());
                    running().remove(&grade);
                    let said = match (&graded.report.not_started, recorded) {
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
                        graded.spent.chats,
                        graded.report.spent_tokens,
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
                    tell(0, None, 0, Some(GradeEnded::Failed), Some(said.to_owned()));
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

/// **Sets the token cap a grade in the project is held to**, on this machine (#2779; the
/// operator's ruling of 2026-10-26): the person who pays for the chats sets it, and the plan shows
/// it before Grade is pressed. The answer is the plan of `skill` at `level` again, with it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_set_cap(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
    profile: Option<String>,
    model: Option<String>,
    tokens: f64,
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("setting a grade's cap", move || {
        flag_on()?;
        if !tokens.is_finite() || tokens.fract() != 0.0 || tokens < 0.0 {
            return Err(
                "A grade's cap is a whole number of tokens, so nothing was changed.".into(),
            );
        }
        grades::cap::set_token_cap(held.root(), tokens as u64)?;
        let on = chats_on(held.root(), profile, model);
        let planned =
            grades::planned(held.root(), &(&level).into(), &skill, on.profile.as_deref())?;
        Ok(SkillGradePlan::of(&planned, running_of(&plane, &skill), on))
    })
    .await
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
}

impl From<Arm> for ScoredArm {
    fn from(arm: Arm) -> ScoredArm {
        match arm {
            Arm::With => ScoredArm::With,
            Arm::Without => ScoredArm::Without,
            Arm::Previous => ScoredArm::Previous,
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
    /// When it ran, the version it graded, what its chats ran on and how it ended, as its row
    /// says each.
    pub graded: String,
    pub version: String,
    pub ran_on: String,
    pub ended: String,
    /// The difference the skill made, in points, where both arms ran.
    pub delta: Option<f64>,
    /// How many of its chats asked the person something and were ended there.
    pub asked: u32,
    pub tests: Vec<ScoredTest>,
}

impl GradeScorecard {
    /// `graded`'s scorecard, each chat `kept` says whether this machine kept.
    fn of(graded: &Graded, kept: impl Fn(&str) -> bool) -> GradeScorecard {
        use purlis_core::skills::studio;
        let mut tests: Vec<ScoredTest> = Vec::new();
        for score in &graded.report.triggers {
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
        GradeScorecard {
            grade: graded.grade.clone(),
            skill: graded.skill.clone(),
            graded: studio::said_at(&graded.ts),
            version: studio::version_said(graded),
            ran_on: studio::ran_on_said(graded),
            ended: grades::ended_said(graded.report.ended).to_owned(),
            delta: graded.report.delta.map(|delta| (delta * 100.0).round()),
            asked: graded.report.asked(),
            tests,
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
/// past the skill's newest grades, or one whose screen could not be read as it ended.
fn not_kept() -> String {
    format!(
        "This chat is not kept on this machine. purlis keeps a grade's chats where the grade ran, \
         for the skill's {} newest grades, where their screen could be read as they ended.",
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
        grades::kept::kept(held.root(), &skill, &grade, &chat).ok_or_else(not_kept)
    })
    .await
}

/// **This app's host for the built-in runner**: each chat a grade asks for runs in the project,
/// as a grade's chat (`curation::start_graded`), one at a time.
struct InChats<'a> {
    held: &'a Arc<Held>,
    /// The skill's copy, read once for the grade.
    snapshot: &'a purlis_core::skills::verbs::drafts::Snapshot,
    /// What the grade may still cost, where its chats say: a chat that passes it is ended.
    cost_left: Option<f64>,
    /// What a chat that says its tokens and no cost is counted at against [`Self::cost_left`],
    /// in dollars a million: the grade's own `Limits::unpriced_usd_per_million`, as the runner
    /// counts it.
    unpriced_at: Option<f64>,
    skill: &'a str,
    profile: &'a str,
    stop: &'a AtomicBool,
    /// What the chats that ran so far spent, and who is told each, with how many asked.
    spent: Spent,
    tell: &'a dyn Fn(u32, Option<u64>, u32),
    /// Who is told what the chats spent so far, each time a chat's end changes it: the grade's
    /// marker (#3582).
    on_spent: &'a dyn Fn(Spent),
    /// The grade's id: what each chat's screen is kept under (SK-59).
    grade: &'a str,
    /// The chats that asked the person something and were ended there (SK-59).
    asked: u32,
    /// The harness the chats ran on, as the first that started said.
    harness: Option<String>,
    /// The plugins its chats were handed off (#3468): every one any chat that started was handed
    /// off, as each start reads the record again. `None` until one starts.
    plugins_off: Option<Vec<String>>,
    /// The model the person picked for its chats, or `None` for the profile's own (SK-60).
    model: Option<&'a str>,
    /// The model each chat's harness said it ran on, in the order they ran (SK-60): what the
    /// record keeps (`grades::reported_model`).
    reported: Vec<String>,
}

impl Chats for InChats<'_> {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let ran = self.run_one(chat, tokens_left);
        let was = self.spent;
        self.spent = counted(self.spent, &ran);
        // Said in the grade's marker as each chat ends, so a grade purlis stops under is
        // recorded with what its chats spent (#3582): written after the chat closed, never
        // while one opens.
        if self.spent != was {
            (self.on_spent)(self.spent);
        }
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
        if matches!(ran.end, ChatEnd::Turn | ChatEnd::TimedOut | ChatEnd::Asked) {
            (self.tell)(self.spent.chats, self.spent.tokens, self.asked);
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
        };
        let root = self.held.root().to_path_buf();
        let skill = chat.arm.has_the_skill().then_some(self.snapshot);
        // The previous version as an arm is #3237: the built-in runner asks for none now.
        if chat.arm == Arm::Previous {
            return not_started("this purlis grades no previous version".to_owned());
        }
        let laid = match purlis_core::skills::verbs::drafts::lay_out_arm(&root, skill) {
            Ok(laid) => laid,
            Err(why) => return not_started(why),
        };
        let watching = self.held.hooks().skill_runs();
        let mut told = None;
        let started = crate::curation::start_graded(
            self.held,
            ChatTyped {
                profile: self.profile.to_owned(),
                persona: None,
                cwd: laid.piece.clone(),
                label: format!("Grade {}", self.skill),
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
        let end = match &told {
            None => ChatEnd::TimedOut,
            Some((_, heard)) => loop {
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
                match heard.recv_timeout(LOOK_EVERY) {
                    Ok(Watched::Started(name)) => {
                        let this = purlis_core::skills::leaf_name(&name) == self.skill;
                        skills.push(name);
                        // A trigger test learns nothing more once the skill under test started.
                        if this {
                            break ChatEnd::Turn;
                        }
                    }
                    Ok(Watched::TurnEnded) => break ChatEnd::Turn,
                    // A grade's chat waits on nobody: ended at its first ask, with that end.
                    Ok(Watched::Asked) => break ChatEnd::Asked,
                    Ok(Watched::Closed) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break ChatEnd::Turn;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            },
        };
        let spent = id
            .as_deref()
            .and_then(|id| purlis_core::usage::spent(&root, id));
        // What its harness said it ran on, read before the close forgets it (SK-60).
        if let Some(model) = self.held.chats().model_of(session) {
            self.reported.push(model);
        }
        // What its terminal showed as it ended, kept to open from the scorecard (SK-59).
        if let Some(chat) = id.as_deref() {
            match screen_of(self.held.chats().sessions(), session) {
                Some(screen) => {
                    if let Err(why) =
                        grades::kept::keep(&root, self.skill, self.grade, chat, &screen)
                    {
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
        }
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

#[cfg(test)]
mod tests;
