// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fallback route** (SK-13, #2776; ADR 0063 as amended 2026-10-20): a harness with MCP and
//! no skill format gets its chat's skills through purlis's `skill_find` and `skill_load`, told so
//! in one line, and a load opens a skill run.
//!
//! The harness is the repo's fake one, a harness purlis has no adapter for: it starts
//! `purlis mcp` as its own child, as a harness starts its MCP servers, and hands that server's
//! stdio to this test over a socket (`fake-harness --mcp-relay`), so the test speaks to the tools
//! as its model would, and the server's line to the hook socket comes from inside the chat.

#![cfg(unix)]

use crate::support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{
    CHAT_ENV, HARNESS_ENV, Hearing, Listener, SOCKET_ENV, TOKEN_ENV, ToolCall, spool,
};
use serde_json::{Value, json};

const CHAT: u32 = 7;

/// The repo's fake harness, built beside this test's `purlis` when it is not there yet.
fn fake_harness() -> PathBuf {
    let beside = Path::new(support::PURLIS)
        .parent()
        .expect("the target folder")
        .join("fake-harness");
    if !beside.is_file() {
        let status = support::command(env!("CARGO"))
            .args([
                "build",
                "--quiet",
                "-p",
                "fake-harness",
                "--bin",
                "fake-harness",
            ])
            .status()
            .expect("cargo runs");
        assert!(status.success(), "fake-harness did not build: {status}");
    }
    beside
}

/// `goose`, a harness the project declares, which takes MCP servers and has no adapter.
const GOOSE: &str = "name = \"goose\"\ntitle = \"Goose\"\nprogram = \"goose\"\n\n[session]\n\n\
                     [terminal]\nready_to_type = \"raw-and-quiet\"\n\n[levels]\n\n\
                     [capabilities]\nmcp_servers = \"yes\"\n";

/// A project with one skill at its root, `deploy`, which takes tools away and has a script.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(
        dir.path().join(purlis_core::plane::MANIFEST),
        "schema = 2\n",
    )
    .expect("manifest");
    let deploy = dir.path().join("skills/deploy");
    std::fs::create_dir_all(&deploy).expect("a skill");
    std::fs::write(
        deploy.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ship the service to staging.\n\
         disallowed-tools: Write\n---\n\n# deploy\n\nRun ship.sh.\n",
    )
    .expect("SKILL.md");
    std::fs::write(deploy.join("ship.sh"), "echo ship\n").expect("a script");
    std::fs::create_dir_all(dir.path().join("harnesses")).expect("harnesses/");
    std::fs::write(dir.path().join("harnesses/goose.toml"), GOOSE).expect("a declaration");
    dir
}

/// The model's end of the relayed server: JSON-RPC lines both ways.
struct Model {
    out: UnixStream,
    lines: std::io::Lines<BufReader<UnixStream>>,
    next: u64,
}

impl Model {
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let line = self.lines.next().expect("an answer").expect("read");
        let answer: Value = serde_json::from_str(&line).expect("JSON-RPC");
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.out, "{message}").expect("write");
        self.out.flush().expect("flush");
    }

    fn call(&mut self, tool: &str, arguments: Value) -> String {
        let answer = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        answer["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("no text: {answer}"))
            .to_owned()
    }
}

#[test]
fn a_harness_purlis_has_no_adapter_for_finds_and_loads_a_skill_over_mcp_and_the_load_opens_a_run() {
    let p = project();
    let root = p.path().canonicalize().expect("the project");
    // The host, listening as the app does.
    let hooks = support::hooks_socket(&root);
    let listener = Listener::bind(&root, &hooks).expect("the hook socket");
    let token = listener
        .tokens()
        .issue_to_this_process(CHAT)
        .expect("a token")
        .expose()
        .to_owned();
    let (into, heard) = mpsc::channel::<ToolCall>();
    let into = Mutex::new(into);
    let _reading = listener.hear(Hearing {
        secret_exec: Box::new(|_, _, writer| purlis_core::secrets::brokered::not_answered(writer)),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| panic!("no ask")),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(move |call| {
            let _ = into.lock().unwrap().send(call);
            Ok(())
        }),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        blocked: Box::new(|_| {}),
        permission: Box::new(|_| None),
    });

    // The harness: its argv names purlis's MCP server and nothing of a skill; its environment is
    // the chat's place and its connection to the app, and a harness the project declares as
    // taking MCP servers, which purlis ships no adapter for.
    let relay = root.join("relay.sock");
    let relayed = UnixListener::bind(&relay).expect("the relay socket");
    let mut harness = support::command(fake_harness())
        .arg("--mcp-relay")
        .arg(&relay)
        .args(["--mcp-server", support::PURLIS, "mcp"])
        .current_dir(&root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &root)
        .env("CHARTER_ROOT", &root)
        .env("PURLIS_SKILL_STUDIO", "1")
        .env(HARNESS_ENV, "goose")
        .env(SOCKET_ENV, &hooks)
        .env(CHAT_ENV, CHAT.to_string())
        .env(TOKEN_ENV, &token)
        .stdin(Stdio::null())
        .spawn()
        .expect("the fake harness starts");
    let (stream, _) = relayed.accept().expect("the harness relays its server");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("a deadline");
    let mut model = Model {
        out: stream.try_clone().expect("the socket twice"),
        lines: BufReader::new(stream).lines(),
        next: 1,
    };

    // Its briefing: the one line, in what the server tells the harness at its start.
    let hello = model.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "goose", "version": "1" },
        }),
    );
    model.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    let instructions = hello["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        instructions.contains(purlis_core::skills::POINTER),
        "{instructions}"
    );
    let listed = model.request("tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    for tool in ["skill_list", "skill_runs", "skill_find", "skill_load"] {
        assert!(names.contains(&tool), "{tool}: {names:?}");
    }

    let found = model.call("skill_find", json!({ "query": "ship to staging" }));
    assert!(found.contains("`deploy` (project)"), "{found}");

    let loaded = model.call("skill_load", json!({ "name": "deploy" }));
    assert!(
        loaded.starts_with("Loaded the skill `deploy` (project)."),
        "{loaded}"
    );
    assert!(
        loaded.contains("Narrowing not applied: this skill takes away Write"),
        "a harness with no skill format cannot apply it: {loaded}"
    );
    assert!(
        loaded.contains(&root.join("skills/deploy/ship.sh").display().to_string()),
        "{loaded}"
    );
    assert!(loaded.ends_with("Run ship.sh.\n"), "{loaded}");

    // The load's run, on the line a tool hook sends, from inside the chat.
    let mut told: Vec<ToolCall> = heard.try_iter().collect();
    told.extend(support::spooled_running(&hooks, CHAT, |line| match line {
        spool::Spooled::Tool(call) => Some(call),
        _ => None,
    }));
    assert_eq!(told.len(), 1, "{told:?}");
    let line = &told[0];
    assert_eq!(line.chat, CHAT);
    assert_eq!(line.tool_hook, purlis_core::hookwire::SKILL_LOAD);
    assert_eq!(line.tool.as_deref(), Some("skill_load"));
    let started = line.skill.as_ref().expect("the run it starts");
    assert_eq!(started.skill, "deploy");
    assert_eq!(
        started.started_by,
        purlis_core::skills::runs::StartedBy::McpLoad
    );
    assert_eq!(started.run.len(), 26, "a ULID: {started:?}");
    assert_eq!(started.workspace, None, "a chat at the project's root");
    assert_eq!(
        started.level,
        Some(purlis_core::skills::runs::Level {
            origin: purlis_core::skills::Origin::Project,
            local: false,
        }),
        "the copy it loaded"
    );

    drop(model);
    let _ = harness.wait();
}
