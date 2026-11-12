// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's slow moments, written to the slow-answer and stall log (#4725,
//! `purlis_core::slowlog`): what `app/src/slowLog.ts` sends, and reads back for a person's
//! diagnostics (#4774).

use purlis_core::slowlog::Slow;
use serde::{Deserialize, Serialize};

/// One slow moment the window felt, as it sends it. `at` is milliseconds since 1970 by the
/// window's clock, a number because `specta` exports no `u64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
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
    /// The core's main thread ran a ping `ms` after it was asked to at `at`, busy with the
    /// command `running` when the wait was noticed (`crate::mainwatch`). The core writes it; the
    /// window only reads it back.
    CoreStall {
        ms: u32,
        at: f64,
        running: Option<String>,
    },
    /// The command `command`, received on the core's main thread at `at`, held it for `ms`
    /// (`crate::mainwatch`). The core writes it; the window only reads it back.
    CoreCommand { command: String, ms: u32, at: f64 },
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
            SlowMoment::CoreStall {
                ms,
                at: when,
                running,
            } => Slow::CoreStall {
                ms,
                at: at(when),
                running,
            },
            SlowMoment::CoreCommand {
                command,
                ms,
                at: when,
            } => Slow::CoreCommand {
                command,
                ms,
                at: at(when),
            },
        }
    }
}

impl From<Slow> for SlowMoment {
    fn from(slow: Slow) -> SlowMoment {
        // A wall clock in milliseconds is far below 2^53, so it is exact as a number.
        let ms_of = |at: u64| at as f64;
        match slow {
            Slow::Answer {
                command,
                ms,
                at,
                hot,
            } => SlowMoment::Answer {
                command,
                ms,
                at: ms_of(at),
                hot,
            },
            Slow::Stall {
                ms,
                at,
                hot,
                front,
                opening,
            } => SlowMoment::Stall {
                ms,
                at: ms_of(at),
                hot,
                front,
                opening,
            },
            Slow::CoreStall { ms, at, running } => SlowMoment::CoreStall {
                ms,
                at: ms_of(at),
                running,
            },
            Slow::CoreCommand { command, ms, at } => SlowMoment::CoreCommand {
                command,
                ms,
                at: ms_of(at),
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

/// **The log's recent entries on this machine, newest last**, or none when there is no log
/// (#4774): what a person's diagnostics carry. On a blocking thread, as the write is.
#[tauri::command]
#[specta::specta]
pub async fn slow_log_recent() -> Result<Vec<SlowMoment>, String> {
    crate::off_the_window("reading the slow-answer log", || {
        Ok(purlis_core::slowlog::recent()
            .into_iter()
            .map(SlowMoment::from)
            .collect())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_read_back_from_the_log_is_the_one_the_window_sent() {
        let sent = [
            SlowMoment::Answer {
                command: "plane_sidebar".into(),
                ms: 1_250,
                at: 1_793_000_000_123.0,
                hot: 3,
            },
            SlowMoment::Stall {
                ms: 340,
                at: 1_793_000_000_456.0,
                hot: 12,
                front: Some("steward 3".into()),
                opening: true,
            },
            SlowMoment::CoreStall {
                ms: 1_250,
                at: 1_793_000_000_789.0,
                running: Some("chat_order".into()),
            },
            SlowMoment::CoreCommand {
                command: "send_input".into(),
                ms: 140,
                at: 1_793_000_000_999.0,
            },
        ];
        for one in sent {
            assert_eq!(SlowMoment::from(Slow::from(one.clone())), one);
        }
    }

    #[test]
    fn a_time_before_1970_or_not_a_number_is_kept_as_1970() {
        for at in [-5.0, f64::NAN, f64::INFINITY] {
            let slow = Slow::from(SlowMoment::Answer {
                command: "x".into(),
                ms: 1_000,
                at,
                hot: 0,
            });
            assert!(matches!(slow, Slow::Answer { at: 0, .. }), "{at}");
        }
    }
}
