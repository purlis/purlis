// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's slow moments, written to the slow-answer and stall log (#4725,
//! `purlis_core::slowlog`): what `app/src/slowLog.ts` sends.

use purlis_core::slowlog::Slow;
use serde::Deserialize;

/// One slow moment the window felt, as it sends it. `at` is milliseconds since 1970 by the
/// window's clock, a number because `specta` exports no `u64`.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlowMoment {
    /// An answer to `command` that took `ms`, asked at `at`, while `hot` chats were working.
    Answer {
        command: String,
        ms: u32,
        at: f64,
        hot: u32,
    },
    /// A gap of `ms` between two of the window's frames, seen at `at`, while `hot` chats were
    /// working, with the tab in front (`front`) and whether a view of a chat was opening.
    Stall {
        ms: u32,
        at: f64,
        hot: u32,
        front: Option<String>,
        opening: bool,
    },
}

/// Milliseconds since 1970, as the core keeps them: a time before then, or not a number, is 0.
fn at(ms: f64) -> u64 {
    if ms.is_finite() && ms > 0.0 {
        // In range: a wall clock in milliseconds is far below 2^53.
        ms.round() as u64
    } else {
        0
    }
}

impl From<SlowMoment> for Slow {
    fn from(moment: SlowMoment) -> Slow {
        match moment {
            SlowMoment::Answer {
                command,
                ms,
                at: when,
                hot,
            } => Slow::Answer {
                command,
                ms,
                at: at(when),
                hot,
            },
            SlowMoment::Stall {
                ms,
                at: when,
                hot,
                front,
                opening,
            } => Slow::Stall {
                ms,
                at: at(when),
                hot,
                front,
                opening,
            },
        }
    }
}

/// **Writes one slow moment to the log**, on a blocking thread: a file is not the window's to
/// wait for, least of all while it is slow.
#[tauri::command]
#[specta::specta]
pub async fn record_slow(moment: SlowMoment) -> Result<(), String> {
    crate::off_the_window("writing the slow-answer log", move || {
        purlis_core::slowlog::record(&moment.into()).map_err(|err| err.to_string())
    })
    .await
}
