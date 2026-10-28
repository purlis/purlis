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
//! - **Bounded** ([`pruned`]): each write drops the runs older than [`KEPT_FOR`], keeping each
//!   runner's last line, then whole runs by when they ended, earliest first, until the file is
//!   at most [`MOST_BYTES`], within what a private file may be read at. So a run that ended
//!   before this week began, which counts nothing toward it, goes before any run that counts,
//!   and a run still open goes last. A run is never split: its start and the stop that ends it
//!   are kept or dropped together.

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
    /// Fields a later purlis may write, kept as they were through every write (#3492).
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// The four fields every line has; any other is kept in [`Line::other`].
const FIELDS: [&str; 4] = ["at", "pool", "runner", "event"];

impl Line {
    /// A line with no other field.
    pub fn new(at: u64, pool: &str, runner: &str, event: Event) -> Line {
        Line {
            at,
            pool: pool.to_owned(),
            runner: runner.to_owned(),
            event,
            other: serde_json::Map::new(),
        }
    }

    fn of(&self, pool: &str, runner: &str) -> bool {
        self.pool == pool && self.runner == runner
    }

    fn text(&self) -> String {
        let mut value = serde_json::json!({
            "at": self.at,
            "pool": self.pool,
            "runner": self.runner,
            "event": match self.event {
                Event::Start => "start",
                Event::Stop => "stop",
            },
        });
        if let Some(fields) = value.as_object_mut() {
            for (key, other) in &self.other {
                if !FIELDS.contains(&key.as_str()) {
                    fields.insert(key.clone(), other.clone());
                }
            }
        }
        value.to_string()
    }

    /// The line `text` is, as [`Line::text`] writes one, or `None`. A field it does not know is
    /// kept, so a line a later purlis wrote is neither skipped nor dropped at the next write.
    fn read(text: &str) -> Option<Line> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let fields = value.as_object()?;
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
            other: fields
                .iter()
                .filter(|(key, _)| !FIELDS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
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
    let line = |at: u64, event: Event| Line::new(held(at), pool, runner, event);
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
/// last line kept, then whole runs by when they ended, earliest first and a run still open last,
/// until they are written in at most [`MOST_BYTES`] (#3723).
///
/// **A run is never split**: a start and the stop that ends it are kept or dropped together, so a
/// run longer than [`KEPT_FOR`] that has just stopped keeps its start, and its hours this week.
pub fn pruned(lines: Vec<Line>, now: u64) -> Vec<Line> {
    // Each line's group: a stop is in its start's, as [`ran_since`] pairs them; any other line
    // is a group of its own, named by its index.
    let mut group: Vec<usize> = (0..lines.len()).collect();
    let mut kept_groups = BTreeSet::new();
    {
        let mut open: HashMap<(&str, &str), usize> = HashMap::new();
        let mut last: HashMap<(&str, &str), usize> = HashMap::new();
        for (at, line) in lines.iter().enumerate() {
            let key = (line.pool.as_str(), line.runner.as_str());
            match line.event {
                Event::Start => {
                    open.entry(key).or_insert(at);
                }
                Event::Stop => {
                    if let Some(start) = open.remove(&key) {
                        group[at] = start;
                    }
                }
            }
            last.insert(key, at);
        }
        let lasts: BTreeSet<usize> = last.into_values().collect();
        for (at, line) in lines.iter().enumerate() {
            if line.at.saturating_add(KEPT_FOR) >= now || lasts.contains(&at) {
                kept_groups.insert(group[at]);
            }
        }
    }
    let kept: Vec<(usize, Line)> = lines
        .into_iter()
        .enumerate()
        .filter(|(at, _)| kept_groups.contains(&group[*at]))
        .map(|(at, line)| (group[at], line))
        .collect();
    // Then whole groups, by when they ended, until the rest fit (#3723): a group's end is its
    // stop's time, or the time of a line alone that is no start. A start alone is a run still
    // open, and goes last. So a run that ended before this week began, which counts nothing
    // toward it, goes before any that counts.
    let mut sizes: HashMap<usize, usize> = HashMap::new();
    let mut ends: HashMap<usize, Option<u64>> = HashMap::new();
    for (of, line) in &kept {
        *sizes.entry(*of).or_default() += line.text().len() + 1;
        let end = ends.entry(*of).or_insert(None);
        if line.event == Event::Stop {
            *end = Some(line.at);
        }
    }
    let mut bytes: usize = sizes.values().sum();
    let mut order: Vec<usize> = sizes.keys().copied().collect();
    order.sort_by_key(|of| (ends[of].is_none(), ends[of], *of));
    let mut dropped = BTreeSet::new();
    for of in order {
        if bytes <= MOST_BYTES {
            break;
        }
        dropped.insert(of);
        bytes -= sizes[&of];
    }
    kept.into_iter()
        .filter(|(of, _)| !dropped.contains(of))
        .map(|(_, line)| line)
        .collect()
}

/// **The record under `config_root`**: none where there is no file, or where it cannot be read
/// safely, which is said; a line purlis would not write is skipped, and one with a field it does
/// not know is kept with it.
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
            Some(line) if line.event == Event::Start => {
                vec![Line::new(now.max(line.at), pool, runner, Event::Stop)]
            }
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
