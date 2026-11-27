// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's slow moments, written to the slow-answer and stall log (#4725,
//! `purlis_core::slowlog`): what `app/src/slowLog.ts` sends, and reads back for a person's
//! diagnostics (#4774).

use purlis_core::slowlog::{Counted, Rendered, Slow, Spread, Took};
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
    /// working, with the tab in front (`front`) and whether a view of a chat was opening, and
    /// what the window's thread did in that frame (felt speed round 4, H9; #7891): `render_ms`,
    /// what React's renders of the measured regions took of the gap, and the top five of each,
    /// the regions whose renders took longest by their own time (`react`), the core's events
    /// handled, each once (`events`), the calls of their listeners (`listeners`), the answers
    /// handled (`answers`) and what happened that costs a frame (`marks`), by name and count.
    /// `dark`: the thread answered all through the gap, so the webview withheld its frames (a
    /// locked screen, a display asleep, a window covered).
    Stall {
        ms: u32,
        at: f64,
        hot: u32,
        front: Option<String>,
        opening: bool,
        dark: bool,
        render_ms: u32,
        react: Vec<StallRendered>,
        events: Vec<StallCounted>,
        listeners: Vec<StallCounted>,
        answers: Vec<StallCounted>,
        marks: Vec<StallCounted>,
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
    /// A new chat's opening that took `ms`, begun at `at`, while `hot` chats were open, and the
    /// phases it passed through in the order it ran them (H12). The core writes it; the window
    /// only reads it back.
    Opening {
        ms: u32,
        at: f64,
        hot: u32,
        phases: Vec<StallTook>,
    },
    /// The window became visible (`visible`) or stopped being visible at `at`: hidden,
    /// minimised, on another Space or covered whole, as WebKit reports it (#7891).
    Visibility { at: f64, visible: bool },
    /// The core's answer to `command` of class `class`, asked at `at`: `queued_ms` waiting for
    /// a thread, then `work_ms` doing it (#7893). The core writes it; the window only reads it
    /// back.
    AnswerSpan {
        command: String,
        class: String,
        queued_ms: u32,
        work_ms: u32,
        at: f64,
    },
    /// How long the core's answers to each command took from `since` to `at` (#7893). The core
    /// writes it; the window only reads it back.
    AnswerSummary {
        at: f64,
        since: f64,
        commands: Vec<AnswerSpread>,
    },
}

/// A region of the window React rendered in a stalled frame, by name: `ms` with the regions
/// inside it, `self_ms` its own body's, and how many `commits` rendered it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct StallRendered {
    pub name: String,
    pub ms: u32,
    pub self_ms: u32,
    pub commits: u32,
}

/// How long `n` answers to `command` of class `class` took: the median and 99th percentile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct AnswerSpread {
    pub command: String,
    pub class: String,
    pub n: u32,
    pub p50_ms: u32,
    pub p99_ms: u32,
}

/// A phase of an opening, by name, and how long it took.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct StallTook {
    pub name: String,
    pub ms: u32,
}

/// Something the window handled or did `n` times in a stalled frame, by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct StallCounted {
    pub name: String,
    pub n: u32,
}

impl From<StallTook> for Took {
    fn from(StallTook { name, ms }: StallTook) -> Took {
        Took { name, ms }
    }
}

impl From<Took> for StallTook {
    fn from(Took { name, ms }: Took) -> StallTook {
        StallTook { name, ms }
    }
}

impl From<StallCounted> for Counted {
    fn from(StallCounted { name, n }: StallCounted) -> Counted {
        Counted { name, n }
    }
}

impl From<Counted> for StallCounted {
    fn from(Counted { name, n }: Counted) -> StallCounted {
        StallCounted { name, n }
    }
}

impl From<StallRendered> for Rendered {
    fn from(
        StallRendered {
            name,
            ms,
            self_ms,
            commits,
        }: StallRendered,
    ) -> Rendered {
        Rendered {
            name,
            ms,
            self_ms,
            commits,
        }
    }
}

impl From<Rendered> for StallRendered {
    fn from(
        Rendered {
            name,
            ms,
            self_ms,
            commits,
        }: Rendered,
    ) -> StallRendered {
        StallRendered {
            name,
            ms,
            self_ms,
            commits,
        }
    }
}

impl From<AnswerSpread> for Spread {
    fn from(
        AnswerSpread {
            command,
            class,
            n,
            p50_ms,
            p99_ms,
        }: AnswerSpread,
    ) -> Spread {
        Spread {
            command,
            class,
            n,
            p50_ms,
            p99_ms,
        }
    }
}

impl From<Spread> for AnswerSpread {
    fn from(
        Spread {
            command,
            class,
            n,
            p50_ms,
            p99_ms,
        }: Spread,
    ) -> AnswerSpread {
        AnswerSpread {
            command,
            class,
            n,
            p50_ms,
            p99_ms,
        }
    }
}

/// Each of `all`, as the other side keeps it.
fn each<A, B: From<A>>(all: Vec<A>) -> Vec<B> {
    all.into_iter().map(B::from).collect()
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
                phases: Vec::new(),
            },
            SlowMoment::Stall {
                ms,
                at: when,
                hot,
                front,
                opening,
                dark,
                render_ms,
                react,
                events,
                listeners,
                answers,
                marks,
            } => Slow::Stall {
                ms,
                at: at(when),
                hot,
                front,
                opening,
                dark,
                render_ms,
                react: each(react),
                events: each(events),
                listeners: each(listeners),
                answers: each(answers),
                marks: each(marks),
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
            SlowMoment::Opening {
                ms,
                at: when,
                hot,
                phases,
            } => Slow::Opening {
                ms,
                at: at(when),
                hot,
                phases: each(phases),
            },
            SlowMoment::Visibility { at: when, visible } => Slow::Visibility {
                at: at(when),
                visible,
            },
            SlowMoment::AnswerSpan {
                command,
                class,
                queued_ms,
                work_ms,
                at: when,
            } => Slow::AnswerSpan {
                command,
                class,
                queued_ms,
                work_ms,
                at: at(when),
            },
            SlowMoment::AnswerSummary {
                at: when,
                since,
                commands,
            } => Slow::AnswerSummary {
                at: at(when),
                since: at(since),
                commands: each(commands),
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
                ..
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
                dark,
                render_ms,
                react,
                events,
                listeners,
                answers,
                marks,
            } => SlowMoment::Stall {
                ms,
                at: ms_of(at),
                hot,
                front,
                opening,
                dark,
                render_ms,
                react: each(react),
                events: each(events),
                listeners: each(listeners),
                answers: each(answers),
                marks: each(marks),
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
            Slow::Opening {
                ms,
                at,
                hot,
                phases,
            } => SlowMoment::Opening {
                ms,
                at: ms_of(at),
                hot,
                phases: each(phases),
            },
            Slow::Visibility { at, visible } => SlowMoment::Visibility {
                at: ms_of(at),
                visible,
            },
            Slow::AnswerSpan {
                command,
                class,
                queued_ms,
                work_ms,
                at,
            } => SlowMoment::AnswerSpan {
                command,
                class,
                queued_ms,
                work_ms,
                at: ms_of(at),
            },
            Slow::AnswerSummary {
                at,
                since,
                commands,
            } => SlowMoment::AnswerSummary {
                at: ms_of(at),
                since: ms_of(since),
                commands: each(commands),
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
                dark: false,
                render_ms: 230,
                react: vec![StallRendered {
                    name: "PlaneView".into(),
                    ms: 230,
                    self_ms: 20,
                    commits: 3,
                }],
                events: vec![StallCounted {
                    name: "plane-changed".into(),
                    n: 1,
                }],
                listeners: vec![StallCounted {
                    name: "plane-changed".into(),
                    n: 40,
                }],
                answers: vec![StallCounted {
                    name: "alertsEverywhere".into(),
                    n: 1,
                }],
                marks: vec![StallCounted {
                    name: "pane-mount".into(),
                    n: 2,
                }],
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
            SlowMoment::Opening {
                ms: 4_300,
                at: 1_793_000_001_000.0,
                hot: 12,
                phases: vec![StallTook {
                    name: "branch_cut".into(),
                    ms: 900,
                }],
            },
            SlowMoment::Visibility {
                at: 1_793_000_001_111.0,
                visible: false,
            },
            SlowMoment::AnswerSpan {
                command: "sandbox_state".into(),
                class: "git".into(),
                queued_ms: 40,
                work_ms: 832,
                at: 1_793_000_001_222.0,
            },
            SlowMoment::AnswerSummary {
                at: 1_793_003_600_000.0,
                since: 1_793_000_000_000.0,
                commands: vec![AnswerSpread {
                    command: "sandbox_state".into(),
                    class: "git".into(),
                    n: 133,
                    p50_ms: 872,
                    p99_ms: 2_400,
                }],
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
