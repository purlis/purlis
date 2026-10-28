// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A duration as the files spell it** (#3471): a whole number above zero followed by one
//! unit, `s`, `m` or `h` (`"30s"`, `"2m"`, `"24h"`), as `[plane].autosave_after` first spelled
//! it (ADR 0051). Each key says which units it takes: `autosave_after` seconds and minutes, a
//! pool's `suspendAfter` hours too. No sign, no fraction, no space and no second unit.

use std::time::Duration;

/// One unit a spelled duration may end in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Seconds,
    Minutes,
    Hours,
}

impl Unit {
    fn suffix(self) -> char {
        match self {
            Unit::Seconds => 's',
            Unit::Minutes => 'm',
            Unit::Hours => 'h',
        }
    }

    fn seconds(self) -> u64 {
        match self {
            Unit::Seconds => 1,
            Unit::Minutes => 60,
            Unit::Hours => 60 * 60,
        }
    }
}

/// Seconds and minutes: `autosave_after`'s units.
pub const SECONDS_OR_MINUTES: &[Unit] = &[Unit::Seconds, Unit::Minutes];

/// Seconds, minutes and hours: a pool's `suspendAfter`'s units.
pub const UP_TO_HOURS: &[Unit] = &[Unit::Seconds, Unit::Minutes, Unit::Hours];

/// `text` read as a duration in one of `units`; `None` for anything else.
pub fn read(text: &str, units: &[Unit]) -> Option<Duration> {
    let (unit, digits) = units
        .iter()
        .find_map(|unit| Some((*unit, text.strip_suffix(unit.suffix())?)))?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u64 = digits.parse().ok().filter(|n| *n > 0)?;
    Some(Duration::from_secs(n.checked_mul(unit.seconds())?))
}

/// `duration` spelled in the largest of `units` it is a whole number of, which [`read`] reads
/// back to it; `None` for a duration of no whole number of any of them, or of none.
pub fn spelled(duration: Duration, units: &[Unit]) -> Option<String> {
    if duration.subsec_nanos() != 0 || duration.is_zero() {
        return None;
    }
    let secs = duration.as_secs();
    units
        .iter()
        .copied()
        .filter(|unit| secs % unit.seconds() == 0)
        .max_by_key(|unit| unit.seconds())
        .map(|unit| format!("{}{}", secs / unit.seconds(), unit.suffix()))
}

#[cfg(test)]
mod tests;
