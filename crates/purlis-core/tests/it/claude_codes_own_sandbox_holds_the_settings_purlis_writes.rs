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
//! checked, and no Claude Code is started. CI sets it on the macOS runner and on a Linux one to
//! the version in `tests/fixtures/harness-versions/claude.txt`, which this test holds it to. On
//! Linux purlis wraps every sandboxed Claude Code chat whole, with Claude Code's own sandbox off
//! (ADR 0067 as amended 2026-11-10), so nothing here runs there ([`its_own_sandbox_runs_here`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::support::git;
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

/// The probes `--here` asserts, each one answer: the sample project's eight, its
/// repositories' thirteen, four of the other harnesses' default sign-ins, two of a chat's temp
/// folder at work and seven names new to the project.
const PROBES: usize = 34;

/// The probes `--repositories-here` asserts, as it is run on Linux: the repositories' thirteen,
/// seven of the names in the chat's folder, four of the other harnesses' default sign-ins, and two
/// of a chat's temp folder at work.
const REPOSITORY_PROBES: usize = 26;

/// The id of the stand-in's one Bash call.
const CALL: &str = "toolu_seatbelt_case";

/// What a chat tries to make in its home.
const HOME_PATHS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/standard-home-paths.sh"
);

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

/// **Whether a sandboxed Claude Code chat here runs on Claude Code's own sandbox**: on macOS.
/// On Linux purlis wraps every sandboxed Claude Code chat whole (ADR 0067 as amended
/// 2026-11-10), with Claude Code's own sandbox off, so there is nothing of its own here to
/// probe.
fn its_own_sandbox_runs_here() -> bool {
    !Machine::this().purlis_wraps(Harness::ClaudeCode)
}

/// A sample project at `root` with `manifest`: a vault, a folder of what macOS starts at login
/// and one beside it, a file nothing holds, Claude Code settings of its own, and two clones and
/// a linked worktree, there as the chat starts.
fn lay_out(root: &Path, manifest: &str) {
    fs::create_dir_all(root.join(".claude")).expect("the project's .claude");
    fs::write(root.join(".claude/settings.json"), PROJECT_SETTINGS).expect("project settings");
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults folder");
    fs::create_dir_all(root.join("Library/LaunchAgents")).expect("a held folder");
    fs::create_dir_all(root.join("Library/LaunchAgentz")).expect("a folder beside it");
    fs::write(root.join(".purlis/vaults/ops.json"), "{}\n").expect("a vault");
    fs::write(root.join("notes.json"), "{}\n").expect("a file nothing holds");
    fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
    // Two clones and a linked worktree, there as the chat starts: `repo`, where git works,
    // `scratch`, where what is refused is tried, and `wt`, a worktree of `repo`.
    for clone in ["repo", "scratch"] {
        let at = root.join(clone);
        fs::create_dir_all(&at).expect("a clone's folder");
        git(&at, &["init", "-q", "-b", "main"]);
        git(&at, &["commit", "-q", "--allow-empty", "-m", "start"]);
    }
    let worktree = root.join("wt");
    git(
        &root.join("repo"),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "wt",
            worktree.to_str().expect("a path"),
        ],
    );
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
    // What a start makes before the chat starts ([`sandbox::Applied::line`]).
    compiled.make_empty();
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
        result_of(body, CALL)
    }

    fn offers_bash(body: &Value) -> bool {
        body["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "Bash"))
    }
}

/// The text of the last `tool_result` for the call `id` in `body`, if there is one.
fn result_of(body: &Value, id: &str) -> Option<String> {
    let messages = body["messages"].as_array()?;
    let mut found = None;
    for block in messages
        .iter()
        .filter_map(|message| message["content"].as_array())
        .flatten()
    {
        if block["type"] != "tool_result" || block["tool_use_id"] != id {
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

/// A stand-in for the Messages API that makes one Bash call per command of `commands`, in
/// turn, each once the one before it has its result, and then ends. What each call's result said
/// is kept in `results`, in the commands' order.
struct Steps {
    commands: Vec<String>,
    results: Arc<Mutex<Vec<String>>>,
}

impl Steps {
    fn call(step: usize) -> String {
        format!("toolu_step_{step}")
    }
}

impl Respond for Steps {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let model = body["model"].as_str().unwrap_or("stand-in").to_owned();
        let stream = body["stream"] == true;
        let said: Vec<String> = (0..self.commands.len())
            .map_while(|step| result_of(&body, &Self::call(step)))
            .collect();
        let next = said.len();
        if next > 0 {
            *self.results.lock().expect("results") = said;
        }
        let Some(command) = self
            .commands
            .get(next)
            .filter(|_| StandIn::offers_bash(&body))
        else {
            let done = json!({ "type": "text", "text": "done" });
            return answer(&model, &done, "end_turn", stream);
        };
        let call = json!({
            "type": "tool_use",
            "id": Self::call(next),
            "name": "Bash",
            "input": { "command": command, "description": "Run a step of the probes" },
        });
        answer(&model, &call, "tool_use", stream)
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
        .env(
            "SHELL",
            if cfg!(target_os = "macos") {
                "/bin/zsh"
            } else {
                "/bin/bash"
            },
        )
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
    let mode = if cfg!(target_os = "macos") {
        "--here"
    } else {
        "--repositories-here"
    };
    let command = format!(
        "/bin/sh '{}' {mode} '{}' '{what}'",
        Path::new(SCRIPT)
            .canonicalize()
            .expect("the script")
            .display(),
        root.display()
    );
    ran(program, home, tmp, root, settings, sources, what, command)
}

/// Claude Code at `program`, started in `root` with the settings at `settings` against a
/// stand-in model whose one Bash call runs `command`: what that call's result said, the last
/// time it was asked.
#[allow(clippy::too_many_arguments)]
fn ran(
    program: &Path,
    home: &Path,
    tmp: &Path,
    root: &Path,
    settings: &Path,
    sources: &[String],
    what: &str,
    command: String,
) -> String {
    ran_with(
        program,
        home,
        tmp,
        root,
        settings,
        sources,
        what,
        command,
        &[],
    )
}

/// [`ran`], with `env` set besides, as a chat's arming sets it.
#[allow(clippy::too_many_arguments)]
fn ran_with(
    program: &Path,
    home: &Path,
    tmp: &Path,
    root: &Path,
    settings: &Path,
    sources: &[String],
    what: &str,
    command: String,
    env: &[(&str, &Path)],
) -> String {
    let results = Arc::new(Mutex::new(Vec::new()));
    let stand_in = StandIn {
        command,
        results: results.clone(),
    };
    let (status, said) = started(
        program, home, tmp, root, settings, sources, what, env, stand_in,
    );
    let results = results.lock().expect("results").clone();
    let Some(result) = results.last() else {
        panic!("{what}: the Bash call's result never reached the stand-in ({status}):\n{said}");
    };
    result.clone()
}

/// Claude Code at `program`, started in `root` with the settings at `settings` against a
/// stand-in model making one Bash call per command of `commands`, in turn: what each call's
/// result said, in their order.
#[allow(clippy::too_many_arguments)]
fn ran_steps(
    program: &Path,
    home: &Path,
    tmp: &Path,
    root: &Path,
    settings: &Path,
    sources: &[String],
    what: &str,
    commands: Vec<String>,
) -> Vec<String> {
    let results = Arc::new(Mutex::new(Vec::new()));
    let wanted = commands.len();
    let stand_in = Steps {
        commands,
        results: results.clone(),
    };
    let (status, said) = started(
        program,
        home,
        tmp,
        root,
        settings,
        sources,
        what,
        &[],
        stand_in,
    );
    let results = results.lock().expect("results").clone();
    assert_eq!(
        results.len(),
        wanted,
        "{what}: not every Bash call's result reached the stand-in ({status}):\n{said}\n{results:?}"
    );
    results
}

/// Claude Code at `program`, started in `root` with the settings at `settings` and `env` besides,
/// against `stand_in` for its model: how it ended, and what it wrote to its output.
#[allow(clippy::too_many_arguments)]
fn started(
    program: &Path,
    home: &Path,
    tmp: &Path,
    root: &Path,
    settings: &Path,
    sources: &[String],
    what: &str,
    env: &[(&str, &Path)],
    stand_in: impl Respond + 'static,
) -> (std::process::ExitStatus, String) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let server = rt.block_on(MockServer::start());
    rt.block_on(async {
        Mock::given(method("POST"))
            .and(path("/v1/messages/count_tokens"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "input_tokens": 1 })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(stand_in)
            .mount(&server)
            .await;
    });
    let log = tmp.join("claude.log");
    let out = fs::File::create(&log).expect("a log");
    let mut command = claude(program, home, tmp);
    command.envs(env.iter().copied());
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
    (status, fs::read_to_string(&log).unwrap_or_default())
}

#[test]
fn claude_codes_own_sandbox_holds_the_settings_purlis_writes() {
    purlis_core::unsteered!();
    if !its_own_sandbox_runs_here() {
        return;
    }
    let program = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from);
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    fs::create_dir_all(home.join(".claude")).expect("a home");
    const OTHERS: [&str; 2] = [".codex/auth.json", ".local/share/opencode/auth.json"];
    const SIGN_IN: &str = "a harness's default sign-in";
    for file in OTHERS {
        let at = home.join(file);
        fs::create_dir_all(at.parent().expect("its folder")).expect("its folder");
        fs::write(at, SIGN_IN).expect("its sign-in");
    }
    fs::create_dir_all(&bin).expect("a folder on the PATH");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    if let Some(program) = &program {
        if !cfg!(any(target_os = "macos", target_os = "linux")) {
            panic!("Claude Code's sandbox is probed on macOS and Linux");
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
        // What purlis records as a chat starts, for the look after its turn.
        const CHAT: &str = "01M6M3VNN8GNE85XEV3194WJE6";
        let watched = sandbox::arrived::Watched {
            folders: vec![root.clone()],
            ..sandbox::arrived::Watched::default()
        };
        sandbox::arrived::begin(&root, CHAT, Some("probes"), &watched).expect("the record");
        // At the floor on macOS a chat can't make these names even in its temp folder, so what
        // the look is to find is laid out here, beside the project, for the chat to move in.
        if cfg!(target_os = "macos") && level == Level::Floor {
            let from = tmp.join("brought");
            fs::create_dir_all(from.join(".git/hooks")).expect("a repository to bring");
            fs::write(from.join(".git/hooks/pre-commit"), "x\n").expect("its hook");
        }
        let result = probed(program, &home, &tmp, &root, &file, &sources, &what);
        // Each wrong answer as the script wrote it, so the run's annotations name it.
        for line in result.lines().filter(|line| line.starts_with("::error::")) {
            println!("{line}");
        }
        // What the probes left in the project is told at the turn's end.
        let landed = root.join("brought/.git/hooks/pre-commit").exists();
        if cfg!(target_os = "macos") {
            assert!(landed, "{what}: nothing to look for:\n{result}");
        }
        let looked = sandbox::arrived::look(&root, CHAT, None)
            .expect("a look")
            .expect("the record");
        assert_eq!(
            looked
                .new
                .contains(&root.join("brought/.git").display().to_string()),
            landed,
            "{what}: {looked:?}"
        );
        if landed {
            assert_eq!(
                sandbox::arrived::told(&root).len(),
                1,
                "{what}: one Notice for the chat"
            );
        }
        let probes = if cfg!(target_os = "macos") {
            PROBES
        } else {
            REPOSITORY_PROBES
        };
        let verdict = format!("seatbelt-case: {what}: {probes} checks, 0 wrong");
        assert!(
            result.lines().any(|line| line.trim() == verdict),
            "{what}: not every probe answered as it should:\n{result}"
        );
        for file in OTHERS {
            assert_eq!(
                fs::read_to_string(home.join(file)).expect("still there"),
                SIGN_IN,
                "{what}: {file} was written"
            );
        }
        probed_levels += 1;
    }
    if program.is_some() {
        assert_eq!(probed_levels, 2, "Claude Code probed at each level");
    }
}

/// **What a chat at the floor can make in a home with no `~/.local` or `~/.config`** (#4357),
/// tried through Claude Code's own sandbox, once its start has made what it makes: the shared
/// folders are there for tools to use, and no other harness's folder is made or planted. Nothing
/// a sandbox put there to hold a path stays once the chat ends.
#[test]
fn a_chat_at_the_floor_makes_its_tools_folders_in_a_bare_home() {
    purlis_core::unsteered!();
    if !its_own_sandbox_runs_here() {
        return;
    }
    let Some(program) = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from) else {
        return;
    };
    let scratch = tempfile::tempdir().expect("a folder");
    let at = scratch.path().canonicalize().expect("a real folder");
    let home = at.join("home");
    let bin = at.join("bin");
    let root = at.join("project");
    let tmp = at.join("tmp");
    for folder in [&home.join(".claude"), &bin, &tmp] {
        fs::create_dir_all(folder).expect("a folder");
    }
    lay_out(&root, "schema = 1\n");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    let decided = sandbox::decide(Harness::ClaudeCode, &root, &machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    let (level, settings) = settings_for(&root, &machine);
    applied.make_usual_folders();
    assert_eq!(level, Level::Floor);
    let file = at.join("claude.json");
    fs::write(&file, settings.to_string()).expect("the settings");
    let screened = purlis_core::harness::claude::project::screen_with(Some(&root), None);
    let what = "claude-code";
    let command = format!(
        "/bin/sh '{}' '{}' '{what}' .codex .local/share/opencode",
        Path::new(HOME_PATHS)
            .canonicalize()
            .expect("the script")
            .display(),
        home.display()
    );
    let said = ran(
        &program,
        &home,
        &tmp,
        &root,
        &file,
        &screened.setting_sources(),
        what,
        command,
    );
    assert!(
        said.lines()
            .any(|line| line.trim() == "standard-paths: claude-code: 16 checks, 0 wrong"),
        "not every answer was as it should be:\n{said}"
    );
    for gone in [".codex", ".local/share/opencode"] {
        assert!(
            fs::symlink_metadata(home.join(gone)).is_err(),
            "{gone} stayed once the chat ended"
        );
    }
}

/// **An outcome eval's shell, run by Claude Code's own sandbox on what purlis compiles for it**
/// (#4476, the person's rulings of 2026-11-07): a command writes its piece and its own temp
/// folder, and is refused a write anywhere else, in the project, the home, the system's temp
/// folder and another shell eval's piece and temp folder, and a connection to a host the project
/// lists for its other chats, which purlis's own proxy refuses. The same project's other grade
/// chats are given that host and its package caches, so the hold is what takes them away.
#[test]
fn an_outcome_evals_shell_writes_only_its_own_folders_and_reaches_no_host() {
    purlis_core::unsteered!();
    if !its_own_sandbox_runs_here() {
        return;
    }
    use purlis_core::sandbox::egress;
    use purlis_core::skills::verbs::drafts;
    let program = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from);
    let scratch = tempfile::tempdir().expect("a folder");
    let at = scratch.path().canonicalize().expect("a real folder");
    let home = at.join("home");
    let bin = at.join("bin");
    let root = at.join("project");
    for folder in [&home.join(".claude"), &bin, &root] {
        fs::create_dir_all(folder).expect("a folder");
    }
    fs::write(
        root.join("charter.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\", \"toolchains\"]\n\
         hosts = [\"example.com\"]\n",
    )
    .expect("the manifest");
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    // Two shell evals' chats of one grade, each with its piece and its temp folder; both go,
    // temp folders and all, however the test ends.
    struct Ended<'a>(&'a Path, Vec<String>);
    impl Drop for Ended<'_> {
        fn drop(&mut self) {
            for id in &self.1 {
                let _ = drafts::end_try(self.0, id);
            }
        }
    }
    let arm = drafts::lay_out_arm(&root, None).expect("an arm's piece");
    let second = drafts::lay_out_arm(&root, None).expect("another arm's piece");
    let _ended = Ended(&root, vec![arm.id.clone(), second.id.clone()]);
    for one in [&arm, &second] {
        drafts::stage(&root, one, &[], &Default::default(), true).expect("staged with a shell");
    }
    let piece = arm.piece.clone();
    let temp = drafts::temp_of(&root, &piece).expect("its temp folder");
    let other_temp = drafts::temp_of(&root, &second.piece).expect("the other's temp folder");
    let decide = || {
        let decided = sandbox::decide(Harness::ClaudeCode, &root, &machine, &|_| true, None, None)
            .expect("decided")
            .expect("a sandbox in force");
        let Decided::Sandboxed(applied) = decided else {
            panic!("not sandboxed: {decided:?}");
        };
        applied
    };
    let settings_of = |applied: &sandbox::Applied| {
        let Form::ClaudeCode(compiled) = applied.form_in(Some(&piece)) else {
            panic!("not Claude Code's settings");
        };
        compiled
    };
    // Not vacuous: without the hold, a grade's chat here is given the host and the caches.
    let other = sandbox::policy::holding_no_kept_grants(decide);
    assert!(
        other
            .confines()
            .hosts
            .iter()
            .any(|host| host == "example.com"),
        "{:?}",
        other.confines().hosts
    );
    assert!(
        settings_of(&other).sandbox["filesystem"]
            .get("allowWrite")
            .is_some(),
        "the project's package caches"
    );
    // Claude Code's shared temp folder is denied to the shell alone: another grade's chat
    // keeps what Claude Code gives every chat.
    let denies_shared_temp = |compiled: &sandbox::claude::Settings| {
        let denied = compiled.sandbox["filesystem"]["denyWrite"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        sandbox::claude::SHARED_TEMP
            .iter()
            .all(|path| denied.contains(&json!(path)))
    };
    assert!(!denies_shared_temp(&settings_of(&other)));
    let held =
        sandbox::policy::holding_no_kept_grants(|| sandbox::policy::holding_to_its_folder(decide));
    assert_eq!(held.level(), Level::Full);
    assert!(
        held.confines().hosts.is_empty(),
        "{:?}",
        held.confines().hosts
    );
    let compiled = settings_of(&held);
    assert_eq!(compiled.sandbox["network"]["allowedDomains"], json!([]));
    assert!(
        compiled.sandbox["filesystem"].get("allowWrite").is_none(),
        "{}",
        compiled.sandbox
    );
    assert!(
        denies_shared_temp(&compiled),
        "Claude Code's shared temp folder: {}",
        compiled.sandbox
    );
    // Its commands are handed a folder inside its own, which Claude Code does only where that
    // is short enough (#5655): on macOS the system's temp folder is not.
    assert!(
        temp.join("claude-4294967295").as_os_str().len() <= drafts::CLAUDE_CODE_TEMP_MOST,
        "{}",
        temp.display()
    );
    let Some(program) = program else {
        return;
    };
    // purlis's own proxy, carrying what the chat was compiled with, as a start runs it.
    let refusals = egress::Refusals::default();
    let proxy = egress::Proxy::start_keeping(held.confines().hosts.clone(), refusals.clone())
        .expect("the chat's proxy");
    compiled.make_empty();
    let reporting = compiled
        .reporting_on(None)
        .through([proxy.port(), proxy.socks_port()]);
    let file = at.join("claude.json");
    fs::write(
        &file,
        // A grade's chat runs in the mode that asks (ADR 0105, amended 2026-10-26), whatever
        // mode the program would start in: no classifier decides its commands.
        json!({
            "sandbox": reporting.sandbox,
            "permissions": { "deny": reporting.deny, "defaultMode": "default" },
        })
        .to_string(),
    )
    .expect("the settings");
    let command = format!(
        "w() {{ if (mkdir -p \"$(dirname \"$1\")\" && echo x > \"$1\") 2>/dev/null; \
         then echo \"wrote $2\"; else echo \"refused $2\"; fi; }}; \
         w '{piece}/made.txt' piece; w \"$TMPDIR/made.txt\" temp; echo \"temp at $TMPDIR\"; \
         w '{root}/outside.txt' project; w '{home}/outside.txt' home; \
         w '{other_piece}/made.txt' other-piece; w '{other_temp}/made.txt' other-temp; \
         w /tmp/purlis-4476-$$ tmp; w /tmp/claude/purlis-4476-$$ claude-tmp; \
         w /private/tmp/claude/purlis-4476-$$ private-claude-tmp; \
         if curl -sS -m 20 -o /dev/null https://example.com/ 2>/dev/null; \
         then echo 'reached example.com'; else echo 'refused example.com'; fi",
        piece = piece.display(),
        root = root.display(),
        home = home.display(),
        other_piece = second.piece.display(),
        other_temp = other_temp.display(),
    );
    let what = "an outcome eval's shell";
    // As its start hands it its temp folder: the program's and Claude Code's.
    let said = ran_with(
        &program,
        &home,
        &temp,
        &piece,
        &file,
        &["--setting-sources".to_owned(), String::new()],
        what,
        command,
        &[("CLAUDE_CODE_TMPDIR", &temp)],
    );
    // Every probe, each with what the ruling wants, and every one that came out otherwise
    // named with this system, so a red run says which platform allowed or refused what.
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };
    let shared = if cfg!(target_os = "macos") {
        Some((
            "refused private-claude-tmp",
            "Claude Code's shared temp folder, by its real name",
        ))
    } else {
        None
    };
    let wrong: Vec<String> = [
        ("wrote piece", "it writes its piece"),
        ("wrote temp", "it writes its temp folder"),
        ("refused project", "the project outside its piece"),
        ("refused home", "the home"),
        ("refused other-piece", "another shell eval's piece"),
        ("refused other-temp", "another shell eval's temp folder"),
        ("refused tmp", "the system's temp folder"),
        (
            "refused claude-tmp",
            "Claude Code's shared temp folder, /tmp/claude",
        ),
        ("refused example.com", "a host the project lists"),
    ]
    .into_iter()
    .chain(shared)
    .filter(|(line, _)| !said.lines().any(|said| said.trim() == *line))
    .map(|(line, why)| format!("{why}: wanted `{line}`"))
    .collect();
    assert!(
        wrong.is_empty(),
        "{what}, on {os} (Claude Code {}), temp folder {}: {}\nwhat its command said:\n{said}",
        SUPPORTED.trim(),
        temp.display(),
        wrong.join("; ")
    );
    // Claude Code hands each command a temp folder inside the one the chat was given.
    assert!(
        said.lines().any(|line| line
            .trim()
            .strip_prefix("temp at ")
            .is_some_and(|at| Path::new(at).starts_with(&temp))),
        "{what}, on {os}: its commands' temp folder is not inside its own, {}:\n{said}",
        temp.display()
    );
    assert!(!root.join("outside.txt").exists() && !home.join("outside.txt").exists());
    assert!(!second.piece.join("made.txt").exists() && !other_temp.join("made.txt").exists());
    // Refused by purlis's proxy, where this Claude Code hands its connections to one.
    let refused = refusals.refused();
    assert!(
        refused.is_empty() || refused.iter().any(|one| one.starts_with("example.com")),
        "{refused:?}"
    );
}

/// **Two Claude Code chats each write a temporary folder of their own, and not the other's**
/// (#5675), through Claude Code's own sandbox, each started with the environment and
/// `--settings` of the line purlis builds for it under its own confinement. Before, every chat
/// on the machine was handed the one `/tmp/claude-<uid>`, so one chat's file of a name took the
/// place of another's. Binds the chats' proxies; runs where `PURLIS_CLAUDE_CODE` names Claude
/// Code, as CI sets it.
#[test]
fn two_claude_code_chats_each_write_a_temporary_folder_of_their_own_and_not_the_others() {
    purlis_core::unsteered!();
    if !its_own_sandbox_runs_here() {
        return;
    }
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let Some(program) = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from) else {
        return;
    };
    let scratch = tempfile::tempdir().expect("a folder");
    let at = scratch.path().canonicalize().expect("a real folder");
    let home = at.join("home");
    let bin = at.join("bin");
    let root = at.join("project");
    let tmp = at.join("tmp");
    for folder in [&home.join(".claude"), &bin, &tmp] {
        fs::create_dir_all(folder).expect("a folder");
    }
    lay_out(&root, LEVELS[0].2);
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    let decided = sandbox::decide(Harness::ClaudeCode, &root, &machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(mut applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    applied.answered(SUPPORTED);
    let Form::ClaudeCode(compiled) = applied.form_in(Some(&root)) else {
        panic!("not Claude Code's settings");
    };
    compiled.make_empty();
    let armed = json!({
        "sandbox": compiled.sandbox,
        "permissions": { "deny": compiled.deny },
    });
    let screened = purlis_core::harness::claude::project::screen_with(Some(&root), None);
    let confinements = [
        applied.confine().expect("started").expect("a proxy"),
        applied.confine().expect("started").expect("a proxy"),
    ];
    let lines = confinements.each_ref().map(|confinement| {
        applied
            .line(
                sandbox::Words {
                    program: program.display().to_string(),
                    command: Vec::new(),
                    armed: vec!["--settings".to_owned(), armed.to_string()],
                    charters: Vec::new(),
                },
                &sandbox::At {
                    cwd: Some(&root),
                    hook_socket: None,
                    confinement: Some(confinement),
                    no_opt_out: false,
                    purlis: None,
                },
            )
            .expect("a line")
    });
    let temps = confinements
        .each_ref()
        .map(|confinement| confinement.tmp().expect("a temporary folder").to_path_buf());
    assert_ne!(temps[0], temps[1], "each chat its own temporary folder");
    // Where Claude Code roots each chat's files: `claude-<uid>` in the folder it is named.
    let uid = fs::metadata(&at).expect("this user's folder").uid();
    let rooted = temps
        .each_ref()
        .map(|temp| temp.join(format!("claude-{uid}")));
    for folder in &rooted {
        fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(folder)
            .expect("its folder");
    }
    let script = at.join("temporary-folders.sh");
    fs::write(
        &script,
        "printf 'tmp=%s\\n' \"$TMPDIR\"\n\
         (echo mine > \"$TMPDIR/body.md\") 2>/dev/null && echo own=written || echo own=refused\n\
         (echo theirs > \"$1/body.md\") 2>/dev/null && echo other=written || echo other=refused\n",
    )
    .expect("the script");
    for (chat, line) in lines.iter().enumerate() {
        let what = format!("chat {chat}");
        let settings = line
            .args
            .iter()
            .position(|word| word == "--settings")
            .and_then(|flag| line.args.get(flag + 1))
            .expect("its settings");
        let file = at.join(format!("claude-{chat}.json"));
        fs::write(&file, settings).expect("the settings");
        // A script the call runs, as the probes are run: Claude Code's own permission check
        // reads a command's words, and a write it can see outside the project is not run.
        let other = rooted[1 - chat].display();
        let command = format!("/bin/sh '{}' '{other}'", script.display());
        let env: Vec<(&str, &Path)> = line
            .env
            .iter()
            .map(|(key, value)| (key.as_str(), Path::new(value)))
            .collect();
        let said = ran_with(
            &program,
            &home,
            &tmp,
            &root,
            &file,
            &screened.setting_sources(),
            &what,
            command,
            &env,
        );
        let named = said
            .lines()
            .find_map(|line| line.trim().strip_prefix("tmp="))
            .map(PathBuf::from)
            .unwrap_or_else(|| panic!("{what}: no $TMPDIR said:\n{said}"));
        let real = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        assert!(
            real(&named).starts_with(real(&temps[chat])),
            "{what}: $TMPDIR {named:?} is not in its own folder {:?}",
            temps[chat]
        );
        assert!(
            said.lines().any(|line| line.trim() == "own=written"),
            "{what}: its own temporary folder was not written:\n{said}"
        );
        assert!(
            said.lines().any(|line| line.trim() == "other=refused"),
            "{what}: the other chat's temporary folder was written:\n{said}"
        );
    }
    for (chat, folder) in rooted.iter().enumerate() {
        assert!(
            !folder.join("body.md").exists()
                || fs::read_to_string(folder.join("body.md")).expect("read") == "mine\n",
            "chat {chat}'s file was overwritten by the other chat"
        );
    }
    // As a chat ends, its folder goes.
    let [first, _second] = confinements;
    drop(first);
    assert!(
        !temps[0].exists(),
        "{:?} stayed once its chat ended",
        temps[0]
    );
}

/// The skill a chat writes in
/// [`a_workspace_made_after_a_claude_code_chat_starts_hands_no_chat_its_skills`].
const MADE_SKILL: &str = "---\nname: made\ndescription: Made by a chat.\n---\n";

/// A shell function, `w <path> <name>`, that writes [`MADE_SKILL`] at the path and says
/// `<name>=written` or `<name>=refused`.
fn write_function() -> String {
    format!(
        "w() {{ if (mkdir -p \"$(dirname \"$1\")\" && printf -- '{}' > \"$1\") 2>/dev/null; \
         then echo \"$2=written\"; else echo \"$2=refused\"; fi; }}\n",
        MADE_SKILL.replace('\n', "\\n")
    )
}

/// **A workspace made after a Claude Code chat starts hands no chat its skills** (#3097, SK-41;
/// ADR 0099 as amended 2026-11-09). purlis holds every workspace's skills folder by its path
/// and by a pattern with one any-name part, `workspaces/*/skills` ([`sandbox::skills_held`]), and
/// Claude Code reads the pattern itself. What it makes of it is measured here, through its own
/// sandbox, against a stand-in model whose chat runs two commands that try to give a workspace
/// made after the start a skill. Whether the pattern held it is printed, so CI's log keeps the
/// measurement.
///
/// On macOS the pattern holds it. On Linux, whatever it holds, what keeps such a skill from
/// every chat is purlis's record of the levels there at the chat's start
/// ([`purlis_core::skills::levels`]): each workspace made since reaches no chat while the record
/// stands, and a workspace there at the start still does. That half runs without Claude Code
/// too, with the chat's writes made by the test.
#[test]
fn a_workspace_made_after_a_claude_code_chat_starts_hands_no_chat_its_skills() {
    purlis_core::unsteered!();
    if !its_own_sandbox_runs_here() {
        return;
    }
    use purlis_core::skills::{self, PlaneSkills, levels};
    let program = std::env::var_os("PURLIS_CLAUDE_CODE").map(PathBuf::from);
    let scratch = tempfile::tempdir().expect("a folder");
    let at = scratch.path().canonicalize().expect("a real folder");
    let home = at.join("home");
    let bin = at.join("bin");
    let root = at.join("project");
    let tmp = at.join("tmp");
    for folder in [
        &home.join(".claude"),
        &bin,
        &tmp,
        &root.join("workspaces/ops"),
    ] {
        fs::create_dir_all(folder).expect("a folder");
    }
    // A sandboxed project whose skills reach its chats whatever this machine's flag says.
    fs::write(
        root.join("purlis.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n\n\
         [[requires]]\nfeature = \"project-skills\"\nsince = \"0.1.0\"\n",
    )
    .expect("the manifest");
    git(&root, &["init", "-q", "-b", "main"]);
    let machine = Machine {
        env: purlis_core::secrets::Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::this(),
    };
    let (level, settings) = settings_for(&root, &machine);
    assert_eq!(level, Level::Full);
    let pattern = root
        .join("workspaces")
        .join(sandbox::ANY_NAME)
        .join("skills");
    assert!(
        settings["sandbox"]["filesystem"]["denyWrite"]
            .as_array()
            .is_some_and(|denied| denied.contains(&json!(pattern.display().to_string()))),
        "the pattern is in what Claude Code is handed: {settings}"
    );
    // What a sandboxed start notes before the chat runs (`hoststart::compose_ready`).
    const CHAT: &str = "01M6YW8CYJ1AVN4BM9SDMZXNTK";
    levels::note(&root, CHAT, "41").expect("the record of the levels there");
    let skill = |workspace: &str| root.join(format!("workspaces/{workspace}/skills/made/SKILL.md"));
    // A workspace there at the start; one made in the command that writes its skill; and one
    // made in a command before the one that writes its skill.
    let [at_start, made_with_it, made_before_it] = [skill("ops"), skill("later"), skill("after")];
    let measured: Option<Vec<String>> = program.as_ref().map(|program| {
        let first = at.join("first.sh");
        let second = at.join("second.sh");
        fs::write(
            &first,
            format!(
                "{}w '{}' existing\nw '{}/workspaces/later/notes.md' level\n\
                 w '{}' same-command\n\
                 if mkdir -p '{}/workspaces/after/skills' 2>/dev/null; \
                 then echo later-folder=made; else echo later-folder=refused; fi\n",
                write_function(),
                at_start.display(),
                root.display(),
                made_with_it.display(),
                root.display(),
            ),
        )
        .expect("the first step");
        fs::write(
            &second,
            format!(
                "{}w '{}' later-command\n",
                write_function(),
                made_before_it.display()
            ),
        )
        .expect("the second step");
        let file = at.join("claude.json");
        fs::write(&file, settings.to_string()).expect("the settings");
        let screened = purlis_core::harness::claude::project::screen_with(Some(&root), None);
        // Scripts the calls run, as the probes are run: Claude Code's own permission check
        // reads a command's words, and a write it sees to a held path is not run at all.
        let said = ran_steps(
            program,
            &home,
            &tmp,
            &root,
            &file,
            &screened.setting_sources(),
            "a later workspace",
            vec![
                format!("/bin/sh '{}'", first.display()),
                format!("/bin/sh '{}'", second.display()),
            ],
        );
        said.iter()
            .flat_map(|step| step.lines())
            .map(|line| line.trim().to_owned())
            .filter(|line| line.contains('='))
            .collect()
    });
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };
    match &measured {
        Some(lines) => {
            let said = |line: &str| lines.iter().any(|one| one == line);
            let held = ["same-command", "later-command"]
                .iter()
                .all(|case| said(&format!("{case}=refused")));
            println!(
                "a mid-path pattern on {os}, Claude Code {}: holds a later workspace: {}",
                SUPPORTED.trim(),
                if held { "yes" } else { "no" }
            );
            assert!(said("existing=refused"), "{os}: {lines:?}");
            assert!(
                said("level=written"),
                "a workspace's folder is made: {lines:?}"
            );
            if cfg!(target_os = "macos") {
                for line in [
                    "same-command=refused",
                    "later-folder=refused",
                    "later-command=refused",
                ] {
                    assert!(
                        said(line),
                        "{os}: the pattern holds a later workspace: {lines:?}"
                    );
                }
            }
        }
        // No Claude Code, so no sandbox is tried: the chat's writes as they would land where
        // nothing held them, for the records to answer for.
        None => {
            for file in [&made_with_it, &made_before_it] {
                fs::create_dir_all(file.parent().expect("its folder")).expect("its folder");
                fs::write(file, MADE_SKILL).expect("a chat's skill");
            }
        }
    }
    assert!(
        !at_start.exists(),
        "a workspace there at the start keeps its hold"
    );
    // Whatever landed, no chat is handed a skill of a workspace made while the chat ran, and a
    // workspace there at the start still reaches its chats.
    let made_since = levels::made_since(&root);
    assert_eq!(
        made_since(&root.join("workspaces/ops")),
        None,
        "there at the start"
    );
    let ops = root.join("workspaces/ops/skills/kept/SKILL.md");
    fs::create_dir_all(ops.parent().expect("its folder")).expect("its folder");
    fs::write(&ops, "---\nname: kept\ndescription: There before.\n---\n").expect("a skill");
    for workspace in ["later", "after"] {
        let folder = root.join("workspaces").join(workspace);
        assert!(folder.is_dir(), "{workspace}: the chat made its folder");
        let why = made_since(&folder).unwrap_or_else(|| panic!("{workspace}: reaches chats"));
        assert!(why.contains("chat 41"), "{workspace}: {why}");
    }
    let names = |workspace: &str| -> Vec<String> {
        let set = skills::for_chat(
            purlis_core::harness::Kit {
                binary: Path::new("/bin/purlis"),
                plugin: None,
                persona: None,
                skills: Some(PlaneSkills {
                    plane: &root,
                    workspace: Some(workspace),
                    repo: &[],
                    own: &[],
                    routed: None,
                }),
                graded: false,
                judging: false,
                writes_its_piece: false,
                runs_a_shell: false,
                own: &[],
                laid_out: None,
            },
            Harness::ClaudeCode,
        );
        set.effective
            .iter()
            .map(|one| one.skill.name.clone())
            .collect()
    };
    assert_eq!(names("ops"), ["kept"], "a workspace there at the start");
    for workspace in ["later", "after"] {
        assert!(
            names(workspace).is_empty(),
            "{workspace} on {os}: {:?}",
            names(workspace)
        );
    }
}
