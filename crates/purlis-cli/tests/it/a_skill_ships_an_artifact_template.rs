// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill ships an artifact template** (#5974, ADR 0122 §1, spec #5948 story 64).
//!
//! A skill keeps a page in its own folder as `artifacts/<name>/`. `purlis artifact pin <name>
//! --skill <skill>` places it in the workspace's `artifacts/<name>/`, without its data files,
//! and pins it at the person's terminal; in a chat it only proposes, naming the skill, and the
//! app places it (`pinproposals.rs`). What placing copies and refuses is proven in
//! `purlis_core::artifact::template`'s own tests.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::repocmd::Say;

/// A project with workspace alpha and a skill `backlog-sync` of alpha shipping `backlog-state`,
/// with a sample data file beside its page.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    let skill = root.join("workspaces/alpha/skills/backlog-sync");
    let template = skill.join("artifacts/backlog-state");
    std::fs::create_dir_all(&template).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: backlog-sync\ndescription: Sync the backlog.\n---\n\n# Sync\n",
    )
    .unwrap();
    std::fs::write(template.join("index.html"), "<title>Backlog state</title>").unwrap();
    std::fs::write(template.join("data.json"), "{\"sample\": true}").unwrap();
    dir
}

fn copy(root: &Path) -> PathBuf {
    root.join("workspaces/alpha/artifacts/backlog-state")
}

fn pins_file(root: &Path) -> PathBuf {
    root.join("workspaces/alpha/artifacts/pins.toml")
}

fn purlis(root: &Path, env: &[(String, String)], args: &[&str]) -> Output {
    support::purlis()
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("CHARTER_ROOT", root)
        .env("CHARTER_WORKSPACE", "alpha")
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn at_the_terminal_a_skill_s_page_is_placed_without_its_data_and_pinned() {
    let project = project();
    let root = project.path();
    let pin = [
        "artifact",
        "pin",
        "backlog-state",
        "--skill",
        "backlog-sync",
    ];
    let out = purlis(root, &[], &pin);
    assert!(out.status.success(), "{}", said(&out));
    assert!(copy(root).join("index.html").is_file(), "placed");
    assert!(
        !copy(root).join("data.json").exists(),
        "never the skill's data"
    );
    let pins = std::fs::read_to_string(pins_file(root)).expect("pinned");
    assert!(pins.contains("folder = \"backlog-state\""), "{pins}");

    // A run again leaves the same page, and the data its runs wrote, as they are.
    std::fs::write(copy(root).join("data.json"), "{\"mine\": 1}").unwrap();
    let again = purlis(root, &[], &pin);
    assert!(again.status.success(), "{}", said(&again));
    assert_eq!(
        std::fs::read_to_string(copy(root).join("data.json")).unwrap(),
        "{\"mine\": 1}"
    );
}

#[test]
fn at_the_terminal_a_skill_that_is_not_there_places_and_pins_nothing() {
    let project = project();
    let root = project.path();
    let out = purlis(
        root,
        &[],
        &["artifact", "pin", "backlog-state", "--skill", "nobody"],
    );
    assert!(!out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("no skill named 'nobody'"),
        "{}",
        said(&out)
    );
    assert!(!copy(root).exists());
    assert!(!pins_file(root).exists());
}

#[test]
fn in_a_chat_the_pin_names_its_skill_to_the_app_and_writes_nothing() {
    let project = project();
    let root = project.path();
    let socket = root.join("app").join("hooks.sock");
    let listener = Listener::bind(root, &socket).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let was_asked = Arc::clone(&asked);
    let _reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            was_asked.lock().unwrap().push(ask.clone());
            Answer::Said {
                lines: vec![Say::Done("held".into())],
                code: 0,
            }
        }),
    );
    let chat = vec![
        (SOCKET_ENV.to_owned(), socket.display().to_string()),
        (CHAT_ENV.to_owned(), "7".to_owned()),
        (TOKEN_ENV.to_owned(), token.expose().to_owned()),
    ];
    let out = purlis(
        root,
        &chat,
        &[
            "artifact",
            "pin",
            "backlog-state",
            "--skill",
            "backlog-sync",
        ],
    );
    assert!(out.status.success(), "{}", said(&out));
    let asked = asked.lock().unwrap().clone();
    let [Ask::ProposePin(proposed)] = asked.as_slice() else {
        panic!("one proposed pin: {asked:?}")
    };
    assert_eq!(proposed.pin.folder, "backlog-state");
    assert_eq!(proposed.pin.skill.as_deref(), Some("backlog-sync"));
    assert!(!copy(root).exists(), "the app places it, never the chat");
    assert!(!pins_file(root).exists());
}
