// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The two time spellings chrono wrote that jiff spells otherwise** (#7089), kept so a record
//! written after the move to jiff has the bytes one written before it had.
//!
//! Every other time the plane or the wire keeps is a `strftime` pattern, which jiff writes as
//! chrono did. These two are not: chrono's `to_rfc3339` ends a UTC time in `+00:00` and gives a
//! fraction 3, 6 or 9 digits long, where jiff ends it in `Z` with the fewest digits; and
//! chrono's `Display` of a date and time puts a space between them, where jiff puts a `T`.

use jiff::Timestamp;
use jiff::civil::DateTime;

/// `at` as chrono's `DateTime<Utc>::to_rfc3339` spelled it: `2026-11-17T19:47:05+00:00`, with
/// a fraction of 3, 6 or 9 digits when the time has one (`.120`, `.123456`, `.000000500`).
#[must_use]
pub fn rfc3339(at: Timestamp) -> String {
    format!(
        "{}{}+00:00",
        at.strftime("%Y-%m-%dT%H:%M:%S"),
        fraction(at.subsec_nanosecond())
    )
}

/// `at` as chrono's `Display` of a `NaiveDateTime` spelled it: `2026-11-17 19:47:05`, with a
/// fraction of 3, 6 or 9 digits when the time has one.
#[must_use]
pub fn date_and_time(at: DateTime) -> String {
    format!(
        "{}{}",
        at.strftime("%Y-%m-%d %H:%M:%S"),
        fraction(at.subsec_nanosecond())
    )
}

/// chrono's `AutoSi` fraction: none for a whole second, else the fewest of 3, 6 or 9 digits
/// that hold it.
fn fraction(nanos: i32) -> String {
    if nanos == 0 {
        String::new()
    } else if nanos % 1_000_000 == 0 {
        format!(".{:03}", nanos / 1_000_000)
    } else if nanos % 1_000 == 0 {
        format!(".{:06}", nanos / 1_000)
    } else {
        format!(".{nanos:09}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each expected spelling is what chrono 0.4.45 wrote for the same time.

    #[test]
    fn a_utc_time_ends_in_a_numeric_offset_with_a_fraction_of_three_six_or_nine_digits() {
        let spelt = |text: &str| rfc3339(text.parse().unwrap());
        assert_eq!(spelt("2023-11-14T22:13:20Z"), "2023-11-14T22:13:20+00:00");
        assert_eq!(
            spelt("2023-11-14T22:13:20.12Z"),
            "2023-11-14T22:13:20.120+00:00"
        );
        assert_eq!(
            spelt("2023-11-14T22:13:20.123456Z"),
            "2023-11-14T22:13:20.123456+00:00"
        );
        assert_eq!(
            spelt("2023-11-14T22:13:20.1234567Z"),
            "2023-11-14T22:13:20.123456700+00:00"
        );
        assert_eq!(
            spelt("2023-11-14T22:13:20.0000005Z"),
            "2023-11-14T22:13:20.000000500+00:00"
        );
    }

    #[test]
    fn a_date_and_time_is_spelt_with_a_space_and_the_same_fraction() {
        let spelt = |text: &str| date_and_time(text.parse().unwrap());
        assert_eq!(spelt("2026-03-04T10:00:00"), "2026-03-04 10:00:00");
        assert_eq!(spelt("2026-03-04T10:00:00.5"), "2026-03-04 10:00:00.500");
        assert_eq!(
            spelt("2026-03-04T10:00:00.000001"),
            "2026-03-04 10:00:00.000001"
        );
    }
}
