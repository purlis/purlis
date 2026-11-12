// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The harness matrix** (ADR 0124, Consequences, "The harness matrix"; R-008, Q88, Q209):
//! each layer, by harness, as the record's table says it, and the test that confirms each cell.
//!
//! The table is copied from ADR 0124 while the record is on its own branch (Q209); phase 1's
//! gate checks it against the record. A cell changes only with its test. A cell this phase does
//! not yet test says which requirement owes its test ([`Test::Owed`]), so the matrix grows
//! honestly at each gate: [`every_harness_matrix_cell_names_a_test_that_exists`] fails on a
//! cell that names neither, and on a named test that does not exist.

use super::*;
use crate::harness::asks::{Admitted, Answerer, Asks};
use crate::harness::hooked::HookAsks;
use crate::harness::model::Channel;
use crate::hookwire::permission::PermissionAsked;
use std::sync::Arc;

/// What confirms one cell.
#[derive(Debug, Clone, Copy)]
enum Test {
    /// A `#[test]` function, by the file it is in (from this crate's root) and its name.
    Named {
        file: &'static str,
        name: &'static str,
    },
    /// Not tested yet: the ledger's requirement (or the phase) whose ticket brings its test.
    Owed { by: &'static str },
}

/// One cell: its value, as ADR 0124's table says it, and what confirms it.
#[derive(Debug, Clone, Copy)]
struct Cell {
    value: &'static str,
    test: Test,
}

/// One layer: its cells for Claude Code, Codex and opencode, in that order.
#[derive(Debug, Clone, Copy)]
struct Row {
    layer: &'static str,
    cells: [Cell; 3],
}

/// The harnesses of each row's cells, in order.
const HARNESSES: [Harness; 3] = [Harness::ClaudeCode, Harness::Codex, Harness::Opencode];

const THIS: &str = "src/harness/tests/matrix.rs";

const fn here(name: &'static str) -> Test {
    Test::Named { file: THIS, name }
}

const fn owed(by: &'static str) -> Test {
    Test::Owed { by }
}

const fn cell(value: &'static str, test: Test) -> Cell {
    Cell { value, test }
}

/// ADR 0124's table, for Basic and Full sandbox (at No sandbox every report is unverified).
const MATRIX: [Row; 10] = [
    Row {
        layer: "Sandbox: files and egress",
        cells: [
            cell(
                "enforced (Bash in the sandbox; file tools by deny rules; web tools denied)",
                owed("ADR 0124 matrix: no requirement names it yet"),
            ),
            cell(
                "enforced (the whole process, its hooks and MCP servers, in purlis's wrap)",
                owed("ADR 0124 matrix: no requirement names it yet"),
            ),
            cell(
                "enforced (the whole process in purlis's wrap)",
                owed("ADR 0124 matrix: no requirement names it yet"),
            ),
        ],
    },
    Row {
        layer: "A guard's block reported",
        cells: [
            cell(
                "enforced (the hook runs outside the chat's reach)",
                owed("R-101"),
            ),
            cell("unverified", owed("R-101")),
            cell("unverified", owed("R-101")),
        ],
    },
    Row {
        layer: "The harness's own check's block reported",
        cells: [
            cell("enforced (`PermissionDenied`)", owed("R-094")),
            cell(
                "Can't tell (no denial signal yet; a hook-reported rejection becomes a block)",
                owed("R-100, R-325"),
            ),
            cell("Can't tell (no denial signal)", owed("R-097")),
        ],
    },
    Row {
        layer: "The harness's prompts and asks reach Asks",
        cells: [
            cell(
                "enforced (the hook)",
                here("a_claude_code_permission_prompt_hook_line_raises_one_ask"),
            ),
            cell(
                "Can't tell (rejected, never asked)",
                here("codex_start_arguments_carry_the_rejecting_approval_policy"),
            ),
            cell(
                "enforced (held in the window)",
                here("an_opencode_permission_ask_is_held_in_the_window_as_one_ask"),
            ),
        ],
    },
    Row {
        layer: "The mode the chat runs in (attendance)",
        cells: [
            cell("enforced (the armed hook)", owed("R-095")),
            cell(
                "Can't tell (bypass reported for every chat; attendance from how purlis started it)",
                owed("R-099"),
            ),
            cell("Can't tell", owed("R-007")),
        ],
    },
    Row {
        layer: "A hold the app performs (merge, push)",
        cells: [
            cell("enforced", owed("R-115, R-116")),
            cell("enforced", owed("R-115, R-116")),
            cell("enforced", owed("R-115, R-116")),
        ],
    },
    // Until phase 3 brings the holds (R-165), each cell is confirmed by what ships: no purlis
    // hook holds a non-purlis MCP tool, so the harness's own check judges it (R-164).
    Row {
        layer: "A non-purlis MCP tool's hold (phase 3)",
        cells: [
            cell(
                "enforced (the hook; the server runs outside the sandbox)",
                here("no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3"),
            ),
            cell(
                "unverified (the hook runs in the wrap)",
                here("no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3"),
            ),
            cell(
                "unverified (the hook runs in the wrap)",
                here("no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3"),
            ),
        ],
    },
    Row {
        layer: "A held built-in tool (phase 3)",
        cells: [
            cell("enforced (the pinned fixture)", owed("R-166, R-168")),
            cell("Can't tell", owed("R-166, R-168")),
            cell("Can't tell", owed("R-166, R-168")),
        ],
    },
    Row {
        layer: "Hosts recorded",
        cells: [
            cell("enforced", owed("R-257")),
            cell("enforced", owed("R-257")),
            cell("enforced", owed("R-257")),
        ],
    },
    Row {
        layer: "The block loop's count",
        cells: [
            cell("enforced", owed("R-206")),
            cell(
                "enforced for sandbox, policy and project blocks; unverified guard reports never count",
                owed("R-206, R-103"),
            ),
            cell(
                "enforced for sandbox, policy and project blocks; unverified guard reports never count",
                owed("R-206, R-103"),
            ),
        ],
    },
];

/// Why `matrix`'s cells fail to name a test that exists, one line per cell; empty when every
/// cell does. `source` reads a file from this crate's root.
fn unconfirmed(matrix: &[Row], source: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut wrong = Vec::new();
    for row in matrix {
        for (harness, cell) in HARNESSES.iter().zip(&row.cells) {
            let at = format!("{} × {}", row.layer, harness.title());
            if cell.value.trim().is_empty() {
                wrong.push(format!("{at}: the cell says nothing"));
            }
            match cell.test {
                Test::Owed { by } if by.trim().is_empty() => {
                    wrong.push(format!("{at}: names no test and nothing that owes one"));
                }
                Test::Owed { .. } => {}
                Test::Named { file, name } => {
                    let found = source(file).is_some_and(|text| declares_test(&text, name));
                    if !found {
                        wrong.push(format!("{at}: no test `{name}` in {file}"));
                    }
                }
            }
        }
    }
    wrong
}

/// Whether `text` declares `name` as a `#[test]` function: the attribute, then the function,
/// with nothing but other attributes and comments between.
fn declares_test(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let declared = format!("fn {name}()");
    lines.iter().enumerate().any(|(at, line)| {
        line.starts_with(&declared)
            && lines[..at]
                .iter()
                .rev()
                .take_while(|line| line.starts_with("#[") || line.starts_with("//"))
                .any(|line| *line == "#[test]")
    })
}

/// A file of this crate, from its root.
fn crate_file(path: &str) -> Option<String> {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).ok()
}

#[test]
fn every_harness_matrix_cell_names_a_test_that_exists() {
    assert_eq!(unconfirmed(&MATRIX, &crate_file), Vec::<String>::new());

    // The check itself: a cell whose test name is taken out, or whose test is gone, fails it.
    let mut unnamed = MATRIX;
    unnamed[3].cells[0].test = owed("");
    assert_eq!(
        unconfirmed(&unnamed, &crate_file),
        [
            "The harness's prompts and asks reach Asks × Claude Code: names no test and nothing \
          that owes one"
        ]
    );
    let without = |path: &str| {
        crate_file(path).map(|text| {
            text.replace(
                "fn a_claude_code_permission_prompt_hook_line_raises_one_ask()",
                "fn gone()",
            )
        })
    };
    assert_eq!(
        unconfirmed(&MATRIX, &without),
        [
            "The harness's prompts and asks reach Asks × Claude Code: no test \
          `a_claude_code_permission_prompt_hook_line_raises_one_ask` in \
          src/harness/tests/matrix.rs"
        ]
    );
}

/// The window, which a person answers from.
fn window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).expect("the window answers")
}

/// The asks one hook line raises when the host hears it: the line, as a permission hook writes
/// it on the hook channel, handed to the host's own hearer ([`crate::hookwire::permission`]).
fn heard(line: &str) -> (Arc<HookAsks>, Option<crate::hookwire::permission::Waiting>) {
    let hooks = Arc::new(HookAsks::new(Arc::new(Asks::new())));
    let hear = crate::hookwire::permission::held_in(Arc::clone(&hooks), Arc::new(|| {}));
    let asked: PermissionAsked = serde_json::from_str(line).expect("a hook line");
    let waiting = hear(asked);
    (hooks, waiting)
}

/// Claude Code's `PermissionRequest` for a shell command, as its hook hands it to the host.
const CLAUDE_CODE_PROMPT: &str = r#"{"chat":7,"permission_request":"claude_code","payload":{"session_id":"abc","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"npm test"},"permission_suggestions":[]},"ran_for_ms":12}"#;

/// opencode's `permission.asked`, as purlis's shim hands it to the hook and the hook to the host.
const OPENCODE_ASK: &str = r#"{"chat":8,"permission_request":"opencode","payload":{"id":"per_1","sessionID":"ses_1","permission":"webfetch","patterns":["https://example.com"],"always":["https://example.com"],"metadata":{}},"ran_for_ms":12}"#;

#[test]
fn a_claude_code_permission_prompt_hook_line_raises_one_ask() {
    // R-093: Claude Code's permission prompt reaches purlis through its hook, as one ask held
    // on that hook, the harness's own prompt answered back where it asked.
    let (hooks, waiting) = heard(CLAUDE_CODE_PROMPT);
    assert!(waiting.is_some(), "the host holds the hook");
    let pending = hooks.pending(std::time::Instant::now());
    assert_eq!(pending.len(), 1, "{pending:?}");
    assert_eq!(pending[0].chat, "7");
    assert_eq!(pending[0].ask.channel, Channel::Hook);
    assert_eq!(pending[0].ask.summary.as_str(), "Run a command: npm test");
}

#[test]
fn an_opencode_permission_ask_is_held_in_the_window_as_one_ask() {
    // R-096: opencode's ask is one ask in the window, and the window's answer reaches the
    // hook that waits on it as opencode's own reply word.
    let (hooks, waiting) = heard(OPENCODE_ASK);
    let waiting = waiting.expect("the host holds the hook");
    let now = std::time::Instant::now();
    let pending = hooks.pending(now);
    assert_eq!(pending.len(), 1, "{pending:?}");
    assert_eq!(pending[0].ask.channel, Channel::Hook);
    let ids: Vec<&str> = pending[0]
        .ask
        .options
        .iter()
        .map(|o| o.id.as_str())
        .collect();
    assert_eq!(ids, ["once", "always", "reject"]);

    hooks
        .answer("8", &pending[0].id, "once", window(), now)
        .expect("answered from the window");
    assert_eq!(waiting.answered.recv().expect("told").id, "once");
}

#[test]
fn codex_start_arguments_carry_the_rejecting_approval_policy() {
    // R-098: a sandboxed Codex chat is started with an approval policy that rejects what Codex
    // would stop to ask for its sandbox, its permissions and its skills, rather than asking.
    // These are the flags the adapter puts last in a wrapped Codex's line, after every word of
    // the profile's and the chat's (`a_sandboxed_codex_chat_is_handed_its_flags_after_its_hooks`
    // holds where), so nothing before them outranks them.
    let flags = crate::sandbox::codex::flags();
    let policy = flags
        .iter()
        .find_map(|arg| arg.strip_prefix("approval_policy="))
        .expect("an approval policy");
    let policy: toml::Table = format!("v = {policy}").parse().expect("TOML");
    let granular = &policy["v"]["granular"];
    for rejected in ["sandbox_approval", "request_permissions", "skill_approval"] {
        assert_eq!(
            granular[rejected].as_bool(),
            Some(false),
            "{rejected}: {policy}"
        );
    }
    // And none of them hands Codex back an ask: no approval flag of its own.
    for flag in ["-a", "--ask-for-approval", "--approve-for-me"] {
        assert!(!flags.iter().any(|arg| arg == flag), "handed {flag}");
    }
}

/// Every `mcp__…` tool a matcher names: its `|`-separated alternatives that are MCP tools.
fn mcp_tools(matcher: &str) -> Vec<&str> {
    matcher
        .split('|')
        .map(str::trim)
        .filter(|tool| tool.starts_with("mcp__"))
        .collect()
}

/// Whether an MCP tool a matcher names is purlis's own.
fn purlis_s_own(tool: &str) -> bool {
    tool.starts_with("mcp__purlis__")
}

#[test]
fn no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3() {
    // R-164: until phase 3, every non-purlis MCP tool is judged by the harness's own check. No
    // hook purlis arms on any harness names one.
    let mut matchers: Vec<(String, String)> = Vec::new();

    // Claude Code: the plugin's hooks, and the one its settings arm.
    let plugin: serde_json::Value =
        serde_json::from_str(&crate::plugin::hooks_json()).expect("hooks.json");
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = super::claude("/bin/charter", empty.path());
    let settings: serde_json::Value =
        serde_json::from_str(super::settings_of(&args)).expect("settings");
    for hooks in [&plugin["hooks"], &settings["hooks"]] {
        for (event, groups) in hooks.as_object().expect("hooks by event") {
            for group in groups.as_array().expect("groups") {
                if let Some(matcher) = group["matcher"].as_str() {
                    matchers.push((format!("Claude Code {event}"), matcher.to_owned()));
                }
            }
        }
    }

    // Codex: every hook its session flags arm.
    let StateHooks::ThisSessionOnly { args, .. } = Harness::Codex.state_hooks(
        super::kit("/bin/charter"),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    for value in args.iter().filter_map(|arg| arg.strip_prefix("hooks.")) {
        let (event, groups) = value.split_once('=').expect("an event");
        let groups: toml::Table = format!("v = {groups}").parse().expect("TOML");
        for group in groups["v"].as_array().expect("groups") {
            if let Some(matcher) = group.get("matcher").and_then(toml::Value::as_str) {
                matchers.push((format!("Codex {event}"), matcher.to_owned()));
            }
        }
    }

    // opencode: the tools its shim routes by name. Any other tool, an MCP tool included, goes
    // to the Bash guard, which judges a command and holds nothing.
    for tool in crate::opencode::TOOLS {
        matchers.push(("opencode tool".to_owned(), tool.id.to_owned()));
        matchers.push(("opencode tool".to_owned(), tool.name.to_owned()));
    }
    assert_eq!(crate::opencode::DEFAULT_PRE, "pretooluse");
    assert_ne!(crate::opencode::DEFAULT_PRE, crate::harness::hooked::WORD);

    assert!(
        matchers.iter().any(|(_, m)| !mcp_tools(m).is_empty()),
        "purlis's own are read"
    );
    let held: Vec<String> = matchers
        .iter()
        .flat_map(|(at, matcher)| {
            mcp_tools(matcher)
                .into_iter()
                .filter(|tool| !purlis_s_own(tool))
                .map(move |tool| format!("{at}: {tool}"))
        })
        .collect();
    assert_eq!(held, Vec::<String>::new());
}
