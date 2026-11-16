// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The turn script** `--script` plays: each turn a prompt and the calls the model asks for in
//! it, `{tool, input, run?, parallel?, background?, subagent?, harness_denies?}`, and a turn's
//! `says` and `fails`.
//!
//! It reads the turn scripts T1.1-03 writes beside each recording (`tests/recordings`,
//! `turns.json`), whose `tool` is the harness's own name for it, and a script written once for
//! every dialect, whose `tool` is a kind of call ([`Kind`]) the dialect turns into its own tool.
//! Its fields beyond these (`harness`, `recording`, `conditions`, `forced`, `step`, `response`)
//! are read where they say something and otherwise left alone.
//!
//! A shell call's `run` is the command it runs for real. A native call (a write, an edit, a
//! read, a fetch) states its target and never its outcome: the fake performs it, and whatever
//! confines the fake decides.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

// Fields read for the script's shape, and said back by `--version` where they are given.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Script {
    /// The harness and version it was recorded from, where it was: what `--version` answers.
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    /// What the recording was made under (T1.1-03): conditions, never behaviour.
    #[serde(default)]
    pub conditions: BTreeMap<String, String>,
    #[serde(default)]
    pub forced: BTreeMap<String, String>,
    pub turns: Vec<Turn>,
}

// `prompt` is what the script expects typed; the fake plays the turn on whatever is typed.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Turn {
    pub prompt: String,
    #[serde(default)]
    pub calls: Vec<Call>,
    /// What the model says as the turn ends, where a harness hands it to a hook.
    #[serde(default)]
    pub says: Option<String>,
    /// The kind of API error that ends the turn, as Claude Code names it in its `StopFailure`
    /// (`rate_limit`, `server_error`, ...), which it fires in place of the turn's `Stop`.
    #[serde(default)]
    pub fails: Option<String>,
}

// `subagent` marks the call T1.1-03's turn script saw start one; the tool says so too.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Call {
    #[serde(default)]
    pub step: Option<String>,
    /// A [`Kind`], or the dialect's own name for its tool.
    pub tool: String,
    #[serde(default)]
    pub input: Value,
    /// The command a shell call runs.
    #[serde(default)]
    pub run: Option<String>,
    /// The model response it was asked for in, from 1: calls of one response that are
    /// `parallel` are asked for at once. Where it is missing, each call is a response of its own.
    #[serde(default)]
    pub response: Option<u32>,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub subagent: bool,
    /// A sub-agent's own calls, and what it says as it ends.
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub says: Option<String>,
    /// For a sub-agent call: the tokens its model reported, which Claude Code says back in its
    /// task notification (`subagent_tokens`). Where it is missing, its last message's length in
    /// fours.
    #[serde(default)]
    pub tokens: Option<u64>,
    /// Why Claude Code's own check refuses the call (its auto mode's classifier, which a stand-in
    /// cannot run): the reason its `PermissionDenied` is told, and the model shown.
    #[serde(default)]
    pub harness_denies: Option<String>,
}

/// The kinds of call a script written once for every dialect names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bash,
    Write,
    Edit,
    Read,
    Web,
    Subagent,
    Mcp,
}

impl Kind {
    pub fn of(tool: &str) -> Option<Kind> {
        Some(match tool {
            "bash" | "shell" => Kind::Bash,
            "write" => Kind::Write,
            "edit" => Kind::Edit,
            "read" => Kind::Read,
            "web" => Kind::Web,
            "subagent" => Kind::Subagent,
            "mcp" => Kind::Mcp,
            _ => return None,
        })
    }
}

impl Call {
    /// The calls grouped into the model's responses, in order.
    pub fn responses(calls: &[Call]) -> Vec<Vec<&Call>> {
        let mut out: Vec<(Option<u32>, Vec<&Call>)> = Vec::new();
        for call in calls {
            match out.last_mut() {
                Some((Some(at), group)) if call.response == Some(*at) && call.parallel => {
                    group.push(call);
                }
                _ => out.push((call.response, vec![call])),
            }
        }
        out.into_iter().map(|(_, group)| group).collect()
    }

    /// A field of a neutral call's input, by any of the names a script may give it.
    pub fn field(&self, names: &[&str]) -> Option<String> {
        names
            .iter()
            .find_map(|name| self.input.get(*name).and_then(Value::as_str))
            .map(str::to_owned)
    }
}

/// The script at `path`, with each `<name>` placeholder T1.1-03's mask left in it replaced by
/// what `places` names, so a masked turn script runs in this run's own folders.
pub fn read(path: &Path, places: &BTreeMap<String, String>) -> Result<Script, String> {
    let mut text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    for (name, value) in places {
        // Inside a JSON string: the value written as JSON would write it there.
        let quoted = serde_json::to_string(value).map_err(|e| e.to_string())?;
        let inner = &quoted[1..quoted.len() - 1];
        text = text.replace(&format!("<{name}>"), inner);
    }
    serde_json::from_str(&text).map_err(|e| format!("{} is no turn script: {e}", path.display()))
}
