// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// 14:00:30 on 2026-10-29, a Thursday, four hours east of UTC.
fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-10-29T14:00:30+04:00").expect("a time")
}

/// `at` (`HH:MM` on the same day as [`now`], or a full RFC 3339 time) in seconds since 1970.
fn at(when: &str) -> i64 {
    let full = if when.len() == 5 {
        format!("2026-10-29T{when}:00+04:00")
    } else {
        when.to_owned()
    };
    DateTime::parse_from_rfc3339(&full)
        .expect("a time")
        .timestamp()
}

/// A `Stop` payload with `tasks` as `(type, status)` and `crons` as `(schedule, recurring)`,
/// shaped as Claude Code 2.1.312's hook input schema says.
fn stop(tasks: &[(&str, &str)], crons: &[(&str, bool)]) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "Stop",
        "background_tasks": tasks.iter().enumerate().map(|(n, (kind, status))| serde_json::json!({
            "id": format!("t{n}"), "type": kind, "status": status, "description": "work",
        })).collect::<Vec<_>>(),
        "session_crons": crons.iter().enumerate().map(|(n, (schedule, recurring))| serde_json::json!({
            "id": format!("c{n}"), "schedule": schedule, "recurring": recurring, "prompt": "check",
        })).collect::<Vec<_>>(),
    })
}

fn read_stop(tasks: &[(&str, &str)], crons: &[(&str, bool)]) -> OwnWork {
    read(Some(&stop(tasks, crons)), now())
}

#[test]
fn helpers_workflows_and_monitors_in_flight_count_with_no_limit() {
    let work = read_stop(
        &[
            ("subagent", "running"),
            ("subagent", "pending"),
            ("workflow", "running"),
            ("monitor", "running"),
            ("monitor", "running"),
        ],
        &[],
    );
    assert_eq!(
        work,
        OwnWork {
            helpers: 2,
            workflows: 1,
            monitors: 2,
            ..OwnWork::default()
        }
    );
    assert!(!work.is_nothing());
    assert!(!work.lapsed_at(at("2027-10-29T14:00:00+04:00")));
}

#[test]
fn work_not_in_flight_and_kinds_not_known_count_as_nothing() {
    let work = read_stop(
        &[
            ("subagent", "completed"),
            ("monitor", "failed"),
            ("dream", "running"),
            ("MCP task", "running"),
        ],
        &[],
    );
    assert!(work.is_nothing(), "{work:?}");
}

#[test]
fn a_background_shell_counts_for_an_hour_from_the_turn_s_end() {
    let work = read_stop(&[("shell", "running")], &[]);
    assert_eq!(work.shells, 1);
    let until = now().timestamp() + 3600;
    assert_eq!(work.until, Some(until));
    assert!(!work.lapsed_at(until - 1));
    assert!(work.lapsed_at(until));
}

#[test]
fn a_shell_beside_a_monitor_waits_with_no_limit() {
    let work = read_stop(&[("shell", "running"), ("monitor", "running")], &[]);
    assert_eq!((work.shells, work.monitors, work.until), (1, 1, None));
}

#[test]
fn a_one_shot_wake_up_counts_until_it_is_late_however_far_off() {
    // 18:30 today, four and a half hours off: past the hour, and it still counts.
    let work = read_stop(&[], &[("30 18 29 10 *", false)]);
    assert_eq!(work.wakes_at, Some(at("18:30")));
    assert_eq!(work.until, Some(at("18:35")));
    // Tomorrow morning counts too.
    let work = read_stop(&[], &[("0 9 30 10 *", false)]);
    assert_eq!(work.wakes_at, Some(at("2026-10-30T09:00:00+04:00")));
}

#[test]
fn a_recurring_cron_counts_only_when_its_next_run_is_within_the_hour() {
    // Every 20 minutes: next at 14:20.
    let soon = read_stop(&[], &[("*/20 * * * *", true)]);
    assert_eq!(soon.wakes_at, Some(at("14:20")));
    assert_eq!(soon.until, Some(at("14:25")));
    // Daily at 09:00: not within the hour, so nothing.
    let far = read_stop(&[], &[("0 9 * * *", true)]);
    assert!(far.is_nothing(), "{far:?}");
    // At 15:00 sharp: 59.5 minutes off, within it.
    let edge = read_stop(&[], &[("0 15 * * *", true)]);
    assert_eq!(edge.wakes_at, Some(at("15:00")));
}

#[test]
fn the_soonest_wake_up_is_said_and_the_latest_end_is_the_limit() {
    let work = read_stop(
        &[("shell", "running")],
        &[("45 14 * * *", true), ("10 14 29 10 *", false)],
    );
    assert_eq!(work.wakes_at, Some(at("14:10")));
    // The shell's hour (15:00:30) outlasts both wake-ups.
    assert_eq!(work.until, Some(now().timestamp() + 3600));
}

#[test]
fn a_schedule_this_cannot_read_counts_as_nothing() {
    for schedule in [
        "@hourly",
        "* * * *",
        "61 * * * *",
        "MON * * * *",
        "*/0 * * * *",
        "",
    ] {
        let work = read_stop(&[], &[(schedule, true)]);
        assert!(work.is_nothing(), "{schedule}: {work:?}");
    }
    // A cron with no `recurring` is not read either.
    let payload = serde_json::json!({"session_crons": [{"id": "c", "schedule": "*/5 * * * *"}]});
    assert!(read(Some(&payload), now()).is_nothing());
}

#[test]
fn an_unreadable_payload_waits_on_nothing() {
    assert!(read(None, now()).is_nothing());
    for payload in [
        serde_json::json!({}),
        serde_json::json!({"background_tasks": "lots", "session_crons": 3}),
        serde_json::json!({"background_tasks": [1, "x", {"type": "monitor"}]}),
    ] {
        assert!(read(Some(&payload), now()).is_nothing(), "{payload}");
    }
}

#[test]
fn days_of_the_month_and_of_the_week_match_as_cron_has_them() {
    // Weekdays at 14:30: today is a Thursday.
    assert_eq!(
        read_stop(&[], &[("30 14 * * 1-5", true)]).wakes_at,
        Some(at("14:30"))
    );
    // Sundays only (0 and 7): not within the hour.
    assert!(read_stop(&[], &[("30 14 * * 0,7", true)]).is_nothing());
    // The 1st, or a Thursday: both name days, so either matches.
    assert_eq!(
        read_stop(&[], &[("30 14 1 * 4", true)]).wakes_at,
        Some(at("14:30"))
    );
    // A one-shot on a Sunday is next Sunday, 1 November.
    assert_eq!(
        read_stop(&[], &[("0 8 * * 7", false)]).wakes_at,
        Some(at("2026-11-01T08:00:00+04:00"))
    );
}

#[test]
fn the_minute_the_turn_ended_in_is_not_a_run_still_to_come() {
    assert_eq!(
        read_stop(&[], &[("0,30 14 * * *", true)]).wakes_at,
        Some(at("14:30"))
    );
}

#[test]
fn it_reads_back_what_it_writes_and_says_nothing_of_nothing() {
    let work = read_stop(&[("monitor", "running")], &[("*/20 * * * *", true)]);
    let line = serde_json::to_string(&work).expect("a line");
    assert_eq!(serde_json::from_str::<OwnWork>(&line).expect("read"), work);
    assert_eq!(
        serde_json::to_string(&OwnWork::default()).expect("a line"),
        "{}"
    );
}
