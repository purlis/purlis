// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Claude Code's own sandbox, run on the settings purlis writes** (#2211).
//!
//! Claude Code is not wrapped: it builds its sandbox itself from the `sandbox` object and the
//! `permissions.deny` rules purlis hands a chat in `--settings` ([`sandbox::claude`]). So the
//! only way to know what that sandbox holds is to have the real Claude Code run a command under
//! it. Its Bash tool runs a command only when its model asks for one, so the model here is a
//! stand-in: a Messages server on a loopback port this test binds, which answers the chat's
//! first turn with one Bash call, `tools/seatbelt-case.sh --here`, and its second with an end.
//! What the command printed comes back to the stand-in as the tool's result, and is checked
//! here. No model is called and no credential is used: Claude Code is given
//! [`NOT_A_CREDENTIAL`], a literal no server but this stand-in accepts.
//!
//! For each level, the full sandbox and the floor, a sample project is laid out as
//! `the_profiles_a_wrapped_chat_starts_under_are_written_out.rs` lays it out, the settings a
//! Claude Code chat in it starts with are built the way a start builds them ([`sandbox::decide`],
//! then [`sandbox::Applied::form_in`] for the folder it opens in, and the chat's proxy ports), and
//! Claude Code is started there with them. Every probe of the script is asserted: what nothing
//! holds is written and read, and a held name, as written and in another case, is refused.
//!
//! The sample project also holds a `.claude/settings.json` of its own with keys beyond what a
//! sandboxed chat keeps (ADR 0067 as amended 2026-10-15), and Claude Code is started with the
//! setting sources purlis's start decides for it ([`purlis_core::harness::claude::project`]),
//! so the probes answer for what a chat in such a project runs under.
//!
//! Without `PURLIS_CLAUDE_CODE` (the path to the `claude` program) the settings are built and
//! checked, and no Claude Code is started. CI sets it on the macOS runner to the version in
//! `tests/fixtures/harness-versions/claude.txt`, which this test holds it to.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::harness::Harness;
use purlis_core::sandbox::{self, Decided, Form, Level, Machine, Os};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// What Claude Code is given as its API key: a literal, not a credential. Only the stand-in
/// below reads it, and it accepts anything.
const NOT_A_CREDENTIAL: &str = "stand-in";

/// The version of Claude Code purlis supports, as it answers `--version`.
const SUPPORTED: &str = include_str!("../../../../tests/fixtures/harness-versions/claude.txt");

/// The probes run as the stand-in's one Bash call.
const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/seatbelt-case.sh");

/// The probes `--here` asserts, each one answer: the sample project's eight, and its
/// repositories' six.
const PROBES: usize = 14;

/// The id of the stand-in's one Bash call.
const CALL: &str = "toolu_seatbelt_case";

/// Each level, its folder's name and the manifest of a project that starts its chats there.
const LEVELS: [(Level, &str, &str); 2] = [
    (
        Level::Full,
        "on",
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n",
    ),
    (Level::Floor, "floor", "schema = 1\n"),
];

/// The chat's proxy ports, HTTP and SOCKS5, as a start names them in its settings. Nothing
/// listens there: no probe reaches the network.
const PROXY_PORTS: [u16; 2] = [4040, 4041];

/// A project settings file with keys a sandboxed chat does not load, beside a deny rule it
/// does.
const PROJECT_SETTINGS: &str = r#"{
    "permissions": {"allow": ["Bash"], "deny": ["Bash(rm -rf /)"], "defaultMode": "acceptEdits"},
    "sandbox": {"enabled": false},
    "env": {"PROBE": "1"}
}"#;

/// `git` with `args`, run outside any sandbox with none of the person's git config, as a
/// project is laid out.
fn git(args: &[&str]) {
    let out = purlis_core::forklock::output(
        Command::new("git")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "user.name=probe",
                "-c",
                "user.email=probe@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args),
    )
    .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A sample project at `root` with `manifest`: a vault, a folder of what macOS starts at login
/// and one beside it, a file nothing holds, Claude Code settings of its own, and two
/// repositories, `repo`, a clone, and `wt`, a linked worktree of it, there as the chat starts.
fn lay_out(root: &Path, manifest: &str) {
    fs::create_dir_all(root.join(".claude")).expect("the project's .claude");
    fs::write(root.join(".claude/settings.json"), PROJECT_SETTINGS).expect("project settings");
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults folder");
    fs::create_dir_all(root.join("Library/LaunchAgents")).expect("a held folder");
    fs::create_dir_all(root.join("Library/LaunchAgentz")).expect("a folder beside it");
    fs::write(root.join(".purlis/vaults/ops.json"), "{}\n").expect("a vault");
    fs::write(root.join("notes.json"), "{}\n").expect("a file nothing holds");
    fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
    let repo = root.join("repo");
    let repo = repo.to_str().expect("a path");
    let worktree = root.join("wt");
    git(&["init", "-q", "-b", "main", repo]);
    git(&["-C", repo, "commit", "-q", "--allow-empty", "-m", "start"]);
    git(&[
        "-C",
        repo,
        "worktree",
        "add",
        "-q",
        "-b",
        "wt",
        worktree.to_str().expect("a path"),
    ]);
}

/// The sandbox part of the `--settings` a Claude Code chat in the project at `root` starts with
/// on `machine`, and the level it starts at.
///
/// No hook socket ([`sandbox::claude::Settings::reporting_on`] with `None`): no app listens in
/// this test, and a socket only adds that one path to the sockets a command may reach and its
/// folder to what is never written, which no probe here touches.
fn settings_for(root: &Path, machine: &Machine) -> (Level, Value) {
    let decided = sandbox::decide(Harness::ClaudeCode, root, machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    let Form::ClaudeCode(compiled) = applied.form_in(Some(root)) else {
        panic!("not Claude Code's settings");
    };
    let settings = compiled.reporting_on(None).through(PROXY_PORTS);
    (
        applied.level(),
        json!({
            "sandbox": settings.sandbox,
            "permissions": { "deny": settings.deny },
        }),
    )
}

/// A stand-in for the Messages API: the chat's first turn is answered with one Bash call that
/// runs `command`, and every later one, or any request that offers no Bash tool, with an end.
/// What the call's result said is kept in `results`.
struct StandIn {
    command: String,
    results: Arc<Mutex<Vec<String>>>,
}

impl StandIn {
    /// The text of each `tool_result` for the stand-in's call in `body`, if there is one.
    fn result_in(body: &Value) -> Option<String> {
        let messages = body["messages"].as_array()?;
        let mut found = None;
        for block in messages
            .iter()
            .filter_map(|message| message["content"].as_array())
            .flatten()
        {
            if block["type"] != "tool_result" || block["tool_use_id"] != CALL {
                continue;
            }
            let text = match &block["content"] {
                Value::String(text) => text.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|part| part["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                other => other.to_string(),
            };
            found = Some(text);
        }
        found
    }

    fn offers_bash(body: &Value) -> bool {
        body["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "Bash"))
    }
}

/// One server-sent event of the Messages stream.
fn event(name: &str, data: &Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

/// A whole answer, `content` ending on `stop`: streamed when the request asked for a stream.
fn answer(model: &str, content: &Value, stop: &str, stream: bool) -> ResponseTemplate {
    let usage = json!({ "input_tokens": 1, "output_tokens": 1 });
    if !stream {
        return ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_stand_in",
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [content],
            "stop_reason": stop,
            "stop_sequence": null,
            "usage": usage,
        }));
    }
    let (start, delta) = if content["type"] == "tool_use" {
        (
            json!({ "type": "tool_use", "id": content["id"], "name": content["name"], "input": {} }),
            json!({ "type": "input_json_delta", "partial_json": content["input"].to_string() }),
        )
    } else {
        (
            json!({ "type": "text", "text": "" }),
            json!({ "type": "text_delta", "text": content["text"] }),
        )
    };
    let body = [
        event(
            "message_start",
            &json!({ "type": "message_start", "message": {
                "id": "msg_stand_in", "type": "message", "role": "assistant", "model": model,
                "content": [], "stop_reason": null, "stop_sequence": null, "usage": usage,
            }}),
        ),
        event(
            "content_block_start",
            &json!({ "type": "content_block_start", "index": 0, "content_block": start }),
        ),
        event(
            "content_block_delta",
            &json!({ "type": "content_block_delta", "index": 0, "delta": delta }),
        ),
        event(
            "content_block_stop",
            &json!({ "type": "content_block_stop", "index": 0 }),
        ),
        event(
            "message_delta",
            &json!({ "type": "message_delta",
                "delta": { "stop_reason": stop, "stop_sequence": null },
                "usage": { "output_tokens": 1 } }),
        ),
        event("message_stop", &json!({ "type": "message_stop" })),
    ]
    .concat();
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

impl Respond for StandIn {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let model = body["model"].as_str().unwrap_or("stand-in").to_owned();
        let stream = body["stream"] == true;
        if let Some(result) = Self::result_in(&body) {
            self.results.lock().expect("results").push(result);
            let done = json!({ "type": "text", "text": "done" });
            return answer(&model, &done, "end_turn", stream);
        }
        if !Self::offers_bash(&body) {
            let words = json!({ "type": "text", "text": "ok" });
            return answer(&model, &words, "end_turn", stream);
        }
        let call = json!({
            "type": "tool_use",
            "id": CALL,
            "name": "Bash",
            "input": { "command": self.command, "description": "Run the sandbox probes" },
        });
        answer(&model, &call, "tool_use", stream)
    }
}

/// `claude` at `program`, with no settings of the person's, no credential and nothing from
/// this process's environment but what is named here.
fn claude(program: &Path, home: &Path, tmp: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home)
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("TMPDIR", tmp)
        .env("SHELL", "/bin/zsh")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .stdin(Stdio::null());
    command
}

/// Claude Code at `program`, started in `root` with the settings at `settings` against a
/// stand-in model whose one Bash call runs the probes of `root` as `what`: what that call's
/// result said, the last time it was asked.
fn probed(
    program: &Path,
    home: &Path,
    tmp: &Path,
    root: &Path,
    settings: &Path,
    sources: &[String],
    what: &str,
) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let server = rt.block_on(MockServer::start());
    let results = Arc::new(Mutex::new(Vec::new()));
    let command = format!(
        "/bin/sh '{}' --here '{}' '{what}'",
        Path::new(SCRIPT)
            .canonicalize()
            .expect("the script")
            .display(),
        root.display()
    );
    rt.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/messages/count_tokens"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "input_tokens": 1 })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(StandIn {
                command,
                results: results.clone(),
            })
            .mount(&server)
            .await;
    });
    let log = tmp.join("claude.log");
    let out = fs::File::create(&log).expect("a log");
    let mut command = claude(program, home, tmp);
    command
        .current_dir(root)
        .env("ANTHROPIC_BASE_URL", server.uri())
        .env("ANTHROPIC_API_KEY", NOT_A_CREDENTIAL)
        .args(["-p", "Run the sandbox probes.", "--bare"])
        .args(sources)
        .arg("--settings")
        .arg(settings)
        .args([
            "--allowedTools",
            "Bash",
            "--output-format",
            "json",
            "--no-session-persistence",
        ])
        .stdout(out.try_clone().expect("the log"))
        .stderr(out);
    let mut child = purlis_core::forklock::spawn(&mut command).expect("claude started");
    let deadline = Instant::now() + Duration::from_secs(300);
    let status = loop {
        if let Some(status) = child.try_wait().expect("claude waited on") {
            break status;
        }
        if Instant::now() > deadline {
            // The one process this test started.
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{what}: claude did not finish:\n{}",
                fs::read_to_string(&log).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let said = fs::read_to_string(&log).unwrap_or_default();
    let results = results.lock().expect("results").clone();
    let Some(result) = results.last() else {
        panic!("{what}: the Bash call's result never reached the stand-in ({status}):\n{said}");
    };
    result.clone()
}

#[test]
fn claude_codes_own_sandbox_holds_the_settings_purlis_writes() {
    purlis_core::unsteered!();
    let program = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from);
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    fs::create_dir_all(home.join(".claude")).expect("a home");
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::MacOs,
    };
    if let Some(program) = &program {
        if !cfg!(target_os = "macos") {
            panic!("Claude Code's sandbox is probed on macOS");
        }
        let version = purlis_core::forklock::output(claude(program, &home, &out).arg("--version"))
            .expect("claude --version");
        assert_eq!(
            String::from_utf8_lossy(&version.stdout).trim(),
            SUPPORTED.trim(),
            "the Claude Code probed is the version purlis supports"
        );
    }
    let mut probed_levels = 0;
    for (level, name, manifest) in LEVELS {
        let root = out.join(name).join("project");
        let tmp = out.join(name).join("tmp");
        lay_out(&root, manifest);
        fs::create_dir_all(&tmp).expect("a temporary folder");
        let (at, settings) = settings_for(&root, &machine);
        assert_eq!(at, level, "Claude Code in the {name} project");
        assert_eq!(settings["sandbox"]["enabled"], true, "{name}: {settings}");
        // As a start decides it for a chat in this project.
        let screened = purlis_core::harness::claude::project::screen_with(Some(&root), None);
        let sources = screened.setting_sources();
        assert_eq!(
            sources,
            ["--setting-sources", "user"],
            "{name}: {screened:?}"
        );
        let file = out.join(format!("claude-{name}.json"));
        fs::write(&file, settings.to_string()).expect("the settings");
        let Some(program) = &program else {
            continue;
        };
        let what = format!("claude-code at {name}");
        let result = probed(program, &home, &tmp, &root, &file, &sources, &what);
        // Each wrong answer as the script wrote it, so the run's annotations name it.
        for line in result.lines().filter(|line| line.starts_with("::error::")) {
            println!("{line}");
        }
        let verdict = format!("seatbelt-case: {what}: {PROBES} checks, 0 wrong");
        assert!(
            result.lines().any(|line| line.trim() == verdict),
            "{what}: not every probe answered as it should:\n{result}"
        );
        probed_levels += 1;
    }
    if program.is_some() {
        assert_eq!(probed_levels, 2, "Claude Code probed at each level");
    }
}
