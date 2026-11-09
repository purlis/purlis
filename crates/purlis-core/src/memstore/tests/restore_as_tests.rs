// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn now() -> chrono::NaiveDateTime {
    "2026-03-02T09:14:00".parse().unwrap()
}

fn asked(restore_as: &str, timestamped: bool) -> std::io::Result<String> {
    moved_as(
        "a-fact.md",
        "# A fact\n",
        timestamped,
        now(),
        Some(restore_as),
    )
}

#[test]
fn a_stamp_that_is_a_real_date_and_time_is_kept_to_the_second() {
    assert_eq!(
        asked("20260302-091437-a-fact", true).unwrap(),
        "20260302-091437-a-fact.md"
    );
    assert_eq!(
        asked("20240229-235959-a-fact", true).unwrap(),
        "20240229-235959-a-fact.md"
    );
    assert_eq!(asked("a-fact", false).unwrap(), "a-fact.md");
}

#[test]
fn a_stamp_of_the_right_shape_that_is_no_date_and_time_is_refused() {
    for name in [
        "99999999-999999-a-fact",
        "20261302-091437-a-fact",
        "20260230-091437-a-fact",
        "20250229-091437-a-fact",
        "20260302-240000-a-fact",
        "20260302-096037-a-fact",
        "20260302-091460-a-fact",
    ] {
        let refused = asked(name, true).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput, "{name}");
    }
}

#[test]
fn names_already_on_disk_are_still_read_by_shape() {
    // Reading is not where a stamp is checked: a journal name of any digits keeps its name.
    assert_eq!(unstamped("99999999-999999-a-fact.md"), "a-fact.md");
}
