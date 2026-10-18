// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How many browsers run, and how long one may sit idle** (#2023, spec #2008's Limits): at
//! most three for the chats of one project, a budget for the whole device that follows its
//! memory class, and ten idle minutes before a chat's browser stops.

use std::time::Duration;

/// The most browsers the chats of one project hold at once.
pub const PER_PROJECT: usize = 3;

/// How long a chat's browser may go unused before it stops.
pub const IDLE_AFTER: Duration = Duration::from_secs(10 * 60);

const GIB: u64 = 1024 * 1024 * 1024;

/// The memory, in GiB, a browser is budgeted: one per four (16 GB: 4, 32 GB: 8, 64 GB: 16).
const GIB_PER_BROWSER: u64 = 4;

/// The class a device is budgeted by where its memory could not be read: the smallest the
/// rulings name, so an unread device is never given more than that.
const UNREAD_CLASS_GIB: u64 = 16;

/// **The limits in force**: per project, on this device, and the idle time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub per_project: usize,
    pub on_this_device: usize,
    pub idle_after: Duration,
}

impl Limits {
    /// The limits on this device: its budget read from its memory, once for the process.
    pub fn here() -> Self {
        static HERE: std::sync::OnceLock<Limits> = std::sync::OnceLock::new();
        *HERE.get_or_init(|| Limits::for_memory(crate::memorypressure::total()))
    }

    /// The limits on a device with `total` bytes of memory, or none read.
    pub fn for_memory(total: Option<u64>) -> Self {
        Limits {
            per_project: PER_PROJECT,
            on_this_device: budget_for(total),
            idle_after: IDLE_AFTER,
        }
    }
}

/// **A device's memory class**, in GiB: the largest power of two its memory reaches, with a
/// tenth to spare, since an operating system reports a little less than is fitted (a 16 GB
/// machine's Linux says about 15.6 GiB). [`UNREAD_CLASS_GIB`] where it could not be read.
pub fn class_gib(total: Option<u64>) -> u64 {
    let Some(total) = total else {
        return UNREAD_CLASS_GIB;
    };
    let reach = total.saturating_add(total / 10) / GIB;
    if reach == 0 {
        return 1;
    }
    1 << reach.ilog2()
}

/// **The device's browser budget**: one browser for each [`GIB_PER_BROWSER`] of its memory
/// class, and never none.
pub fn budget_for(total: Option<u64>) -> usize {
    usize::try_from((class_gib(total) / GIB_PER_BROWSER).max(1)).unwrap_or(usize::MAX)
}

/// What a chat's browser past the project's limit of `most` is refused with.
pub fn project_full_said(most: usize) -> String {
    format!(
        "this project's chats have {} open, the most a project holds at once: close \
         one of this chat's with browser_close first, or go on without one",
        browsers(most)
    )
}

/// What a chat's browser past the device's budget of `most` is refused with.
pub fn device_full_said(most: usize) -> String {
    format!(
        "this device runs {} already, the most its memory is budgeted for: close one \
         of this chat's with browser_close first, or go on without one",
        browsers(most)
    )
}

/// What the person's browser past the device's budget of `most` is refused with.
pub fn device_full_told(most: usize) -> String {
    format!(
        "This device runs {} already, the most its memory is budgeted for. Close one, then \
         open this one again.",
        browsers(most)
    )
}

/// `n` browsers, in words.
fn browsers(n: usize) -> String {
    if n == 1 {
        "1 browser".to_owned()
    } else {
        format!("{n} browsers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gib(n: f64) -> Option<u64> {
        Some((n * GIB as f64) as u64)
    }

    #[test]
    fn the_budget_follows_the_ruled_memory_classes() {
        assert_eq!(budget_for(gib(16.0)), 4);
        assert_eq!(budget_for(gib(32.0)), 8);
        assert_eq!(budget_for(gib(64.0)), 16);
    }

    #[test]
    fn a_device_reporting_a_little_less_than_fitted_keeps_its_class() {
        assert_eq!(budget_for(gib(15.6)), 4);
        assert_eq!(budget_for(gib(31.2)), 8);
    }

    #[test]
    fn a_device_between_classes_is_budgeted_by_the_class_beneath() {
        assert_eq!(budget_for(gib(24.0)), 4);
        assert_eq!(budget_for(gib(48.0)), 8);
        assert_eq!(budget_for(gib(128.0)), 32);
    }

    #[test]
    fn a_small_device_has_a_small_budget_and_never_none() {
        assert_eq!(budget_for(gib(8.0)), 2);
        assert_eq!(budget_for(gib(4.0)), 1);
        assert_eq!(budget_for(gib(2.0)), 1);
        assert_eq!(budget_for(Some(0)), 1);
    }

    #[test]
    fn a_device_whose_memory_could_not_be_read_gets_the_smallest_ruled_budget() {
        assert_eq!(budget_for(None), 4);
    }

    #[test]
    fn the_limits_are_three_a_project_and_ten_idle_minutes() {
        let limits = Limits::for_memory(gib(32.0));
        assert_eq!(limits.per_project, 3);
        assert_eq!(limits.on_this_device, 8);
        assert_eq!(limits.idle_after, Duration::from_secs(600));
    }
}
