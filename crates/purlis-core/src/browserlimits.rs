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

/// **The limits in force**: per project, on this device, and the idle time (`None`: a chat's
/// browser never stops for going unused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub per_project: usize,
    pub on_this_device: usize,
    pub idle_after: Option<Duration>,
}

impl Limits {
    /// **The limits on this device now**: purlis's defaults, its budget from its memory (read
    /// once for the process), and over them what the person set in Settings › This machine ›
    /// Browsers, read from this machine's store each time, so a change applies at the next
    /// browser call.
    pub fn here() -> Self {
        Limits::defaults_here().set_to(set_here())
    }

    /// purlis's defaults on this device, with nothing set: its budget from its memory, read
    /// once for the process.
    pub fn defaults_here() -> Self {
        static MEMORY: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
        Limits::for_memory(*MEMORY.get_or_init(crate::memorypressure::total))
    }

    /// The limits on a device with `total` bytes of memory, or none read, with nothing set.
    pub fn for_memory(total: Option<u64>) -> Self {
        Limits {
            per_project: PER_PROJECT,
            on_this_device: budget_for(total),
            idle_after: Some(IDLE_AFTER),
        }
    }

    /// These limits, with what the person set in place of each default they set.
    pub fn set_to(self, set: crate::machine::BrowserLimits) -> Self {
        let count = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
        Limits {
            per_project: set.per_project.map_or(self.per_project, count),
            on_this_device: set.on_this_device.map_or(self.on_this_device, count),
            idle_after: match set.idle_minutes {
                None => self.idle_after,
                Some(0) => None,
                Some(minutes) => Some(Duration::from_secs(u64::from(minutes) * 60)),
            },
        }
    }
}

/// What the person set on this machine. A fenced (test) build never reads a store outside its
/// fence: there it is nothing set, and purlis's defaults are in force.
fn set_here() -> crate::machine::BrowserLimits {
    let Some(config) = crate::machine::config_root_unheld() else {
        return crate::machine::BrowserLimits::default();
    };
    #[cfg(feature = "fenced")]
    if !crate::fence::inside(&config, &crate::fence::fence()) {
        return crate::machine::BrowserLimits::default();
    }
    crate::machine::read(&config).store.browsers
}

/// **One browser limit the person sets** (#2023): its key in `machine.json`'s `browsers` table
/// and its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    PerProject,
    OnThisDevice,
    IdleMinutes,
}

impl Limit {
    /// Its key in the `browsers` table of `machine.json`.
    pub fn key(self) -> &'static str {
        match self {
            Limit::PerProject => "per_project",
            Limit::OnThisDevice => "on_this_device",
            Limit::IdleMinutes => "idle_minutes",
        }
    }

    /// The least and the most it may be set to.
    pub fn bounds(self) -> (u32, u32) {
        match self {
            Limit::PerProject => (1, 16),
            Limit::OnThisDevice => (1, 64),
            // 0 is never; a day is the most.
            Limit::IdleMinutes => (0, 24 * 60),
        }
    }
}

/// Whether `n` is a value `limit` may be set to.
pub fn within(limit: Limit, n: u32) -> bool {
    let (least, most) = limit.bounds();
    (least..=most).contains(&n)
}

/// **Sets `limit` on this machine to `to`**, or back to purlis's default with `None` (#2023), in
/// the store under `config_root`. Refused where `to` is out of its bounds, as the next read would
/// read it as not set.
pub fn set(config_root: &std::path::Path, limit: Limit, to: Option<u32>) -> Result<(), String> {
    if let Some(n) = to
        && !within(limit, n)
    {
        let (least, most) = limit.bounds();
        return Err(format!(
            "Nothing was changed: that one is set between {least} and {most}."
        ));
    }
    crate::machine::update(config_root, |store| {
        let browsers = &mut store.browsers;
        match limit {
            Limit::PerProject => browsers.per_project = to,
            Limit::OnThisDevice => browsers.on_this_device = to,
            Limit::IdleMinutes => browsers.idle_minutes = to,
        }
    })
    .map(|_| ())
    .map_err(|why| format!("purlis could not keep it, so nothing was changed: {why}"))
}

/// Where the person sets every browser limit, as a refusal names it.
pub const SETTING: &str = "Settings › This machine › Browsers";

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
        "this project's chats have {} open, the most a project holds at once ({SETTING} sets it): \
         close one of this chat's with browser_close first, or go on without one",
        browsers(most)
    )
}

/// What a browser handed to a chat past the project's limit of `most` is refused with: the
/// person's words (#2022).
pub fn project_full_told(most: usize) -> String {
    format!(
        "This project's chats have {} open, the most a project holds at once. Close one of \
         theirs, then hand this one over, or raise the limit in {SETTING}.",
        browsers(most)
    )
}

/// What a chat's browser past the device's budget of `most` is refused with: purlis's side
/// first.
pub fn device_full_said(most: usize) -> String {
    format!(
        "purlis runs at most {} at once on this device, the budget for its memory ({SETTING} \
         sets it), and that many run now: close one of this chat's with browser_close first, or \
         go on without one",
        browsers(most)
    )
}

/// What the person's browser past the device's budget of `most` is refused with.
pub fn device_full_told(most: usize) -> String {
    format!(
        "purlis runs at most {} at once on this device, the budget for its memory, and that many \
         run now. Close one, then open this one again, or raise the budget in {SETTING}.",
        browsers(most)
    )
}

/// `n` browsers, in words.
fn browsers(n: usize) -> String {
    crate::counts::counted(n, "browser", "browsers")
}

#[cfg(test)]
mod tests;
