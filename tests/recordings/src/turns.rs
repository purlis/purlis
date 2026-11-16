// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The turn script of a recording** (T1.1-03 item 4): what the fake harness (ticket 4,
//! `fake-harness --script`) is given to reproduce the recording, written beside it as
//! `turns.json`. Each turn is its prompt and the calls the model asked for, in order, as the
//! harness names its tools: `{tool, input, run?, parallel?, background?, subagent?}`. `run` is
//! the command a shell call runs for real; a native call's outcome is never stated, since the
//! sandbox decides it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::recording::Recording;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnScript {
    pub harness: String,
    pub version: String,
    /// The recording it reproduces, beside it.
    pub recording: String,
    /// What the recorder did in place of the real hook, as the recording says (`stall`, `exit2`).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub forced: std::collections::BTreeMap<String, String>,
    pub turns: Vec<Turn>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub prompt: String,
    pub calls: Vec<TurnCall>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnCall {
    /// The covering script's step it came from.
    pub step: String,
    pub tool: String,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The model response it was asked for in, from 1: calls of one response with `parallel`
    /// were asked for at once.
    pub response: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parallel: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subagent: bool,
}

/// The shell tools' names, in every harness, and where each keeps its command.
const SHELLS: &[(&str, &str)] = &[
    ("Bash", "/command"),
    ("bash", "/command"),
    ("shell_command", "/command"),
    ("exec_command", "/cmd"),
    ("shell", "/command/2"),
];

/// The turn script that reproduces `recording` (already masked, so its paths are placeholders).
pub fn of(recording: &Recording) -> TurnScript {
    let calls = recording
        .model
        .iter()
        .enumerate()
        .flat_map(|(at, asked)| {
            asked.calls.iter().map(move |call| TurnCall {
                step: call.step.clone(),
                tool: call.tool.clone(),
                input: call.input.clone(),
                run: SHELLS
                    .iter()
                    .find(|(name, _)| *name == call.tool)
                    .and_then(|(_, at)| call.input.pointer(at))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                response: at as u32 + 1,
                parallel: asked.parallel,
                background: call.background,
                subagent: call.subagent,
            })
        })
        .collect();
    TurnScript {
        harness: recording.harness.clone(),
        version: recording.version.clone(),
        recording: "recording.json".to_owned(),
        forced: recording.forced.clone(),
        turns: vec![Turn {
            prompt: recording.prompt.clone(),
            calls,
        }],
    }
}
