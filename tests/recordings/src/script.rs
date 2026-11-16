// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A covering script** (T1.1-03 item 2): what the stand-in model asks a harness to do, once,
//! in one chat. It is written once for every harness: each step names a kind of call (`bash`,
//! `write`, `edit`, `read`, `web`, `subagent`), and the stand-in asks for it with the tool the
//! harness offers for that kind, in that tool's own input shape ([`crate::standin::native`]).
//!
//! A string may name a place the recording's driver fills in: `{work}` (the chat's work tree),
//! `{outside}` (a folder the chat may not write), `{denied_read}` (a file the chat may not read),
//! `{refused_url}` (a URL whose host the chat's sandbox does not list).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The script.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub name: String,
    /// The one prompt the chat is started with.
    pub prompt: String,
    /// The model's responses, in order: each asks for its calls, and the next is given once
    /// every one of them has its result. After the last, the turn ends.
    pub responses: Vec<Response>,
    /// What a sub-agent the script starts is answered with, the same way.
    #[serde(default)]
    pub subagent: Vec<Response>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub calls: Vec<Step>,
    /// Whether the calls are asked for at once, in one response.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub parallel: bool,
    /// Whether the recording's driver presses Esc once this response's calls have started,
    /// where the harness has one: the script's last response, since the turn ends there.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub esc: bool,
    /// The harnesses this response is given to; every harness when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialects: Option<Vec<String>>,
}

/// One call, by its kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Its name in the recording (`refused-bash`).
    pub step: String,
    /// `bash`, `write`, `edit`, `read`, `web` or `subagent`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
}

impl Script {
    pub fn read(path: &Path) -> Result<Script, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The script `name` in `scripts/`.
    pub fn named(name: &str) -> Result<Script, String> {
        Script::read(&crate::root().join("scripts").join(format!("{name}.json")))
    }

    /// The script as `harness` is given it: the responses for it alone, every place filled in.
    pub fn for_harness(&self, harness: &str, places: &BTreeMap<String, String>) -> Script {
        let keep = |r: &&Response| {
            r.dialects
                .as_ref()
                .is_none_or(|d| d.iter().any(|d| d == harness))
        };
        let fill = |r: &Response| {
            let mut value = serde_json::to_value(r).expect("a response is JSON");
            fill_places(&mut value, places);
            serde_json::from_value(value).expect("a filled response is a response")
        };
        Script {
            name: self.name.clone(),
            prompt: self.prompt.clone(),
            responses: self.responses.iter().filter(keep).map(fill).collect(),
            subagent: self.subagent.iter().filter(keep).map(fill).collect(),
        }
    }
}

/// Every `{place}` in `value`'s strings, filled in from `places`.
pub fn fill_places(value: &mut Value, places: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            for (name, with) in places {
                let mark = format!("{{{name}}}");
                if text.contains(&mark) {
                    *text = text.replace(&mark, with);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| fill_places(v, places)),
        Value::Object(map) => map.values_mut().for_each(|v| fill_places(v, places)),
        _ => {}
    }
}
