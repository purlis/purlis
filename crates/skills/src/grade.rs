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

pub use fixtures::{
    EVALS_DIR, Fixtures, MOST_EVAL_FILES, MOST_EXPECTATION_BYTES, MOST_EXPECTATIONS,
    MOST_FIXTURE_BYTES, MOST_OUTCOME_EVALS, MOST_PROMPT_BYTES, MOST_TRIGGER_TESTS, OUTCOME_FILE,
    OutcomeEval, Read, TRIGGER_FILE, TriggerTest, fixtures_of, outcomes_of, triggers_of,
};

/// How many times each fixture runs in each arm: skill-creator's and `claude plugin eval`'s
/// default.
pub const RUNS: u8 = 3;

/// The share of a trigger test's runs that has to start the skill for it to count as triggered.
pub const TRIGGER_THRESHOLD: f64 = 0.5;

/// How many chats in a row that did not start end a grade: the harness or its profile is not
/// working, and every chat after them would not start either.
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
}

impl Arm {
    /// The word a record and a message carry: `with`, `without` or `previous`.
    pub fn word(self) -> &'static str {
        match self {
            Self::With => "with",
            Self::Without => "without",
            Self::Previous => "previous",
        }
    }

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
        [Arm::Previous, Arm::Without]
            .into_iter()
            .find(|arm| self.arms.contains(arm))
    }
}

/// **The run-chat request**: one chat for the host to run, in one arm, started on `prompt`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunChat {
    pub arm: Arm,
    pub prompt: String,
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
            .any(|one| one.rsplit(':').next() == Some(skill))
    }
}

/// **The host a grade asks for its chats**: purlis's, or a test's.
pub trait Chats {
    /// Runs `chat` and answers what it did.
    fn run(&mut self, chat: &RunChat) -> ChatRan;
    /// Whether the person stopped the grade: no chat is asked for after.
    fn cancelled(&self) -> bool;
}

/// **What a grade will run, before it runs**: the cost shown first (decision 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// The chats it runs in all, each one turn.
    pub chats: u32,
    /// Of those, the chats of each arm.
    pub per_arm: u32,
    pub trigger_tests: u32,
    /// Outcome evals the skill has that this runner does not grade yet: shown, never run.
    pub outcome_evals_waiting: u32,
}

/// **The plan of `request`**: one chat per trigger test, arm and run.
pub fn plan(request: &GradeRequest) -> Plan {
    let tests = u32::try_from(request.triggers.len()).unwrap_or(u32::MAX);
    let per_arm = tests.saturating_mul(u32::from(request.runs));
    Plan {
        chats: per_arm.saturating_mul(u32::try_from(request.arms.len()).unwrap_or(u32::MAX)),
        per_arm,
        trigger_tests: tests,
        outcome_evals_waiting: u32::try_from(request.outcomes.len()).unwrap_or(u32::MAX),
    }
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
}

impl GradeReport {
    /// The arm `arm`'s score, where it was graded.
    pub fn arm(&self, arm: Arm) -> Option<&ArmScore> {
        self.arms.iter().find(|one| one.arm == arm)
    }

    /// How many trigger tests passed with the skill, of those that ran: the grade a skill's
    /// history shows for its version.
    pub fn passed_with(&self) -> (u32, u32) {
        let with: Vec<&TriggerScore> = self
            .triggers
            .iter()
            .filter(|score| score.arm == Arm::With && score.pass.is_some())
            .collect();
        let passed = with.iter().filter(|score| score.pass == Some(true)).count();
        (
            u32::try_from(passed).unwrap_or(u32::MAX),
            u32::try_from(with.len()).unwrap_or(u32::MAX),
        )
    }
}

/// **Grades `request` with purlis's built-in runner**, asking `chats` for every chat: each run in
/// turn, every trigger test in it, every arm for each, so a grade the person stops has scored
/// every arm alike so far. Outcome evals are not graded by this runner yet: [`Plan`] says how
/// many wait.
pub fn grade(request: &GradeRequest, chats: &mut dyn Chats) -> GradeReport {
    let skill = request.skill.as_str();
    // ran[test][arm] = each run's result, in run order.
    let mut ran: Vec<Vec<Vec<ChatRan>>> =
        vec![vec![Vec::new(); request.arms.len()]; request.triggers.len()];
    let mut ended = GradeEnd::Whole;
    let mut not_started_in_a_row = 0u32;
    let mut not_started: Option<String> = None;
    'runs: for _run in 0..request.runs {
        for (t, test) in request.triggers.iter().enumerate() {
            for (a, arm) in request.arms.iter().enumerate() {
                if chats.cancelled() {
                    ended = GradeEnd::Cancelled;
                    break 'runs;
                }
                let result = chats.run(&RunChat {
                    arm: *arm,
                    prompt: test.query.clone(),
                });
                match &result.end {
                    ChatEnd::NotStarted { why } => {
                        not_started_in_a_row += 1;
                        not_started = Some(why.clone());
                    }
                    ChatEnd::Cancelled => {
                        ran[t][a].push(result);
                        ended = GradeEnd::Cancelled;
                        break 'runs;
                    }
                    ChatEnd::Turn | ChatEnd::TimedOut => not_started_in_a_row = 0,
                }
                ran[t][a].push(result);
                if not_started_in_a_row >= NOT_STARTED_IN_A_ROW {
                    ended = GradeEnd::ChatsDidNotStart;
                    break 'runs;
                }
            }
        }
    }

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
            triggers.push(TriggerScore {
                query: test.query.clone(),
                should_trigger: test.should_trigger,
                arm: *arm,
                runs: u32::try_from(runs).unwrap_or(u32::MAX),
                triggered: u32::try_from(triggered).unwrap_or(u32::MAX),
                rate,
                pass: rate.map(|rate| (rate >= TRIGGER_THRESHOLD) == test.should_trigger),
                chats: results.iter().map(|one| one.chat.clone()).collect(),
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
            ArmScore {
                arm: *arm,
                pass_rate: Stat::of(&per_run),
                tokens: Stat::of(&tokens),
                seconds: Stat::of(&seconds),
                chats: u32::try_from(chats).unwrap_or(u32::MAX),
                not_run: u32::try_from(all.len() - chats).unwrap_or(u32::MAX),
            }
        })
        .collect();

    let mean_of = |arm: Option<Arm>| {
        arm.and_then(|arm| arms.iter().find(|one| one.arm == arm))
            .and_then(|one| one.pass_rate)
            .map(|stat| stat.mean)
    };
    let delta = mean_of(Some(Arm::With))
        .zip(mean_of(request.baseline()))
        .map(|(with, base)| with - base);

    GradeReport {
        grader: BUILT_IN.to_owned(),
        triggers,
        arms,
        delta,
        ended,
        not_started: (ended == GradeEnd::ChatsDidNotStart)
            .then_some(not_started)
            .flatten(),
    }
}

#[cfg(test)]
mod tests;
