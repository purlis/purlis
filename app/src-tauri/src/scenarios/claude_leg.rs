// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Claude Code leg** (T1.1-10, #6857): the scenario host with the real Claude Code binary
//! in place of the fake harness, under Claude Code's own sandbox, on macOS.
//!
//! On macOS purlis does not wrap Claude Code: it hands the chat the `sandbox` object and the
//! `permissions.deny` rules of `sandbox::claude` in `--settings`, and Claude Code builds its own
//! seatbelt from them, with its Bash tool's network going through purlis's proxy
//! (`httpProxyPort`, `socksProxyPort`). The fake harness cannot play that, so a cell that wants to
//! know what Claude Code's sandbox holds runs here.
//!
//! - **Started as production starts it**: [`Scenario::starts_claude_code`] composes the chat the
//!   window's way (`purlis_core::start::ready`, then the host's chat start), so the launch line,
//!   the environment, the bundled plugin and the settings are what purlis writes. Never a hand
//!   launch.
//! - **A stand-in model** ([`Model`]) answers its model calls: T1.1-03's stand-in (Anthropic's
//!   Messages API on a loopback port), asking for each step's Bash call in turn. No model is
//!   called and no credential is used. Claude Code's main process is unsandboxed on macOS and
//!   does not use purlis's proxy, so what it adds to the keep-list
//!   ([`ADDED`]) is a base URL for the stand-in, a key that is no credential, and the two
//!   variables that keep its own start-up traffic in: each named in the test plane's
//!   `charter.local.toml` `[chat_env] pass` ([`chat_env_pass`]), with its value in the scenario
//!   host's environment.
//! - **The transcript is the stand-in's request log**: what the real harness showed the model,
//!   in T1.1-03's projection, one line to a tool result in the shape the fake's own transcript
//!   has ([`transcript_of`]), so the cells' expectations and the global checks read the same.
//! - **A canary** ([`canary`]) fails the leg if Claude Code runs a command unsandboxed: one write
//!   outside the project and one direct connect, each seen refused in the tool's own result and
//!   from outside (the file a write left, a listener's connections), judged as the fake's canary
//!   is ([`super::checks::judge_canary`]).
//!
//! - **What this leg does not exercise**: the chat starts with `--permission-mode
//!   bypassPermissions` and the prompt after the window's own words, and the profile's first-run
//!   state accepts the bypass warning (`bypassPermissionsModeAccepted`,
//!   `skipDangerousModePermissionPrompt`), each listed in `switches.json`. So Claude Code's `ask`
//!   rules and its permission prompts are never reached here: only its sandbox and purlis's
//!   `permissions.deny` rules are what a cell can learn about.
//! - **An escape cell needs two witnesses**: the transcript is what the model was shown, which
//!   Claude Code's own words shape, so a cell that says a command got out pairs it with evidence
//!   from outside (a file left, a listener's connections), as the canary does.
//!
//! The leg runs on macOS with `PURLIS_CLAUDE_CODE` naming the `claude` at the version
//! `tests/fixtures/harness-versions/claude.txt` pins, checked at run time ([`program`]). On Linux it is reported not run: purlis
//! wraps Claude Code there itself (ticket 53). On CI a leg that cannot run fails.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_recordings::script::{Response, Script, Step};
use purlis_recordings::standin::{self, StandIn};
use serde_json::{Value, json};

use super::checks::{Canary, StepResult};
use super::host::{self, Scenario};

/// What Claude Code is given as its API key: a literal, not a credential. Only the stand-in reads
/// it, and it accepts anything.
pub(crate) const NOT_A_CREDENTIAL: &str = "stand-in-model-no-credential";

/// The variable that points Claude Code at the stand-in model.
pub(crate) const BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// **What the leg adds to a chat's keep-list**, each named in the plane's `[chat_env] pass`:
/// the stand-in's address and key, and the two variables that keep Claude Code's own start-up
/// traffic (an update check, telemetry) from going out, which its unsandboxed main process
/// would do without purlis's proxy. Each is in ticket 6's list of substitutions
/// (`switches.json`), covered by ticket 44's live pass.
pub(crate) const ADDED: [&str; 4] = [
    BASE_URL,
    "ANTHROPIC_API_KEY",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "DISABLE_AUTOUPDATER",
];

/// The project's local `[chat_env]` naming [`ADDED`], as an operator writes it.
pub(crate) fn chat_env_pass() -> String {
    let names: Vec<String> = ADDED.iter().map(|name| format!("\"{name}\"")).collect();
    format!("[chat_env]\npass = [{}]\n", names.join(", "))
}

/// The version `tests/fixtures/harness-versions/claude.txt` pins.
const PINNED: &str = include_str!("../../../../tests/fixtures/harness-versions/claude.txt");

/// What `program --version` prints.
fn version_of(program: &Path) -> String {
    purlis_core::forklock::output(std::process::Command::new(program).arg("--version"))
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// **The binary's version is the one pinned**: what `claude --version` said, against the pin's
/// own line, so a leg never runs on a version nobody recorded.
fn version_matches(said: &str, pinned: &str) -> Result<(), String> {
    if said.trim() == pinned.trim() {
        Ok(())
    } else {
        Err(format!(
            "{} is `{}`, and `tests/fixtures/harness-versions/claude.txt` pins `{}`",
            host::CLAUDE_CODE_VAR,
            said.trim(),
            pinned.trim()
        ))
    }
}

/// **The real Claude Code this run starts**, or none, saying why: on macOS the file
/// `PURLIS_CLAUDE_CODE` names. Elsewhere the leg is reported not run for `test` (purlis wraps
/// Claude Code on Linux, ticket 53's leg); on macOS without the program it cannot run, which on
/// CI fails ([`host::cannot_run`]).
pub(crate) fn program(test: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        let reason = "Claude Code runs its own sandbox on macOS alone; on Linux purlis wraps it \
                      (ticket 53's leg)";
        let _ = super::report::write(&[super::a_record_of(
            "T1.1-10",
            test,
            Some(purlis_scenarios::model::Dialect::Claude),
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::NotRun,
            reason,
        )]);
        return host::does_not_apply(reason);
    }
    let named = std::env::var_os(host::CLAUDE_CODE_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    match named {
        Some(program) if program.is_file() => {
            let said = version_of(&program);
            match version_matches(&said, PINNED) {
                Ok(()) => Some(program),
                Err(why) => host::cannot_run(&why),
            }
        }
        Some(program) => host::cannot_run(&format!(
            "{} names {}, which is not there",
            host::CLAUDE_CODE_VAR,
            program.display()
        )),
        None => host::cannot_run(&format!(
            "{} names no `claude`: install the version `tests/fixtures/harness-versions/claude.txt` \
             pins and name it",
            host::CLAUDE_CODE_VAR
        )),
    }
}

/// **The four variables of [`ADDED`], with their values**, for the scenario's child: the
/// stand-in's address is a loopback port this process picks and the child, which reads it back
/// from its environment, binds ([`Model::serve`]). `None` where no port can be bound here.
pub(crate) fn environment() -> Option<Vec<(String, String)>> {
    // Always this process's own pick: an `ANTHROPIC_BASE_URL` in the ambient environment is never
    // honoured, so a chat is never pointed at a server nobody here started. The port is freed
    // for the child to bind: the child is another process, so the listener cannot be held.
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(why) => return host::cannot_run(&format!("no loopback port to bind: {why}")),
    };
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("its address").port()
    );
    drop(listener);
    Some(
        [
            (BASE_URL, url.as_str()),
            ("ANTHROPIC_API_KEY", NOT_A_CREDENTIAL),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
            ("DISABLE_AUTOUPDATER", "1"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect(),
    )
}

/// **What a first run of Claude Code keeps of a person's answers**, written as a person who has
/// answered them would have: onboarding done, the folder trusted, the stand-in's key approved
/// and the bypass-permissions warning accepted, in the profile's own state at `config`, never in
/// purlis's settings. purlis seeds this for the Linux wrap alone (`claude_wrap.rs`); on macOS a
/// first run would stop at its questions.
pub(crate) fn seed_first_run(config: &Path, root: &Path) {
    let state = config.join(".claude.json");
    let mut value: Value = std::fs::read_to_string(&state)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    let tail: String = NOT_A_CREDENTIAL
        .chars()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    value["hasCompletedOnboarding"] = true.into();
    value["theme"] = "dark".into();
    value["bypassPermissionsModeAccepted"] = true.into();
    value["customApiKeyResponses"] = json!({"approved": [tail], "rejected": []});
    value["projects"][root.display().to_string()] =
        json!({"hasTrustDialogAccepted": true, "allowedTools": []});
    std::fs::create_dir_all(config).expect("the profile's folder");
    std::fs::write(
        &state,
        serde_json::to_vec_pretty(&value).expect("state is JSON"),
    )
    .expect("the profile's state");
    // Where Claude Code 2.1 reads that the person accepted the warning.
    let settings = config.join("settings.json");
    let mut own: Value = std::fs::read_to_string(&settings)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    own["skipDangerousModePermissionPrompt"] = true.into();
    std::fs::write(
        &settings,
        serde_json::to_vec_pretty(&own).expect("settings are JSON"),
    )
    .expect("the profile's settings");
}

/// One chat's stand-in, and the transcript file its request log is written to.
struct Chat {
    stand_in: Arc<StandIn>,
    file: Option<PathBuf>,
    /// What the file holds now, so it is rewritten only when the log moved.
    last: String,
}

/// **The stand-in model of a leg**: one server on the loopback port `ANTHROPIC_BASE_URL` names,
/// across the chats of a scenario, each with a stand-in and a script of its own
/// ([`Model::plays`]); each chat's request log is written to the file of its own
/// ([`Model::attach`]), which a later chat never touches.
pub(crate) struct Model {
    /// The stand-in that answers now: the newest chat's.
    current: Arc<Mutex<Arc<StandIn>>>,
    /// Every chat's stand-in and transcript file, in the order the chats were played.
    chats: Arc<Mutex<Vec<Chat>>>,
    stop: Arc<AtomicBool>,
}

impl Model {
    /// Serves on the port this child's environment names ([`environment`]).
    pub(crate) fn serve() -> Model {
        let url = std::env::var(BASE_URL).expect("the scenario's child names the stand-in's port");
        let port: u16 = url
            .rsplit(':')
            .next()
            .and_then(|port| port.trim_end_matches('/').parse().ok())
            .unwrap_or_else(|| panic!("{BASE_URL} is {url}: no loopback port"));
        let listener = TcpListener::bind(("127.0.0.1", port))
            .unwrap_or_else(|why| panic!("the stand-in model cannot listen on {port}: {why}"));
        Model::serve_on(Some(listener))
    }

    /// Serves on `listener`, or on none (a test that plays the stand-in itself).
    fn serve_on(listener: Option<TcpListener>) -> Model {
        let current = Arc::new(Mutex::new(Arc::new(StandIn::new(
            "claude",
            script_of("idle", &[]),
        ))));
        let chats: Arc<Mutex<Vec<Chat>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(listener) = listener {
            let current = Arc::clone(&current);
            std::thread::spawn(move || {
                let _ = standin::serve_on(
                    listener,
                    Arc::new(move || Arc::clone(&current.lock().expect("the stand-in"))),
                );
            });
        }
        {
            let (chats, stop) = (Arc::clone(&chats), Arc::clone(&stop));
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    for chat in chats.lock().expect("the chats").iter_mut() {
                        let Some(file) = chat.file.clone() else {
                            continue;
                        };
                        let text = transcript_of(&chat.stand_in)
                            .iter()
                            .map(Value::to_string)
                            .collect::<Vec<_>>()
                            .join("\n");
                        if text != chat.last || !file.exists() {
                            // Atomically: a reader never sees half a line.
                            let aside = file.with_extension("part");
                            if std::fs::write(&aside, format!("{text}\n")).is_ok() {
                                let _ = std::fs::rename(&aside, &file);
                            }
                            chat.last = text;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
        Model {
            current,
            chats,
            stop,
        }
    }

    /// **The next chat's script**: each of `steps` (a name and a Bash command) asked for in turn,
    /// once the one before has its result. Answers the prompt the chat starts with, which is how
    /// the stand-in tells the chat's requests from the harness's own. The chat has a stand-in
    /// of its own, kept for its transcript.
    pub(crate) fn plays(&self, steps: &[(&str, String)]) -> String {
        let mut chats = self.chats.lock().expect("the chats");
        let script = script_of(&format!("leg chat {}", chats.len() + 1), steps);
        let prompt = script.prompt.clone();
        let stand_in = Arc::new(StandIn::new("claude", script));
        *self.current.lock().expect("the stand-in") = Arc::clone(&stand_in);
        chats.push(Chat {
            stand_in,
            file: None,
            last: String::new(),
        });
        prompt
    }

    /// The transcript of the chat just played ([`Model::plays`]) is `file`, written from its
    /// request log. An earlier chat's file is left as it was.
    pub(crate) fn attach(&self, file: &Path) {
        self.chats
            .lock()
            .expect("the chats")
            .last_mut()
            .expect("a chat is played before it is attached")
            .file = Some(file.to_path_buf());
    }

    /// How many requests the current chat's stand-in has heard.
    pub(crate) fn requests(&self) -> usize {
        self.current
            .lock()
            .expect("the stand-in")
            .heard
            .lock()
            .expect("the log")
            .len()
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A script that asks for each of `steps` as a Bash call, in turn.
fn script_of(name: &str, steps: &[(&str, String)]) -> Script {
    Script {
        name: name.to_owned(),
        prompt: format!("Run the steps of {name}."),
        responses: steps
            .iter()
            .map(|(step, command)| Response {
                calls: vec![Step {
                    step: (*step).to_owned(),
                    kind: "bash".to_owned(),
                    command: Some(command.clone()),
                    path: None,
                    content: None,
                    old: None,
                    new: None,
                    url: None,
                    prompt: None,
                    background: false,
                }],
                parallel: false,
                esc: false,
                dialects: None,
            })
            .collect(),
        subagent: Vec::new(),
        then: Vec::new(),
    }
}

/// **The transcript, read from the stand-in's request log** (T1.1-03 item 7): what the real
/// harness showed the model, one value a line. A tool result is a `tool_result` line in the
/// shape the fake's transcript has: the `step` its call was asked for as, and its `exit` where the
/// result says one (Claude Code's Bash tool says `Exit code N` first on a failure and nothing on
/// a success), so [`StepResult::of`] and the cells read it alike. A result with no exit, such as
/// a call a hook denied, never ran.
pub(crate) fn transcript_of(stand_in: &StandIn) -> Vec<Value> {
    let sequence = stand_in.script.sequence();
    stand_in
        .shown()
        .into_iter()
        .map(|shown| {
            if shown.kind != "tool_result" {
                return json!({"kind": shown.kind, "text": shown.text});
            }
            let failed = shown.is_error == Some(true);
            let mut line = json!({
                "kind": "tool_result",
                "text": shown.text,
                "stdout": shown.text,
                "stderr": "",
                "is_error": failed,
            });
            if let Some(id) = &shown.tool_use_id {
                line["tool_use_id"] = json!(id);
                if let Some(step) = step_of(&sequence, id) {
                    line["step"] = json!(step);
                }
            }
            if let Some(exit) = exit_of(&shown.text, failed) {
                line["exit"] = json!(exit);
            }
            line
        })
        .collect()
}

/// The step the stand-in asked for as tool-use `id` (`toolu_cs_<response>_<call>`).
fn step_of(sequence: &[(u32, &Response)], id: &str) -> Option<String> {
    let mut parts = id.strip_prefix("toolu_cs_")?.split('_');
    let response: usize = parts.next()?.parse().ok()?;
    let call: usize = parts.next()?.parse().ok()?;
    Some(sequence.get(response)?.1.calls.get(call)?.step.clone())
}

/// The exit status a Bash tool result says: `Exit code N` as its first words, or 0 for a result
/// that is no error; none for an error that names no exit (a call that did not run).
fn exit_of(text: &str, failed: bool) -> Option<i64> {
    match text.strip_prefix("Exit code ") {
        Some(rest) => rest
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok(),
        None => (!failed).then_some(0),
    }
}

/// **The canary's two commands**, as the fake's canary runs them: a write outside the project to
/// `file` and a connect that does not go through the proxy, to `port` on this machine.
pub(crate) fn canary_commands(file: &Path, port: u16) -> [String; 2] {
    let script = super::checks::canary_script(file, port);
    let call = |at: usize| {
        script["turns"][0]["calls"][at]["run"]
            .as_str()
            .expect("the canary's command")
            .to_owned()
    };
    [call(0), call(1)]
}

/// **The leg's canary**: before anything else, the real Claude Code runs one write outside the
/// project and one direct connect, and both must be refused by its own sandbox, seen in each
/// tool result and from outside (the file a write left, a listener's connections). A canary that
/// sees either get through fails the leg ([`super::checks::judge_canary`]): Claude Code ran a
/// command unsandboxed, and nothing the leg says after that is evidence. The person then keeps
/// whatever the refusals asked blocked, and the chat is closed.
pub(crate) fn canary(scenario: &Scenario, model: &Model) {
    let outside = scenario.a_folder_outside("canary");
    let file = outside.join("written-by-the-chat");
    let listener = TcpListener::bind("127.0.0.1:0").expect("the canary's listener");
    listener.set_nonblocking(true).expect("non-blocking");
    let port = listener.local_addr().expect("its address").port();
    let dialled = |counted: &mut usize| {
        while listener.accept().is_ok() {
            *counted += 1;
        }
        *counted
    };
    let mut accepted = 0;
    // The positive controls, from this process, outside Claude Code's sandbox: it can write there
    // and dial there, so a refusal below is Claude Code's.
    std::fs::write(outside.join("written-by-the-test"), "control")
        .expect("the control write is allowed");
    std::net::TcpStream::connect(("127.0.0.1", port)).expect("the control dial is allowed");
    let deadline = Instant::now() + Duration::from_secs(10);
    while dialled(&mut accepted) == 0 {
        assert!(
            Instant::now() < deadline,
            "the canary's listener never saw its own control dial"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let controls = accepted;

    let [write, connect] = canary_commands(&file, port);
    // A third ask: the chat appends to its own transcript, which lives outside the project, and
    // must be refused: the transcript is the evidence, and the chat cannot be the one to write it.
    let transcript = scenario.next_transcript();
    let tamper = format!("echo '{TAMPER}' >> '{}'", transcript.display());
    let prompt = model.plays(&[
        ("canary-write", write),
        ("canary-connect", connect),
        ("canary-transcript", tamper),
    ]);
    let chat = scenario.starts_claude_code(model, &prompt);
    scenario.until_played(
        chat,
        &["canary-write", "canary-connect", "canary-transcript"],
    );
    let seen = Canary {
        wrote: file.exists(),
        dialled: dialled(&mut accepted) - controls,
        write: StepResult::of(scenario.result_of(chat, "canary-write").as_ref()),
        connect: StepResult::of(scenario.result_of(chat, "canary-connect").as_ref()),
    };
    if let Err(why) = super::checks::judge_canary(&seen) {
        scenario.fails_at(
            "canary",
            "one refused write and one refused connect, by Claude Code's own sandbox",
            &why,
        );
    }
    let tamper = StepResult::of(scenario.result_of(chat, "canary-transcript").as_ref());
    let left_in_it = std::fs::read_to_string(&transcript)
        .is_ok_and(|text| text.lines().any(|line| line == TAMPER));
    if left_in_it || (tamper.ran && tamper.exit == Some(0)) {
        scenario.fails_at(
            "canary",
            "the chat cannot write the transcript file outside the project",
            &format!("it wrote it ({left_in_it}): {tamper:?}"),
        );
    }
    scenario.keeps_listed_asks_blocked();
    scenario.forget_the_canary_chat(chat);
    scenario.the_canary_ran();
}

/// The line the canary's third ask tries to leave in the transcript file.
const TAMPER: &str = "written-by-the-chat-into-the-transcript";

/// The command a process runs, as `ps` says it.
fn command_of(pid: u32) -> String {
    purlis_core::forklock::output(std::process::Command::new("/bin/ps").args([
        "-o",
        "command=",
        "-p",
        &pid.to_string(),
    ]))
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::host::{Homes, Protection, Scenario};
    use super::super::person::Press;
    use super::*;

    /// **The leg is set up for a scenario's child** or the test ends, saying why: the real
    /// Claude Code, and its four variables.
    macro_rules! the_leg {
        ($test:expr) => {{
            let Some(program) = program($test) else {
                return;
            };
            let Some(env) = environment() else {
                return;
            };
            let pairs: Vec<(&str, &str)> = env
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            let Some(homes) = Homes::of_this_scenario_ahead(&pairs, Some(&program)) else {
                return;
            };
            homes
        }};
    }

    /// A request to the stand-in as Claude Code makes one: the chat's prompt, and each earlier
    /// call and its result.
    fn request(prompt: &str, earlier: &[(&str, &str, bool)]) -> Vec<u8> {
        let mut messages = vec![json!({
            "role": "user", "content": [{"type": "text", "text": prompt}]
        })];
        for (id, result, is_error) in earlier {
            messages.push(json!({
                "role": "assistant",
                "content": [{"type": "tool_use", "id": id, "name": "Bash", "input": {}}]
            }));
            messages.push(json!({
                "role": "user",
                "content": [{"type": "tool_result", "tool_use_id": id,
                             "content": result, "is_error": is_error}]
            }));
        }
        serde_json::to_vec(&json!({
            "model": "stand-in", "stream": false, "messages": messages,
            "tools": [{"name": "Bash", "input_schema": {"type": "object"}}]
        }))
        .expect("a request")
    }

    /// **The transcript is the stand-in model's request log** (T1.1-10, ticket 3 item 7): what
    /// the harness showed the model, a tool result a line with its step and its exit, read by
    /// the same code that reads the fake's transcript. Needs no Claude Code.
    #[test]
    fn transcript_expectations_read_the_stand_in_models_request_log() {
        let script = script_of(
            "log",
            &[
                ("first", "echo one".to_owned()),
                ("second", "curl https://refused.example.test/".to_owned()),
            ],
        );
        let prompt = script.prompt.clone();
        let stand_in = StandIn::new("claude", script);
        // The chat's first request is answered with the first step's call; its result brings
        // the second's.
        let first = stand_in.answer("POST", "/v1/messages", &request(&prompt, &[]));
        assert!(
            String::from_utf8_lossy(&first.body).contains("echo one"),
            "the first request is answered with the first step: {first:?}"
        );
        let second = stand_in.answer(
            "POST",
            "/v1/messages",
            &request(&prompt, &[("toolu_cs_0_0", "one", false)]),
        );
        assert!(
            String::from_utf8_lossy(&second.body).contains("refused.example.test"),
            "the first result brings the second step: {second:?}"
        );
        stand_in.answer(
            "POST",
            "/v1/messages",
            &request(
                &prompt,
                &[
                    ("toolu_cs_0_0", "one", false),
                    (
                        "toolu_cs_1_0",
                        "Exit code 56\ncurl: (56) CONNECT tunnel failed, response 403",
                        true,
                    ),
                ],
            ),
        );

        let transcript = transcript_of(&stand_in);
        let results: Vec<&Value> = transcript
            .iter()
            .filter(|line| line["kind"] == "tool_result")
            .collect();
        assert_eq!(
            results.len(),
            2,
            "one line to each tool result the model was shown: {transcript:?}"
        );
        assert_eq!(results[0]["step"], "first");
        assert_eq!(results[0]["exit"], 0);
        assert_eq!(results[1]["step"], "second");
        assert_eq!(results[1]["exit"], 56);

        // Read as the cells and the global checks read the fake's transcript.
        let read = StepResult::of(Some(results[1]));
        assert!(read.ran && read.exit == Some(56), "{read:?}");
        assert!(read.said.contains("403"), "{read:?}");
        // A call that did not run (a hook denied it) has no exit, so it never ran.
        let denied = StepResult::of(Some(&json!({
            "kind": "tool_result", "text": "blocked by a hook", "is_error": true
        })));
        assert!(!denied.ran, "{denied:?}");
        assert_eq!(exit_of("blocked by a hook", true), None);
        assert_eq!(exit_of("Exit code 7\nfailed", true), Some(7));
        assert_eq!(exit_of("fine", false), Some(0));
    }

    /// **A command Claude Code runs outside its sandbox fails the leg's canary** (T1.1-10): the
    /// canary's own two commands run by a shell with no sandbox at all, as Claude Code would run
    /// a command it did not sandbox, write outside the project and get through, and what the
    /// canary judges is red; the same two commands, refused, are green.
    #[test]
    fn an_unsandboxed_command_fails_the_legs_canary() {
        let outside = std::env::temp_dir().join(format!("leg-canary-{}", std::process::id()));
        std::fs::create_dir_all(&outside).expect("a folder");
        let file = outside.join("written-by-the-chat");
        // A port nothing listens on: the connect fails whatever stops it, so the write alone
        // is what the unsandboxed run shows.
        let [write, connect] = canary_commands(&file, 9);
        let ran = |command: &str| {
            let out = purlis_core::forklock::output(
                std::process::Command::new("/bin/sh").args(["-c", command]),
            )
            .expect("a shell");
            StepResult {
                ran: true,
                exit: out.status.code().map(i64::from),
                said: String::from_utf8_lossy(&out.stderr).to_lowercase(),
                hook: None,
            }
        };
        let unsandboxed = Canary {
            wrote: {
                let write = ran(&write);
                assert_eq!(write.exit, Some(0), "an unsandboxed write goes through");
                file.exists()
            },
            dialled: 0,
            write: StepResult {
                ran: true,
                exit: Some(0),
                said: String::new(),
                hook: None,
            },
            connect: ran(&connect),
        };
        let red = super::super::checks::judge_canary(&unsandboxed)
            .expect_err("a write that got through must fail the canary");
        assert!(red.contains("write outside the project"), "{red}");
        std::fs::remove_dir_all(&outside).expect("the folder goes");

        // And what the canary asks of a sandbox is green.
        let refused = Canary {
            wrote: false,
            dialled: 0,
            write: StepResult {
                ran: true,
                exit: Some(1),
                said: "sh: /x/y: operation not permitted".to_owned(),
                hook: None,
            },
            connect: StepResult {
                ran: true,
                exit: Some(7),
                said: "curl: (7) failed to connect".to_owned(),
                hook: None,
            },
        };
        assert!(super::super::checks::judge_canary(&refused).is_ok());
    }

    /// **The transcript of a chat survives the next chat's start** (T1.1-10, review F3): each
    /// chat has a stand-in and a file of its own, so a two-chat cell reads chat 1's result after
    /// chat 2 began. Needs no Claude Code.
    #[test]
    fn a_chats_transcript_is_still_readable_after_the_next_chat_starts() {
        let dir = std::env::temp_dir().join(format!("leg-two-chats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let model = Model::serve_on(None);
        let (first, second) = (dir.join("one.jsonl"), dir.join("two.jsonl"));

        let prompt = model.plays(&[("only", "echo one".to_owned())]);
        model.attach(&first);
        let stand_in = Arc::clone(&model.current.lock().expect("the stand-in"));
        stand_in.answer("POST", "/v1/messages", &request(&prompt, &[]));
        stand_in.answer(
            "POST",
            "/v1/messages",
            &request(&prompt, &[("toolu_cs_0_0", "one", false)]),
        );
        let read = |file: &Path| {
            std::fs::read_to_string(file)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .any(|line| line["kind"] == "tool_result" && line["step"] == "only")
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !read(&first) {
            assert!(
                Instant::now() < deadline,
                "chat 1's transcript was never written"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        // Chat 2 starts, the way the host starts it: played, then attached.
        model.plays(&[("other", "echo two".to_owned())]);
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            read(&first),
            "chat 1's result was wiped once chat 2 was played"
        );
        model.attach(&second);
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            read(&first),
            "chat 1's result was wiped once chat 2 was attached"
        );
        assert!(!read(&second), "chat 2 has no result of chat 1's step");
        std::fs::remove_dir_all(&dir).expect("the folder goes");
    }

    /// **A Claude Code scenario at Full whose canary never ran fails** (T1.1-10, review F1).
    #[test]
    fn a_claude_code_cell_that_never_ran_the_canary_fails() {
        let owed = super::super::host::canary_owed(true, false)
            .expect_err("a leg at Full that skipped its canary must fail");
        assert!(owed.contains("canary never ran"), "{owed}");
        assert!(super::super::host::canary_owed(true, true).is_ok());
        assert!(
            super::super::host::canary_owed(false, false).is_ok(),
            "a scenario that owes none is not failed"
        );
    }

    /// **A binary at another version than the pin does not run the leg** (T1.1-10).
    #[test]
    fn a_claude_code_at_another_version_than_the_pin_is_refused() {
        assert!(version_matches("2.1.325 (Claude Code)\n", "2.1.325 (Claude Code)\n").is_ok());
        let why = version_matches("2.1.400 (Claude Code)", "2.1.325 (Claude Code)")
            .expect_err("another version");
        assert!(why.contains("2.1.400") && why.contains("2.1.325"), "{why}");
        assert!(
            version_matches("", PINNED).is_err(),
            "no answer is no match"
        );
    }

    /// **Claude Code starts through the host's own chat start** (T1.1-10): the chat is composed
    /// as the window composes one and started in the session host, with the settings, plugin and
    /// proxy purlis writes, on the real binary and no fake. Claude Code is not wrapped by purlis
    /// here: it is the real program, below the host, with its own sandbox in its settings.
    #[test]
    fn the_claude_leg_starts_claude_code_through_the_hosts_own_chat_start() {
        let homes = the_leg!("the_claude_leg_starts_claude_code_through_the_hosts_own_chat_start");
        let model = Model::serve();
        let Some(scenario) = Scenario::for_claude_code(&homes, Protection::Full, &model) else {
            return;
        };
        let prompt = model.plays(&[("hello", "echo hello-from-the-leg".to_owned())]);
        let chat = scenario.starts_claude_code(&model, &prompt);
        scenario.until_played(chat, &["hello"]);

        let said = scenario.said();
        // The host's own chat start: its own Confine started the chat's confinement, and the
        // launch line is the window's composition of the harness's words.
        assert!(
            scenario
                .confines
                .started()
                .iter()
                .all(|one| one.planned != purlis_host::Planned::Elsewhere)
                && !scenario.confines.started().is_empty(),
            "the host's own Confine started the chat's confinement\n{said}"
        );
        // What the window composes (`start::ready`): the harness, its session and its mode. The
        // settings and the plugin are armed at the host's compose, so they are read off the
        // process below.
        for word in ["claude", "--session-id", "bypassPermissions"] {
            assert!(said.contains(word), "the launch line lacks {word}\n{said}");
        }
        assert!(
            !said.contains("fake-harness"),
            "the fake harness was started\n{said}"
        );
        let applied = scenario.applied_of(chat).expect("a sandbox was composed");
        assert_eq!(applied.level(), purlis_core::sandbox::Level::Full, "{said}");
        assert_eq!(
            applied.wrap_program(),
            None,
            "purlis does not wrap Claude Code on macOS: it runs its own sandbox\n{said}"
        );
        // The process is the real binary, and the stand-in model answered it.
        let commands: Vec<String> = scenario.pids().into_iter().map(command_of).collect();
        assert!(
            commands.iter().any(|command| command.contains("claude")
                && command.contains("--settings")
                && command.contains("--plugin-dir"))
                && !commands.iter().any(|c| c.contains("fake-harness")),
            "the chat's process is not the real Claude Code, armed with purlis's settings and \
             plugin: {commands:?}\n{said}"
        );
        assert!(
            model.requests() > 0,
            "the stand-in model heard nothing\n{said}"
        );
        let result = StepResult::of(scenario.result_of(chat, "hello").as_ref());
        assert!(
            result.ran && result.exit == Some(0) && result.said.contains("hello-from-the-leg"),
            "{result:?}\n{said}"
        );
    }

    /// **The smoke cell** (T1.1-10): a connection a command of Claude Code's makes is forwarded by
    /// its own sandbox to purlis's proxy, which holds it for the person; the person keeps it
    /// blocked, and the command is refused by the proxy's own word. The leg's canary runs first, inside
    /// [`Scenario::for_claude_code`].
    #[test]
    fn claude_codes_own_sandbox_forwards_a_held_connection_to_the_purlis_proxy() {
        let homes =
            the_leg!("claude_codes_own_sandbox_forwards_a_held_connection_to_the_purlis_proxy");
        let model = Model::serve();
        let Some(scenario) = Scenario::for_claude_code(&homes, Protection::Full, &model) else {
            return;
        };

        let host = "held-by-claude.example.test";
        let prompt = model.plays(&[("held-connection", super::super::reaches(host))]);
        let chat = scenario.starts_claude_code(&model, &prompt);
        // The proxy holds the connection and the window lists its ask: the connection reached
        // purlis's proxy through Claude Code's own sandbox.
        let (_, kept) = scenario.person().when_listed(
            chat,
            host,
            &Press::Keep(None),
            Duration::from_millis(50),
        );
        kept.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
        scenario.until_played(chat, &["held-connection"]);
        let result = StepResult::of(scenario.result_of(chat, "held-connection").as_ref());
        assert!(
            result.ran && result.exit.is_some_and(|code| code != 0 && code != 127),
            "the held connection was not refused once the person kept it blocked: {result:?}\n{}",
            scenario.said()
        );
        scenario.keeps_listed_asks_blocked();
        let _ = super::super::report::write(&[super::super::a_record_of(
            "T1.1-10",
            "claude_codes_own_sandbox_forwards_a_held_connection_to_the_purlis_proxy",
            Some(purlis_scenarios::model::Dialect::Claude),
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::Pass,
            "the real Claude Code's own sandbox forwarded a held connection to purlis's proxy",
        )]);
    }
}
