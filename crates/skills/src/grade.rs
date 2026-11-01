// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grading a skill** (SK-16, #2779; spec #2762 decisions 8, 21, 22 and 43; ADR 0101 §5 and
//! ADR 0105): the grading framework's neutral shapes, and purlis's own, the **built-in runner**.
//!
//! A grade compares **arms**: the skill's chats against chats without it ([`Arm::Without`]), or
//! against its previous version ([`Arm::Previous`]). Every fixture runs in every arm,
//! [`RUNS`] times each, as skill-creator does, and the scorecard is each arm's pass rate as a
//! mean and standard deviation over the runs, with the difference against the baseline.
//!
//! **The runner never runs a chat.** It asks its host for each one ([`Chats`]), as a Grader asks
//! purlis: one [`RunChat`] in, one [`ChatRan`] back, whose skill runs say what the chat started.
//! purlis-core's host lays out a throwaway piece for the arm and runs a real chat in it, under the
//! sandbox, on the harness the person chose; a test's host answers from a script. So the runner
//! knows no harness, no sandbox and no file layout, and a Grader extension gets exactly the
//! same four messages ([`GradeRequest`], [`RunChat`], [`ChatRan`], [`GradeReport`]).
//!
//! **A trigger test is scored by skill runs** (ADR 0101 §5), never by a first tool call: a chat
//! triggered when a run of the skill started in it, whichever route its harness took, so one
//! trigger test means the same on every harness. A test passes in an arm when the share of its
//! runs that started the skill is at least [`TRIGGER_THRESHOLD`] for a query that should trigger
//! it, and below it for one that should not.

mod fixtures;
pub mod outcomes;
pub mod routers;

pub use fixtures::{
    EVALS_DIR, FixtureFile, Fixtures, MOST_EXPECTATION_BYTES, MOST_EXPECTATIONS,
    MOST_OUTCOME_EVALS, MOST_PROMPT_BYTES, MOST_TRIGGER_TESTS, OutcomeEval, Read, Refused,
    TriggerTest, Wrong, fixtures_of, outcomes_path, triggers_path, triggers_template,
};
pub use outcomes::{Check, OutcomeRun, OutcomeScore};

/// How many times each fixture runs in each arm: skill-creator's and `claude plugin eval`'s
/// default.
pub const RUNS: u8 = 3;

/// The share of a trigger test's runs that has to start the skill for it to count as triggered.
pub const TRIGGER_THRESHOLD: f64 = 0.5;

/// How many chats in a row that did not start end a grade, in all or in one arm: the harness,
/// its profile or the arm's router is not working, and every chat after them would not start
/// either.
pub const NOT_STARTED_IN_A_ROW: u32 = 2;

/// The framework a grade's report says graded it, when it is purlis's own runner.
pub const BUILT_IN: &str = "purlis";

/// **One side of a grade's comparison.**
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Arm {
    /// The skill as it is now.
    With,
    /// No skill at all: the built-in skills alone.
    Without,
    /// The skill as its previous version had it.
    Previous,
    /// The library, routed by the router under grade (SK-15e, ADR 0102 §8): a router's grade.
    Router,
    /// The library, routed by the built-in router: a router's grade's baseline.
    BuiltInRouter,
}

impl Arm {
    /// Its word, as a grade's messages spell it.
    pub fn word(self) -> &'static str {
        match self {
            Self::With => "with",
            Self::Without => "without",
            Self::Previous => "previous",
            Self::Router => "router",
            Self::BuiltInRouter => "built-in-router",
        }
    }

    /// Whether this arm has the skill in it, as it is or as it was.
    pub fn has_the_skill(self) -> bool {
        !matches!(self, Self::Without)
    }
}

/// **The grade request**: what to grade. The skill by name, its trigger tests and outcome evals,
/// the arms to compare and how many runs each fixture has in each.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GradeRequest {
    pub skill: String,
    pub triggers: Vec<TriggerTest>,
    pub outcomes: Vec<OutcomeEval>,
    pub arms: Vec<Arm>,
    pub runs: u8,
}

impl GradeRequest {
    /// The arm the others are compared with: the previous version where the grade has one,
    /// else no skill.
    pub fn baseline(&self) -> Option<Arm> {
        [Arm::Previous, Arm::Without, Arm::BuiltInRouter]
            .into_iter()
            .find(|arm| self.arms.contains(arm))
    }

    /// The arm the baseline is compared with: the skill as it is, or, in a router's grade, the
    /// router under grade.
    pub fn graded(&self) -> Option<Arm> {
        [Arm::With, Arm::Router]
            .into_iter()
            .find(|arm| self.arms.contains(arm))
    }
}

/// **The run-chat request**: one chat for the host to run, in one arm, started on `prompt`, for
/// what `kind` says. A Grader's is read against this closed shape: a key it does not know refuses
/// it (#3236), and only a trigger test's chat is one it may ask for ([`Held::ask`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunChat {
    pub arm: Arm,
    pub prompt: String,
    /// What the chat is for: a trigger test's where it is not written.
    #[serde(default, skip_serializing_if = "ChatKind::is_trigger")]
    pub kind: ChatKind,
}

/// **What a run-chat request's chat is for** (SK-55, #3235).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "is", rename_all = "kebab-case")]
pub enum ChatKind {
    /// A trigger test's: the host ends it once the skill under test starts, or at its turn's end.
    #[default]
    Trigger,
    /// An outcome eval's: it runs its turn to the end, with `files` (paths in the skill's folder,
    /// as the eval names them) staged into its piece first, and the host keeps what it left for
    /// its judge.
    Outcome { files: Vec<String> },
    /// A judge's, of the outcome eval chat `of` (its id, as its [`ChatRan`] said it): its piece
    /// holds only what that chat left, as purlis kept it, and `task`, the task it grades
    /// ([`outcomes::judge_task`]). It changes nothing: the host holds it to reading.
    Judge { of: String, task: String },
}

impl ChatKind {
    /// Whether it is a trigger test's: the kind not written on the wire.
    pub fn is_trigger(&self) -> bool {
        matches!(self, Self::Trigger)
    }
}

/// How a chat the host ran came to its end.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "end", rename_all = "kebab-case")]
pub enum ChatEnd {
    /// Its first turn ended.
    Turn,
    /// Its turn ran past the time a grade's chat has, and the host ended it.
    TimedOut,
    /// It never started, and why, in a sentence.
    NotStarted { why: String },
    /// The person stopped the grade while it ran.
    Cancelled,
    /// It asked the person something (a permission, or a notice that it waits on them), and the
    /// host ended it there: a grade's chat waits on nobody. It is no run, never a "did not
    /// trigger" (the review of #3229).
    Asked,
    /// The grade reached its token cap while it ran, and the host ended it. It is no run: it
    /// was cut short before it could say what it would have started.
    OverCap,
    /// What the grade's chats cost passed its dollar limit while it ran, and the host ended it
    /// (#3469). It is no run, as at the cap.
    OverLimit,
    /// An outcome eval's or a judge's chat closed before its turn ended (a crash, say): no run,
    /// and nothing left to judge. It spent as any chat does, so one that does not say what stops
    /// a capped grade, as a chat that never started does not (SK-55).
    ClosedEarly { why: String },
    /// An outcome eval's or a judge's chat asked to use `what` (`the tool Bash`), and the host
    /// ended it there: a limit of a grade, said by name so it does not read as the skill's fault.
    AskedFor { what: String },
    /// An outcome eval's chat was not started, as purlis could not hold it to its limits: it spent
    /// nothing, and it fails that run with `why` (the operator's ruling of 2026-11-01).
    NotHeld { why: String },
    /// A judge reached past what purlis laid out for it, as its turn's end found: it fails that
    /// run of its eval, the operator's ruling of 2026-11-01.
    ReadPast,
}

/// **The run-chat result**: what one chat did.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatRan {
    /// The chat, by its id, where it started.
    pub chat: Option<String>,
    /// Each skill a run started for in it, by the name its harness started it by.
    pub started: Vec<String>,
    /// What it spent, where its harness says.
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    /// How long it ran, in milliseconds.
    pub ms: u64,
    #[serde(flatten)]
    pub end: ChatEnd,
    /// **What its turn ended saying**, where its harness handed its last message back (SK-55):
    /// what a judge's answer is read from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

impl ChatRan {
    /// Whether it ran to the end of its turn, or past its time: a chat that says what it started.
    fn ran(&self) -> bool {
        matches!(self.end, ChatEnd::Turn | ChatEnd::TimedOut)
    }

    /// Whether a run of `skill` started in it, by whatever name its harness gave it: a plugin's
    /// skill is started as `<plugin>:<skill>`.
    fn started(&self, skill: &str) -> bool {
        self.started
            .iter()
            .any(|one| crate::leaf_name(one) == skill)
    }
}

/// **The host a grade asks for its chats**: purlis's, or a test's.
pub trait Chats {
    /// Runs `chat` and answers what it did. `tokens_left` is what the grade may still spend
    /// before its cap: the host ends the chat once it has spent more ([`ChatEnd::OverCap`]).
    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan;
    /// Whether the person stopped the grade: no chat is asked for after.
    fn cancelled(&self) -> bool;
}

/// **What a grade will run, before it runs**: the cost shown first (decision 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// The chats it runs in all, each one turn: judges' chats among them.
    pub chats: u32,
    /// Of those, the chats of each arm.
    pub per_arm: u32,
    pub trigger_tests: u32,
    /// The outcome evals it runs (SK-55), each in each arm and run, and the judge chats that
    /// grade them: one for each of their chats, at most.
    pub outcome_evals: u32,
    pub judge_chats: u32,
}

/// **The plan of `request`**: one chat per trigger test, arm and run; and per outcome eval with
/// expectations, arm and run, two: its own and its judge's. An eval with no expectations has
/// nothing to judge, so it runs no chat.
pub fn plan(request: &GradeRequest) -> Plan {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let runs = u32::from(request.runs);
    let tests = count(request.triggers.len());
    let evals = count(judged_evals(request).count());
    let judges_per_arm = evals.saturating_mul(runs);
    let per_arm = tests
        .saturating_mul(runs)
        .saturating_add(judges_per_arm.saturating_mul(2));
    let arms = count(request.arms.len());
    Plan {
        chats: per_arm.saturating_mul(arms),
        per_arm,
        trigger_tests: tests,
        outcome_evals: evals,
        judge_chats: judges_per_arm.saturating_mul(arms),
    }
}

/// The outcome evals of `request` a grade runs: those with an expectation to judge.
fn judged_evals(request: &GradeRequest) -> impl Iterator<Item = &OutcomeEval> {
    request
        .outcomes
        .iter()
        .filter(|eval| !eval.expectations.is_empty())
}

/// A mean, its standard deviation, and the least and the most, of some numbers.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stat {
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
}

impl Stat {
    /// Of `values`, or `None` for none. The standard deviation is the sample's, as
    /// skill-creator's benchmark gives it, and 0 for one value.
    pub fn of(values: &[f64]) -> Option<Self> {
        let n = values.len();
        if n == 0 {
            return None;
        }
        let mean = values.iter().sum::<f64>() / n as f64;
        let stddev = if n < 2 {
            0.0
        } else {
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
        };
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Some(Self {
            mean,
            stddev,
            min,
            max,
        })
    }
}

/// **One trigger test in one arm**, over its runs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TriggerScore {
    pub query: String,
    pub should_trigger: bool,
    pub arm: Arm,
    /// The runs that ran: a chat that never started, or was stopped, is no run.
    pub runs: u32,
    /// Of those, the runs that started the skill.
    pub triggered: u32,
    /// `triggered / runs`, or `None` where nothing ran.
    pub rate: Option<f64>,
    /// Whether it passed: `None` where nothing ran.
    pub pass: Option<bool>,
    /// Each run's chat, where it started, in run order.
    pub chats: Vec<Option<String>>,
    /// Of its chats, the ones that asked the person something and were ended there (SK-59,
    /// #3239): no run, never a "did not trigger", and said in the grade rather than as a notice
    /// on a chat nobody can see. Absent where none did, and in a row written before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub asked: u32,
}

/// Whether `n` is nothing: an `asked` that is not written.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// **One arm over the whole grade.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmScore {
    pub arm: Arm,
    /// The share of trigger tests that passed in each run, over the runs.
    pub pass_rate: Option<Stat>,
    /// Each chat's tokens, over the chats whose harness said them.
    pub tokens: Option<Stat>,
    /// Each chat's seconds.
    pub seconds: Option<Stat>,
    /// The chats it ran, and the ones that did not run.
    pub chats: u32,
    pub not_run: u32,
    /// **Its outcome evals** (SK-55): the share of an eval's expectations that passed in each
    /// judged run, over every eval's runs (skill-creator's benchmark); and the chats they asked
    /// for, the judges' among them, that ran and that did not. Absent in a grade with none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_pass_rate: Option<Stat>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub outcome_chats: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub outcome_not_run: u32,
}

/// How a grade came to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GradeEnd {
    /// Every chat it planned was asked for.
    Whole,
    /// The person stopped it.
    Cancelled,
    /// Chats stopped starting ([`NOT_STARTED_IN_A_ROW`]), so it stopped asking for more.
    ChatsDidNotStart,
    /// It reached its token cap: the chat running then was ended, and no more were asked for
    /// (the operator's ruling of 2026-10-26).
    OverCap,
    /// It reached its dollar limit before its token cap: no more chats were asked for (the
    /// operator's ruling G12 of 2026-10-27, #3469). What its chats cost, where their harness
    /// said, and their tokens at the dearest price where it did not, are what the limit counts.
    OverLimit,
    /// A chat that ran did not say what it spent, so the grade could not be held to its cap
    /// and stopped asking for more.
    SpendNotReported,
    /// purlis stopped before the grade ended (a crash, a fault in it, or a quit), so it was never
    /// scored: recorded by the host at its next start, never by this runner (#3353).
    Interrupted,
    /// A Grader extension's question ended it (#3236): it answered out of its closed shape, past
    /// the plan or its limits, too late, or not at all. purlis scored what ran itself, and
    /// [`GradeReport::failed`] says why in a sentence.
    GraderFailed,
}

/// **The grade report**: the scores per trigger test and arm, each arm's summary, and the
/// difference of the arm with the skill against the baseline.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GradeReport {
    /// The framework that graded it: [`BUILT_IN`] for purlis's own runner.
    pub grader: String,
    pub triggers: Vec<TriggerScore>,
    pub arms: Vec<ArmScore>,
    /// The arm with the skill's mean pass rate less the baseline's, where both ran.
    pub delta: Option<f64>,
    pub ended: GradeEnd,
    /// Why the chats stopped starting, where they did: the last one's sentence.
    pub not_started: Option<String>,
    /// The cap it was held to, and the tokens its chats spent, where they said.
    pub cap: Option<u64>,
    pub spent_tokens: Option<u64>,
    /// The cost it was held to besides, and what its chats cost in all, where their harness
    /// said: a second bound, for what a harness spends past the tokens it reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_cap_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spent_usd: Option<f64>,
    /// What the dollar limit counted, where a chat said its tokens and not its cost: what the
    /// others cost, and its tokens at the dearest price (#3469). Absent where every chat said
    /// its cost, as the limit then counted [`Self::spent_usd`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counted_usd: Option<f64>,
    /// The price the dollar limit was worked out at ([`Limits::priced`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priced: Option<Priced>,
    /// **Each outcome eval in each arm** (SK-55), and the difference its pass rate made, as
    /// [`Self::delta`] is of the trigger tests'. Absent in a grade with none, and in a row
    /// written before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<OutcomeScore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_delta: Option<f64>,
    /// **What its outcome chats spent, judges' among them**, of [`Self::spent_tokens`] and
    /// [`Self::spent_usd`], where they said (SK-55): an eval's whole turn spends unlike a trigger
    /// test's, so an estimate shares each over its own chats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_usd: Option<f64>,
    /// Why a Grader's question ended the grade, where it did ([`GradeEnd::GraderFailed`]): a
    /// sentence. Absent in every other report, and in a row written before #3236.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

/// **What a grade may spend**: a cap in tokens, and a dollar limit besides where one is set. It
/// reaches its end at whichever it reaches first ([`GradeEnd::OverCap`], [`GradeEnd::OverLimit`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    pub tokens: u64,
    /// The dollar limit (the operator's ruling G12, #3469).
    pub cost_usd: Option<f64>,
    /// The price, in dollars a million tokens, [`Self::cost_usd`] was worked out at: kept in the
    /// report.
    pub priced: Option<Priced>,
    /// What a chat that says its tokens and not its cost is counted at against
    /// [`Self::cost_usd`], in dollars a million tokens: the dearest known price, so the limit
    /// never under-counts it. `None` counts only what chats said they cost.
    pub unpriced_usd_per_million: Option<f64>,
}

/// **The price a grade's dollar limit was worked out at**: in dollars a million tokens, the
/// dearest a token of any kind costs on its model, or the dearest any model costs where its
/// model's price is not known.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Priced {
    pub usd_per_million: f64,
    pub from: PricedFrom,
    /// The model its chats run on, where the harness says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Where a [`Priced`] price comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PricedFrom {
    /// The model its chats run on.
    Model,
    /// The dearest known price: the model is not known, or its price is not.
    Dearest,
}

impl GradeReport {
    /// The arm `arm`'s score, where it was graded.
    pub fn arm(&self, arm: Arm) -> Option<&ArmScore> {
        self.arms.iter().find(|one| one.arm == arm)
    }

    /// How many of its chats asked the person something and were ended there (SK-59, #3239).
    pub fn asked(&self) -> u32 {
        let evals = self
            .outcomes
            .iter()
            .flat_map(|eval| &eval.results)
            .filter(|run| run.asked)
            .count();
        self.triggers
            .iter()
            .fold(u32::try_from(evals).unwrap_or(u32::MAX), |all, score| {
                all.saturating_add(score.asked)
            })
    }

    /// How many trigger tests passed with the skill, of those that ran: the grade a skill's
    /// history shows for its version.
    pub fn passed_with(&self) -> (u32, u32) {
        self.passed_in(Arm::With)
    }

    /// How many trigger tests passed in `arm`, of those that ran in it.
    pub fn passed_in(&self, arm: Arm) -> (u32, u32) {
        let with: Vec<&TriggerScore> = self
            .triggers
            .iter()
            .filter(|score| score.arm == arm && score.pass.is_some())
            .collect();
        let passed = with.iter().filter(|score| score.pass == Some(true)).count();
        (
            u32::try_from(passed).unwrap_or(u32::MAX),
            u32::try_from(with.len()).unwrap_or(u32::MAX),
        )
    }
}

/// **Grades `request` with purlis's built-in runner**, asking `chats` for every chat: each run in
/// turn, every trigger test in it and then every outcome eval, every arm for each, so a grade the
/// person stops has scored every arm alike so far. An outcome eval's chat is followed by its
/// judge's, which grades what it left ([`outcomes`]) and is held like any chat. Held to `limits` where they are given ([`Held`]): each chat is told the
/// tokens left, the grade stops at the cap or at the dollar limit, whichever comes first, and a
/// chat that started and did not say what it spent stops it too, however it ended, as the cap
/// could not be held after it.
pub fn grade(request: &GradeRequest, chats: &mut dyn Chats, limits: Option<Limits>) -> GradeReport {
    let mut held = Held::new(request, chats, limits);
    'runs: for _run in 0..request.runs {
        for test in &request.triggers {
            for arm in &request.arms {
                let chat = RunChat {
                    arm: *arm,
                    prompt: test.query.clone(),
                    kind: ChatKind::Trigger,
                };
                // Asked in the plan's own order, so no chat is past it.
                if held.ask(&chat).is_err() || held.ended().is_some() {
                    break 'runs;
                }
            }
        }
        for e in 0..held.evals.len() {
            for a in 0..request.arms.len() {
                held.run_eval(e, a);
                if held.ended().is_some() {
                    break 'runs;
                }
            }
        }
    }
    held.scored(BUILT_IN, None)
}

/// **A grade held to its plan and to its limits** (ADR 0105 §1 and §4; #3236): every chat a
/// grade's framework asks for, whether purlis's own runner or a Grader extension, goes through
/// this, so both are counted alike and neither can make purlis run more than the plan showed or
/// spend more than the person accepted.
///
/// - **The plan**: a chat is of one of the grade's arms, on one of its trigger tests' queries, and
///   each test runs at most [`GradeRequest::runs`] times in each arm. A chat past that is refused
///   ([`Self::ask`]'s `Err`), and is never run. An outcome eval's chats are the built-in runner's
///   alone ([`Self::run_eval`]): a Grader never names a judge or what one reads.
/// - **The limits**: each chat is told the tokens left; the grade ends at its cap, at its dollar
///   limit, at the person's Stop, after [`NOT_STARTED_IN_A_ROW`] chats that did not start, or
///   after a chat that did not say what it spent where the grade is capped. Once it has ended no
///   chat is run: [`Asked::Ended`] says how it ended.
/// - **The score**: [`Self::scored`] scores what ran as the built-in runner does, and
///   [`Self::reported`] holds a Grader's own scores to what ran and puts purlis's own count of the
///   spend in its report.
pub struct Held<'a> {
    request: &'a GradeRequest,
    chats: &'a mut dyn Chats,
    cap: Option<u64>,
    cost_cap: Option<f64>,
    unpriced_at: Option<f64>,
    priced: Option<Priced>,
    /// ran[test][arm] = each run's result, in run order.
    ran: Vec<Vec<Vec<ChatRan>>>,
    /// The outcome evals it runs; judged[eval][arm] = each run, in run order; and each arm's
    /// outcome chats, judges' among them, asked for and of those run.
    evals: Vec<&'a OutcomeEval>,
    judged: Vec<Vec<Vec<OutcomeRun>>>,
    outcome_asked: Vec<(u32, u32)>,
    // The judges' alone (SK-55): the eval chats between them start, and a judge that does not
    // start leaves every eval chat before it spent for nothing.
    judges_not_started_in_a_row: u32,
    outcome_tokens: Option<u64>,
    outcome_usd: Option<f64>,
    ended: Option<GradeEnd>,
    // Counted in all, and per arm: arms take turns, so a chat of one arm that starts says
    // nothing of another arm whose chats do not (a router that never answers, SK-15e).
    not_started_in_a_row: u32,
    not_started_in_its_arm: Vec<u32>,
    not_started: Option<String>,
    spent: Option<u64>,
    spent_usd: Option<f64>,
    // What the limit counts: what chats cost, and the tokens of those that said no cost at the
    // dearest price; and whether any did.
    counted_usd: f64,
    counted_unpriced: bool,
}

/// What [`Held::ask`] answers for a chat within the plan.
#[derive(Debug, Clone, PartialEq)]
pub enum Asked {
    /// The chat ran, or tried to: what it did.
    Ran(ChatRan),
    /// The grade had ended, so no chat was run: how it ended.
    Ended(GradeEnd),
}

impl<'a> Held<'a> {
    /// `request`'s grade, its chats asked of `chats` and held to `limits` where they are given.
    pub fn new(
        request: &'a GradeRequest,
        chats: &'a mut dyn Chats,
        limits: Option<Limits>,
    ) -> Self {
        Self {
            ran: vec![vec![Vec::new(); request.arms.len()]; request.triggers.len()],
            evals: judged_evals(request).collect(),
            judged: vec![vec![Vec::new(); request.arms.len()]; judged_evals(request).count()],
            outcome_asked: vec![(0, 0); request.arms.len()],
            judges_not_started_in_a_row: 0,
            outcome_tokens: None,
            outcome_usd: None,
            not_started_in_its_arm: vec![0; request.arms.len()],
            request,
            chats,
            cap: limits.as_ref().map(|limits| limits.tokens),
            cost_cap: limits.as_ref().and_then(|limits| limits.cost_usd),
            unpriced_at: limits
                .as_ref()
                .and_then(|limits| limits.unpriced_usd_per_million),
            priced: limits.and_then(|limits| limits.priced),
            ended: None,
            not_started_in_a_row: 0,
            not_started: None,
            spent: None,
            spent_usd: None,
            counted_usd: 0.0,
            counted_unpriced: false,
        }
    }

    /// How the grade ended, once it has.
    pub fn ended(&self) -> Option<GradeEnd> {
        self.ended
    }

    /// Whether the person stopped the grade.
    pub fn cancelled(&self) -> bool {
        self.chats.cancelled()
    }

    /// **Ends the grade as stopped, where the person stopped it and nothing else ended it**: for
    /// a Grader's question, which a Stop ends between chats, with no further chat to find it.
    pub fn stop_if_cancelled(&mut self) {
        if self.ended.is_none() && self.chats.cancelled() {
            self.ended = Some(GradeEnd::Cancelled);
        }
    }

    /// Why the chats stopped starting, where they did: the last one's sentence.
    pub fn not_started(&self) -> Option<&str> {
        self.not_started.as_deref()
    }

    /// Every chat that ran, or tried to, in no particular order.
    pub fn ran(&self) -> impl Iterator<Item = &ChatRan> {
        self.ran.iter().flatten().flatten()
    }

    /// **The test and arm `chat` is a run of**, or why it is past the plan: an arm the grade does
    /// not compare, a prompt that is none of its trigger tests' queries, or a test that has run
    /// every time the plan has it run in that arm.
    fn slot(&self, chat: &RunChat) -> Result<(usize, usize), String> {
        let a = self
            .request
            .arms
            .iter()
            .position(|arm| *arm == chat.arm)
            .ok_or_else(|| {
                format!(
                    "it asked for a chat in an arm this grade does not compare ({})",
                    chat.arm.word()
                )
            })?;
        let mut of_the_query = self
            .request
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, test)| test.query == chat.prompt)
            .peekable();
        if of_the_query.peek().is_none() {
            return Err(
                "it asked for a chat on a prompt that is none of this grade's trigger tests"
                    .to_owned(),
            );
        }
        of_the_query
            .map(|(t, _)| t)
            .find(|t| self.ran[*t][a].len() < usize::from(self.request.runs))
            .map(|t| (t, a))
            .ok_or_else(|| {
                format!(
                    "it asked for more runs of a trigger test in the {} arm than the {} the plan \
                     runs",
                    chat.arm.word(),
                    self.request.runs
                )
            })
    }

    /// **Runs `chat`, held to the plan and the limits**: `Err` where it is past the plan, and is
    /// not run; [`Asked::Ended`] where the grade has ended, before or as it was asked; else what
    /// it did.
    pub fn ask(&mut self, chat: &RunChat) -> Result<Asked, String> {
        if !chat.kind.is_trigger() {
            return Err(
                "it asked for an outcome eval's or a judge's chat, which purlis's own runner \
                 alone asks for"
                    .to_owned(),
            );
        }
        let (t, a) = self.slot(chat)?;
        let asked = self.run(chat, a);
        if let Asked::Ran(result) = &asked {
            self.ran[t][a].push(result.clone());
        }
        Ok(asked)
    }

    /// **One run of the outcome eval at `e` in the arm at `a`**: its chat, then, where that left
    /// an outcome, its judge's, each counted and held as any chat is.
    fn run_eval(&mut self, e: usize, a: usize) {
        let eval = self.evals[e];
        let arm = self.request.arms[a];
        let chat = RunChat {
            arm,
            prompt: outcomes::outcome_prompt(eval),
            kind: ChatKind::Outcome {
                files: eval.files.clone(),
            },
        };
        let Asked::Ran(done) = self.run(&chat, a) else {
            return;
        };
        self.count_outcome(a, &done);
        let mut run = OutcomeRun {
            chat: done.chat.clone(),
            judge: None,
            passed: None,
            checks: Vec::new(),
            failed: None,
            asked: matches!(done.end, ChatEnd::Asked | ChatEnd::AskedFor { .. }),
        };
        // A chat that asked, a write its guard did not answer for among what asks, or one that
        // could not be held fails the run, and says why (the operator's ruling of 2026-11-01).
        match &done.end {
            ChatEnd::Asked => {
                run.passed = Some(0);
                run.failed = Some(asked_said("something"));
            }
            ChatEnd::AskedFor { what } => {
                run.passed = Some(0);
                run.failed = Some(asked_said(what));
            }
            ChatEnd::NotHeld { why } => {
                run.passed = Some(0);
                run.failed = Some(why.clone());
            }
            _ => {}
        }
        // Only a chat that ran its turn, or past its time, left an outcome to judge.
        let of = done.chat.clone().filter(|_| done.ran());
        if let (Some(of), None) = (of, self.ended) {
            let judge = RunChat {
                arm,
                prompt: outcomes::judge_prompt(),
                kind: ChatKind::Judge {
                    of,
                    task: outcomes::judge_task(eval),
                },
            };
            if let Asked::Ran(verdict) = self.run(&judge, a) {
                self.count_outcome(a, &verdict);
                verdict_of(&mut run, &verdict, &eval.expectations);
            }
        }
        self.judged[e][a].push(run);
    }

    /// Counts the outcome eval's or judge's chat `one` in the arm at `a`.
    fn count_outcome(&mut self, a: usize, one: &ChatRan) {
        let counted = &mut self.outcome_asked[a];
        counted.0 = counted.0.saturating_add(1);
        if one.ran() {
            counted.1 = counted.1.saturating_add(1);
        }
    }

    /// **Runs `chat`, of the arm at `a`, held to the limits**: [`Asked::Ended`] where the grade
    /// has ended, before or as it was asked; else what it did.
    fn run(&mut self, chat: &RunChat, a: usize) -> Asked {
        if let Some(ended) = self.ended {
            return Asked::Ended(ended);
        }
        if self.chats.cancelled() {
            return Asked::Ended(*self.ended.insert(GradeEnd::Cancelled));
        }
        let left = self
            .cap
            .map(|cap| cap.saturating_sub(self.spent.unwrap_or(0)));
        if left == Some(0) {
            return Asked::Ended(*self.ended.insert(GradeEnd::OverCap));
        }
        if self.over_the_limit() {
            return Asked::Ended(*self.ended.insert(GradeEnd::OverLimit));
        }
        let result = self.chats.run(chat, left);
        if let Some(tokens) = result.tokens {
            self.spent = Some(self.spent.unwrap_or(0).saturating_add(tokens));
            if !chat.kind.is_trigger() {
                self.outcome_tokens = Some(self.outcome_tokens.unwrap_or(0).saturating_add(tokens));
            }
        }
        if !chat.kind.is_trigger()
            && let Some(cost) = result
                .cost_usd
                .filter(|cost| cost.is_finite() && *cost >= 0.0)
        {
            self.outcome_usd = Some(self.outcome_usd.unwrap_or(0.0) + cost);
        }
        match result
            .cost_usd
            .filter(|cost| cost.is_finite() && *cost >= 0.0)
        {
            Some(cost) => {
                self.spent_usd = Some(self.spent_usd.unwrap_or(0.0) + cost);
                self.counted_usd += cost;
            }
            None => {
                if let Some((tokens, at)) = result.tokens.zip(self.unpriced_at) {
                    self.counted_usd += tokens as f64 * at / 1_000_000.0;
                    self.counted_unpriced = true;
                }
            }
        }
        // A chat that started and did not say what it spent stops a capped grade, however it
        // ended: an ask, a Stop or the cap itself spends as a turn does, and the cap cannot be
        // held after a chat whose spend is unknown.
        let unreported = self.cap.is_some()
            && !matches!(
                result.end,
                ChatEnd::NotStarted { .. } | ChatEnd::NotHeld { .. }
            )
            && result.tokens.is_none();
        if matches!(chat.kind, ChatKind::Judge { .. }) {
            self.judges_not_started_in_a_row = match result.end {
                ChatEnd::NotStarted { .. } | ChatEnd::ClosedEarly { .. } => {
                    self.judges_not_started_in_a_row + 1
                }
                _ => 0,
            };
        }
        let stop = match &result.end {
            ChatEnd::NotStarted { why }
            | ChatEnd::ClosedEarly { why }
            | ChatEnd::NotHeld { why } => {
                self.not_started_in_a_row += 1;
                self.not_started_in_its_arm[a] += 1;
                self.not_started = Some(why.clone());
                None
            }
            ChatEnd::Cancelled => Some(GradeEnd::Cancelled),
            ChatEnd::OverCap => Some(GradeEnd::OverCap),
            ChatEnd::OverLimit => Some(GradeEnd::OverLimit),
            ChatEnd::Turn
            | ChatEnd::TimedOut
            | ChatEnd::Asked
            | ChatEnd::AskedFor { .. }
            | ChatEnd::ReadPast => {
                self.not_started_in_a_row = 0;
                self.not_started_in_its_arm[a] = 0;
                None
            }
        };
        self.ended = if unreported {
            Some(GradeEnd::SpendNotReported)
        } else if stop.is_some() {
            stop
        } else if self.cap.is_some_and(|cap| self.spent.unwrap_or(0) >= cap) {
            Some(GradeEnd::OverCap)
        } else if self.over_the_limit() {
            Some(GradeEnd::OverLimit)
        } else if self
            .not_started_in_a_row
            .max(self.not_started_in_its_arm[a])
            .max(self.judges_not_started_in_a_row)
            >= NOT_STARTED_IN_A_ROW
        {
            Some(GradeEnd::ChatsDidNotStart)
        } else {
            None
        };
        Asked::Ran(result)
    }

    /// Whether what the chats cost, as the limit counts it, has reached the dollar limit.
    fn over_the_limit(&self) -> bool {
        self.cost_cap
            .is_some_and(|cost_cap| self.counted_usd >= cost_cap)
    }

    /// **What ran, scored as purlis's built-in runner scores it**, graded by `grader`, ended as
    /// the grade ended, else as `ended` says, else whole; `failed` is the sentence of a grade a
    /// Grader's question ended ([`GradeEnd::GraderFailed`]).
    pub fn scored(&self, grader: &str, failed: Option<String>) -> GradeReport {
        let request = self.request;
        let skill = request.skill.as_str();
        let ran = &self.ran;
        let mut triggers = Vec::new();
        for (t, test) in request.triggers.iter().enumerate() {
            for (a, arm) in request.arms.iter().enumerate() {
                let results = &ran[t][a];
                let runs = results.iter().filter(|one| one.ran()).count();
                let triggered = results
                    .iter()
                    .filter(|one| one.ran() && one.started(skill))
                    .count();
                let rate = (runs > 0).then(|| triggered as f64 / runs as f64);
                let asked = results
                    .iter()
                    .filter(|one| one.end == ChatEnd::Asked)
                    .count();
                triggers.push(TriggerScore {
                    query: test.query.clone(),
                    should_trigger: test.should_trigger,
                    arm: *arm,
                    runs: u32::try_from(runs).unwrap_or(u32::MAX),
                    triggered: u32::try_from(triggered).unwrap_or(u32::MAX),
                    rate,
                    pass: rate.map(|rate| (rate >= TRIGGER_THRESHOLD) == test.should_trigger),
                    chats: results.iter().map(|one| one.chat.clone()).collect(),
                    asked: u32::try_from(asked).unwrap_or(u32::MAX),
                });
            }
        }

        let mut outcome_scores = Vec::new();
        for (e, eval) in self.evals.iter().enumerate() {
            for (a, arm) in request.arms.iter().enumerate() {
                let results = self.judged[e][a].clone();
                let shares: Vec<f64> = results
                    .iter()
                    .filter_map(|run| run.share(eval.expectations.len()))
                    .collect();
                outcome_scores.push(OutcomeScore {
                    eval: eval.id,
                    arm: *arm,
                    expectations: eval.expectations.clone(),
                    runs: u32::try_from(shares.len()).unwrap_or(u32::MAX),
                    pass_rate: Stat::of(&shares),
                    results,
                });
            }
        }

        let arms: Vec<ArmScore> = request
            .arms
            .iter()
            .enumerate()
            .map(|(a, arm)| {
                // Every judged run of every eval in this arm, as skill-creator's benchmark pools
                // them.
                let outcome_shares: Vec<f64> = self
                    .evals
                    .iter()
                    .enumerate()
                    .flat_map(|(e, eval)| {
                        self.judged[e][a]
                            .iter()
                            .filter_map(|run| run.share(eval.expectations.len()))
                    })
                    .collect();
                let (outcome_all, outcome_ran) = self.outcome_asked[a];
                // Each run's share of the tests that passed in it, over the tests it ran.
                let per_run: Vec<f64> = (0..usize::from(request.runs))
                    .filter_map(|r| {
                        let in_run: Vec<bool> = request
                            .triggers
                            .iter()
                            .enumerate()
                            .filter_map(|(t, test)| {
                                let one = ran[t][a].get(r).filter(|one| one.ran())?;
                                Some(one.started(skill) == test.should_trigger)
                            })
                            .collect();
                        (!in_run.is_empty()).then(|| {
                            in_run.iter().filter(|passed| **passed).count() as f64
                                / in_run.len() as f64
                        })
                    })
                    .collect();
                let all: Vec<&ChatRan> = ran.iter().flat_map(|of_test| of_test[a].iter()).collect();
                let tokens: Vec<f64> = all
                    .iter()
                    .filter_map(|one| one.tokens)
                    .map(|tokens| tokens as f64)
                    .collect();
                let seconds: Vec<f64> = all
                    .iter()
                    .filter(|one| one.ran())
                    .map(|one| one.ms as f64 / 1000.0)
                    .collect();
                let chats = all.iter().filter(|one| one.ran()).count();
                ArmScore {
                    arm: *arm,
                    pass_rate: Stat::of(&per_run),
                    tokens: Stat::of(&tokens),
                    seconds: Stat::of(&seconds),
                    chats: u32::try_from(chats).unwrap_or(u32::MAX),
                    not_run: u32::try_from(all.len() - chats).unwrap_or(u32::MAX),
                    outcome_pass_rate: Stat::of(&outcome_shares),
                    outcome_chats: outcome_ran,
                    outcome_not_run: outcome_all - outcome_ran,
                }
            })
            .collect();

        let difference = |of: fn(&ArmScore) -> Option<Stat>| {
            let mean_of = |arm: Option<Arm>| {
                arm.and_then(|arm| arms.iter().find(|one| one.arm == arm))
                    .and_then(of)
                    .map(|stat| stat.mean)
            };
            mean_of(request.graded())
                .zip(mean_of(request.baseline()))
                .map(|(with, base)| with - base)
        };
        let delta = difference(|arm| arm.pass_rate);
        let outcome_delta = difference(|arm| arm.outcome_pass_rate);
        let mut report = self.closed(grader, triggers, arms, delta, failed);
        report.outcomes = outcome_scores;
        report.outcome_delta = outcome_delta;
        report.outcome_tokens = self.outcome_tokens;
        report.outcome_usd = self.outcome_usd;
        report
    }

    /// **The report of these scores, graded by `grader`**, with purlis's own count of what the
    /// grade spent and the limits it was held to, never a framework's; ended as the grade ended
    /// (whole where it did not), with why its chats did not start where that ended it, or as
    /// `failed` says, the sentence of a Grader's question that ended it, whatever else did.
    fn closed(
        &self,
        grader: &str,
        triggers: Vec<TriggerScore>,
        arms: Vec<ArmScore>,
        delta: Option<f64>,
        failed: Option<String>,
    ) -> GradeReport {
        let ended = match &failed {
            Some(_) => GradeEnd::GraderFailed,
            None => self.ended.unwrap_or(GradeEnd::Whole),
        };
        GradeReport {
            grader: grader.to_owned(),
            triggers,
            arms,
            delta,
            ended,
            not_started: (ended == GradeEnd::ChatsDidNotStart)
                .then(|| self.not_started.clone())
                .flatten(),
            cap: self.cap,
            spent_tokens: self.spent,
            cost_cap_usd: self.cost_cap,
            spent_usd: self.spent_usd,
            counted_usd: (self.cost_cap.is_some() && self.counted_unpriced)
                .then_some(self.counted_usd),
            priced: self.priced.clone(),
            outcomes: Vec::new(),
            outcome_delta: None,
            outcome_tokens: None,
            outcome_usd: None,
            failed,
        }
    }

    /// **A Grader's own scores, `reported`, held to what ran** (#3236): each test one of the
    /// grade's, in one of its arms, run no more than the plan runs it, each chat it names one
    /// this grade ran, and each arm once. Its report is then `grader`'s scores with purlis's own
    /// count of the spend and the limits, ended as purlis saw the grade end (whole where it saw
    /// no other end), or why it is not one purlis records.
    pub fn reported(&self, grader: &str, reported: Reported) -> Result<GradeReport, String> {
        let request = self.request;
        let triggers: Vec<TriggerScore> = reported.triggers.into_iter().map(Into::into).collect();
        let arms: Vec<ArmScore> = reported.arms.into_iter().map(Into::into).collect();
        let ran: Vec<&str> = self.ran().filter_map(|one| one.chat.as_deref()).collect();
        let mut named: Vec<&str> = Vec::new();
        for test in &triggers {
            if !request.arms.contains(&test.arm) {
                return Err(format!(
                    "its report scores an arm this grade does not compare ({})",
                    test.arm.word()
                ));
            }
            if !request
                .triggers
                .iter()
                .any(|one| one.query == test.query && one.should_trigger == test.should_trigger)
            {
                return Err("its report scores a trigger test this grade does not have".to_owned());
            }
            for chat in test.chats.iter().flatten() {
                if !ran.contains(&chat.as_str()) {
                    return Err("its report names a chat this grade did not run".to_owned());
                }
                if named.contains(&chat.as_str()) {
                    return Err("its report names one chat twice".to_owned());
                }
                named.push(chat);
            }
        }
        let mut scored: Vec<Arm> = Vec::new();
        for arm in &arms {
            if !request.arms.contains(&arm.arm) || scored.contains(&arm.arm) {
                return Err(format!(
                    "its report scores the {} arm twice, or one this grade does not compare",
                    arm.arm.word()
                ));
            }
            scored.push(arm.arm);
        }
        if triggers.len() > request.triggers.len() * request.arms.len() {
            return Err("its report scores more tests than this grade runs".to_owned());
        }
        // The chats each arm ran are facts purlis counted, and what the record adds its spend up
        // from: a report that says otherwise would be recorded and then skipped as it is read back.
        for (a, arm) in request.arms.iter().enumerate() {
            let chats = self
                .ran
                .iter()
                .flat_map(|test| test[a].iter())
                .filter(|one| one.ran())
                .count() as u64;
            let said = arms
                .iter()
                .find(|one| one.arm == *arm)
                .map_or(0, |one| u64::from(one.chats));
            if said != chats {
                return Err(format!(
                    "its report says the {} arm ran {said} chats, and purlis counted {chats}",
                    arm.word()
                ));
            }
        }
        Ok(self.closed(grader, triggers, arms, reported.delta, None))
    }
}

/// **A Grader's grade report**, as its last answer carries it (#3236): its scores, and nothing of
/// what the grade spent, how it ended or who graded it, which are purlis's to say. Read against
/// this closed shape, each part of it closed too: a key it does not know refuses it.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reported {
    triggers: Vec<ReportedTest>,
    arms: Vec<ReportedArm>,
    delta: Option<f64>,
}

/// A [`TriggerScore`], as a Grader says it.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportedTest {
    query: String,
    should_trigger: bool,
    arm: Arm,
    runs: u32,
    triggered: u32,
    rate: Option<f64>,
    pass: Option<bool>,
    chats: Vec<Option<String>>,
    #[serde(default)]
    asked: u32,
}

impl From<ReportedTest> for TriggerScore {
    fn from(test: ReportedTest) -> TriggerScore {
        let ReportedTest {
            query,
            should_trigger,
            arm,
            runs,
            triggered,
            rate,
            pass,
            chats,
            asked,
        } = test;
        TriggerScore {
            query,
            should_trigger,
            arm,
            runs,
            triggered,
            rate,
            pass,
            chats,
            asked,
        }
    }
}

/// An [`ArmScore`], as a Grader says it.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportedArm {
    arm: Arm,
    pass_rate: Option<ReportedStat>,
    tokens: Option<ReportedStat>,
    seconds: Option<ReportedStat>,
    chats: u32,
    not_run: u32,
}

impl From<ReportedArm> for ArmScore {
    fn from(arm: ReportedArm) -> ArmScore {
        ArmScore {
            arm: arm.arm,
            pass_rate: arm.pass_rate.map(Into::into),
            tokens: arm.tokens.map(Into::into),
            seconds: arm.seconds.map(Into::into),
            chats: arm.chats,
            not_run: arm.not_run,
            outcome_pass_rate: None,
            outcome_chats: 0,
            outcome_not_run: 0,
        }
    }
}

/// A [`Stat`], as a Grader says it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportedStat {
    mean: f64,
    stddev: f64,
    min: f64,
    max: f64,
}

impl From<ReportedStat> for Stat {
    fn from(stat: ReportedStat) -> Stat {
        Stat {
            mean: stat.mean,
            stddev: stat.stddev,
            min: stat.min,
            max: stat.max,
        }
    }
}

/// Why an outcome eval's chat that asked for `what` failed its run: a limit of a grade's chat, not
/// a fault of the skill.
fn asked_said(what: &str) -> String {
    format!(
        "Its chat asked for {what}, which a grade's chat may not do on its own, so purlis ended it. \
         That is a limit of grading, not a fault of the skill, and the run counts as failed."
    )
}

/// **What the judge's chat `verdict` made of `run`**, graded on `expectations`: its answer taken
/// whole ([`outcomes::judged`]), or the eval failed there with why; a judge that never ran (not
/// started, stopped, ended at a cap, or handed back no answer) judged nothing, so the run is no
/// run. A judge that asked the person something failed it.
fn verdict_of(run: &mut OutcomeRun, verdict: &ChatRan, expectations: &[String]) {
    run.judge = verdict.chat.clone();
    let failed = |run: &mut OutcomeRun, why: String| {
        run.passed = Some(0);
        run.failed = Some(why);
    };
    match &verdict.end {
        ChatEnd::Turn | ChatEnd::TimedOut => match verdict.answer.as_deref() {
            None => {}
            Some(answer) => match outcomes::judged(answer, expectations) {
                Ok(checks) => {
                    let passed = checks.iter().filter(|check| check.passed).count();
                    run.passed = Some(u32::try_from(passed).unwrap_or(u32::MAX));
                    run.checks = checks;
                }
                Err(why) => failed(run, format!("{why} So the eval failed.")),
            },
        },
        ChatEnd::Asked | ChatEnd::AskedFor { .. } => failed(
            run,
            "The judge asked the person something, which a judge may not, so the eval failed."
                .to_owned(),
        ),
        ChatEnd::ReadPast => failed(
            run,
            "The judge reached past what it was given to read, so the eval failed.".to_owned(),
        ),
        ChatEnd::NotStarted { .. }
        | ChatEnd::NotHeld { .. }
        | ChatEnd::ClosedEarly { .. }
        | ChatEnd::Cancelled
        | ChatEnd::OverCap
        | ChatEnd::OverLimit => {}
    }
}

#[cfg(test)]
mod tests;
