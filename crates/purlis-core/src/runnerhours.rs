// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **When each runner ran** (#3492, the person's ruling of 2026-10-28): `<config>/runner-hours.jsonl`,
//! Machine and device-bound, one line per runner start or stop, `{"at", "pool", "runner",
//! "event"}`, so the Devices view can sum each pool's hours this week.
//!
//! - **The times are Docker's own** ([`seen`]): a line is written as purlis reads a runner's
//!   container, from what Docker says of its latest run ([`Ran`]), and a stop as a runner is
//!   removed ([`stopped`]). A runner that suspends itself stops its own container; its stop is
//!   written at the next read. Reading the same container twice writes nothing more.
//! - **A run counts once it has a start** ([`ran_since`]): a stop with no start before it counts
//!   nothing, and a start with no stop after it counts up to now only while Docker says the
//!   container runs.
//! - **This week** is the ISO week in this machine's local time, from Monday 00:00
//!   ([`week_began`]).
//! - **Bounded** ([`pruned`]): each write drops the lines older than [`KEPT_FOR`], keeping each
//!   runner's last line, then the oldest lines until the file is at most [`MOST_BYTES`], within
//!   what a private file may be read at.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::dockerpool::Ran;
use crate::machine::{RUNNER_HOURS_FILE, RUNNER_HOURS_LOCK};

/// A runner started or stopped running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Start,
    Stop,
}

/// One line of the record: when, which runner of which pool, and whether it started or stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// In seconds since the epoch.
    pub at: u64,
    pub pool: String,
    pub runner: String,
    pub event: Event,
}

impl Line {
    fn of(&self, pool: &str, runner: &str) -> bool {
        self.pool == pool && self.runner == runner
    }

    fn text(&self) -> String {
        serde_json::json!({
            "at": self.at,
            "pool": self.pool,
            "runner": self.runner,
            "event": match self.event {
                Event::Start => "start",
                Event::Stop => "stop",
            },
        })
        .to_string()
    }

    /// The line `text` is, exactly as [`Line::text`] writes one, or `None`.
    fn read(text: &str) -> Option<Line> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let fields = value.as_object()?;
        if fields.len() != 4 {
            return None;
        }
        let name = |key: &str| {
            fields
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|name| plain_name(name))
                .map(str::to_owned)
        };
        let event = match fields.get("event")?.as_str()? {
            "start" => Event::Start,
            "stop" => Event::Stop,
            _ => return None,
        };
        Some(Line {
            at: fields.get("at")?.as_u64()?,
            pool: name("pool")?,
            runner: name("runner")?,
            event,
        })
    }
}

/// A pool's or runner's name as purlis keeps one: lower-case letters, digits and dashes.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// How long a line is kept: two weeks, so this week is always whole, whatever day it is.
pub const KEPT_FOR: u64 = 14 * 24 * 60 * 60;

/// The most the file is written at: below what a private file is read at (64 KiB).
pub const MOST_BYTES: usize = 48 * 1024;

/// **The lines Docker's word `ran` of `runner`'s container adds** to `lines` (#3492), at `now`:
/// none where the record says so already.
pub fn after_seen(lines: &[Line], pool: &str, runner: &str, ran: Ran, now: u64) -> Vec<Line> {
    let last = lines.iter().rev().find(|line| line.of(pool, runner));
    let floor = last.map_or(0, |line| line.at);
    // No line goes before the runner's last one, nor after now.
    let held = |at: u64| at.max(floor).min(now.max(floor));
    let line = |at: u64, event: Event| Line {
        at: held(at),
        pool: pool.to_owned(),
        runner: runner.to_owned(),
        event,
    };
    let open = last.is_some_and(|line| line.event == Event::Start);
    let mut added = Vec::new();
    match (ran.running, open) {
        (true, false) => added.push(line(ran.started.unwrap_or(now), Event::Start)),
        (true, true) => {
            // Started again since the start kept: the run that start began has ended, at the
            // latest when this one started.
            if let Some(started) = ran.started.filter(|started| *started > floor) {
                let ended = ran
                    .finished
                    .filter(|ended| (floor..=started).contains(ended))
                    .unwrap_or(started);
                added.push(line(ended, Event::Stop));
                added.push(line(started, Event::Start));
            }
        }
        (false, true) => match ran.started.filter(|started| *started > floor) {
            // It ran again, and stopped, since the start kept.
            Some(started) => {
                added.push(line(started, Event::Stop));
                added.push(line(started, Event::Start));
                added.push(line(ran.finished.unwrap_or(started), Event::Stop));
            }
            // A stop Docker gives no time for counts nothing more.
            None => added.push(line(ran.finished.unwrap_or(floor), Event::Stop)),
        },
        (false, false) => {
            // A whole run since the last line.
            if let (Some(started), Some(ended)) = (ran.started, ran.finished)
                && started > floor
                && ended >= started
            {
                added.push(line(started, Event::Start));
                added.push(line(ended, Event::Stop));
            }
        }
    }
    added
}

/// **The seconds `runner` of `pool` ran from `since`** (#3492), as `lines` say: each start to
/// the stop after it, and a start with none after it up to `open_until` where that is given
/// (Docker says it runs), else not at all.
pub fn ran_since(
    lines: &[Line],
    pool: &str,
    runner: &str,
    since: u64,
    open_until: Option<u64>,
) -> u64 {
    let within = |from: u64, to: u64| to.saturating_sub(from.max(since));
    let mut total = 0;
    let mut began = None;
    for line in lines.iter().filter(|line| line.of(pool, runner)) {
        match (line.event, began) {
            (Event::Start, None) => began = Some(line.at),
            (Event::Stop, Some(from)) => {
                total += within(from, line.at);
                began = None;
            }
            _ => {}
        }
    }
    match (began, open_until) {
        (Some(from), Some(until)) => total + within(from, until),
        _ => total,
    }
}

/// The runners of `pool` the record names, a removed one too.
pub fn runners_of(lines: &[Line], pool: &str) -> BTreeSet<String> {
    lines
        .iter()
        .filter(|line| line.pool == pool)
        .map(|line| line.runner.clone())
        .collect()
}

/// **When the ISO week holding `now` began**, Monday 00:00 in `now`'s time zone, in seconds
/// since the epoch. A Monday with no midnight (a clock moved forward over it) begins at its
/// first hour that is.
pub fn week_began<Tz: chrono::TimeZone>(now: &chrono::DateTime<Tz>) -> u64 {
    use chrono::Datelike;
    let local = now.naive_local();
    let back = chrono::Days::new(u64::from(local.weekday().num_days_from_monday()));
    let Some(monday) = local.date().checked_sub_days(back) else {
        return 0;
    };
    (0..24)
        .filter_map(|hour| monday.and_hms_opt(hour, 0, 0))
        .find_map(|midnight| now.timezone().from_local_datetime(&midnight).earliest())
        .and_then(|began| u64::try_from(began.timestamp()).ok())
        .unwrap_or(0)
}

/// [`week_began`] for now, in this machine's local time.
pub fn this_week_began() -> u64 {
    week_began(&chrono::Local::now())
}

/// **`lines`, bounded** (#3492): those older than [`KEPT_FOR`] at `now` dropped, each runner's
/// last line kept, then the oldest dropped until they are written in at most [`MOST_BYTES`].
pub fn pruned(lines: Vec<Line>, now: u64) -> Vec<Line> {
    let mut last: HashMap<(&str, &str), usize> = HashMap::new();
    for (at, line) in lines.iter().enumerate() {
        last.insert((line.pool.as_str(), line.runner.as_str()), at);
    }
    let lasts: BTreeSet<usize> = last.into_values().collect();
    let mut kept: Vec<Line> = lines
        .iter()
        .enumerate()
        .filter(|(at, line)| line.at.saturating_add(KEPT_FOR) >= now || lasts.contains(at))
        .map(|(_, line)| line.clone())
        .collect();
    let mut bytes: usize = kept.iter().map(|line| line.text().len() + 1).sum();
    let mut oldest = 0;
    while bytes > MOST_BYTES && oldest < kept.len() {
        bytes -= kept[oldest].text().len() + 1;
        oldest += 1;
    }
    kept.drain(..oldest);
    kept
}

/// **The record under `config_root`**: none where there is no file, or where it cannot be read
/// safely, which is said; a line purlis would not write is skipped.
pub fn read(config_root: &Path) -> Vec<Line> {
    let path = crate::machine::dir(config_root).join(RUNNER_HOURS_FILE);
    match purlis_same_user::read_private_file(&path) {
        Ok(text) => text.lines().filter_map(Line::read).collect(),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(why) => {
            tracing::warn!(
                "purlis: when each runner ran could not be read safely ({}), so no hours are \
                 counted from it: {why}",
                path.display()
            );
            Vec::new()
        }
    }
}

/// **Writes what Docker's word `ran` of `runner`'s container adds** (#3492): nothing where the
/// record says so already. Under the record's lock, the record bounded as it is written.
pub fn seen(
    config_root: &Path,
    pool: &str,
    runner: &str,
    ran: Ran,
    now: u64,
) -> Result<(), String> {
    changed(config_root, now, |lines| {
        after_seen(lines, pool, runner, ran, now)
    })
}

/// **Writes that `runner` of `pool` stopped at `now`**, where its last line is a start: as it is
/// removed (#3492).
pub fn stopped(config_root: &Path, pool: &str, runner: &str, now: u64) -> Result<(), String> {
    changed(config_root, now, |lines| {
        let last = lines.iter().rev().find(|line| line.of(pool, runner));
        match last {
            Some(line) if line.event == Event::Start => vec![Line {
                at: now.max(line.at),
                pool: pool.to_owned(),
                runner: runner.to_owned(),
                event: Event::Stop,
            }],
            _ => Vec::new(),
        }
    })
}

fn changed(
    config_root: &Path,
    now: u64,
    adding: impl FnOnce(&[Line]) -> Vec<Line>,
) -> Result<(), String> {
    let _locked = crate::machine::Lock::named(config_root, RUNNER_HOURS_LOCK);
    let mut lines = read(config_root);
    let added = adding(&lines);
    if added.is_empty() {
        return Ok(());
    }
    lines.extend(added);
    let text: String = pruned(lines, now)
        .iter()
        .map(|line| line.text() + "\n")
        .collect();
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .and_then(|folder| folder.replace_file(RUNNER_HOURS_FILE, text.as_bytes()))
        .map_err(|why| format!("when each runner ran could not be written: {why}"))
}

#[cfg(test)]
mod tests;
