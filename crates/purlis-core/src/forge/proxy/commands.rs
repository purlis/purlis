// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A write's body, read once, strictly, and sent on as what was read.
//!
//! **Read by its type.** A body needs a `Content-Type`, and only JSON and a urlencoded form are
//! read; anything else on a write is refused (a multipart upload is not a thing a chat needs to
//! send a forge). A JSON object with a repeated key is refused, because two readers pick two
//! values. A body that names a credential (`private_token`, `access_token`, `token`…) as a key is
//! refused. What the forge is sent is the **canonical re-serialisation** of what was read
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

/// Every character a line of text may end at, as a reader of it would see it: the usual breaks,
/// the vertical tab and form feed, NEL, and the Unicode line and paragraph separators.
const LINE_BREAKS: &[char] = &[
    '\r', '\n', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// Whether `label` is one a bot merges on, compared as the bots do: case folded, `_` and spaces
/// taken as `-`, and the `~` and quotes of a GitLab quick action's label left off.
pub(crate) fn is_merge_label(label: &str) -> bool {
    let label = label
        .trim()
        .trim_matches(|c| matches!(c, '~' | '"' | '\''))
        .to_lowercase()
        .replace(['_', ' '], "-");
    MERGE_LABELS.contains(&label.as_str())
}

/// Whether a GitLab label quick action (`/label ~automerge`, `/relabel ~"ready to merge"`) on
/// `line` (lower case, trimmed) names a merge label.
fn is_label_quick_action(line: &str) -> bool {
    let rest = ["/label", "/relabel"].iter().find_map(|command| {
        line.strip_prefix(command)
            .filter(|rest| rest.starts_with(char::is_whitespace))
    });
    let Some(rest) = rest else {
        return false;
    };
    let plain: String = rest.chars().filter(|c| !matches!(c, '~' | '"')).collect();
    is_merge_label(&plain)
        || plain
            .split(|c: char| c.is_whitespace() || c == ',')
            .any(is_merge_label)
}

/// Whether any line of `text` is one of [`COMMANDS`] or a label quick action naming a merge label,
/// or the whole of it is a merge label. GitLab deletes every `\r` before it reads quick actions
/// (`content.delete!("\r")`), so the text is read as written and with every `\r` removed.
pub(crate) fn has_merge_command(text: &str) -> bool {
    has_merge_command_as_written(text)
        || (text.contains('\r') && has_merge_command_as_written(&text.replace('\r', "")))
}

fn has_merge_command_as_written(text: &str) -> bool {
    if is_merge_label(text) {
        return true;
    }
    text.split(LINE_BREAKS).any(|line| {
        let line = line.trim().to_lowercase();
        is_label_quick_action(&line)
            || COMMANDS.iter().any(|command| {
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

/// The quick actions that touch only the labels of the item written to, so they reach nothing
/// outside the recorded repo. A merge label in one is refused by [`has_merge_command`].
const LABEL_ONLY_ACTIONS: &[&str] = &["label", "unlabel", "relabel"];

/// Whether any line of `text` is a GitLab quick action: a `/` and a lower-case word at the start
/// of a line. GitLab ignores one in a code block, but the proxy refuses it there too: a fence is
/// not worth reading to be right about.
pub(crate) fn has_quick_action(text: &str) -> bool {
    has_quick_action_as_written(text)
        || (text.contains('\r') && has_quick_action_as_written(&text.replace('\r', "")))
}

fn has_quick_action_as_written(text: &str) -> bool {
    text.split(LINE_BREAKS).any(|line| {
        line.trim_start().strip_prefix('/').is_some_and(|rest| {
            // GitLab folds case with Unicode rules before it matches (a Kelvin sign is a `k`), so
            // the word is read as Unicode letters and digits, and a non-ASCII one is refused.
            let word: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            !word.is_empty()
                && (!word.is_ascii() || !LABEL_ONLY_ACTIONS.contains(&word.to_lowercase().as_str()))
                && rest[word.len()..]
                    .chars()
                    .next()
                    .is_none_or(char::is_whitespace)
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
    /// The `Content-Type` to send it with: the type it was read by, said once.
    pub(crate) media: String,
    /// Every label a body names, one each: GitLab's `labels`, `add_labels` and `remove_labels`
    /// are comma-separated, so a bot's trigger can ride in a list.
    pub(crate) labels: Vec<String>,
}

/// The keys a body names labels by.
const LABEL_KEYS: &[&str] = &[
    "labels",
    "add_labels",
    "remove_labels",
    "label",
    "label_names",
];

/// The keys a forge reads as the chat's own login. A chat holds no token, so one in a body is one
/// it found or made up.
const CREDENTIAL_KEYS: &[&str] = &[
    "private_token",
    "access_token",
    "job_token",
    "deploy_token",
    "oauth_token",
    "auth_token",
    "token",
];

/// A key as the forge reads it: lower case, `-` as `_`, and each bracketed part of a form key
/// (`a[token]`, `labels[]`) on its own.
fn key_parts(key: &str) -> impl Iterator<Item = String> + '_ {
    key.split(['[', ']'])
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase().replace('-', "_"))
}

fn json_has_credential_key(json: &Json) -> bool {
    match json {
        Json::Object(map) => map.iter().any(|(key, value)| {
            key_parts(key).any(|part| CREDENTIAL_KEYS.contains(&part.as_str()))
                || json_has_credential_key(value)
        }),
        Json::Array(items) => items.iter().any(json_has_credential_key),
        _ => false,
    }
}

fn push_labels(value: &str, out: &mut Vec<String>) {
    out.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_string),
    );
}

fn json_labels(json: &Json, out: &mut Vec<String>) {
    match json {
        Json::Object(map) => {
            for (key, value) in map {
                if key_parts(key).any(|part| LABEL_KEYS.contains(&part.as_str())) {
                    let mut strings = Vec::new();
                    json_strings(value, &mut strings);
                    strings.iter().for_each(|s| push_labels(s, out));
                }
                json_labels(value, out);
            }
        }
        Json::Array(items) => items.iter().for_each(|v| json_labels(v, out)),
        _ => {}
    }
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
            if json_has_credential_key(&json) {
                return Err(Refusal::CredentialInBody);
            }
            let mut strings = Vec::new();
            json_strings(&json, &mut strings);
            let mut labels = Vec::new();
            json_labels(&json, &mut labels);
            let canonical = serde_json::to_vec(&json).map_err(|_| Refusal::BodyUnreadable)?;
            Ok(Some(Body {
                strings,
                canonical,
                json: Some(json),
                form: Vec::new(),
                media: "application/json".to_string(),
                labels,
            }))
        }
        "application/x-www-form-urlencoded" => {
            let form: Vec<(String, String)> = url::form_urlencoded::parse(body)
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            if form
                .iter()
                .any(|(k, _)| key_parts(k).any(|part| CREDENTIAL_KEYS.contains(&part.as_str())))
            {
                return Err(Refusal::CredentialInBody);
            }
            let mut labels = Vec::new();
            for (key, value) in &form {
                if key_parts(key).any(|part| LABEL_KEYS.contains(&part.as_str())) {
                    push_labels(value, &mut labels);
                }
            }
            let mut out = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in &form {
                out.append_pair(k, v);
            }
            Ok(Some(Body {
                strings: form.iter().map(|(_, v)| v.clone()).collect(),
                canonical: out.finish().into_bytes(),
                json: None,
                form,
                media: "application/x-www-form-urlencoded".to_string(),
                labels,
            }))
        }
        "multipart/form-data" => Err(Refusal::Multipart),
        _ => Err(Refusal::BodyUnreadable),
    }
}

/// A multipart body that is one file and nothing else: the one shape a chat may send, to a route
/// that takes an upload. The parts are counted by the boundary, which may not appear in the file;
/// the part is named `file`, with a file name that is a name and not a path; and its headers are
/// `Content-Disposition` and, at most, `Content-Type`. Anything else is refused whole, and what is
/// sent is the bytes that were read.
pub(crate) fn parse_upload(body: &[u8], content_type: Option<&str>) -> Result<Body, Refusal> {
    if body.is_empty() || body.len() > MAX_BODY {
        return Err(Refusal::BodyUnreadable);
    }
    let content_type = content_type.ok_or(Refusal::Multipart)?;
    let mut parts = content_type.split(';');
    if !parts
        .next()
        .is_some_and(|m| m.trim().eq_ignore_ascii_case("multipart/form-data"))
    {
        return Err(Refusal::Multipart);
    }
    let params: Vec<&str> = parts.map(str::trim).collect();
    let [param] = params.as_slice() else {
        return Err(Refusal::Multipart);
    };
    let boundary = param
        .strip_prefix("boundary=")
        .map(|b| b.trim_matches('"'))
        .filter(|b| {
            (1..=70).contains(&b.len())
                && b.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"'()+_,-./:=?".contains(&c))
        })
        .ok_or(Refusal::Multipart)?;
    let open = format!("--{boundary}\r\n");
    let close = format!("\r\n--{boundary}--");
    let inner = body
        .strip_prefix(open.as_bytes())
        .ok_or(Refusal::Multipart)?;
    let text_end = find(inner, close.as_bytes()).ok_or(Refusal::Multipart)?;
    // Only a closing line, then at most one line break, follows.
    let after = &inner[text_end + close.len()..];
    if !(after.is_empty() || after == b"\r\n") {
        return Err(Refusal::Multipart);
    }
    let part = &inner[..text_end];
    // The boundary inside the part is a second part.
    if find(part, format!("--{boundary}").as_bytes()).is_some() {
        return Err(Refusal::Multipart);
    }
    let split = find(part, b"\r\n\r\n").ok_or(Refusal::Multipart)?;
    let headers = std::str::from_utf8(&part[..split]).map_err(|_| Refusal::Multipart)?;
    let mut disposition = false;
    for line in headers.split("\r\n") {
        let (name, value) = line.split_once(':').ok_or(Refusal::Multipart)?;
        match name.trim().to_ascii_lowercase().as_str() {
            "content-disposition" if !disposition => {
                disposition = true;
                let value = value.trim();
                let ok = value.starts_with("form-data;")
                    && param_of(value, "name") == Some("file")
                    && file_name_of(value).is_some_and(|f| {
                        !f.is_empty()
                            && f.len() <= 255
                            && !f.contains(['/', '\\'])
                            && f != ".."
                            && !f.bytes().any(|b| b.is_ascii_control())
                    });
                if !ok {
                    return Err(Refusal::Multipart);
                }
            }
            "content-type" => {}
            _ => return Err(Refusal::Multipart),
        }
    }
    if !disposition {
        return Err(Refusal::Multipart);
    }
    Ok(Body {
        strings: Vec::new(),
        canonical: body.to_vec(),
        json: None,
        form: Vec::new(),
        media: format!("multipart/form-data; boundary={boundary}"),
        labels: Vec::new(),
    })
}

fn file_name_of(disposition: &str) -> Option<&str> {
    param_of(disposition, "filename")
}

/// The value of one quoted parameter of a `Content-Disposition`, matched by its whole name:
/// `name` is not found inside `filename`.
fn param_of<'a>(disposition: &'a str, wanted: &str) -> Option<&'a str> {
    let mut found = None;
    for part in disposition.split(';').skip(1) {
        let (name, value) = part.trim().split_once('=')?;
        if name.trim().eq_ignore_ascii_case(wanted) {
            if found.is_some() {
                return None;
            }
            found = Some(value.trim().strip_prefix('"')?.strip_suffix('"')?);
        }
    }
    found
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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
                        // Only a number: this key is serde_json's own spelling of one, never a
                        // way for a string to be read as JSON of another shape.
                        let digits: String = map.next_value()?;
                        return match serde_json::from_str::<Json>(&digits) {
                            Ok(number @ Json::Number(_)) => Ok(number),
                            _ => Err(de::Error::custom("not a number")),
                        };
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
