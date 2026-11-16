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

use std::collections::{BTreeMap, BTreeSet};

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

/// A refused step's claim on what a recording heard: its name, its model calls, their tool-use
/// ids and their targets.
type Claim<'a> = (&'a String, Vec<Value>, Vec<String>, Vec<String>);

fn split(recording: &Value, steps: &BTreeSet<String>) -> (Value, Value) {
    // Each step's tool-use ids and targets.
    let claims: Vec<Claim> = steps
        .iter()
        .map(|step| {
            let calls: Vec<Value> = calls_of(recording)
                .filter(|call| call["step"] == step.as_str())
                .cloned()
                .collect();
            let ids: Vec<String> = calls
                .iter()
                .filter_map(|c| c["tool_use_id"].as_str().map(str::to_owned))
                .collect();
            let aims: Vec<String> = calls.iter().flat_map(|c| targets(&c["input"])).collect();
            (step, calls, ids, aims)
        })
        .collect();
    // The step an item belongs to: the one whose tool-use id it carries, or else the one whose
    // target it holds most of (the refused curl's whole command over the refused fetch's bare
    // URL, which the command holds too), never the one that sorts first.
    let owner = |id: Option<&str>, text: &str| -> Option<String> {
        if let Some(id) = id
            && let Some((step, ..)) = claims
                .iter()
                .find(|(_, _, ids, _)| ids.iter().any(|i| i == id))
        {
            return Some((*step).clone());
        }
        claims
            .iter()
            .filter_map(|(step, _, _, aims)| {
                aims.iter()
                    .filter(|aim| text.contains(aim.as_str()))
                    .map(String::len)
                    .max()
                    .map(|len| (len, *step))
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, step)| step.clone())
    };
    let hook_owner = |call: &Value| {
        let stdin = &call["stdin"];
        let input = stdin.get("tool_input").unwrap_or(stdin);
        owner(
            crate::recording::tool_use_id(stdin).as_deref(),
            &input.to_string(),
        )
    };
    let shown_owner = |item: &Value| {
        owner(
            item["tool_use_id"].as_str(),
            item["text"].as_str().unwrap_or_default(),
        )
    };
    let mut rest = recording.clone();
    let take = |list: &mut Value, owner: &dyn Fn(&Value) -> Option<String>| {
        let mut taken: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        if let Some(items) = list.as_array_mut() {
            let mut left = Vec::new();
            for item in items.drain(..) {
                match owner(&item) {
                    Some(step) => taken.entry(step).or_default().push(item),
                    None => left.push(item),
                }
            }
            *items = left;
        }
        taken
    };
    let mut hook_calls = take(&mut rest["hook_calls"], &hook_owner);
    let mut shown_items = take(&mut rest["shown"], &shown_owner);
    for asked in rest["model"].as_array_mut().into_iter().flatten() {
        if let Some(calls) = asked["calls"].as_array_mut() {
            calls.retain(|call| {
                !call["step"]
                    .as_str()
                    .is_some_and(|step| steps.contains(step))
            });
        }
    }
    let refused = claims
        .into_iter()
        .map(|(step, calls, ..)| {
            (
                step.clone(),
                serde_json::json!({
                    "model": calls,
                    "hook_calls": hook_calls.remove(step).unwrap_or_default(),
                    "shown": shown_items.remove(step).unwrap_or_default(),
                }),
            )
        })
        .collect();
    (Value::Object(refused), rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A refused fetch of a URL, and a refused curl of the same URL: OpenCode's hooks carry no
    /// tool-use id, so each hook call is found by its target alone.
    fn two_refused_calls_at_one_url(fetch: &str, curl: &str) -> Value {
        let url = "http://127.0.0.1:9/";
        json!({
            "model": [
                {"calls": [{"step": fetch, "tool": "webfetch", "input": {"url": url}, "tool_use_id": "<tool-use-1>"}]},
                {"calls": [{"step": curl, "tool": "bash", "input": {"command": format!("curl -sS -m 10 {url}")}, "tool_use_id": "<tool-use-2>"}]}
            ],
            "hook_calls": [
                {"argv": ["hook", "pretooluse"], "stdin": {"tool_input": {"url": url}}},
                {"argv": ["hook", "pretooluse"], "stdin": {"tool_input": {"command": format!("curl -sS -m 10 {url}")}}}
            ],
            "shown": []
        })
    }

    #[test]
    fn a_hook_call_goes_to_the_refused_step_whose_target_it_holds_most_of_whatever_their_names() {
        // Whichever of the two steps sorts first, the curl's hook call is the curl's and the
        // fetch's is the fetch's.
        for (fetch, curl) in [
            ("refused-a-web", "refused-z-net"),
            ("refused-z-web", "refused-a-net"),
        ] {
            let recording = two_refused_calls_at_one_url(fetch, curl);
            let steps: BTreeSet<String> = [fetch.to_owned(), curl.to_owned()].into();
            let (refused, rest) = split(&recording, &steps);
            assert_eq!(rest["hook_calls"], json!([]), "{fetch}, {curl}");
            let commands = |step: &str| -> Vec<Value> {
                refused[step]["hook_calls"]
                    .as_array()
                    .expect("its hook calls")
                    .iter()
                    .map(|c| c["stdin"]["tool_input"].clone())
                    .collect()
            };
            assert_eq!(
                commands(fetch),
                vec![json!({"url": "http://127.0.0.1:9/"})],
                "{fetch}"
            );
            assert_eq!(
                commands(curl),
                vec![json!({"command": "curl -sS -m 10 http://127.0.0.1:9/"})],
                "{curl}"
            );
        }
    }
}
