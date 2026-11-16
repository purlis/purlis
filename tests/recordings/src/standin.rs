// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The stand-in model** (T1.1-03 item 2): one scripted model on a loopback port, speaking the
//! three APIs the harnesses use: Anthropic's Messages API for Claude Code, OpenAI's Responses API
//! for Codex, and OpenAI's Chat Completions for OpenCode's OpenAI-compatible provider. It needs no
//! credential and calls no model.
//!
//! It is stateless about the chat: which response a request gets is read from the request
//! itself, the first of the script's responses some call of which has no result in it yet
//! ([`StandIn::answer`]). So a harness's retry is answered the same, and a request that is not
//! the chat's (a title, a quota check, a token count) is answered with a word and moves nothing.
//! A request is the chat's when its user text holds the script's prompt, or a sub-agent's when it
//! holds a sub-agent step's prompt.
//!
//! Every request is kept ([`Heard`]): its projection (what the model was shown,
//! [`crate::projection`]) and which response it was given, from which a recording's `model` and
//! `shown` are made.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::projection::{Api, Shown, project};
use crate::recording::{Asked, AskedCall};
use crate::script::{Response, Script, Step};

/// The longest a stand-in serves, so one a run never ended ends by itself.
pub const AT_MOST: Duration = Duration::from_secs(900);

/// What the turn ends with.
pub const DONE: &str = "Covering script done.";

/// Which conversation a request belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conversation {
    Main,
    Subagent,
    /// A request that is not the chat's: answered with a word.
    Side,
}

/// One request the stand-in heard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heard {
    pub path: String,
    pub conversation: Conversation,
    /// The response it was given, from 0; the end of the turn is the script's length.
    pub response: Option<usize>,
    pub shown: Vec<Shown>,
    /// The tools it offered, by name.
    pub offered: Vec<String>,
    /// The calls it was asked to make, as its harness names them.
    pub asked: Vec<AskedCall>,
}

/// An HTTP answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

/// The stand-in for one harness and one script.
pub struct StandIn {
    pub harness: String,
    pub script: Script,
    pub heard: Mutex<Vec<Heard>>,
}

/// A tool a request offers: its name and whether it is a free-form (`custom`) tool.
#[derive(Debug, Clone)]
struct Offer {
    name: String,
    custom: bool,
}

impl StandIn {
    pub fn new(harness: &str, script: Script) -> StandIn {
        StandIn {
            harness: harness.to_owned(),
            script,
            heard: Mutex::new(Vec::new()),
        }
    }

    /// The answer to one request.
    pub fn answer(&self, method: &str, path: &str, body: &[u8]) -> Answer {
        // No model catalogue: a harness that asks for one keeps its own. Codex 0.169 read an
        // empty one as a model it knows nothing of, and offered it no `apply_patch`.
        if method == "GET" {
            return Answer {
                status: 404,
                content_type: "text/plain",
                body: b"the stand-in keeps no model catalogue".to_vec(),
            };
        }
        if path
            .split('?')
            .next()
            .unwrap_or(path)
            .ends_with("/count_tokens")
        {
            return json_answer(&json!({"input_tokens": 1}));
        }
        let Some(api) = Api::of_path(path) else {
            return Answer {
                status: 404,
                content_type: "text/plain",
                body: format!("the stand-in serves no {path}").into_bytes(),
            };
        };
        let body: Value = match serde_json::from_slice(body) {
            Ok(body) => body,
            Err(e) => {
                return Answer {
                    status: 400,
                    content_type: "text/plain",
                    body: format!("the stand-in reads JSON only: {e}").into_bytes(),
                };
            }
        };
        let shown = project(api, &body);
        let offers = offers(api, &body);
        // A request that offers no tool is not the chat's turn, whatever it quotes of it: a
        // harness's title or summary of the chat, answered with a word.
        let conversation = if offers.is_empty() {
            Conversation::Side
        } else {
            self.conversation_of(&shown)
        };
        let responses: &[Response] = match conversation {
            Conversation::Main => &self.script.responses,
            Conversation::Subagent => &self.script.subagent,
            Conversation::Side => &[],
        };
        let answered = results_in(api, &body);
        let next = (conversation != Conversation::Side).then(|| {
            responses
                .iter()
                .enumerate()
                .position(|(at, r)| {
                    (0..r.calls.len()).any(|i| !answered.contains(&self.id(conversation, at, i)))
                })
                .unwrap_or(responses.len())
        });
        let stream = body["stream"] == true;
        let model = body["model"].as_str().unwrap_or("stand-in");
        let given = next.and_then(|at| responses.get(at).map(|r| (at, r)));
        let calls: Vec<(NativeCall, &Step)> = given.map_or_else(Vec::new, |(at, response)| {
            response
                .calls
                .iter()
                .enumerate()
                .map(|(i, step)| {
                    (
                        native(api, step, &offers, self.id(conversation, at, i)),
                        step,
                    )
                })
                .collect()
        });
        self.heard.lock().expect("the stand-in's log").push(Heard {
            path: path.to_owned(),
            conversation,
            response: next,
            shown,
            offered: offers.iter().map(|o| o.name.clone()).collect(),
            asked: calls
                .iter()
                .map(|(call, step)| AskedCall {
                    step: step.step.clone(),
                    tool: call.name.clone(),
                    input: call.input.clone(),
                    tool_use_id: call.id.clone(),
                    background: step.background,
                    subagent: step.kind == "subagent",
                })
                .collect(),
        });
        if given.is_some() {
            let calls: Vec<NativeCall> = calls.into_iter().map(|(call, _)| call).collect();
            encode(api, model, stream, &calls, None)
        } else {
            let words = if conversation == Conversation::Side {
                "ok"
            } else {
                DONE
            };
            encode(api, model, stream, &[], Some(words))
        }
    }

    fn conversation_of(&self, shown: &[Shown]) -> Conversation {
        let says = |prompt: &str| {
            !prompt.is_empty()
                && shown
                    .iter()
                    .any(|s| s.kind == "user_text" && s.text.contains(prompt))
        };
        if says(&self.script.prompt) {
            return Conversation::Main;
        }
        let subagent_prompts = self
            .script
            .responses
            .iter()
            .flat_map(|r| &r.calls)
            .filter(|s| s.kind == "subagent")
            .filter_map(|s| s.prompt.as_deref());
        for prompt in subagent_prompts {
            if says(prompt) {
                return Conversation::Subagent;
            }
        }
        Conversation::Side
    }

    /// The id of call `call` of response `response`, in the harness's API's spelling.
    pub fn id(&self, conversation: Conversation, response: usize, call: usize) -> String {
        let prefix = match self.harness.as_str() {
            "claude" => "toolu",
            _ => "call",
        };
        let who = match conversation {
            Conversation::Subagent => "sub",
            _ => "cs",
        };
        format!("{prefix}_{who}_{response}_{call}")
    }

    /// What the stand-in asked for in the main conversation, response by response, once each.
    pub fn asked(&self) -> Vec<Asked> {
        let heard = self.heard.lock().expect("the stand-in's log");
        let mut given: Vec<(usize, Asked)> = Vec::new();
        for h in heard
            .iter()
            .filter(|h| h.conversation == Conversation::Main)
        {
            if let Some(at) = h.response.filter(|&at| at < self.script.responses.len())
                && !given.iter().any(|(g, _)| *g == at)
            {
                given.push((
                    at,
                    Asked {
                        turn: 1,
                        parallel: self.script.responses[at].parallel,
                        calls: h.asked.clone(),
                    },
                ));
            }
        }
        given.into_iter().map(|(_, asked)| asked).collect()
    }

    /// What the model was shown across every request of the chat and its sub-agents, once each.
    pub fn shown(&self) -> Vec<Shown> {
        let heard = self.heard.lock().expect("the stand-in's log");
        let mut shown = Vec::new();
        for h in heard
            .iter()
            .filter(|h| h.conversation != Conversation::Side)
        {
            crate::projection::extend_once(&mut shown, h.shown.clone());
        }
        shown
    }
}

/// One call as the harness's API asks for it.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeCall {
    pub name: String,
    pub input: Value,
    pub id: String,
    /// Whether it goes as a free-form tool's text (Codex's `apply_patch`), not as JSON.
    pub custom: bool,
}

/// The tools `body` offers.
fn offers(api: Api, body: &Value) -> Vec<Offer> {
    let tools = body["tools"].as_array().map_or(&[][..], Vec::as_slice);
    tools
        .iter()
        .filter_map(|tool| {
            let name = match api {
                Api::ChatCompletions => tool["function"]["name"].as_str(),
                _ => tool["name"].as_str(),
            }
            .or_else(|| tool["type"].as_str())?;
            Some(Offer {
                name: name.to_owned(),
                custom: tool["type"] == "custom",
            })
        })
        .collect()
}

/// Every tool-use id `body` carries a result for.
fn results_in(api: Api, body: &Value) -> Vec<String> {
    project(api, body)
        .into_iter()
        .filter(|s| s.kind == "tool_result")
        .filter_map(|s| s.tool_use_id)
        .collect()
}

/// `step` as the harness's API asks for it, with the tool the request offers for its kind.
pub fn native_for(api: Api, step: &Step, offered: &[(&str, bool)], id: String) -> NativeCall {
    let offers: Vec<Offer> = offered
        .iter()
        .map(|(name, custom)| Offer {
            name: (*name).to_owned(),
            custom: *custom,
        })
        .collect();
    native(api, step, &offers, id)
}

fn native(api: Api, step: &Step, offers: &[Offer], id: String) -> NativeCall {
    let offered = |name: &str| offers.iter().find(|o| o.name == name);
    let text = |s: &Option<String>| s.clone().unwrap_or_default();
    let (name, input, custom) = match (api, step.kind.as_str()) {
        (Api::Messages, "bash") => {
            let mut input = json!({"command": text(&step.command), "description": step.step});
            if step.background {
                input["run_in_background"] = Value::Bool(true);
            }
            ("Bash".to_owned(), input, false)
        }
        (Api::Messages, "write") => (
            "Write".to_owned(),
            json!({"file_path": text(&step.path), "content": text(&step.content)}),
            false,
        ),
        (Api::Messages, "edit") => (
            "Edit".to_owned(),
            json!({"file_path": text(&step.path), "old_string": text(&step.old), "new_string": text(&step.new)}),
            false,
        ),
        (Api::Messages, "read") => (
            "Read".to_owned(),
            json!({"file_path": text(&step.path)}),
            false,
        ),
        (Api::Messages, "web") => (
            "WebFetch".to_owned(),
            json!({"url": text(&step.url), "prompt": "Say what the page says."}),
            false,
        ),
        (Api::Messages, "subagent") => (
            if offered("Agent").is_some() {
                "Agent"
            } else {
                "Task"
            }
            .to_owned(),
            json!({"description": step.step, "prompt": text(&step.prompt), "subagent_type": "general-purpose"}),
            false,
        ),
        (Api::ChatCompletions, "bash") => (
            "bash".to_owned(),
            json!({"command": text(&step.command), "description": step.step}),
            false,
        ),
        (Api::ChatCompletions, "write") => (
            "write".to_owned(),
            json!({"filePath": text(&step.path), "content": text(&step.content)}),
            false,
        ),
        (Api::ChatCompletions, "edit") => (
            "edit".to_owned(),
            json!({"filePath": text(&step.path), "oldString": text(&step.old), "newString": text(&step.new)}),
            false,
        ),
        (Api::ChatCompletions, "read") => (
            "read".to_owned(),
            json!({"filePath": text(&step.path)}),
            false,
        ),
        (Api::ChatCompletions, "web") => (
            "webfetch".to_owned(),
            json!({"url": text(&step.url), "format": "text"}),
            false,
        ),
        (Api::ChatCompletions, "subagent") => (
            "task".to_owned(),
            json!({"description": step.step, "prompt": text(&step.prompt), "subagent_type": "general"}),
            false,
        ),
        (Api::Responses, "bash") => {
            let command = text(&step.command);
            if offered("shell_command").is_some() {
                (
                    "shell_command".to_owned(),
                    json!({"command": command}),
                    false,
                )
            } else if offered("exec_command").is_some() {
                ("exec_command".to_owned(), json!({"cmd": command}), false)
            } else {
                (
                    "shell".to_owned(),
                    json!({"command": ["bash", "-lc", command]}),
                    false,
                )
            }
        }
        (Api::Responses, "write" | "edit") => {
            let patch = if step.kind == "write" {
                format!(
                    "*** Begin Patch\n*** Add File: {}\n+{}\n*** End Patch\n",
                    text(&step.path),
                    text(&step.content)
                )
            } else {
                format!(
                    "*** Begin Patch\n*** Update File: {}\n@@\n-{}\n+{}\n*** End Patch\n",
                    text(&step.path),
                    text(&step.old),
                    text(&step.new)
                )
            };
            // Its own tool where it offers one; else through its shell, as its instructions tell
            // a model of its own to (`apply_patch` and the patch), which Codex takes over and
            // applies itself (0.169.0, a custom provider's model).
            let heredoc = format!("apply_patch <<'PATCH'\n{patch}PATCH\n");
            match (
                offered("apply_patch"),
                offered("exec_command"),
                offered("shell_command"),
            ) {
                (Some(o), _, _) if !o.custom => {
                    ("apply_patch".to_owned(), json!({"input": patch}), false)
                }
                (Some(_), _, _) => ("apply_patch".to_owned(), Value::String(patch), true),
                (None, Some(_), _) => ("exec_command".to_owned(), json!({"cmd": heredoc}), false),
                (None, None, Some(_)) => (
                    "shell_command".to_owned(),
                    json!({"command": heredoc}),
                    false,
                ),
                (None, None, None) => (
                    "shell".to_owned(),
                    json!({"command": ["apply_patch", patch]}),
                    false,
                ),
            }
        }
        (_, kind) => (
            format!("unmapped-{kind}"),
            json!({"step": step.step}),
            false,
        ),
    };
    NativeCall {
        name,
        input,
        id,
        custom,
    }
}

fn json_answer(value: &Value) -> Answer {
    Answer {
        status: 200,
        content_type: "application/json",
        body: value.to_string().into_bytes(),
    }
}

fn sse(name: Option<&str>, data: &Value) -> String {
    match name {
        Some(name) => format!("event: {name}\ndata: {data}\n\n"),
        None => format!("data: {data}\n\n"),
    }
}

/// The response asking for `calls`, or ending the turn with `words`, in `api`'s shape.
pub fn encode(
    api: Api,
    model: &str,
    stream: bool,
    calls: &[NativeCall],
    words: Option<&str>,
) -> Answer {
    match api {
        Api::Messages => messages(model, stream, calls, words),
        Api::Responses => responses(model, stream, calls, words),
        Api::ChatCompletions => chat(model, stream, calls, words),
    }
}

fn messages(model: &str, stream: bool, calls: &[NativeCall], words: Option<&str>) -> Answer {
    let usage = json!({"input_tokens": 1, "output_tokens": 1});
    let stop = if calls.is_empty() {
        "end_turn"
    } else {
        "tool_use"
    };
    let blocks: Vec<Value> = match words {
        Some(words) => vec![json!({"type": "text", "text": words})],
        None => calls
            .iter()
            .map(|c| json!({"type": "tool_use", "id": c.id, "name": c.name, "input": c.input}))
            .collect(),
    };
    if !stream {
        return json_answer(&json!({
            "id": "msg_stand_in", "type": "message", "role": "assistant", "model": model,
            "content": blocks, "stop_reason": stop, "stop_sequence": null, "usage": usage,
        }));
    }
    let mut body = sse(
        Some("message_start"),
        &json!({"type": "message_start", "message": {
            "id": "msg_stand_in", "type": "message", "role": "assistant", "model": model,
            "content": [], "stop_reason": null, "stop_sequence": null, "usage": usage}}),
    );
    for (index, block) in blocks.iter().enumerate() {
        let (start, delta) = if block["type"] == "tool_use" {
            (
                json!({"type": "tool_use", "id": block["id"], "name": block["name"], "input": {}}),
                json!({"type": "input_json_delta", "partial_json": block["input"].to_string()}),
            )
        } else {
            (
                json!({"type": "text", "text": ""}),
                json!({"type": "text_delta", "text": block["text"]}),
            )
        };
        body += &sse(
            Some("content_block_start"),
            &json!({"type": "content_block_start", "index": index, "content_block": start}),
        );
        body += &sse(
            Some("content_block_delta"),
            &json!({"type": "content_block_delta", "index": index, "delta": delta}),
        );
        body += &sse(
            Some("content_block_stop"),
            &json!({"type": "content_block_stop", "index": index}),
        );
    }
    body += &sse(
        Some("message_delta"),
        &json!({"type": "message_delta", "delta": {"stop_reason": stop, "stop_sequence": null},
            "usage": {"output_tokens": 1}}),
    );
    body += &sse(Some("message_stop"), &json!({"type": "message_stop"}));
    Answer {
        status: 200,
        content_type: "text/event-stream",
        body: body.into_bytes(),
    }
}

fn responses(model: &str, stream: bool, calls: &[NativeCall], words: Option<&str>) -> Answer {
    let items: Vec<Value> = match words {
        Some(words) => vec![json!({
            "type": "message", "id": "msg_stand_in", "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": words, "annotations": []}]})],
        None => calls
            .iter()
            .map(|c| {
                if c.custom {
                    json!({"type": "custom_tool_call", "id": format!("ctc_{}", c.id),
                        "call_id": c.id, "name": c.name, "input": c.input, "status": "completed"})
                } else {
                    json!({"type": "function_call", "id": format!("fc_{}", c.id),
                        "call_id": c.id, "name": c.name, "arguments": c.input.to_string(),
                        "status": "completed"})
                }
            })
            .collect(),
    };
    let usage = json!({"input_tokens": 1, "input_tokens_details": {"cached_tokens": 0},
        "output_tokens": 1, "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 2});
    let whole = json!({"id": "resp_stand_in", "object": "response", "model": model,
        "status": "completed", "output": items, "usage": usage});
    if !stream {
        return json_answer(&whole);
    }
    let mut body = sse(
        Some("response.created"),
        &json!({"type": "response.created", "response": {"id": "resp_stand_in", "object": "response",
            "model": model, "status": "in_progress", "output": []}}),
    );
    for (index, item) in items.iter().enumerate() {
        body += &sse(
            Some("response.output_item.added"),
            &json!({"type": "response.output_item.added", "output_index": index, "item": item}),
        );
        body += &sse(
            Some("response.output_item.done"),
            &json!({"type": "response.output_item.done", "output_index": index, "item": item}),
        );
    }
    body += &sse(
        Some("response.completed"),
        &json!({"type": "response.completed", "response": whole}),
    );
    Answer {
        status: 200,
        content_type: "text/event-stream",
        body: body.into_bytes(),
    }
}

fn chat(model: &str, stream: bool, calls: &[NativeCall], words: Option<&str>) -> Answer {
    let tool_calls: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(index, c)| {
            json!({"index": index, "id": c.id, "type": "function",
                "function": {"name": c.name, "arguments": c.input.to_string()}})
        })
        .collect();
    let finish = if calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    let usage = json!({"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2});
    if !stream {
        let mut message = json!({"role": "assistant", "content": words});
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        return json_answer(
            &json!({"id": "chatcmpl_stand_in", "object": "chat.completion",
            "created": 0, "model": model,
            "choices": [{"index": 0, "message": message, "finish_reason": finish}],
            "usage": usage}),
        );
    }
    let chunk = |delta: Value, finish: Option<&str>| {
        sse(
            None,
            &json!({"id": "chatcmpl_stand_in", "object": "chat.completion.chunk", "created": 0,
                "model": model, "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}),
        )
    };
    let mut body = match words {
        Some(words) => chunk(json!({"role": "assistant", "content": words}), None),
        None => chunk(
            json!({"role": "assistant", "content": null, "tool_calls": tool_calls}),
            None,
        ),
    };
    body += &chunk(json!({}), Some(finish));
    body += &sse(
        None,
        &json!({"id": "chatcmpl_stand_in", "object": "chat.completion.chunk", "created": 0,
            "model": model, "choices": [], "usage": usage}),
    );
    body += "data: [DONE]\n\n";
    Answer {
        status: 200,
        content_type: "text/event-stream",
        body: body.into_bytes(),
    }
}

/// Serves `stand_in` on a loopback port until [`AT_MOST`] has passed, saying its port to `port`
/// once it listens.
pub fn serve(stand_in: Arc<StandIn>, port: impl FnOnce(u16)) -> std::io::Result<()> {
    let server = TcpListener::bind(("127.0.0.1", 0))?;
    port(server.local_addr()?.port());
    server.set_nonblocking(true)?;
    let ends = Instant::now() + AT_MOST;
    while Instant::now() < ends {
        match server.accept() {
            Ok((stream, _)) => {
                let stand_in = Arc::clone(&stand_in);
                std::thread::spawn(move || {
                    let _ = connection(stream, &stand_in);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Answers every request one connection carries.
fn connection(stream: TcpStream, stand_in: &StandIn) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut first = String::new();
        if reader.read_line(&mut first)? == 0 {
            return Ok(());
        }
        let mut words = first.split_whitespace();
        let (method, path) = (
            words.next().unwrap_or_default().to_owned(),
            words.next().unwrap_or_default().to_owned(),
        );
        let (mut length, mut chunked, mut close) = (0usize, false, false);
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                let value = value.trim();
                match name.to_ascii_lowercase().as_str() {
                    "content-length" => length = value.parse().unwrap_or(0),
                    "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
                    "connection" => close = value.eq_ignore_ascii_case("close"),
                    _ => {}
                }
            }
        }
        let body = if chunked {
            read_chunked(&mut reader)?
        } else {
            let mut body = vec![0; length];
            reader.read_exact(&mut body)?;
            body
        };
        let answer = stand_in.answer(&method, &path, &body);
        let head = format!(
            "HTTP/1.1 {} {}\r\ncontent-type: {}\r\ncontent-length: {}\r\ncache-control: no-cache\r\n\r\n",
            answer.status,
            if answer.status == 200 { "OK" } else { "Error" },
            answer.content_type,
            answer.body.len()
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(&answer.body)?;
        stream.flush()?;
        if close {
            return Ok(());
        }
    }
}

fn read_chunked(reader: &mut impl BufRead) -> std::io::Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let mut size = String::new();
        reader.read_line(&mut size)?;
        let size =
            usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
        if size == 0 {
            let mut end = String::new();
            reader.read_line(&mut end)?;
            return Ok(body);
        }
        let mut chunk = vec![0; size + 2];
        reader.read_exact(&mut chunk)?;
        chunk.truncate(size);
        body.extend(chunk);
    }
}
