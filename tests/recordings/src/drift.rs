// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The nightly re-record's diff** (T1.1-03 item 5): a fresh recording, at the pinned version or
//! at the harness's latest release, against the one checked in, both masked.
//!
//! - A difference in anything but a refused call is **drift**: it opens a `harness-drift` issue
//!   (or comments on the one open), so a harness update is seen the night it ships.
//! - A difference in a refused call (a covering step named `refused-*`) may be a harness that
//!   stopped refusing it. That is an **escape**, never drift (Q301): the re-record's escape entry,
//!   listed `kind: escape`, fails, which files it as a public `area:security`, `priority:p0`
//!   issue. Only that: a failed install, build or recording is an ordinary red.
//!
//! A refused call is found by what it is, never by where it stands: its model call by its step,
//! its hook calls by its target in their input ([`refused_part`]), whichever harness spells the
//! call (OpenCode's hooks are handed no tool-use id), and what the model was shown of it by its
//! id or its target. Each recording is split into its refused calls, compared step by step, and
//! the rest, compared place by place.

use std::collections::BTreeSet;

use serde_json::Value;

/// The label a drift issue carries.
pub const DRIFT_LABEL: &str = "harness-drift";

/// What differs, sorted into drift and escape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diff {
    pub drift: Vec<String>,
    pub escape: Vec<String>,
}

/// An issue to open.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub title: String,
    pub labels: Vec<String>,
    pub body: String,
}

/// `fresh` against `checked_in`, both masked recordings. Their `version` is not compared: the
/// latest release's re-record is held to the pinned version's recording.
pub fn compare(checked_in: &Value, fresh: &Value) -> Diff {
    let without_version = |v: &Value| {
        let mut v = v.clone();
        if let Some(map) = v.as_object_mut() {
            map.remove("version");
        }
        v
    };
    let (checked_in, fresh) = (without_version(checked_in), without_version(fresh));
    let steps: BTreeSet<String> = refused_steps(&checked_in)
        .into_iter()
        .chain(refused_steps(&fresh))
        .collect();
    let (refused_a, rest_a) = split(&checked_in, &steps);
    let (refused_b, rest_b) = split(&fresh, &steps);
    Diff {
        drift: crate::mask::differences(&rest_a, &rest_b),
        escape: crate::mask::differences(&refused_a, &refused_b)
            .into_iter()
            .map(|d| format!("/refused{d}"))
            .collect(),
    }
}

/// The drift issue for `diff`, if it has drift; `which` says which re-record it was (`pinned`
/// or `latest`).
pub fn issue(harness: &str, version: &str, which: &str, diff: &Diff, run: &str) -> Option<Issue> {
    if diff.drift.is_empty() {
        return None;
    }
    Some(Issue {
        title: format!("Harness drift: {harness} {version} ({which}) differs from its recording"),
        labels: vec![DRIFT_LABEL.to_owned(), "area:harness".to_owned()],
        body: format!(
            "The nightly re-record of {harness} at {version} ({which}) differs from \
             `tests/recordings/recorded/{harness}/` after masking, outside any refused call \
             (T1.1-03). Re-record on purpose if the change is understood, and say which \
             contract moved.\n\n{}\n\nRun: {run}\n",
            listed(&diff.drift)
        ),
    })
}

/// What a later night says on the drift issue already open: its run and what differs now.
pub fn comment(diff: &Diff, run: &str) -> String {
    format!("Drifted again: {run}\n\n{}\n", listed(&diff.drift))
}

/// The differences as a Markdown list, at most 60 of them.
fn listed(differences: &[String]) -> String {
    let mut lines: Vec<String> = differences
        .iter()
        .take(60)
        .map(|d| format!("- `{}`", d.replace('`', "'")))
        .collect();
    let more = differences.len().saturating_sub(60);
    if more > 0 {
        lines.push(format!("- and {more} more"));
    }
    lines.join("\n")
}

/// The `gh` commands that open `issue` in `repo`, or comment on the open one with its title.
pub fn gh_commands(issue: &Issue, repo: &str) -> Vec<Vec<String>> {
    let s = |x: &str| x.to_owned();
    vec![
        vec![
            s("label"),
            s("create"),
            s(DRIFT_LABEL),
            s("--repo"),
            s(repo),
            s("--force"),
            s("--color"),
            s("c5def5"),
            s("--description"),
            s("A harness's nightly re-record differs from its checked-in recording (T1.1-03)"),
        ],
        vec![
            s("issue"),
            s("create"),
            s("--repo"),
            s(repo),
            s("--title"),
            issue.title.clone(),
            s("--label"),
            issue.labels.join(","),
            s("--body"),
            issue.body.clone(),
        ],
    ]
}

/// The steps of the recording's refused calls.
fn refused_steps(recording: &Value) -> BTreeSet<String> {
    calls_of(recording)
        .filter_map(|call| call["step"].as_str())
        .filter(|step| step.starts_with("refused-"))
        .map(str::to_owned)
        .collect()
}

fn calls_of(recording: &Value) -> impl Iterator<Item = &Value> {
    recording["model"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|asked| asked["calls"].as_array().into_iter().flatten())
}

/// The keys of a call's input that say how, not what: never a refused call's target.
const NOT_A_TARGET: &[&str] = &[
    "description",
    "prompt",
    "format",
    "subagent_type",
    "content",
    "old_string",
    "new_string",
    "oldString",
    "newString",
];

/// What a refused call aimed at, as its input says it: each path, URL or command, and each file
/// a patch names.
pub fn targets(input: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let mut each = |text: &str| {
        if text.contains("*** Begin Patch") {
            for line in text.lines() {
                for mark in ["*** Add File: ", "*** Update File: ", "*** Delete File: "] {
                    if let Some(path) = line.strip_prefix(mark) {
                        found.push(path.trim().to_owned());
                    }
                }
            }
        } else if !text.trim().is_empty() {
            found.push(text.to_owned());
        }
    };
    match input {
        Value::String(text) => each(text),
        Value::Object(map) => {
            for (key, value) in map {
                if NOT_A_TARGET.contains(&key.as_str()) {
                    continue;
                }
                match value {
                    Value::String(text) => each(text),
                    Value::Array(items) => {
                        items.iter().filter_map(Value::as_str).for_each(&mut each)
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    found
}

/// The recording split into its refused calls, by step (each step's model calls, hook calls and
/// what the model was shown of it), and the rest without them.
pub fn refused_part(recording: &Value, step: &str) -> Value {
    split(recording, &BTreeSet::from([step.to_owned()])).0[step].clone()
}

fn split(recording: &Value, steps: &BTreeSet<String>) -> (Value, Value) {
    let mut rest = recording.clone();
    let mut refused = serde_json::Map::new();
    for step in steps {
        let calls: Vec<Value> = calls_of(recording)
            .filter(|call| call["step"] == step.as_str())
            .cloned()
            .collect();
        let ids: Vec<&str> = calls
            .iter()
            .filter_map(|c| c["tool_use_id"].as_str())
            .collect();
        let aims: Vec<String> = calls.iter().flat_map(|c| targets(&c["input"])).collect();
        let aimed = |text: &str| aims.iter().any(|aim| text.contains(aim.as_str()));
        let hook = |call: &Value| {
            let stdin = &call["stdin"];
            let input = stdin.get("tool_input").unwrap_or(stdin);
            aimed(&input.to_string())
                || crate::recording::tool_use_id(stdin).is_some_and(|id| ids.contains(&id.as_str()))
        };
        let shown = |item: &Value| {
            item["tool_use_id"]
                .as_str()
                .is_some_and(|id| ids.contains(&id))
                || item["text"].as_str().is_some_and(aimed)
        };
        let take = |list: &mut Value, keep: &dyn Fn(&Value) -> bool| -> Vec<Value> {
            let Some(items) = list.as_array_mut() else {
                return Vec::new();
            };
            let (taken, left): (Vec<Value>, Vec<Value>) =
                items.drain(..).partition(|item| keep(item));
            *items = left;
            taken
        };
        let hook_calls = take(&mut rest["hook_calls"], &hook);
        let shown_items = take(&mut rest["shown"], &shown);
        for asked in rest["model"].as_array_mut().into_iter().flatten() {
            take(&mut asked["calls"], &|call: &Value| {
                call["step"] == step.as_str()
            });
        }
        refused.insert(
            step.clone(),
            serde_json::json!({"model": calls, "hook_calls": hook_calls, "shown": shown_items}),
        );
    }
    (Value::Object(refused), rest)
}
