// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A cell's verdict under its `expected_fail` marks** (spec principle 3: known divergence is
//! pinned, not waved through).
//!
//! The runner (ticket 7's assertions, or the live checker) says how each assertion came out
//! and which global checks failed; this decides what that means:
//!
//! - a failure no mark pins (same step, same assertion, same observed value; for a `within`
//!   assertion, a value inside the mark's range) is red;
//! - a mark that applies to this run and pinned nothing (its assertion passed, or failed
//!   differently) is red: a pass under `expected_fail` is red until the mark is removed;
//! - otherwise the cell is `expected_fail` when a mark pinned something, and a pass when not.
//!
//! A mark applies only to the leg, timings, dialect and OS it names, so the others are judged
//! as if it were not there. Every assertion and every global check is judged either way.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::model::{Cell, Dialect, Leg, Mark, Os, TimingsMode};
use crate::timings::Expr;
use crate::validate::applies_to;

/// **The run a verdict is for**, with each Timings name's value in seconds at its timings.
#[derive(Debug, Clone)]
pub struct Run {
    pub dialect: Dialect,
    pub os: Os,
    pub leg: Leg,
    pub timings: TimingsMode,
    pub seconds: BTreeMap<&'static str, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Observed {
    Pass,
    /// What was seen: a count, an outcome, or for a `within` assertion seconds.
    Fail(Value),
}

/// How one assertion of one step came out.
#[derive(Debug, Clone, PartialEq)]
pub struct Assertion {
    pub step: u32,
    pub assertion: String,
    pub observed: Observed,
}

/// A global check that failed, and the refusal it failed on.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalFailure {
    pub check: String,
    pub refusal: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Pass,
    ExpectedFail { issues: BTreeSet<u64> },
    Fail { reasons: Vec<String> },
}

/// **The verdict on `cell` for `run`.**
pub fn judge(
    cell: &Cell,
    run: &Run,
    assertions: &[Assertion],
    globals: &[GlobalFailure],
) -> Verdict {
    let marks: Vec<&Mark> = cell
        .expected_fail
        .iter()
        .filter(|m| applies(m, run))
        .collect();
    let mut used = vec![false; marks.len()];
    let mut reasons = Vec::new();
    let mut pin = |hits: Vec<usize>, what: String| {
        if hits.is_empty() {
            reasons.push(format!(
                "{}: {what}, and no expected_fail mark pins it",
                cell.id
            ));
        }
        for i in hits {
            used[i] = true;
        }
    };
    for a in assertions {
        let Observed::Fail(seen) = &a.observed else {
            continue;
        };
        let hits = (0..marks.len())
            .filter(|&i| {
                let m = marks[i];
                m.step == Some(a.step)
                    && m.assertion.as_deref() == Some(a.assertion.as_str())
                    && m.observed
                        .as_ref()
                        .is_some_and(|want| observed_matches(want, seen, run))
            })
            .collect();
        pin(
            hits,
            format!("step {} {} failed, observed {seen}", a.step, a.assertion),
        );
    }
    for g in globals {
        let hits = (0..marks.len())
            .filter(|&i| {
                marks[i].check.as_deref() == Some(g.check.as_str())
                    && marks[i].refusal.as_deref() == Some(g.refusal.as_str())
            })
            .collect();
        pin(
            hits,
            format!("global check {} failed on {}", g.check, g.refusal),
        );
    }
    for (m, used) in marks.iter().zip(&used) {
        if !used && !m.may_pass {
            let what = match (&m.check, m.step, &m.assertion) {
                (Some(check), _, _) => format!(
                    "global check {check} on {}",
                    m.refusal.as_deref().unwrap_or("?")
                ),
                (None, Some(step), Some(a)) => format!("step {step} {a}"),
                _ => "an assertion".to_owned(),
            };
            reasons.push(format!(
                "{}: #{} is marked expected_fail on {what}, which did not fail as pinned: remove the mark, or pin what it does now",
                cell.id, m.issue
            ));
        }
    }
    if !reasons.is_empty() {
        return Verdict::Fail { reasons };
    }
    let issues: BTreeSet<u64> = marks
        .iter()
        .zip(&used)
        .filter(|(_, u)| **u)
        .map(|(m, _)| m.issue)
        .collect();
    if issues.is_empty() {
        Verdict::Pass
    } else {
        Verdict::ExpectedFail { issues }
    }
}

fn applies(mark: &Mark, run: &Run) -> bool {
    applies_to(mark, run.dialect, run.os)
        && mark.leg.is_none_or(|l| l == run.leg)
        && mark.timings.is_none_or(|t| t == run.timings)
}

/// Whether `seen` is what `want` pins: inside it for a range in Timings units, one of its values
/// for a set, else equal.
fn observed_matches(want: &Value, seen: &Value, run: &Run) -> bool {
    if let Some(set) = want.as_array() {
        return set.iter().any(|one| observed_matches(one, seen, run));
    }
    if let Some(range) = want.as_object() {
        let end = |key: &str| {
            range
                .get(key)
                .and_then(Value::as_str)
                .and_then(|t| Expr::parse(t).ok())
                .and_then(|e| e.seconds(&run.seconds))
        };
        return match (end("from"), end("to"), seen.as_f64()) {
            (Some(lo), Some(hi), Some(x)) => lo <= x && x <= hi,
            _ => false,
        };
    }
    match (want.as_f64(), seen.as_f64()) {
        (Some(a), Some(b)) => a == b,
        _ => want == seen,
    }
}
