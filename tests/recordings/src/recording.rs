// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A recording** (T1.1-03): one run of a covering script by a real harness under purlis's
//! wrap, against a stand-in model. Assembled from what the recorder wrote of each hook call
//! ([`crate::recorder`]) and what the stand-in wrote of each request ([`crate::standin`]), then
//! masked ([`crate::mask`]) and checked in as `recorded/<harness>/<version>/recording.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::recorder::Line;

/// The harnesses recorded, by the name their version file has (`tests/fixtures/harness-versions`).
pub const HARNESSES: [&str; 3] = ["claude", "codex", "opencode"];

/// A recording, as checked in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    pub harness: String,
    pub version: String,
    /// The protection level it ran at: `full`.
    pub level: String,
    pub os: String,
    /// The covering script it ran (`scripts/<name>.json`), and the prompt it started with.
    pub script: String,
    pub prompt: String,
    /// What the recorder did in place of the real hook, by what: `stall` (a hook word whose
    /// first call it held past the hook's timeout) and `exit2` (one whose first call it answered
    /// with exit 2 and a reason on standard error), so the recording shows what the harness does
    /// with each.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub forced: BTreeMap<String, String>,
    /// The absolute paths the mask replaces, by placeholder name. Only a raw recording has them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roots: BTreeMap<String, String>,
    /// Every hook call, in the order the harness made them.
    pub hook_calls: Vec<HookCall>,
    /// What the stand-in model asked for, response by response.
    pub model: Vec<Asked>,
    /// What the model was shown from purlis's side (item 7): the projection of every request the
    /// stand-in received, without the system prompt or the tool schemas, de-duplicated.
    pub shown: Vec<crate::projection::Shown>,
}

/// One hook call: what the harness handed it and what it answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookCall {
    /// The arguments after the program: `["hook", "pretooluse"]`.
    pub argv: Vec<String>,
    /// Its standard input, as JSON where it was JSON.
    pub stdin: Value,
    /// The variables of its environment purlis or the harness set ([`crate::recorder::KEPT`]).
    pub env: BTreeMap<String, String>,
    /// Milliseconds since the hook call before it began; masked.
    pub gap_ms: Value,
    /// The parallel group it belongs to: the stand-in response that asked for its call with
    /// others at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u32>,
    /// What it answered; `None` when the harness ended it first (a hook timeout).
    pub reply: Option<Reply>,
    /// What the recorder did in place of the real hook for this call, if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub exit: i32,
    pub stdout: Value,
    pub stderr: String,
}

/// One response of the stand-in: the calls it asked for, or its end of the turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asked {
    /// The covering script's turn it belongs to, from 1.
    pub turn: u32,
    /// The calls, each as the harness names its tool; none for the end of a turn.
    pub calls: Vec<AskedCall>,
    /// Whether the calls were asked for at once.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parallel: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskedCall {
    /// The script step it came from (`refused-bash`, `native-write`).
    pub step: String,
    pub tool: String,
    pub input: Value,
    pub tool_use_id: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subagent: bool,
}

/// Where a harness's recording at `version` lives in the repo.
pub fn folder(harness: &str, version: &str) -> PathBuf {
    crate::root().join("recorded").join(harness).join(version)
}

/// The version `tests/fixtures/harness-versions/<harness>.txt` names, as purlis-core reads a
/// harness's `--version` answer.
pub fn pinned_version(harness: &str) -> Result<String, String> {
    let path = crate::repo()
        .join("tests/fixtures/harness-versions")
        .join(format!("{harness}.txt"));
    let said = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    purlis_core::profilestore::pin::version_in(harness, &said)
        .map(|v| v.to_string())
        .ok_or_else(|| format!("{}: {said:?} is no {harness} version", path.display()))
}

/// Each version file whose version has no recording checked in, with what is missing.
pub fn versions_without_a_recording(recorded: &Path) -> Vec<String> {
    let mut missing = Vec::new();
    for harness in HARNESSES {
        match pinned_version(harness) {
            Ok(version) => {
                for file in ["recording.json", "turns.json"] {
                    let path = recorded.join(harness).join(&version).join(file);
                    if !path.is_file() {
                        missing.push(format!(
                            "tests/fixtures/harness-versions/{harness}.txt names {version}, and recorded/{harness}/{version}/{file} is not there"
                        ));
                    }
                }
            }
            Err(e) => missing.push(e),
        }
    }
    missing
}

/// The hook calls the recorder wrote, in the order they began: each call joined with its reply,
/// its gap from the call before, and the parallel group its tool-use id was asked for in.
pub fn hook_calls(lines: &[Line], model: &[Asked]) -> Vec<HookCall> {
    let mut groups: BTreeMap<&str, u32> = BTreeMap::new();
    for (at, asked) in model.iter().enumerate() {
        if asked.parallel && asked.calls.len() > 1 {
            for call in &asked.calls {
                groups.insert(call.tool_use_id.as_str(), at as u32 + 1);
            }
        }
    }
    let mut calls: Vec<HookCall> = Vec::new();
    let mut before: Option<u64> = None;
    // The hooks the harness runs at once: each begun before the one before it answered (Codex
    // runs every SessionStart hook together).
    let mut previous: Option<(String, u64, Option<u64>)> = None;
    for line in lines.iter().filter(|l| l.phase == "call") {
        let reply = lines
            .iter()
            .find(|l| l.phase == "reply" && l.id == line.id)
            .map(|l| Reply {
                exit: l.exit.unwrap_or(-1),
                stdout: as_json(l.stdout.as_deref().unwrap_or_default()),
                stderr: l.stderr.clone().unwrap_or_default(),
            });
        let stdin = as_json(line.stdin.as_deref().unwrap_or_default());
        // By the call's id where the hook is handed one; else by its input, which OpenCode's
        // shim hands on as the model asked for it.
        let group = tool_use_id(&stdin)
            .and_then(|id| groups.get(id.as_str()).copied())
            .or_else(|| {
                let input = stdin.get("tool_input")?;
                model.iter().enumerate().find_map(|(at, asked)| {
                    (asked.parallel
                        && asked.calls.len() > 1
                        && asked.calls.iter().any(|call| &call.input == input))
                    .then_some(at as u32 + 1)
                })
            });
        let event = stdin
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let answered_at = lines
            .iter()
            .find(|l| l.phase == "reply" && l.id == line.id)
            .map(|l| l.at_ms);
        // Begun before the hook call before it answered, whatever its event: OpenCode's shim
        // runs `notification` and `permissionrequest` for one ask at once.
        // One the harness cut off (no reply) runs on only beside hooks of its own event.
        let at_once = group.is_none()
            && previous
                .as_ref()
                .is_some_and(|(e, _, replied)| match replied {
                    Some(replied) => line.at_ms < *replied,
                    None => *e == event,
                });
        let group = if at_once {
            calls.last().and_then(|c| c.group).or_else(|| {
                let first = calls.len() as u32;
                if let Some(last) = calls.last_mut() {
                    last.group = Some(EVENT_GROUPS + first);
                }
                Some(EVENT_GROUPS + first)
            })
        } else {
            group
        };
        previous = Some((event, line.at_ms, answered_at));
        calls.push(HookCall {
            argv: line.argv.clone(),
            stdin,
            env: line.env.clone().unwrap_or_default(),
            gap_ms: before.map_or(Value::Null, |b| Value::from(line.at_ms.saturating_sub(b))),
            group,
            reply,
            forced: lines
                .iter()
                .find(|l| l.phase == "reply" && l.id == line.id)
                .and_then(|l| l.forced.clone())
                .or_else(|| line.forced.clone()),
        });
        before = Some(line.at_ms);
    }
    calls
}

/// Where the numbers of the groups of hooks one event ran at once begin, apart from the
/// stand-in's responses' numbers.
const EVENT_GROUPS: u32 = 10_000;

/// The tool-use id a hook's input names, in any harness's spelling.
pub fn tool_use_id(stdin: &Value) -> Option<String> {
    crate::mask::TOOL_USE_KEYS
        .iter()
        .find_map(|key| stdin.get(*key).and_then(Value::as_str))
        .map(str::to_owned)
}

/// `text` as JSON where it parses whole, else as a string.
fn as_json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
}

/// The recording, masked, as its checked-in JSON.
pub fn masked(recording: &Recording) -> Value {
    crate::mask::mask(&serde_json::to_value(recording).expect("a recording is JSON"))
}

/// What is wrong with a checked-in recording: a secret-shaped value, or a root left unmasked.
pub fn problems(masked: &Value) -> Vec<String> {
    let mut found: Vec<String> = crate::mask::secrets(masked)
        .into_iter()
        .map(|at| format!("a secret-shaped value survives the mask at {at}"))
        .collect();
    if masked.get("roots").is_some() {
        found.push("the recording still names its roots".to_owned());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, phase: &str, at_ms: u64, event: &str) -> Line {
        Line {
            id: id.to_owned(),
            phase: phase.to_owned(),
            at_ms,
            argv: vec!["hook".to_owned(), event.to_lowercase()],
            stdin: (phase == "call").then(|| format!("{{\"hook_event_name\":\"{event}\"}}")),
            env: None,
            exit: (phase == "reply").then_some(0),
            stdout: None,
            stderr: None,
            forced: None,
        }
    }

    #[test]
    fn hooks_begun_before_the_last_answered_are_one_group() {
        // Two SessionStart hooks at once, then a Stop after both answered.
        let lines = [
            line("a", "call", 100, "SessionStart"),
            line("b", "call", 101, "SessionStart"),
            line("a", "reply", 150, ""),
            line("b", "reply", 151, ""),
            line("c", "call", 200, "SessionStart"),
            line("c", "reply", 210, ""),
        ];
        let calls = hook_calls(&lines, &[]);
        assert_eq!(calls[0].group, calls[1].group);
        assert!(calls[0].group.is_some());
        assert_eq!(calls[2].group, None, "begun after the one before answered");
    }
}
