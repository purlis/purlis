// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A key chat's token reaches its harness in the process environment alone** (T1a-15, R-066):
//! a Claude Code chat on a key route is armed as its start arms it, its `--settings` put in its
//! file under `chat-settings` where it is too long for a word, and `crates/fake-harness` is
//! started in Claude Code's place with those words and that environment, in the chat's run
//! folder. It records what reached it. The token is in its environment as
//! `ANTHROPIC_AUTH_TOKEN`, and in none of its argv, the run folder or the `chat-settings` folder.

use std::path::{Path, PathBuf};
use std::process::Command;

use purlis_core::harness::{Harness, Kit, StateHooks};
use purlis_core::keyroute::{KeyRoute, Listener};

/// The run's token, as its listener hands it.
const RUN_TOKEN: &str = "run-token-canary-5d1e8b7c2a904f63";

/// The repo's fake harness, built where the workspace's binaries are when it is not there yet.
fn fake_harness() -> PathBuf {
    let dir = purlis_core::testrun::bin_dir().expect("a test build");
    let beside = dir.join(format!("fake-harness{}", std::env::consts::EXE_SUFFIX));
    if !beside.is_file() {
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--quiet",
                "-p",
                "fake-harness",
                "--bin",
                "fake-harness",
            ])
            .status()
            .expect("cargo runs");
        assert!(status.success(), "fake-harness did not build: {status}");
    }
    beside
}

/// Every file under `dir`, read as bytes.
fn every_file(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut left = vec![dir.to_path_buf()];
    while let Some(at) = left.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                left.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                found.push((path, bytes));
            }
        }
    }
    found
}

fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// R-066: the token appears in the spawned environment only; argv and the run folder carry it
/// nowhere.
#[test]
fn adapter_writes_the_token_only_into_the_process_environment() {
    let run = tempfile::tempdir().expect("a run folder");
    let data = tempfile::tempdir().expect("an app data folder");
    let records = tempfile::tempdir().expect("a folder for the harness's record");
    let chat_settings = data.path().join("chat-settings");
    let route = KeyRoute::new(
        Listener {
            port: 47_321,
            token: RUN_TOKEN.to_owned(),
        },
        Some("claude-haiku-background"),
        |_| None,
    );
    let StateHooks::ThisSessionOnly { args, env, .. } = Harness::ClaudeCode.state_hooks(
        Kit {
            binary: Path::new("/bin/purlis"),
            plugin: Some(Path::new("/app/plugin")),
            persona: None,
            skills: None,
            graded: false,
            judging: false,
            writes_its_piece: false,
            runs_a_shell: false,
            own: &[],
            laid_out: None,
            key_route: Some(&route),
        },
        Some(run.path()),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("Claude Code is armed per session");
    };
    let args = purlis_core::hoststart::settings_in_a_file(
        Some(&chat_settings),
        run.path(),
        "01JD0000000000000000000000",
        args,
    )
    .expect("its settings are kept as a start keeps them");
    let record = records.path().join("record.json");

    let status = Command::new(fake_harness())
        .args(&args)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .env("FAKE_HARNESS_RECORD", &record)
        .current_dir(run.path())
        .status()
        .expect("the fake harness runs");

    assert!(status.success(), "{status}");
    let said: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&record).expect("its record")).expect("JSON");
    assert_eq!(
        said["env"]["ANTHROPIC_AUTH_TOKEN"], RUN_TOKEN,
        "the token is in its environment"
    );
    let argv = said["argv"].as_array().expect("its argv");
    assert!(
        argv.iter()
            .all(|arg| !arg.as_str().unwrap_or_default().contains(RUN_TOKEN)),
        "the token is in argv: {argv:?}"
    );
    for (name, value) in said["env"].as_object().expect("its environment") {
        if name != "ANTHROPIC_AUTH_TOKEN" {
            assert!(
                !value.as_str().unwrap_or_default().contains(RUN_TOKEN),
                "the token is in {name} too"
            );
        }
    }
    for folder in [run.path(), data.path()] {
        let files = every_file(folder);
        for (path, bytes) in &files {
            assert!(
                !holds(bytes, RUN_TOKEN),
                "the token is in {}",
                path.display()
            );
        }
    }
    // The settings it reached were the key route's, so the scan above read what was handed.
    let settings = every_file(&chat_settings);
    assert!(
        settings
            .iter()
            .any(|(_, bytes)| holds(bytes, "http://127.0.0.1:47321")),
        "the route's settings were kept in chat-settings: {settings:?}"
    );
}
