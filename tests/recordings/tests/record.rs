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
//! - `PURLIS_RECORD_SCRIPT`, optionally: the script it runs, `covering` (the default) or, for
//!   Claude Code alone, `asks`, the pass that keeps Claude Code's own prompts and answers one;
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
/// call it answers with exit 2. The held one is a PostToolUse, not the one SessionStart, so
/// purlis's SessionStart context reaches the model, and the harness's wait on it is measured
/// (`command-timeout.txt` in the evidence).
const STALL: &str = "posttooluse-blocked";
const EXIT2: &str = "pretooluse";

/// The permission mode a Claude Code covering recording runs in ([`harness_words`]), and the
/// one its `asks` pass runs in.
const PERMISSION_MODE: &str = "bypassPermissions";
const ASKS_PERMISSION_MODE: &str = "default";

/// What the vault the refused read aims at holds: it must never reach the model.
const VAULT_MARKER: &str = "COVERING-VAULT-MARKER-5e1d";

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
            // A person's chat's own: the recording runs at production timings.
            &purlis_core::sandbox::timings::Timings::production(),
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
    let script_name = std::env::var("PURLIS_RECORD_SCRIPT").unwrap_or_else(|_| "covering".into());
    let asks = script_name == "asks";
    assert!(
        !asks || harness == "claude",
        "the asks pass is Claude Code's alone"
    );
    let evidence = out.join("evidence").join(&harness).join(&script_name);
    fs::create_dir_all(&evidence).expect("the evidence folder");
    // An escape is said in a file of its own before the run fails on it, so the nightly's escape
    // entry is red on an escape alone, and on nothing else (`.github/actions/harness-recordings`).
    let escaped_file = evidence.join("escaped.txt");
    let _ = fs::remove_file(&escaped_file);

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
        // No host at all: the stand-in is reached by its granted loopback port, and every other
        // host, a real vendor's above all, is refused by the chat's proxy.
        "[sandbox]\nmode = \"on\"\negress = []\n",
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
    fs::write(
        &denied_read,
        format!("{{\"covering\": \"a vault no chat reads\", \"marker\": \"{VAULT_MARKER}\"}}\n"),
    )
    .expect("a vault");
    fs::write(outside.join("existing.txt"), "before\n").expect("a file outside");

    // What the refused dials aim at: a loopback listener of this run's, which could be dialled,
    // on a port the chat is never granted. Nothing may arrive at it.
    let refused = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a refused listener");
    let refused_target = format!(
        "127.0.0.1:{}",
        refused.local_addr().expect("its port").port()
    );
    let refused_url = format!("http://{refused_target}/");
    let arrived = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let arrived = Arc::clone(&arrived);
        refused.set_nonblocking(true).expect("a listener");
        std::thread::spawn(move || {
            let ends = Instant::now() + standin::AT_MOST;
            while Instant::now() < ends {
                match refused.accept() {
                    Ok((_, from)) => arrived.lock().expect("arrivals").push(from.to_string()),
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        });
    }

    // The stand-in, on a port granted to the chat.
    let places: BTreeMap<String, String> = [
        ("work", root.display().to_string()),
        ("outside", outside.display().to_string()),
        ("denied_read", denied_read.display().to_string()),
        ("refused_url", refused_url.clone()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let script = Script::named(&script_name).expect("the script");
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
    ready.args.extend(harness_words(&harness, &base_url, asks));
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
        ]
        .into_iter()
        .chain(
            (!asks)
                .then(|| {
                    [
                        (recorder::STALL, STALL.to_owned()),
                        (recorder::EXIT2, EXIT2.to_owned()),
                    ]
                })
                .into_iter()
                .flatten(),
        )
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

    // Until the chat's last turn ends: the stand-in said it was done, or the Esc response's call
    // started and Esc was pressed. Each turn after the first is typed once the one before it
    // ended normally and its Stop hook answered.
    let sequence = given.sequence();
    let esc_at = sequence.iter().position(|(_, r)| r.esc);
    let prompts = given.prompts();
    let started = Instant::now();
    let mut pressed = false;
    let mut typed = 1usize;
    let mut accepted_prompts = 0;
    let mut bypass_presses = 0u32;
    let mut harness_asks = 0u32;
    let mut dialog_since: Option<Instant> = None;
    let settled = |word: &str| {
        recorder::read(&recorder_out).is_ok_and(|lines| {
            lines
                .iter()
                .filter(|l| l.phase == "call" && l.argv.get(1).is_some_and(|w| w == word))
                .all(|call| lines.iter().any(|l| l.phase == "reply" && l.id == call.id))
        })
    };
    loop {
        assert!(
            started.elapsed() < AT_MOST,
            "{harness} did not finish the {script_name} script within {AT_MOST:?}; its screen:\n{}\nthe stand-in heard: {:#?}",
            session.screen().lines.join("\n"),
            stand_in.heard.lock().expect("the log")
        );
        let heard = stand_in.heard.lock().expect("the log").clone();
        let ended = |turn: usize| {
            heard
                .iter()
                .any(|h| h.conversation == Conversation::Main && h.ended_turn == Some(turn as u32))
        };
        if ended(prompts.len()) {
            std::thread::sleep(Duration::from_secs(5));
            break;
        }
        if typed < prompts.len() && ended(typed) {
            // Its Stop hook answered, or a little while for a harness that runs none.
            let since = Instant::now();
            while !settled("stop") && since.elapsed() < Duration::from_secs(15) {
                std::thread::sleep(Duration::from_millis(250));
            }
            std::thread::sleep(Duration::from_secs(3));
            session
                .write(prompts[typed].as_bytes())
                .expect("the next prompt is typed");
            std::thread::sleep(Duration::from_millis(800));
            session.write(b"\r").expect("Enter is pressed");
            typed += 1;
            continue;
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
        // Codex 0.169 offers a newer model before the chat: a person keeping theirs takes
        // "Use existing model".
        if accepted_prompts < 3 && screen.contains("Use existing model") {
            session.write(b"\x1b[B").expect("Down is pressed");
            std::thread::sleep(Duration::from_millis(600));
            session.write(b"\r").expect("Use existing model is chosen");
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
        // a folder outside the project; Claude Code's "Do you want to create …?" in its asks pass)
        // is allowed once, as a person who lets the sandbox judge allows it: then the wrap, not
        // the harness, decides the call. Counted in the recording's conditions. Only once
        // purlis's own hook on the ask has answered (`permissionrequest`): pressed sooner,
        // OpenCode cuts that hook off, and the recording says so (1.19.5, run 9).
        let asking =
            screen.contains("Permission required") || (asks && screen.contains("Do you want to"));
        // In the asks pass the dialog is left until Claude Code tells its Notification hook a
        // person is wanted, which it does only once the dialog has waited (run 43836719317
        // answered at once and recorded none), or 20 s.
        let asked_since = *dialog_since.get_or_insert_with(Instant::now);
        let notified = !asks
            || asked_since.elapsed() > Duration::from_secs(20)
            || recorder::read(&recorder_out).is_ok_and(|lines| {
                lines.iter().any(|l| {
                    l.phase == "call" && l.argv.get(1).is_some_and(|w| w == "notification")
                })
            });
        if !asking {
            dialog_since = None;
        }
        if harness_asks < 8 && settled("permissionrequest") && asking && notified {
            session.write(b"\r").expect("Allow once is chosen");
            harness_asks += 1;
            std::thread::sleep(Duration::from_secs(2));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let screen = session.screen().lines.join("\n");
    // The chat ends as a person ends it, and is left to run its closing hooks: every hook call
    // it began answers before the confinement goes. Claude Code is ended with its own `/exit`:
    // ended with Ctrl-C, its SessionEnd fired in some runs and not others, whatever its budget
    // (CI runs 43834558626 to 43861652433). The others, and Claude Code if it is still running,
    // with Ctrl-C until they exit.
    if harness == "claude" {
        std::thread::sleep(Duration::from_secs(2));
        let _ = session.write(b"/exit");
        std::thread::sleep(Duration::from_millis(800));
        let _ = session.write(b"\r");
        if !matches!(session.wait(Duration::from_secs(15)), Ok(Some(_))) {
            // What Claude Code shows when /exit did not end it, for the run's reader.
            let _ = fs::write(
                evidence.join("screen-after-exit.txt"),
                session.screen().lines.join("\n"),
            );
        }
    }
    for _ in 0..3 {
        if matches!(session.wait(Duration::from_millis(100)), Ok(Some(_))) {
            break;
        }
        let _ = session.write(b"\x03");
        if matches!(session.wait(Duration::from_secs(3)), Ok(Some(_))) {
            break;
        }
    }
    let _ = session.wait(Duration::from_secs(10));
    let since = Instant::now();
    while since.elapsed() < Duration::from_secs(15)
        && !recorder::read(&recorder_out).is_ok_and(|lines| {
            lines
                .iter()
                .filter(|l| l.phase == "call" && l.forced.is_none())
                .all(|call| lines.iter().any(|l| l.phase == "reply" && l.id == call.id))
        })
    {
        std::thread::sleep(Duration::from_millis(250));
    }
    session.end();

    // **What the refused calls aimed at is untouched** (Q301): a file outside the chat's folders
    // written, or one there changed, a dial that arrived at the listener the chat was never
    // granted, or the refused vault's contents in front of the model, is an escape, filed as a
    // public `area:security`, `priority:p0` issue and fixed first, never pinned.
    let mut escaped: Vec<String> = [
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
    for from in arrived.lock().expect("arrivals").iter() {
        escaped.push(format!("a dial arrived at {refused_url} from {from}"));
    }
    for heard in stand_in.heard.lock().expect("the log").iter() {
        if heard.shown.iter().any(|s| s.text.contains(VAULT_MARKER)) {
            escaped.push(format!(
                "the refused vault's contents reached the model in a request to {}",
                heard.path
            ));
        }
    }
    if !escaped.is_empty() {
        fs::write(&escaped_file, escaped.join("\n")).expect("the escape is said");
    }
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
    // The refused dial's target, whose port is this run's, as a URL or a host and port.
    roots.insert("refused-target".to_owned(), refused_target.clone());
    let made = Recording {
        harness: harness.clone(),
        version: version.clone(),
        level: "full".to_owned(),
        os: "linux".to_owned(),
        conditions: [(
            "summary".to_owned(),
            if asks {
                "Full sandbox, Claude Code's own prompts kept (permission mode default) and each allowed once".to_owned()
            } else {
                recording::FULL_BYPASSED.to_owned()
            },
        )]
        .into_iter()
        .chain((harness == "claude").then(|| {
            (
                "permission_mode".to_owned(),
                if asks { ASKS_PERMISSION_MODE } else { PERMISSION_MODE }.to_owned(),
            )
        }))
        .chain((harness == "claude").then(|| {
            (
                "sessionend_budget".to_owned(),
                format!("CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS={SESSIONEND_BUDGET_MS}"),
            )
        }))
        .chain((harness_asks > 0).then(|| {
            (
                "harness_asks".to_owned(),
                format!("allowed once, {harness_asks} times"),
            )
        }))
        .collect(),
        script: given.name.clone(),
        prompts: prompts.clone(),
        forced: (!asks)
            .then_some([("stall", STALL), ("exit2", EXIT2)])
            .into_iter()
            .flatten()
            .chain(pressed.then_some(("esc", "pressed")))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        roots,
        hook_calls: recording::hook_calls(&lines, &model),
        model,
        shown: stand_in.shown(),
        said: stand_in.said(),
    };
    let masked = recording::masked(&made);
    let folder = out.join(&harness).join(&version);
    fs::create_dir_all(&folder).expect("the recording's folder");
    let (recording_file, turns_file) = recording::files_of(&script_name);
    write_json(&folder.join(&recording_file), &masked);
    let checked: Recording = serde_json::from_value(masked.clone()).expect("a masked recording");
    write_json(
        &folder.join(&turns_file),
        &serde_json::to_value(purlis_recordings::turns::of(&checked)).expect("JSON"),
    );
    // What a reader of a red or surprising run needs, never checked in.
    fs::write(evidence.join("screen.txt"), &screen).expect("the screen");
    // How long the harness waited on the hook call the recorder held: from the held call's
    // start to the next hook call's, which the harness made only once it went on.
    let held = lines
        .iter()
        .position(|l| l.phase == "call" && l.forced.as_deref() == Some("stall"));
    if let Some(at) = held {
        let began = lines[at].at_ms;
        let next = lines[at + 1..]
            .iter()
            .find(|l| l.phase == "call")
            .map(|l| l.at_ms.saturating_sub(began));
        let said = match next {
            Some(ms) => format!(
                "{harness} {version}: the held `{STALL}` call (its entry's timeout: 5 s) began, and the next hook call {ms} ms later\n"
            ),
            None => format!("{harness} {version}: no hook call followed the held `{STALL}` call\n"),
        };
        print!("{said}");
        fs::write(evidence.join("command-timeout.txt"), said).expect("the measured wait");
    }
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

    let mut problems = recording::problems(&masked);
    problems.extend(recording::overlapping(&lines));
    assert!(problems.is_empty(), "the recording: {problems:?}");
    // Codex's native write: its own `apply_patch`, never the patch through its shell.
    if harness == "codex" {
        for call in checked
            .model
            .iter()
            .flat_map(|a| &a.calls)
            .filter(|c| c.step == "refused-write" || c.step == "refused-edit")
        {
            assert_eq!(
                call.tool, "apply_patch",
                "Codex was not offered its native apply_patch for {}",
                call.step
            );
        }
    }
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
fn harness_words(harness: &str, base_url: &str, asks: bool) -> Vec<String> {
    match harness {
        // Claude Code's default, auto mode, checks each call on Anthropic's servers, which a
        // stand-in model is not: it stops the turn to ask (2.1.325's first recording). The
        // covering recording runs without its prompts instead; its deny rules and purlis's
        // hooks hold as they do in every mode. The asks pass keeps them, in the `default` mode.
        "claude" => vec![
            "--permission-mode".to_owned(),
            if asks {
                ASKS_PERMISSION_MODE
            } else {
                PERMISSION_MODE
            }
            .to_owned(),
        ],
        // A model in the catalogue Codex 0.169 carries, so it offers that model's tools, its
        // freeform `apply_patch` among them: `gpt-5.3-codex` is not in it, and under it Codex
        // offered nothing to write with (0.169.0's first recording). Only the name: every
        // request goes to the stand-in.
        "codex" => [
            "model_provider=\"standin\"".to_owned(),
            format!("model=\"{CODEX_MODEL}\""),
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

/// The time Claude Code is told its SessionEnd hooks may take, a recording condition.
const SESSIONEND_BUDGET_MS: &str = "10000";

/// The model Codex is told it runs: one its own catalogue lists with a freeform `apply_patch`.
const CODEX_MODEL: &str = "gpt-5.5";

/// The variables that point a harness at the stand-in.
fn model_env(harness: &str, base_url: &str, opencode_config: &Path) -> Vec<(String, String)> {
    let pairs: Vec<(&str, String)> = match harness {
        "claude" => vec![
            ("ANTHROPIC_BASE_URL", base_url.to_owned()),
            ("ANTHROPIC_API_KEY", STAND_IN_KEY.to_owned()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_owned()),
            // A recording condition: Claude Code's own budget for SessionEnd hooks cut the hook
            // off before it had begun in one of two runs (run 43836719317).
            (
                "CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS",
                SESSIONEND_BUDGET_MS.to_owned(),
            ),
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
