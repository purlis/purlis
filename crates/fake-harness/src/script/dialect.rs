// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What each harness calls things and hands its hooks**, as T1.1-03's recordings show it:
//! Claude Code 2.1.325, Codex 0.169.0 and OpenCode 1.19.5 under purlis's wrap at Full. The
//! fidelity test (`the_fake_harness_reproduces_each_recorded_harness_session`) holds every
//! shape here to a recording.

use serde_json::{Value, json};

use super::turns::{Call, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dialect {
    Claude,
    Codex,
    Opencode,
}

impl Dialect {
    pub fn parse(name: &str) -> Option<Dialect> {
        match name {
            "claude" | "claude-code" => Some(Dialect::Claude),
            "codex" => Some(Dialect::Codex),
            "opencode" => Some(Dialect::Opencode),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dialect::Claude => "claude",
            Dialect::Codex => "codex",
            Dialect::Opencode => "opencode",
        }
    }

    /// What the program answers `--version` with, as the real one does.
    pub fn version_line(self, version: &str) -> String {
        match self {
            Dialect::Claude => format!("{version} (Claude Code)"),
            Dialect::Codex => format!("codex-cli {version}"),
            Dialect::Opencode => version.to_owned(),
        }
    }

    /// The model it reports to its hooks, where it reports one, unless told another.
    pub fn default_model(self) -> &'static str {
        match self {
            Dialect::Claude => "claude-opus-5-5",
            Dialect::Codex => "gpt-5.5",
            Dialect::Opencode => "stand-in",
        }
    }

    /// Whether the session's start is reported before the first prompt (Claude Code), or only
    /// inside the first turn (Codex, OpenCode: `reports_its_start_before_the_first_prompt`).
    pub fn starts_before_the_first_prompt(self) -> bool {
        self == Dialect::Claude
    }

    /// A new session's id, in the harness's own shape.
    pub fn new_session_id(self) -> String {
        match self {
            Dialect::Claude | Dialect::Codex => uuid(),
            Dialect::Opencode => format!("ses_{}", alnum(26)),
        }
    }

    /// A tool call's id, in the harness's own shape.
    pub fn new_call_id(self) -> String {
        match self {
            Dialect::Claude => format!("toolu_{}", alnum(24)),
            Dialect::Codex | Dialect::Opencode => format!("call_{}", alnum(24)),
        }
    }

    /// The tool the model calls for `call`, by the dialect's name, and its input: a neutral
    /// [`Kind`] turned into the dialect's own tool, or the dialect's own name as given. `None`
    /// where the dialect offers no such tool.
    pub fn tool_of(self, call: &Call) -> Option<(String, Value)> {
        let Some(kind) = Kind::of(&call.tool) else {
            return Some((call.tool.clone(), call.input.clone()));
        };
        let command = || {
            call.run
                .clone()
                .or_else(|| call.field(&["command", "cmd"]))
                .unwrap_or_default()
        };
        let path = || {
            call.field(&["path", "file_path", "filePath"])
                .unwrap_or_default()
        };
        let content = || call.field(&["content"]).unwrap_or_default();
        let old = || {
            call.field(&["old", "old_string", "oldString"])
                .unwrap_or_default()
        };
        let new = || {
            call.field(&["new", "new_string", "newString"])
                .unwrap_or_default()
        };
        let url = || call.field(&["url"]).unwrap_or_default();
        let step = call.step.clone().unwrap_or_default();
        let prompt = || call.field(&["prompt"]).unwrap_or_default();
        let description = || call.field(&["description"]).unwrap_or_else(|| step.clone());
        Some(match (self, kind) {
            (_, Kind::Mcp) => {
                let name = call.field(&["name", "tool"]).unwrap_or_default();
                let arguments = call.input.get("arguments").cloned().unwrap_or(json!({}));
                (self.mcp_name(&name), arguments)
            }
            (Dialect::Claude, Kind::Bash) => (
                "Bash".to_owned(),
                with_background(
                    json!({"command": command(), "description": description()}),
                    call.background,
                ),
            ),
            (Dialect::Claude, Kind::Write) => (
                "Write".to_owned(),
                json!({"file_path": path(), "content": content()}),
            ),
            (Dialect::Claude, Kind::Edit) => (
                "Edit".to_owned(),
                json!({"file_path": path(), "old_string": old(), "new_string": new()}),
            ),
            (Dialect::Claude, Kind::Read) => ("Read".to_owned(), json!({"file_path": path()})),
            (Dialect::Claude, Kind::Web) => (
                "WebFetch".to_owned(),
                json!({"url": url(), "prompt": "Say what the page says."}),
            ),
            (Dialect::Claude, Kind::Subagent) => (
                "Agent".to_owned(),
                json!({"description": description(), "prompt": prompt(), "subagent_type": "general-purpose"}),
            ),
            (Dialect::Codex, Kind::Bash) => ("exec_command".to_owned(), json!({"cmd": command()})),
            (Dialect::Codex, Kind::Write) => (
                "apply_patch".to_owned(),
                Value::String(format!(
                    "*** Begin Patch\n*** Add File: {}\n{}*** End Patch\n",
                    path(),
                    content()
                        .lines()
                        .map(|line| format!("+{line}\n"))
                        .collect::<String>()
                )),
            ),
            (Dialect::Codex, Kind::Edit) => (
                "apply_patch".to_owned(),
                Value::String(format!(
                    "*** Begin Patch\n*** Update File: {}\n@@\n-{}\n+{}\n*** End Patch\n",
                    path(),
                    old(),
                    new()
                )),
            ),
            // Codex offers no tool of its own to read a file or fetch a page, and no sub-agent
            // the stand-in can ask for.
            (Dialect::Codex, Kind::Read | Kind::Web | Kind::Subagent) => return None,
            (Dialect::Opencode, Kind::Bash) => (
                "bash".to_owned(),
                json!({"command": command(), "description": description()}),
            ),
            (Dialect::Opencode, Kind::Write) => (
                "write".to_owned(),
                json!({"filePath": path(), "content": content()}),
            ),
            (Dialect::Opencode, Kind::Edit) => (
                "edit".to_owned(),
                json!({"filePath": path(), "oldString": old(), "newString": new()}),
            ),
            (Dialect::Opencode, Kind::Read) => ("read".to_owned(), json!({"filePath": path()})),
            (Dialect::Opencode, Kind::Web) => (
                "webfetch".to_owned(),
                json!({"url": url(), "format": "text"}),
            ),
            (Dialect::Opencode, Kind::Subagent) => (
                "task".to_owned(),
                json!({"description": description(), "prompt": prompt(), "subagent_type": "general"}),
            ),
        })
    }

    /// An MCP tool's name as the model calls it: `mcp__<server>__<tool>` in Claude Code and
    /// Codex, `<server>_<tool>` in OpenCode.
    pub fn mcp_name(self, name: &str) -> String {
        let (server, tool) = split_mcp(name).unwrap_or(("purlis", name));
        match self {
            Dialect::Claude | Dialect::Codex => format!("mcp__{server}__{tool}"),
            Dialect::Opencode => format!("{server}_{tool}"),
        }
    }

    /// What a dialect's tool does, by its own name.
    pub fn action_of(self, tool: &str, input: &Value) -> Action {
        if let Some((server, name)) = self.mcp_parts(tool) {
            return Action::Mcp {
                server: server.to_owned(),
                tool: name.to_owned(),
            };
        }
        let str_of = |key: &str| input.get(key).and_then(Value::as_str).unwrap_or_default();
        match (self, tool) {
            (Dialect::Claude, "Bash") | (Dialect::Opencode, "bash") => {
                Action::Shell(str_of("command").to_owned())
            }
            (Dialect::Codex, "exec_command") => Action::Shell(str_of("cmd").to_owned()),
            (Dialect::Codex, "shell_command") => Action::Shell(str_of("command").to_owned()),
            (Dialect::Claude, "Write") => Action::Write {
                path: str_of("file_path").to_owned(),
                content: str_of("content").to_owned(),
            },
            (Dialect::Opencode, "write") => Action::Write {
                path: str_of("filePath").to_owned(),
                content: str_of("content").to_owned(),
            },
            (Dialect::Claude, "Edit") => Action::Edit {
                path: str_of("file_path").to_owned(),
                old: str_of("old_string").to_owned(),
                new: str_of("new_string").to_owned(),
            },
            (Dialect::Opencode, "edit") => Action::Edit {
                path: str_of("filePath").to_owned(),
                old: str_of("oldString").to_owned(),
                new: str_of("newString").to_owned(),
            },
            (Dialect::Codex, "apply_patch") => patch_action(input.as_str().unwrap_or_default()),
            (Dialect::Claude, "Read") => Action::Read(str_of("file_path").to_owned()),
            (Dialect::Opencode, "read") => Action::Read(str_of("filePath").to_owned()),
            (Dialect::Claude, "WebFetch") | (Dialect::Opencode, "webfetch") => {
                Action::Web(str_of("url").to_owned())
            }
            (Dialect::Claude, "Agent" | "Task") | (Dialect::Opencode, "task") => Action::Subagent,
            _ => Action::Unknown,
        }
    }

    fn mcp_parts(self, tool: &str) -> Option<(&str, &str)> {
        match self {
            Dialect::Claude | Dialect::Codex => split_mcp(tool),
            // OpenCode names an MCP tool `<server>_<tool>`; purlis's is the one server it is
            // handed (`OPENCODE_CONFIG_CONTENT`'s `mcp`).
            Dialect::Opencode => tool.strip_prefix("purlis_").map(|t| ("purlis", t)),
        }
    }

    /// The tool's name and input a hook is handed for a call of `tool` with `input`: Codex
    /// hands its shell tools to a hook as `Bash` with `command` (`HookToolName::bash`).
    pub fn hook_tool(self, tool: &str, input: &Value) -> (String, Value) {
        match (self, tool) {
            (Dialect::Codex, "exec_command") => (
                "Bash".to_owned(),
                json!({"command": input.get("cmd").cloned().unwrap_or(Value::Null)}),
            ),
            (Dialect::Codex, "shell_command") => (
                "Bash".to_owned(),
                json!({"command": input.get("command").cloned().unwrap_or(Value::Null)}),
            ),
            (Dialect::Codex, "apply_patch") => (
                "apply_patch".to_owned(),
                json!({"command": input.as_str().unwrap_or_default()}),
            ),
            (Dialect::Claude, "Edit") => {
                let mut input = input.clone();
                if let Some(map) = input.as_object_mut() {
                    map.entry("replace_all").or_insert(Value::Bool(false));
                }
                (tool.to_owned(), input)
            }
            _ => (tool.to_owned(), input.clone()),
        }
    }
}

/// What a call does, whichever dialect asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Shell(String),
    Write {
        path: String,
        content: String,
    },
    Edit {
        path: String,
        old: String,
        new: String,
    },
    Read(String),
    Web(String),
    Subagent,
    Mcp {
        server: String,
        tool: String,
    },
    Unknown,
}

fn with_background(mut input: Value, background: bool) -> Value {
    if background && let Some(map) = input.as_object_mut() {
        map.insert("run_in_background".to_owned(), Value::Bool(true));
    }
    input
}

fn split_mcp(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    rest.split_once("__")
}

/// The one file an `apply_patch` adds or updates, as Codex's own patch reads it.
fn patch_action(patch: &str) -> Action {
    let mut lines = patch.lines();
    while let Some(line) = lines.next() {
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let content: String = lines
                .by_ref()
                .take_while(|l| !l.starts_with("*** "))
                .filter_map(|l| l.strip_prefix('+'))
                .map(|l| format!("{l}\n"))
                .collect();
            return Action::Write {
                path: path.to_owned(),
                content,
            };
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            let (mut old, mut new) = (String::new(), String::new());
            for l in lines.by_ref().take_while(|l| !l.starts_with("*** ")) {
                if let Some(o) = l.strip_prefix('-') {
                    old.push_str(o);
                } else if let Some(n) = l.strip_prefix('+') {
                    new.push_str(n);
                }
            }
            return Action::Edit {
                path: path.to_owned(),
                old,
                new,
            };
        }
    }
    Action::Unknown
}

/// `n` random characters of `[0-9A-Za-z]`.
pub fn alnum(n: usize) -> String {
    const SET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    random_bytes(n)
        .into_iter()
        .map(|b| SET[usize::from(b) % SET.len()] as char)
        .collect()
}

/// `n` random lowercase characters of `[0-9a-z]`.
pub fn lower_alnum(n: usize) -> String {
    const SET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    random_bytes(n)
        .into_iter()
        .map(|b| SET[usize::from(b) % SET.len()] as char)
        .collect()
}

/// A version 4 UUID.
pub fn uuid() -> String {
    let mut b = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn random_bytes(n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut out = vec![0u8; n];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut out))
        .is_err()
    {
        // No `/dev/urandom` under a wrap that hides it: the clock and the pid will do for ids.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
            ^ u128::from(std::process::id());
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = (seed >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(31);
        }
    }
    out
}
