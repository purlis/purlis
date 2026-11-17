// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the model is shown** (T1.1-03 item 7): the projection of a request a stand-in model
//! received, in any of the three APIs the harnesses speak. It keeps what the harness put in
//! front of the model from purlis's side: the user's text, each tool's result, hook-injected
//! context and deny reasons, and the lines purlis types into the chat. It leaves out the system
//! prompt (`system`, `instructions`, a `system` message) and the tool schemas (`tools`), so a
//! harness changing its own prompt changes no recording, and, by the closed list
//! [`CLAUDE_CODE_OWN`], the text Claude Code adds of its own among the messages. Across a chat's
//! requests, each thing shown is kept once, where it was first shown.

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

/// How much of a context item a piece of Claude Code's own text takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// The whole item, when the item begins with the opening.
    Item,
    /// Each paragraph (text between blank lines) that is the opening whole, wherever it stands.
    Paragraph,
    /// From a paragraph that begins with the opening to the end of the item.
    ToTheEnd,
}

/// A piece of text Claude Code adds among the messages of its own accord, from neither purlis
/// nor the model's task: by its name, the words it opens with, and how far it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessOwn {
    pub name: &'static str,
    pub opening: &'static str,
    pub extent: Extent,
}

/// Every piece of Claude Code's own text the projection leaves out, by name (Claude Code
/// 2.1.325's covering and asks recordings). Nothing is added without a recording showing it.
pub const CLAUDE_CODE_OWN: &[HarnessOwn] = &[
    HarnessOwn {
        name: "attribution",
        opening: "Attribution for git commits and pull requests you create from here on",
        extent: Extent::Item,
    },
    HarnessOwn {
        name: "not-heard-from",
        opening: "The user hasn't heard from you in a while. As you continue, keep them updated \
                  when there's something to tell — a finding, a change of plan.",
        extent: Extent::Paragraph,
    },
    HarnessOwn {
        name: "deferred-tools",
        opening: "The following deferred tools are now available via ToolSearch.",
        extent: Extent::Item,
    },
    HarnessOwn {
        name: "agent-types",
        opening: "Available agent types for the Agent tool:",
        extent: Extent::Item,
    },
    // Its environment section, and what it puts after it in the same message (the model, the
    // deferred tools, the agent types, its MCP servers' instructions, the skills, the mode,
    // the date): in Claude Code 2.1.325 it follows a SessionStart hook's context in one
    // `system` message.
    HarnessOwn {
        name: "environment",
        opening: "# Environment\nYou have been invoked in the following environment:",
        extent: Extent::ToTheEnd,
    },
];

/// `text`, a context item, without the pieces of `own`; empty when nothing else is left.
pub fn without_harness_own(text: &str, own: &[HarnessOwn]) -> String {
    if own
        .iter()
        .any(|o| o.extent == Extent::Item && text.starts_with(o.opening))
    {
        return String::new();
    }
    let mut kept: Vec<&str> = Vec::new();
    for paragraph in text.split("\n\n") {
        if own
            .iter()
            .any(|o| o.extent == Extent::ToTheEnd && paragraph.starts_with(o.opening))
        {
            break;
        }
        if own
            .iter()
            .any(|o| o.extent == Extent::Paragraph && paragraph.trim() == o.opening)
        {
            continue;
        }
        kept.push(paragraph);
    }
    kept.join("\n\n").trim().to_owned()
}

/// What `body`, a request in `api`, shows the model, in order.
pub fn project(api: Api, body: &Value) -> Vec<Shown> {
    let mut shown = shown_in(api, body);
    if api == Api::Messages {
        shown = shown
            .into_iter()
            .filter_map(|mut item| {
                if item.kind != "context" {
                    return Some(item);
                }
                item.text = without_harness_own(&item.text, CLAUDE_CODE_OWN);
                (!item.text.is_empty()).then_some(item)
            })
            .collect();
    }
    shown
}

/// What `body` shows the model, Claude Code's own text and all.
fn shown_in(api: Api, body: &Value) -> Vec<Shown> {
    let mut out = Vec::new();
    match api {
        Api::Messages => {
            for message in array(body, "messages") {
                // A system message among the chat's messages is what a hook added, not the
                // harness's prompt (`system`, apart): Claude Code 2.1.325 sends a SessionStart
                // hook's context so.
                if message["role"] == "system" {
                    push(
                        &mut out,
                        "context",
                        text_of(&message["content"]),
                        None,
                        None,
                    );
                    continue;
                }
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
            // Codex sends its own system prompt as the developer messages before the first
            // user message (its instructions, its permissions): the harness's prompt, left out.
            // A hook's context comes after.
            let mut met_a_user = false;
            for item in array(body, "input") {
                match item["type"].as_str() {
                    Some("message") | None if item.get("role").is_some() => {
                        let role = item["role"].as_str().unwrap_or_default();
                        if role == "system" || role == "assistant" {
                            continue;
                        }
                        let text = text_of(&item["content"]);
                        if role == "developer" {
                            if met_a_user {
                                push(&mut out, "context", text, None, None);
                            }
                        } else {
                            met_a_user = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reminder(text: &str) -> String {
        format!("<system-reminder>\n{text}\n</system-reminder>")
    }

    #[test]
    fn claude_codes_own_text_is_left_out_and_purliss_and_the_tasks_are_kept() {
        let not_heard = "The user hasn't heard from you in a while. As you continue, keep them \
                         updated when there's something to tell — a finding, a change of plan.";
        let body = json!({"messages": [
            {"role": "system", "content": "SessionStart hook additional context: purlis's own.\n\n\
                # Environment\nYou have been invoked in the following environment: \n - Platform: linux\n\n\
                The following skills are available for use with the Skill tool:\n- a skill"},
            {"role": "user", "content": [
                {"type": "text", "text": reminder("Attribution for git commits and pull requests you create from here on (…):\n- End git commit messages with: x")},
                {"type": "text", "text": "THE TASK"},
                {"type": "text", "text": reminder(&format!("{not_heard}\n\n{not_heard}"))},
                {"type": "text", "text": reminder("The following deferred tools are now available via ToolSearch. …\nMonitor")},
                {"type": "text", "text": reminder("Available agent types for the Agent tool:\n- claude: …\n\n# MCP Server Instructions")},
                {"type": "text", "text": reminder(&format!("{not_heard}\n\n<task-notification>\n<result>done</result>\n</task-notification>"))},
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": ""}
            ]}
        ]});
        let shown: Vec<(String, String)> = project(Api::Messages, &body)
            .into_iter()
            .map(|s| (s.kind, s.text))
            .collect();
        let pair = |kind: &str, text: &str| (kind.to_owned(), text.to_owned());
        assert_eq!(
            shown,
            [
                pair(
                    "context",
                    "SessionStart hook additional context: purlis's own."
                ),
                pair("user_text", "THE TASK"),
                pair(
                    "context",
                    "<task-notification>\n<result>done</result>\n</task-notification>"
                ),
                pair("tool_result", ""),
            ]
        );
    }

    #[test]
    fn only_claude_codes_requests_lose_its_own_text() {
        let body = json!({"messages": [{"role": "user", "content":
            "<system-reminder>Available agent types for the Agent tool:</system-reminder>hi"}]});
        let shown = project(Api::ChatCompletions, &body);
        assert_eq!(shown[0].text, "Available agent types for the Agent tool:");
    }
}
