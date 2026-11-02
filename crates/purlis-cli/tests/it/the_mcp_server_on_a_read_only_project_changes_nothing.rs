// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! On a project this purlis cannot write (FR-24), `purlis mcp` still serves a chat: its read
//! tools answer, and every tool that writes is refused (#1051, folded into #1089). This holds
//! the second half: on a lived-in project, **every** tool the server lists is called, and the
//! whole tree is compared with what it was after each call.
//!
//! The project is lived in so that a write made on the way is not hidden by an empty project's
//! early return: a workspace with a vision, a memory, a todo, a change record and a session
//! record. It is then made to name a feature this build lacks, as a teammate on a newer purlis
//! would leave it, the way `a_project_charter_cannot_write_is_read_only.rs` does.

use crate::support;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// What `charter.toml` starts with once a newer purlis has used the project: a feature this
/// build lacks, so it is read-only here.
const REQUIRES: &str = "schema = 2\n\
    requires = [{ feature = \"memory-proposals\", since = \"0.9.0\" }]\n";

/// The session record the project holds, by its file name.
const RECORD: &str = "20260501-120000-the-widget.md";

/// The tools that only read, which answer on a read-only project.
const READS: [&str; 5] = [
    "todo_list",
    "memory_search",
    "session_record_list",
    "session_record_read",
    "change_status",
];

/// The tools that write, each refused on a read-only project. The rest (`persona_where`,
/// `dispatch_list`, `ask_operator`) ask the app or the person, whom no test has, and are held
/// only to changing nothing.
const WRITES: [&str; 9] = [
    "todo_add",
    "todo_done",
    "memory_add",
    "persona_remember",
    "session_record",
    "workspace_vision",
    "workspace_section",
    "dispatch",
    "dispatch_report",
];

struct Project {
    _tmp: tempfile::TempDir,
    fence: PathBuf,
    root: PathBuf,
    home: PathBuf,
}

impl Project {
    /// A project a purlis has been used on, which then names a feature this purlis lacks.
    fn lived_in() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let fence = tmp.path().canonicalize().unwrap();
        let root = fence.join("project");
        let home = fence.join("home");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let project = Self {
            _tmp: tmp,
            fence,
            root,
            home,
        };
        for args in [
            &["init", "--forge", "github", "--owner", "acme"][..],
            &[
                "workspace",
                "create",
                "alpha",
                "--vision",
                "Ship the widget",
            ],
            &[
                "workspace",
                "remember",
                "The widget ships on Friday",
                "-w",
                "alpha",
            ],
            &["workspace", "todo", "Paint the widget", "-w", "alpha"],
            &[
                "change",
                "create",
                "widget-v2",
                "--why",
                "Ship the second widget",
                "-w",
                "alpha",
            ],
        ] {
            let out = project.command(args).output().expect("the binary runs");
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let sessions = project.root.join("workspaces/alpha/sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(
            sessions.join(RECORD),
            "---\ntitle: The widget\ndate: 2026-05-01 12:00:00\nchat: 1\nchat-name: unknown\n\
             persona: none\nharness: claude\nprofile: unknown\nconversation: unknown\n\
             workspace: alpha\ncwd: unknown\n---\n\n# The widget\n\n## Goal\nShip it.\n",
        )
        .unwrap();
        let manifest = project.root.join("charter.toml");
        let written = std::fs::read_to_string(&manifest).unwrap();
        let required = written.replacen("schema = 1\n", REQUIRES, 1);
        assert_ne!(written, required, "init writes `schema = 1`: {written}");
        std::fs::write(&manifest, required).unwrap();
        project
    }

    /// `purlis <args>` on the project, with nothing of this process's environment but a PATH.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("CHARTER_PLANE_FENCE", &self.fence)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .env("NO_COLOR", "1");
        command
    }

    /// The slug of the one todo the project holds.
    fn todo(&self) -> String {
        let todos = self.root.join("workspaces/alpha/todos");
        let mut slugs: Vec<String> = std::fs::read_dir(&todos)
            .expect("alpha's todos")
            .filter_map(Result::ok)
            .filter_map(|entry| {
                Path::new(&entry.file_name())
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .collect();
        slugs.sort();
        slugs.into_iter().next().expect("a todo")
    }

    /// Every file and folder under the project, with each file's bytes.
    fn tree(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(dir: &Path, base: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let rel = path.strip_prefix(base).unwrap().to_path_buf();
                if path.is_dir() {
                    into.insert(rel, Vec::new());
                    walk(&path, base, into);
                } else {
                    into.insert(rel, std::fs::read(&path).unwrap());
                }
            }
        }
        let mut tree = BTreeMap::new();
        walk(&self.root, &self.root, &mut tree);
        tree
    }
}

/// A running `purlis mcp` for a chat in workspace `alpha` of `project`.
struct Server {
    /// Where the server's standard error goes, said when the server stops answering.
    said: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    fn start(project: &Project) -> Self {
        let mut child = project
            .command(&["mcp"])
            .env("CHARTER_ROOT", &project.root)
            .env("CHARTER_WORKSPACE", "alpha")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                std::fs::File::create(project.fence.join("mcp.stderr")).expect("a log"),
            ))
            .spawn()
            .expect("purlis mcp starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            said: project.fence.join("mcp.stderr"),
            child,
            stdin,
            stdout,
            next: 1,
        }
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read");
        if line.is_empty() {
            let status = self.child.wait().ok();
            let said = std::fs::read_to_string(&self.said).unwrap_or_default();
            let tail: String = said
                .chars()
                .rev()
                .take(3000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            panic!("the server closed its stdout at {method} {params}, {status:?}: {tail}");
        }
        let answer: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON-RPC ({e}): {line:?}"));
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    /// `initialize`, at a version the harnesses speak, with no elicitation: `ask_operator`
    /// says it cannot ask, rather than waiting on a person no test has.
    fn initialize(&mut self) -> Value {
        let answer = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test-harness", "version": "1" },
            }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        answer
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What each tool is called with: everything it needs to go as far as it would on a project it
/// could write. A tool with no entry fails the test, so a tool added later is called here too.
fn arguments(tool: &str, todo: &str) -> Value {
    let record = "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n\
                  ## How to resume\n\nr\n";
    match tool {
        "todo_list" | "session_record_list" | "persona_where" | "dispatch_list" => json!({}),
        "dispatch_read" => json!({ "chat": 9 }),
        "todo_add" => json!({ "text": "Polish the widget" }),
        "todo_done" => json!({ "slug": todo }),
        "memory_search" => json!({ "query": "widget" }),
        "memory_add" => json!({ "text": "The widget is blue" }),
        "persona_remember" => json!({ "text": "Widgets ship on Fridays", "shared": true }),
        "session_record_read" => json!({ "file": RECORD }),
        "session_record" => json!({ "title": "Read only", "body": record }),
        "workspace_vision" => json!({ "text": "Ship every widget" }),
        // The app's to run, and refused on a project this purlis cannot write (#3831).
        "worktree_update" => json!({}),
        "worktree_remove" => json!({ "repo": "widget", "piece": "p1" }),
        "workspace_section" => json!({ "section": "decisions", "text": "Blue, because it sells" }),
        "change_status" => json!({ "change": "widget-v2" }),
        "dispatch" => json!({ "name": "paint", "brief": "Paint the widget" }),
        "dispatch_report" => json!({ "outcome": "done", "text": "Painted" }),
        // Every other dispatch subcommand's tool (#3786): the app's to answer.
        "dispatch_wait" => json!({ "chat": 9, "wait_seconds": 1 }),
        "dispatch_tell" | "dispatch_answer" => json!({ "chat": 9, "text": "Blue" }),
        "dispatch_cancel" | "dispatch_close" | "dispatch_pass_on" => json!({ "chat": 9 }),
        "dispatch_note" => json!({ "text": "Half painted" }),
        "dispatch_ask" => json!({ "question": "Which blue?", "wait_seconds": 1 }),
        "ask_operator" => json!({ "question": "Ship it?" }),
        // A browser tool (#2013) is the app's to answer, and writes nothing of the project's.
        browser if purlis_core::browsertools::is_one(browser) => json!({}),
        // The theme specimen (#2330) is the app's to draw, from its own fixture.
        "theme_preview" => json!({ "theme": "purlis-classic" }),
        other => panic!("no arguments for {other}: add it to this test"),
    }
}

fn text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn every_tool_leaves_every_file_of_a_read_only_project_as_it_was() {
    let project = Project::lived_in();
    let todo = project.todo();
    let before = project.tree();

    let mut server = Server::start(&project);
    let hello = server.initialize();
    assert_eq!(hello["result"]["serverInfo"]["name"], "purlis", "{hello}");
    let listed: Vec<String> = server.request("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
        .collect();
    let every: Vec<&str> = purlis_core::chattools::TOOLS
        .iter()
        .chain(purlis_core::browsertools::TOOLS.iter())
        .chain(purlis_core::themepreview::TOOLS.iter())
        .map(|tool| tool.name)
        .collect();
    assert_eq!(listed, every, "the server lists every tool");

    for tool in &listed {
        let said = server.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments(tool, &todo) }),
        );
        let result = &said["result"];
        assert!(result.is_object(), "{tool}: {said}");
        assert_eq!(project.tree(), before, "{tool} changed the project: {said}");
        if READS.contains(&tool.as_str()) {
            assert_ne!(result["isError"], true, "{tool} did not answer: {said}");
            assert!(!text(result).is_empty(), "{tool}: {said}");
        } else if WRITES.contains(&tool.as_str()) {
            assert_eq!(result["isError"], true, "{tool} was not refused: {said}");
        }
    }
}

#[test]
fn the_read_tools_answer_with_what_the_project_holds() {
    let project = Project::lived_in();
    let mut server = Server::start(&project);
    server.initialize();
    let call = |server: &mut Server, tool: &str, args: Value| {
        text(&server.request("tools/call", json!({ "name": tool, "arguments": args }))["result"])
    };

    assert!(
        call(&mut server, "todo_list", json!({})).contains("Paint the widget"),
        "the todo is listed"
    );
    assert!(
        call(&mut server, "memory_search", json!({ "query": "Friday" }))
            .contains("The widget ships on Friday"),
        "the memory is found"
    );
    assert!(
        call(
            &mut server,
            "session_record_read",
            json!({ "file": RECORD })
        )
        .contains("Ship it."),
        "the record is read"
    );
    assert!(
        call(
            &mut server,
            "change_status",
            json!({ "change": "widget-v2" })
        )
        .contains("widget-v2"),
        "the change is told"
    );
}
