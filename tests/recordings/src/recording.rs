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

/// What every covering recording is made under, said in its header and in the README.
pub const FULL_BYPASSED: &str =
    "Full sandbox, harness prompts bypassed (bypassPermissions / OpenCode ask allowed once)";

/// The file a recording of `script` is checked in as, beside its turn script: the covering
/// script's `recording.json` and `turns.json`, another's `<script>-recording.json` and
/// `<script>-turns.json`.
pub fn files_of(script: &str) -> (String, String) {
    if script == "covering" {
        ("recording.json".to_owned(), "turns.json".to_owned())
    } else {
        (
            format!("{script}-recording.json"),
            format!("{script}-turns.json"),
        )
    }
}

/// The turn script beside the recording at `path`.
pub fn turns_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .replace("recording.json", "turns.json");
    path.with_file_name(name)
}

/// Whether `path` names a checked-in recording.
pub fn is_recording(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == "recording.json" || n.ends_with("-recording.json"))
}

/// A recording, as checked in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    pub harness: String,
    pub version: String,
    /// The protection level it ran at: `full`.
    pub level: String,
    pub os: String,
    /// What the recording was made under, which the fake harness copies as conditions, never as
    /// behaviour: `summary` (what the sandbox and the harness's own prompts were, in a sentence),
    /// `permission_mode` (Claude Code's), `harness_asks` (how often the harness's own ask was
    /// allowed once).
    pub conditions: BTreeMap<String, String>,
    /// The covering script it ran (`scripts/<name>.json`), and the prompt of each of its turns.
    pub script: String,
    pub prompts: Vec<String>,
    /// What the recorder did in place of the real hook, by what: `stall` (a hook word whose
    /// first call it held past the hook's timeout) and `exit2` (one whose first call it answered
    /// with exit 2 and a reason on standard error), so the recording shows what the harness does
    /// with each; and `esc`, when Esc was pressed.
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
    /// What the model said as each turn that ended normally ended: its last message. A turn Esc
    /// interrupted has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub said: Vec<Said>,
}

/// The model's last message of a turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Said {
    /// The turn, from 1.
    pub turn: u32,
    pub says: String,
}

/// One hook call: what the harness handed it and what it answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookCall {
    /// The arguments after the program: `["hook", "pretooluse"]`.
    pub argv: Vec<String>,
    /// Its standard input, as JSON where it was JSON.
    pub stdin: Value,
    /// The name of every variable of its environment, each value withheld
    /// ([`crate::recorder::names`]).
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
    /// For a sub-agent call: what the stand-in asked the sub-agent for, response by response.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_model: Vec<Asked>,
    /// For a sub-agent call: the sub-agent's last message, once its turn ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub says: Option<String>,
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
/// its gap from the call before, and the parallel group the stand-in asked for it in. Only a
/// response the stand-in scripted as parallel makes a group: hooks that merely overlapped in
/// time keep the order they began in, ungrouped, so the mask never reorders them.
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
    for line in lines.iter().filter(|l| l.phase == "call") {
        let replied = reply_of(lines, line);
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
        calls.push(HookCall {
            argv: line.argv.clone(),
            stdin,
            env: line.env.clone().unwrap_or_default(),
            gap_ms: before.map_or(Value::Null, |b| Value::from(line.at_ms.saturating_sub(b))),
            group,
            reply: replied.map(|l| Reply {
                exit: l.exit.unwrap_or(-1),
                stdout: as_json(l.stdout.as_deref().unwrap_or_default()),
                stderr: l.stderr.clone().unwrap_or_default(),
            }),
            forced: replied
                .and_then(|l| l.forced.clone())
                .or_else(|| line.forced.clone()),
        });
        before = Some(line.at_ms);
    }
    calls
}

fn reply_of<'a>(lines: &'a [Line], call: &Line) -> Option<&'a Line> {
    lines.iter().find(|l| l.phase == "reply" && l.id == call.id)
}

/// Each PreToolUse and PostToolUse of one tool use that overlapped: the PostToolUse began before
/// the PreToolUse answered, so the harness ran the tool without waiting for purlis's verdict.
/// Any one fails the recording outright; the mask never hides it.
pub fn overlapping(lines: &[Line]) -> Vec<String> {
    let event = |line: &Line| {
        as_json(line.stdin.as_deref().unwrap_or_default())
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    // The same tool use: its id where the hook is handed one, else its tool and input.
    let tool_use = |line: &Line| {
        let stdin = as_json(line.stdin.as_deref().unwrap_or_default());
        tool_use_id(&stdin).or_else(|| {
            let tool = stdin.get("tool_name")?;
            let input = stdin.get("tool_input")?;
            Some(format!("{tool} {input}"))
        })
    };
    let calls: Vec<&Line> = lines.iter().filter(|l| l.phase == "call").collect();
    let mut found = Vec::new();
    for pre in calls.iter().filter(|l| event(l) == "PreToolUse") {
        let Some(key) = tool_use(pre) else { continue };
        let answered = reply_of(lines, pre).map_or(u64::MAX, |r| r.at_ms);
        for post in calls.iter().filter(|l| {
            matches!(event(l).as_str(), "PostToolUse" | "PostToolUseFailure")
                && tool_use(l).as_ref() == Some(&key)
        }) {
            if post.at_ms < answered {
                found.push(format!(
                    "{} began before {} of the same tool use answered: the harness did not wait for the verdict ({key})",
                    post.argv.join(" "),
                    pre.argv.join(" ")
                ));
            }
        }
    }
    found
}

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
    fn hooks_that_merely_overlap_keep_the_order_they_began_in_ungrouped() {
        // A Stop begun before a PostToolUse answered, as OpenCode's shim runs them.
        let lines = [
            line("a", "call", 100, "PostToolUse"),
            line("b", "call", 101, "Stop"),
            line("b", "reply", 150, ""),
            line("a", "reply", 151, ""),
        ];
        let calls = hook_calls(&lines, &[]);
        assert_eq!(calls[0].argv, ["hook", "posttooluse"]);
        assert_eq!(calls[1].argv, ["hook", "stop"]);
        assert!(calls.iter().all(|c| c.group.is_none()), "{calls:#?}");
    }
}
