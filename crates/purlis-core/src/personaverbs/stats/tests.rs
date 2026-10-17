// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_percentage_rounds_half_to_even_as_python_does() {
    assert_eq!(pct(1, 8), 12);
    assert_eq!(pct(3, 8), 38);
    assert_eq!(pct(1, 3), 33);
    assert_eq!(pct(2, 3), 67);
}

#[test]
fn advice_is_followed_only_by_what_came_at_or_after_it() {
    let rows: Vec<serde_json::Value> = [
        r#"{"agent": "a", "ts": "2026-03-02T09:00:00"}"#,
        r#"{"event": "advice", "ts": "2026-03-02T09:30:00"}"#,
        r#"{"agent": "b", "ts": "2026-03-02T09:30:00"}"#,
        r#"{"agent": "c", "event": "resume", "ts": "2026-03-02T10:00:00+00:00"}"#,
    ]
    .iter()
    .map(|l| serde_json::from_str(l).unwrap())
    .collect();
    let (fired, first, followed) = advice(&rows);
    assert_eq!(fired, 1);
    assert_eq!(first.unwrap().to_string(), "2026-03-02");
    assert_eq!(followed, 2);
}

#[test]
fn a_stamp_is_read_as_fromisoformat_reads_it_and_dated_in_its_own_offset() {
    let at = |raw: &str| ts(&serde_json::json!({ "ts": raw }));
    let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    assert_eq!(at("2026-03-02T09:33").unwrap().1, d("2026-03-02"));
    assert_eq!(
        at("2026-03-04T10").unwrap().0.to_string(),
        "2026-03-04 10:00:00"
    );
    assert_eq!(
        at("20260303T101010").unwrap().0.to_string(),
        "2026-03-03 10:10:10"
    );
    assert_eq!(
        at("2026-03-02").unwrap().0.to_string(),
        "2026-03-02 00:00:00"
    );
    let late = at("2026-03-01T23:30:00-05:00").unwrap();
    assert_eq!(late.0.to_string(), "2026-03-02 04:30:00");
    assert_eq!(late.1, d("2026-03-01"));
    assert_eq!(
        at("2026-03-02T09:00:00Z").unwrap().0.to_string(),
        "2026-03-02 09:00:00"
    );
    assert!(at("yesterday").is_none());
}

/// Every answer here is CPython 3.12's `datetime.fromisoformat` (3.11 agrees on every one),
/// read as `ts` reads it: the instant in UTC, or `None` where Python raises.
#[test]
fn a_time_is_refused_exactly_where_fromisoformat_refuses_it() {
    let at = |raw: &str| {
        ts(&serde_json::json!({ "ts": format!("2026-03-04T{raw}") }))
            .map(|(utc, _)| utc.format("%H:%M:%S%.6f").to_string())
    };
    let read = [
        ("10", "10:00:00.000000"),
        ("1000", "10:00:00.000000"),
        ("10:00", "10:00:00.000000"),
        ("100000", "10:00:00.000000"),
        ("10:00:00", "10:00:00.000000"),
        ("10:00:00.5", "10:00:00.500000"),
        ("10:00:00,5", "10:00:00.500000"),
        ("100000,123", "10:00:00.123000"),
        // Past six digits the fraction is cut, not rounded.
        ("10:00:00.1234567", "10:00:00.123456"),
        // A fraction may follow the hour or the minute as well as the second.
        ("10.5", "10:00:00.500000"),
        ("10:00.5", "10:00:00.500000"),
        ("1000.5", "10:00:00.500000"),
        // After the seconds, a third `:` reads what follows as the fraction.
        ("10:00:00:25", "10:00:00.250000"),
        // A separator the string ends on is Python's "not the end yet", and an offset ends it.
        ("10:+05:00", "05:00:00.000000"),
        ("10:00:00.Z", "10:00:00.000000"),
        ("10x+05:00", "05:00:00.000000"),
    ];
    for (raw, want) in read {
        assert_eq!(at(raw).as_deref(), Some(want), "{raw:?}");
    }
    let refused = [
        "1",       // a one-digit hour
        "100",     // the issue's stamp: three digits
        "10000",   // five
        "1000000", // seven
        "1:00",    // a one-digit hour before a separator
        "10:0",    // a one-digit minute
        "10:00:0", // a one-digit second
        "10:0000", // separators begun and then dropped
        "1000:00", // separators begun late
        "10:",     // a separator with nothing after it
        "10:00:",
        "10:00:00.",
        "10.",
        "10:00:00.12a",
        "10x", // a stray character with no offset after it to forgive it
        " 10",
        "24:00", // out of range: 3.12 refuses what 3.14 rolls over
        "23:60",
        "23:59:60",
        "\u{661}\u{660}:00", // digits Python's `int` reads, and `fromisoformat` does not
    ];
    for raw in refused {
        assert_eq!(at(raw), None, "{raw:?}");
    }
}

/// CPython 3.12's answers again: the character after the date is a separator whatever it
/// is, and a date's digits are exactly as many as `date.fromisoformat` wants.
#[test]
fn any_one_character_separates_the_date_from_the_time() {
    let at = |raw: &str| {
        ts(&serde_json::json!({ "ts": raw }))
            .map(|(utc, _)| utc.format("%Y-%m-%d %H:%M:%S").to_string())
    };
    assert_eq!(at("2026-03-04x10").as_deref(), Some("2026-03-04 10:00:00"));
    assert_eq!(at("2026-03-04é10").as_deref(), Some("2026-03-04 10:00:00"));
    assert_eq!(at("20260304110").as_deref(), Some("2026-03-04 10:00:00"));
    // `+` straight after the date is the separator, so `05:00` is the time, not an offset.
    assert_eq!(
        at("2026-03-04+05:00").as_deref(),
        Some("2026-03-04 05:00:00")
    );
    assert_eq!(at("2026-03-04Z"), None);
    assert_eq!(at("2026-03-04T"), None);
    assert_eq!(at("2026-3-04T10:00"), None);
    assert_eq!(at("2026-02-30"), None);
    assert_eq!(at("2026-W10-3T10:00"), None, "the week forms are not read");
}

#[test]
fn an_offset_is_read_to_the_microsecond_and_must_stay_inside_a_day() {
    let at = |raw: &str| {
        ts(&serde_json::json!({ "ts": raw }))
            .map(|(utc, _)| utc.format("%Y-%m-%d %H:%M:%S%.6f").to_string())
    };
    let utc = |raw: &str| at(&format!("2026-03-04T10:00{raw}"));
    assert_eq!(utc("+05:99").as_deref(), Some("2026-03-04 03:21:00.000000"));
    assert_eq!(
        utc("-05:00:00.25").as_deref(),
        Some("2026-03-04 15:00:00.250000")
    );
    assert_eq!(utc("+0530").as_deref(), Some("2026-03-04 04:30:00.000000"));
    // A whole-second offset of zero is UTC and its fraction is dropped (`if (tzoffset == 0)
    // return UTC`); any other keeps its fraction.
    assert_eq!(
        utc("+00:00:00.5").as_deref(),
        Some("2026-03-04 10:00:00.000000")
    );
    assert_eq!(
        utc("-00:00:00.5").as_deref(),
        Some("2026-03-04 10:00:00.000000")
    );
    assert_eq!(
        utc("+00:00:01.5").as_deref(),
        Some("2026-03-04 09:59:58.500000")
    );
    assert_eq!(utc("+24:00"), None);
    assert_eq!(utc("+5"), None);
    assert_eq!(utc("+05:"), None);
    assert_eq!(utc("Zx"), None);
}

#[test]
fn a_memory_blind_role_is_never_dormant() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("personas/router");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("persona.md"), "---\nactivity: Orchestrator\n---\n").unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 9, 24).unwrap();
    assert_eq!(row(dir.path(), "router", 14, today).status, "orchestrator");
}

mod recorded;
