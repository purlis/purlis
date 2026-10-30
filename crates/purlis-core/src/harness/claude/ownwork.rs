// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What Claude Code's `Stop` says its chat waits on of its own** (#3878): the Claude Code half
//! of [`crate::ownwork`], whose rules this hands its reading to.
//!
//! Claude Code's `Stop` input carries `background_tasks`, each with a `type` (its label for the
//! kind of work) and a `status`, and `session_crons`, each with a five-field cron `schedule`
//! and whether it is `recurring` (read from the hook input schema of 2.1.312). Its crons fire on
//! the wall clock of the machine the chat runs on, which is the machine its hook runs on: so a
//! schedule is read there, on that clock, into seconds from now, and the app stamps those on
//! its own ([`crate::ownwork::Said::at`]).
//!
//! Anything not read as one of these counts as nothing, so a payload this cannot read hands the
//! end of the turn to the person, as before.

use chrono::{Datelike, Duration, NaiveDateTime, Timelike};

use crate::ownwork::{InFlight, OWN_WORK_LIMIT, Said, WakeUp};

/// The kinds of background work, by Claude Code's label for them.
const HELPER: &str = "subagent";
const WORKFLOW: &str = "workflow";
const MONITOR: &str = "monitor";
const SHELL: &str = "shell";

/// The statuses of work still in flight: running, or pending. Read from the code that builds
/// the list in Claude Code 2.1.296, which keeps only an entry with one of these two.
const IN_FLIGHT: [&str; 2] = ["running", "pending"];

/// How far ahead a one-shot wake-up is looked for: a year and a day, the longest a five-field
/// cron that names one minute of one day can lie ahead.
const ONE_SHOT_LOOKS_AHEAD: Duration = Duration::days(367);

/// How far back a one-shot wake-up still listed is read as overdue, and so due now: one that
/// matched in the last day has not fired yet (Claude Code drops one that has).
const OVERDUE_LOOKS_BACK: Duration = Duration::days(1);

/// **What Claude Code's `Stop` `payload` says the chat waits on of its own**, read at `now` on
/// the chat's own wall clock. Nothing for a payload that is absent or says nothing readable.
pub fn read(payload: Option<&serde_json::Value>, now: NaiveDateTime) -> Said {
    let Some(payload) = payload else {
        return Said::default();
    };
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut in_flight = InFlight::default();
    for task in list(payload, "background_tasks") {
        if !text(task, "status").is_some_and(|status| IN_FLIGHT.contains(&status.as_str())) {
            continue;
        }
        let count = match text(task, "type").as_deref() {
            Some(HELPER) => &mut in_flight.helpers,
            Some(WORKFLOW) => &mut in_flight.workflows,
            Some(MONITOR) => &mut in_flight.monitors,
            Some(SHELL) => &mut in_flight.shells,
            _ => continue,
        };
        *count = count.saturating_add(1);
    }
    let wake_ups: Vec<WakeUp> = list(payload, "session_crons")
        .iter()
        .filter_map(|cron| {
            let schedule = Cron::parse(&text(cron, "schedule")?)?;
            let recurring = cron.get("recurring")?.as_bool()?;
            let due_in = if !recurring && schedule.fired_lately(now) {
                // Due already, or overdue: due now (#3878).
                0
            } else {
                let ahead = if recurring {
                    OWN_WORK_LIMIT
                } else {
                    ONE_SHOT_LOOKS_AHEAD
                };
                (schedule.next_after(now, ahead)? - now).num_seconds()
            };
            Some(WakeUp { due_in, recurring })
        })
        .collect();
    crate::ownwork::said(in_flight, &wake_ups)
}

/// The entries of the array `key` of `payload`, or none.
fn list<'a>(payload: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    payload
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map_or(&[] as &[serde_json::Value], Vec::as_slice)
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

    fn matches(&self, at: NaiveDateTime) -> bool {
        self.day_matches(at) && bit(self.hours, at.hour()) && bit(self.minutes, at.minute())
    }

    /// The minute it next fires in, from the minute after `now` to `ahead` past it, on the
    /// chat's own wall clock: the minute `now` is in is one a due run already fired in.
    fn next_after(&self, now: NaiveDateTime, ahead: Duration) -> Option<NaiveDateTime> {
        let last = now + ahead;
        let mut at = now.with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
        while at <= last {
            if !self.day_matches(at) {
                at = at.date().succ_opt()?.and_hms_opt(0, 0, 0)?;
            } else if !bit(self.hours, at.hour()) {
                at = at.with_minute(0)? + Duration::hours(1);
            } else if !bit(self.minutes, at.minute()) {
                at += Duration::minutes(1);
            } else {
                return Some(at);
            }
        }
        None
    }

    /// Whether it fired in the minute `now` is in, or in the [`OVERDUE_LOOKS_BACK`] before it.
    fn fired_lately(&self, now: NaiveDateTime) -> bool {
        let Some(minute) = now.with_second(0).and_then(|at| at.with_nanosecond(0)) else {
            return false;
        };
        (0..=OVERDUE_LOOKS_BACK.num_minutes())
            .any(|back| self.matches(minute - Duration::minutes(back)))
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
