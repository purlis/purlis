// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which skills a chat started, read from its harness's print-mode output.**

use std::path::Path;

use crate::Skill;

/// **A harness the CI grader reads.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    /// `claude -p --output-format stream-json --verbose`.
    ClaudeCode,
    /// `codex exec --json`.
    Codex,
    /// `opencode run --format json`.
    Opencode,
}

/// **What a chat was handed**, for the routes that need it: on Codex, the briefed skills, each by
/// its `SKILL.md`, and the folder the chat ran in, which a read may name a path from.
#[derive(Debug, Clone, Copy)]
pub struct Listing<'a> {
    pub cwd: &'a Path,
    pub briefed: &'a [Skill],
}

/// **The skills a chat started**, each once, by its name without a plugin's namespace, in the
/// order they first started.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Started {
    pub skills: Vec<String>,
}

/// **Output that says nothing about what a chat started**: no run, never a "did not start".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Unread {
    pub why: String,
}

/// **The skills the chat whose output `output` is started**, read by `harness`'s own shapes.
pub fn started(harness: Harness, output: &str, listing: Listing<'_>) -> Result<Started, Unread> {
    let mut calls = Calls::default();
    for (at, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return Err(Unread {
                why: format!(
                    "line {} is not {}'s JSON output",
                    at.saturating_add(1),
                    harness.shown()
                ),
            });
        };
        match harness {
            Harness::ClaudeCode => claude_code(&event, &mut calls),
            Harness::Codex => codex(&event, listing, &mut calls),
            Harness::Opencode => opencode(&event, &mut calls),
        }
    }
    calls.started(harness)
}

impl Harness {
    /// Its name, as a person reads it.
    pub fn shown(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Opencode => "opencode",
        }
    }
}

/// **The calls of a chat that would start a skill**, by the call's id where its harness says
/// one, and the ids whose call failed: a call that failed loaded nothing.
#[derive(Default)]
struct Calls {
    /// Whether the output said the chat began: without that, it says nothing of what it started.
    began: bool,
    starts: Vec<(Option<String>, String)>,
    failed: std::collections::HashSet<String>,
    /// What the harness said went wrong, where it said so.
    error: Option<String>,
}

impl Calls {
    fn start(&mut self, id: Option<&str>, skill: &str) {
        self.starts.push((id.map(str::to_owned), skill.to_owned()));
    }

    /// What the chat started; no run where it never began, or where it failed and started
    /// nothing, as a chat that never ran says nothing of what it would have started.
    fn started(self, harness: Harness) -> Result<Started, Unread> {
        if !self.began {
            return Err(Unread {
                why: format!("{} never said the chat began", harness.shown()),
            });
        }
        let mut started = Started { skills: Vec::new() };
        for (id, skill) in self.starts {
            if !id.is_some_and(|id| self.failed.contains(&id)) {
                started.start(&skill);
            }
        }
        match self.error {
            Some(why) if started.skills.is_empty() => Err(Unread {
                why: format!("{} failed: {why}", harness.shown()),
            }),
            _ => Ok(started),
        }
    }
}

impl Started {
    /// `name` started, as a run records it: without a plugin's namespace, and once.
    fn start(&mut self, name: &str) {
        let name = crate::run_name(name).into_owned();
        if !self.skills.contains(&name) {
            self.skills.push(name);
        }
    }
}

/// purlis's MCP server, as each harness names the tools it serves.
const SERVER: &str = "purlis";

/// purlis's tool that loads a skill by its `name` (SK-13): an exact start on every harness.
const SKILL_LOAD: &str = "skill_load";

/// One line of Claude Code's `stream-json`: an assistant message's `tool_use` blocks, and a
/// user message's `tool_result` blocks that say their call failed.
fn claude_code(event: &serde_json::Value, calls: &mut Calls) {
    let blocks = event["message"]["content"].as_array().into_iter().flatten();
    match event["type"].as_str() {
        Some("system") if event["subtype"] == "init" => calls.began = true,
        Some("result") if event["is_error"] == serde_json::Value::Bool(true) => {
            let why = event["result"].as_str().unwrap_or("its result is an error");
            calls.error = Some(why.to_owned());
        }
        Some("assistant") => {
            for block in blocks.filter(|block| block["type"] == "tool_use") {
                let key = match block["name"].as_str() {
                    Some("Skill") => "skill",
                    Some(tool) if tool == format!("mcp__{SERVER}__{SKILL_LOAD}") => "name",
                    _ => continue,
                };
                if let Some(skill) = block["input"][key].as_str() {
                    calls.start(block["id"].as_str(), skill);
                }
            }
        }
        Some("user") => {
            for block in blocks.filter(|block| {
                block["type"] == "tool_result" && block["is_error"] == serde_json::Value::Bool(true)
            }) {
                if let Some(id) = block["tool_use_id"].as_str() {
                    calls.failed.insert(id.to_owned());
                }
            }
        }
        _ => {}
    }
}

/// One line of Codex's `exec --json`: a finished item that read a briefed `SKILL.md` in a
/// command that only reads, or called `skill_load`.
fn codex(event: &serde_json::Value, listing: Listing<'_>, calls: &mut Calls) {
    match event["type"].as_str() {
        Some("thread.started") => calls.began = true,
        Some("turn.failed") => {
            let why = event["error"]["message"]
                .as_str()
                .unwrap_or("its turn failed");
            calls.error = Some(why.to_owned());
        }
        Some("item.completed") => {
            let item = &event["item"];
            if item["status"] != "completed" {
                return;
            }
            match item["type"].as_str() {
                Some("command_execution") if item["exit_code"] == 0 => {
                    let Some(command) = item["command"].as_str().map(inner_command) else {
                        return;
                    };
                    if crate::runs::is_a_plain_read(command)
                        && let Some(skill) =
                            crate::runs::started_by_a_read(command, listing.cwd, listing.briefed)
                    {
                        calls.start(None, &skill.name);
                    }
                }
                Some("mcp_tool_call") if item["server"] == SERVER && item["tool"] == SKILL_LOAD => {
                    if let Some(skill) = item["arguments"]["name"].as_str() {
                        calls.start(None, skill);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// One line of opencode's `run --format json`: a finished call of its `skill` tool, or of
/// purlis's `skill_load`, which opencode names `<server>_<tool>`.
fn opencode(event: &serde_json::Value, calls: &mut Calls) {
    match event["type"].as_str() {
        Some("step_start") => calls.began = true,
        // As opencode prints it itself: the error's message where it has one, else its name.
        Some("error") => {
            let error = &event["error"];
            let why = error["data"]["message"]
                .as_str()
                .or(error["name"].as_str())
                .unwrap_or("it reported an error");
            calls.error = Some(why.to_owned());
        }
        Some("tool_use") => {
            let part = &event["part"];
            let loads = part["tool"] == "skill"
                || part["tool"].as_str() == Some(&format!("{SERVER}_{SKILL_LOAD}"));
            if loads
                && part["state"]["status"] == "completed"
                && let Some(skill) = part["state"]["input"]["name"].as_str()
            {
                calls.start(None, skill);
            }
        }
        _ => {}
    }
}

/// **The command a shell runs, where `command` is a shell given one with `-c`** (`bash -lc
/// '<command>'`): how Codex writes what its shell ran. Read inside, a command that edits a skill
/// is not a read of it however it is wrapped.
fn inner_command(command: &str) -> &str {
    let mut words = command.trim_start().splitn(3, char::is_whitespace);
    let (Some(program), Some(flags), Some(rest)) = (words.next(), words.next(), words.next())
    else {
        return command;
    };
    let shell = program.rsplit('/').next().unwrap_or(program);
    let runs_one = flags.starts_with('-') && !flags.starts_with("--") && flags.ends_with('c');
    if !matches!(shell, "bash" | "sh" | "zsh") || !runs_one {
        return command;
    }
    let rest = rest.trim();
    ['\'', '"']
        .into_iter()
        .find_map(|quote| {
            rest.strip_prefix(quote)
                .and_then(|inner| inner.strip_suffix(quote))
        })
        .unwrap_or(rest)
}
