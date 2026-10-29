// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat waits on of its own when its turn ends** (#3878, superseding D-1644-1).
//!
//! A chat whose turn ends while background work of its own is in flight, work that wakes it
//! when it is done, is **waiting**: not at work, and not handing the next move to the person.
//! This reads that work from what a harness said at the end of the turn, into [`OwnWork`], the
//! neutral shape the state model holds ([`crate::harness::model::Turn::WaitsOnItsOwnWork`]).
//!
//! **Claude Code is the one harness read** ([`read`]): its `Stop` input carries
//! `background_tasks`, each with a `type` and a `status`, and `session_crons`, each with a cron
//! `schedule` and whether it is `recurring` (read from the hook input schema of 2.1.312).
//! Another harness says nothing of the kind, and its turn's end stays the person's.
//!
//! **What counts** (the operator's ruling of 2026-10-29):
//!
//! - A helper (`subagent`), a `workflow`, a `monitor`: always, while in flight. Each ends by
//!   itself or wakes the chat, so the wait has no limit.
//! - A one-shot wake-up: always, until it is due, and [`A_WAKE_UP_IS_LATE_AFTER`] past that.
//! - A background `shell`: for [`OWN_WORK_LIMIT`] after the turn ended. A dev server or a log
//!   tail may run for as long as the chat does, so past that the chat has stopped.
//! - A recurring cron: only where its next run is within [`OWN_WORK_LIMIT`], and until that run
//!   is late.
//!
//! What is not read as one of these counts as nothing, so a payload this cannot read hands the
//! end of the turn to the person, as before. **The limit runs on the app's clock**: the time a
//! wait lapses ([`OwnWork::until`]) is set here, and the app ends the wait at that time whether
//! or not the chat ever wakes ([`crate::state::Board::own_work_lapsed`]).

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDateTime, Timelike};

/// **How long a background shell, or the wait for a recurring cron's next run, holds a chat's
/// turn's end from the person** (#3878): one hour, the operator's ruling. Past it with no wake,
/// the chat needs the person.
pub const OWN_WORK_LIMIT: Duration = Duration::minutes(60);

/// **How long past its time a wake-up may come before the wait for it is over** (#3878): the
/// harness fires a due cron at its next idle check, and a wait that lapsed on the minute would
/// raise the hand a moment before the turn the wake-up starts.
pub const A_WAKE_UP_IS_LATE_AFTER: Duration = Duration::minutes(5);

/// The kinds of background work, by Claude Code's label, that end by themselves and wake the
/// chat when they do: an agent, and a workflow of agents.
const HELPER: &str = "subagent";
const WORKFLOW: &str = "workflow";
/// A watch that wakes the chat when what it watches says something.
const MONITOR: &str = "monitor";
/// A command left running in the background.
const SHELL: &str = "shell";

/// The statuses of work still in flight: running, or pending. Read from the code that builds
/// the list in Claude Code 2.1.296, which keeps only an entry with one of these two.
const IN_FLIGHT: [&str; 2] = ["running", "pending"];

/// How far ahead a one-shot wake-up is looked for: a year and a day, the longest a five-field
/// cron that names one minute of one day can lie ahead.
const ONE_SHOT_LOOKS_AHEAD: Duration = Duration::days(367);

/// **What a chat waits on of its own**, as its turn ended (#3878): how many of each kind of
/// background work are in flight, when it next wakes on a schedule of its own, and when the
/// wait lapses. Nothing at all where it waits on nothing ([`OwnWork::is_nothing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct OwnWork {
    /// Helpers (sub-agents) at work in the background.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub helpers: u16,
    /// Workflows of helpers at work.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub workflows: u16,
    /// Monitors watching for something to wake it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub monitors: u16,
    /// Commands left running in the background, within [`OWN_WORK_LIMIT`] of the turn's end.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shells: u16,
    /// When its next wake-up of its own is due, in seconds since 1970: the soonest one-shot
    /// wake-up, or recurring run within [`OWN_WORK_LIMIT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wakes_at: Option<i64>,
    /// When the wait lapses with no wake, in seconds since 1970; none while a helper, a
    /// workflow or a monitor is in flight, which ends by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

fn is_zero(n: &u16) -> bool {
    *n == 0
}

impl OwnWork {
    /// Whether it waits on nothing: its turn's end is the person's.
    pub fn is_nothing(&self) -> bool {
        self.helpers == 0
            && self.workflows == 0
            && self.monitors == 0
            && self.shells == 0
            && self.wakes_at.is_none()
    }

    /// Whether the wait has lapsed at `now`, in seconds since 1970 ([`OwnWork::until`]).
    pub fn lapsed_at(&self, now: i64) -> bool {
        self.until.is_some_and(|until| until <= now)
    }

    /// The wait an older hook's line says, which counted helpers alone and not how many
    /// (#1626): one helper, with no limit, as it was read then.
    pub fn helpers_of_an_older_hook() -> Self {
        Self {
            helpers: 1,
            ..Self::default()
        }
    }
}

/// **What Claude Code's `Stop` `payload` says the chat waits on of its own**, as of `now` on
/// the machine's clock (the module's rules). Nothing for a payload that is absent or says
/// nothing readable.
pub fn read(payload: Option<&serde_json::Value>, now: DateTime<FixedOffset>) -> OwnWork {
    let Some(payload) = payload else {
        return OwnWork::default();
    };
    let mut work = OwnWork::default();
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    for task in list(payload, "background_tasks") {
        if !text(task, "status").is_some_and(|status| IN_FLIGHT.contains(&status.as_str())) {
            continue;
        }
        let count = match text(task, "type").as_deref() {
            Some(HELPER) => &mut work.helpers,
            Some(WORKFLOW) => &mut work.workflows,
            Some(MONITOR) => &mut work.monitors,
            Some(SHELL) => &mut work.shells,
            _ => continue,
        };
        *count = count.saturating_add(1);
    }
    let mut ends: Vec<i64> = Vec::new();
    if work.shells > 0 {
        ends.push((now + OWN_WORK_LIMIT).timestamp());
    }
    for cron in list(payload, "session_crons") {
        let Some(schedule) = text(cron, "schedule").and_then(|text| Cron::parse(&text)) else {
            continue;
        };
        let Some(recurring) = cron.get("recurring").and_then(serde_json::Value::as_bool) else {
            continue;
        };
        let ahead = if recurring {
            OWN_WORK_LIMIT
        } else {
            ONE_SHOT_LOOKS_AHEAD
        };
        let Some(due) = schedule.next_after(now, ahead) else {
            continue;
        };
        work.wakes_at = Some(work.wakes_at.map_or(due, |soonest| soonest.min(due)));
        ends.push(due + A_WAKE_UP_IS_LATE_AFTER.num_seconds());
    }
    let open = work.helpers > 0 || work.workflows > 0 || work.monitors > 0;
    work.until = if open { None } else { ends.into_iter().max() };
    work
}

/// The entries of the array `key` of `payload`, or none.
fn list<'a>(payload: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    payload
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// **A five-field cron schedule**: minute, hour, day of the month, month, day of the week, each
/// a `*`, a number, a range, a step of either, or a list of those. Read in the machine's local
/// time, as Claude Code fires it. A schedule this does not read is none, and counts as nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cron {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    /// Whether the day of the month was `*`, and the day of the week: where both name days, a
    /// day either names matches, as cron has it.
    any_day: bool,
    any_weekday: bool,
}

impl Cron {
    fn parse(text: &str) -> Option<Self> {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields.as_slice() else {
            return None;
        };
        let weekdays = field(weekday, 0, 7)?;
        Some(Self {
            minutes: field(minute, 0, 59)?,
            hours: field(hour, 0, 23)?,
            days: field(day, 1, 31)?,
            months: field(month, 1, 12)?,
            // Sunday is 0 and 7 alike.
            weekdays: (weekdays | (weekdays >> 7)) & 0x7f,
            any_day: day.starts_with('*'),
            any_weekday: weekday.starts_with('*'),
        })
    }

    fn day_matches(&self, at: NaiveDateTime) -> bool {
        let day = bit(self.days, at.day());
        let weekday = bit(self.weekdays, at.weekday().num_days_from_sunday());
        let month = bit(self.months, at.month());
        month
            && if self.any_day || self.any_weekday {
                day && weekday
            } else {
                day || weekday
            }
    }

    /// When it next fires after `now`, within `ahead`, in seconds since 1970: from the minute
    /// after `now`, since the minute `now` is in is the one a due run already fired in.
    fn next_after(&self, now: DateTime<FixedOffset>, ahead: Duration) -> Option<i64> {
        let local = now.naive_local();
        let last = local + ahead;
        let mut at = local.with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
        while at <= last {
            if !self.day_matches(at) {
                at = at.date().succ_opt()?.and_hms_opt(0, 0, 0)?;
            } else if !bit(self.hours, at.hour()) {
                at = at.with_minute(0)? + Duration::hours(1);
            } else if !bit(self.minutes, at.minute()) {
                at += Duration::minutes(1);
            } else {
                return Some(at.and_local_timezone(*now.offset()).single()?.timestamp());
            }
        }
        None
    }
}

fn bit(set: u64, n: u32) -> bool {
    set & (1 << n) != 0
}

/// One field of a cron schedule, as the set of the numbers from `low` to `high` it names.
fn field(text: &str, low: u32, high: u32) -> Option<u64> {
    let mut set = 0u64;
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, step.parse::<u32>().ok().filter(|step| *step > 0)?),
            None => (part, 1),
        };
        let (from, to): (u32, u32) = match range {
            "*" => (low, high),
            _ => match range.split_once('-') {
                Some((from, to)) => (from.parse().ok()?, to.parse().ok()?),
                // `5/15` is every fifteenth from five on.
                None if part.contains('/') => (range.parse().ok()?, high),
                None => {
                    let one = range.parse().ok()?;
                    (one, one)
                }
            },
        };
        if from < low || to > high || from > to {
            return None;
        }
        set |= (from..=to)
            .step_by(step as usize)
            .fold(0u64, |set, n| set | 1u64 << n);
    }
    Some(set)
}

#[cfg(test)]
mod tests;
