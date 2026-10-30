// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A sandbox block in what a Bash command came back with reaches the app as an operation and a
//! kind, and nothing more (#1338): never the path, the command or what it printed — not on the
//! block's line, not on the tool call's, and not on the hook's stderr, which goes to the
//! harness's log. The chat is told, in fixed words, that the person has a Notice and it should
//! say what was blocked and wait (#1631).
//!
//! Fed recorded violation lines, as Claude Code hands them to `PostToolUse` and
//! `PostToolUseFailure`.

use crate::support;

use std::process::Stdio;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{
    CHAT_ENV, HARNESS_ENV, Hearing, Listener, SANDBOXED_ENV, SOCKET_ENV, SandboxBlocked, TOKEN_ENV,
    ToolCall, spool,
};
use purlis_core::sandboxblock::{Block, CHAT_DIR_ENV, Kind, Operation};

const CANARY: &str = "CANARY-blocked-91c3";

/// What the host heard of one `purlis hook <word>` run as chat 7 with `payload`.
struct Heard {
    tool: Option<ToolCall>,
    blocked: Vec<SandboxBlocked>,
    said: String,
}

fn hook(word: &str, payload: &serde_json::Value) -> Heard {
    hook_in(word, payload, true)
}

/// [`hook`], in a chat the app started sandboxed or not.
fn hook_in(word: &str, payload: &serde_json::Value, sandboxed: bool) -> Heard {
    hook_of(word, payload, sandboxed, "claude")
}

/// [`hook_in`], in a chat of `harness`.
fn hook_of(word: &str, payload: &serde_json::Value, sandboxed: bool, harness: &str) -> Heard {
    let dir = tempfile::tempdir().expect("a directory");
    let path = support::hooks_socket(dir.path());
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tools, heard_tool) = mpsc::channel();
    let tools = Mutex::new(tools);
    let (blocks, heard_block) = mpsc::channel();
    let blocks = Mutex::new(blocks);
    let _reading = listener.hear(Hearing {
        secret_exec: Box::new(|_, _, writer| purlis_core::secrets::brokered::not_answered(writer)),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| panic!("no ask")),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(move |call| {
            let _ = tools.lock().unwrap().send(call);
            Ok(())
        }),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        blocked: Box::new(move |blocked| blocks.lock().unwrap().send(blocked).unwrap()),
        permission: Box::new(|_| None),
    });
    let mut command = support::purlis();
    command.env_clear();
    if sandboxed {
        command.env(SANDBOXED_ENV, "1");
    }
    let mut child = command
        .args(["hook", word])
        .current_dir(dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", "/Users/dev")
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .env(HARNESS_ENV, harness)
        .env(CHAT_DIR_ENV, "/Users/dev/plane/workspaces/a/repo")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("purlis runs");
    stand_in::feed(&mut child, payload.to_string().as_bytes());
    let out = child.wait_with_output().expect("purlis finishes");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains(CANARY),
        "the hook's stderr, which goes to the harness's log, names the path: {stderr}"
    );
    // Spooled, where the host was slower than the hook waits, or recorded live.
    let tool = support::told(&path, &heard_tool, |line| match line {
        spool::Spooled::Tool(call) => Some(call),
        _ => None,
    })
    .into_iter()
    .next();
    let blocked =
        std::iter::from_fn(|| heard_block.recv_timeout(Duration::from_millis(300)).ok()).collect();
    Heard {
        tool,
        blocked,
        said: String::from_utf8_lossy(&out.stdout).into_owned(),
    }
}

/// What a block word told the chat, from the one line it printed for `event`.
fn context_of(said: &str, event: &str) -> String {
    let line: serde_json::Value = serde_json::from_str(said.trim()).expect("one JSON line");
    assert_eq!(line["hookSpecificOutput"]["hookEventName"], event, "{said}");
    line["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("a context")
        .to_owned()
}

fn holds_nothing_of_the_call(heard: &Heard) {
    let mut lines: Vec<String> = heard
        .blocked
        .iter()
        // What a grant would name is the one thing a block carries of the call (#1342), and
        // only for the Notice's Allow: everything else on the line names nothing.
        .map(|one| {
            serde_json::to_string(&SandboxBlocked {
                target: None,
                ..one.clone()
            })
            .unwrap()
        })
        .collect();
    lines.extend(
        heard
            .tool
            .iter()
            .map(|call| serde_json::to_string(call).unwrap()),
    );
    for line in lines {
        for leak in [CANARY, "/Users/dev", "secret-title", "sessions"] {
            assert!(!line.contains(leak), "{leak} is on the line: {line}");
        }
    }
}

#[test]
fn a_violation_of_the_purlis_process_is_purlis_own_and_one_of_gh_is_not() {
    let error = format!(
        "Exit code 1\npurlis: could not save /Users/dev/plane/workspaces/a/sessions/{CANARY}.md\n\
         <sandbox_violations>\n\
         purlis(48211) deny(1) file-write-create /Users/dev/plane/workspaces/a/sessions/{CANARY}.md\n\
         gh(48212) deny(1) mach-lookup com.apple.trustd.agent\n\
         </sandbox_violations>"
    );
    let heard = hook(
        "posttoolusefailure-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_use_id": "toolu_1",
            "tool_input": {"command": format!("purlis session record --title secret-title {CANARY}")},
            "error": error,
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    let told = context_of(&heard.said, "PostToolUseFailure");
    assert!(
        told.contains("Report"),
        "purlis's own is a bug to report: {told}"
    );
    for leak in [CANARY, "/Users/dev", "secret-title", "sessions"] {
        assert!(
            !told.contains(leak),
            "{leak} is in what the chat is told: {told}"
        );
    }
    let tool = heard.tool.as_ref().expect("the host heard the tool call");
    assert_eq!(tool.tool_hook, "posttoolusefailure-blocked");
    assert_eq!(
        heard.blocked,
        vec![
            SandboxBlocked {
                chat: 7,
                sandbox_blocked: Block {
                    operation: Operation::Write,
                    // Not inside a project this hook can see, so outside it: under the home.
                    kind: Kind::Home,
                    ours: true,
                },
                harness: Some("claude".to_owned()),
                target: None,
                read_back: false,
                suggested: None,
            },
            SandboxBlocked {
                chat: 7,
                sandbox_blocked: Block {
                    operation: Operation::Lookup,
                    kind: Kind::CertificateCheck,
                    // gh's own refusal, in the same command: never purlis's.
                    ours: false,
                },
                harness: Some("claude".to_owned()),
                // The sandbox's own line names the one service a grant names (ADR 0093).
                target: Some("com.apple.trustd.agent".to_owned()),
                read_back: false,
                suggested: None,
            },
        ]
    );
    holds_nothing_of_the_call(&heard);
}

#[test]
fn a_command_that_came_back_with_a_refusal_on_stderr_is_a_block_of_the_chats_own() {
    let heard = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": format!("cargo build {CANARY}")},
            "tool_response": {
                "stdout": "",
                "stderr": format!("touch: /Users/dev/.cargo/registry/{CANARY}: Operation not permitted"),
                "interrupted": false,
            },
        }),
    );
    assert_eq!(
        heard
            .blocked
            .iter()
            .map(|one| one.sandbox_blocked)
            .collect::<Vec<_>>(),
        vec![Block {
            operation: Operation::Write,
            kind: Kind::ToolchainCache,
            ours: false,
        }]
    );
    // A write a person could grant carries its path, for the Notice's Allow (#1342).
    assert_eq!(
        heard.blocked[0].target.as_deref(),
        Some(format!("/Users/dev/.cargo/registry/{CANARY}").as_str())
    );
    holds_nothing_of_the_call(&heard);
}

#[test]
fn what_a_command_printed_on_standard_output_sends_no_block() {
    let heard = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat notes.txt"},
            "tool_response": {
                "stdout": format!("touch: /opt/{CANARY}: Operation not permitted"),
                "stderr": "",
            },
        }),
    );
    assert!(heard.blocked.is_empty(), "{:?}", heard.blocked);
    assert!(heard.tool.is_some(), "the call itself is still heard");
    assert_eq!(heard.said, "", "no block, so the chat is told nothing");
}

#[test]
fn a_chat_the_app_did_not_start_sandboxed_sends_no_block() {
    // A refusal there is macOS's or a file's own, not the sandbox's.
    let heard = hook_in(
        "posttoolusefailure-blocked",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "purlis todo add x"},
            "error": format!(
                "Exit code 1\n<sandbox_violations>\npurlis(1) deny(1) file-write-create \
                 /Users/dev/plane/{CANARY}\n</sandbox_violations>"
            ),
        }),
        false,
    );
    assert!(heard.blocked.is_empty(), "{:?}", heard.blocked);
    assert!(heard.tool.is_some(), "the call itself is still heard");
    assert_eq!(heard.said, "", "no block, so the chat is told nothing");
}

/// qw349, in git's measured words: a checkout the sandbox stopped at an editor's file came back
/// with exit status 0, at the new commit, with that file left behind. The chat is told what
/// the folder now is and the way back, in the one line the block word prints; a chat the app
/// did not sandbox is told nothing, since the refusal there is not the sandbox's.
#[test]
fn a_git_move_the_sandbox_stopped_partway_tells_the_chat_the_way_back() {
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git checkout feature"},
        "tool_response": {
            "stdout": "",
            "stderr": "error: unable to unlink old 'app/.vscode/tasks.json': Operation not \
                       permitted\nHEAD is now at 6d66ac1 two\nM\tapp/.vscode/tasks.json\n",
            "interrupted": false,
        },
    });
    let heard = hook("posttooluse-blocked", &payload);
    let told = context_of(&heard.said, "PostToolUse");
    assert!(told.contains("⬢ git stopped partway"), "{told}");
    assert!(told.contains("`app/.vscode/tasks.json`"), "{told}");
    assert!(told.contains("half-updated"), "{told}");
    assert!(
        told.contains("`git restore --source=HEAD --worktree -- app/.vscode/tasks.json`"),
        "{told}"
    );
    holds_nothing_of_the_call(&heard);
    let unsandboxed = hook_in("posttooluse-blocked", &payload, false);
    assert_eq!(unsandboxed.said, "", "not the sandbox's refusal");
}

/// #3861: a line the guard could not place (`cd -`) ran `git commit` in a branch folder, and
/// the sandbox refused the write of its index in the clone's `.git/worktrees`. The person got a
/// Notice for what was the chat's own work. Now the block still reaches the app for the record,
/// and the chat alone is told the command that commits there, with no Notice and no wait.
#[test]
fn a_block_in_a_branch_folders_git_record_is_told_to_the_chat_alone() {
    let record = "/Users/dev/plane/workspaces/a/repo/.git/worktrees/fix/index.lock";
    let heard = hook(
        "posttoolusefailure-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": {"command": "cd - && git commit -am wip"},
            "error": format!(
                "fatal: Unable to create '{record}': Operation not permitted\n\
                 <sandbox_violations>\n\
                 git(5) deny(1) file-write-create {record}\n\
                 </sandbox_violations>"
            ),
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    assert_eq!(
        heard
            .blocked
            .iter()
            .map(|one| (one.sandbox_blocked, one.target.as_deref()))
            .collect::<Vec<_>>(),
        vec![(
            Block {
                operation: Operation::Write,
                kind: Kind::ProtectedFile,
                ours: false,
            },
            Some(record),
        )],
        "kept in the record"
    );
    let told = context_of(&heard.said, "PostToolUseFailure");
    assert!(told.contains("`purlis worktree commit"), "{told}");
    assert!(told.contains("yours to retry"), "{told}");
    for never in ["Notice", "wait for their answer"] {
        assert!(!told.contains(never), "{never}: {told}");
    }
}

/// #1631, replayed: a command that reached three hosts came back with exit status 0, and Claude
/// Code's proxy said on its standard error that it refused each. No Notice was raised and the
/// chat, told nothing, went around the block.
#[test]
fn each_host_claude_codes_proxy_refused_reaches_the_app_and_the_chat_is_told_to_wait() {
    let heard = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": format!("curl -s -o /dev/null https://api.example.com/{CANARY}")},
            "tool_response": {
                "stdout": format!("https://api.example.com/{CANARY} 000\nhttps://console.example.com/ready 000\n"),
                "stderr": "\nShell cwd was reset to /Users/dev/plane/workspaces/a/repo\n\
                    <sandbox_violations>\n\
                    deny network-outbound api.example.com:443 (host is not on the allow list)\n\
                    deny network-outbound console.example.com:443 (host is not on the allow list)\n\
                    </sandbox_violations>",
                "interrupted": false,
            },
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    let host = Block {
        operation: Operation::Connect,
        kind: Kind::Host,
        ours: false,
    };
    assert_eq!(
        heard
            .blocked
            .iter()
            .map(|one| (one.sandbox_blocked, one.target.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (host, Some("api.example.com:443")),
            (host, Some("console.example.com:443")),
        ]
    );
    let told = context_of(&heard.said, "PostToolUse");
    for words in [
        "a connection to an internet host",
        "Notice",
        "wait for their answer",
    ] {
        assert!(told.contains(words), "{words}: {told}");
    }
    assert!(!told.contains(CANARY), "{told}");
    holds_nothing_of_the_call(&heard);
}

/// #1663, replayed: a dispatched task ran a database client through a brokered run with both
/// streams on standard output, and the host's lookup failed. The failure's error is all the
/// hook has, and no Notice came. Now the lookup is a block naming its host, which the chat is
/// not told back: its context stays fixed words.
#[test]
fn a_lookup_refused_inside_a_2_1_run_reaches_the_app_with_its_host() {
    let heard = hook(
        "posttoolusefailure-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": {"command": format!(
                "purlis secret exec prod-db -- sh -c 'usql \"$DSN\" -f {CANARY}.sql' 2>&1"
            )},
            "error": "Exit code 1\nerror: dial tcp: lookup db.prod.example.com: no such host\n",
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    let lookup = Block {
        operation: Operation::Lookup,
        kind: Kind::Host,
        ours: false,
    };
    assert_eq!(
        heard
            .blocked
            .iter()
            .map(|one| (one.sandbox_blocked, one.target.as_deref()))
            .collect::<Vec<_>>(),
        vec![(lookup, Some("db.prod.example.com"))]
    );
    let told = context_of(&heard.said, "PostToolUseFailure");
    assert!(told.contains("a lookup of an internet host"), "{told}");
    assert!(!told.contains("db.prod"), "fixed words only: {told}");
    holds_nothing_of_the_call(&heard);
}

/// qw338, replayed: a task chat ran `docker run … 2>&1 | tail`, the sandbox refused Docker its
/// socket, and the pipe ended with `tail`'s status, so Claude Code handed the result over as one
/// that came back. The block reached the app naming no socket, and its Notice had no Allow. A
/// line that runs a container client now names the socket; the same words from a line that only
/// reads raise no block at all (#1972).
#[test]
fn a_docker_run_down_a_pipe_reaches_the_app_naming_its_socket() {
    let said = "docker: permission denied while trying to connect to the docker API at \
                unix:///Users/dev/.docker/run/docker.sock: Head \
                \"http://%2FUsers%2Fdev%2F.docker%2Frun%2Fdocker.sock/_ping\": dial unix \
                /Users/dev/.docker/run/docker.sock: connect: operation not permitted\n\n\
                Run 'docker run --help' for more information\n";
    let came_back = |command: String| {
        hook(
            "posttooluse-blocked",
            &serde_json::json!({
                "hook_event_name": "PostToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": command},
                "tool_response": {"stdout": said, "stderr": "", "interrupted": false},
                "cwd": "/Users/dev/plane/workspaces/a/repo",
            }),
        )
    };
    let socket = Block {
        operation: Operation::Connect,
        kind: Kind::LocalSocket,
        ours: false,
    };
    let named = |heard: &Heard| {
        heard
            .blocked
            .iter()
            .map(|one| (one.sandbox_blocked, one.target.clone()))
            .collect::<Vec<_>>()
    };
    let ran = came_back(format!(
        "docker run --rm -v \"$PWD/api\":/src -w /src golang:1.26.6 sh -c 'go build -o \
         /tmp/{CANARY} ./cmd' 2>&1 | tail -40"
    ));
    assert_eq!(
        named(&ran),
        vec![(
            socket,
            Some("/Users/dev/.docker/run/docker.sock".to_owned())
        )]
    );
    let told = context_of(&ran.said, "PostToolUse");
    assert!(told.contains("a connection to a local socket"), "{told}");
    assert!(!told.contains("docker.sock"), "fixed words only: {told}");
    holds_nothing_of_the_call(&ran);
    let read = came_back(format!("cat {CANARY}.log | tail -40"));
    assert_eq!(named(&read), Vec::new());
    holds_nothing_of_the_call(&read);
}

/// #1884: a refusal a chat only read back, in the standard error of a background command it
/// polled, reaches the app marked as read back, so its Notice is a quiet line nobody has to
/// answer; and the chat is told that, not to wait. What its file tool read of that output
/// raises nothing (#1972). The same words from a command the chat ran are a block as ever.
#[test]
fn a_refusal_a_chat_only_read_back_reaches_the_app_marked_so_and_the_chat_is_not_told_to_wait() {
    let said = "docker: permission denied while trying to connect to the docker API at \
                unix:///Users/dev/.docker/run/docker.sock: dial unix \
                /Users/dev/.docker/run/docker.sock: connect: operation not permitted\n";
    let file_read = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {"file_path": "/private/tmp/claude-501/x/tasks/b1.output"},
            "tool_response": {"file": {"content": said}},
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    assert_eq!(file_read.blocked, Vec::new());
    let read = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "BashOutput",
            "tool_input": {"bash_id": "b1"},
            "tool_response": {
                "stdout": "",
                "stderr": "touch: /opt/tool/cache: Operation not permitted",
                "status": "completed",
            },
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    assert_eq!(read.blocked.len(), 1, "{:?}", read.blocked);
    assert!(read.blocked[0].read_back);
    assert_eq!(read.blocked[0].sandbox_blocked.operation, Operation::Write);
    let told = context_of(&read.said, "PostToolUse");
    assert!(told.contains("What you just read holds"), "{told}");
    assert!(told.contains("nothing waits on their answer"), "{told}");
    assert!(!told.contains("wait for their answer"), "{told}");
    assert!(!told.contains("docker.sock"), "fixed words only: {told}");
    // A command the chat ran that printed the same: a block of its own, and it waits.
    let ran = hook(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "make up 2>&1 | tail -40"},
            "tool_response": {"stdout": said, "stderr": "", "interrupted": false},
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
    );
    assert_eq!(ran.blocked.len(), 1);
    assert!(!ran.blocked[0].read_back);
    assert_eq!(ran.blocked[0].target, None);
    let told = context_of(&ran.said, "PostToolUse");
    assert!(told.contains("then wait for their answer"), "{told}");
}

/// #1663: an opencode chat runs behind purlis's own proxy, which tells the app each host it
/// refused by name; its hook leaves those to it, so one refusal is one Notice. A lookup it
/// still tells.
#[test]
fn a_chat_purlis_wraps_leaves_its_refused_hosts_to_the_proxy() {
    let heard = hook_of(
        "posttooluse-blocked",
        &serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "bash",
            "tool_response": {
                "stderr": "purlis's sandbox does not allow x.example:443: no egress preset or \
                           host of this project lists it\n\
                           curl: (6) Could not resolve host: api.example.com",
                "mixed": true,
            },
            "cwd": "/Users/dev/plane/workspaces/a/repo",
        }),
        true,
        "opencode",
    );
    assert_eq!(
        heard
            .blocked
            .iter()
            .map(|one| (one.sandbox_blocked.operation, one.sandbox_blocked.kind))
            .collect::<Vec<_>>(),
        vec![(Operation::Lookup, Kind::Host)]
    );
}
