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
    /// Whether this arm has the skill in it, as it is or as it was.
    pub fn has_the_skill(self) -> bool {
        !matches!(self, Self::Without)
    }
}

/// **The grade request**: what to grade. The skill by name, its trigger tests and outcome evals,
/// the arms to compare and how many runs each fixture has in each.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
/// what `kind` says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
/// judge's, which grades what it left ([`outcomes`]). Held to `limits` where they are given: each
/// chat is told the tokens left, the grade stops at the cap or at the dollar limit, whichever
/// comes first, and a chat that started and did not say what it spent stops it too, however it
/// ended, as the cap could not be held after it. A judge's chat is one of the grade's chats, held
/// to the same limits.
pub fn grade(request: &GradeRequest, chats: &mut dyn Chats, limits: Option<Limits>) -> GradeReport {
    let skill = request.skill.as_str();
    let mut asker = Asker::new(chats, limits.as_ref(), request.arms.len());
    // ran[test][arm] = each run's result, in run order.
    let mut ran: Vec<Vec<Vec<ChatRan>>> =
        vec![vec![Vec::new(); request.arms.len()]; request.triggers.len()];
    let evals: Vec<&OutcomeEval> = judged_evals(request).collect();
    // judged[eval][arm] = each run, in run order.
    let mut judged: Vec<Vec<Vec<OutcomeRun>>> =
        vec![vec![Vec::new(); request.arms.len()]; evals.len()];
    // asked[arm] = the outcome chats of that arm, judges' among them, and of those the ones that
    // ran.
    let mut outcome_asked = vec![(0u32, 0u32); request.arms.len()];
    let mut ended = GradeEnd::Whole;
    'runs: for _run in 0..request.runs {
        for (t, test) in request.triggers.iter().enumerate() {
            for (a, arm) in request.arms.iter().enumerate() {
                let chat = RunChat {
                    arm: *arm,
                    prompt: test.query.clone(),
                    kind: ChatKind::Trigger,
                };
                match asker.ask(&chat, a) {
                    Err(end) => {
                        ended = end;
                        break 'runs;
                    }
                    Ok((result, end)) => {
                        ran[t][a].push(result);
                        if let Some(end) = end {
                            ended = end;
                            break 'runs;
                        }
                    }
                }
            }
        }
        for (e, eval) in evals.iter().enumerate() {
            for (a, arm) in request.arms.iter().enumerate() {
                let chat = RunChat {
                    arm: *arm,
                    prompt: outcomes::outcome_prompt(eval),
                    kind: ChatKind::Outcome {
                        files: eval.files.clone(),
                    },
                };
                let (done, end) = match asker.ask(&chat, a) {
                    Err(end) => {
                        ended = end;
                        break 'runs;
                    }
                    Ok(asked) => asked,
                };
                count_outcome(&mut outcome_asked[a], &done);
                let mut run = OutcomeRun {
                    chat: done.chat.clone(),
                    judge: None,
                    passed: None,
                    checks: Vec::new(),
                    failed: None,
                    asked: done.end == ChatEnd::Asked,
                };
                if let Some(end) = end {
                    judged[e][a].push(run);
                    ended = end;
                    break 'runs;
                }
                // Only a chat that ran its turn, or past its time, left an outcome to judge.
                let Some(of) = done.chat.clone().filter(|_| done.ran()) else {
                    judged[e][a].push(run);
                    continue;
                };
                let judge = RunChat {
                    arm: *arm,
                    prompt: outcomes::judge_prompt(),
                    kind: ChatKind::Judge {
                        of,
                        task: outcomes::judge_task(eval),
                    },
                };
                match asker.ask(&judge, a) {
                    Err(end) => {
                        judged[e][a].push(run);
                        ended = end;
                        break 'runs;
                    }
                    Ok((verdict, end)) => {
                        count_outcome(&mut outcome_asked[a], &verdict);
                        verdict_of(&mut run, &verdict, &eval.expectations);
                        judged[e][a].push(run);
                        if let Some(end) = end {
                            ended = end;
                            break 'runs;
                        }
                    }
                }
            }
        }
    }
    let Asker {
        cap,
        cost_cap,
        spent,
        spent_usd,
        counted_usd,
        counted_unpriced,
        not_started,
        ..
    } = asker;

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
    for (e, eval) in evals.iter().enumerate() {
        for (a, arm) in request.arms.iter().enumerate() {
            let results = judged[e][a].clone();
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
                        in_run.iter().filter(|passed| **passed).count() as f64 / in_run.len() as f64
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
            // Every judged run of every eval in this arm, as skill-creator's benchmark pools them.
            let outcome_shares: Vec<f64> = evals
                .iter()
                .enumerate()
                .flat_map(|(e, eval)| {
                    judged[e][a]
                        .iter()
                        .filter_map(|run| run.share(eval.expectations.len()))
                })
                .collect();
            let (outcome_all, outcome_ran) = outcome_asked[a];
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

    GradeReport {
        grader: BUILT_IN.to_owned(),
        triggers,
        arms,
        delta,
        ended,
        not_started: (ended == GradeEnd::ChatsDidNotStart)
            .then_some(not_started)
            .flatten(),
        cap,
        spent_tokens: spent,
        cost_cap_usd: cost_cap,
        spent_usd,
        counted_usd: (cost_cap.is_some() && counted_unpriced).then_some(counted_usd),
        priced: limits.and_then(|limits| limits.priced),
        outcomes: outcome_scores,
        outcome_delta,
    }
}

/// Counts the outcome eval's or judge's chat `one` in `(asked, ran)`.
fn count_outcome(counted: &mut (u32, u32), one: &ChatRan) {
    counted.0 = counted.0.saturating_add(1);
    if one.ran() {
        counted.1 = counted.1.saturating_add(1);
    }
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
        ChatEnd::Asked => failed(
            run,
            "The judge asked the person something, which a judge may not, so the eval failed."
                .to_owned(),
        ),
        ChatEnd::NotStarted { .. } | ChatEnd::Cancelled | ChatEnd::OverCap | ChatEnd::OverLimit => {
        }
    }
}

/// **What asks a grade's host for its chats, one at a time, and keeps the grade to its limits.**
struct Asker<'a> {
    chats: &'a mut dyn Chats,
    cap: Option<u64>,
    cost_cap: Option<f64>,
    unpriced_at: Option<f64>,
    spent: Option<u64>,
    spent_usd: Option<f64>,
    // What the limit counts: what chats cost, and the tokens of those that said no cost at the
    // dearest price; and whether any did.
    counted_usd: f64,
    counted_unpriced: bool,
    // Counted in all, and per arm: arms take turns, so a chat of one arm that starts says
    // nothing of another arm whose chats do not (a router that never answers, SK-15e).
    not_started_in_a_row: u32,
    not_started_in_its_arm: Vec<u32>,
    // And the judges' alone (SK-55): the eval chats between them start, and a judge that does
    // not start leaves every eval chat before it spent for nothing.
    judges_not_started_in_a_row: u32,
    not_started: Option<String>,
}

impl<'a> Asker<'a> {
    fn new(chats: &'a mut dyn Chats, limits: Option<&Limits>, arms: usize) -> Self {
        Self {
            chats,
            cap: limits.map(|limits| limits.tokens),
            cost_cap: limits.and_then(|limits| limits.cost_usd),
            unpriced_at: limits.and_then(|limits| limits.unpriced_usd_per_million),
            spent: None,
            spent_usd: None,
            counted_usd: 0.0,
            counted_unpriced: false,
            not_started_in_a_row: 0,
            not_started_in_its_arm: vec![0; arms],
            judges_not_started_in_a_row: 0,
            not_started: None,
        }
    }

    /// **Asks for `chat`, of the arm at `a`**: `Err` with how the grade ends where it ends before
    /// the chat is asked for (Stop, the cap or the limit reached); else what it did, and how the
    /// grade ends after it, where it does.
    fn ask(&mut self, chat: &RunChat, a: usize) -> Result<(ChatRan, Option<GradeEnd>), GradeEnd> {
        if self.chats.cancelled() {
            return Err(GradeEnd::Cancelled);
        }
        let left = self
            .cap
            .map(|cap| cap.saturating_sub(self.spent.unwrap_or(0)));
        if left == Some(0) {
            return Err(GradeEnd::OverCap);
        }
        if self
            .cost_cap
            .is_some_and(|cost_cap| self.counted_usd >= cost_cap)
        {
            return Err(GradeEnd::OverLimit);
        }
        let result = self.chats.run(chat, left);
        if let Some(tokens) = result.tokens {
            self.spent = Some(self.spent.unwrap_or(0).saturating_add(tokens));
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
            && !matches!(result.end, ChatEnd::NotStarted { .. })
            && result.tokens.is_none();
        if matches!(chat.kind, ChatKind::Judge { .. }) {
            self.judges_not_started_in_a_row = match result.end {
                ChatEnd::NotStarted { .. } => self.judges_not_started_in_a_row + 1,
                _ => 0,
            };
        }
        let stop = match &result.end {
            ChatEnd::NotStarted { why } => {
                self.not_started_in_a_row += 1;
                self.not_started_in_its_arm[a] += 1;
                self.not_started = Some(why.clone());
                None
            }
            ChatEnd::Cancelled => Some(GradeEnd::Cancelled),
            ChatEnd::OverCap => Some(GradeEnd::OverCap),
            ChatEnd::OverLimit => Some(GradeEnd::OverLimit),
            ChatEnd::Turn | ChatEnd::TimedOut | ChatEnd::Asked => {
                self.not_started_in_a_row = 0;
                self.not_started_in_its_arm[a] = 0;
                None
            }
        };
        let end = if unreported {
            Some(GradeEnd::SpendNotReported)
        } else if stop.is_some() {
            stop
        } else if self.cap.is_some_and(|cap| self.spent.unwrap_or(0) >= cap) {
            Some(GradeEnd::OverCap)
        } else if self
            .cost_cap
            .is_some_and(|cost_cap| self.counted_usd >= cost_cap)
        {
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
        Ok((result, end))
    }
}

#[cfg(test)]
mod tests;
