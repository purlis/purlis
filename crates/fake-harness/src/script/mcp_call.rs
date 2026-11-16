// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An MCP tool call, as the harness makes it**: the server entry purlis wrote for the dialect
//! is read where the harness reads it (Claude Code's `--mcp-config`, Codex's
//! `-c mcp_servers.<name>=`, OpenCode's `OPENCODE_CONFIG_CONTENT`), its program started on stdio
//! inside whatever confines the fake, and asked `initialize` then `tools/call`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::dialect::Dialect;

/// A server's program, its words, and the variables it is handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub command: String,
    pub args: Vec<String>,
    /// Variables set for it, beside the fake's own (Claude Code's `env`).
    pub env: Vec<(String, String)>,
    /// Where the harness hands the server only the variables it names (Codex's `env_vars`).
    pub only: Option<Vec<String>>,
}

/// The server named `name`, as `dialect` was handed it.
pub fn server(dialect: Dialect, name: &str, argv: &[String]) -> Option<Server> {
    match dialect {
        Dialect::Claude => {
            let at = argv.iter().position(|a| a == "--mcp-config")?;
            argv[at + 1..]
                .iter()
                .take_while(|a| !a.starts_with("--"))
                .find_map(|value| {
                    let text = if value.trim_start().starts_with('{') {
                        value.clone()
                    } else {
                        std::fs::read_to_string(value).ok()?
                    };
                    let doc: Value = serde_json::from_str(&text).ok()?;
                    let entry = doc.get("mcpServers")?.get(name)?;
                    Some(Server {
                        command: entry.get("command")?.as_str()?.to_owned(),
                        args: strings(entry.get("args")),
                        env: entry
                            .get("env")
                            .and_then(Value::as_object)
                            .map(|env| {
                                env.iter()
                                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        only: None,
                    })
                })
        }
        Dialect::Codex => argv.windows(2).find_map(|pair| {
            if pair[0] != "-c" {
                return None;
            }
            let value = pair[1].strip_prefix(&format!("mcp_servers.{name}="))?;
            let table = format!("v = {value}").parse::<toml::Table>().ok()?;
            let entry = serde_json::to_value(&table["v"]).ok()?;
            Some(Server {
                command: entry.get("command")?.as_str()?.to_owned(),
                args: strings(entry.get("args")),
                env: Vec::new(),
                only: Some(strings(entry.get("env_vars"))),
            })
        }),
        Dialect::Opencode => {
            let config: Value =
                serde_json::from_str(&std::env::var(purlis_core::opencode::CONFIG_ENV).ok()?)
                    .ok()?;
            let words = strings(config.get("mcp")?.get(name)?.get("command"));
            let (command, args) = words.split_first()?;
            Some(Server {
                command: command.clone(),
                args: args.to_vec(),
                env: Vec::new(),
                only: None,
            })
        }
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// What a `tools/call` came back with: its text, and whether the server called it an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub text: String,
    pub is_error: bool,
}

/// `tool` called with `arguments` on `server`, waited on at most `patience`.
pub fn call(
    server: &Server,
    tool: &str,
    arguments: &Value,
    patience: Duration,
) -> Result<Answered, String> {
    let mut command = Command::new(&server.command);
    command.args(&server.args);
    if let Some(only) = &server.only {
        command.env_clear();
        for name in only.iter().chain(["PATH", "HOME"].map(str::to_owned).iter()) {
            if let Ok(value) = std::env::var(name) {
                command.env(name, value);
            }
        }
    }
    let mut child = command
        .envs(server.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start the MCP server {}: {e}", server.command))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines_tx.send(line).is_err() {
                return;
            }
        }
    });
    let deadline = Instant::now() + patience;
    let mut send = |message: Value| writeln!(stdin, "{message}").and_then(|()| stdin.flush());
    let answer = |id: u64| -> Result<Value, String> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = lines_rx
                .recv_timeout(left)
                .map_err(|_| format!("the MCP server did not answer request {id}"))?;
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                return Ok(message);
            }
        }
    };
    let result = (|| {
        send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "fake-harness", "version": env!("CARGO_PKG_VERSION")}
        }}))
        .map_err(|e| e.to_string())?;
        answer(1)?;
        send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .map_err(|e| e.to_string())?;
        send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": tool, "arguments": arguments
        }}))
        .map_err(|e| e.to_string())?;
        let answered = answer(2)?;
        if let Some(error) = answered.get("error") {
            return Ok(Answered {
                text: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                is_error: true,
            });
        }
        let result = answered.get("result").cloned().unwrap_or_default();
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| c.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Answered {
            text,
            is_error: result.get("isError").and_then(Value::as_bool) == Some(true),
        })
    })();
    drop(send);
    let _ = child.kill();
    let _ = child.wait();
    result
}
