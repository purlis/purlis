// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Settings › Diagnostics' **Detailed remote log for one hour** (#7864): the window's two
//! commands over `purlis_core::detailedlog`'s switch, and the timer that turns it off when its
//! hour is up whether or not a window asks.

use purlis_core::detailedlog::{self, Detailed, LASTS};

/// What the switch says, as the window reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DetailedRemoteLog {
    /// When it turns itself off, in RFC 3339, while it is on; `null` while it is off.
    pub until: Option<String>,
}

impl From<Detailed> for DetailedRemoteLog {
    fn from(said: Detailed) -> Self {
        Self {
            until: said.until.map(|until| until.to_string()),
        }
    }
}

/// Whether the detailed remote log is on, and until when.
#[tauri::command]
#[specta::specta]
pub fn detailed_remote_log() -> DetailedRemoteLog {
    detailedlog::this_process()
        .at(jiff::Timestamp::now())
        .into()
}

/// Turns the detailed remote log on, for an hour from now, or off now.
#[tauri::command]
#[specta::specta]
pub fn set_detailed_remote_log(on: bool) -> DetailedRemoteLog {
    let switch = detailedlog::this_process();
    if !on {
        return switch.turn_off().into();
    }
    let said = switch.turn_on(jiff::Timestamp::now());
    // Asked once its hour is up, it turns itself off. A timer of an earlier turn-on that wakes
    // first finds the hour not up yet and changes nothing.
    let lasts = std::time::Duration::try_from(LASTS).unwrap_or_default();
    let _ = std::thread::Builder::new()
        .name("purlis-detailed-log".into())
        .spawn(move || {
            std::thread::sleep(lasts + std::time::Duration::from_secs(1));
            let _ = detailedlog::this_process().at(jiff::Timestamp::now());
        });
    said.into()
}
