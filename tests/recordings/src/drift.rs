// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The nightly re-record's diff** (T1.1-03 item 5): a fresh recording, at the pinned version or
//! at the harness's latest release, against the one checked in, both masked.
//!
//! - A difference in anything but a refused call is **drift**: it opens a `harness-drift` issue
//!   (or comments on the one open), so a harness update is seen the night it ships.
//! - A difference in a refused call (a covering step named `refused-*`: its hook calls, what the
//!   model was shown of its result, or the call itself) may be a harness that stopped refusing
//!   it. That is an **escape**, never drift (Q301): the re-record fails, and the nightly entry,
//!   listed `kind: escape`, files it as a public `area:security`, `priority:p0` issue.

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
    let (checked_in, fresh) = (&without_version(checked_in), &without_version(fresh));
    let refused = refused_ids(checked_in)
        .union(&refused_ids(fresh))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut diff = Diff::default();
    for difference in crate::mask::differences(checked_in, fresh) {
        if touches_refused(&difference, checked_in, fresh, &refused) {
            diff.escape.push(difference);
        } else {
            diff.drift.push(difference);
        }
    }
    diff
}

/// The drift issue for `diff`, if it has drift; `which` says which re-record it was (`pinned`
/// or `latest`).
pub fn issue(harness: &str, version: &str, which: &str, diff: &Diff, run: &str) -> Option<Issue> {
    if diff.drift.is_empty() {
        return None;
    }
    let shown: Vec<String> = diff
        .drift
        .iter()
        .take(60)
        .map(|d| format!("- `{}`", d.replace('`', "'")))
        .collect();
    let more = diff.drift.len().saturating_sub(60);
    Some(Issue {
        title: format!("Harness drift: {harness} {version} ({which}) differs from its recording"),
        labels: vec![DRIFT_LABEL.to_owned(), "area:harness".to_owned()],
        body: format!(
            "The nightly re-record of {harness} at {version} ({which}) differs from \
             `tests/recordings/recorded/{harness}/` after masking, outside any refused call \
             (T1.1-03). Re-record on purpose if the change is understood, and say which \
             contract moved.\n\n{}{}\n\nRun: {run}\n",
            shown.join("\n"),
            if more > 0 {
                format!("\n- and {more} more")
            } else {
                String::new()
            }
        ),
    })
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

/// The tool-use ids of the recording's refused calls.
fn refused_ids(recording: &Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for asked in recording["model"].as_array().into_iter().flatten() {
        for call in asked["calls"].as_array().into_iter().flatten() {
            if call["step"]
                .as_str()
                .is_some_and(|s| s.starts_with("refused-"))
                && let Some(id) = call["tool_use_id"].as_str()
            {
                ids.insert(id.to_owned());
            }
        }
    }
    ids
}

/// Whether `difference`, a JSON pointer and its values, is about a refused call.
fn touches_refused(difference: &str, a: &Value, b: &Value, refused: &BTreeSet<String>) -> bool {
    let pointer = difference.split(':').next().unwrap_or_default();
    let parts: Vec<&str> = pointer.split('/').skip(1).collect();
    let holds = |value: Option<&Value>| {
        value.is_some_and(|v| {
            let text = v.to_string();
            refused.iter().any(|id| text.contains(id.as_str()))
        })
    };
    match parts.as_slice() {
        ["hook_calls", n, ..] | ["shown", n, ..] => {
            let list = parts[0];
            holds(a[list].get(n.parse::<usize>().unwrap_or(usize::MAX)))
                || holds(b[list].get(n.parse::<usize>().unwrap_or(usize::MAX)))
        }
        ["model", n, "calls", m, ..] => {
            let at = |v: &Value| {
                v["model"]
                    .get(n.parse::<usize>().unwrap_or(usize::MAX))
                    .and_then(|r| r["calls"].get(m.parse::<usize>().unwrap_or(usize::MAX)))
                    .cloned()
            };
            [at(a), at(b)].iter().flatten().any(|call| {
                call["step"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("refused-"))
            })
        }
        _ => false,
    }
}
