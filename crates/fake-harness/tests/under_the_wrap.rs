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
}

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
    let transcript = base_path.join("transcript.jsonl");
    let state = base_path.join("fake-state");

    let homes = purlis_core::profilestore::Homes::here().expect("the run's homes");
    let made = purlis_core::profilestore::create_sandbox_only(&homes, "work", &dialect, 1)
        .expect("the sandbox-only profile is made");
    let profile_config = made.record.folders.config.clone();
    let plugin = repo().join("app/src-tauri/plugin");
    let kit = Kit {
        binary: Some(recorder_bin.clone()),
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
        // As the recording was made: Claude Code's own prompts bypassed, its deny rules kept.
        ready.args.extend([
            "--permission-mode".to_owned(),
            "bypassPermissions".to_owned(),
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
    let started = Instant::now();
    loop {
        let screen = session.screen().lines.join("\n");
        assert!(
            started.elapsed() < AT_MOST,
            "the fake did not finish its script within {AT_MOST:?}; its screen:\n{screen}"
        );
        let done = read_jsonl(&transcript);
        if let Some(step) = &given.esc_step
            && heard_guard_of(&recorder_out, step, &first)
        {
            std::thread::sleep(Duration::from_secs(2));
            session.write(b"\x1b").expect("Esc is pressed");
            std::thread::sleep(Duration::from_secs(4));
            break;
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
    for _ in 0..3 {
        let _ = session.write(b"\x03");
        if matches!(session.wait(Duration::from_secs(3)), Ok(Some(_))) {
            break;
        }
    }
    let _ = session.wait(Duration::from_secs(10));
    session.end();

    let lines = recorder::read(&recorder_out)
        .unwrap_or_else(|e| panic!("the recorder wrote nothing ({e}); the screen:\n{screen}"));
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
    root_of("harness", fake.parent().expect("its folder"));
    root_of("repo", &repo());
    if let Some(tmp) = &chat_tmp {
        root_of("chat-tmp", tmp);
    }
    if let Ok(home) = std::env::var("HOME") {
        root_of("home", Path::new(&home));
    }
    let played = Played {
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
    let needle = call["run"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| call["input"].to_string());
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

/// The checked-in recording and turn script of the dialect at its pinned version.
fn recorded(dialect: &str) -> (PathBuf, Recording) {
    let version = recording::pinned_version(dialect).expect("a pinned version");
    let folder = recording::folder(dialect, &version);
    let text = fs::read_to_string(folder.join("recording.json")).expect("the recording");
    let rec: Recording = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{dialect} {version}'s recording does not parse: {e}"));
    (folder.join("turns.json"), rec)
}

#[test]
#[ignore = "binds a hook socket and confines: CI's fake-harness wrap job runs it on Linux"]
fn the_fake_harness_reproduces_each_recorded_harness_session() {
    let dialect = var("FAKE_WRAP_DIALECT");
    let (turns, checked) = recorded(&dialect);
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
    });

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
    let mut model = checked.model.clone();
    for asked in &mut model {
        for call in &mut asked.calls {
            if let Some(id) = by_step.get(&call.step) {
                call.tool_use_id.clone_from(id);
            }
        }
    }
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
    let want = serde_json::to_value(&checked).expect("JSON");
    let mut differences = Vec::new();
    for part in ["hook_calls", "shown"] {
        for d in purlis_recordings::mask::differences(&want[part], &fresh[part]) {
            differences.push(format!("{part}{d}"));
        }
    }
    if let Ok(out) = std::env::var("FAKE_WRAP_OUT") {
        let _ = fs::write(
            PathBuf::from(out).join(format!("{dialect}-fake-recording.json")),
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
    });
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
        assert!(
            [
                "EACCES",
                "EPERM",
                "EROFS",
                "PermissionDenied",
                "Failed to write"
            ]
            .iter()
            .any(|w| text.contains(w)),
            "{step} was not refused by the OS: {text}"
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
        json!({"version": version(), "turns": [{"prompt": "use purlis's browser", "calls": [
            {"step": "mcp", "tool": "mcp", "input": {"name": "mcp__purlis__browser_navigate", "arguments": {"url": "http://127.0.0.1:9/"}}},
        ]}]})
        .to_string(),
    )
    .expect("the script");
    let played = play(&Given {
        script,
        forced: BTreeMap::new(),
        esc_step: None,
    });
    let result = played
        .transcript
        .iter()
        .find(|l| l["step"] == "mcp")
        .unwrap_or_else(|| panic!("no result for the MCP call: {:#?}", played.transcript));
    let text = result["text"].as_str().unwrap_or_default();
    // An answer from purlis's own server, whatever it says of the browser: not the fake's.
    for fake_said in [
        "no MCP server named",
        "cannot start the MCP server",
        "did not answer",
        "unsupported call",
    ] {
        assert!(
            !text.contains(fake_said),
            "the call never reached purlis's server: {text}"
        );
    }
    assert!(!text.is_empty(), "purlis's server answered nothing");
}

/// The pinned version of the dialect, which `--version` answers with.
fn version() -> String {
    recording::pinned_version(&var("FAKE_WRAP_DIALECT")).expect("a pinned version")
}
