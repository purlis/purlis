// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter mcp`, spoken to as a harness speaks to it: JSON-RPC lines on its stdin and stdout
//! (HP-7). What each tool does is `purlis_core::chattools`'s to test; this holds the protocol,
//! the chat's place, and the question that only the person answers.

use crate::support;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

use serde_json::{Value, json};

/// A project with one workspace, `alpha`, and another, `beta`.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join("charter.toml"), "").expect("charter.toml");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(dir.path().join("workspaces").join(ws)).expect("a workspace");
    }
    // Shared workspaces: LIVE, so a skill run there says where it ran (SK-20).
    std::fs::write(
        dir.path().join(".gitignore"),
        purlis_core::wscmd::live_block(dir.path(), ["alpha", "beta"]),
    )
    .expect(".gitignore");
    dir
}

/// A running `charter mcp` for a chat in `workspace`, with nothing of this process's own
/// environment but what a program needs to run.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    fn start(root: &Path, workspace: &str) -> Self {
        Self::start_with(root, workspace, &[])
    }

    /// [`Server::start`], with `env` set too: what a chat the app started hands it.
    fn start_with(root: &Path, workspace: &str, env: &[(&str, &std::ffi::OsStr)]) -> Self {
        Self::start_as(root, workspace, env, &[])
    }

    /// [`Server::start_with`], with `args` after `mcp`: a grade's chat's `--graded`.
    fn start_as(
        root: &Path,
        workspace: &str,
        env: &[(&str, &std::ffi::OsStr)],
        args: &[&str],
    ) -> Self {
        let mut child = support::purlis()
            .arg("mcp")
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", root)
            .env("CHARTER_ROOT", root)
            .env("CHARTER_WORKSPACE", workspace)
            .env("CHARTER_CHAT_TOKEN", "the-chat-token")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("charter mcp starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin: Some(stdin),
            stdout,
            next: 1,
        }
    }

    /// `initialize` at a protocol version the harnesses speak today, offering elicitation or not.
    fn initialize(&mut self, elicitation: bool) -> Value {
        let capabilities = if elicitation {
            json!({ "elicitation": {} })
        } else {
            json!({})
        };
        let answer = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": capabilities,
                "clientInfo": { "name": "test-harness", "version": "1" },
            }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        answer
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("the server's stdin is open");
        writeln!(stdin, "{message}").expect("write");
        stdin.flush().expect("flush");
    }

    /// The harness lets go: its end of the connection closed, and the server left to finish.
    fn hang_up(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read");
        assert!(!line.is_empty(), "the server closed its stdout");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON-RPC ({e}): {line:?}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let answer = self.read();
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )["result"]
            .clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_harness_lists_charter_s_tools() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    let hello = server.initialize(false);
    assert_eq!(hello["result"]["serverInfo"]["name"], "purlis", "{hello}");

    let listed = server.request("tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "todo_list",
            "todo_add",
            "todo_done",
            "memory_search",
            "memory_add",
            "persona_remember",
            "persona_where",
            "session_record_list",
            "session_record_read",
            "session_record",
            "workspace_vision",
            "workspace_section",
            "change_status",
            "dispatch",
            "dispatch_list",
            "dispatch_read",
            "dispatch_report",
            "ask_operator",
            // A real browser the session host runs (#2013).
            "browser_open",
            "browser_navigate",
            "browser_snapshot",
            "browser_find",
            "browser_click",
            "browser_double_click",
            "browser_hover",
            "browser_type",
            "browser_press",
            "browser_select",
            "browser_drag",
            "browser_scroll",
            "browser_back",
            "browser_wait_for_text",
            "browser_pages",
            "browser_switch_page",
            "browser_screenshot",
            "browser_new",
            "browser_close",
            "browser_login",
            "theme_preview",
        ]
    );
}

/// `goose`, a harness the project declares, which takes MCP servers and has no adapter.
const GOOSE: &str = "name = \"goose\"\ntitle = \"Goose\"\nprogram = \"goose\"\n\n[session]\n\n\
                     [terminal]\nready_to_type = \"raw-and-quiet\"\n\n[levels]\n\n\
                     [capabilities]\nmcp_servers = \"yes\"\n";

/// A project with one skill at its root, `deploy`, and the Skill studio's flag on for its
/// server, the harness named `harness`.
fn with_a_skill(harness: &str) -> (tempfile::TempDir, Server) {
    let p = project();
    std::fs::write(p.path().join(purlis_core::plane::MANIFEST), "").expect("manifest");
    let deploy = p.path().join("skills/deploy");
    std::fs::create_dir_all(&deploy).expect("a skill");
    std::fs::write(
        deploy.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ship the service.\n---\n\nShip it.\n",
    )
    .expect("SKILL.md");
    std::fs::create_dir_all(p.path().join("harnesses")).expect("harnesses/");
    std::fs::write(p.path().join("harnesses/goose.toml"), GOOSE).expect("a declaration");
    // Granted to every workspace, so the chat in `alpha` has it.
    std::fs::write(p.path().join("skills/deploy.toml"), "grant = \"all\"\n").expect("a grant");
    let server = Server::start_with(
        p.path(),
        "alpha",
        &[
            ("PURLIS_SKILL_STUDIO", "1".as_ref()),
            (purlis_core::hookwire::HARNESS_ENV, harness.as_ref()),
        ],
    );
    (p, server)
}

#[test]
fn with_the_studio_on_a_harness_with_no_skill_format_is_told_its_skills_are_over_mcp() {
    // SK-13: the fallback route's one line, in what the server tells the harness at its start.
    let (p, mut server) = with_a_skill("goose");
    let hello = server.initialize(false);
    let instructions = hello["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        instructions.contains(purlis_core::skills::POINTER),
        "{instructions}"
    );
    let listed = server.request("tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(
        names.ends_with(&["skill_list", "skill_runs", "skill_find", "skill_load"]),
        "{names:?}"
    );

    let loaded = text(&server.call("skill_load", json!({ "name": "deploy" })));
    assert!(
        loaded.starts_with("Loaded the skill `deploy` (project)."),
        "{loaded}"
    );
    assert!(loaded.ends_with("Ship it.\n"), "{loaded}");
    // No app listens: the load's run is in the project's skill log, started by the load.
    let runs = purlis_core::skilluse::runs(p.path());
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(
        runs[0].started_by,
        Some(purlis_core::skills::runs::StartedBy::McpLoad)
    );
    let ran = text(&server.call("skill_runs", json!({})));
    assert!(ran.contains("`deploy`, started by mcp-load"), "{ran}");
}

#[test]
fn a_harness_with_a_route_of_its_own_or_none_is_not_told_the_line() {
    // The three keep their routes; a harness no declaration says takes MCP has no route.
    for harness in ["claude-code", "codex", "opencode", "undeclared"] {
        let (_p, mut server) = with_a_skill(harness);
        let hello = server.initialize(false);
        let instructions = hello["result"]["instructions"].as_str().unwrap_or_default();
        assert!(
            !instructions.contains(purlis_core::skills::POINTER),
            "{harness}: {instructions}"
        );
        let found = text(&server.call("skill_find", json!({ "query": "ship" })));
        assert!(found.contains("`deploy` (project)"), "{harness}: {found}");
    }
}

#[test]
fn with_the_studio_off_there_is_no_skill_tool() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    let answer = server.request(
        "tools/call",
        json!({ "name": "skill_list", "arguments": {} }),
    );
    assert!(answer["error"].is_object(), "{answer}");
}

#[test]
fn theme_preview_with_no_app_says_the_app_draws_the_specimen_and_the_session_goes_on() {
    // #2330: the theme specimen is drawn by the app, in a window of its own; a chat no app
    // started has none to ask, and is told so as a tool error, not a dropped session.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    let said = server.call("theme_preview", json!({ "theme": "purlis-classic" }));
    assert_eq!(said["isError"], true, "{said}");
    assert!(text(&said).contains("the purlis app draws it"), "{said}");
    // What is no theme's id, a path among them, is refused before any app is asked (#2536).
    let said = server.call("theme_preview", json!({ "theme": "/etc/passwd" }));
    assert_eq!(said["isError"], true, "{said}");
    assert!(text(&said).contains("is not a theme's id"), "{said}");
    assert_eq!(server.call("todo_list", json!({}))["isError"], false);
}

const RECORD_BODY: &str = "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n\
                           ## How to resume\n\nr\n";

#[test]
fn a_session_record_written_over_mcp_with_no_app_lands_where_the_chat_works() {
    // No hook socket in the server's environment: no app to hand it to, so the server writes
    // it itself, as `purlis session record` would (#1332).
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);

    let written = server.call(
        "session_record",
        json!({ "title": "Over MCP", "body": RECORD_BODY }),
    );

    assert_ne!(written["isError"], true, "{written}");
    assert!(
        text(&written).contains("Session record → workspaces/alpha/sessions/"),
        "{written}"
    );
    assert!(
        text(&written).contains("tab will not close by itself"),
        "{written}"
    );
    let records: Vec<String> = std::fs::read_dir(p.path().join("workspaces/alpha/sessions"))
        .expect("alpha's records")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with("-over-mcp.md"))
        .collect();
    assert_eq!(records.len(), 1, "{records:?}");
    assert!(!p.path().join("workspaces/beta/sessions").exists());
}

#[test]
fn a_session_record_that_is_not_one_is_a_tool_error_and_nothing_is_written() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);

    let refused = server.call(
        "session_record",
        json!({ "title": "Half", "body": "## Goal\n\ng\n" }),
    );

    assert_eq!(refused["isError"], true, "{refused}");
    assert!(text(&refused).contains("nothing was written"), "{refused}");
    assert!(!p.path().join("workspaces/alpha/sessions").exists());
}

#[test]
fn a_todo_recorded_over_mcp_lands_in_the_chat_s_own_workspace() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);

    let added = server.call("todo_add", json!({ "text": "Wire the MCP server" }));
    assert_ne!(added["isError"], true, "{added}");
    let listed = server.call("todo_list", json!({}));
    assert!(text(&listed).contains("Wire the MCP server"), "{listed}");

    assert!(
        std::fs::read_dir(p.path().join("workspaces/alpha/todos"))
            .expect("alpha's todos")
            .count()
            > 0
    );
    assert!(!p.path().join("workspaces/beta/todos").exists());
}

#[test]
fn a_refusal_is_a_tool_error_the_model_reads_and_the_session_goes_on() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);

    let refused = server.call("todo_done", json!({ "slug": "no-such-todo" }));
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(text(&refused).contains("no-such-todo"), "{refused}");

    let listed = server.call("todo_list", json!({}));
    assert_ne!(listed["isError"], true, "{listed}");
}

#[test]
fn a_tool_charter_does_not_have_is_a_protocol_error() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    let answer = server.request(
        "tools/call",
        json!({ "name": "vault_read", "arguments": {} }),
    );
    assert!(answer["error"].is_object(), "{answer}");
}

#[test]
fn ask_operator_puts_the_question_to_the_person_through_the_harness() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(true);

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": { "name": "ask_operator", "arguments": { "question": "Merge the train?" } },
    }));
    // The harness is asked, and shows it to the person: an elicitation, never an answer the
    // model gives itself.
    let asked = server.read();
    assert_eq!(asked["method"], "elicitation/create", "{asked}");
    assert_eq!(
        asked["params"]["message"], "This chat asks: Merge the train?",
        "{asked}"
    );
    assert_eq!(
        asked["params"]["requestedSchema"]["properties"]["answer"]["description"],
        "Don't type a password or secret here.",
        "{asked}"
    );
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": asked["id"],
        "result": { "action": "accept", "content": { "answer": "yes, after CI" } },
    }));

    let answer = server.read();
    assert_eq!(answer["id"], 99, "{answer}");
    assert_eq!(
        text(&answer["result"]),
        "The operator answered: yes, after CI",
        "{answer}"
    );
}

#[test]
fn ask_operator_on_a_harness_that_cannot_ask_says_so() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    let said = server.call("ask_operator", json!({ "question": "Merge?" }));
    assert_eq!(said["isError"], true, "{said}");
    assert!(text(&said).contains("cannot ask"), "{said}");
}

#[test]
fn ask_operator_pointed_at_a_browser_sends_no_elicitation_and_with_no_app_says_there_is_no_browser()
{
    // #2015: a browser that needs the person is the app's Ask, raised at once; the harness's own
    // prompt is not used, so the call's own answer is the next thing the harness hears.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(true);
    let said = server.call(
        "ask_operator",
        json!({ "question": "The page wants the letters", "browser": "b1" }),
    );
    assert_eq!(said["isError"], true, "{said}");
    assert!(text(&said).contains("no browser"), "{said}");
}

#[test]
fn ask_operator_with_no_browser_named_still_asks_the_person_through_the_harness() {
    // #2015: a null `browser` is the plain question, put through the harness's own prompt.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(true);

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 98,
        "method": "tools/call",
        "params": {
            "name": "ask_operator",
            "arguments": { "question": "Merge the train?", "browser": null },
        },
    }));

    let asked = server.read();
    assert_eq!(asked["method"], "elicitation/create", "{asked}");
    assert_eq!(
        asked["params"]["message"], "This chat asks: Merge the train?",
        "{asked}"
    );
}

#[test]
fn no_answer_carries_the_chat_s_token() {
    // The server is handed the chat's whole environment by Claude Code and opencode; nothing
    // it answers repeats it.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    for (tool, args) in [
        ("todo_list", json!({})),
        ("memory_search", json!({})),
        ("session_record_list", json!({})),
        ("change_status", json!({})),
    ] {
        let said = server.call(tool, args);
        assert!(
            !said.to_string().contains("the-chat-token"),
            "{tool}: {said}"
        );
    }
}

/// What a 2026-07-28 harness (Claude Code 2.1.288, measured) puts on every request instead of
/// an `initialize`: its version, itself and what it can do.
fn stateless_meta(elicitation: bool) -> Value {
    let capabilities = if elicitation {
        json!({ "elicitation": { "form": {} } })
    } else {
        json!({})
    };
    json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test-harness", "version": "1" },
        "io.modelcontextprotocol/clientCapabilities": capabilities,
    })
}

#[test]
fn at_2026_07_28_ask_operator_asks_by_returning_input_required_and_reads_the_retry() {
    // SEP-2322: the server asks by returning `input_required`, and the harness retries the
    // call with the person's answer.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    let asked = server.request(
        "tools/call",
        json!({
            "_meta": stateless_meta(true),
            "name": "ask_operator",
            "arguments": { "question": "Merge the train?" },
        }),
    )["result"]
        .clone();
    assert_eq!(asked["resultType"], "input_required", "{asked}");
    let request = &asked["inputRequests"]["answer"];
    assert_eq!(request["method"], "elicitation/create", "{asked}");
    assert_eq!(
        request["params"]["message"], "This chat asks: Merge the train?",
        "{asked}"
    );

    let answered = server.request(
        "tools/call",
        json!({
            "_meta": stateless_meta(true),
            "name": "ask_operator",
            "arguments": { "question": "Merge the train?" },
            "inputResponses": { "answer": { "action": "decline" } },
            "requestState": asked["requestState"],
        }),
    )["result"]
        .clone();
    assert_eq!(
        text(&answered),
        "The operator declined to answer.",
        "{answered}"
    );
}

#[test]
fn at_2026_07_28_a_harness_that_cannot_ask_is_told_so() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    let said = server.request(
        "tools/call",
        json!({
            "_meta": stateless_meta(false),
            "name": "ask_operator",
            "arguments": { "question": "Merge?" },
        }),
    )["result"]
        .clone();
    assert_eq!(said["isError"], true, "{said}");
    assert!(text(&said).contains("cannot ask"), "{said}");
}

#[test]
fn at_2026_07_28_an_answer_retried_for_another_question_is_not_read() {
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    let asked = server.request(
        "tools/call",
        json!({
            "_meta": stateless_meta(true),
            "name": "ask_operator",
            "arguments": { "question": "Merge the train?" },
        }),
    )["result"]
        .clone();
    let retried = server.request(
        "tools/call",
        json!({
            "_meta": stateless_meta(true),
            "name": "ask_operator",
            "arguments": { "question": "Delete the branch?" },
            "inputResponses": { "answer": { "action": "accept", "content": { "answer": "yes" } } },
            "requestState": asked["requestState"],
        }),
    )["result"]
        .clone();
    assert_eq!(retried["isError"], true, "{retried}");
    assert!(!text(&retried).contains("yes"), "{retried}");
}

#[test]
fn a_write_is_refused_once_the_project_turns_read_only_mid_session() {
    // FR-24: an update or a pull can move the project to a format this charter cannot write
    // while the chat, and its server, keep running.
    let p = project();
    let mut server = Server::start(p.path(), "alpha");
    server.initialize(false);
    let before = server.call("todo_add", json!({ "text": "Before" }));
    assert_ne!(before["isError"], true, "{before}");

    std::fs::write(p.path().join("charter.toml"), "schema = 999\n").expect("charter.toml");
    let after = server.call("memory_add", json!({ "text": "After" }));
    assert_eq!(after["isError"], true, "{after}");
    assert!(!p.path().join("workspaces/alpha/memory").exists());
}

// ------------------------------------------------------------------------------------------
// An `ask_operator` wait in needs you (#1089, #1048)
// ------------------------------------------------------------------------------------------

/// `ask_operator` called on `server` as request 99: the elicitation it sends the harness.
fn ask(server: &mut Server, question: &str) -> Value {
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": { "name": "ask_operator", "arguments": { "question": question } },
    }));
    let asked = server.read();
    assert_eq!(asked["method"], "elicitation/create", "{asked}");
    asked
}

/// The person's answer to elicitation `asked`, and the tool's answer read back.
fn answer(server: &mut Server, asked: &Value, said: &str) -> Value {
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": asked["id"],
        "result": { "action": "accept", "content": { "answer": said } },
    }));
    let answered = server.read();
    assert_eq!(answered["id"], 99, "{answered}");
    answered
}

mod claude_code_says_it {
    //! Claude Code says an MCP server's form itself: `Notification` with `notification_type`
    //! `elicitation_dialog` as it shows it, and `elicitation_complete` once it is answered (its
    //! hooks reference, 2.1.296). Replayed here around the server's own elicitation, each
    //! payload read as the chat's hook reads it and applied to the board the window draws
    //! from, so the whole road is held: the server asks, the chat waits on a form in needs
    //! you, the person answers, and the chat is back at work.

    use super::*;
    use purlis_core::harness::Harness;
    use purlis_core::harness::model::Prompt;
    use purlis_core::hookwire::{
        CHAT_ENV, CLAUDE_CONVERSATION_ENV, CLAUDE_PID_ENV, HARNESS_ENV, Report, SOCKET_ENV,
    };
    use purlis_core::state::{Board, Event, State};

    const SESSION: &str = "11111111-2222-4333-8444-555555555555";
    const CHAT: u32 = 7;

    fn env(name: &str) -> Option<String> {
        match name {
            SOCKET_ENV => Some("/tmp/s.sock".to_owned()),
            CHAT_ENV => Some(CHAT.to_string()),
            CLAUDE_CONVERSATION_ENV => Some(SESSION.to_owned()),
            CLAUDE_PID_ENV => Some("4242".to_owned()),
            HARNESS_ENV => Some("claude-code".to_owned()),
            _ => None,
        }
    }

    fn hear(board: &mut Board, event: Event, mut payload: Value) {
        payload["session_id"] = SESSION.into();
        payload["hook_event_name"] = event.word().into();
        let report = Report::read(event, &payload.to_string(), &env).expect("a report");
        board.reported(&report);
    }

    #[test]
    fn a_chat_waiting_on_ask_operator_is_in_needs_you_on_a_form_until_it_is_answered() {
        let p = project();
        let mut server = Server::start(p.path(), "alpha");
        server.initialize(true);
        let mut board = Board::new();
        board.opened(CHAT, Some(Harness::ClaudeCode), Some(SESSION.to_owned()));
        hear(
            &mut board,
            Event::SessionStart,
            json!({"source": "startup"}),
        );
        hear(
            &mut board,
            Event::UserPromptSubmit,
            json!({"prompt": "merge it?"}),
        );
        assert_eq!(board.state(CHAT), State::Running);

        let asked = ask(&mut server, "Merge the train?");
        hear(
            &mut board,
            Event::Notification,
            json!({
                "notification_type": "elicitation_dialog",
                "message": asked["params"]["message"],
            }),
        );
        assert_eq!(
            board.needs_you(),
            vec![CHAT],
            "the chat waits on the person"
        );
        assert_eq!(
            board.its_prompt(CHAT),
            Some(Prompt::Form),
            "named as a form"
        );
        assert!(board.waits_on_its_prompt(CHAT));

        let answered = answer(&mut server, &asked, "yes, after CI");
        assert_eq!(
            text(&answered["result"]),
            "The operator answered: yes, after CI"
        );
        hear(
            &mut board,
            Event::Notification,
            json!({"notification_type": "elicitation_complete"}),
        );
        assert!(board.needs_you().is_empty(), "the answer took it out");
        assert_eq!(board.its_prompt(CHAT), None);
        assert_eq!(board.state(CHAT), State::Running);
    }
}

mod the_server_says_it {
    //! A harness with no notice of its own (Codex, opencode) has the server say the wait and
    //! its end on the chat's hook socket; Claude Code, which says it itself, has it say
    //! nothing. These bind a socket, so they first run on CI.

    use super::*;
    use purlis_core::harness::model::Prompt;
    use purlis_core::hookwire::{
        CHAT_ENV, ChatToken, HARNESS_ENV, Hearing, Listener, Report, SOCKET_ENV, TOKEN_ENV, spool,
    };
    use purlis_core::state::{Event, Notified};
    use std::cell::RefCell;
    use std::sync::{Mutex, mpsc};

    /// The chat the host issued its token to.
    const CHAT: u32 = 7;

    /// How the host hears a report.
    #[derive(Clone, Copy, PartialEq)]
    enum Hears {
        /// At once.
        AtOnce,
        /// Not until the test lets it go ([`Host::let_go`]): far past the 250 ms a server waits
        /// for it (`A_NOTICE_TAKES_AT_MOST`), so the server spools its line.
        WhenLetGo,
    }

    /// A report the host was told: recorded by it live, or spooled beside its socket for the
    /// next drain, as `deliver_report` does when the host is slower than it waits.
    #[derive(Debug)]
    enum Heard {
        Live(Report),
        Spooled(Report),
    }

    impl std::ops::Deref for Heard {
        type Target = Report;

        fn deref(&self) -> &Report {
            match self {
                Heard::Live(report) | Heard::Spooled(report) => report,
            }
        }
    }

    /// A host listening as the app does, and the reports it hears.
    struct Host {
        _dir: tempfile::TempDir,
        socket: std::path::PathBuf,
        token: String,
        heard: mpsc::Receiver<Report>,
        /// Reports told by the spool whose live copy has not come yet ([`support::paired`]).
        awaiting: RefCell<Vec<Report>>,
        /// Held while the hearer is to stall; dropped, it lets the hearer go. Before the
        /// reading, which waits for its hearer as it is dropped.
        stall: RefCell<Option<mpsc::Sender<()>>>,
        _reading: purlis_core::hookwire::Reading,
    }

    fn host() -> Host {
        host_that(Hears::AtOnce)
    }

    fn host_that(hears: Hears) -> Host {
        let dir = tempfile::tempdir().expect("a directory");
        // Where the app binds it, so a report the server spools is kept (V63).
        let socket = support::hooks_socket(dir.path());
        let listener = Listener::bind(dir.path(), &socket).expect("a socket");
        let token = listener
            .tokens()
            .issue_to_this_process(CHAT)
            .expect("a token")
            .expose()
            .to_owned();
        let (into, heard) = mpsc::channel();
        let into = Mutex::new(into);
        let (stall, held) = mpsc::channel::<()>();
        let held = Mutex::new(held);
        let reading = listener.hear(Hearing {
            secret_exec: Box::new(|_, _, writer| {
                purlis_core::secrets::brokered::not_answered(writer)
            }),
            each: Box::new(move |report| {
                if hears == Hears::WhenLetGo {
                    // Until the test lets go; only ever disconnected, never sent to.
                    let _ = held.lock().unwrap().recv();
                }
                // A hearer let go late may run after the test has stopped listening.
                let _ = into.lock().unwrap().send(report);
                Ok(())
            }),
            answer: Box::new(|_, _| panic!("no ask")),
            noticed: Box::new(|_| {}),
            saved: Box::new(|_| {}),
            refused: Box::new(|_| Ok(())),
            tool: Box::new(|_| Ok(())),
            doing: Box::new(|_| {}),
            output: Box::new(|_| {}),
            touching: Box::new(|_| {}),
            blocked: Box::new(|_| {}),
            permission: Box::new(|_| None),
        });
        Host {
            _dir: dir,
            socket,
            token,
            heard,
            awaiting: RefCell::new(Vec::new()),
            stall: RefCell::new(Some(stall)),
            _reading: reading,
        }
    }

    impl Host {
        /// A server for chat 7 of `harness`, handed this host's socket and token.
        fn server(&self, root: &Path, harness: &str) -> Server {
            Server::start_with(
                root,
                "alpha",
                &[
                    (SOCKET_ENV, self.socket.as_os_str()),
                    (CHAT_ENV, CHAT.to_string().as_ref()),
                    (TOKEN_ENV, self.token.as_ref()),
                    (HARNESS_ENV, harness.as_ref()),
                ],
            )
        }

        /// What the host was told since it was last asked, each report once: spooled, then
        /// recorded live (`support::told`'s order and why).
        ///
        /// **Nothing is waited for**, and the server is still running: read only where the
        /// test knows every report the server means to send is sent. A server says the wait
        /// before it hands the harness the question, and its end before it answers the call
        /// that ended it or exits, and `deliver_report` returns only once the host has recorded
        /// the line or it is durable in the spool (FD-30). So once the harness has read that
        /// answer, no append is in progress for the strict drain to meet half written.
        fn told(&self) -> Vec<Heard> {
            let spooled = support::spooled_running(&self.socket, CHAT, |line| match line {
                spool::Spooled::Report(report) => Some(report),
                _ => None,
            });
            // A report the server spooled because the host was slow is recorded live too once
            // the host gets to it, maybe by a later read: that copy is not said again. Two
            // live copies are two reports said (`support::paired`).
            let (spooled, live) = support::paired(
                spooled,
                self.heard.try_iter(),
                &mut self.awaiting.borrow_mut(),
            );
            let mut heard: Vec<Heard> = spooled.into_iter().map(Heard::Spooled).collect();
            heard.extend(live.into_iter().map(Heard::Live));
            heard
        }

        /// Whether every report told by the spool has had its live copy recorded too.
        fn every_late_copy_came(&self) -> bool {
            self.awaiting.borrow().is_empty()
        }

        /// The one report told since the host was last asked.
        fn said(&self) -> Heard {
            let mut told = self.told();
            assert_eq!(told.len(), 1, "one report: {told:?}");
            told.remove(0)
        }

        /// The hearer, stalled, goes on: it records what it was holding, late.
        fn let_go(&self) {
            drop(self.stall.borrow_mut().take());
        }

        /// The server says nothing more: once its harness has let go and it has exited, and so
        /// has no report left to send, the host was told nothing it had not been.
        fn nothing_more(&self, server: &mut Server) {
            server.hang_up();
            let told = self.told();
            assert!(told.is_empty(), "the host heard {told:?}");
        }
    }

    #[test]
    fn on_codex_the_server_says_the_form_waits_and_then_that_it_was_answered() {
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "codex");
        server.initialize(true);

        let asked = ask(&mut server, "Merge the train?");
        let waits = host.said();
        assert_eq!((waits.chat, waits.event), (7, Event::Notification));
        assert_eq!(waits.detail.notified, Notified::Asks(Prompt::Form));
        assert!(
            !format!("{waits:?}").contains("Merge the train"),
            "no word of the question is on the line: {waits:?}"
        );

        answer(&mut server, &asked, "yes");
        let ends = host.said();
        assert_eq!(ends.detail.notified, Notified::Answered);
        host.nothing_more(&mut server);
    }

    /// #2409: a host slower than the server waits is told the wait by the spool. The server
    /// does not wait past `A_NOTICE_TAKES_AT_MOST`; it spools the report and asks the harness.
    /// The host here records nothing until the test has read what it was told, so only the
    /// spool can hold the wait then.
    #[test]
    fn a_wait_the_host_is_too_slow_to_take_is_in_the_spool_when_the_question_is_asked() {
        let p = project();
        let host = host_that(Hears::WhenLetGo);
        let mut server = host.server(p.path(), "codex");
        server.initialize(true);

        let asked = ask(&mut server, "Merge the train?");
        let waits = host.said();
        assert!(matches!(waits, Heard::Spooled(_)), "{waits:?}");
        assert_eq!((waits.chat, waits.event), (CHAT, Event::Notification));
        assert_eq!(waits.detail.notified, Notified::Asks(Prompt::Form));

        // Let go, the host records the wait late too: the same report, told once.
        host.let_go();
        answer(&mut server, &asked, "yes");
        let ends = host.said();
        assert_eq!(ends.detail.notified, Notified::Answered);
        host.nothing_more(&mut server);
        // The hearer let go records the wait it held: it comes, and is not said again.
        let late = std::time::Instant::now();
        while !host.every_late_copy_came() {
            assert!(
                late.elapsed() < std::time::Duration::from_secs(10),
                "the hearer let go records the wait it held"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
            let told = host.told();
            assert!(told.is_empty(), "the late copy is not said again: {told:?}");
        }
    }

    /// A report said twice is two reports, though they are equal: the server says the wait
    /// once and its end once (#1089), and only a spooled report's own live copy is not
    /// counted again.
    #[test]
    fn a_report_said_twice_live_is_not_taken_for_one() {
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "codex");
        server.initialize(true);
        ask(&mut server, "Merge the train?");
        let waits = host.said();
        assert!(matches!(waits, Heard::Live(_)), "{waits:?}");

        // The same wait, said twice more, each on a line of its own.
        let token = ChatToken::read(&|name| (name == TOKEN_ENV).then(|| host.token.clone()))
            .expect("a token");
        let say = || {
            let said = purlis_core::hookwire::deliver_report(&host.socket, Some(&token), &waits);
            assert!(said.is_ok(), "{said:?}");
        };
        say();
        say();
        let told = host.told();
        assert_eq!(told.len(), 2, "two reports said: {told:?}");

        say();
        say();
        let one = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.said()));
        assert!(
            one.is_err(),
            "a report said twice is not the one report said"
        );
    }

    #[test]
    fn on_claude_code_the_server_says_nothing_since_claude_code_says_it() {
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "claude-code");
        server.initialize(true);
        let asked = ask(&mut server, "Merge the train?");
        answer(&mut server, &asked, "yes");
        host.nothing_more(&mut server);
    }

    #[test]
    fn a_question_that_never_reaches_the_person_is_not_said_to_wait() {
        // Refused before it asks, or on a harness that cannot ask: nobody waits.
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "codex");
        server.initialize(false);
        let said = server.call("ask_operator", json!({ "question": "Merge?" }));
        assert_eq!(said["isError"], true, "{said}");
        let said = server.call("ask_operator", json!({}));
        assert_eq!(said["isError"], true, "{said}");
        host.nothing_more(&mut server);
    }

    #[test]
    fn a_question_asked_by_returning_ends_its_wait_at_the_harness_s_next_call() {
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "opencode");
        let asked = server.request(
            "tools/call",
            json!({
                "_meta": stateless_meta(true),
                "name": "ask_operator",
                "arguments": { "question": "Merge the train?" },
            }),
        )["result"]
            .clone();
        assert_eq!(asked["resultType"], "input_required", "{asked}");
        let waits = host.said();
        assert_eq!(waits.detail.notified, Notified::Asks(Prompt::Form));

        server.request(
            "tools/call",
            json!({
                "_meta": stateless_meta(true),
                "name": "ask_operator",
                "arguments": { "question": "Merge the train?" },
                "inputResponses": { "answer": { "action": "decline" } },
                "requestState": asked["requestState"],
            }),
        );
        let ends = host.said();
        assert_eq!(ends.detail.notified, Notified::Answered);
        host.nothing_more(&mut server);
    }

    #[test]
    fn a_server_whose_harness_lets_go_mid_question_ends_its_wait() {
        let p = project();
        let host = host();
        let mut server = host.server(p.path(), "codex");
        server.initialize(true);
        ask(&mut server, "Merge the train?");
        host.said();
        // The harness closes the connection with the question unanswered.
        server.hang_up();
        let ends = host.said();
        assert_eq!(ends.detail.notified, Notified::Answered);
        host.nothing_more(&mut server);
    }
}

/// SK-16 (the operator's ruling of 2026-10-26 on #3229): a grade's chat is listed and answered
/// the tools that only read, and none that writes, dispatches, asks the person or drives a
/// browser.
#[test]
fn a_grades_chat_is_listed_and_answered_the_tools_that_only_read() {
    let p = project();
    let mut server = Server::start_as(p.path(), "alpha", &[], &["--graded"]);
    server.initialize(false);
    let listed = server.request("tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(!names.is_empty(), "{listed}");
    for name in &names {
        assert!(
            purlis_core::chattools::PRE_ALLOWED.contains(name),
            "{name} is listed to a grade's chat"
        );
    }
    for writes in [
        "todo_add",
        "memory_add",
        "dispatch",
        "ask_operator",
        "session_record",
    ] {
        assert!(!names.contains(&writes), "{writes}");
        let called = server.request("tools/call", json!({ "name": writes, "arguments": {} }));
        assert!(
            called.get("error").is_some(),
            "{writes} answered a grade's chat: {called}"
        );
    }
}
