// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › This machine › Browsers** (#2023, ruled 2026-10-18): the most browsers a
//! project's chats hold, the most this device runs, and how long a chat's browser may go
//! unused, each kept in this machine's store (`machine.json`'s `browsers` table) and read at the
//! next browser call. Every rule is the core's ([`purlis_core::browserlimits`]); this says what
//! is in force and writes what the person chose. The window's alone (`ui::WINDOW_ONLY`): only
//! the person sets their machine's limits.

use purlis_core::browserlimits::{self as limits, Limit, Limits};

/// One limit, as its row draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserLimitShown {
    /// What is in force now.
    pub value: u32,
    /// What purlis's default is here: for the device's budget, what its memory gives.
    pub default: u32,
    /// Whether the person set it, so a reset is on offer.
    pub set: bool,
    pub least: u32,
    pub most: u32,
}

/// What the Browsers group draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserLimitsShown {
    pub per_project: BrowserLimitShown,
    pub on_this_device: BrowserLimitShown,
    /// `0` is never.
    pub idle_minutes: BrowserLimitShown,
}

/// Which limit a row sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BrowserLimit {
    PerProject,
    OnThisDevice,
    IdleMinutes,
}

impl From<BrowserLimit> for Limit {
    fn from(limit: BrowserLimit) -> Self {
        match limit {
            BrowserLimit::PerProject => Limit::PerProject,
            BrowserLimit::OnThisDevice => Limit::OnThisDevice,
            BrowserLimit::IdleMinutes => Limit::IdleMinutes,
        }
    }
}

fn minutes(limits: &Limits) -> u32 {
    limits.idle_after.map_or(0, |after| {
        u32::try_from(after.as_secs().div_ceil(60)).unwrap_or(u32::MAX)
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// What the group shows, from what is set (`set`) over `defaults`.
pub fn shown(defaults: Limits, set: purlis_core::machine::BrowserLimits) -> BrowserLimitsShown {
    let now = defaults.set_to(set);
    let one = |limit: Limit, value: u32, default: u32, set: Option<u32>| {
        let (least, most) = limit.bounds();
        BrowserLimitShown {
            value,
            default,
            set: set.is_some(),
            least,
            most,
        }
    };
    BrowserLimitsShown {
        per_project: one(
            Limit::PerProject,
            count(now.per_project),
            count(defaults.per_project),
            set.per_project,
        ),
        on_this_device: one(
            Limit::OnThisDevice,
            count(now.on_this_device),
            count(defaults.on_this_device),
            set.on_this_device,
        ),
        idle_minutes: one(
            Limit::IdleMinutes,
            minutes(&now),
            minutes(&defaults),
            set.idle_minutes,
        ),
    }
}

fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep it in.".to_owned())
}

/// **The browser limits in force on this machine**, and their defaults here.
#[tauri::command]
#[specta::specta]
pub async fn browser_limits() -> Result<BrowserLimitsShown, String> {
    crate::off_the_window("reading the browser limits", || {
        let root = config_root()?;
        let set = purlis_core::machine::read(&root).store.browsers;
        Ok(shown(Limits::defaults_here(), set))
    })
    .await
}

/// **Sets one browser limit on this machine**, or with no `to`, puts it back to purlis's
/// default. It applies at the next browser call; a browser running past a lowered limit runs on.
#[tauri::command]
#[specta::specta]
pub async fn set_browser_limit(limit: BrowserLimit, to: Option<u32>) -> Result<(), String> {
    crate::off_the_window("setting a browser limit", move || {
        limits::set(&config_root()?, limit.into(), to)
    })
    .await
}

#[cfg(test)]
mod tests;
