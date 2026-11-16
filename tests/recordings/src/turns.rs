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

use crate::projection::Shown;
use crate::recording::Recording;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnScript {
    pub harness: String,
    pub version: String,
    /// The recording it reproduces, beside it.
    pub recording: String,
    /// What the recording was made under (its `conditions`): the fake harness takes these as the
    /// conditions it reproduces the recording in, never as what the harness does.
    pub conditions: std::collections::BTreeMap<String, String>,
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
    /// The model's last message as the turn ended; none for a turn Esc interrupted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub says: Option<String>,
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
    /// For a sub-agent call: the calls its sub-agent made, in order, `response` counted in the
    /// sub-agent's own responses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<TurnCall>,
    /// For a sub-agent call: the sub-agent's last message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub says: Option<String>,
    /// For a sub-agent call: the tokens the harness's task notification says it used (Claude
    /// Code's `<subagent_tokens>`), which the harness works out itself and no stand-in gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

/// The shell tools' names, in every harness, and where each keeps its command.
const SHELLS: &[(&str, &str)] = &[
    ("Bash", "/command"),
    ("bash", "/command"),
    ("shell_command", "/command"),
    ("exec_command", "/cmd"),
    ("shell", "/command/2"),
];

/// The turn script that reproduces `recording` (already masked, so its paths are placeholders):
/// a turn for each of its prompts, holding the calls asked for in it.
pub fn of(recording: &Recording) -> TurnScript {
    let mut turns: Vec<Turn> = recording
        .prompts
        .iter()
        .enumerate()
        .map(|(at, prompt)| Turn {
            prompt: prompt.clone(),
            calls: Vec::new(),
            says: recording
                .said
                .iter()
                .find(|s| s.turn as usize == at + 1)
                .map(|s| s.says.clone()),
        })
        .collect();
    for (at, asked) in recording.model.iter().enumerate() {
        let Some(turn) = turns.get_mut((asked.turn as usize).saturating_sub(1)) else {
            continue;
        };
        turn.calls
            .extend(calls_of(asked, at as u32 + 1, &recording.shown));
    }
    TurnScript {
        harness: recording.harness.clone(),
        version: recording.version.clone(),
        recording: crate::recording::files_of(&recording.script).0,
        conditions: recording.conditions.clone(),
        forced: recording.forced.clone(),
        turns,
    }
}

/// The tokens a task notification among `shown` says the sub-agent of the call `tool_use_id`
/// used.
fn subagent_tokens(shown: &[Shown], tool_use_id: &str) -> Option<u64> {
    let id = format!("<tool-use-id>{tool_use_id}</tool-use-id>");
    shown
        .iter()
        .filter(|s| s.text.contains("<task-notification>") && s.text.contains(&id))
        .find_map(|s| {
            let after = s.text.split("<subagent_tokens>").nth(1)?;
            after
                .split("</subagent_tokens>")
                .next()?
                .trim()
                .parse()
                .ok()
        })
}

/// The calls of one model response, `response` its number; a sub-agent call with its own.
fn calls_of(asked: &crate::recording::Asked, response: u32, shown: &[Shown]) -> Vec<TurnCall> {
    asked
        .calls
        .iter()
        .map(|call| TurnCall {
            step: call.step.clone(),
            tool: call.tool.clone(),
            input: call.input.clone(),
            run: SHELLS
                .iter()
                .find(|(name, _)| *name == call.tool)
                .and_then(|(_, at)| call.input.pointer(at))
                .and_then(Value::as_str)
                .map(str::to_owned),
            response,
            parallel: asked.parallel,
            background: call.background,
            subagent: call.subagent,
            calls: call
                .subagent_model
                .iter()
                .enumerate()
                .flat_map(|(at, inner)| calls_of(inner, at as u32 + 1, shown))
                .collect(),
            says: call.says.clone(),
            tokens: call
                .subagent
                .then(|| subagent_tokens(shown, &call.tool_use_id))
                .flatten(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(id: &str, tokens: u32) -> Shown {
        Shown {
            kind: "context".to_owned(),
            text: format!(
                "<task-notification>\n<tool-use-id>{id}</tool-use-id>\n<status>completed</status>\n\
                 <usage><subagent_tokens>{tokens}</subagent_tokens><tool_uses>1</tool_uses></usage>\n\
                 </task-notification>"
            ),
            tool_use_id: None,
            is_error: None,
        }
    }

    #[test]
    fn a_subagents_tokens_are_read_from_its_own_task_notification() {
        let shown = [
            notification("<tool-use-3>", 41),
            notification("<tool-use-12>", 23),
        ];
        assert_eq!(subagent_tokens(&shown, "<tool-use-12>"), Some(23));
        assert_eq!(subagent_tokens(&shown, "<tool-use-3>"), Some(41));
        assert_eq!(subagent_tokens(&shown, "<tool-use-9>"), None);
    }
}
