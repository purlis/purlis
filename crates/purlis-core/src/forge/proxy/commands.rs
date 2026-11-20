// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Text that tells a forge or a merge bot to merge: the merge by other means.
//!
//! A GitLab quick action (`/merge`) in a comment or a description merges when the person's token
//! writes it; a bot (`bors r+`, `@mergifyio queue`) merges when the comment is read. Both are
//! found by a line that **starts** with the command, which is how each reads it, so a sentence
//! about merging is not one.

use serde_json::Value as Json;

/// Command words, lower case. A line starts with one, then ends or goes on after a space.
const COMMANDS: &[&str] = &[
    "/merge",
    "/approve",
    "/lgtm",
    "/submit_review",
    "/land",
    "/shipit",
    "bors r+",
    "bors r=",
    "bors merge",
    "@bors",
    "@mergifyio",
    "@dependabot merge",
    "@dependabot squash",
    "@kodiak merge",
];

/// Whether any line of `text` is one of [`COMMANDS`].
pub(crate) fn has_merge_command(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim().to_lowercase();
        COMMANDS.iter().any(|command| {
            line.strip_prefix(command).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(char::is_whitespace)
                    || command.ends_with('=')
                    || command.ends_with('+')
            })
        })
    })
}

/// Every string a request body holds: the values of a JSON document, or of a form, and the body's
/// own text when it is neither.
pub(crate) fn body_strings(body: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(json) = serde_json::from_slice::<Json>(body) {
        json_strings(&json, &mut out);
        return out;
    }
    out.extend(
        url::form_urlencoded::parse(body)
            .map(|(_, v)| v.into_owned())
            .collect::<Vec<_>>(),
    );
    out.push(String::from_utf8_lossy(body).into_owned());
    out
}

pub(crate) fn json_strings(json: &Json, out: &mut Vec<String>) {
    match json {
        Json::String(s) => out.push(s.clone()),
        Json::Array(items) => items.iter().for_each(|v| json_strings(v, out)),
        Json::Object(map) => map.values().for_each(|v| json_strings(v, out)),
        _ => {}
    }
}
