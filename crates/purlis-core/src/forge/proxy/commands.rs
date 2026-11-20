// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A write's body, read once, strictly, and sent on as what was read.
//!
//! **Read by its type.** A body needs a `Content-Type`, and only JSON and a urlencoded form are
//! read; anything else on a write is refused (a multipart upload is not a thing a chat needs to
//! send a forge). A JSON object with a repeated key is refused, because two readers pick two
//! values. What the forge is sent is the **canonical re-serialisation** of what was read
//! ([`Body::canonical`]), never the chat's own bytes, so the bytes that were checked are the bytes
//! that are acted on.
//!
//! **Merge by other means.** A GitLab quick action (`/merge`) in a comment or a description
//! merges when the person's token writes it; a bot (`bors r+`, `@mergifyio queue`) merges when the
//! comment is read; a label (`automerge`) is a bot's trigger. A command is found by a line that
//! **starts** with it, which is how each reads it, so a sentence about merging is not one.

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value as Json;

use super::policy::Refusal;

/// The largest body read. A write over it is refused, never scanned in part.
pub(crate) const MAX_BODY: usize = 1 << 20;

/// Command words, lower case. A line starts with one, then ends or goes on after a space.
const COMMANDS: &[&str] = &[
    "/merge",
    "/approve",
    "/lgtm",
    "/rebase",
    "/lock",
    "/ship",
    "/submit_review",
    "/land",
    "/shipit",
    "bors r+",
    "bors r=",
    "bors merge",
    "@bors",
    "@mergify",
    "@mergifyio",
    "@dependabot merge",
    "@dependabot squash",
    "@kodiak merge",
    "@kodiakhq",
];

/// Labels a bot merges on, compared with `_` and spaces taken as `-`, in lower case.
const MERGE_LABELS: &[&str] = &[
    "automerge",
    "auto-merge",
    "ready-to-merge",
    "merge-when-ready",
    "merge-queue",
    "merge",
    "ship-it",
    "bors",
    "kodiak",
    "mergify",
    "queue",
    "lgtm",
    "approved",
];

/// Whether any line of `text` is one of [`COMMANDS`], or the whole of it is a merge label.
pub(crate) fn has_merge_command(text: &str) -> bool {
    let label = text.trim().to_lowercase().replace(['_', ' '], "-");
    if MERGE_LABELS.contains(&label.as_str()) {
        return true;
    }
    text.split(['\r', '\n']).any(|line| {
        let line = line.trim().to_lowercase();
        COMMANDS.iter().any(|command| {
            line.strip_prefix(command).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(char::is_whitespace)
                    || command.ends_with('=')
                    || command.ends_with('+')
                    || command.starts_with('@')
            })
        })
    })
}

/// A body that was read.
#[derive(Debug)]
pub(crate) struct Body {
    /// Every string it holds.
    pub(crate) strings: Vec<String>,
    /// What to send in its place.
    pub(crate) canonical: Vec<u8>,
    /// The JSON object, when it is JSON.
    pub(crate) json: Option<Json>,
    /// The pairs, when it is a form.
    pub(crate) form: Vec<(String, String)>,
}

/// Read a write's body by its `Content-Type`. `Ok(None)` is no body at all.
pub(crate) fn parse(body: &[u8], content_type: Option<&str>) -> Result<Option<Body>, Refusal> {
    if body.is_empty() {
        return Ok(None);
    }
    if body.len() > MAX_BODY {
        return Err(Refusal::BodyUnreadable);
    }
    let media = content_type
        .and_then(|ct| ct.split(';').next())
        .map(|m| m.trim().to_ascii_lowercase())
        .ok_or(Refusal::BodyUnreadable)?;
    match media.as_str() {
        "application/json" => {
            let Strict(json) = serde_json::from_slice(body).map_err(|_| Refusal::BodyUnreadable)?;
            let mut strings = Vec::new();
            json_strings(&json, &mut strings);
            let canonical = serde_json::to_vec(&json).map_err(|_| Refusal::BodyUnreadable)?;
            Ok(Some(Body {
                strings,
                canonical,
                json: Some(json),
                form: Vec::new(),
            }))
        }
        "application/x-www-form-urlencoded" => {
            let form: Vec<(String, String)> = url::form_urlencoded::parse(body)
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            let mut out = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in &form {
                out.append_pair(k, v);
            }
            Ok(Some(Body {
                strings: form.iter().map(|(_, v)| v.clone()).collect(),
                canonical: out.finish().into_bytes(),
                json: None,
                form,
            }))
        }
        _ => Err(Refusal::BodyUnreadable),
    }
}

pub(crate) fn json_strings(json: &Json, out: &mut Vec<String>) {
    match json {
        Json::String(s) => out.push(s.clone()),
        Json::Array(items) => items.iter().for_each(|v| json_strings(v, out)),
        Json::Object(map) => map.values().for_each(|v| json_strings(v, out)),
        _ => {}
    }
}

/// JSON read strictly: an object may not repeat a key.
struct Strict(Json);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON with no repeated key")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(v.into())
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(v.into())
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(serde_json::Number::from_f64(v).map_or(Json::Null, Json::Number))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::String(v.to_string()))
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_none<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
                let mut items = Vec::new();
                while let Some(Strict(item)) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Json::Array(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
                let mut out = serde_json::Map::new();
                // With `arbitrary_precision`, serde_json hands a number over as a one-key map.
                const NUMBER: &str = "$serde_json::private::Number";
                while let Some(key) = map.next_key::<String>()? {
                    if key == NUMBER && out.is_empty() {
                        let digits: String = map.next_value()?;
                        return serde_json::from_str(&digits).map_err(de::Error::custom);
                    }
                    let Strict(value) = map.next_value()?;
                    if out.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("repeated key {key}")));
                    }
                }
                Ok(Json::Object(out))
            }
        }
        d.deserialize_any(V).map(Strict)
    }
}
