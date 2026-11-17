// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A reading of a wall clock as an instant**, as chrono read one (#7089).
//!
//! jiff settles a reading a clock change makes ambiguous or skips by itself; chrono gave none
//! for a reading the clock skipped, and what was written then does not move now. These say
//! which reading is meant, and give nothing where chrono gave nothing.

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::{AmbiguousOffset, TimeZone};

/// `at` on `zone`'s clock when it names exactly one instant; `None` for a reading a clock
/// change repeats or skips (chrono's `single`).
#[must_use]
pub fn single(zone: &TimeZone, at: DateTime) -> Option<Timestamp> {
    match zone.to_ambiguous_timestamp(at).offset() {
        AmbiguousOffset::Unambiguous { offset } => offset.to_timestamp(at).ok(),
        AmbiguousOffset::Gap { .. } | AmbiguousOffset::Fold { .. } => None,
    }
}

/// `at` on `zone`'s clock, the earlier of two where a clock change repeats it; `None` for a
/// reading a clock change skips (chrono's `earliest`).
#[must_use]
pub fn earliest(zone: &TimeZone, at: DateTime) -> Option<Timestamp> {
    match zone.to_ambiguous_timestamp(at).offset() {
        AmbiguousOffset::Unambiguous { offset } | AmbiguousOffset::Fold { before: offset, .. } => {
            offset.to_timestamp(at).ok()
        }
        AmbiguousOffset::Gap { .. } => None,
    }
}

/// `at` as an instant jiff holds: the first or last it holds for a time beyond the years
/// -9999 to 9999 (chrono, given one, panicked).
#[must_use]
pub fn instant(at: std::time::SystemTime) -> Timestamp {
    Timestamp::try_from(at).unwrap_or_else(|_| {
        if at < std::time::SystemTime::UNIX_EPOCH {
            Timestamp::MIN
        } else {
            Timestamp::MAX
        }
    })
}

/// The day `at` falls on in UTC.
#[must_use]
pub fn utc_day(at: Timestamp) -> Date {
    at.to_zoned(TimeZone::UTC).date()
}

/// The day `year`-`month`-`day`, or `None` for one no calendar has (chrono's `from_ymd_opt`,
/// within the years jiff holds, -9999 to 9999).
#[must_use]
pub fn day(year: i32, month: u32, day: u32) -> Option<Date> {
    Date::new(
        i16::try_from(year).ok()?,
        i8::try_from(month).ok()?,
        i8::try_from(day).ok()?,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Central Europe: 2026-03-29 02:30 is skipped, 2026-10-25 02:30 comes twice.
    fn berlin() -> TimeZone {
        TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").unwrap()
    }

    fn at(text: &str) -> DateTime {
        text.parse().unwrap()
    }

    #[test]
    fn a_reading_the_clock_shows_once_is_one_instant_either_way() {
        let noon = at("2026-06-01T12:00");
        let instant: Timestamp = "2026-06-01T10:00:00Z".parse().unwrap();
        assert_eq!(single(&berlin(), noon), Some(instant));
        assert_eq!(earliest(&berlin(), noon), Some(instant));
    }

    #[test]
    fn a_reading_the_clock_skips_is_no_instant() {
        let skipped = at("2026-03-29T02:30");
        assert_eq!(single(&berlin(), skipped), None);
        assert_eq!(earliest(&berlin(), skipped), None);
    }

    #[test]
    fn a_reading_the_clock_shows_twice_is_the_earlier_or_none() {
        let twice = at("2026-10-25T02:30");
        assert_eq!(single(&berlin(), twice), None);
        assert_eq!(
            earliest(&berlin(), twice),
            Some("2026-10-25T00:30:00Z".parse().unwrap())
        );
    }

    #[test]
    fn a_day_no_calendar_has_is_none() {
        assert_eq!(day(2026, 2, 29), None);
        assert_eq!(day(2028, 2, 29), Some(Date::constant(2028, 2, 29)));
        assert_eq!(day(2026, 13, 1), None);
        assert_eq!(day(70_000, 1, 1), None);
    }
}
