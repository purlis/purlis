// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// 14:00:30 on 2026-10-29, a Thursday, on the chat's own wall clock.
fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-10-29 14:00:30", "%Y-%m-%d %H:%M:%S").expect("a time")
}

/// Seconds from [`now`] to `when` (`HH:MM` today, or `YYYY-MM-DD HH:MM`).
fn in_secs(when: &str) -> u32 {
    let full = if when.len() == 5 {
        format!("2026-10-29 {when}")
    } else {
        when.to_owned()
    };
    let at =
        NaiveDateTime::parse_from_str(&format!("{full}:00"), "%Y-%m-%d %H:%M:%S").expect("a time");
    u32::try_from((at - now()).num_seconds()).expect("ahead")
}

const LATE: u32 = 300;

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

fn read_stop(tasks: &[(&str, &str)], crons: &[(&str, bool)]) -> Said {
    read(Some(&stop(tasks, crons)), now())
}

#[test]
fn each_kind_in_flight_is_counted_by_claude_code_s_label() {
    let said = read_stop(
        &[
            ("subagent", "running"),
            ("subagent", "pending"),
            ("workflow", "running"),
            ("monitor", "running"),
            ("shell", "running"),
        ],
        &[],
    );
    assert_eq!(
        (said.helpers, said.workflows, said.monitors, said.shells),
        (2, 1, 1, 1)
    );
}

#[test]
fn work_not_in_flight_and_kinds_not_known_count_as_nothing() {
    let said = read_stop(
        &[
            ("subagent", "completed"),
            ("monitor", "failed"),
            ("dream", "running"),
            ("MCP task", "running"),
        ],
        &[],
    );
    assert!(said.is_nothing(), "{said:?}");
}

#[test]
fn a_one_shot_wake_up_is_read_as_seconds_from_now_however_far_off() {
    let said = read_stop(&[], &[("30 18 29 10 *", false)]);
    assert_eq!(said.wakes_in, Some(in_secs("18:30")));
    assert_eq!(said.lapses_in, Some(in_secs("18:30") + LATE));
    let said = read_stop(&[], &[("0 9 30 10 *", false)]);
    assert_eq!(said.wakes_in, Some(in_secs("2026-10-30 09:00")));
}

#[test]
fn a_one_shot_wake_up_due_this_minute_or_overdue_is_due_now_not_next_year() {
    for schedule in ["0 14 29 10 *", "45 13 29 10 *", "30 14 28 10 *"] {
        let said = read_stop(&[], &[(schedule, false)]);
        assert_eq!(said.wakes_in, Some(0), "{schedule}");
        assert_eq!(said.lapses_in, Some(LATE), "{schedule}");
    }
}

#[test]
fn a_recurring_cron_counts_only_when_its_next_run_is_within_the_hour() {
    let soon = read_stop(&[], &[("*/20 * * * *", true)]);
    assert_eq!(soon.wakes_in, Some(in_secs("14:20")));
    assert!(read_stop(&[], &[("0 9 * * *", true)]).is_nothing());
    assert_eq!(
        read_stop(&[], &[("0 15 * * *", true)]).wakes_in,
        Some(in_secs("15:00"))
    );
    // A recurring run in the minute the turn ended in has fired already.
    assert_eq!(
        read_stop(&[], &[("0,30 14 * * *", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
}

#[test]
fn days_of_the_month_and_of_the_week_match_as_cron_has_them() {
    assert_eq!(
        read_stop(&[], &[("30 14 * * 1-5", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
    assert!(read_stop(&[], &[("30 14 * * 0,7", true)]).is_nothing());
    assert_eq!(
        read_stop(&[], &[("30 14 1 * 4", true)]).wakes_in,
        Some(in_secs("14:30"))
    );
    assert_eq!(
        read_stop(&[], &[("0 8 * * 7", false)]).wakes_in,
        Some(in_secs("2026-11-01 08:00"))
    );
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
        let said = read_stop(&[], &[(schedule, true)]);
        assert!(said.is_nothing(), "{schedule}: {said:?}");
    }
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
fn what_it_reads_is_seconds_from_now_on_the_chat_s_clock_whatever_the_day() {
    // The same schedule read at the same wall time on any day says the same: no time zone and
    // no time of day leave the chat.
    let later = now() + Duration::days(40);
    let one = read(Some(&stop(&[], &[("*/20 * * * *", true)])), now());
    let other = read(Some(&stop(&[], &[("*/20 * * * *", true)])), later);
    assert_eq!(one, other);
}
