// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The mask** (T1.1-03 item 3): what may differ between two recordings of the same script and
//! still be the same recording. It is a closed list, and nothing else is masked:
//!
//! 1. **session ids**: the value of a key in [`SESSION_KEYS`], and a value of a session id's
//!    shape ([`SESSION_SHAPES`]: a UUID, OpenCode's `ses_…`), wherever it then appears;
//! 2. **tool-use ids**: the value of a key in [`TOOL_USE_KEYS`], wherever it then appears;
//! 3. **absolute paths**: each root the recording names in `roots` (its work tree, its home, its
//!    temp folder, the harness's install), wherever a string holds it;
//! 4. **timestamps and gaps**: the value of a key in [`TIME_KEYS`], an ISO 8601 time inside any
//!    string, and each hook call's `gap_ms`;
//! 5. **the order of calls the harness itself runs in parallel**: the hook calls of one parallel
//!    group (`group`, from the stand-in's account of which calls it asked for at once) are put
//!    in one canonical order, in the places the group took.
//!
//! Each masked value becomes a numbered placeholder (`<session-1>`, `<tool-use-2>`), the same
//! value the same placeholder throughout, so a reference from one call to another survives.
//! Anything else that differs is a difference ([`differences`]), and a secret-shaped value is
//! never masked: it fails the recording ([`secrets`]).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

/// Keys whose value is a session's id, in each harness's spelling.
pub const SESSION_KEYS: &[&str] = &[
    "session_id",
    "sessionId",
    "sessionID",
    "thread_id",
    "threadId",
    "conversation_id",
    "conversationId",
];

/// Keys whose value is one tool call's id, in each harness's and each model API's spelling.
pub const TOOL_USE_KEYS: &[&str] = &[
    "tool_use_id",
    "toolUseId",
    "toolUseID",
    "call_id",
    "callId",
    "callID",
    "tool_call_id",
    "toolCallId",
];

/// Keys whose value is a time.
pub const TIME_KEYS: &[&str] = &[
    "timestamp",
    "time",
    "created",
    "created_at",
    "createdAt",
    "at_ms",
];

/// A hook call's distance from the one before it, in milliseconds.
pub const GAP_KEY: &str = "gap_ms";

/// The shapes of a session id a harness writes into text where no key names it: a sub-agent's
/// session in a task's result, a transcript's file name.
pub static SESSION_SHAPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|ses_[0-9A-Za-z]{20,})\b",
    )
    .expect("a pattern")
});

static ISO_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?")
        .expect("a pattern")
});

/// Shapes a credential has. A recording holding one fails, masked or not.
static SECRET_SHAPES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("an Anthropic key", r"sk-ant-[A-Za-z0-9_-]{16,}"),
        ("an OpenAI key", r"sk-(?:proj-)?[A-Za-z0-9_-]{20,}"),
        (
            "a GitHub token",
            r"(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})",
        ),
        ("an AWS access key", r"AKIA[0-9A-Z]{16}"),
        ("a Slack token", r"xox[abprs]-[A-Za-z0-9-]{10,}"),
        ("a private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
        (
            "a JSON web token",
            r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        ),
    ]
    .into_iter()
    .map(|(what, pattern)| (what, Regex::new(pattern).expect("a pattern")))
    .collect()
});

/// `recording` with the closed list masked and its `roots` left out. Masking a masked recording
/// changes nothing.
pub fn mask(recording: &Value) -> Value {
    let mut value = recording.clone();
    let roots = take_roots(&mut value);
    let mut ids = Ids::default();
    // What the stand-in asked for first: it is in the script's order, so a tool-use id's number
    // does not hang on the order the harness ran a parallel group in.
    if let Some(model) = value.get("model") {
        ids.collect(model);
    }
    ids.collect(&value);
    let mut masked = rewrite(&value, None, &roots, &ids);
    canonical_parallel_order(&mut masked);
    masked
}

/// Where `a` and `b` differ, each as a JSON pointer and the two values; none when they are the
/// same recording.
pub fn differences(a: &Value, b: &Value) -> Vec<String> {
    let mut found = Vec::new();
    diff("", a, b, &mut found);
    found
}

/// Every secret-shaped value in `value`, each as a JSON pointer and what it looks like.
pub fn secrets(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk_strings("", value, &mut |at, text| {
        for (what, shape) in SECRET_SHAPES.iter() {
            if shape.is_match(text) {
                found.push(format!("{at}: {what}"));
            }
        }
    });
    found
}

/// The recording's roots, longest first, each with its placeholder; taken out of the recording.
fn take_roots(value: &mut Value) -> Vec<(String, String)> {
    let mut roots: Vec<(String, String)> = value
        .as_object_mut()
        .and_then(|map| map.remove("roots"))
        .and_then(|roots| match roots {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, path)| {
            let path = path.as_str()?.trim_end_matches('/').to_owned();
            (!path.is_empty()).then(|| (path, format!("<{name}>")))
        })
        .collect();
    roots.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    roots
}

/// Session and tool-use ids, numbered in the order they are first met.
#[derive(Default)]
struct Ids {
    named: BTreeMap<String, String>,
    sessions: usize,
    tool_uses: usize,
}

impl Ids {
    fn collect(&mut self, value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if let Some(id) = child
                        .as_str()
                        .filter(|id| !id.is_empty() && !is_placeholder(id))
                    {
                        if SESSION_KEYS.contains(&key.as_str()) {
                            self.session(id);
                        } else if TOOL_USE_KEYS.contains(&key.as_str())
                            && !self.named.contains_key(id)
                        {
                            self.tool_uses += 1;
                            self.named
                                .insert(id.to_owned(), format!("<tool-use-{}>", self.tool_uses));
                        }
                    }
                    self.collect(child);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.collect(item)),
            Value::String(text) => {
                for found in SESSION_SHAPES.find_iter(text) {
                    self.session(found.as_str());
                }
            }
            _ => {}
        }
    }

    fn session(&mut self, id: &str) {
        if !self.named.contains_key(id) {
            self.sessions += 1;
            self.named
                .insert(id.to_owned(), format!("<session-{}>", self.sessions));
        }
    }

    /// `text` with every id it holds replaced, longest first so no id eats part of another.
    fn replace(&self, text: &str) -> String {
        let mut ids: Vec<(&String, &String)> = self.named.iter().collect();
        ids.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(b.0)));
        let mut out = text.to_owned();
        for (id, placeholder) in ids {
            if out.contains(id.as_str()) {
                out = out.replace(id.as_str(), placeholder);
            }
        }
        out
    }
}

fn is_placeholder(text: &str) -> bool {
    text.starts_with('<') && text.ends_with('>')
}

fn rewrite(value: &Value, key: Option<&str>, roots: &[(String, String)], ids: &Ids) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, child)| (k.clone(), rewrite(child, Some(k), roots, ids)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| rewrite(item, None, roots, ids))
                .collect(),
        ),
        _ if key == Some(GAP_KEY) && !value.is_null() => Value::String("<gap>".to_owned()),
        Value::Number(_) if key.is_some_and(|k| TIME_KEYS.contains(&k)) => {
            Value::String("<time>".to_owned())
        }
        Value::String(text) => {
            if key.is_some_and(|k| TIME_KEYS.contains(&k)) && !text.is_empty() {
                return Value::String("<time>".to_owned());
            }
            let mut out = ids.replace(text);
            for (path, placeholder) in roots {
                if out.contains(path.as_str()) {
                    out = out.replace(path.as_str(), placeholder);
                }
            }
            if ISO_TIME.is_match(&out) {
                out = ISO_TIME.replace_all(&out, "<time>").into_owned();
            }
            Value::String(out)
        }
        _ => value.clone(),
    }
}

/// The hook calls of each parallel group, in one canonical order (their masked text), in the
/// places the group took. Calls outside a group keep their order.
fn canonical_parallel_order(recording: &mut Value) {
    let Some(calls) = recording
        .get_mut("hook_calls")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (at, call) in calls.iter().enumerate() {
        if let Some(group) = call.get("group").filter(|g| !g.is_null()) {
            groups.entry(group.to_string()).or_default().push(at);
        }
    }
    for places in groups.values() {
        let mut members: Vec<Value> = places.iter().map(|&at| calls[at].clone()).collect();
        members.sort_by_key(|call| call.to_string());
        for (&at, call) in places.iter().zip(members) {
            calls[at] = call;
        }
    }
}

fn diff(at: &str, a: &Value, b: &Value, found: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for key in keys {
                let here = format!("{at}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (x.get(key), y.get(key)) {
                    (Some(p), Some(q)) => diff(&here, p, q, found),
                    (p, q) => found.push(format!("{here}: {} != {}", shown(p), shown(q))),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let here = format!("{at}/{i}");
                match (x.get(i), y.get(i)) {
                    (Some(p), Some(q)) => diff(&here, p, q, found),
                    (p, q) => found.push(format!("{here}: {} != {}", shown(p), shown(q))),
                }
            }
        }
        _ if a != b => found.push(format!("{at}: {a} != {b}")),
        _ => {}
    }
}

fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "(absent)".to_owned(), Value::to_string)
}

fn walk_strings(at: &str, value: &Value, each: &mut dyn FnMut(&str, &str)) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                walk_strings(&format!("{at}/{key}"), child, each);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk_strings(&format!("{at}/{i}"), item, each);
            }
        }
        Value::String(text) => each(at, text),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn masking_twice_is_masking_once() {
        let raw = json!({
            "roots": {"work": "/tmp/r/work"},
            "hook_calls": [{"stdin": {"session_id": "abc-1", "cwd": "/tmp/r/work/x",
                "timestamp": "2026-11-16T04:39:00Z"}, "gap_ms": 12}]
        });
        let once = mask(&raw);
        assert_eq!(mask(&once), once);
        assert_eq!(once["hook_calls"][0]["stdin"]["cwd"], "<work>/x");
        assert_eq!(once["hook_calls"][0]["stdin"]["session_id"], "<session-1>");
        assert_eq!(once["hook_calls"][0]["gap_ms"], "<gap>");
        assert_eq!(once["hook_calls"][0]["stdin"]["timestamp"], "<time>");
    }

    #[test]
    fn a_session_id_written_into_text_is_masked_where_no_key_names_it() {
        let raw = json!({"shown": [{"text": "<task id=\"ses_e1dfb2d30ffeB0AQ9FzuJG8kmI\" state=\"completed\">"}]});
        assert_eq!(
            mask(&raw)["shown"][0]["text"],
            "<task id=\"<session-1>\" state=\"completed\">"
        );
    }

    #[test]
    fn a_parallel_group_is_put_in_one_order_and_other_calls_keep_theirs() {
        let a = json!({"hook_calls": [
            {"w": "stop"}, {"w": "a", "group": 1}, {"w": "x"}, {"w": "b", "group": 1}]});
        let b = json!({"hook_calls": [
            {"w": "stop"}, {"w": "b", "group": 1}, {"w": "x"}, {"w": "a", "group": 1}]});
        assert_eq!(mask(&a), mask(&b));
        let c = json!({"hook_calls": [
            {"w": "x"}, {"w": "a", "group": 1}, {"w": "stop"}, {"w": "b", "group": 1}]});
        assert_ne!(mask(&a), mask(&c));
    }
}
