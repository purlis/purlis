// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `--model`: a stand-in for the Messages API on a port of this machine's loopback, so a real
//! Claude Code runs a command only a model could ask for (#4083). A chat's first turn is
//! answered with one Bash call of the command named, and every later one, or a request that
//! offers no Bash tool, with an end. What the call's result said goes to the file named. It
//! needs no credential and calls no model.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The longest it stays, so one its test never ended ends by itself.
const AT_MOST: Duration = Duration::from_secs(600);

/// The id of its one Bash call.
const CALL: &str = "toolu_fake_harness";

/// Serves, saying its port on standard output, until it has been up [`AT_MOST`].
pub fn serve(command: &str, said: &Path) -> Result<(), String> {
    let server =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|err| format!("cannot serve: {err}"))?;
    let port = server
        .local_addr()
        .map_err(|err| format!("cannot read its port: {err}"))?
        .port();
    println!("{port}");
    std::io::stdout()
        .flush()
        .map_err(|err| format!("cannot say its port: {err}"))?;
    server
        .set_nonblocking(true)
        .map_err(|err| format!("cannot serve: {err}"))?;
    let ends = Instant::now() + AT_MOST;
    while Instant::now() < ends {
        match server.accept() {
            Ok((stream, _)) => {
                let (command, said) = (command.to_owned(), said.to_path_buf());
                std::thread::spawn(move || {
                    let _ = answer(stream, &command, &said);
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(format!("cannot take a connection: {err}")),
        }
    }
    Ok(())
}

/// Answers every request `stream` carries.
fn answer(stream: TcpStream, command: &str, said: &Path) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        let mut length = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let path = request.split_whitespace().nth(1).unwrap_or_default();
        let path = path.split('?').next().unwrap_or_default();
        let (kind, text) = match path {
            "/v1/messages" => respond(&body, command, said),
            "/v1/messages/count_tokens" => {
                ("application/json", json!({ "input_tokens": 1 }).to_string())
            }
            _ => ("application/json", "{}".to_owned()),
        };
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: {kind}\r\ncontent-length: {}\r\n\r\n{text}",
            text.len()
        )?;
        stream.flush()?;
    }
}

/// What the request `body` is answered with, and its type.
fn respond(body: &Value, command: &str, said: &Path) -> (&'static str, String) {
    let model = body["model"].as_str().unwrap_or("stand-in");
    let stream = body["stream"] == true;
    if let Some(result) = result_in(body) {
        let _ = std::fs::write(said, result);
        let done = json!({ "type": "text", "text": "done" });
        return message(model, &done, "end_turn", stream);
    }
    let offers_bash = body["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "Bash"));
    if !offers_bash {
        let words = json!({ "type": "text", "text": "ok" });
        return message(model, &words, "end_turn", stream);
    }
    let call = json!({
        "type": "tool_use",
        "id": CALL,
        "name": "Bash",
        "input": { "command": command, "description": "Run the probes" },
    });
    message(model, &call, "tool_use", stream)
}

/// The text of the `tool_result` for its one call in `body`, if there is one.
fn result_in(body: &Value) -> Option<String> {
    let mut found = None;
    for block in body["messages"]
        .as_array()?
        .iter()
        .filter_map(|message| message["content"].as_array())
        .flatten()
    {
        if block["type"] != "tool_result" || block["tool_use_id"] != CALL {
            continue;
        }
        found = Some(match &block["content"] {
            Value::String(text) => text.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            other => other.to_string(),
        });
    }
    found
}

/// One server-sent event of the Messages stream.
fn event(name: &str, data: &Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

/// A whole answer, `content` ending on `stop`: streamed when the request asked for a stream.
fn message(model: &str, content: &Value, stop: &str, stream: bool) -> (&'static str, String) {
    let usage = json!({ "input_tokens": 1, "output_tokens": 1 });
    if !stream {
        let whole = json!({
            "id": "msg_stand_in", "type": "message", "role": "assistant", "model": model,
            "content": [content], "stop_reason": stop, "stop_sequence": null, "usage": usage,
        });
        return ("application/json", whole.to_string());
    }
    let (start, delta) = if content["type"] == "tool_use" {
        (
            json!({ "type": "tool_use", "id": content["id"], "name": content["name"], "input": {} }),
            json!({ "type": "input_json_delta", "partial_json": content["input"].to_string() }),
        )
    } else {
        (
            json!({ "type": "text", "text": "" }),
            json!({ "type": "text_delta", "text": content["text"] }),
        )
    };
    let events = [
        event(
            "message_start",
            &json!({ "type": "message_start", "message": {
                "id": "msg_stand_in", "type": "message", "role": "assistant", "model": model,
                "content": [], "stop_reason": null, "stop_sequence": null, "usage": usage,
            }}),
        ),
        event(
            "content_block_start",
            &json!({ "type": "content_block_start", "index": 0, "content_block": start }),
        ),
        event(
            "content_block_delta",
            &json!({ "type": "content_block_delta", "index": 0, "delta": delta }),
        ),
        event(
            "content_block_stop",
            &json!({ "type": "content_block_stop", "index": 0 }),
        ),
        event(
            "message_delta",
            &json!({ "type": "message_delta",
                "delta": { "stop_reason": stop, "stop_sequence": null },
                "usage": { "output_tokens": 1 } }),
        ),
        event("message_stop", &json!({ "type": "message_stop" })),
    ];
    ("text/event-stream", events.concat())
}
