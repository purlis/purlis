// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The production-timing set, derived rather than hand-tagged**, and which runs take a cell.
//!
//! A cell is production-timing when any `wait`, `within` or press `delay` names a member of a
//! `production_only` pair (ticket 1's `Timings`), when a call carries a harness or client
//! timeout, or when it restarts with `host_killed`. A hand `production_only` tag adds a cell;
//! nothing removes a derived one. `production_nightly_only` adds the cell too, for the nightly
//! production run only.
//!
//! Derived membership adds a production run and keeps the scaled one. Only a hand
//! `production_only` tag (a tie scaling cannot keep) and `restart: host_killed` skip scaled
//! runs. The `production-timings` label and the merge queue's production legs skip a
//! `production_nightly_only` cell and report it skipped-for-timing; only the nightly
//! production run executes it.

use std::collections::BTreeSet;

use crate::model::{Batch, Cell, Press, Restart, TimingsMode};
use crate::timings::{Expr, TimingsCatalogue};
use crate::validate::{restarts, withins};

/// **Why a cell is or is not production-timing.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimingProfile {
    /// What derives it, one line each; empty when nothing does.
    pub derived_by: Vec<String>,
    /// A hand `production_only` tag.
    pub hand: bool,
    pub host_killed: bool,
    pub nightly_only: Option<String>,
}

impl TimingProfile {
    pub fn in_production_set(&self) -> bool {
        !self.derived_by.is_empty() || self.hand || self.nightly_only.is_some()
    }

    pub fn skips_scaled(&self) -> bool {
        self.hand
    }
}

/// What started a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    PullRequest,
    /// The `production-timings` label on a pull request.
    Label,
    MergeQueue,
    Nightly,
}

/// What a run does with a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheduled {
    Run,
    /// Reported as skipped-for-timing, never as passed.
    SkippedForTiming,
    /// Not this run's: a cell outside the production-timing set in a production run.
    NotInThisRun,
}

/// **`cell`'s timing profile** against `cat`'s `production_only` pairs.
pub fn profile(cell: &Cell, cat: &dyn TimingsCatalogue) -> TimingProfile {
    let members = cat.production_only_members();
    let mut derived_by = Vec::new();
    let mut names = |what: &str, text: &str| {
        if let Ok(expr) = Expr::parse(text) {
            for name in expr.names() {
                if members.contains(name) {
                    derived_by.push(format!("{what} '{text}' names {name}"));
                }
            }
        }
    };
    for step in &cell.steps {
        if let Some(wait) = &step.wait {
            names("wait", wait);
        }
        if let Some(Press::Button(button)) = &step.press
            && let Some(delay) = &button.delay
        {
            names("press delay", delay);
        }
    }
    for within in withins(cell) {
        names("within", within);
    }
    for step in &cell.steps {
        for call in &step.calls {
            if let Some(timeout) = &call.timeout {
                derived_by.push(format!(
                    "a {} call carries a timeout of {timeout}",
                    call.tool
                ));
            }
        }
    }
    let host_killed = restarts(cell, Restart::HostKilled);
    if host_killed {
        derived_by.push("restart: host_killed".to_owned());
    }
    TimingProfile {
        derived_by,
        hand: cell.production_only.is_some(),
        host_killed,
        nightly_only: cell.production_nightly_only.clone(),
    }
}

/// **What a run at `timings`, started by `trigger`, does with `cell`.**
pub fn schedule(
    cell: &Cell,
    cat: &dyn TimingsCatalogue,
    timings: TimingsMode,
    trigger: Trigger,
) -> Scheduled {
    let p = profile(cell, cat);
    match timings {
        TimingsMode::Scaled if p.skips_scaled() => Scheduled::SkippedForTiming,
        TimingsMode::Scaled => Scheduled::Run,
        TimingsMode::Production if !p.in_production_set() => Scheduled::NotInThisRun,
        TimingsMode::Production if p.nightly_only.is_some() && trigger != Trigger::Nightly => {
            Scheduled::SkippedForTiming
        }
        TimingsMode::Production => Scheduled::Run,
    }
}

/// **The derived production-timing set**: every cell id in `batches` in it.
pub fn production_set<'a>(batches: &'a [Batch], cat: &dyn TimingsCatalogue) -> BTreeSet<&'a str> {
    batches
        .iter()
        .flat_map(|b| &b.cells)
        .filter(|c| profile(c, cat).in_production_set())
        .map(|c| c.id.as_str())
        .collect()
}
