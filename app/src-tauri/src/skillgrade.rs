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

/// What a grade's chats spent last time, worked out for this one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
pub struct GradeEstimate {
    pub tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    /// The chats of the grade it is worked out from.
    pub chats_then: u32,
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
    pub sandbox: Option<String>,
    pub refused: Option<String>,
    /// What pressing Grade consents to: handed back with the press, and a grade on another plan
    /// is refused.
    pub consent: Option<String>,
}

/// **Whether a grade's chats can run on `profile`** (the project's default where none) in the
/// project at `root`: the profile's name and the sandbox's, or why not (R1 and R2 of the
/// operator's ruling of 2026-10-26); and the price its dollar limit is worked out at there
/// (`grades::priced`), the dearest where no chat could start.
fn chats_on(
    root: &std::path::Path,
    profile: Option<String>,
) -> (Option<String>, Result<String, String>, grading::Priced) {
    let profile = match profile {
        Some(profile) => profile,
        None => match crate::curation::default_profile(root, "no grade can run") {
            Ok((profile, _)) => profile,
            Err(why) => return (None, Err(why), grades::price::of_model(None)),
        },
    };
    let start = purlis_core::start::Start {
        profile: Some(profile.clone()),
        name: "grade".to_owned(),
        cwd: Some(root.to_path_buf()),
        made: purlis_core::sandbox::Made::Window,
        ..Default::default()
    };
    let ready = grades::ready_to_grade(&start, root);
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
    (Some(profile), checked, priced)
}

impl SkillGradePlan {
    fn of(
        planned: &Planned,
        running: Option<String>,
        on: (Option<String>, Result<String, String>, grading::Priced),
    ) -> Self {
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
        let (profile, sandbox, priced) = on;
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
            estimate: planned.estimate.map(|estimate| GradeEstimate {
                tokens: estimate.tokens.map(|tokens| tokens as f64),
                cost_usd: estimate.cost_usd,
                chats_then: estimate.chats_then,
            }),
            running,
            token_cap: planned.cap.tokens as f64,
            cost_cap_usd: planned.cap.cost_usd(&priced),
            priced: (&priced).into(),
            cap_set_here: planned.cap.from == grades::cap::From::Yours,
            chat_minutes: u32::try_from(grades::CHAT_TIME.as_secs() / 60).unwrap_or(u32::MAX),
            profile,
            refused: sandbox.as_ref().err().cloned(),
            sandbox: sandbox.ok(),
            consent: planned.consent.clone(),
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
/// a grade never stays running with nothing left to end it. Where a build aborts on a panic, the
/// app ends with it, and the project's next start records the grade as interrupted
/// (`grades::underway`).
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
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a grade runs", move || {
        flag_on()?;
        let planned = grades::planned(held.root(), &(&level).into(), &skill)?;
        let on = chats_on(held.root(), profile);
        Ok(SkillGradePlan::of(&planned, running_of(&plane, &skill), on))
    })
    .await
}

/// **Grades the copy of `skill` at `level`**, on `profile` (the project's default where none
/// is picked): its chats run one at a time behind the window, which is told each step
/// ([`EVENT`]). The answer is the grade's id. Refused while a grade runs in the project.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grade_start(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    level: SkillLevel,
    skill: String,
    profile: Option<String>,
    consent: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting a grade", move || {
        flag_on()?;
        let level: Level = (&level).into();
        let root = held.root().to_path_buf();
        let planned = grades::planned(&root, &level, &skill)?;
        // What runs is what the plan showed (the review of #3229): the tests, the skill's files
        // and what its folder holds, read once here and laid out from this one copy.
        if planned.consent.as_deref() != Some(consent.as_str()) {
            return Err(format!(
                "{skill} or its trigger tests changed since the plan was shown, so nothing was \
                 started: read the plan again."
            ));
        }
        // The one read of the copy the consent was taken from: laid out and recorded as is.
        let (Some(copy), Some(request)) = (planned.copy, planned.request) else {
            return Err(format!(
                "{skill} has no trigger tests to grade, so nothing was started."
            ));
        };
        let (profile, on, priced) = chats_on(&root, profile);
        on?;
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
                            ended,
                            said,
                        },
                    );
                };
                tell(0, None, None, None);
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
                        tell: &|ran, tokens| tell(ran, tokens, None, None),
                        harness: None,
                    };
                    let report = grading::grade(&request, &mut chats, Some(limits.clone()));
                    let harness = chats.harness.take();
                    let spent = chats.spent;
                    let graded = Graded {
                        harness,
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
        let planned = grades::planned(held.root(), &(&level).into(), &skill)?;
        let on = chats_on(held.root(), profile);
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
    /// What the chats that ran so far spent, and who is told each.
    spent: Spent,
    tell: &'a dyn Fn(u32, Option<u64>),
    /// The harness the chats ran on, as the first that started said.
    harness: Option<String>,
}

impl Chats for InChats<'_> {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let ran = self.run_one(chat, tokens_left);
        // What every chat that started spent counts toward the cap, a chat ended at it too.
        if let Some(tokens) = ran.tokens {
            *self.spent.tokens.get_or_insert(0) += tokens;
        }
        // As the runner counts it: a cost no reader could hold is no cost.
        match (
            ran.cost_usd.filter(|cost| cost.is_finite() && *cost >= 0.0),
            ran.tokens,
        ) {
            (Some(cost), _) => {
                *self.spent.cost_usd.get_or_insert(0.0) += cost;
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
        if matches!(ran.end, ChatEnd::Turn | ChatEnd::TimedOut) {
            self.spent.chats += 1;
            (self.tell)(self.spent.chats, self.spent.tokens);
        }
        ran
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

#[cfg(test)]
mod tests;
