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

/// Where the harness's own check's blocks are confirmed (R-094, R-325).
const HARNESS_CHECK: &str = "src/eventlog/stops/harness_check_tests.rs";

const fn here(name: &'static str) -> Test {
    Test::Named { file: THIS, name }
}

/// Where the Chat log's summary is confirmed: what it says purlis can't tell of a chat's
/// harness (R-097, R-099, R-100).
const CHAT_LOG: &str = "src/eventlog/chatlog/tests.rs";

const fn chat_log(name: &'static str) -> Test {
    Test::Named {
        file: CHAT_LOG,
        name,
    }
}

/// Where files and egress are confirmed enforced on each harness (R-353).
const ENFORCED: &str = "src/sandbox/enforced_tests.rs";

const fn enforced(name: &'static str) -> Test {
    Test::Named {
        file: ENFORCED,
        name,
    }
}

const fn owed(by: &'static str) -> Test {
    Test::Owed { by }
}

const fn cell(value: &'static str, test: Test) -> Cell {
    Cell { value, test }
}

/// R-101: a guard's block from each harness, plain from Claude Code's under a backend that
/// leaves its hooks outside (Q235), and unverified from Codex's and opencode's.
const GUARD_REPORTED: Test = Test::Named {
    file: "src/eventlog/stops/guard_tests.rs",
    name: "a_guard_report_is_verified_only_from_claude_code_under_a_backend_that_leaves_its_hooks_outside",
};

/// ADR 0124's table, for Basic and Full sandbox (at No sandbox every report is unverified).
const MATRIX: [Row; 10] = [
    Row {
        layer: "Sandbox: files and egress",
        cells: [
            // The Full sandbox's half; the floor's (web tools allowed, Q254) is pinned beside it
            // by `at_basic_sandbox_claude_codes_denial_classes_are_held_and_its_web_tools_allowed`.
            cell(
                "enforced (Bash in the sandbox; file tools by deny rules where a denial class \
                 holds the path; web tools denied at Full sandbox and allowed at Basic sandbox, \
                 whose proxy carries every host)",
                enforced("files_and_egress_are_enforced_on_claude_code"),
            ),
            cell(
                "enforced (the whole process, its hooks and MCP servers, in purlis's wrap)",
                enforced("files_and_egress_are_enforced_on_codex"),
            ),
            cell(
                "enforced (the whole process in purlis's wrap)",
                enforced("files_and_egress_are_enforced_on_opencode"),
            ),
        ],
    },
    Row {
        layer: "A guard's block reported",
        cells: [
            cell(
                "enforced (the hook runs outside the chat's reach)",
                GUARD_REPORTED,
            ),
            cell("unverified", GUARD_REPORTED),
            cell("unverified", GUARD_REPORTED),
        ],
    },
    Row {
        layer: "The harness's own check's block reported",
        cells: [
            cell(
                "enforced (`PermissionDenied`)",
                Test::Named {
                    file: HARNESS_CHECK,
                    name: "a_permission_denied_hook_line_writes_one_block_from_the_harness_own_check",
                },
            ),
            // R-325's core half, a block from a reported rejection and none guessed, is tested
            // in `HARNESS_CHECK` on a fixture, and no cell names that test: Codex is not armed
            // with the word, so no Codex rejection is reported today. The Can't tell the Chat
            // log heads with is its summary's (R-100, R-097); the window's words are its
            // stories' (`app/src/ChatLogTab.stories.tsx`).
            cell(
                "Can't tell (no denial signal yet; a hook-reported rejection becomes a block)",
                chat_log("a_codex_chats_summary_cant_tell_its_own_checks_blocks"),
            ),
            cell(
                "Can't tell (no denial signal)",
                chat_log("an_opencode_chats_summary_cant_tell_its_own_checks_blocks"),
            ),
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
                "sandbox, permission and skill approvals: Can't tell (rejected, never asked); \
                 rule prompts and MCP elicitations are still asked, and reach Asks through the \
                 PermissionRequest hook (unmeasured for MCP elicitations)",
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
            cell("enforced (the armed hook)", ATTENDANCE_MODE),
            cell(
                "Can't tell (bypass reported for every chat; attendance from how purlis started it)",
                chat_log("a_codex_chats_summary_cant_tell_its_mode"),
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
    // Phase 3 brings these holds, and their tests (R-165). What ships until then, that no
    // purlis hook holds a non-purlis MCP tool (R-164), is a requirement of its own, not a cell:
    // `no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3` is its test.
    Row {
        layer: "A non-purlis MCP tool's hold (phase 3)",
        cells: [
            cell(
                "enforced (the hook; the server runs outside the sandbox)",
                owed("R-165"),
            ),
            cell("unverified (the hook runs in the wrap)", owed("R-165")),
            cell("unverified (the hook runs in the wrap)", owed("R-165")),
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

/// R-095: Claude Code's mode, as attendance reads it, comes from its armed hook alone (T1-17).
const ATTENDANCE_MODE: Test = Test::Named {
    file: "tests/it/attendance_is_one_fact_purlis_sets.rs",
    name: "claude_code_mode_is_read_only_from_the_armed_hook",
};

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
                Test::Owed { by } if !by.split(',').all(|id| requirement(id.trim())) => {
                    wrong.push(format!(
                        "{at}: owed by `{by}`, which is not a list of R-IDs"
                    ));
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

/// Whether `id` is a requirement of the ledger by its id: `R-` and its number.
fn requirement(id: &str) -> bool {
    id.strip_prefix("R-")
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
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
    let mut prose = MATRIX;
    prose[0].cells[0].test = owed("ADR 0124 matrix: no requirement names it yet");
    prose[0].cells[1].test = owed("R-101, phase 3");
    prose[0].cells[2].test = owed("R-");
    assert_eq!(
        unconfirmed(&prose, &crate_file),
        [
            "Sandbox: files and egress × Claude Code: owed by `ADR 0124 matrix: no requirement \
             names it yet`, which is not a list of R-IDs",
            "Sandbox: files and egress × Codex: owed by `R-101, phase 3`, which is not a list of \
             R-IDs",
            "Sandbox: files and egress × opencode: owed by `R-`, which is not a list of R-IDs",
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
    // on that hook, the harness's own prompt answered back where it asked. That the settings
    // arm the hook is covered by the existing tests in `harness/tests.rs`
    // (`the_settings_arm_only_the_permission_hook_so_no_hook_is_armed_twice`).
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
    // hook that waits on it as opencode's own reply word. That the shim arms the hook is covered
    // by the existing tests (`opencode_s_permission_prompt_is_also_an_ask_answered_on_opencode_s_
    // own_client`, in `opencode/tests.rs`).
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
    // Exactly one: Codex honours the last of several, so a second would outrank this one.
    let policies: Vec<&str> = flags
        .iter()
        .filter_map(|arg| arg.strip_prefix("approval_policy="))
        .collect();
    let [policy] = policies[..] else {
        panic!("one approval policy: {flags:?}");
    };
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

/// The events a hook group can name tools on.
const TOOL_EVENTS: [&str; 4] = [
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
];

/// One group of hooks purlis arms on one harness: its event, its tool matcher (if any), and the
/// `purlis hook` words its hooks run.
struct Group {
    harness: Harness,
    event: String,
    matcher: Option<String>,
    words: Vec<String>,
}

impl Group {
    fn at(&self) -> String {
        format!("{} {}", self.harness.title(), self.event)
    }
}

/// The `purlis hook` word a hook's command runs.
fn word_of(command: &str) -> String {
    command
        .rsplit_once(" hook ")
        .map_or(command, |(_, word)| word)
        .to_owned()
}

/// The tools a matcher names, when it is a plain `|`-list of names; `None` when it is a
/// pattern, which may catch any tool. An alternative under purlis's own prefix names only
/// purlis's tools, a pattern (`caught`'s `mcp__purlis__.*`) or not.
fn plain_names(matcher: &str) -> Option<Vec<&str>> {
    matcher
        .split('|')
        .map(str::trim)
        .map(|name| {
            let plain = !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
            (plain || purlis_s_own(name)).then_some(name)
        })
        .collect()
}

/// Whether an MCP tool a matcher names is purlis's own.
fn purlis_s_own(tool: &str) -> bool {
    tool.starts_with("mcp__purlis__")
}

/// Whether a group that names every tool holds none of them, and why.
///
/// - `PermissionRequest` to [`crate::harness::hooked::WORD`]: it relays the harness's own
///   prompt to the window and answers back only what the person answered, so the harness's own
///   check still judges the tool (R-093, R-096).
/// - opencode's default route ([`crate::opencode::DEFAULT_PRE`]), where a tool with no route of
///   its own goes, MCP tools included: the Bash guard, which can block a call carrying a
///   `command` and never holds one (R-164's shipped exception, Q225).
fn holds_nothing(group: &Group) -> bool {
    let only = |word: &str| !group.words.is_empty() && group.words.iter().all(|w| w == word);
    (group.event == "PermissionRequest" && only(crate::harness::hooked::WORD))
        || (group.harness == Harness::Opencode
            && group.event == "PreToolUse"
            && group.matcher.is_none()
            && crate::opencode::DEFAULT_PRE == "pretooluse"
            && only(crate::opencode::DEFAULT_PRE))
}

/// The non-purlis MCP tools `groups` hold, one line each. A group on a tool-call event with no
/// matcher, or with a pattern, names every MCP tool.
fn held_mcp_tools(groups: &[Group]) -> Vec<String> {
    let mut held = Vec::new();
    for group in groups
        .iter()
        .filter(|g| TOOL_EVENTS.contains(&g.event.as_str()))
    {
        if holds_nothing(group) {
            continue;
        }
        match group.matcher.as_deref().and_then(plain_names) {
            None => held.push(format!(
                "{}: every MCP tool (matcher {:?}, {:?})",
                group.at(),
                group.matcher,
                group.words
            )),
            Some(names) => held.extend(
                names
                    .into_iter()
                    .filter(|tool| tool.starts_with("mcp__") && !purlis_s_own(tool))
                    .map(|tool| format!("{}: {tool}", group.at())),
            ),
        }
    }
    held
}

/// Every hook group purlis arms on each harness.
fn armed_groups() -> Vec<Group> {
    let mut groups = Vec::new();

    // Claude Code: the plugin's hooks, and the one its settings arm.
    let plugin: serde_json::Value =
        serde_json::from_str(&crate::plugin::hooks_json()).expect("hooks.json");
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = super::claude("/bin/charter", empty.path());
    let settings: serde_json::Value =
        serde_json::from_str(super::settings_of(&args)).expect("settings");
    for hooks in [&plugin["hooks"], &settings["hooks"]] {
        for (event, list) in hooks.as_object().expect("hooks by event") {
            for group in list.as_array().expect("groups") {
                groups.push(Group {
                    harness: Harness::ClaudeCode,
                    event: event.clone(),
                    matcher: group["matcher"].as_str().map(str::to_owned),
                    words: group["hooks"]
                        .as_array()
                        .expect("hooks")
                        .iter()
                        .map(|hook| word_of(hook["command"].as_str().expect("a command")))
                        .collect(),
                });
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
        let (event, list) = value.split_once('=').expect("an event");
        let list: toml::Table = format!("v = {list}").parse().expect("TOML");
        for group in list["v"].as_array().expect("groups") {
            groups.push(Group {
                harness: Harness::Codex,
                event: event.to_owned(),
                matcher: group
                    .get("matcher")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned),
                words: group["hooks"]
                    .as_array()
                    .expect("hooks")
                    .iter()
                    .map(|hook| word_of(hook["command"].as_str().expect("a command")))
                    .collect(),
            });
        }
    }

    // opencode: the tools its shim routes by id, and the route every other tool takes.
    let route = |event: &str, matcher: Option<&str>, word: &str| Group {
        harness: Harness::Opencode,
        event: event.to_owned(),
        matcher: matcher.map(str::to_owned),
        words: vec![word.to_owned()],
    };
    for tool in crate::opencode::TOOLS {
        if let Some(word) = tool.pre {
            groups.push(route("PreToolUse", Some(tool.id), word));
        }
        if let Some(word) = tool.post {
            groups.push(route("PostToolUse", Some(tool.id), word));
        }
    }
    groups.push(route("PreToolUse", None, crate::opencode::DEFAULT_PRE));
    groups
}

#[test]
fn no_purlis_hook_holds_non_purlis_mcp_tools_before_phase_3() {
    // R-164: until phase 3, every non-purlis MCP tool is judged by the harness's own check. No
    // hook purlis arms on any harness names one, by name or by catching every tool.
    let groups = armed_groups();

    // Each harness's tool matchers were read, so a parse that finds nothing goes red.
    for harness in HARNESSES {
        assert!(
            groups.iter().any(|g| g.harness == harness
                && TOOL_EVENTS.contains(&g.event.as_str())
                && g.matcher.as_deref().and_then(plain_names).is_some()),
            "no tool matcher read on {}",
            harness.title()
        );
    }
    assert_eq!(held_mcp_tools(&groups), Vec::<String>::new());

    // The check itself: a guard on every tool, by no matcher or by a pattern, holds them all.
    let every = |matcher: Option<&str>| Group {
        harness: Harness::Codex,
        event: "PreToolUse".to_owned(),
        matcher: matcher.map(str::to_owned),
        words: vec!["pretooluse".to_owned()],
    };
    assert_eq!(
        held_mcp_tools(&[
            every(None),
            every(Some(".*")),
            every(Some("Bash|mcp__github__x"))
        ]),
        [
            r#"Codex PreToolUse: every MCP tool (matcher None, ["pretooluse"])"#,
            r#"Codex PreToolUse: every MCP tool (matcher Some(".*"), ["pretooluse"])"#,
            "Codex PreToolUse: mcp__github__x",
        ]
    );
}
