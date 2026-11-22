// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`fake-harness --script` started as purlis starts a harness**, under purlis's wrap at Full
//! (T1.1-04): the start composed as the session host composes it, its confinement started for
//! real, its hooks heard on a hook socket, and its hook entries naming T1.1-03's recorder, as
//! the recordings were made (`tests/recordings/tests/record.rs`). The fake is the program, through
//! the dialect's shim in `crates/fake-harness/shims`, which is first on the `PATH`.
//!
//! `#[ignore]`d: it binds a hook socket and confines on Linux, so it runs in CI's
//! `fake-harness under the wrap` job (`.github/workflows/fake-harness-wrap.yml`), with
//!
//! - `FAKE_WRAP_DIALECT`: `claude`, `codex` or `opencode`;
//! - `FAKE_WRAP_PURLIS`: the built `purlis` the wrap and the recorder run;
//! - `FAKE_WRAP_RECORDER`: the built `purlis-recorder`;
//! - `FAKE_HARNESS_BIN`: the built `fake-harness` the shims run;
//! - `FAKE_HARNESS_SCRIPT`: the dialect's checked-in turn script (what `--version` answers from);
//! - `FAKE_WRAP_OUT`: where evidence is written;
//! - `PURLIS_CONFIG_HOME` and `PURLIS_DATA_HOME`: folders of the run's own.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::hoststart::{self, Beside, Kit, Starting};
use purlis_core::sandbox::hosted::{self, Hosted, Planned};
use purlis_recordings::recorder;
use purlis_recordings::recording::{self, Recording};
use purlis_session_protocol::session::{self, ConfinePlan, Confined, ProgramKind, StartProgram};
use serde_json::{Value, json};

const AT_MOST: Duration = Duration::from_secs(300);

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

struct Host {
    root: PathBuf,
    hosted: Mutex<Option<Hosted>>,
}

impl Beside for Host {
    fn confine(&self, plan: &ConfinePlan) -> Result<Confined, String> {
        let confine = session::Confine {
            confinement: "01JFAKEWRAP000000000000000".to_owned(),
            project: "fake-wrap".to_owned(),
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
            &purlis_core::sandbox::timings::Timings::production(),
        )?;
        let confined = started.confined();
        *self.hosted.lock().expect("the confinement") = Some(started);
        Ok(confined)
    }

    fn record(&self, _entry: Value) -> Result<(), String> {
        Ok(())
    }
}

/// One chat of the fake, played to its end.
struct Played {
    /// The run's folder, kept until the test is done with what is in it.
    _base: tempfile::TempDir,
    dialect: String,
    lines: Vec<recorder::Line>,
    transcript: Vec<Value>,
    roots: BTreeMap<String, String>,
    outside: PathBuf,
    root: PathBuf,
    screen: String,
}

/// What a chat is started with: the script, and what the recorder does in place of a hook.
struct Given {
    script: PathBuf,
    forced: BTreeMap<String, String>,
    /// The step whose call is interrupted with Esc, once its guard was heard.
    esc_step: Option<String>,
    /// Whether the hooks and the MCP server run T1.1-03's recorder (`true`) or purlis itself.
    recorded: bool,
    /// Claude Code's `--permission-mode`, as the recording was made in.
    permission_mode: &'static str,
    /// Whether each of Claude Code's own asks is allowed once, as T1.1-03's driver allows it in
    /// its `asks` pass: Enter, once purlis's permission hook answered and Claude Code told its
    /// Notification hook a person is wanted.
    allows_asks: bool,
}

/// The permission mode of a run no recording is compared with: Claude Code's own prompts
/// bypassed, its deny rules kept, as its covering recording was made.
const BYPASS: &str = "bypassPermissions";

fn play(given: &Given) -> Played {
    let dialect = var("FAKE_WRAP_DIALECT");
    let purlis = PathBuf::from(var("FAKE_WRAP_PURLIS"));
    let recorder_bin = PathBuf::from(var("FAKE_WRAP_RECORDER"));
    let fake = PathBuf::from(var("FAKE_HARNESS_BIN"));
    let base_in = std::env::var_os("FAKE_WRAP_BASE")
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
        "[sandbox]\nmode = \"on\"\negress = []\n",
    )
    .expect("the manifest");
    fs::create_dir_all(root.join(".purlis/vaults")).expect("the vaults");
    fs::write(
        root.join(".purlis/vaults/covering.json"),
        "{\"covering\": \"a vault no chat reads\"}\n",
    )
    .expect("a vault");
    fs::write(outside.join("existing.txt"), "before\n").expect("a file outside");
    // What the refused dials aim at: a loopback listener of this run's, on a port the chat is
    // never granted. Anything arriving there is an escape.
    let refused = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a refused listener");
    let refused_target = format!(
        "127.0.0.1:{}",
        refused.local_addr().expect("its port").port()
    );
    let arrived = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let arrived = Arc::clone(&arrived);
        refused.set_nonblocking(true).expect("a listener");
        std::thread::spawn(move || {
            let ends = Instant::now() + AT_MOST;
            while Instant::now() < ends {
                match refused.accept() {
                    Ok((_, from)) => arrived.lock().expect("arrivals").push(from.to_string()),
                    Err(_) => std::thread::sleep(Duration::from_millis(20)),
                }
            }
        });
    }
    let transcript = base_path.join("transcript.jsonl");
    let state = base_path.join("fake-state");

    let homes = purlis_core::profilestore::Homes::here().expect("the run's homes");
    let made = purlis_core::profilestore::create_sandbox_only(&homes, "work", &dialect, 1)
        .expect("the sandbox-only profile is made");
    let profile_config = made.record.folders.config.clone();
    let plugin = repo().join("app/src-tauri/plugin");
    let kit = Kit {
        binary: Some(if given.recorded {
            recorder_bin.clone()
        } else {
            purlis.clone()
        }),
        plugin: Some(plugin.clone()),
        git_hooks: None,
        skill_sets: None,
        chat_settings: Some(kit_dir.join("chat-settings")),
    };

    let socket = root.join(".purlis/app/hooks.sock");
    fs::create_dir_all(socket.parent().expect("a folder")).expect("the socket's folder");
    let listener = purlis_core::hookwire::Listener::bind(&root, &socket).expect("a hook socket");
    let token = listener
        .tokens()
        .issue_to_this_process(1)
        .expect("a chat token");
    let _reading = listener.each(Box::new(|_| {}));

    let script = fs::read_to_string(&given.script).expect("the script");
    let first: Value = serde_json::from_str(&script).expect("JSON");
    let prompt = first["turns"][0]["prompt"]
        .as_str()
        .unwrap_or("go")
        .to_owned();
    let start = StartProgram {
        program: Some("01JFAKEWRAPCHAT00000000000".into()),
        run: Some("01JFAKEWRAPRUN000000000000".into()),
        profile: Some("work".into()),
        first_message: Some(prompt),
        ..StartProgram::of("01JFAKEWRAPPRJCT0000000000", ProgramKind::Harness)
    };
    let reporting = session::Reporting {
        seal: None,
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
    assert_eq!(applied.level(), purlis_core::sandbox::Level::Full);
    applied.wrap_with(purlis.clone());
    if dialect == "claude" {
        ready.args.extend([
            "--permission-mode".to_owned(),
            given.permission_mode.to_owned(),
        ]);
    }
    let host = Host {
        root: root.clone(),
        hosted: Mutex::new(None),
    };
    let mut launch = hoststart::compose_ready(&starting, ready, &kit, &host).expect("the launch");
    launch.reporting = Some(reporting.clone());
    let chat_tmp = host
        .hosted
        .lock()
        .expect("the confinement")
        .as_ref()
        .and_then(|h| h.confined().tmp)
        .map(PathBuf::from);
    let recorder_out = chat_tmp
        .as_ref()
        .map_or(base_path.join("recorder.jsonl"), |tmp| {
            tmp.join("recorder.jsonl")
        });
    // The transcript and the cursor where the chat may write: its own temp folder.
    let (transcript, state) = match &chat_tmp {
        Some(tmp) => (tmp.join("transcript.jsonl"), tmp.join("fake-state")),
        None => (transcript, state),
    };
    let places = json!({
        "work": root.display().to_string(),
        "outside": outside.display().to_string(),
        "refused-target": refused_target,
    });
    launch.set.extend(
        [
            (recorder::REAL, purlis.display().to_string()),
            (recorder::OUT, recorder_out.display().to_string()),
            ("FAKE_HARNESS_BIN", fake.display().to_string()),
            ("FAKE_HARNESS_SCRIPT", given.script.display().to_string()),
            ("FAKE_HARNESS_TRANSCRIPT", transcript.display().to_string()),
            ("FAKE_HARNESS_STATE", state.display().to_string()),
            ("FAKE_HARNESS_PLACES", places.to_string()),
        ]
        .into_iter()
        .chain(stand_in_conditions(&dialect))
        .chain(given.forced.iter().filter_map(|(k, v)| match k.as_str() {
            "stall" => Some((recorder::STALL, v.clone())),
            "exit2" => Some((recorder::EXIT2, v.clone())),
            _ => None,
        }))
        .map(|(k, v)| (k.to_owned(), v)),
    );
    let size = purlis_core::engine::Size {
        columns: 160,
        rows: 50,
    };
    let spec = purlis_core::chatspec::from_launch_here(&launch, 1, size).expect("the spec");
    let engine = purlis_core::engine::AlacrittyEngine::new(size, 2000);
    let session = purlis_core::session::Session::spawn(spec, Box::new(engine))
        .expect("the fake starts in its terminal");

    // Until the script's last turn is played: every scripted step has a result, or Esc was
    // pressed on the step it names.
    let steps = steps_of(&first);
    let turns = first["turns"].as_array().cloned().unwrap_or_default();
    // Each turn after the first is typed once the one before it is played, as a person types
    // it (T1.1-03's driver types its covering script's second turn so).
    let mut typed = 1usize;
    let mut allowed = 0usize;
    let started = Instant::now();
    loop {
        let screen = session.screen().lines.join("\n");
        if given.allows_asks
            && screen.contains("Do you want to")
            && settled(&recorder_out, "permissionrequest")
            && allowed < heard(&recorder_out, "notification")
        {
            std::thread::sleep(Duration::from_millis(500));
            session.write(b"\r").expect("Allow once is chosen");
            allowed += 1;
            continue;
        }
        let done = read_jsonl(&transcript);
        if started.elapsed() >= AT_MOST {
            let heard: Vec<String> = recorder::read(&recorder_out)
                .unwrap_or_default()
                .iter()
                .map(|l| format!("{} {}", l.phase, l.argv.join(" ")))
                .collect();
            panic!(
                "the fake did not finish its script within {AT_MOST:?}, {typed} turn(s) typed; \
                 the steps with a result: {:?}; the hooks heard: {heard:#?}; its screen:\n{screen}",
                done.iter()
                    .filter_map(|l| l["step"].as_str())
                    .collect::<Vec<_>>()
            );
        }
        if let Some(step) = &given.esc_step
            && heard_guard_of(&recorder_out, step, &first)
        {
            std::thread::sleep(Duration::from_secs(2));
            session.write(b"\x1b").expect("Esc is pressed");
            std::thread::sleep(Duration::from_secs(4));
            break;
        }
        if typed < turns.len()
            && steps_of(&json!({"turns": turns[..typed]}))
                .iter()
                .all(|s| done.iter().any(|l| l["step"] == s.as_str()))
        {
            std::thread::sleep(Duration::from_secs(3));
            let prompt = turns[typed]["prompt"].as_str().unwrap_or_default();
            session
                .write(prompt.as_bytes())
                .expect("the next prompt is typed");
            std::thread::sleep(Duration::from_millis(800));
            session.write(b"\r").expect("Enter is pressed");
            typed += 1;
            continue;
        }
        let answered = steps
            .iter()
            .filter(|s| Some(*s) != given.esc_step.as_ref())
            .all(|s| done.iter().any(|l| l["step"] == s.as_str()));
        if answered && given.esc_step.is_none() {
            std::thread::sleep(Duration::from_secs(3));
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let screen = session.screen().lines.join("\n");
    // Claude Code ended with its own `/exit`, as T1.1-03's driver ends it, then Enter on the
    // default of what it asks about its background work; the others with Ctrl-C.
    if dialect == "claude" {
        std::thread::sleep(Duration::from_secs(2));
        let _ = session.write(b"/exit");
        std::thread::sleep(Duration::from_millis(800));
        let _ = session.write(b"\r");
        let _ = session.wait(Duration::from_secs(20));
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
    session.end();

    let lines = if given.recorded {
        recorder::read(&recorder_out)
            .unwrap_or_else(|e| panic!("the recorder wrote nothing ({e}); the screen:\n{screen}"))
    } else {
        Vec::new()
    };
    let mut roots = BTreeMap::new();
    let mut root_of = |name: &str, path: &Path| {
        roots.insert(name.to_owned(), path.display().to_string());
    };
    root_of("work", &root);
    root_of("outside", &outside);
    root_of("kit", &kit_dir);
    root_of("base", &base_path);
    root_of("config-home", Path::new(&var("PURLIS_CONFIG_HOME")));
    root_of("data-home", Path::new(&var("PURLIS_DATA_HOME")));
    root_of("profile", &profile_config);
    root_of("purlis", purlis.parent().expect("purlis's folder"));
    root_of("recorder", recorder_bin.parent().expect("its folder"));
    root_of("plugin", &plugin);
    // Where the fake and `purlis` are built into one folder, the hook command's path is masked as
    // `purlis`'s, as in a recording, where Claude Code's own folder is another.
    if fake.parent() != purlis.parent() {
        root_of("harness", fake.parent().expect("its folder"));
    }
    root_of("repo", &repo());
    if let Some(tmp) = &chat_tmp {
        root_of("chat-tmp", tmp);
    }
    if let Ok(home) = std::env::var("HOME") {
        root_of("home", Path::new(&home));
    }
    roots.insert("refused-target".to_owned(), refused_target.clone());
    let dialled = arrived.lock().expect("arrivals").clone();
    assert!(
        dialled.is_empty(),
        "ESCAPE: a dial arrived at {refused_target}, which the chat was never granted: {dialled:?}"
    );
    let played = Played {
        _base: base,
        dialect,
        lines,
        transcript: read_jsonl(&transcript),
        roots,
        outside,
        root,
        screen,
    };
    evidence(&played, &given.script);
    played
}

/// How many calls of `word` the recorder heard.
fn heard(out: &Path, word: &str) -> usize {
    recorder::read(out).map_or(0, |lines| {
        lines
            .iter()
            .filter(|l| l.phase == "call" && l.argv.get(1).is_some_and(|w| w == word))
            .count()
    })
}

/// Whether every call of `word` the recorder heard has its reply, and there was one.
fn settled(out: &Path, word: &str) -> bool {
    recorder::read(out).is_ok_and(|lines| {
        let calls: Vec<_> = lines
            .iter()
            .filter(|l| l.phase == "call" && l.argv.get(1).is_some_and(|w| w == word))
            .collect();
        !calls.is_empty()
            && calls
                .iter()
                .all(|call| lines.iter().any(|l| l.phase == "reply" && l.id == call.id))
    })
}

/// Each step a script names, in order, sub-agents' own among them.
fn steps_of(script: &Value) -> Vec<String> {
    fn walk(calls: &Value, out: &mut Vec<String>) {
        for call in calls.as_array().into_iter().flatten() {
            if let Some(step) = call["step"].as_str() {
                out.push(step.to_owned());
            }
            walk(&call["calls"], out);
        }
    }
    let mut out = Vec::new();
    for turn in script["turns"].as_array().into_iter().flatten() {
        walk(&turn["calls"], &mut out);
    }
    out
}

/// Whether the recorder heard the guard of `step`'s call (its command or its path in a
/// PreToolUse's input).
fn heard_guard_of(out: &Path, step: &str, script: &Value) -> bool {
    let Some(call) = script["turns"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|t| t["calls"].as_array().cloned().unwrap_or_default())
        .find(|c| c["step"] == step)
    else {
        return false;
    };
    // As it stands inside the hook's JSON input: a command's quotes are escaped there.
    let needle = match call["run"].as_str() {
        Some(run) => {
            let quoted = Value::from(run).to_string();
            quoted[1..quoted.len() - 1].to_owned()
        }
        None => call["input"].to_string(),
    };
    recorder::read(out).is_ok_and(|lines| {
        lines.iter().any(|l| {
            l.phase == "call"
                && l.argv.get(1).is_some_and(|w| w.starts_with("pretooluse"))
                && l.stdin.as_deref().is_some_and(|s| s.contains(&needle))
        })
    })
}

fn read_jsonl(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("the repo")
}

fn evidence(played: &Played, script: &Path) {
    let Ok(out) = std::env::var("FAKE_WRAP_OUT") else {
        return;
    };
    let name = script
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("script");
    let dir = PathBuf::from(out).join(&played.dialect).join(name);
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("screen.txt"), &played.screen);
    let _ = fs::write(
        dir.join("transcript.json"),
        serde_json::to_string_pretty(&played.transcript).unwrap_or_default(),
    );
}

/// The checked-in recording and turn script of the dialect at its pinned version: the
/// covering one (`pass` empty), or the pass `pass` names (`asks-recording.json`).
fn recorded(dialect: &str, pass: &str) -> (PathBuf, Recording) {
    let version = recording::pinned_version(dialect).expect("a pinned version");
    let folder = recording::folder(dialect, &version);
    let prefix = if pass.is_empty() {
        String::new()
    } else {
        format!("{pass}-")
    };
    let text = fs::read_to_string(folder.join(format!("{prefix}recording.json")))
        .unwrap_or_else(|e| panic!("{dialect} {version} has no {prefix}recording.json: {e}"));
    let rec: Recording = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{dialect} {version}'s recording does not parse: {e}"));
    (folder.join(format!("{prefix}turns.json")), rec)
}

/// The permission mode `checked` was recorded in: a recording that says none was Claude
/// Code's covering one, made with its own prompts bypassed.
fn mode_of(checked: &Recording) -> &'static str {
    match checked
        .conditions
        .get("permission_mode")
        .map(String::as_str)
    {
        Some("default") => "default",
        Some(BYPASS) | None => BYPASS,
        Some(other) => panic!("a recording made in permission mode {other}, which no run plays"),
    }
}

#[test]
#[ignore = "binds a hook socket and confines: CI's fake-harness wrap job runs it on Linux"]
fn the_fake_harness_reproduces_each_recorded_harness_session() {
    let dialect = var("FAKE_WRAP_DIALECT");
    let (turns, checked) = recorded(&dialect, "");
    let esc_step = checked
        .forced
        .contains_key("esc")
        .then(|| {
            checked
                .model
                .last()
                .and_then(|a| a.calls.first())
                .map(|c| c.step.clone())
        })
        .flatten();
    let played = play(&Given {
        script: turns,
        forced: checked.forced.clone(),
        esc_step,
        recorded: true,
        permission_mode: mode_of(&checked),
        allows_asks: false,
    });
    reproduces(&dialect, &checked, &played, "");
}

#[test]
#[ignore = "binds a hook socket and confines: CI's fake-harness wrap job runs it on Linux"]
fn the_fake_claude_code_reproduces_its_recorded_permission_ask() {
    let dialect = var("FAKE_WRAP_DIALECT");
    assert_eq!(dialect, "claude", "the asks pass is Claude Code's alone");
    let (turns, checked) = recorded(&dialect, "asks");
    assert_eq!(mode_of(&checked), "default", "{:?}", checked.conditions);
    let played = play(&Given {
        script: turns,
        forced: checked.forced.clone(),
        esc_step: None,
        recorded: true,
        permission_mode: mode_of(&checked),
        allows_asks: true,
    });
    reproduces(&dialect, &checked, &played, "asks-");
}

/// The fake's run against the recording it plays: its hook calls and what the model was shown,
/// masked as T1.1-03 masks a recording, the same.
fn reproduces(dialect: &str, checked: &Recording, played: &Played, prefix: &str) {
    // The fake's recording: its hook calls as the recorder wrote them, grouped by the parallel
    // responses of the checked-in recording, and what its transcript says the model was shown.
    let by_step: BTreeMap<String, String> = played
        .transcript
        .iter()
        .filter_map(|l| {
            Some((
                l["step"].as_str()?.to_owned(),
                l["tool_use_id"].as_str()?.to_owned(),
            ))
        })
        .collect();
    // Each call's id the fake's, by its step, a sub-agent's own calls among them.
    let mut model_json = serde_json::to_value(&checked.model).expect("JSON");
    with_fake_ids(&mut model_json, &by_step);
    let model: Vec<purlis_recordings::recording::Asked> =
        serde_json::from_value(model_json).expect("the model, with the fake's ids");
    let mut shown = Vec::new();
    purlis_recordings::projection::extend_once(
        &mut shown,
        played
            .transcript
            .iter()
            .map(|l| purlis_recordings::projection::Shown {
                kind: l["kind"].as_str().unwrap_or_default().to_owned(),
                text: l["text"].as_str().unwrap_or_default().to_owned(),
                tool_use_id: l["tool_use_id"].as_str().map(str::to_owned),
                is_error: l["is_error"].as_bool(),
            })
            .collect(),
    );
    let fake = Recording {
        roots: played.roots.clone(),
        hook_calls: recording::hook_calls(&played.lines, &model),
        model,
        shown,
        ..checked.clone()
    };
    let fresh = recording::masked(&fake);
    let mut want = serde_json::to_value(checked).expect("JSON");
    let mut fresh = fresh;
    for recording in [&mut want, &mut fresh] {
        without_rig_variables(recording);
    }
    without_harness_reminders(&mut want);
    let mut differences = Vec::new();
    for part in ["hook_calls", "shown"] {
        for d in purlis_recordings::mask::differences(&want[part], &fresh[part]) {
            differences.push(format!("{part}{d}"));
        }
    }
    if let Ok(out) = std::env::var("FAKE_WRAP_OUT") {
        let _ = fs::write(
            PathBuf::from(out).join(format!("{dialect}-{prefix}fake-recording.json")),
            serde_json::to_string_pretty(&fresh).unwrap_or_default(),
        );
    }
    assert!(
        differences.is_empty(),
        "the fake's {dialect} differs from the recording of {} {}, masked:\n{}",
        checked.harness,
        checked.version,
        differences.join("\n")
    );
}

#[test]
#[ignore = "binds a hook socket and confines: CI's fake-harness wrap job runs it on Linux"]
fn a_native_write_outside_the_allowed_folders_is_refused_by_the_wrap_not_the_script() {
    let base = tempfile::tempdir().expect("a folder");
    let script = base.path().join("native-write.json");
    fs::write(
        &script,
        json!({"version": version(), "turns": [{"prompt": "write outside", "calls": [
            {"step": "native-write", "tool": "write", "input": {"path": "<outside>/by-fake.txt", "content": "by-fake"}},
            {"step": "native-edit", "tool": "edit", "input": {"path": "<outside>/existing.txt", "old": "before", "new": "after"}},
            {"step": "control-write", "tool": "write", "input": {"path": "<work>/inside.txt", "content": "inside"}},
        ]}]})
        .to_string(),
    )
    .expect("the script");
    let played = play(&Given {
        script,
        forced: BTreeMap::new(),
        esc_step: None,
        recorded: true,
        permission_mode: BYPASS,
        allows_asks: false,
    });
    assert!(
        played.outside.join("existing.txt").is_file(),
        "the folder outside and its file are there, so a refusal cannot be a missing path"
    );
    // ESCAPE if either happened (Q301): never pinned, filed as a p0.
    assert!(
        !played.outside.join("by-fake.txt").exists(),
        "ESCAPE: the native write outside happened under the wrap"
    );
    assert_eq!(
        fs::read_to_string(played.outside.join("existing.txt"))
            .ok()
            .as_deref(),
        Some("before\n"),
        "ESCAPE: the native edit outside happened under the wrap"
    );
    assert_eq!(
        fs::read_to_string(played.root.join("inside.txt"))
            .ok()
            .as_deref(),
        Some(if played.dialect == "codex" {
            "inside\n"
        } else {
            "inside"
        }),
        "the positive control: a native write in the chat's folder is done"
    );
    for step in ["native-write", "native-edit"] {
        let result = played
            .transcript
            .iter()
            .find(|l| l["step"] == step)
            .unwrap_or_else(|| panic!("no result for {step}: {:#?}", played.transcript));
        // The wrap refused it: no hook refused it, no rule held it, and the I/O was tried.
        assert!(
            result.get("reason").is_none(),
            "a hook refused {step}: {result}"
        );
        let text = result["text"].as_str().unwrap_or_default();
        assert!(
            !text.contains("denied by your permission settings"),
            "a deny rule, not the wrap, held {step}: {text}"
        );
        // What the OS said, whatever the dialect shows the model: the wrap's refusal, and not
        // a missing folder or file (the outside folder and its file are there, above).
        assert!(
            ["EACCES", "EPERM", "EROFS"].contains(&result["os_error"].as_str().unwrap_or_default()),
            "{step} was not refused by the OS: {result}"
        );
    }
}

#[test]
#[ignore = "binds a hook socket and confines: CI's fake-harness wrap job runs it on Linux"]
fn an_mcp_call_reaches_purlis_mcp_tool_under_the_wrap() {
    let base = tempfile::tempdir().expect("a folder");
    let script = base.path().join("mcp-call.json");
    fs::write(
        &script,
        json!({"version": version(), "turns": [{"prompt": "list purlis's queries", "calls": [
            {"step": "mcp", "tool": "mcp", "input": {"name": "mcp__purlis__queries", "arguments": {}}},
        ]}]})
        .to_string(),
    )
    .expect("the script");
    let played = play(&Given {
        script,
        forced: BTreeMap::new(),
        esc_step: None,
        // purlis itself, as a person's chat has it: the recorder's own variable is not among
        // those a harness hands its MCP server.
        recorded: false,
        permission_mode: BYPASS,
        allows_asks: false,
    });
    let result = played
        .transcript
        .iter()
        .find(|l| l["step"] == "mcp")
        .unwrap_or_else(|| panic!("no result for the MCP call: {:#?}", played.transcript));
    // purlis's own tool, answered: the catalogue it lists, and not an error of any kind.
    assert_ne!(result["is_error"], true, "{result}");
    let answered: Value = serde_json::from_str(result["text"].as_str().unwrap_or_default())
        .unwrap_or_else(|e| {
            panic!("purlis's queries tool did not answer its catalogue ({e}): {result}")
        });
    assert_eq!(answered, purlis_core::catalogue::listing(), "{result}");
}

/// What every recording was made under that points a harness at its stand-in model
/// (`tests/recordings/tests/record.rs`, `model_env`): conditions, not behaviour, so the fake is
/// started under them too. It calls no model; no value here is a credential.
fn stand_in_conditions(dialect: &str) -> Vec<(&'static str, String)> {
    const KEY: &str = "stand-in-model-no-credential";
    match dialect {
        "claude" => vec![
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:9".to_owned()),
            ("ANTHROPIC_API_KEY", KEY.to_owned()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_owned()),
            (
                "CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS",
                "10000".to_owned(),
            ),
        ],
        "codex" => vec![("OPENAI_API_KEY", KEY.to_owned())],
        _ => vec![
            ("OPENAI_API_KEY", KEY.to_owned()),
            ("OPENCODE_CONFIG", "/dev/null".to_owned()),
            ("OPENCODE_DISABLE_AUTOUPDATE", "1".to_owned()),
        ],
    }
}

/// The variables each test rig sets for its own run, and nothing a harness or purlis sets: the
/// recording driver's (`PURLIS_RECORD_*`, under both prefixes) and this driver's
/// (`FAKE_HARNESS_*`). Left out of every hook call's environment on both sides.
const RIG_VARIABLES: [&str; 3] = ["PURLIS_RECORD_", "CHARTER_RECORD_", "FAKE_HARNESS_"];

fn without_rig_variables(recording: &mut Value) {
    for call in recording["hook_calls"].as_array_mut().into_iter().flatten() {
        if let Some(env) = call["env"].as_object_mut() {
            env.retain(|name, _| !RIG_VARIABLES.iter().any(|p| name.starts_with(p)));
        }
    }
}

/// Claude Code's own reminders, which T1.1-03's projection keeps as `context` though no hook of
/// purlis's said them: its attribution line, its time-based "you have been quiet" nudge, and
/// its listings of deferred tools and agent types. A closed list, by how each begins.
const HARNESS_REMINDERS: [&str; 4] = [
    "Attribution for git commits and pull requests",
    "The user hasn't heard from you in a while",
    "The following deferred tools are now available",
    "Available agent types for the Agent tool",
];

/// Where Claude Code's own environment section begins, in the system message it sends a
/// SessionStart hook's context in: what comes after it is Claude Code's, not the hook's.
const CLAUDE_ENVIRONMENT: &str =
    "\n\n# Environment\nYou have been invoked in the following environment";

fn without_harness_reminders(recording: &mut Value) {
    for shown in recording["shown"].as_array_mut().into_iter().flatten() {
        if shown["kind"] == "context"
            && let Some(text) = shown["text"].as_str()
            && let Some(at) = text.find(CLAUDE_ENVIRONMENT)
        {
            shown["text"] = Value::String(text[..at].to_owned());
        }
    }
    if let Some(shown) = recording["shown"].as_array_mut() {
        shown.retain(|s| {
            s["kind"] != "context"
                || !HARNESS_REMINDERS
                    .iter()
                    .any(|r| s["text"].as_str().is_some_and(|t| t.starts_with(r)))
        });
    }
}

/// Each call with a `step` takes the fake's id for that step, at any depth.
fn with_fake_ids(value: &mut Value, by_step: &BTreeMap<String, String>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map
                .get("step")
                .and_then(Value::as_str)
                .and_then(|step| by_step.get(step))
                .cloned()
                && map.contains_key("tool_use_id")
            {
                map.insert("tool_use_id".to_owned(), Value::String(id));
            }
            for v in map.values_mut() {
                with_fake_ids(v, by_step);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| with_fake_ids(v, by_step)),
        _ => {}
    }
}

/// The pinned version of the dialect, which `--version` answers with.
fn version() -> String {
    recording::pinned_version(&var("FAKE_WRAP_DIALECT")).expect("a pinned version")
}
