// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat waits on of its own when its turn ends** (#3878, superseding D-1644-1).
//!
//! A chat whose turn ends while background work of its own is in flight, work that wakes it
//! when it is done, is **waiting**: not at work, and not handing the next move to the person.
//! This is the neutral half (ADR 0073): what counts, and for how long. A harness's adapter reads
//! its own payload into the kinds of work and the wake-ups below ([`said`]); Claude Code's is
//! the one that does (`crate::harness::claude::ownwork`). Another harness says nothing of the
//! kind, and its turn's end stays the person's.
//!
//! **What counts** (the operator's ruling of 2026-10-29):
//!
//! - A helper, a workflow of helpers, a monitor: always, while in flight. Each ends by itself
//!   or wakes the chat, so the wait has no limit.
//! - A one-shot wake-up: always, until it is due, and [`A_WAKE_UP_IS_LATE_AFTER`] past that.
//!   One due already, or overdue, is due now.
//! - A background shell: for [`OWN_WORK_LIMIT`] after the turn ended. A dev server or a log
//!   tail may run for as long as the chat does, so past that the chat has stopped.
//! - A recurring wake-up: only where its next run is within [`OWN_WORK_LIMIT`], and until that
//!   run is late.
//!
//! **What the harness said is durations from the turn's end** ([`Said`]), never a time of day:
//! a chat on a runner may keep another clock and another time zone than the app. **The app
//! stamps them on its own clock** as it applies the turn's end ([`Said::at`], through
//! [`crate::state::Waits::now`]), and ends the wait at its limit whether or not the chat ever
//! wakes ([`crate::state::Board::own_work_lapsed`]).

use chrono::Duration;

/// **How long a background shell, or the wait for a recurring wake-up's next run, holds a
/// chat's turn's end from the person** (#3878): one hour, the operator's ruling. Past it with no
/// wake, the chat needs the person.
pub const OWN_WORK_LIMIT: Duration = Duration::minutes(60);

/// **How long past its time a wake-up may come before the wait for it is over** (#3878): the
/// harness fires a due wake-up at its next idle check, and a wait that lapsed on the minute
/// would raise the hand a moment before the turn the wake-up starts.
pub const A_WAKE_UP_IS_LATE_AFTER: Duration = Duration::minutes(5);

/// The kinds of background work a harness can say are in flight at its turn's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InFlight {
    /// Helpers (sub-agents) at work.
    pub helpers: u16,
    /// Workflows of helpers at work.
    pub workflows: u16,
    /// Monitors watching for something to wake it.
    pub monitors: u16,
    /// Commands left running in the background.
    pub shells: u16,
}

/// A wake-up of the chat's own the harness said is scheduled: how many seconds after the
/// turn's end it is next due (0 for one due now or overdue), and whether it runs again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeUp {
    pub due_in: i64,
    pub recurring: bool,
}

/// **What a harness said its chat waits on of its own, as its turn ended** (#3878): how many of
/// each kind of work are in flight, and in how many seconds from then it next wakes and the
/// wait lapses. Nothing at all where it waits on nothing ([`Said::is_nothing`]). Carried on the
/// turn's end ([`crate::state::Detail::own_work`]); the board stamps it ([`Said::at`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Said {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub helpers: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub workflows: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub monitors: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shells: u16,
    /// Seconds from the turn's end to its next wake-up of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wakes_in: Option<u32>,
    /// Seconds from the turn's end to when the wait lapses with no wake; none while a helper, a
    /// workflow or a monitor is in flight, which ends by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapses_in: Option<u32>,
}

fn is_zero(n: &u16) -> bool {
    *n == 0
}

impl Said {
    /// Whether it waits on nothing: its turn's end is the person's.
    pub fn is_nothing(&self) -> bool {
        self.helpers == 0
            && self.workflows == 0
            && self.monitors == 0
            && self.shells == 0
            && self.wakes_in.is_none()
    }

    /// The wait an older hook's line says, which counted helpers alone and not how many
    /// (#1626): one helper, with no limit, as it was read then.
    pub fn helpers_of_an_older_hook() -> Self {
        Self {
            helpers: 1,
            ..Self::default()
        }
    }

    /// **The wait, stamped at `now`** (seconds since 1970 on the app's clock), as the turn's end
    /// is applied; or none where it has a limit and no clock was said (`now` of 0), which hands
    /// the end to the person rather than hold it for ever.
    pub fn at(self, now: i64) -> Option<OwnWork> {
        if now <= 0 && self.lapses_in.is_some() {
            return None;
        }
        let stamp = |secs: u32| (now > 0).then(|| now.saturating_add(i64::from(secs)));
        Some(OwnWork {
            helpers: self.helpers,
            workflows: self.workflows,
            monitors: self.monitors,
            shells: self.shells,
            wakes_at: self.wakes_in.and_then(stamp),
            until: self.lapses_in.and_then(stamp),
        })
    }
}

/// **What a chat waits on of its own**, as the board holds it (#3878): how many of each kind of
/// work are in flight, when it next wakes on a schedule of its own, and when the wait lapses,
/// each in seconds since 1970 on the app's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct OwnWork {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub helpers: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub workflows: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub monitors: u16,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shells: u16,
    /// When its next wake-up of its own is due.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wakes_at: Option<i64>,
    /// When the wait lapses with no wake; none while it has no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

impl OwnWork {
    /// Whether the wait has lapsed at `now`, in seconds since 1970 ([`OwnWork::until`]).
    pub fn lapsed_at(&self, now: i64) -> bool {
        self.until.is_some_and(|until| until <= now)
    }

    /// Whether anything is left to wait on.
    pub fn is_nothing(&self) -> bool {
        self.helpers == 0
            && self.workflows == 0
            && self.monitors == 0
            && self.shells == 0
            && self.wakes_at.is_none()
    }
}

/// **What a chat waits on of its own, by the module's rules**: the work `in_flight` and the
/// wake-ups `wake_ups` a harness said it has at its turn's end. A recurring wake-up not due
/// within [`OWN_WORK_LIMIT`] counts as nothing.
pub fn said(in_flight: InFlight, wake_ups: &[WakeUp]) -> Said {
    let limit = OWN_WORK_LIMIT.num_seconds();
    let late = A_WAKE_UP_IS_LATE_AFTER.num_seconds();
    let secs = |n: i64| u32::try_from(n.max(0)).unwrap_or(u32::MAX);
    let mut ends: Vec<i64> = Vec::new();
    if in_flight.shells > 0 {
        ends.push(limit);
    }
    let mut wakes_in: Option<i64> = None;
    for wake_up in wake_ups {
        let due = wake_up.due_in.max(0);
        if wake_up.recurring && due > limit {
            continue;
        }
        wakes_in = Some(wakes_in.map_or(due, |soonest| soonest.min(due)));
        ends.push(due.saturating_add(late));
    }
    let open = in_flight.helpers > 0 || in_flight.workflows > 0 || in_flight.monitors > 0;
    Said {
        helpers: in_flight.helpers,
        workflows: in_flight.workflows,
        monitors: in_flight.monitors,
        shells: in_flight.shells,
        wakes_in: wakes_in.map(secs),
        lapses_in: if open {
            None
        } else {
            ends.into_iter().max().map(secs)
        },
    }
}

#[cfg(test)]
mod tests;
