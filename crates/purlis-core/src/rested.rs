// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether a chat is rested, and if not, why** (SC-4c, #7632; spec #1391, *What rested reads*;
//! ADR 0076 as amended 2026-11-25).
//!
//! A chat is **rested** when the app may hibernate it by itself (CONTEXT.md): a governed, local
//! chat at a turn boundary, on a harness that resumes by id and reports its own work, with no
//! own work in flight, every task it dispatched ended and reported, no report of its own owed,
//! and nobody having looked at it for the setting's time.
//!
//! [`rested`] is one pure answer over plain [`Facts`]. It takes no dispatch ledger and keeps no
//! timer: the caller reads the ledger, the board and the looked-at clock (SC-4g, #7636) and hands
//! the answers in, so nothing here can ask a task for its report as a side effect. Its
//! [`NotRested`] is the one typed reason that both the automatic path and the row's explanation
//! read, and [`by_hand`] is what the operator's own Hibernate (SC-4a, #7630) refuses by, from the
//! same reasons.

use crate::dispatched;
use crate::ownwork::OwnWork;
use crate::state::run::{Reason, RunState};

/// What is read of one chat to say whether it is rested. Plain facts: the caller gathers them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// Its run's state, with the reason where it is `input-required`.
    pub run: RunState,
    /// Its harness picks its conversation back up by id, and it has one (ADR 0076 §5).
    pub resumes_by_id: bool,
    /// Its harness reports its own work, so that none in flight is something purlis can see
    /// ([`crate::harness::Harness::reports_its_own_work`]).
    pub reports_its_own_work: bool,
    /// What it has running of its own.
    pub own_work: OwnWorkSeen,
    /// Each task it dispatched that is still open.
    pub tasks: Vec<Task>,
    /// Where it is itself a dispatched task: whether its report is still owed.
    pub as_a_task: Option<AsATask>,
    /// A remote chat purlis only watches (ADR 0076 §7).
    pub observed: bool,
    /// A runner's chat, not one on this machine (ADR 0078).
    pub on_a_runner: bool,
    /// A sub-agent's run under a chat's, not a chat's own (ADR 0076 §6).
    pub child_run: bool,
    /// Someone has looked at it or typed in it within the setting's time: the looked-at clock's
    /// answer (SC-4g, #7636).
    pub looked_at: bool,
}

/// What a chat has running of its own (ADR 0068, amended 2026-11-08), as the board holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OwnWorkSeen {
    /// Nothing.
    #[default]
    Nothing,
    /// A wait on work of its own, lapsed or not: a lapse changes only what the board shows.
    InFlight(OwnWork),
    /// Its wait passed its limit with the work still out: it still runs, and counts.
    OutAfterItsWait,
}

impl OwnWorkSeen {
    fn is_out(self) -> bool {
        match self {
            Self::Nothing => false,
            Self::InFlight(work) => !work.is_nothing(),
            Self::OutAfterItsWait => true,
        }
    }
}

/// One task a chat dispatched, as its asking chat's ledger reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    /// Where it stands ([`dispatched::Ledger::state`]).
    pub state: dispatched::State,
    /// It is hibernated: its program ended at a turn boundary, its conversation kept.
    pub hibernated: bool,
}

impl Task {
    /// It has ended and reported, or ended and will never report: nothing is to come from it.
    /// A hibernated task that has reported counts as ended.
    fn is_over(self) -> bool {
        match self.state {
            dispatched::State::Reported(_) | dispatched::State::Ended => true,
            dispatched::State::ReportedStillRunning(_) => self.hibernated,
            dispatched::State::Running
            | dispatched::State::Idle
            | dispatched::State::WaitingOnItsOwnWork
            | dispatched::State::NeedsThePerson
            | dispatched::State::NeedsApproval
            | dispatched::State::AsksYou
            | dispatched::State::Cancelling => false,
        }
    }
}

/// A chat that is itself a dispatched task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsATask {
    /// Its report is owed (`HandedFrom.report == Owed::Due`): its parent waits on it, or will.
    /// The one condition both a report owed and a parent waiting are.
    pub owes_its_report: bool,
}

impl AsATask {
    /// It, as its own record says (`from`).
    pub fn of(from: &crate::reopen::HandedFrom) -> Self {
        Self {
            owes_its_report: from.report == crate::reopen::Owed::Due,
        }
    }
}

/// **Why a chat is not rested**, or why the operator's Hibernate refuses it: one value each
/// reader takes the window's words for (`docs/ui-copy.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotRested {
    /// Its program is not at a turn boundary: it is queued, starting or in a turn.
    Working,
    /// It is stopped on an ask: the ask would be lost with its program.
    Asking,
    /// It is paused.
    Paused,
    /// It is hibernated already.
    Hibernated,
    /// Its run has ended.
    Ended,
    /// Its harness cannot pick its conversation up again by itself.
    CannotResume,
    /// Its harness does not report its own work, so purlis cannot see what it left running.
    CannotSeeItsOwnWork,
    /// It has work running of its own.
    OwnWork,
    /// A task it dispatched is still running, or has not reported.
    TaskOpen,
    /// It is a dispatched task, and its parent waits on its report.
    ReportOwed,
    /// It is a dispatched task (the operator's Hibernate refuses one until a tell wakes it,
    /// SC-4j, #7639).
    IsATask,
    /// It is a remote chat purlis only watches.
    Observed,
    /// It runs on a runner.
    OnARunner,
    /// It is a sub-agent's run, not a chat's.
    ChildRun,
    /// Someone looked at it or typed in it within the setting's time.
    LookedAt,
}

/// **Whether a chat may be hibernated by itself**, and if not, the first reason why.
pub fn rested(facts: &Facts) -> Result<(), NotRested> {
    stands_apart(facts)?;
    at_a_turn_boundary(facts.run)?;
    if !facts.resumes_by_id {
        return Err(NotRested::CannotResume);
    }
    if facts.own_work.is_out() {
        return Err(NotRested::OwnWork);
    }
    if !facts.tasks.iter().all(|task| task.is_over()) {
        return Err(NotRested::TaskOpen);
    }
    if facts.as_a_task.is_some_and(|task| task.owes_its_report) {
        return Err(NotRested::ReportOwed);
    }
    // Last, so a reason the person can act on is said first: on such a harness the own-work
    // fact is never seen, so nothing above hides behind it.
    if !facts.reports_its_own_work {
        return Err(NotRested::CannotSeeItsOwnWork);
    }
    if facts.looked_at {
        return Err(NotRested::LookedAt);
    }
    Ok(())
}

/// **Whether the operator may hibernate a chat by hand** (SC-4a, #7630), and if not, why: the
/// reasons [`rested`] gives, but for two that only hold back the app (the looked-at clock, and a
/// harness whose own work purlis cannot see, on which a by-hand one is warned of instead, SC-4i,
/// #7638), and with two refusals it keeps until the act can wait on them: any open task of its
/// own (SC-4j, #7639) and being a task at all (SC-4j). Own work is refused, as [`rested`] does,
/// until the act asks first (SC-4d, #7633).
pub fn by_hand(facts: &Facts) -> Result<(), NotRested> {
    rested(&Facts {
        reports_its_own_work: true,
        looked_at: false,
        ..facts.clone()
    })?;
    if !facts.tasks.is_empty() {
        return Err(NotRested::TaskOpen);
    }
    if facts.as_a_task.is_some() {
        return Err(NotRested::IsATask);
    }
    Ok(())
}

/// The kinds of chat never hibernated: observed, a runner's, a child run.
fn stands_apart(facts: &Facts) -> Result<(), NotRested> {
    if facts.observed {
        return Err(NotRested::Observed);
    }
    if facts.child_run {
        return Err(NotRested::ChildRun);
    }
    if facts.on_a_runner {
        return Err(NotRested::OnARunner);
    }
    Ok(())
}

/// At a turn boundary: `input-required` with reason `ready` or `turn-ended`, never `asked`, as
/// the run table's own hibernation reads it ([`crate::state::run::Run::after_a_host_crash`]).
fn at_a_turn_boundary(run: RunState) -> Result<(), NotRested> {
    match run {
        RunState::InputRequired(Reason::Ready | Reason::TurnEnded) => Ok(()),
        RunState::InputRequired(Reason::Asked) => Err(NotRested::Asking),
        RunState::Paused => Err(NotRested::Paused),
        RunState::Hibernated => Err(NotRested::Hibernated),
        RunState::Completed | RunState::Failed | RunState::Stopped => Err(NotRested::Ended),
        RunState::Queued | RunState::Starting | RunState::Working => Err(NotRested::Working),
    }
}

#[cfg(test)]
mod tests;
