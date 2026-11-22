// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The real-harness legs** (T1.1-10, #6857; T1.1-53, #6900): the scenario host with a real
//! harness binary in place of the fake one, started as production starts it, against a stand-in
//! model, with a canary that fails the leg if the binary runs outside its wrap.
//!
//! Who holds the real binary differs by harness and OS, and the leg says which in every report:
//!
//! - **Claude Code on macOS** is not wrapped by purlis. purlis hands the chat the `sandbox`
//!   object and the `permissions.deny` rules of `sandbox::claude` in `--settings`, and Claude Code
//!   builds its own seatbelt from them, its Bash tool's network going through purlis's proxy
//!   (T1.1-10).
//! - **Claude Code on Linux, and Codex and OpenCode on both OSes,** are wrapped whole by purlis
//!   (`claude_wrap.rs`, `<kit> _sandbox`, Seatbelt): Claude Code's own sandbox is off there
//!   (T1.1-53).
//!
//! - **Started as production starts it**: [`Scenario::starts_real`] composes the chat the window's
//!   way (`purlis_core::start::ready`, then the host's chat start), so the launch line, the
//!   environment, the bundled plugin and the settings are what purlis writes. Never a hand launch.
//! - **A stand-in model** ([`Model`]) answers its model calls in the harness's own API (T1.1-03's
//!   stand-in: Messages for Claude Code, Responses for Codex, Chat Completions for OpenCode, on a
//!   loopback port), asking for each step's Bash call in turn. No model is called and no credential
//!   is used. What each harness is given to reach it is named in the test plane's
//!   `charter.local.toml` `[chat_env] pass` ([`added`], [`chat_env_pass`]) with its value in the
//!   scenario host's environment, or, for Codex, in its own `-c` words ([`launch_words`]): a
//!   substitution of ticket 6's list (`switches.json`), covered by ticket 44's live pass.
//! - **The transcript is the stand-in's request log**: what the real harness showed the model, in
//!   T1.1-03's projection, one line to a tool result in the shape the fake's own transcript has
//!   ([`transcript_of`]), so the cells' expectations and the global checks read the same. Each
//!   harness says an exit status in its own words ([`exit_of`]); OpenCode says none, so the leg's
//!   commands to it end by saying their own ([`EXIT_MARKER`]).
//! - **A canary** ([`canary`]) fails the leg if the harness runs a command outside its wrap: one
//!   write outside the project and one direct connect, each seen refused in the tool's own result
//!   and from outside (the file a write left, a listener's connections), judged as the fake's
//!   canary is ([`super::checks::judge_canary`]).
//! - **The cells** ([`cells`]): the probe (the real binary, not the fake, answered by the stand-in
//!   and played), one file cell (a plain file reads, the project's vault is refused, the project
//!   is written) and one network cell (at Full a held connection raises an ask the person keeps
//!   blocked; at the floor, where no host is asked about, a link-local address is refused by
//!   purlis's proxy), at Full and at Basic. The harness's own start-up traffic (an update check,
//!   a model list, a plugin install) is silenced in its own words where it has them, and any ask
//!   the proxy still raises for it is kept blocked at once ([`Scenario::answers_until_played`]).
//!
//! - **What these legs do not exercise**: Claude Code starts with `--permission-mode
//!   bypassPermissions` and the prompt after the window's own words, and its profile's first-run
//!   state accepts the bypass warning, each listed in `switches.json`; so its `ask` rules and its
//!   permission prompts are never reached here: only its sandbox (or purlis's wrap) and purlis's
//!   `permissions.deny` rules are what a cell can learn about.
//! - **An escape cell needs two witnesses**: the transcript is what the model was shown, which
//!   the harness's own words shape, so a cell that says a command got out pairs it with evidence
//!   from outside (a file left, a listener's connections), as the canary does.
//!
//! Each harness runs with the binary its variable names ([`host::program_var`]), at the version
//! `tests/fixtures/harness-versions/<harness>.txt` pins, checked at run time ([`program`]). On CI
//! a leg that cannot run fails.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_recordings::script::{Response, Script, Step};
use purlis_recordings::standin::{self, StandIn};
use serde_json::{Value, json};

use super::checks::{Canary, StepResult};
use super::host::{self, Dialect, Homes, Protection, Scenario};
use super::person::Press;

/// What a harness is given as its API key: a literal, not a credential. Only the stand-in reads
/// it, and it accepts anything.
pub(crate) const NOT_A_CREDENTIAL: &str = "stand-in-model-no-credential";

/// The variable that points Claude Code at the stand-in model.
pub(crate) const BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// The variable that tells the scenario host's own process which loopback port the stand-in
/// serves on, for Codex and OpenCode, whose chats are told it in their own words ([`launch_words`],
/// [`opencode_config`]). It is in the host's environment and in no chat's: no `[chat_env] pass`
/// names it.
pub(crate) const STAND_IN_URL: &str = "PURLIS_STAND_IN_URL";

/// What a leg's commands to OpenCode end by saying: the exit status of the command, which
/// OpenCode's tool result does not say. `exit_of` reads it back.
pub(crate) const EXIT_MARKER: &str = "purlis-leg-exit=";

/// The model Codex is told it runs: one its own catalogue lists with a freeform `apply_patch`
/// (`tests/recordings/tests/record.rs`, which the recordings are made with). Only the name:
/// every request goes to the stand-in.
const CODEX_MODEL: &str = "gpt-5.5";

/// A command a chat of the floor runs, to a place no chat is let reach: a link-local address,
/// which purlis's proxy refuses with its own words. Through the proxy the chat's environment
/// names, so a chat that is not held by one reaches the address itself and fails the cell.
pub(crate) const REFUSED_CLASS: &str =
    "curl -sS --max-time 20 http://169.254.169.254/latest/meta-data/";

/// The words purlis's proxy answers a refused destination with.
pub(crate) const PROXY_REFUSAL: &str = "purlis's sandbox does not allow";

/// The variable that points `dialect`'s harness at the stand-in's address.
pub(crate) fn url_var(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Claude => BASE_URL,
        Dialect::Codex | Dialect::OpenCode => STAND_IN_URL,
    }
}

/// **What the leg adds to a chat's keep-list**, each named in the plane's `[chat_env] pass`.
///
/// - Claude Code: the stand-in's address and key, and the two variables that keep its own
///   start-up traffic (an update check, telemetry) from going out, which its unsandboxed main
///   process on macOS would do without purlis's proxy.
/// - Codex: its API key, a literal that is no credential. Where its model is, and which API, is
///   in its own `-c` words ([`launch_words`]).
/// - OpenCode: a file holding its whole configuration, with the stand-in as its one provider
///   ([`opencode_config`], named by `OPENCODE_CONFIG`), and the variable that stops its
///   self-update.
///
/// Each is in ticket 6's list of substitutions (`switches.json`), covered by ticket 44's live
/// pass. A person's chat is given none of them.
pub(crate) fn added(dialect: Dialect) -> &'static [&'static str] {
    match dialect {
        Dialect::Claude => &[
            BASE_URL,
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "DISABLE_AUTOUPDATER",
        ],
        Dialect::Codex => &["OPENAI_API_KEY"],
        Dialect::OpenCode => &[
            "OPENCODE_CONFIG",
            "OPENCODE_DISABLE_AUTOUPDATE",
            "OPENCODE_DISABLE_MODELS_FETCH",
            "OPENCODE_DISABLE_DEFAULT_PLUGINS",
            "OPENCODE_DISABLE_LSP_DOWNLOAD",
        ],
    }
}

/// The project's local `[chat_env]` naming what [`added`] says, as an operator writes it.
pub(crate) fn chat_env_pass(dialect: Dialect) -> String {
    let names: Vec<String> = added(dialect)
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect();
    format!("[chat_env]\npass = [{}]\n", names.join(", "))
}

/// **OpenCode's configuration**: the stand-in as its one provider, over Chat Completions, for the
/// model and the small model alike, with no update and no sharing. What
/// `tests/recordings/tests/record.rs` writes to a file for the recordings, and the file
/// `OPENCODE_CONFIG` names ([`opencode_config_path`]): not `OPENCODE_CONFIG_CONTENT`, which
/// purlis sets itself, for the plugin that carries its hooks.
pub(crate) fn opencode_config(url: &str) -> String {
    json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {"standin": {
            "npm": "@ai-sdk/openai-compatible",
            "name": "stand-in",
            "options": {"baseURL": format!("{url}/v1"), "apiKey": NOT_A_CREDENTIAL},
            "models": {"stand-in": {"name": "stand-in", "tool_call": true}}
        }},
        "model": "standin/stand-in",
        "small_model": "standin/stand-in",
        "disabled_providers": ["opencode"],
        "autoupdate": false,
        "share": "disabled"
    })
    .to_string()
}

/// Where OpenCode's configuration for the stand-in at `url` is kept: a file in a folder of this
/// run's own, in the temp folder, named for the stand-in's port, outside every project. The
/// scenario's parent creates the folder (never one that is already there) and writes the file
/// ([`environment`]), and removes both ([`runs`]); the child, which has the same port, finds
/// the file where the parent put it.
pub(crate) fn opencode_config_path(url: &str) -> PathBuf {
    opencode_config_dir(url).join("opencode.json")
}

/// The folder of [`opencode_config_path`].
fn opencode_config_dir(url: &str) -> PathBuf {
    let port = url
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    std::env::temp_dir().join(format!("purlis-leg-opencode-{port}"))
}

/// **The words a harness is started with beside what purlis composes**, ending with its first
/// message as the harness takes one (`purlis_core::handoff::first_message_argv`): Claude Code's
/// permission mode (the recordings' conditions: a mode that sends no call to Anthropic's servers
/// to judge, `tests/recordings/README.md`), and Codex's model provider, which is the stand-in.
pub(crate) fn launch_words(dialect: Dialect, url: &str, prompt: &str) -> Vec<String> {
    let mut words: Vec<String> = match dialect {
        Dialect::Claude => ["--permission-mode", purlis_recordings::PERMISSION_MODE]
            .map(str::to_owned)
            .to_vec(),
        Dialect::Codex => [
            "model_provider=\"standin\"".to_owned(),
            format!("model=\"{CODEX_MODEL}\""),
            "model_providers.standin.name=\"stand-in\"".to_owned(),
            format!("model_providers.standin.base_url=\"{url}/v1\""),
            "model_providers.standin.wire_api=\"responses\"".to_owned(),
            "model_providers.standin.request_max_retries=0".to_owned(),
            "model_providers.standin.stream_max_retries=0".to_owned(),
        ]
        .into_iter()
        .flat_map(|setting| ["-c".to_owned(), setting])
        .collect(),
        Dialect::OpenCode => Vec::new(),
    };
    words.extend(
        purlis_core::handoff::first_message_argv(dialect.word(), prompt)
            .expect("the harness takes a first message"),
    );
    words
}

/// Whether `dialect`'s harness holds itself on this OS, which purlis does not wrap: Claude Code on
/// macOS, under its own sandbox (T1.1-10).
pub(crate) fn holds_itself(dialect: Dialect) -> bool {
    dialect == Dialect::Claude && cfg!(target_os = "macos")
}

/// The version `tests/fixtures/harness-versions/<harness>.txt` pins, as the harness's own
/// `--version` prints it.
fn pinned(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Claude => include_str!("../../../../tests/fixtures/harness-versions/claude.txt"),
        Dialect::Codex => include_str!("../../../../tests/fixtures/harness-versions/codex.txt"),
        Dialect::OpenCode => {
            include_str!("../../../../tests/fixtures/harness-versions/opencode.txt")
        }
    }
}

/// What `program --version` prints.
fn version_of(program: &Path) -> String {
    purlis_core::forklock::output(std::process::Command::new(program).arg("--version"))
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// **The binary's version is the one pinned**: what `<harness> --version` said, against the pin's
/// own line, so a leg never runs on a version nobody recorded.
fn version_matches(dialect: Dialect, said: &str, pinned: &str) -> Result<(), String> {
    if said.trim() == pinned.trim() {
        Ok(())
    } else {
        Err(format!(
            "{} is `{}`, and `tests/fixtures/harness-versions/{}.txt` pins `{}`",
            host::program_var(dialect),
            said.trim(),
            dialect.word(),
            pinned.trim()
        ))
    }
}

/// **The real harness this run starts**, or none, saying why: the file its variable
/// ([`host::program_var`]) names, at the version its pin says ([`version_matches`]). On macOS or
/// Linux without the program the leg cannot run, which on CI fails ([`host::cannot_run`]).
pub(crate) fn program(dialect: Dialect) -> Option<PathBuf> {
    let var = host::program_var(dialect);
    match std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        Some(program) if program.is_file() => {
            match version_matches(dialect, &version_of(&program), pinned(dialect)) {
                Ok(()) => Some(program),
                Err(why) => host::cannot_run(&why),
            }
        }
        Some(program) => host::cannot_run(&format!(
            "{var} names {}, which is not there",
            program.display()
        )),
        None => host::cannot_run(&format!(
            "{var} names no `{}`: install the version \
             `tests/fixtures/harness-versions/{}.txt` pins and name it",
            dialect.word(),
            dialect.word()
        )),
    }
}

/// **Claude Code's own sandbox is the leg on macOS alone** (T1.1-10): elsewhere purlis wraps
/// Claude Code (T1.1-53's leg), and this one is reported not run for `test`.
pub(crate) fn macos_claude(test: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        let reason = "Claude Code runs its own sandbox on macOS alone; on Linux purlis wraps it \
                      (T1.1-53's leg)";
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
    program(Dialect::Claude)
}

/// **The variables of [`added`], with their values**, for the scenario's child, and for Codex and
/// OpenCode the stand-in's address ([`STAND_IN_URL`]): a loopback port this process picks and the
/// child, which reads it back from its environment, binds ([`Model::serve`]). `None` where no
/// port can be bound here.
pub(crate) fn environment(dialect: Dialect) -> Option<Vec<(String, String)>> {
    // Always this process's own pick: an address in the ambient environment is never honoured, so
    // a chat is never pointed at a server nobody here started. The port is freed for the child to
    // bind: the child is another process, so the listener cannot be held.
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => listener,
        Err(why) => return host::cannot_run(&format!("no loopback port to bind: {why}")),
    };
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("its address").port()
    );
    drop(listener);
    // OpenCode's configuration is a file the parent writes: the child's own call is unused, and
    // writes none.
    if dialect == Dialect::OpenCode && !host::in_the_scenarios_child() {
        let file = opencode_config_path(&url);
        // `create_dir`, not `create_dir_all`: a folder that is already there is not this run's.
        let written = std::fs::create_dir(opencode_config_dir(&url))
            .and_then(|()| std::fs::write(&file, opencode_config(&url)));
        if let Err(why) = written {
            return host::cannot_run(&format!(
                "OpenCode's configuration at {}: {why}",
                file.display()
            ));
        }
    }
    Some(environment_at(dialect, &url))
}

/// [`environment`] for the stand-in at `url`.
pub(crate) fn environment_at(dialect: Dialect, url: &str) -> Vec<(String, String)> {
    let pairs: Vec<(&str, String)> = match dialect {
        Dialect::Claude => vec![
            (BASE_URL, url.to_owned()),
            ("ANTHROPIC_API_KEY", NOT_A_CREDENTIAL.to_owned()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_owned()),
            ("DISABLE_AUTOUPDATER", "1".to_owned()),
        ],
        Dialect::Codex => vec![
            (STAND_IN_URL, url.to_owned()),
            ("OPENAI_API_KEY", NOT_A_CREDENTIAL.to_owned()),
        ],
        Dialect::OpenCode => vec![
            (
                "OPENCODE_CONFIG",
                opencode_config_path(url).display().to_string(),
            ),
            (STAND_IN_URL, url.to_owned()),
            ("OPENCODE_DISABLE_AUTOUPDATE", "1".to_owned()),
            ("OPENCODE_DISABLE_MODELS_FETCH", "1".to_owned()),
            ("OPENCODE_DISABLE_DEFAULT_PLUGINS", "1".to_owned()),
            ("OPENCODE_DISABLE_LSP_DOWNLOAD", "1".to_owned()),
        ],
    };
    pairs
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
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

/// **The stand-in model of a leg**: one server on the loopback port the harness's address names,
/// across the chats of a scenario, each with a stand-in and a script of its own
/// ([`Model::plays`]); each chat's request log is written to the file of its own
/// ([`Model::attach`]), which a later chat never touches.
pub(crate) struct Model {
    dialect: Dialect,
    port: u16,
    /// The stand-in that answers now: the newest chat's.
    current: Arc<Mutex<Arc<StandIn>>>,
    /// Every chat's stand-in and transcript file, in the order the chats were played.
    chats: Arc<Mutex<Vec<Chat>>>,
    stop: Arc<AtomicBool>,
}

impl Model {
    /// Serves on the port this child's environment names ([`environment`]).
    pub(crate) fn serve(dialect: Dialect) -> Model {
        let var = url_var(dialect);
        let url = std::env::var(var).expect("the scenario's child names the stand-in's port");
        let port: u16 = url
            .rsplit(':')
            .next()
            .and_then(|port| port.trim_end_matches('/').parse().ok())
            .unwrap_or_else(|| panic!("{var} is {url}: no loopback port"));
        let listener = TcpListener::bind(("127.0.0.1", port))
            .unwrap_or_else(|why| panic!("the stand-in model cannot listen on {port}: {why}"));
        Model::serve_on(dialect, port, Some(listener))
    }

    /// Serves on `listener`, or on none (a test that plays the stand-in itself).
    fn serve_on(dialect: Dialect, port: u16, listener: Option<TcpListener>) -> Model {
        let current = Arc::new(Mutex::new(Arc::new(StandIn::new(
            dialect.word(),
            script_of(dialect, "idle", &[]),
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
                        let text = transcript_of(dialect, &chat.stand_in)
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
            dialect,
            port,
            current,
            chats,
            stop,
        }
    }

    /// The stand-in's loopback port, which the chat's sandbox is granted where it walls the
    /// network.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// The stand-in's address, as the harness is told it.
    pub(crate) fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// **The next chat's script**: each of `steps` (a name and a Bash command) asked for in turn,
    /// once the one before has its result. Answers the prompt the chat starts with, which is how
    /// the stand-in tells the chat's requests from the harness's own. The chat has a stand-in of
    /// its own, kept for its transcript.
    pub(crate) fn plays(&self, steps: &[(&str, String)]) -> String {
        let mut chats = self.chats.lock().expect("the chats");
        let script = script_of(
            self.dialect,
            &format!("leg chat {}", chats.len() + 1),
            steps,
        );
        let prompt = script.prompt.clone();
        let stand_in = Arc::new(StandIn::new(self.dialect.word(), script));
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

    /// What the current chat's stand-in has heard, for a red run's reader: each request's path,
    /// whom it was taken for and what the model was shown of it.
    pub(crate) fn heard_said(&self) -> String {
        let stand_in = Arc::clone(&self.current.lock().expect("the stand-in"));
        let heard = stand_in.heard.lock().expect("the log");
        let mut said = format!("the stand-in model heard {} requests:", heard.len());
        for one in heard.iter() {
            let shown: Vec<String> = one
                .shown
                .iter()
                .map(|item| {
                    format!(
                        "{}: {}",
                        item.kind,
                        item.text.chars().take(120).collect::<String>()
                    )
                })
                .collect();
            if let Some(raw) = &one.raw {
                said.push_str(&format!("\n  its request, bar what it said: {raw}"));
            }
            said.push_str(&format!(
                "\n  {} as {:?}, response {:?}, offered {:?}, asked {:?}, shown {:?}",
                one.path,
                one.conversation,
                one.response,
                one.offered,
                one.asked.iter().map(|call| &call.step).collect::<Vec<_>>(),
                shown
            ));
        }
        said.chars().take(8000).collect()
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

/// The command as `dialect`'s harness is asked to run it: OpenCode's tool result says no exit
/// status, so its commands end by saying their own ([`EXIT_MARKER`]); the others' results say it.
pub(crate) fn command_for(dialect: Dialect, command: &str) -> String {
    match dialect {
        Dialect::OpenCode => format!("{{ {command}\n}}; echo \"{EXIT_MARKER}$?\""),
        Dialect::Claude | Dialect::Codex => command.to_owned(),
    }
}

/// A script that asks for each of `steps` as a Bash call, in turn.
fn script_of(dialect: Dialect, name: &str, steps: &[(&str, String)]) -> Script {
    Script {
        name: name.to_owned(),
        prompt: format!("Run the steps of {name}."),
        responses: steps
            .iter()
            .map(|(step, command)| Response {
                calls: vec![Step {
                    step: (*step).to_owned(),
                    kind: "bash".to_owned(),
                    command: Some(command_for(dialect, command)),
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
/// shape the fake's transcript has ([`line_of`]): the `step` its call was asked for as, and its
/// `exit` where the result says one ([`exit_of`]), so [`StepResult::of`] and the cells read it
/// alike. A result with no exit, such as a call a hook denied, never ran.
pub(crate) fn transcript_of(dialect: Dialect, stand_in: &StandIn) -> Vec<Value> {
    let sequence = stand_in.script.sequence();
    stand_in
        .shown()
        .into_iter()
        .map(|shown| line_of(dialect, &sequence, &shown))
        .collect()
}

/// One thing the model was shown, as a line of the transcript.
pub(crate) fn line_of(
    dialect: Dialect,
    sequence: &[(u32, &Response)],
    shown: &purlis_recordings::projection::Shown,
) -> Value {
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
        if let Some(step) = step_of(sequence, id) {
            line["step"] = json!(step);
        }
    }
    if let Some(exit) = exit_of(dialect, &shown.text, failed) {
        line["exit"] = json!(exit);
    }
    line
}

/// The step the stand-in asked for as tool-use `id` (`toolu_cs_<response>_<call>` in Messages,
/// `call_cs_<response>_<call>` in the other two).
fn step_of(sequence: &[(u32, &Response)], id: &str) -> Option<String> {
    let mut parts = id
        .strip_prefix("toolu_cs_")
        .or_else(|| id.strip_prefix("call_cs_"))?
        .split('_');
    let response: usize = parts.next()?.parse().ok()?;
    let call: usize = parts.next()?.parse().ok()?;
    Some(sequence.get(response)?.1.calls.get(call)?.step.clone())
}

/// The number that follows `prefix` in `text`, the first time or the last.
fn number_after(text: &str, prefix: &str, last: bool) -> Option<i64> {
    let at = if last {
        text.rfind(prefix)?
    } else {
        text.find(prefix)?
    } + prefix.len();
    let digits: String = text[at..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// **The exit status a Bash tool result says**, in the harness's own words:
///
/// - Claude Code: `Exit code N` as its first words on a failure, and nothing on a success (0 for a
///   result that is no error; none for an error that names no exit, a call that did not run).
/// - Codex: `Process exited with code N` in the result's header (and `Exit code: N` for its
///   `apply_patch`); a call a hook denied, or one still running, says neither.
/// - OpenCode: none of its own: [`EXIT_MARKER`], which the leg's command ends by saying.
pub(crate) fn exit_of(dialect: Dialect, text: &str, failed: bool) -> Option<i64> {
    match dialect {
        Dialect::Claude => match text.strip_prefix("Exit code ") {
            Some(rest) => rest
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok(),
            None => (!failed).then_some(0),
        },
        Dialect::Codex => number_after(text, "Process exited with code ", false)
            .or_else(|| number_after(text, "Exit code: ", false)),
        Dialect::OpenCode => number_after(text, EXIT_MARKER, true),
    }
}

/// **The canary's two commands**, as the fake's canary runs them: a write outside the project to
/// `file` and a connect that does not go through the proxy, to `port` on this machine.
pub(crate) fn canary_commands(file: &Path, port: u16) -> [String; 2] {
    let script = super::checks::canary_script(file, std::net::Ipv4Addr::LOCALHOST.into(), port);
    let call = |at: usize| {
        script["turns"][0]["calls"][at]["run"]
            .as_str()
            .expect("the canary's command")
            .to_owned()
    };
    [call(0), call(1)]
}

/// Who holds the harness, as a failure names it.
fn holder(dialect: Dialect) -> &'static str {
    if holds_itself(dialect) {
        "Claude Code's own sandbox"
    } else {
        "purlis's wrap"
    }
}

/// **The leg's canary**: before anything else, the real harness runs one write outside the
/// project and one direct connect, and both must be refused by its holder, seen in each tool
/// result and from outside (the file a write left, a listener's connections). A canary that sees
/// either get through fails the leg ([`super::checks::judge_canary`]): the harness ran a command
/// outside its wrap, and nothing the leg says after that is evidence. The person then keeps
/// whatever the refusals asked blocked, and the chat is closed.
pub(crate) fn canary(scenario: &Scenario, model: &Model) {
    let dialect = scenario.dialect;
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
    // The positive controls, from this process, outside the harness's wrap: it can write there
    // and dial there, so a refusal below is the wrap's.
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
    let chat = scenario.starts_real(model, &prompt);
    let heard = || model.requests() > 0;
    let evidence = || model.heard_said();
    scenario.answers_until_played(
        chat,
        &["canary-write", "canary-connect", "canary-transcript"],
        &host::Driving {
            heard: &heard,
            held: None,
            kept: &std::cell::Cell::new(false),
            evidence: &evidence,
        },
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
            &format!(
                "one refused write and one refused connect, by {}",
                holder(dialect)
            ),
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

/// How many short polls a held connection's result is waited for: each is at most seven seconds
/// (a harness may give a command's result back after ten), so the proxy's hold of thirty seconds
/// and its group window are covered with room.
const POLLS: usize = 24;

/// **A command the proxy holds, run from a script**: the script `script` (a file the host wrote,
/// so no quote of it matters), its result written to `file` as its own words and its exit
/// (`net-exit=N`), with no stdin. A harness that gives a long command's result back early (Codex's
/// tool does after ten seconds, "Process running") leaves it running in its session, which
/// finishes it and the file; one that waits (Claude Code's, OpenCode's) returns when it ends. The
/// result is read by [`poll_for`] either way. It is not run in the background: Codex ends every
/// process its call left behind, in a session of its own too (CI run 11). No `rm` in it:
/// purlis's guard asks the person about a command that deletes, which Codex then waits on.
fn held_command(script: &Path, file: &Path) -> String {
    format!(
        "sh '{script}' > '{file}' 2>&1 < /dev/null; echo ended",
        script = script.display(),
        file = file.display(),
    )
}

/// The script [`held_command`] runs: `command`, and its exit said last.
fn script_of_the_held(command: &str) -> String {
    format!("{command}\necho \"net-exit=$?\"\n")
}

/// **One poll for what [`held_command`] wrote**: up to seven seconds, then whatever the
/// file holds.
fn poll_for(file: &Path) -> String {
    format!(
        "for i in 1 2 3 4 5 6 7; do grep -q net-exit '{file}' 2>/dev/null && break; sleep 1; done; \
         cat '{file}' 2>/dev/null; ls -l '{file}' 2>&1; echo polled",
        file = file.display(),
    )
}

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

fn model_dialect(dialect: Dialect) -> purlis_scenarios::model::Dialect {
    match dialect {
        Dialect::Claude => purlis_scenarios::model::Dialect::Claude,
        Dialect::Codex => purlis_scenarios::model::Dialect::Codex,
        Dialect::OpenCode => purlis_scenarios::model::Dialect::Opencode,
    }
}

fn model_protection(protection: Protection) -> purlis_scenarios::model::Protection {
    match protection {
        Protection::Off => purlis_scenarios::model::Protection::None,
        Protection::Basic => purlis_scenarios::model::Protection::Basic,
        Protection::Full => purlis_scenarios::model::Protection::Full,
    }
}

/// Writes the leg's record for `test`, in ticket 5's schema.
fn record(
    test: &str,
    dialect: Dialect,
    protection: Protection,
    verdict: purlis_scenarios::model::ReportVerdict,
    reason: &str,
) {
    let _ = super::report::write(&[super::a_record_of(
        "T1.1-53",
        test,
        Some(model_dialect(dialect)),
        Some(model_protection(protection)),
        verdict,
        reason,
    )]);
}

/// **A real harness under purlis's wrap, in the host** (T1.1-53, #6900): `dialect`'s binary
/// started through the host's own chat start at `protection`, against the stand-in, with the
/// canary first where the sandbox is full, and then the leg's cells ([`cells`]). The test ends,
/// saying why, where the leg cannot be set up here.
pub(crate) fn runs(dialect: Dialect, protection: Protection, test: &str) {
    let Some(program) = program(dialect) else {
        return;
    };
    let Some(env) = environment(dialect) else {
        return;
    };
    let pairs: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let Some(homes) = Homes::of_this_scenario_ahead(&pairs, Some(&program)) else {
        // The parent, once its child has run the scenario: OpenCode's configuration is its own.
        if dialect == Dialect::OpenCode
            && let Some((_, url)) = env.iter().find(|(name, _)| name == STAND_IN_URL)
        {
            let _ = std::fs::remove_dir_all(opencode_config_dir(url));
        }
        return;
    };
    let model = Model::serve(dialect);
    let Some(scenario) = Scenario::for_real_harness(&homes, protection, dialect, &model) else {
        return;
    };
    cells(&scenario, &model, protection, test);
}

/// **The leg's cells**, in one chat: the probe, one file cell and one network cell.
///
/// - **The probe**: the chat runs the real binary, never the fake, under the sandbox its
///   protection names, the stand-in model answers it, and its first command runs.
/// - **The file cell**: a plain file of the project reads (the control) and the project is
///   written; the project's vault, which no chat reads, is refused.
/// - **The network cell**: at Full a connection to a host the project does not list is held and
///   the window lists its ask, the person keeps it blocked, and the command is refused by the
///   proxy; at the floor, where no host is asked about, a link-local address is refused by
///   purlis's proxy in its own words.
fn cells(scenario: &Scenario, model: &Model, protection: Protection, test: &str) {
    use purlis_scenarios::model::ReportVerdict;
    let dialect = scenario.dialect;
    let vaults = scenario.root.join(".purlis").join("vaults");
    std::fs::create_dir_all(&vaults).expect("the vaults");
    std::fs::write(
        vaults.join("covering.json"),
        "{\"covering\": \"VAULT-HELD\"}\n",
    )
    .expect("a vault");
    let plain = scenario.root.join("plain.txt");
    std::fs::write(&plain, "PLAIN-READS\n").expect("a plain file");
    let made = scenario.root.join("made-by-the-chat");
    let host = "held-by-the-real-harness.example.test";

    let mut steps: Vec<(&str, String)> = vec![
        ("hello", "echo hello-from-the-leg".to_owned()),
        ("reads-plain", format!("cat {}", plain.display())),
        (
            "reads-the-vault",
            format!(
                "d='{}/.pur'; d=\"${{d}}lis\"; cat \"$d/vaults/covering.json\" 2>&1; echo vault-exit=$?",
                scenario.root.display()
            ),
        ),
        (
            "writes-inside",
            format!("echo wrote-by-the-chat > {0} && cat {0}", made.display()),
        ),
    ];
    let network = scenario.root.join("network-result");
    let polls: Vec<String> = (1..=POLLS)
        .map(|n| format!("network-poll-{n:02}"))
        .collect();
    match protection {
        Protection::Full => {
            // The connection is held for the person for the proxy's hold (30 s here), and a
            // harness may give a command's result back before it ends (Codex's tool does after
            // ten seconds, "Process running"): the command runs in the foreground of its call,
            // writing a file, and the result is read by short polls of that file.
            let script = scenario.root.join("network.sh");
            std::fs::write(&script, script_of_the_held(&super::reaches(host)))
                .expect("the held connection's script");
            steps.push(("network-start", held_command(&script, &network)));
            for name in &polls {
                steps.push((name.as_str(), poll_for(&network)));
            }
        }
        _ => steps.push(("network", REFUSED_CLASS.to_owned())),
    }
    let prompt = model.plays(&steps);
    let chat = scenario.starts_real(model, &prompt);
    let heard = || model.requests() > 0;
    let evidence = || model.heard_said();
    let kept = std::cell::Cell::new(false);
    let driving = host::Driving {
        heard: &heard,
        held: (protection == Protection::Full).then_some(host),
        kept: &kept,
        evidence: &evidence,
    };
    scenario.answers_until_played(
        chat,
        &["hello", "reads-plain", "reads-the-vault", "writes-inside"],
        &driving,
    );
    probe(scenario, model, chat, protection);

    // The file cell.
    let said = scenario.said();
    let read = StepResult::of(scenario.result_of(chat, "reads-plain").as_ref());
    assert!(
        read.said.contains("plain-reads"),
        "the control: a plain file of the project does not read: {read:?}\n{said}"
    );
    let wrote = StepResult::of(scenario.result_of(chat, "writes-inside").as_ref());
    assert!(
        wrote.said.contains("wrote-by-the-chat") && made.is_file(),
        "the control: the chat could not write its project: {wrote:?}\n{said}"
    );
    let vault_said = scenario
        .result_of(chat, "reads-the-vault")
        .map(|line| line.to_string())
        .unwrap_or_default();
    assert!(
        !vault_said.contains("VAULT-HELD"),
        "the real {} read the project's vault, so it ran outside its wrap: {vault_said}\n{said}",
        dialect.word()
    );
    let refused = StepResult::of(scenario.result_of(chat, "reads-the-vault").as_ref());
    assert!(
        ["operation not permitted", "permission denied"]
            .iter()
            .any(|words| refused.said.contains(words))
            && !refused.said.contains("vault-exit=0"),
        "the vault read was not refused by the wrap (no guard reads its path: it is put together \
         in the shell): {vault_said}\n{said}"
    );

    // The network cell.
    let how = if protection == Protection::Full {
        let last = polls.last().expect("polls").as_str();
        scenario.answers_until_played(chat, &[last], &driving);
        // The command's own word: curl's exit, said into the file the polls read.
        let started = scenario
            .result_of(chat, "network-start")
            .map(|line| line["text"].to_string());
        let said_by_polls: Vec<String> = polls
            .iter()
            .filter_map(|name| scenario.result_of(chat, name))
            .map(|line| line["text"].as_str().unwrap_or_default().to_owned())
            .collect();
        let exit = said_by_polls
            .iter()
            .find_map(|text| number_after(text, "net-exit=", false));
        let said = scenario.said();
        assert!(
            exit.is_some_and(|code| code != 0 && code != 127),
            "the connection to a host the project does not list was not refused at Full \
             (curl's exit: {exit:?}); it was started with {started:?}; the first polls: \
             {:?}\n{said}",
            &said_by_polls[..said_by_polls.len().min(2)]
        );
        // And the refusal is recorded against the host: a block of it in the log, which a
        // connection that never reached purlis's proxy would not leave.
        let events = scenario.try_events().unwrap_or_default();
        assert!(
            events.iter().any(|event| {
                event.kind == "stop.block"
                    && event.body["target"]["host"] == host
                    && event.body["source"] == "sandbox"
            }),
            "no block of {host} is in the event log: the connection did not reach purlis's \
             proxy\n{said}"
        );
        // The hold is the window's to show: an ask raised for the held connection, which the
        // person kept blocked, or at least one the proxy wrote when it raised the hold. On Linux
        // every leg showed one; on macOS the sandbox's block is written without an ask being
        // listed in time (OpenCode's Full leg), so there the block above is what is asserted and
        // the record says whether an ask was kept.
        assert!(
            !cfg!(target_os = "linux")
                || kept.get()
                || events
                    .iter()
                    .any(|event| event.kind == purlis_core::eventlog::stops::ASK_RAISED),
            "the held connection to {host} raised no ask: the person was never asked\n{said}"
        );
        if kept.get() {
            "the window listed its ask and the person kept it blocked"
        } else {
            "its hold ended unanswered, and the proxy refused it"
        }
    } else {
        scenario.answers_until_played(chat, &["network"], &driving);
        let result = StepResult::of(scenario.result_of(chat, "network").as_ref());
        let said = scenario.said();
        assert!(
            result.said.contains(PROXY_REFUSAL),
            "the link-local address was not refused by purlis's proxy: {result:?}\n{said}"
        );
        "a link-local address was refused by purlis's proxy in its own words"
    };
    scenario.keeps_listed_asks_blocked();
    record(
        test,
        dialect,
        protection,
        ReportVerdict::Pass,
        &format!(
            "the real {} under {}: the probe, a file cell and a network cell ({how})",
            dialect.word(),
            holder(dialect)
        ),
    );
}

/// **The probe** (T1.1-53, the §3.0 probe for a real binary): chat `chat` was started through the
/// host's own chat start, on the real binary and no fake, under the sandbox `protection` names,
/// and the stand-in model answered it.
fn probe(scenario: &Scenario, model: &Model, chat: u32, protection: Protection) {
    let dialect = scenario.dialect;
    let said = scenario.said();
    // The host's own chat start: its own Confine started the chat's confinement.
    assert!(
        scenario
            .confines
            .started()
            .iter()
            .all(|one| one.planned != purlis_host::Planned::Elsewhere)
            && !scenario.confines.started().is_empty(),
        "the host's own Confine started the chat's confinement\n{said}"
    );
    assert!(
        said.contains(dialect.word()) && !said.contains("fake-harness"),
        "the launch line does not name the real {}, or the fake harness was started\n{said}",
        dialect.word()
    );
    // The sandbox the window composed.
    let applied = scenario.applied_of(chat).expect("a sandbox was composed");
    let level = match protection {
        Protection::Full => purlis_core::sandbox::Level::Full,
        _ => purlis_core::sandbox::Level::Floor,
    };
    assert_eq!(applied.level(), level, "{said}");
    // Where purlis wraps the harness, the wrap is composed: a leg that ran unwrapped would pass
    // its cells for the wrong reason. Guaranteed at Full on both OSes and at the floor on Linux
    // (Landlock and seccomp); the floor on macOS composes no wrap program, and no more than that
    // is asserted for it.
    if holds_itself(dialect) {
        assert!(
            applied.wrap_program().is_none(),
            "{} holds itself and purlis wraps it too\n{said}",
            dialect.word()
        );
    } else if protection == Protection::Full || cfg!(target_os = "linux") {
        assert!(
            applied.wrap_program().is_some(),
            "{} is not wrapped by purlis here, and it should be\n{said}",
            dialect.word()
        );
    }
    // The process is the real binary, and the stand-in model answered it.
    let commands: Vec<String> = scenario.pids().into_iter().map(command_of).collect();
    assert!(
        commands
            .iter()
            .any(|command| command.contains(dialect.word()))
            && !commands.iter().any(|c| c.contains("fake-harness")),
        "the chat's process is not the real {}: {commands:?}\n{said}",
        dialect.word()
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

#[cfg(test)]
mod tests {
    use purlis_recordings::projection::Shown;

    use super::super::host::{Homes, Protection, Scenario};
    use super::*;

    /// **The Claude Code leg is set up for a scenario's child** or the test ends, saying why: the
    /// real Claude Code, and its four variables. Claude Code's own sandbox is the leg on macOS.
    macro_rules! the_claude_leg {
        ($test:expr) => {{
            let Some(program) = macos_claude($test) else {
                return;
            };
            let Some(env) = environment(Dialect::Claude) else {
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

    /// A request to the stand-in as `dialect`'s harness makes one, and the path it posts to: the
    /// chat's prompt, and each earlier call and its result.
    fn request(
        dialect: Dialect,
        prompt: &str,
        earlier: &[(&str, &str, bool)],
    ) -> (&'static str, Vec<u8>) {
        let body = match dialect {
            Dialect::Claude => {
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
                json!({
                    "model": "stand-in", "stream": false, "messages": messages,
                    "tools": [{"name": "Bash", "input_schema": {"type": "object"}}]
                })
            }
            Dialect::Codex => {
                let mut input = vec![json!({
                    "type": "message", "role": "user",
                    "content": [{"type": "input_text", "text": prompt}]
                })];
                for (id, result, _) in earlier {
                    input.push(json!({
                        "type": "function_call", "call_id": id,
                        "name": "shell_command", "arguments": "{}"
                    }));
                    input.push(json!({
                        "type": "function_call_output", "call_id": id, "output": result
                    }));
                }
                json!({
                    "model": "stand-in", "stream": false, "input": input,
                    "tools": [{"type": "function", "name": "shell_command"}]
                })
            }
            Dialect::OpenCode => {
                let mut messages = vec![json!({"role": "user", "content": prompt})];
                for (id, result, _) in earlier {
                    messages.push(json!({
                        "role": "assistant", "content": null,
                        "tool_calls": [{"id": id, "type": "function",
                                        "function": {"name": "bash", "arguments": "{}"}}]
                    }));
                    messages.push(json!({
                        "role": "tool", "tool_call_id": id, "content": result
                    }));
                }
                json!({
                    "model": "stand-in", "stream": false, "messages": messages,
                    "tools": [{"type": "function", "function": {"name": "bash"}}]
                })
            }
        };
        let path = match dialect {
            Dialect::Claude => "/v1/messages",
            Dialect::Codex => "/v1/responses",
            Dialect::OpenCode => "/v1/chat/completions",
        };
        (path, serde_json::to_vec(&body).expect("a request"))
    }

    /// What `dialect`'s harness shows the model of a command that ran and exited `code`, as the
    /// recordings (`tests/recordings/recorded`) have it, with `output` its output.
    fn result_of_a_command(dialect: Dialect, code: i64, output: &str) -> (String, bool) {
        match dialect {
            Dialect::Claude if code == 0 => (output.to_owned(), false),
            Dialect::Claude => (format!("Exit code {code}\n{output}"), true),
            Dialect::Codex => (
                format!(
                    "Chunk ID: 1a2b3c\nWall time: 0.0000 seconds\nProcess exited with code \
                     {code}\nOriginal token count: 30\nOutput:\n{output}\n"
                ),
                false,
            ),
            Dialect::OpenCode => (format!("{output}\n{EXIT_MARKER}{code}\n"), false),
        }
    }

    /// One thing the model was shown, a tool result.
    fn shown(text: &str, is_error: bool) -> Shown {
        Shown {
            kind: "tool_result".to_owned(),
            text: text.to_owned(),
            tool_use_id: None,
            is_error: Some(is_error),
        }
    }

    /// **The transcript is the stand-in model's request log** (T1.1-10 and T1.1-53, ticket 3
    /// item 7): what each harness showed the model, in its own API, a tool result a line with
    /// its step and its exit, read by the same code that reads the fake's transcript. Needs no
    /// harness.
    #[test]
    fn transcript_expectations_read_the_stand_in_models_request_log() {
        for dialect in Dialect::ALL {
            let word = dialect.word();
            let script = script_of(
                dialect,
                "log",
                &[
                    ("first", "echo one".to_owned()),
                    ("second", "curl https://refused.example.test/".to_owned()),
                ],
            );
            let prompt = script.prompt.clone();
            let stand_in = StandIn::new(word, script);
            let id = |response: usize| {
                stand_in.id(purlis_recordings::standin::Conversation::Main, response, 0)
            };
            let (first_id, second_id) = (id(0), id(1));
            let (one, _) = result_of_a_command(dialect, 0, "one");
            let (refused, refused_failed) = result_of_a_command(
                dialect,
                56,
                "curl: (56) CONNECT tunnel failed, response 403",
            );

            // The chat's first request is answered with the first step's call; its result
            // brings the second's.
            let (path, body) = request(dialect, &prompt, &[]);
            let first = stand_in.answer("POST", path, &body);
            assert!(
                String::from_utf8_lossy(&first.body).contains("echo one"),
                "{word}: the first request is answered with the first step: {first:?}"
            );
            let (path, body) = request(dialect, &prompt, &[(&first_id, &one, false)]);
            let second = stand_in.answer("POST", path, &body);
            assert!(
                String::from_utf8_lossy(&second.body).contains("refused.example.test"),
                "{word}: the first result brings the second step: {second:?}"
            );
            let (path, body) = request(
                dialect,
                &prompt,
                &[
                    (&first_id, &one, false),
                    (&second_id, &refused, refused_failed),
                ],
            );
            stand_in.answer("POST", path, &body);

            let transcript = transcript_of(dialect, &stand_in);
            let results: Vec<&Value> = transcript
                .iter()
                .filter(|line| line["kind"] == "tool_result")
                .collect();
            assert_eq!(
                results.len(),
                2,
                "{word}: one line to each tool result the model was shown: {transcript:?}"
            );
            assert_eq!(results[0]["step"], "first", "{word}");
            assert_eq!(results[0]["exit"], 0, "{word}");
            assert_eq!(results[1]["step"], "second", "{word}");
            assert_eq!(results[1]["exit"], 56, "{word}");

            // Read as the cells and the global checks read the fake's transcript.
            let read = StepResult::of(Some(results[1]));
            assert!(read.ran && read.exit == Some(56), "{word}: {read:?}");
            assert!(read.said.contains("403"), "{word}: {read:?}");
        }
    }

    /// **Each harness says an exit status in its own words** (T1.1-53), and a result that says
    /// none never ran: a call a hook denied, one still running, a command whose status OpenCode
    /// was not told.
    #[test]
    fn each_harness_says_its_exit_status_in_its_own_words() {
        // Claude Code: `Exit code N` first on a failure, nothing on a success.
        assert_eq!(
            exit_of(Dialect::Claude, "Exit code 7\nfailed", true),
            Some(7)
        );
        assert_eq!(exit_of(Dialect::Claude, "fine", false), Some(0));
        assert_eq!(exit_of(Dialect::Claude, "blocked by a hook", true), None);
        // Codex: in its result's header, and `Exit code: N` for its `apply_patch`.
        let (codex, _) = result_of_a_command(Dialect::Codex, 3, "said Process exited with code 9");
        assert_eq!(exit_of(Dialect::Codex, &codex, false), Some(3));
        assert_eq!(
            exit_of(
                Dialect::Codex,
                "Exit code: 1\nOutput:\nFailed to write file",
                false
            ),
            Some(1)
        );
        assert_eq!(
            exit_of(
                Dialect::Codex,
                "Command blocked by PreToolUse hook: refused. Command: echo x",
                false
            ),
            None
        );
        assert_eq!(
            exit_of(
                Dialect::Codex,
                "Process running with session ID 1000",
                false
            ),
            None
        );
        // OpenCode: the marker its command ends by saying, the last one in the result.
        let (opencode, _) = result_of_a_command(Dialect::OpenCode, 56, "said purlis-leg-exit=9");
        assert_eq!(exit_of(Dialect::OpenCode, &opencode, false), Some(56));
        assert_eq!(
            exit_of(
                Dialect::OpenCode,
                "purlis-recorder: refused with exit 2",
                false
            ),
            None
        );
    }

    /// **A command is asked of OpenCode so that its result says its exit** (T1.1-53): the others'
    /// own results say it, so their commands are asked as they are.
    #[test]
    fn a_command_asked_of_opencode_ends_by_saying_its_exit_status() {
        let asked = command_for(Dialect::OpenCode, "false # a comment");
        assert!(asked.contains("false # a comment"), "{asked}");
        assert!(
            asked.ends_with(&format!("echo \"{EXIT_MARKER}$?\"")),
            "a comment in the command must not eat what says the exit: {asked}"
        );
        assert_eq!(command_for(Dialect::Codex, "echo hi"), "echo hi");
        assert_eq!(command_for(Dialect::Claude, "echo hi"), "echo hi");
        // And it reads back what a shell says of the command.
        let out = purlis_core::forklock::output(
            std::process::Command::new("/bin/sh").args(["-c", &asked]),
        )
        .expect("a shell");
        let said = String::from_utf8_lossy(&out.stdout);
        assert_eq!(exit_of(Dialect::OpenCode, &said, false), Some(1), "{said}");
    }

    /// **Each harness is started with the words that reach the stand-in** (T1.1-53), and its
    /// first message last, as the harness takes one: Claude Code's permission mode, Codex's model
    /// provider (the stand-in, over Responses, with no retry), and OpenCode's `--prompt`.
    #[test]
    fn each_harness_is_started_with_the_words_that_reach_the_stand_in() {
        let url = "http://127.0.0.1:4321";
        let claude = launch_words(Dialect::Claude, url, "go on");
        assert_eq!(claude, ["--permission-mode", "bypassPermissions", "go on"]);

        let codex = launch_words(Dialect::Codex, url, "go on");
        assert_eq!(codex.last().map(String::as_str), Some("go on"));
        let settings: Vec<&str> = codex
            .iter()
            .zip(codex.iter().skip(1))
            .filter(|(flag, _)| *flag == "-c")
            .map(|(_, setting)| setting.as_str())
            .collect();
        for wanted in [
            "model_provider=\"standin\"",
            "model_providers.standin.base_url=\"http://127.0.0.1:4321/v1\"",
            "model_providers.standin.wire_api=\"responses\"",
            "model_providers.standin.request_max_retries=0",
        ] {
            assert!(settings.contains(&wanted), "{wanted} is missing: {codex:?}");
        }

        let opencode = launch_words(Dialect::OpenCode, url, "go on");
        assert_eq!(opencode, ["--prompt", "go on"]);
    }

    /// **What the leg adds to a chat names no credential** (T1.1-53): each variable it names is a
    /// literal that is no credential, a switch, or the stand-in's loopback address, the host-only
    /// variable is in no chat's list, and OpenCode's configuration file names the stand-in as
    /// its one provider.
    #[test]
    fn what_the_leg_adds_to_a_chat_names_no_credential_and_only_the_stand_in() {
        let url = "http://127.0.0.1:4321";
        for dialect in Dialect::ALL {
            let environment = environment_at(dialect, url);
            let pass = chat_env_pass(dialect);
            for name in added(dialect) {
                let value = &environment
                    .iter()
                    .find(|(one, _)| one == name)
                    .unwrap_or_else(|| panic!("{name} has no value"))
                    .1;
                assert!(
                    value == NOT_A_CREDENTIAL
                        || value == "1"
                        || value == url
                        || *value == opencode_config_path(url).display().to_string(),
                    "{name} is given {value}, which is no literal the leg owns"
                );
                assert!(
                    pass.contains(&format!("\"{name}\"")),
                    "{name} is not passed: {pass}"
                );
            }
            assert!(
                !pass.contains(STAND_IN_URL),
                "the host-only address is in a chat's list: {pass}"
            );
            for (name, value) in &environment {
                assert!(
                    added(dialect).contains(&name.as_str()) || name == STAND_IN_URL,
                    "{name} is a variable the leg gives that no list names"
                );
                assert!(
                    !value.contains("sk-") && !value.contains("Bearer"),
                    "{name} looks like a credential"
                );
            }
        }
        // The file OpenCode is told of holds that configuration, and is not the inline variable
        // purlis sets for its own plugin.
        assert!(!added(Dialect::OpenCode).contains(&"OPENCODE_CONFIG_CONTENT"));
        assert!(
            opencode_config_path(url)
                .file_name()
                .is_some_and(|name| name == "opencode.json")
                && opencode_config_dir(url)
                    .file_name()
                    .is_some_and(|name| name == "purlis-leg-opencode-4321")
        );
        let config: Value = serde_json::from_str(&opencode_config(url)).expect("JSON");
        assert_eq!(
            config["provider"]["standin"]["options"]["baseURL"],
            "http://127.0.0.1:4321/v1"
        );
        assert_eq!(config["model"], "standin/stand-in");
        assert_eq!(config["autoupdate"], false);
        assert_eq!(config["provider"].as_object().map(|one| one.len()), Some(1));
    }

    /// **A real binary outside the wrap fails the leg's canary** (T1.1-53), read as each harness
    /// says its results: the canary's write got through (exit 0, and the file is there) and the
    /// canary is red, whichever harness ran it; the same two commands refused, in each harness's
    /// own words, are green; and a call that never ran (no exit, as a hook's refusal leaves it)
    /// is no refusal.
    #[test]
    fn a_real_binary_outside_the_wrap_fails_the_canary() {
        let line = |dialect: Dialect, code: i64, output: &str| {
            let (text, failed) = result_of_a_command(dialect, code, output);
            StepResult::of(Some(&line_of(dialect, &[], &shown(&text, failed))))
        };
        for dialect in Dialect::ALL {
            let word = dialect.word();
            // Outside the wrap: the write went through and the file is there.
            let outside = Canary {
                wrote: true,
                dialled: 0,
                write: line(dialect, 0, ""),
                connect: line(
                    dialect,
                    7,
                    "curl: (7) Failed to connect to 127.0.0.1 port 9",
                ),
            };
            let red = super::super::checks::judge_canary(&outside)
                .expect_err("a write that got through must fail the canary");
            assert!(red.contains("write outside the project"), "{word}: {red}");
            // Outside the wrap: the dial arrived.
            let dialled = Canary {
                wrote: false,
                dialled: 1,
                write: line(dialect, 1, "sh: /x/y: Permission denied"),
                connect: line(dialect, 0, ""),
            };
            let red = super::super::checks::judge_canary(&dialled)
                .expect_err("a dial that got through must fail the canary");
            assert!(red.contains("direct dials"), "{word}: {red}");
            // Inside it: both refused, said in the harness's words.
            let refused = Canary {
                wrote: false,
                dialled: 0,
                write: line(dialect, 1, "/bin/bash: line 1: /x/y: Permission denied"),
                connect: line(
                    dialect,
                    7,
                    "curl: (7) Failed to connect to 127.0.0.1 port 9",
                ),
            };
            assert_eq!(
                super::super::checks::judge_canary(&refused),
                Ok(()),
                "{word}: a refused write and a refused connect are green"
            );
            // A call that never ran refuses nothing.
            let never_ran = Canary {
                wrote: false,
                dialled: 0,
                write: StepResult::of(Some(&line_of(
                    dialect,
                    &[],
                    &shown("Command blocked by PreToolUse hook: refused", true),
                ))),
                connect: line(
                    dialect,
                    7,
                    "curl: (7) Failed to connect to 127.0.0.1 port 9",
                ),
            };
            let red = super::super::checks::judge_canary(&never_ran)
                .expect_err("a write that never ran is not a refused write");
            assert!(red.contains("write was not seen refused"), "{word}: {red}");
        }
    }

    /// **The transcript of a chat survives the next chat's start** (T1.1-10, review F3, and
    /// each harness here): each chat has a stand-in and a file of its own, so a two-chat cell
    /// reads chat 1's result after chat 2 began. Needs no harness.
    #[test]
    fn a_chats_transcript_is_still_readable_after_the_next_chat_starts() {
        for dialect in Dialect::ALL {
            let word = dialect.word();
            let dir =
                std::env::temp_dir().join(format!("leg-two-chats-{word}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a folder");
            let model = Model::serve_on(dialect, 0, None);
            let (first, second) = (dir.join("one.jsonl"), dir.join("two.jsonl"));

            let prompt = model.plays(&[("only", "echo one".to_owned())]);
            model.attach(&first);
            let stand_in = Arc::clone(&model.current.lock().expect("the stand-in"));
            let id = stand_in.id(purlis_recordings::standin::Conversation::Main, 0, 0);
            let (path, body) = request(dialect, &prompt, &[]);
            stand_in.answer("POST", path, &body);
            let (output, _) = result_of_a_command(dialect, 0, "one");
            let (path, body) = request(dialect, &prompt, &[(&id, &output, false)]);
            stand_in.answer("POST", path, &body);
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
                    "{word}: chat 1's transcript was never written"
                );
                std::thread::sleep(Duration::from_millis(20));
            }

            // Chat 2 starts, the way the host starts it: played, then attached.
            model.plays(&[("other", "echo two".to_owned())]);
            std::thread::sleep(Duration::from_millis(500));
            assert!(
                read(&first),
                "{word}: chat 1's result was wiped once chat 2 was played"
            );
            model.attach(&second);
            std::thread::sleep(Duration::from_millis(500));
            assert!(
                read(&first),
                "{word}: chat 1's result was wiped once chat 2 was attached"
            );
            assert!(
                !read(&second),
                "{word}: chat 2 has no result of chat 1's step"
            );
            std::fs::remove_dir_all(&dir).expect("the folder goes");
        }
    }

    /// **A real-harness scenario at Full whose canary never ran fails** (T1.1-10, review F1).
    #[test]
    fn a_real_harness_cell_that_never_ran_the_canary_fails() {
        let owed = super::super::host::canary_owed(true, false)
            .expect_err("a leg at Full that skipped its canary must fail");
        assert!(owed.contains("canary never ran"), "{owed}");
        assert!(super::super::host::canary_owed(true, true).is_ok());
        assert!(
            super::super::host::canary_owed(false, false).is_ok(),
            "a scenario that owes none is not failed"
        );
    }

    /// **A binary at another version than the pin does not run the leg** (T1.1-10, and each
    /// harness here): what `--version` says is the pin's own line, or the leg does not run.
    #[test]
    fn a_harness_at_another_version_than_the_pin_is_refused() {
        for (dialect, newer) in [
            (Dialect::Claude, "2.1.400 (Claude Code)"),
            (Dialect::Codex, "codex-cli 0.200.0"),
            (Dialect::OpenCode, "1.99.0"),
        ] {
            let pin = pinned(dialect);
            assert!(
                version_matches(dialect, pin, pin).is_ok(),
                "{dialect:?}'s own pin is its own version"
            );
            assert!(
                version_matches(dialect, &format!("{}\n", pin.trim()), pin).is_ok(),
                "{dialect:?}: a trailing newline is no other version"
            );
            let why = version_matches(dialect, newer, pin).expect_err("another version");
            assert!(
                why.contains(newer)
                    && why.contains(pin.trim())
                    && why.contains(host::program_var(dialect)),
                "{why}"
            );
            assert!(
                version_matches(dialect, "", pin).is_err(),
                "{dialect:?}: no answer is no match"
            );
        }
    }

    /// **A person answers what a real harness's screen asks** (T1.1-53): Codex's offer of a newer
    /// model is answered with "Use existing model" even though its words also say "press enter to
    /// confirm" (taking that for a first-run question and pressing Enter chose the new model,
    /// which Codex offers no tool: CI run 44919252197), and each other screen is answered as the
    /// recordings' driver answers it, within its bounds. Needs no harness.
    #[test]
    fn a_person_answers_each_screen_a_real_harness_puts_to_them() {
        use host::{Answered, Reply, reply_to};
        let offer = "Introducing gpt-6-sol\n  1. Try new model\n> 2. Use existing model\n\
                     Press enter to confirm or esc to go back";
        let fresh = Answered::default();
        assert_eq!(reply_to(offer, true, &fresh), Some(Reply::UseExistingModel));
        assert_eq!(
            reply_to(offer, false, &fresh),
            Some(Reply::UseExistingModel),
            "the offer is answered whether or not the model has been heard"
        );
        assert_eq!(
            reply_to(
                "Do you trust the files in this folder?\nEnter to continue",
                true,
                &fresh
            ),
            Some(Reply::Enter)
        );
        assert_eq!(
            reply_to("Do you trust the files in this folder?", false, &fresh),
            None,
            "a first-run question is answered only before the model has heard anything"
        );
        assert_eq!(
            reply_to(
                "WARNING: Bypass Permissions mode\n1. No, exit\n2. Yes, I accept",
                false,
                &fresh
            ),
            Some(Reply::AcceptTheWarning)
        );
        assert_eq!(
            reply_to(
                "Permission required\nAllow once  Allow always  Reject",
                false,
                &fresh
            ),
            Some(Reply::AllowOnce)
        );
        assert_eq!(reply_to("Run the steps of leg chat 1.", true, &fresh), None);
        // Bounded: a screen that does not go away is not answered for ever.
        let spent = Answered {
            newer_model: 3,
            ..Answered::default()
        };
        assert_eq!(reply_to(offer, true, &spent), Some(Reply::Enter));
    }

    /// **A held connection's result is read by short polls** (T1.1-53): whether the call that runs
    /// the command waits for it or gives its result back early (Codex's tool, after ten seconds,
    /// leaves it running), the file the command writes says its own exit, and a poll before then
    /// returns within seven seconds saying nothing of one. Needs no harness.
    #[test]
    fn a_held_commands_result_is_read_by_short_polls() {
        let dir = std::env::temp_dir().join(format!("leg-held-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let file = dir.join("network-result");
        let script = dir.join("held.sh");
        std::fs::write(
            &script,
            script_of_the_held("echo refused; sleep 9; sh -c 'exit 56'"),
        )
        .expect("a script");
        let sh = |command: &str| {
            purlis_core::forklock::output(
                std::process::Command::new("/bin/sh").args(["-c", command]),
            )
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .expect("a shell")
        };
        // The harness's call returns at once here, as Codex's does at its yield: the command goes
        // on beside it.
        let call = held_command(&script, &file);
        let mut running =
            purlis_core::forklock::spawn(std::process::Command::new("/bin/sh").args(["-c", &call]))
                .expect("the call");
        let started = Instant::now();
        let early = sh(&poll_for(&file));
        assert!(!early.contains("net-exit="), "{early}");
        assert!(
            early.trim_end().ends_with("polled") && started.elapsed() < Duration::from_secs(9),
            "a poll waited past its seven seconds ({:?}): {early}",
            started.elapsed()
        );
        let mut late = String::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !late.contains("net-exit=") && Instant::now() < deadline {
            late = sh(&poll_for(&file));
        }
        assert_eq!(number_after(&late, "net-exit=", false), Some(56), "{late}");
        assert!(late.contains("refused"), "{late}");
        running.wait().expect("the call ends");
        std::fs::remove_dir_all(&dir).expect("the folder goes");
    }

    /// **Only Claude Code on macOS holds itself** (T1.1-10): purlis wraps every other harness on
    /// every OS, at the floor as at Full, so the leg asks every cell of each.
    #[test]
    fn only_claude_code_on_macos_holds_itself() {
        assert!(holds_itself(Dialect::Claude) == cfg!(target_os = "macos"));
        assert!(!holds_itself(Dialect::Codex) && !holds_itself(Dialect::OpenCode));
    }

    /// **Each real harness is pinned by version and checksum, for each OS it runs on** (T1.1-53):
    /// the lockfile CI installs from holds, for each OS and CPU, the platform package at the
    /// version the harness's file pins, from the npm registry, with a SHA-512 integrity, which
    /// `npm ci` checks of every tarball it unpacks.
    #[test]
    fn every_real_harness_is_pinned_by_version_and_checksum_for_each_os_it_runs_on() {
        let pins = host::repo().join("tests/fixtures/harness-versions");
        let pinned = |file: &str| {
            std::fs::read_to_string(pins.join(file))
                .expect("a pin")
                .trim()
                .to_owned()
        };
        // The lockfile's folder, and the package and version it holds for an OS and CPU.
        let held = |folder: &str, os: &str, arch: &str| -> (String, String) {
            match folder {
                "claude-code" => (
                    format!("node_modules/@anthropic-ai/claude-code-{os}-{arch}"),
                    pinned("claude.txt")
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                ),
                "codex" => (
                    format!("node_modules/@openai/codex-{os}-{arch}"),
                    format!(
                        "{}-{os}-{arch}",
                        pinned("codex.txt").trim_start_matches("codex-cli ")
                    ),
                ),
                _ => (
                    format!("node_modules/opencode-{os}-{arch}"),
                    pinned("opencode.txt"),
                ),
            }
        };
        for folder in ["claude-code", "codex", "opencode"] {
            let lock: Value = serde_json::from_str(
                &std::fs::read_to_string(pins.join(folder).join("package-lock.json"))
                    .expect("a lockfile"),
            )
            .expect("JSON");
            for os in ["linux", "darwin"] {
                for arch in ["x64", "arm64"] {
                    let (key, version) = held(folder, os, arch);
                    let entry = &lock["packages"][&key];
                    assert_eq!(
                        entry["version"],
                        version.as_str(),
                        "{folder}'s lockfile holds {key} at another version than its pin"
                    );
                    assert!(
                        entry["integrity"]
                            .as_str()
                            .is_some_and(|sum| sum.starts_with("sha512-") && sum.len() == 95),
                        "{folder}'s lockfile holds {key} with no SHA-512 integrity: {entry}"
                    );
                    assert!(
                        entry["resolved"]
                            .as_str()
                            .is_some_and(|url| url.starts_with("https://registry.npmjs.org/")),
                        "{folder}'s lockfile resolves {key} from somewhere but the npm registry: \
                         {entry}"
                    );
                }
            }
        }
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

    /// Whether this is the OS where Claude Code is wrapped whole, or the test ends, reporting
    /// `test` not run on another.
    fn wrapped_whole_here(test: &str) -> bool {
        if cfg!(target_os = "linux") {
            return true;
        }
        let reason = "purlis wraps Claude Code whole on Linux alone; on macOS it runs its own \
                      sandbox (T1.1-10's leg)";
        for protection in [Protection::Full, Protection::Basic] {
            record(
                test,
                Dialect::Claude,
                protection,
                purlis_scenarios::model::ReportVerdict::NotRun,
                reason,
            );
        }
        let _ = host::does_not_apply::<()>(reason);
        false
    }

    /// **A real Codex under purlis's wrap raises a held ask in the host** (T1.1-53, #6900), at
    /// Full, on each OS: the canary (a write outside and a direct connect, both refused), the
    /// probe, the file cell and a held connection the person keeps blocked.
    #[test]
    fn a_real_codex_under_the_wrap_raises_a_held_ask_in_the_host() {
        let test = "a_real_codex_under_the_wrap_raises_a_held_ask_in_the_host";
        if cfg!(target_os = "macos") {
            // Codex 0.169.0 cannot start under the full macOS sandbox in the scenario host: its
            // state database does not migrate (#7352). A gap for the person's ruling, said, never
            // a pass; the floor and Linux run.
            let reason = "Codex cannot start under the full macOS sandbox here: its state \
                          database does not migrate (#7352)";
            record(
                test,
                Dialect::Codex,
                Protection::Full,
                purlis_scenarios::model::ReportVerdict::NotRun,
                reason,
            );
            let _ = host::does_not_apply::<()>(reason);
            return;
        }
        runs(Dialect::Codex, Protection::Full, test);
    }

    /// **A real Codex at the floor is held in the host** (T1.1-53), on each OS: the probe, the
    /// file cell and the network cell at Basic, where no host is asked about.
    #[test]
    fn a_real_codex_at_the_floor_is_held_in_the_host() {
        runs(
            Dialect::Codex,
            Protection::Basic,
            "a_real_codex_at_the_floor_is_held_in_the_host",
        );
    }

    /// **A real OpenCode under purlis's wrap raises a held ask in the host** (T1.1-53, #6900), at
    /// Full, on each OS.
    #[test]
    fn a_real_opencode_under_the_wrap_raises_a_held_ask_in_the_host() {
        runs(
            Dialect::OpenCode,
            Protection::Full,
            "a_real_opencode_under_the_wrap_raises_a_held_ask_in_the_host",
        );
    }

    /// **A real OpenCode at the floor is held in the host** (T1.1-53), on each OS.
    #[test]
    fn a_real_opencode_at_the_floor_is_held_in_the_host() {
        runs(
            Dialect::OpenCode,
            Protection::Basic,
            "a_real_opencode_at_the_floor_is_held_in_the_host",
        );
    }

    /// **A real Claude Code wrapped whole on Linux raises a held ask** (T1.1-53, #6900): there
    /// purlis wraps Claude Code itself, its own sandbox off (`claude_wrap.rs`), at Full.
    #[test]
    fn a_real_claude_code_wrapped_whole_on_linux_raises_a_held_ask() {
        let test = "a_real_claude_code_wrapped_whole_on_linux_raises_a_held_ask";
        if wrapped_whole_here(test) {
            runs(Dialect::Claude, Protection::Full, test);
        }
    }

    /// **A real Claude Code wrapped whole on Linux is held at the floor** (T1.1-53).
    #[test]
    fn a_real_claude_code_wrapped_whole_on_linux_is_held_at_the_floor() {
        let test = "a_real_claude_code_wrapped_whole_on_linux_is_held_at_the_floor";
        if wrapped_whole_here(test) {
            runs(Dialect::Claude, Protection::Basic, test);
        }
    }

    /// **Claude Code starts through the host's own chat start** (T1.1-10): the chat is composed
    /// as the window composes one and started in the session host, with the settings, plugin and
    /// proxy purlis writes, on the real binary and no fake. Claude Code is not wrapped by purlis
    /// here: it is the real program, below the host, with its own sandbox in its settings.
    #[test]
    fn the_claude_leg_starts_claude_code_through_the_hosts_own_chat_start() {
        let homes =
            the_claude_leg!("the_claude_leg_starts_claude_code_through_the_hosts_own_chat_start");
        let model = Model::serve(Dialect::Claude);
        let Some(scenario) =
            Scenario::for_real_harness(&homes, Protection::Full, Dialect::Claude, &model)
        else {
            return;
        };
        let prompt = model.plays(&[("hello", "echo hello-from-the-leg".to_owned())]);
        let chat = scenario.starts_real(&model, &prompt);
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
    /// blocked, and the command is refused by the proxy's own word. The leg's canary runs first.
    #[test]
    fn claude_codes_own_sandbox_forwards_a_held_connection_to_the_purlis_proxy() {
        let homes = the_claude_leg!(
            "claude_codes_own_sandbox_forwards_a_held_connection_to_the_purlis_proxy"
        );
        let model = Model::serve(Dialect::Claude);
        let Some(scenario) =
            Scenario::for_real_harness(&homes, Protection::Full, Dialect::Claude, &model)
        else {
            return;
        };

        let host = "held-by-claude.example.test";
        let prompt = model.plays(&[("held-connection", super::super::reaches(host))]);
        let chat = scenario.starts_real(&model, &prompt);
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
