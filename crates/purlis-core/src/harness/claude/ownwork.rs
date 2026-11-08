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

use crate::ownwork::pieces::{Kind, Piece};
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

/// **The piece of own work a finished tool call started** (#5750), from Claude Code's
/// `PostToolUse` `payload`, or none: a command it moved to the background (`backgroundTaskId`),
/// a `Monitor`, an `Agent` launched in the background, a `Workflow`, a `CronCreate` and a
/// `ScheduleWakeup`. `output_of` finds the file Claude Code keeps a task's output in, by the
/// task's id ([`output_file`]).
///
/// Read from the tool's input and result schemas of 2.1.319. A result this cannot read starts
/// nothing: the turn's `Stop` still says what is in flight ([`pieces`]).
pub fn started(
    payload: &serde_json::Value,
    output_of: &dyn Fn(&str) -> Option<String>,
) -> Option<Piece> {
    let input = &payload["tool_input"];
    let response = &payload["tool_response"];
    let text = |value: &serde_json::Value| value.as_str().unwrap_or_default().to_owned();
    let either = |first: &serde_json::Value, second: &serde_json::Value| {
        first
            .as_str()
            .filter(|said| !said.trim().is_empty())
            .or_else(|| second.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    let task = |kind: Kind, id: &serde_json::Value, what: String| {
        let id = id.as_str()?;
        Some(Piece {
            output: output_of(id),
            ..Piece::new(kind, id, &what)?
        })
    };
    let launched = response["status"].as_str() == Some("async_launched");
    match payload["tool_name"].as_str()? {
        "Bash" => task(
            Kind::Shell,
            &response["backgroundTaskId"],
            either(&input["command"], &input["description"]),
        ),
        "Monitor" => task(
            Kind::Monitor,
            &response["taskId"],
            either(&input["description"], &input["command"]),
        ),
        "Agent" | "Task" if launched => {
            let id = response["agentId"].as_str()?;
            let named = response["outputFile"]
                .as_str()
                .filter(|path| !path.is_empty())
                .map(str::to_owned);
            Some(Piece {
                output: named.or_else(|| output_of(id)),
                ..Piece::new(
                    Kind::Subagent,
                    id,
                    &either(&response["description"], &input["description"]),
                )?
            })
        }
        "Workflow" if launched => task(
            Kind::Workflow,
            &response["taskId"],
            either(&response["workflowName"], &input["name"]),
        ),
        "CronCreate" => {
            let recurring = response["recurring"]
                .as_bool()
                .or_else(|| input["recurring"].as_bool())
                .unwrap_or(true);
            let kind = if recurring { Kind::Cron } else { Kind::WakeUp };
            Piece {
                schedule: Some(text(&input["cron"])),
                prompt: Some(text(&input["prompt"])),
                due_at: (!recurring)
                    .then(|| response["nextFireAt"].as_i64().map(|ms| ms / 1000))
                    .flatten(),
                ..Piece::new(kind, response["id"].as_str()?, "")?
            }
            .sound()
        }
        "ScheduleWakeup" => {
            if response["stopped"].as_bool() == Some(true) {
                return None;
            }
            let due = response["scheduledFor"].as_i64()? / 1000;
            Piece {
                prompt: Some(text(&input["prompt"])),
                due_at: Some(due),
                ..Piece::new(
                    Kind::WakeUp,
                    payload["tool_use_id"].as_str()?,
                    &text(&input["reason"]),
                )?
            }
            .sound()
        }
        _ => None,
    }
}

/// **Each piece of own work Claude Code's `Stop` `payload` says is in flight** (#5750), the
/// authority on what still runs ([`crate::ownwork::pieces::at_stop`]): its in-flight
/// `background_tasks` of the kinds [`read`] counts, and every one of its `session_crons`. A
/// cron that runs again is a [`Kind::Cron`], whenever it next runs, since it is lost all the
/// same; one that runs once is a [`Kind::WakeUp`], due as [`read`] reads it. None for a payload
/// that names neither list (an older Claude Code), which says nothing either way.
///
/// `now` is the chat's own wall clock and `epoch` the same moment in seconds since 1970.
pub fn pieces(
    payload: Option<&serde_json::Value>,
    now: NaiveDateTime,
    epoch: i64,
    output_of: &dyn Fn(&str) -> Option<String>,
) -> Option<Vec<Piece>> {
    let payload = payload?;
    if payload.get("background_tasks").is_none() && payload.get("session_crons").is_none() {
        return None;
    }
    let text = |entry: &serde_json::Value, key: &str| {
        entry
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut said: Vec<Piece> = Vec::new();
    for task in list(payload, "background_tasks") {
        if !IN_FLIGHT.contains(&text(task, "status").as_str()) {
            continue;
        }
        let kind = match text(task, "type").as_str() {
            HELPER => Kind::Subagent,
            WORKFLOW => Kind::Workflow,
            MONITOR => Kind::Monitor,
            SHELL => Kind::Shell,
            _ => continue,
        };
        let what = [
            text(task, "command"),
            text(task, "name"),
            text(task, "description"),
        ]
        .into_iter()
        .find(|said| !said.trim().is_empty())
        .unwrap_or_default();
        let id = text(task, "id");
        if let Some(piece) = Piece::new(kind, &id, &what) {
            said.push(Piece {
                output: output_of(&id),
                ..piece
            });
        }
    }
    for cron in list(payload, "session_crons") {
        let schedule = text(cron, "schedule");
        let recurring = cron["recurring"].as_bool().unwrap_or(true);
        let due_at = (!recurring)
            .then(|| Cron::parse(&schedule))
            .flatten()
            .and_then(|parsed| {
                // Due now only where it matched in the last few minutes: a wake-up's schedule
                // (`ScheduleWakeup`'s `M H * * *`) matches every day, so a day's look back
                // would call one still hours ahead overdue.
                if parsed.fired_within(now, crate::ownwork::A_WAKE_UP_IS_LATE_AFTER) {
                    Some(epoch)
                } else {
                    let next = parsed.next_after(now, ONE_SHOT_LOOKS_AHEAD)?;
                    Some(epoch.saturating_add((next - now).num_seconds()))
                }
            });
        let kind = if recurring { Kind::Cron } else { Kind::WakeUp };
        if let Some(piece) = Piece::new(kind, &text(cron, "id"), "") {
            said.extend(
                Piece {
                    schedule: Some(schedule),
                    prompt: Some(text(cron, "prompt")),
                    due_at,
                    ..piece
                }
                .sound(),
            );
        }
    }
    let mut kept = Vec::new();
    for piece in said {
        crate::ownwork::pieces::started(&mut kept, piece);
    }
    Some(kept)
}

/// **The file Claude Code keeps the output of task `id` in**, for the conversation `session`:
/// `<root>/claude-<uid>/<project>/<session>/tasks/<id>.output`, where the root is the chat's own
/// temporary folder (`CLAUDE_CODE_TMPDIR`, read through `env`) or else `/tmp`, as Claude Code
/// roots it (#5675). Looked for, not worked out: how Claude Code names the project's folder is
/// its own. None where no such file is there, or `session` or `id` is no id.
pub fn output_file(
    session: &str,
    id: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    output_in(&tasks_folder(session, env)?, id)
}

/// The file Claude Code keeps the output of task `id` in, in the conversation's `tasks` folder,
/// where it is there.
pub fn output_in(tasks: &std::path::Path, id: &str) -> Option<String> {
    if !crate::ownwork::pieces::an_id(id) {
        return None;
    }
    let file = tasks.join(format!("{id}.output"));
    file.exists()
        .then(|| file.to_str().map(str::to_owned))
        .flatten()
}

/// **The folder Claude Code keeps conversation `session`'s task output in** ([`output_file`]),
/// looked for once, so a `Stop` naming many pieces lists the temporary folder once.
pub fn tasks_folder(
    session: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<std::path::PathBuf> {
    if session.is_empty()
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    let uid = rustix::process::getuid().as_raw();
    let roots = env(crate::sandbox::claude_wrap::TMP_ENV)
        .filter(|root| !root.is_empty())
        .into_iter()
        .chain(std::iter::once("/tmp".to_owned()));
    for root in roots {
        let base = std::path::Path::new(&root).join(format!("claude-{uid}"));
        let Ok(projects) = std::fs::read_dir(&base) else {
            continue;
        };
        for project in projects.flatten() {
            let tasks = project.path().join(session).join("tasks");
            if tasks.is_dir() {
                return Some(tasks);
            }
        }
    }
    None
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
        self.fired_within(now, OVERDUE_LOOKS_BACK)
    }

    /// Whether it fired in the minute `now` is in, or in the `back` before it.
    fn fired_within(&self, now: NaiveDateTime, back: Duration) -> bool {
        let Some(minute) = now.with_second(0).and_then(|at| at.with_nanosecond(0)) else {
            return false;
        };
        (0..=back.num_minutes()).any(|ago| self.matches(minute - Duration::minutes(ago)))
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
