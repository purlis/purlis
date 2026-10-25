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

/// What a grade's chats spent last time, worked out for this one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Type)]
pub struct GradeEstimate {
    pub tokens: Option<f64>,
    pub cost_usd: Option<f64>,
    /// The chats of the grade it is worked out from.
    pub chats_then: u32,
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
    pub left_out: Vec<String>,
    /// The outcome evals the skill has, which this version does not grade yet.
    pub outcome_evals: u32,
    pub outcomes_refused: Option<String>,
    pub estimate: Option<GradeEstimate>,
    /// The grade of this skill running now, where one is.
    pub running: Option<String>,
}

impl SkillGradePlan {
    fn of(planned: &Planned, running: Option<String>) -> Self {
        use purlis_core::skills::grade::Read;
        let refused = |read: &Read<_>| match read {
            Read::Refused(why) => Some(why.clone()),
            Read::Absent | Read::Read(_) => None,
        };
        let refused_outcomes =
            |read: &Read<Vec<purlis_core::skills::grade::OutcomeEval>>| match read {
                Read::Refused(why) => Some(why.clone()),
                Read::Absent | Read::Read(_) => None,
            };
        let tests = planned
            .request
            .as_ref()
            .map(|request| {
                request
                    .triggers
                    .iter()
                    .map(|test| GradeQuery {
                        query: purlis_unicode::marked(&test.query, purlis_unicode::Breaks::Marked),
                        should_trigger: test.should_trigger,
                        hides: purlis_unicode::hides(&test.query, purlis_unicode::Breaks::Kept),
                    })
                    .collect()
            })
            .unwrap_or_default();
        SkillGradePlan {
            tests,
            chats: planned.plan.map(|plan| plan.chats).unwrap_or_default(),
            runs: u32::from(grading::RUNS),
            tests_refused: refused(&planned.fixtures.triggers),
            left_out: planned
                .left_out
                .iter()
                .map(|(at, why)| format!("Test {}: {why}", at + 1))
                .collect(),
            outcome_evals: planned
                .fixtures
                .outcomes
                .read()
                .map(|evals| u32::try_from(evals.len()).unwrap_or(u32::MAX))
                .unwrap_or_default(),
            outcomes_refused: refused_outcomes(&planned.fixtures.outcomes),
            estimate: planned.estimate.map(|estimate| GradeEstimate {
                tokens: estimate.tokens.map(|tokens| tokens as f64),
                cost_usd: estimate.cost_usd,
                chats_then: estimate.chats_then,
            }),
            running,
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
}

impl From<GradeEnd> for GradeEnded {
    fn from(end: GradeEnd) -> GradeEnded {
        match end {
            GradeEnd::Whole => GradeEnded::Whole,
            GradeEnd::Cancelled => GradeEnded::Stopped,
            GradeEnd::ChatsDidNotStart => GradeEnded::ChatsDidNotStart,
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
) -> Result<SkillGradePlan, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what a grade runs", move || {
        flag_on()?;
        let planned = grades::planned(held.root(), &(&level).into(), &skill)?;
        Ok(SkillGradePlan::of(&planned, running_of(&plane, &skill)))
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
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("starting a grade", move || {
        flag_on()?;
        let level: Level = (&level).into();
        let root = held.root().to_path_buf();
        let planned = grades::planned(&root, &level, &skill)?;
        let request = planned.request.ok_or_else(|| {
            format!("{skill} has no trigger tests to grade, so nothing was started.")
        })?;
        let profile = match profile {
            Some(profile) => profile,
            None => crate::curation::default_profile(&root, "the grade did not start")?.0,
        };
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
        let grade = id.clone();
        std::thread::Builder::new()
            .name("purlis-grade".to_owned())
            .spawn(move || {
                let tell = |ran: u32, ended: Option<GradeEnded>, said: Option<String>| {
                    let _ = app.emit(
                        EVENT,
                        SkillGradeStep {
                            plane: plane.clone(),
                            skill: skill.clone(),
                            grade: grade.clone(),
                            ran,
                            of,
                            ended,
                            said,
                        },
                    );
                };
                tell(0, None, None);
                let mut chats = InChats {
                    held: &held,
                    level: &level,
                    skill: &skill,
                    profile: &profile,
                    stop: &stop,
                    spent: Spent::default(),
                    tell: &|ran| tell(ran, None, None),
                    harness: None,
                };
                let (version, commit) = grades::version_of(&root, &level, &skill);
                let report = grading::grade(&request, &mut chats);
                let harness = chats.harness.take();
                let spent = chats.spent;
                let graded = Graded {
                    v: grades::VERSION,
                    grade: grade.clone(),
                    skill: skill.clone(),
                    level: grades::level_word(&root, &level),
                    tree: version.word().to_owned(),
                    commit,
                    harness,
                    profile: Some(profile.clone()),
                    ts: purlis_core::dispatch::stamp(chrono::Utc::now()),
                    runs: request.runs,
                    spent,
                    report,
                };
                let recorded =
                    grades::record(&root, &graded, &purlis_core::machine::this_log_name());
                running().remove(&grade);
                let said = match (&graded.report.not_started, recorded) {
                    (Some(why), _) => Some(why.clone()),
                    (None, None) => Some(
                        "The grade ran, but this project could not be written, so it was not \
                         recorded."
                            .to_owned(),
                    ),
                    (None, Some(_)) => None,
                };
                tell(graded.spent.chats, Some(graded.report.ended.into()), said);
            })
            .map_err(|why| {
                running().remove(&id);
                format!("The grade could not start ({why}).")
            })?;
        Ok(id)
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
    level: &'a Level,
    skill: &'a str,
    profile: &'a str,
    stop: &'a AtomicBool,
    /// What the chats that ran so far spent, and who is told each.
    spent: Spent,
    tell: &'a dyn Fn(u32),
    /// The harness the chats ran on, as the first that started said.
    harness: Option<String>,
}

impl Chats for InChats<'_> {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn run(&mut self, chat: &RunChat) -> ChatRan {
        let ran = self.run_one(chat);
        if matches!(ran.end, ChatEnd::Turn | ChatEnd::TimedOut) {
            self.spent.chats += 1;
            if let Some(tokens) = ran.tokens {
                *self.spent.tokens.get_or_insert(0) += tokens;
            }
            if let Some(cost) = ran.cost_usd {
                *self.spent.cost_usd.get_or_insert(0.0) += cost;
            }
            (self.tell)(self.spent.chats);
        }
        ran
    }
}

impl InChats<'_> {
    fn run_one(&mut self, chat: &RunChat) -> ChatRan {
        let not_started = |why: String| ChatRan {
            chat: None,
            started: Vec::new(),
            tokens: None,
            cost_usd: None,
            ms: 0,
            end: ChatEnd::NotStarted { why },
        };
        let root = self.held.root().to_path_buf();
        let skill = chat.arm.has_the_skill().then_some((self.level, self.skill));
        // The previous version is laid out by a later version of the runner; this one asks for
        // none.
        if chat.arm == Arm::Previous {
            return not_started("this purlis grades no previous version yet".to_owned());
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
                match heard.recv_timeout(LOOK_EVERY) {
                    Ok(Watched::Started(name)) => {
                        let this = name.rsplit(':').next() == Some(self.skill);
                        skills.push(name);
                        // A trigger test learns nothing more once the skill under test started.
                        if this {
                            break ChatEnd::Turn;
                        }
                    }
                    Ok(Watched::TurnEnded) => break ChatEnd::Turn,
                    Ok(Watched::Closed) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break ChatEnd::Turn;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            },
        };
        let id = self.held.chats().chat_at(session).and_then(|at| at.id);
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
