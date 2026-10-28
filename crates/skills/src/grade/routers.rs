// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grading a router** (SK-15e, #3233; ADR 0102 §8): a library's trigger tests, each run with the
//! router under grade in place ([`Arm::Router`]) and with the built-in router
//! ([`Arm::BuiltInRouter`]), the baseline.
//!
//! A router's grade is the grades of every skill of the library that has trigger tests, each as
//! [`super::grade`] grades one skill, in the two arms. A test passes where the skill that should
//! trigger is started, by the harness's own pick or through `skill_load` (both start a skill run,
//! which is all a trigger test is scored by), and where one that should not is not. The skills are
//! graded one after another, held to one cap and one dollar limit over them all: a grade that
//! stops, at its cap, its limit or the person's Stop, keeps what its skills scored so far.

use super::{
    Arm, BUILT_IN, Chats, GradeEnd, GradeReport, GradeRequest, Limits, Plan, Priced,
    plan as plan_of,
};

/// The two arms of a router's grade: the router, then the built-in router.
pub const ARMS: [Arm; 2] = [Arm::Router, Arm::BuiltInRouter];

/// **What a router's grade asks**: the router, by its extension's id, and each skill of the
/// library with trigger tests, as one skill's grade asks them, in [`ARMS`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RouterGradeRequest {
    pub router: String,
    pub skills: Vec<GradeRequest>,
}

/// **The host a router's grade asks for its chats**: told which skill's tests run next, so each
/// chat can be watched for that skill.
pub trait LibraryChats: Chats {
    fn grading(&mut self, skill: &str);
}

/// **One skill's part of a router's grade.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillPart {
    pub skill: String,
    pub report: GradeReport,
}

/// **A router's grade report**: each skill's part, in the order they ran, and how the grade
/// ended over them all.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RouterGradeReport {
    /// The framework that graded it: [`BUILT_IN`] for purlis's own runner.
    pub grader: String,
    pub router: String,
    /// Each skill graded, as far as the grade got: a grade that stopped has no part for the
    /// skills it never reached.
    pub skills: Vec<SkillPart>,
    pub ended: GradeEnd,
    /// Why the chats stopped starting, where they did.
    pub not_started: Option<String>,
    pub cap: Option<u64>,
    pub spent_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_cap_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spent_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priced: Option<Priced>,
}

impl RouterGradeReport {
    /// How many trigger tests passed in `arm`, of those that ran in it, over every skill.
    pub fn passed_in(&self, arm: Arm) -> (u32, u32) {
        self.skills.iter().fold((0, 0), |(passed, of), part| {
            let (p, o) = part.report.passed_in(arm);
            (passed.saturating_add(p), of.saturating_add(o))
        })
    }

    /// How many of its chats asked the person something and were ended there.
    pub fn asked(&self) -> u32 {
        self.skills
            .iter()
            .fold(0u32, |all, part| all.saturating_add(part.report.asked()))
    }
}

/// **The plan of `request`**: every skill's chats, summed.
pub fn plan(request: &RouterGradeRequest) -> Plan {
    request.skills.iter().map(plan_of).fold(
        Plan {
            chats: 0,
            per_arm: 0,
            trigger_tests: 0,
            outcome_evals_waiting: 0,
        },
        |all, one| Plan {
            chats: all.chats.saturating_add(one.chats),
            per_arm: all.per_arm.saturating_add(one.per_arm),
            trigger_tests: all.trigger_tests.saturating_add(one.trigger_tests),
            outcome_evals_waiting: all
                .outcome_evals_waiting
                .saturating_add(one.outcome_evals_waiting),
        },
    )
}

/// **Grades the router of `request`**, each skill in turn, asking `chats` for every chat, held to
/// `limits` over them all: each skill is graded with what is left of the cap and the limit, and
/// the grade ends with the first skill whose grade did not end whole.
pub fn grade(
    request: &RouterGradeRequest,
    chats: &mut dyn LibraryChats,
    limits: Option<Limits>,
) -> RouterGradeReport {
    let mut parts = Vec::new();
    let mut ended = GradeEnd::Whole;
    let mut not_started = None;
    let mut spent: Option<u64> = None;
    let mut spent_usd: Option<f64> = None;
    let mut counted_usd = 0.0_f64;
    for skill in &request.skills {
        let left = limits.as_ref().map(|limits| Limits {
            tokens: limits.tokens.saturating_sub(spent.unwrap_or(0)),
            cost_usd: limits.cost_usd.map(|usd| (usd - counted_usd).max(0.0)),
            ..limits.clone()
        });
        if left.as_ref().is_some_and(|left| left.tokens == 0) {
            ended = GradeEnd::OverCap;
            break;
        }
        if left
            .as_ref()
            .and_then(|left| left.cost_usd)
            .is_some_and(|usd| usd <= 0.0)
        {
            ended = GradeEnd::OverLimit;
            break;
        }
        if chats.cancelled() {
            ended = GradeEnd::Cancelled;
            break;
        }
        chats.grading(&skill.skill);
        let report = super::grade(skill, chats, left);
        if let Some(tokens) = report.spent_tokens {
            spent = Some(spent.unwrap_or(0).saturating_add(tokens));
        }
        if let Some(usd) = report.spent_usd {
            spent_usd = Some(spent_usd.unwrap_or(0.0) + usd);
        }
        counted_usd += report.counted_usd.or(report.spent_usd).unwrap_or(0.0);
        let this_ended = report.ended;
        if this_ended == GradeEnd::ChatsDidNotStart {
            not_started.clone_from(&report.not_started);
        }
        parts.push(SkillPart {
            skill: skill.skill.clone(),
            report,
        });
        if this_ended != GradeEnd::Whole {
            ended = this_ended;
            break;
        }
    }
    RouterGradeReport {
        grader: BUILT_IN.to_owned(),
        router: request.router.clone(),
        skills: parts,
        ended,
        not_started,
        cap: limits.as_ref().map(|limits| limits.tokens),
        spent_tokens: spent,
        cost_cap_usd: limits.as_ref().and_then(|limits| limits.cost_usd),
        spent_usd,
        priced: limits.and_then(|limits| limits.priced),
    }
}

#[cfg(test)]
mod tests;
