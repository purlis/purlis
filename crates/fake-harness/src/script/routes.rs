// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which hook runs on what, and how long it is waited on**, for each dialect, derived from
//! purlis-core and never copied (T1.1-04 item 3):
//!
//! - **Claude Code:** the generated plugin's `hooks.json` ([`purlis_core::plugin::hooks_json`],
//!   out of [`purlis_core::hookreg::HANDLERS`]), and the `PermissionRequest` hook purlis arms on
//!   `--settings` ([`purlis_core::harness::hooked::WORD`]).
//! - **Codex:** the `-c hooks.<Event>=` words it was started with, or, started without them,
//!   the ones purlis-core arms a Codex chat with ([`purlis_core::harness::Harness::state_hooks`],
//!   out of [`purlis_core::plugin::CODEX`]).
//! - **OpenCode:** the shim's table, [`purlis_core::opencode::TOOLS`], its default guard and its
//!   session hooks, with each word's timeout from the registry, as `opencode::shim` writes them.
//!
//! [`FIRES`] is what the fake knows how to fire, per dialect. A hook armed on an event the fake
//! does not fire is a test's failure (`the_fake_harness_routes_come_from_purlis_core_for_every_dialect`):
//! purlis-core gaining a hook the fake does not follow.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::dialect::Dialect;

/// One hook a dialect runs: on `event`, for the tools `matcher` names (or every one), the
/// command and its word, waited on `timeout` seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Route {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub word: String,
    pub timeout: u64,
    /// The command a shell runs (Claude Code, Codex), or the program's words without a shell
    /// (OpenCode's shim spawns `[BINARY, "hook", word]`).
    pub command: String,
    /// The tool's name in the payload, where the dialect hands a hook another name than its
    /// own (OpenCode's `bash` is `Bash`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

/// The events the fake fires, per dialect. A hook armed on another is never run.
pub const FIRES: [(Dialect, &[&str]); 3] = [
    (
        Dialect::Claude,
        &[
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "Stop",
            "SubagentStop",
            "SessionEnd",
            "Notification",
            "PermissionRequest",
            "PermissionDenied",
            "UserPromptExpansion",
            "StopFailure",
        ],
    ),
    (
        Dialect::Codex,
        &[
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "Stop",
            "SubagentStop",
            "SessionEnd",
            "PermissionRequest",
        ],
    ),
    (
        Dialect::Opencode,
        &[
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "Stop",
            "Notification",
            "PermissionRequest",
        ],
    ),
];

pub fn fires(dialect: Dialect) -> &'static [&'static str] {
    FIRES
        .iter()
        .find(|(d, _)| *d == dialect)
        .map_or(&[], |(_, events)| events)
}

/// The routes of `dialect`, armed with `binary`; `argv` is what the fake was started with, read
/// for Codex's `-c hooks.<Event>=` words.
pub fn of(dialect: Dialect, binary: &Path, argv: &[String]) -> Vec<Route> {
    match dialect {
        Dialect::Claude => claude(binary),
        Dialect::Codex => {
            let armed = codex_from_words(argv);
            if armed.is_empty() {
                codex(binary)
            } else {
                armed
            }
        }
        Dialect::Opencode => opencode(binary),
    }
}

/// The hooks `hooks.json` holds, read as Claude Code reads them, and the permission hook.
fn claude(binary: &Path) -> Vec<Route> {
    let doc: Value = serde_json::from_str(&purlis_core::plugin::hooks_json()).unwrap_or_default();
    let mut routes = from_hooks_object(doc.get("hooks").unwrap_or(&Value::Null), None);
    routes.push(Route {
        event: "PermissionRequest".to_owned(),
        matcher: None,
        word: purlis_core::harness::hooked::WORD.to_owned(),
        timeout: purlis_core::harness::hooked::HOOK_TIMEOUT.as_secs(),
        command: purlis_core::plugin::command_at(binary, purlis_core::harness::hooked::WORD),
        tool: None,
    });
    routes
}

/// What purlis-core arms a Codex chat with, read off its `-c` words.
fn codex(binary: &Path) -> Vec<Route> {
    let purlis_core::harness::StateHooks::ThisSessionOnly { args, .. } =
        purlis_core::harness::Harness::Codex.state_hooks(
            purlis_core::harness::Kit {
                binary,
                plugin: None,
                persona: None,
                skills: None,
                graded: false,
                judging: false,
                writes_its_piece: false,
                runs_a_shell: false,
                own: &[],
                laid_out: None,
            },
            None,
            &purlis_core::harness_plugin::Chosen::new(),
            None,
        )
    else {
        return Vec::new();
    };
    codex_from_words(&args)
}

/// Each `-c hooks.<Event>=<TOML>` in `argv`, read as Codex reads it.
pub fn codex_from_words(argv: &[String]) -> Vec<Route> {
    let mut routes = Vec::new();
    for pair in argv.windows(2) {
        if pair[0] != "-c" {
            continue;
        }
        let Some((key, value)) = pair[1].split_once('=') else {
            continue;
        };
        let Some(event) = key.strip_prefix("hooks.") else {
            continue;
        };
        let Ok(table) = format!("v = {value}").parse::<toml::Table>() else {
            continue;
        };
        let Ok(groups) = serde_json::to_value(&table["v"]) else {
            continue;
        };
        let mut wrapped = serde_json::Map::new();
        wrapped.insert(event.to_owned(), groups);
        routes.extend(from_hooks_object(&Value::Object(wrapped), None));
    }
    routes
}

/// `{"<Event>": [{"matcher"?, "hooks": [{"command", "timeout"}]}]}`, as both Claude Code and
/// Codex read a hooks table.
fn from_hooks_object(hooks: &Value, tool: Option<&str>) -> Vec<Route> {
    let mut routes = Vec::new();
    let Some(events) = hooks.as_object() else {
        return routes;
    };
    for (event, groups) in events {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .map(str::to_owned);
            for hook in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(command) = hook.get("command").and_then(Value::as_str) else {
                    continue;
                };
                routes.push(Route {
                    event: event.clone(),
                    matcher: matcher.clone(),
                    word: word_of(command),
                    timeout: hook.get("timeout").and_then(Value::as_u64).unwrap_or(600),
                    command: command.to_owned(),
                    tool: tool.map(str::to_owned),
                });
            }
        }
    }
    routes
}

/// The word after `hook` in a hook's command.
fn word_of(command: &str) -> String {
    command
        .rsplit_once(" hook ")
        .map(|(_, word)| word.trim().to_owned())
        .unwrap_or_default()
}

/// The binary the first of Codex's `-c hooks.` commands runs, unquoted.
pub fn binary_in_words(argv: &[String]) -> Option<PathBuf> {
    let route = codex_from_words(argv).into_iter().next()?;
    let (binary, _) = route.command.rsplit_once(" hook ")?;
    let binary = binary.trim();
    let unquoted = binary
        .strip_prefix('\'')
        .and_then(|b| b.strip_suffix('\''))
        .map_or_else(|| binary.to_owned(), |b| b.replace(r"'\''", "'"));
    Some(PathBuf::from(unquoted))
}

/// The shim's routes: each tool's guard before and word after, its default guard, and its
/// session hooks, each waited on the registry's timeout.
fn opencode(binary: &Path) -> Vec<Route> {
    use purlis_core::opencode::{DEFAULT_PRE, TOOLS};
    let timeout = |word: &str| {
        if word == purlis_core::harness::hooked::QUESTION_WORD {
            return purlis_core::harness::hooked::QUESTION_HOOK_TIMEOUT.as_secs();
        }
        if word == purlis_core::harness::hooked::WORD {
            return purlis_core::harness::hooked::HOOK_TIMEOUT.as_secs();
        }
        purlis_core::hookreg::find(word).map_or(SHIM_DEADLINE, |h| u64::from(h.timeout))
    };
    let route = |event: &str, matcher: Option<&str>, word: &str, tool: Option<&str>| Route {
        event: event.to_owned(),
        matcher: matcher.map(str::to_owned),
        word: word.to_owned(),
        timeout: timeout(word),
        command: format!("{} hook {word}", binary.display()),
        tool: tool.map(str::to_owned),
    };
    let mut routes = Vec::new();
    for event in ["SessionStart", "UserPromptSubmit"] {
        routes.push(route(event, None, &event.to_lowercase(), None));
    }
    for tool in TOOLS {
        routes.push(route(
            "PreToolUse",
            Some(tool.id),
            tool.pre.unwrap_or(DEFAULT_PRE),
            Some(tool.name),
        ));
        if let Some(post) = tool.post {
            routes.push(route("PostToolUse", Some(tool.id), post, Some(tool.name)));
        }
    }
    // Any other tool, MCP tools among them, goes to the default guard under its own name.
    routes.push(route("PreToolUse", None, DEFAULT_PRE, None));
    routes.push(route(
        "PreToolUse",
        Some(purlis_core::harness::hooked::OPENCODE_QUESTION_TOOL),
        purlis_core::harness::hooked::QUESTION_WORD,
        None,
    ));
    routes.push(route("Stop", None, "stop", None));
    routes.push(route("Notification", None, "notification", None));
    routes.push(route(
        "PermissionRequest",
        None,
        purlis_core::harness::hooked::WORD,
        None,
    ));
    routes
}

/// What the shim waits on a word the registry does not time (`{{DEADLINE}}`).
const SHIM_DEADLINE: u64 = 10;

/// Whether `tool` is one a hook with `matcher` hears, as Claude Code and Codex read a matcher:
/// none or `*` hears every tool; a matcher of plain names is a list of whole names split on `|`;
/// anything else is a pattern matched whole.
pub fn hears(matcher: Option<&str>, tool: &str) -> bool {
    let Some(matcher) = matcher else {
        return true;
    };
    if matcher.is_empty() || matcher == "*" {
        return true;
    }
    if matcher
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '|')
    {
        return matcher.split('|').any(|name| name == tool);
    }
    regex::Regex::new(&format!("^(?:{matcher})$")).is_ok_and(|re| re.is_match(tool))
}
