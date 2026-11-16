// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The recording driver** (T1.1-03 item 2): runs one real harness's covering script under
//! purlis's wrap at Full, against the stand-in model, and writes its recording and turn script.
//!
//! The chat is started as the session host composes a start (`purlis_core::start::ready`, then
//! `hoststart::compose_ready`, then `chatspec`), in a pseudo-terminal as the host runs it, on a
//! sandbox-only profile, with its confinement started for real (`sandbox::hosted::start`) and
//! its hooks heard on a hook socket as the window hears them. Its hook entries name the
//! recorder in place of `purlis` (the kit's binary). What differs from a person's chat: the model
//! is the stand-in, on a loopback port granted to the chat (`sandbox::local::grant_port`), and
//! the recorder holds or answers two hook calls itself (`forced`).
//!
//! `#[ignore]`d: it runs on Linux, in CI's recording job and the nightly re-record, with
//!
//! - `PURLIS_RECORD_HARNESS`: `claude`, `codex` or `opencode`, whose program is first on `PATH`;
//! - `PURLIS_RECORD_PURLIS`: the built `purlis` the wrap and the recorder run;
//! - `PURLIS_RECORD_OUT`: the folder `<harness>/<version>/` is written under;
//! - `PURLIS_CONFIG_HOME` and `PURLIS_DATA_HOME`: folders of the run's own.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use purlis_core::hoststart::{self, Beside, Kit, Starting};
use purlis_core::sandbox::hosted::{self, Hosted, Planned};
use purlis_recordings::recorder;
use purlis_recordings::recording::{self, Recording};
use purlis_recordings::script::Script;
use purlis_recordings::standin::{self, Conversation, StandIn};
use purlis_session_protocol::session::{self, ConfinePlan, Confined, ProgramKind, StartProgram};

/// The longest a recording runs before it is cut short and fails.
const AT_MOST: Duration = Duration::from_secs(420);

/// What the API key the stand-in is given says: no credential, and no credential's shape.
const STAND_IN_KEY: &str = "stand-in-model-no-credential";

/// The hook word whose first call the recorder holds past its timeout, and the one whose first
/// call it answers with exit 2.
const STALL: &str = "sessionstart";
const EXIT2: &str = "pretooluse";

/// The permission mode a Claude Code recording runs in ([`harness_words`]).
const PERMISSION_MODE: &str = "bypassPermissions";

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// The confinement, started for real, and every line the host would have recorded.
struct Host {
    root: PathBuf,
    hosted: Mutex<Option<Hosted>>,
    recorded: Mutex<Vec<serde_json::Value>>,
}

impl Beside for Host {
    fn confine(&self, plan: &ConfinePlan) -> Result<Confined, String> {
        let confine = session::Confine {
            confinement: "01JRECORDING00000000000000".to_owned(),
            project: "recording".to_owned(),
            plan: plan.clone(),
            asks: false,
        };
        let started = hosted::start(
            &self.root,
            &confine,
            Planned::Composed,
            &[],
            Arc::new(|_| {}),
            Arc::new(|| false),
        )?;
        let confined = started.confined();
        *self.hosted.lock().expect("the confinement") = Some(started);
        Ok(confined)
    }

    fn record(&self, entry: serde_json::Value) -> Result<(), String> {
        self.recorded.lock().expect("the record").push(entry);
        Ok(())
    }
}

#[test]
#[ignore = "records a real harness: CI's recording job and the nightly re-record run it on Linux"]
fn records_a_harness_under_purlis_wrap_at_full() {
    let harness = var("PURLIS_RECORD_HARNESS");
    let purlis = PathBuf::from(var("PURLIS_RECORD_PURLIS"));
    let out = PathBuf::from(var("PURLIS_RECORD_OUT"));
    let config_home = PathBuf::from(var("PURLIS_CONFIG_HOME"));
    let data_home = PathBuf::from(var("PURLIS_DATA_HOME"));
    let program = which(&harness);
    let version = version_of(&harness, &program);

    // Outside the system's temp folders, which a chat at Full may write.
    // Outside any git work tree, or a harness reads the repo's state into its prompt: the
    // run's own folder where it names one (`PURLIS_RECORD_BASE`).
    let base_in = std::env::var_os("PURLIS_RECORD_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    fs::create_dir_all(&base_in).expect("the run's folder");
    let base = tempfile::tempdir_in(&base_in).expect("a folder");
    let base_path = base.path().canonicalize().expect("the folder");
    let root = base_path.join("project");
    let outside = base_path.join("outside");
    let kit_dir = base_path.join("kit");
    for dir in [&root, &outside, &kit_dir] {
        fs::create_dir_all(dir).expect("a folder");
    }
    fs::write(
        root.join("purlis.toml"),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
    )
    .expect("the manifest");
    // The stand-in's key is this machine's, handed to the chat as a person hands an API key:
    // by name, in the project's local `[chat_env] pass` (a profile handed a key needs no
    // sign-in). The run sets it in its own environment (`.github/actions/harness-recordings`).
    let key_name = match harness.as_str() {
        "claude" => "ANTHROPIC_API_KEY",
        _ => "OPENAI_API_KEY",
    };
    assert_eq!(
        std::env::var(key_name).ok().as_deref(),
        Some(STAND_IN_KEY),
        "{key_name} is the stand-in's key in the run's environment"
    );
    fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!("[chat_env]\npass = [\"{key_name}\"]\n"),
    )
    .expect("the local settings");
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults");
    let denied_read = root.join(".purlis/vaults/covering.json");
    fs::write(&denied_read, "{\"covering\": \"a vault no chat reads\"}\n").expect("a vault");
    fs::write(outside.join("existing.txt"), "before\n").expect("a file outside");

    // The stand-in, on a port granted to the chat.
    let places: BTreeMap<String, String> = [
        ("work", root.display().to_string()),
        ("outside", outside.display().to_string()),
        ("denied_read", denied_read.display().to_string()),
        ("refused_url", "https://refused.example.invalid/".to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let script = Script::named("covering").expect("the covering script");
    let given = script.for_harness(&harness, &places);
    let stand_in = Arc::new(StandIn::new(&harness, given.clone()));
    let (port_tx, port_rx) = mpsc::channel();
    {
        let stand_in = Arc::clone(&stand_in);
        std::thread::spawn(move || {
            standin::serve(stand_in, |port| {
                let _ = port_tx.send(port);
            })
            .expect("the stand-in serves");
        });
    }
    let port = port_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the stand-in's port");
    purlis_core::sandbox::local::grant_port(&root, port).expect("the port is granted");
    let base_url = format!("http://127.0.0.1:{port}");

    // The sandbox-only profile `work`, and what purlis ships, with the recorder as its `purlis`.
    let homes = purlis_core::profilestore::Homes::here().expect("the run's homes");
    let made = purlis_core::profilestore::create_sandbox_only(&homes, "work", &harness, 1)
        .expect("the sandbox-only profile is made");
    let profile_config = made.record.folders.config.clone();
    let opencode_config = base_path.join("opencode-stand-in.json");
    prepare_harness(
        &harness,
        &profile_config,
        &root,
        &opencode_config,
        &base_url,
    );
    let recorder_bin = PathBuf::from(env!("CARGO_BIN_EXE_purlis-recorder"));
    let recorder_out = base_path.join("recorder.jsonl");
    let plugin = purlis_recordings::repo().join("app/src-tauri/plugin");
    let kit = Kit {
        binary: Some(recorder_bin.clone()),
        plugin: Some(plugin.clone()),
        git_hooks: None,
        skill_sets: None,
        chat_settings: Some(kit_dir.join("chat-settings")),
    };

    // The hook socket, as the window listens on it.
    let socket = root.join(".purlis/app/hooks.sock");
    fs::create_dir_all(socket.parent().expect("a folder")).expect("the socket's folder");
    let listener = purlis_core::hookwire::Listener::bind(&root, &socket).expect("a hook socket");
    let token = listener
        .tokens()
        .issue_to_this_process(1)
        .expect("a chat token");
    let (heard_tx, heard_rx) = mpsc::channel();
    let _reading = listener.each(Box::new(move |report| {
        let _ = heard_tx.send(format!("{report:?}"));
    }));

    // The start, as the session host composes it.
    let start = StartProgram {
        program: Some("01JRECORDINGCHAT0000000000".into()),
        run: Some("01JRECORDINGRUN00000000000".into()),
        profile: Some("work".into()),
        first_message: Some(given.prompt.clone()),
        ..StartProgram::of("01JRECORDINGPRJCT000000000", ProgramKind::Harness)
    };
    let reporting = session::Reporting {
        socket: socket.display().to_string(),
        token: token.expose().to_owned(),
    };
    let starting = Starting {
        root: &root,
        number: 1,
        start: &start,
        reporting: Some(&reporting),
        prompt_key: None,
        started_by: "terminal",
    };
    let mut ready = purlis_core::start::ready(
        &purlis_core::start::Start {
            profile: Some("work".into()),
            persona: None,
            name: "chat 1".into(),
            cwd: Some(root.clone()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: Default::default(),
            made: purlis_core::sandbox::Made::Unnamed,
        },
        &root,
    )
    .expect("the start is ready");
    let applied = ready.sandbox.as_mut().expect("the chat starts sandboxed");
    assert_eq!(
        applied.level(),
        purlis_core::sandbox::Level::Full,
        "the recording is made at Full"
    );
    applied.wrap_with(purlis.clone());
    ready.args.extend(harness_words(&harness, &base_url));
    let host = Host {
        root: root.clone(),
        hosted: Mutex::new(None),
        recorded: Mutex::new(Vec::new()),
    };
    let mut launch = hoststart::compose_ready(&starting, ready, &kit, &host).expect("the launch");
    launch.reporting = Some(reporting.clone());
    // The recorder writes where the chat may write: its own temp folder, which the confinement
    // made as the launch was composed.
    let chat_tmp = host
        .hosted
        .lock()
        .expect("the confinement")
        .as_ref()
        .and_then(|h| h.confined().tmp)
        .map(PathBuf::from);
    let recorder_out = chat_tmp
        .as_ref()
        .map_or(recorder_out, |tmp| tmp.join("recorder.jsonl"));
    launch.set.extend(
        [
            (recorder::REAL, purlis.display().to_string()),
            (recorder::OUT, recorder_out.display().to_string()),
            (recorder::STALL, STALL.to_owned()),
            (recorder::EXIT2, EXIT2.to_owned()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .chain(model_env(&harness, &base_url, &opencode_config)),
    );
    let size = purlis_core::engine::Size {
        columns: 160,
        rows: 50,
    };
    let spec = purlis_core::chatspec::from_launch_here(&launch, 1, size).expect("the spec");

    let engine = purlis_core::engine::AlacrittyEngine::new(size, 2000);
    let session = purlis_core::session::Session::spawn(spec, Box::new(engine))
        .expect("the harness starts in its terminal");

    // Until the turn ends: the stand-in said it was done, or the Esc response's call started
    // and Esc was pressed.
    let esc_at = given.responses.iter().position(|r| r.esc);
    let started = Instant::now();
    let mut pressed = false;
    let mut accepted_prompts = 0;
    let mut bypass_presses = 0u32;
    let mut harness_asks = 0u32;
    loop {
        assert!(
            started.elapsed() < AT_MOST,
            "{harness} did not finish the covering script within {AT_MOST:?}; its screen:\n{}\nthe stand-in heard: {:#?}",
            session.screen().lines.join("\n"),
            stand_in.heard.lock().expect("the log")
        );
        let heard = stand_in.heard.lock().expect("the log").clone();
        let done = heard.iter().any(|h| {
            h.conversation == Conversation::Main && h.response == Some(given.responses.len())
        });
        if done {
            std::thread::sleep(Duration::from_secs(5));
            break;
        }
        if let Some(esc_at) = esc_at
            && !pressed
            && heard
                .iter()
                .any(|h| h.conversation == Conversation::Main && h.response == Some(esc_at))
        {
            std::thread::sleep(Duration::from_secs(4));
            // OpenCode asks for Esc again before it interrupts ("esc interrupt"), measured on
            // 1.19.5's first recording; the others interrupt on one.
            session.write(b"\x1b").expect("Esc is pressed");
            if harness == "opencode" {
                std::thread::sleep(Duration::from_millis(500));
                session.write(b"\x1b").expect("Esc is pressed again");
            }
            pressed = true;
            std::thread::sleep(Duration::from_secs(8));
            break;
        }
        // A first-run question the harness asks in its terminal before the chat (a folder's
        // trust, a key's use) is answered as a person answers it: Enter on its default.
        let screen = session.screen().lines.join("\n");
        if accepted_prompts < 3 && heard.is_empty() && asks_to_go_on(&screen) {
            session.write(b"\r").expect("Enter is pressed");
            accepted_prompts += 1;
            std::thread::sleep(Duration::from_secs(2));
        }
        // Claude Code's bypass-permissions warning defaults to "No, exit": a person who chose
        // the mode takes "Yes, I accept".
        if bypass_presses < 5
            && screen.contains("Bypass Permissions mode")
            && screen.contains("Yes, I accept")
        {
            std::thread::sleep(Duration::from_secs(1));
            session.write(b"\x1b[B").expect("Down is pressed");
            std::thread::sleep(Duration::from_millis(600));
            session.write(b"\r").expect("Yes, I accept is chosen");
            bypass_presses += 1;
            std::thread::sleep(Duration::from_secs(2));
        }
        // A harness's own permission ask during the turn (OpenCode's "Permission required" for
        // a folder outside the project) is allowed once, as a person who lets the sandbox judge
        // allows it: then the wrap, not the harness, decides the call. Counted in the recording.
        if harness_asks < 8 && screen.contains("Permission required") {
            session.write(b"\r").expect("Allow once is chosen");
            harness_asks += 1;
            std::thread::sleep(Duration::from_secs(2));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let screen = session.screen().lines.join("\n");
    for _ in 0..2 {
        let _ = session.write(b"\x03");
        std::thread::sleep(Duration::from_millis(700));
    }
    let _ = session.wait(Duration::from_secs(10));
    session.end();

    // **What the refused calls aimed at is untouched** (Q301): a file outside the chat's folders
    // written, or one there changed, is an escape, filed as a public `area:security`,
    // `priority:p0` issue and fixed first, never pinned.
    let escaped: Vec<String> = [
        (outside.join("by-bash.txt"), None),
        (outside.join("by-write.txt"), None),
        (outside.join("existing.txt"), Some("before\n")),
    ]
    .into_iter()
    .filter_map(|(path, want)| {
        let now = fs::read_to_string(&path).ok();
        (now.as_deref() != want).then(|| format!("{}: {now:?}", path.display()))
    })
    .collect();
    assert!(
        escaped.is_empty(),
        "ESCAPE: {harness} {version} under purlis's wrap at Full did what was refused: {escaped:?}"
    );

    // The recording.
    let lines = recorder::read(&recorder_out)
        .unwrap_or_else(|e| panic!("the recorder wrote nothing ({e}); the screen:\n{screen}"));
    let model = stand_in.asked();
    let mut roots: BTreeMap<String, String> = BTreeMap::new();
    let mut root_of = |name: &str, path: &Path| {
        roots.insert(name.to_owned(), path.display().to_string());
        if let Ok(real) = path.canonicalize()
            && real != path
        {
            roots.insert(format!("{name}-real"), real.display().to_string());
        }
    };
    root_of("work", &root);
    root_of("outside", &outside);
    root_of("kit", &kit_dir);
    root_of("base", &base_path);
    root_of("config-home", &config_home);
    root_of("data-home", &data_home);
    root_of("profile", &profile_config);
    root_of("purlis", purlis.parent().expect("purlis's folder"));
    root_of(
        "recorder",
        recorder_bin.parent().expect("the recorder's folder"),
    );
    root_of("plugin", &plugin);
    root_of("harness", program.parent().expect("the harness's folder"));
    root_of("repo", &purlis_recordings::repo());
    if let Some(tmp) = &chat_tmp {
        root_of("chat-tmp", tmp);
    }
    if let Ok(home) = std::env::var("HOME") {
        root_of("home", Path::new(&home));
    }
    let made = Recording {
        harness: harness.clone(),
        version: version.clone(),
        level: "full".to_owned(),
        os: "linux".to_owned(),
        script: given.name.clone(),
        prompt: given.prompt.clone(),
        forced: [("stall", STALL), ("exit2", EXIT2)]
            .into_iter()
            .chain(pressed.then_some(("esc", "pressed")))
            .chain((harness == "claude").then_some(("permission_mode", PERMISSION_MODE)))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .chain((harness_asks > 0).then(|| {
                (
                    "harness_asks".to_owned(),
                    format!("allowed once, {harness_asks} times"),
                )
            }))
            .collect(),
        roots,
        hook_calls: recording::hook_calls(&lines, &model),
        model,
        shown: stand_in.shown(),
    };
    let masked = recording::masked(&made);
    let folder = out.join(&harness).join(&version);
    fs::create_dir_all(&folder).expect("the recording's folder");
    write_json(&folder.join("recording.json"), &masked);
    let checked: Recording = serde_json::from_value(masked.clone()).expect("a masked recording");
    write_json(
        &folder.join("turns.json"),
        &serde_json::to_value(purlis_recordings::turns::of(&checked)).expect("JSON"),
    );
    // What a reader of a red or surprising run needs, never checked in.
    let evidence = out.join("evidence").join(&harness);
    fs::create_dir_all(&evidence).expect("the evidence folder");
    fs::write(evidence.join("screen.txt"), &screen).expect("the screen");
    write_json(
        &evidence.join("heard.json"),
        &serde_json::to_value(&*stand_in.heard.lock().expect("the log")).expect("JSON"),
    );
    write_json(
        &evidence.join("recorded-by-the-host.json"),
        &serde_json::Value::Array(host.recorded.lock().expect("the record").clone()),
    );
    let told: Vec<String> = heard_rx.try_iter().collect();
    fs::write(evidence.join("hook-socket.txt"), told.join("\n")).expect("the socket's lines");

    let problems = recording::problems(&masked);
    assert!(problems.is_empty(), "the recording: {problems:?}");
    assert!(
        !checked.hook_calls.is_empty(),
        "{harness} made no hook call; the screen:\n{screen}"
    );
}

fn write_json(path: &Path, value: &serde_json::Value) {
    let mut text = serde_json::to_string_pretty(value).expect("JSON");
    text.push('\n');
    fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// The harness's program, as a shell finds it on `PATH`.
fn which(harness: &str) -> PathBuf {
    let name = match harness {
        "claude" => "claude",
        "codex" => "codex",
        _ => "opencode",
    };
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("no {name} on PATH"))
}

/// The version `program` answers `--version` with, as purlis-core reads it.
fn version_of(harness: &str, program: &Path) -> String {
    let said = std::process::Command::new(program)
        .arg("--version")
        .output()
        .expect("the harness answers --version");
    let said = String::from_utf8_lossy(&said.stdout).into_owned();
    purlis_core::profilestore::pin::version_in(harness, said.lines().next().unwrap_or_default())
        .map(|v| v.to_string())
        .unwrap_or_else(|| panic!("{harness} answered {said:?}"))
}

/// What a first run of the harness keeps of a person's answers, written as a person who has
/// answered them would have: the profile's own state, never purlis's settings.
fn prepare_harness(
    harness: &str,
    profile_config: &Path,
    root: &Path,
    opencode_config: &Path,
    base_url: &str,
) {
    match harness {
        "claude" => {
            // Onboarding done, the folder trusted, the stand-in's key approved.
            let state = profile_config.join(".claude.json");
            let mut value: serde_json::Value = fs::read_to_string(&state)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            let tail: String = STAND_IN_KEY
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
            // And in the profile's own settings, where 2.1 reads that the person accepted it.
            let settings = profile_config.join("settings.json");
            let mut own: serde_json::Value = fs::read_to_string(&settings)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            own["skipDangerousModePermissionPrompt"] = true.into();
            fs::create_dir_all(profile_config).expect("the profile's folder");
            write_json(&settings, &own);
            value["customApiKeyResponses"] =
                serde_json::json!({"approved": [tail], "rejected": []});
            value["projects"][root.display().to_string()] =
                serde_json::json!({"hasTrustDialogAccepted": true, "allowedTools": []});
            fs::create_dir_all(profile_config).expect("the profile's folder");
            write_json(&state, &value);
        }
        "opencode" => {
            write_json(
                opencode_config,
                &serde_json::json!({
                    "$schema": "https://opencode.ai/config.json",
                    "provider": {"standin": {
                        "npm": "@ai-sdk/openai-compatible",
                        "name": "stand-in",
                        "options": {"baseURL": format!("{base_url}/v1"), "apiKey": STAND_IN_KEY},
                        "models": {"stand-in": {"name": "stand-in", "tool_call": true}}
                    }},
                    "model": "standin/stand-in",
                    "small_model": "standin/stand-in",
                    "autoupdate": false,
                    "share": "disabled"
                }),
            );
        }
        _ => {}
    }
}

/// The words a harness is started with to reach the stand-in, beside purlis's own.
fn harness_words(harness: &str, base_url: &str) -> Vec<String> {
    match harness {
        // Claude Code's default, auto mode, checks each call on Anthropic's servers, which a
        // stand-in model is not: it stops the turn to ask (2.1.325's first recording). The
        // recording runs without its prompts instead; its deny rules and purlis's hooks hold
        // as they do in every mode.
        "claude" => vec!["--permission-mode".to_owned(), PERMISSION_MODE.to_owned()],
        // A model Codex knows, so it offers that model's tools (its freeform `apply_patch`
        // among them): under a name it does not know it offered none to write with (0.169.0's
        // first recording). Only the name: every request goes to the stand-in.
        // A model Codex knows, so it offers that model's tools (its freeform `apply_patch`
        // among them): under a name it does not know it offered none to write with (0.169.0's
        // first recording). Only the name: every request goes to the stand-in.
        "codex" => [
            "model_provider=\"standin\"".to_owned(),
            "model=\"gpt-5.3-codex\"".to_owned(),
            "model_providers.standin.name=\"stand-in\"".to_owned(),
            format!("model_providers.standin.base_url=\"{base_url}/v1\""),
            "model_providers.standin.wire_api=\"responses\"".to_owned(),
            "model_providers.standin.request_max_retries=0".to_owned(),
            "model_providers.standin.stream_max_retries=0".to_owned(),
        ]
        .into_iter()
        .flat_map(|setting| ["-c".to_owned(), setting])
        .collect(),
        _ => Vec::new(),
    }
}

/// The variables that point a harness at the stand-in.
fn model_env(harness: &str, base_url: &str, opencode_config: &Path) -> Vec<(String, String)> {
    let pairs: Vec<(&str, String)> = match harness {
        "claude" => vec![
            ("ANTHROPIC_BASE_URL", base_url.to_owned()),
            ("ANTHROPIC_API_KEY", STAND_IN_KEY.to_owned()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_owned()),
        ],
        "codex" => vec![("OPENAI_API_KEY", STAND_IN_KEY.to_owned())],
        _ => vec![
            ("OPENCODE_CONFIG", opencode_config.display().to_string()),
            ("OPENCODE_DISABLE_AUTOUPDATE", "1".to_owned()),
        ],
    };
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// Whether the screen asks a first-run question that Enter answers with its default.
fn asks_to_go_on(screen: &str) -> bool {
    let screen = screen.to_lowercase();
    [
        "do you trust",
        "trust the files",
        "press enter",
        "enter to confirm",
        "yes, proceed",
        // Codex 0.169's "Folder access" for a folder purlis marks untrusted.
        "open restricted",
        "enter continue",
    ]
    .iter()
    .any(|ask| screen.contains(ask))
}
