// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const HOUR: u64 = 60 * 60;
/// Monday 2026-10-26 00:00 UTC.
const MONDAY: u64 = 1_792_972_800;

fn running(started: u64) -> Ran {
    Ran {
        running: true,
        started: Some(started),
        finished: None,
    }
}

fn ended(started: u64, finished: u64) -> Ran {
    Ran {
        running: false,
        started: Some(started),
        finished: Some(finished),
    }
}

/// The record after each of `reads` of the runner `builds` of pool `builds`, at its time.
fn after(reads: &[(Ran, u64)]) -> Vec<Line> {
    let mut lines = Vec::new();
    for (ran, now) in reads {
        let added = after_seen(&lines, "builds", "builds", *ran, *now);
        lines.extend(added);
    }
    lines
}

fn events(lines: &[Line]) -> Vec<(u64, Event)> {
    lines.iter().map(|line| (line.at, line.event)).collect()
}

#[test]
fn a_run_is_recorded_at_dockers_times_and_reading_it_again_adds_nothing() {
    let start = MONDAY + HOUR;
    let end = MONDAY + 4 * HOUR;
    let lines = after(&[
        (running(start), start + 60),
        (running(start), start + 120),
        (ended(start, end), end + 600),
        (ended(start, end), end + 900),
    ]);
    assert_eq!(events(&lines), [(start, Event::Start), (end, Event::Stop)]);
    assert_eq!(
        ran_since(&lines, "builds", "builds", MONDAY, None),
        3 * HOUR
    );
}

#[test]
fn a_runner_that_suspended_itself_between_two_reads_is_stopped_when_it_stopped() {
    // Read running, then suspended and started again by purlis: the stop is Docker's.
    let first = MONDAY + HOUR;
    let suspended = MONDAY + 2 * HOUR;
    let again = MONDAY + 5 * HOUR;
    let lines = after(&[
        (running(first), first + 10),
        (ended(first, suspended), again - 1),
        (running(again), again + 10),
    ]);
    assert_eq!(
        events(&lines),
        [
            (first, Event::Start),
            (suspended, Event::Stop),
            (again, Event::Start)
        ]
    );
    // It runs now: its open run counts up to now.
    let now = again + HOUR;
    assert_eq!(
        ran_since(&lines, "builds", "builds", MONDAY, Some(now)),
        2 * HOUR
    );
}

#[test]
fn a_container_started_again_outside_purlis_ends_the_run_kept_at_its_last_finish() {
    let first = MONDAY + HOUR;
    let ended = MONDAY + 2 * HOUR;
    let again = MONDAY + 3 * HOUR;
    let lines = after(&[
        (running(first), first + 10),
        (
            Ran {
                running: true,
                started: Some(again),
                finished: Some(ended),
            },
            again + 10,
        ),
    ]);
    assert_eq!(
        events(&lines),
        [
            (first, Event::Start),
            (ended, Event::Stop),
            (again, Event::Start)
        ]
    );
}

#[test]
fn a_whole_run_between_two_reads_is_recorded() {
    let start = MONDAY + HOUR;
    let end = MONDAY + 2 * HOUR;
    let lines = after(&[(ended(start, end), end + 10)]);
    assert_eq!(events(&lines), [(start, Event::Start), (end, Event::Stop)]);
}

#[test]
fn a_stop_docker_gives_no_time_for_counts_nothing_more() {
    let start = MONDAY + HOUR;
    let lines = after(&[
        (running(start), start + 10),
        (
            Ran {
                running: false,
                started: Some(start),
                finished: None,
            },
            start + 5 * HOUR,
        ),
    ]);
    assert_eq!(
        events(&lines),
        [(start, Event::Start), (start, Event::Stop)]
    );
    assert_eq!(ran_since(&lines, "builds", "builds", MONDAY, None), 0);
}

#[test]
fn an_open_start_counts_only_while_docker_says_it_runs_and_only_this_week() {
    let lines = after(&[(running(MONDAY - 2 * HOUR), MONDAY + 10)]);
    let now = MONDAY + 3 * HOUR;
    assert_eq!(
        ran_since(&lines, "builds", "builds", MONDAY, Some(now)),
        3 * HOUR
    );
    assert_eq!(ran_since(&lines, "builds", "builds", MONDAY, None), 0);
}

#[test]
fn a_stop_with_no_start_before_it_counts_nothing_and_other_runners_count_apart() {
    let line = |at, runner: &str, event| Line::new(at, "builds", runner, event);
    let lines = vec![
        line(MONDAY + HOUR, "builds", Event::Stop),
        line(MONDAY + HOUR, "builds-1a2b3c4d", Event::Start),
        line(MONDAY + 2 * HOUR, "builds", Event::Start),
        line(MONDAY + 3 * HOUR, "builds-1a2b3c4d", Event::Stop),
        line(MONDAY + 4 * HOUR, "builds", Event::Stop),
    ];
    assert_eq!(
        ran_since(&lines, "builds", "builds", MONDAY, None),
        2 * HOUR
    );
    assert_eq!(
        ran_since(&lines, "builds", "builds-1a2b3c4d", MONDAY, None),
        2 * HOUR
    );
    assert_eq!(
        runners_of(&lines, "builds").into_iter().collect::<Vec<_>>(),
        ["builds", "builds-1a2b3c4d"]
    );
}

#[test]
fn this_week_begins_on_monday_at_midnight_in_local_time() {
    use chrono::TimeZone as _;
    // Wednesday 2026-10-28 10:00 at UTC+02:00 began its week on Monday 00:00 there,
    // which is Sunday 22:00 UTC.
    let east = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
    let wednesday = east.with_ymd_and_hms(2026, 10, 28, 10, 0, 0).unwrap();
    assert_eq!(week_began(&wednesday), MONDAY - 2 * HOUR);
    // Monday at midnight is its own week's start; Sunday night is the week before's.
    let monday = chrono::Utc.with_ymd_and_hms(2026, 10, 26, 0, 0, 0).unwrap();
    assert_eq!(week_began(&monday), MONDAY);
    let sunday = chrono::Utc
        .with_ymd_and_hms(2026, 10, 25, 23, 59, 59)
        .unwrap();
    assert_eq!(week_began(&sunday), MONDAY - 7 * 24 * HOUR);
}

#[test]
fn the_record_drops_what_is_two_weeks_old_but_each_runners_last_run() {
    let now = MONDAY + 20 * 24 * HOUR;
    let line = |at, runner: &str, event| Line::new(at, "builds", runner, event);
    let lines = vec![
        line(MONDAY, "gone", Event::Start),
        line(MONDAY + HOUR, "gone", Event::Stop),
        line(MONDAY + 2 * HOUR, "old", Event::Start),
        line(MONDAY + 3 * HOUR, "old", Event::Stop),
        line(MONDAY + 4 * HOUR, "long", Event::Start),
        line(now - HOUR, "new", Event::Start),
        line(MONDAY + 5 * HOUR, "gone", Event::Start),
        line(MONDAY + 6 * HOUR, "gone", Event::Stop),
    ];
    // `gone`'s first run is two weeks old and not its last: it goes, start and stop together.
    // `old`'s last run stays whole, its start with its stop.
    assert_eq!(pruned(lines.clone(), now), lines[2..].to_vec());
}

/// A run longer than two weeks that has just stopped keeps its start, so its hours this week
/// are counted from Monday to its stop, and are not lost as it stops.
#[test]
fn a_run_longer_than_two_weeks_keeps_its_start_when_it_stops() {
    let now = MONDAY + 3 * 24 * HOUR;
    let lines = vec![
        Line::new(now - 15 * 24 * HOUR, "builds", "builds", Event::Start),
        Line::new(now - 24 * HOUR, "builds", "builds", Event::Stop),
    ];
    let kept = pruned(lines.clone(), now);
    assert_eq!(kept, lines);
    assert_eq!(
        ran_since(&kept, "builds", "builds", MONDAY, None),
        now - 24 * HOUR - MONDAY
    );
}

#[test]
fn the_record_is_held_to_its_most_bytes_by_dropping_the_oldest() {
    let now = MONDAY + 7 * 24 * HOUR;
    let lines: Vec<Line> = (0..2_000)
        .map(|n| {
            let event = if n % 2 == 0 {
                Event::Start
            } else {
                Event::Stop
            };
            Line::new(MONDAY + n, "builds", "builds", event)
        })
        .collect();
    let kept = pruned(lines.clone(), now);
    let bytes: usize = kept.iter().map(|line| line.text().len() + 1).sum();
    assert!(bytes <= MOST_BYTES, "{bytes}");
    assert!(kept.len() < lines.len());
    assert_eq!(kept.last(), lines.last(), "the newest are kept");
    // Runs are dropped whole: what is kept begins with a start, never with a stop alone.
    assert_eq!(kept[0].event, Event::Start);
    assert_eq!(kept.len() % 2, 0);
}

#[test]
fn the_record_is_written_for_its_user_alone_and_read_back() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let start = MONDAY + HOUR;
    seen(
        config.path(),
        "builds",
        "builds",
        running(start),
        start + 10,
    )
    .unwrap();
    stopped(config.path(), "builds", "builds", start + HOUR).unwrap();
    // A second stop adds nothing: the last line is a stop already.
    stopped(config.path(), "builds", "builds", start + 2 * HOUR).unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNER_HOURS_FILE);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!(
            "{{\"at\":{start},\"pool\":\"builds\",\"runner\":\"builds\",\"event\":\"start\"}}\n\
             {{\"at\":{},\"pool\":\"builds\",\"runner\":\"builds\",\"event\":\"stop\"}}\n",
            start + HOUR
        )
    );
    assert_eq!(
        events(&read(config.path())),
        [(start, Event::Start), (start + HOUR, Event::Stop)]
    );
}

#[test]
fn a_line_purlis_would_not_write_is_skipped() {
    let config = tempfile::tempdir().unwrap();
    seen(
        config.path(),
        "builds",
        "builds",
        running(MONDAY),
        MONDAY + 1,
    )
    .unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNER_HOURS_FILE);
    let mut text = std::fs::read_to_string(&path).unwrap();
    for bad in [
        "not json",
        r#"{"at":1,"event":"start","pool":"builds"}"#,
        r#"{"at":1,"event":"paused","pool":"builds","runner":"builds"}"#,
        r#"{"at":-1,"event":"stop","pool":"builds","runner":"builds"}"#,
        r#"{"at":1,"event":"stop","pool":"Builds","runner":"builds"}"#,
    ] {
        text.push_str(bad);
        text.push('\n');
    }
    std::fs::write(&path, text).unwrap();
    assert_eq!(events(&read(config.path())), [(MONDAY, Event::Start)]);
}

/// A line a later purlis wrote, with a field this one does not know, is read and written back
/// with it, not skipped and dropped at the next write.
#[test]
fn a_field_a_later_purlis_wrote_is_kept_through_a_write() {
    let config = tempfile::tempdir().unwrap();
    let path = crate::machine::dir(config.path()).join(crate::machine::RUNNER_HOURS_FILE);
    seen(
        config.path(),
        "builds",
        "builds",
        running(MONDAY),
        MONDAY + 1,
    )
    .unwrap();
    let later = format!(
        "{{\"at\":{},\"pool\":\"tests\",\"runner\":\"tests\",\"event\":\"start\",\"claim\":\"c-1\"}}\n",
        MONDAY + 2
    );
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&later);
    std::fs::write(&path, text).unwrap();
    assert_eq!(read(config.path()).len(), 2);
    stopped(config.path(), "builds", "builds", MONDAY + HOUR).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains(later.trim_end()), "{after}");
}

/// A test time zone whose clocks go forward from 00:00 to 01:00 on Monday 2026-10-26.
#[derive(Debug, Clone, Copy)]
struct SkipsMondayMidnight;

impl SkipsMondayMidnight {
    fn switch() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 26)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    fn offset(summer: bool) -> chrono::FixedOffset {
        chrono::FixedOffset::east_opt(if summer { 3600 } else { 0 }).unwrap()
    }
}

impl chrono::TimeZone for SkipsMondayMidnight {
    type Offset = chrono::FixedOffset;

    fn from_offset(_: &chrono::FixedOffset) -> Self {
        SkipsMondayMidnight
    }

    fn offset_from_local_date(
        &self,
        _: &chrono::NaiveDate,
    ) -> chrono::MappedLocalTime<chrono::FixedOffset> {
        chrono::MappedLocalTime::None
    }

    fn offset_from_local_datetime(
        &self,
        local: &chrono::NaiveDateTime,
    ) -> chrono::MappedLocalTime<chrono::FixedOffset> {
        let switch = Self::switch();
        if *local < switch {
            chrono::MappedLocalTime::Single(Self::offset(false))
        } else if *local < switch + chrono::Duration::hours(1) {
            chrono::MappedLocalTime::None
        } else {
            chrono::MappedLocalTime::Single(Self::offset(true))
        }
    }

    fn offset_from_utc_date(&self, _: &chrono::NaiveDate) -> chrono::FixedOffset {
        Self::offset(false)
    }

    fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> chrono::FixedOffset {
        Self::offset(*utc >= Self::switch())
    }
}

#[test]
fn a_week_whose_monday_has_no_midnight_begins_at_its_first_hour() {
    use chrono::TimeZone as _;
    let wednesday = SkipsMondayMidnight
        .with_ymd_and_hms(2026, 10, 28, 10, 0, 0)
        .unwrap();
    // Monday 01:00 there, the first hour it has, is 00:00 UTC.
    assert_eq!(week_began(&wednesday), MONDAY);
}
