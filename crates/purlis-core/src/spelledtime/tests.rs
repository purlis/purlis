// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use super::*;

#[test]
fn a_whole_number_and_one_unit_is_a_duration() {
    assert_eq!(read("30s", UP_TO_HOURS), Some(Duration::from_secs(30)));
    assert_eq!(read("90m", UP_TO_HOURS), Some(Duration::from_secs(5400)));
    assert_eq!(read("24h", UP_TO_HOURS), Some(Duration::from_secs(86_400)));
}

#[test]
fn a_unit_the_key_does_not_take_is_not_read() {
    assert_eq!(read("2h", SECONDS_OR_MINUTES), None);
    assert_eq!(
        read("2m", SECONDS_OR_MINUTES),
        Some(Duration::from_secs(120))
    );
}

#[test]
fn anything_but_a_whole_number_above_zero_and_one_unit_is_not_read() {
    for text in [
        "",
        "h",
        "0h",
        "-1h",
        "+1h",
        "1.5h",
        " 1h",
        "1h ",
        "1 h",
        "1h30m",
        "1d",
        "1H",
        "24",
        "١h",
        "99999999999999999999h",
    ] {
        assert_eq!(read(text, UP_TO_HOURS), None, "{text:?}");
    }
}

#[test]
fn a_duration_is_spelled_in_its_largest_whole_unit() {
    assert_eq!(
        spelled(Duration::from_secs(86_400), UP_TO_HOURS).as_deref(),
        Some("24h")
    );
    assert_eq!(
        spelled(Duration::from_secs(5400), UP_TO_HOURS).as_deref(),
        Some("90m")
    );
    assert_eq!(
        spelled(Duration::from_secs(61), UP_TO_HOURS).as_deref(),
        Some("61s")
    );
    assert_eq!(
        spelled(Duration::from_secs(7200), SECONDS_OR_MINUTES).as_deref(),
        Some("120m")
    );
    assert_eq!(spelled(Duration::ZERO, UP_TO_HOURS), None);
    assert_eq!(spelled(Duration::from_millis(1500), UP_TO_HOURS), None);
}
