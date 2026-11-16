// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the model is shown** (T1.1-03 item 7): the projection of a request a stand-in model
//! received, in any of the three APIs the harnesses speak. It keeps what the harness put in
//! front of the model from purlis's side: the user's text, each tool's result, hook-injected
//! context and deny reasons, and the lines purlis types into the chat. It leaves out the system
//! prompt (`system`, `instructions`, a `system` message) and the tool schemas (`tools`), so a
//! harness changing its own prompt changes no recording. Across a chat's requests, each thing
//! shown is kept once, where it was first shown.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One thing the model was shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shown {
    /// `user_text`, `context` (a `<system-reminder>`, a developer message: what a hook or the
    /// harness added) or `tool_result`.
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Which API a request was made in, by the path it was posted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// Anthropic's Messages API (`/v1/messages`): Claude Code.
    Messages,
    /// OpenAI's Responses API (`/v1/responses`): Codex.
    Responses,
    /// OpenAI's Chat Completions API (`/v1/chat/completions`): OpenCode's OpenAI-compatible
    /// provider.
    ChatCompletions,
}

impl Api {
    pub fn of_path(path: &str) -> Option<Api> {
        let path = path.split('?').next().unwrap_or(path);
        if path.ends_with("/messages") {
            Some(Api::Messages)
        } else if path.ends_with("/responses") {
            Some(Api::Responses)
        } else if path.ends_with("/chat/completions") {
            Some(Api::ChatCompletions)
        } else {
            None
        }
    }
}

/// What `body`, a request in `api`, shows the model, in order.
pub fn project(api: Api, body: &Value) -> Vec<Shown> {
    let mut out = Vec::new();
    match api {
        Api::Messages => {
            for message in array(body, "messages") {
                if message["role"] != "user" {
                    continue;
                }
                match &message["content"] {
                    Value::String(text) => user_text(text, &mut out),
                    Value::Array(blocks) => {
                        for block in blocks {
                            match block["type"].as_str() {
                                Some("text") => user_text(str_of(block, "text"), &mut out),
                                Some("tool_result") => out.push(Shown {
                                    kind: "tool_result".to_owned(),
                                    text: text_of(&block["content"]),
                                    tool_use_id: block["tool_use_id"].as_str().map(str::to_owned),
                                    is_error: block["is_error"].as_bool(),
                                }),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Api::Responses => {
            for item in array(body, "input") {
                match item["type"].as_str() {
                    Some("message") | None if item.get("role").is_some() => {
                        let role = item["role"].as_str().unwrap_or_default();
                        if role == "system" || role == "assistant" {
                            continue;
                        }
                        let text = text_of(&item["content"]);
                        if role == "developer" {
                            push(&mut out, "context", text, None, None);
                        } else {
                            user_text(&text, &mut out);
                        }
                    }
                    Some("function_call_output") | Some("custom_tool_call_output") => {
                        out.push(Shown {
                            kind: "tool_result".to_owned(),
                            text: text_of(&item["output"]),
                            tool_use_id: item["call_id"].as_str().map(str::to_owned),
                            is_error: None,
                        });
                    }
                    _ => {}
                }
            }
        }
        Api::ChatCompletions => {
            for message in array(body, "messages") {
                match message["role"].as_str() {
                    Some("user") => user_text(&text_of(&message["content"]), &mut out),
                    Some("tool") => out.push(Shown {
                        kind: "tool_result".to_owned(),
                        text: text_of(&message["content"]),
                        tool_use_id: message["tool_call_id"].as_str().map(str::to_owned),
                        is_error: None,
                    }),
                    _ => {}
                }
            }
        }
    }
    out
}

/// `shown` with what it already holds, and then each thing of `more` it does not hold yet.
pub fn extend_once(shown: &mut Vec<Shown>, more: Vec<Shown>) {
    for item in more {
        if !shown.contains(&item) {
            shown.push(item);
        }
    }
}

/// A user's text, with each `<system-reminder>` it carries taken apart as context.
fn user_text(text: &str, out: &mut Vec<Shown>) {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut rest = text;
    let mut plain = String::new();
    while let Some(start) = rest.find(OPEN) {
        plain.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let end = after.find(CLOSE).unwrap_or(after.len());
        push(out, "context", after[..end].trim().to_owned(), None, None);
        rest = after.get(end + CLOSE.len()..).unwrap_or_default();
    }
    plain.push_str(rest);
    push(out, "user_text", plain.trim().to_owned(), None, None);
}

fn push(
    out: &mut Vec<Shown>,
    kind: &str,
    text: String,
    tool_use_id: Option<String>,
    is_error: Option<bool>,
) {
    if !text.is_empty() {
        out.push(Shown {
            kind: kind.to_owned(),
            text,
            tool_use_id,
            is_error,
        });
    }
}

fn array<'a>(body: &'a Value, key: &str) -> &'a [Value] {
    body.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The text of a content value in any of the APIs' shapes: a string, or a list of parts whose
/// text is joined by newlines.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(text) => Some(text.clone()),
                _ => part
                    .get("text")
                    .or_else(|| part.get("output"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
