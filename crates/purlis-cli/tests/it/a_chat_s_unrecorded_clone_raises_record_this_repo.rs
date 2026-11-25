// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A3, Record this repo** (T1.1-56, R-328), through the production wiring: a chat calls the
//! `forge_milestones` tool of its own `purlis mcp` for a clone the project records no repo for.
//! The MCP process holds no window, so it hands the refusal to the app over the chat's hook
//! socket (`Ask::WindowOnly`), and the chat is told what the app answered: that the person was
//! asked, that nobody was (no window listens), that the app and this purlis are different
//! builds, or that no app was there.
//!
//! The app here is a listener that answers as the real one does once it has looked the chat up
//! (`app/src-tauri/src/windowonlyasks.rs`): by `purlis_core::windowonly::ask_to_record`, for chat
//! 3 in `alpha`. Every project runs under `[policy] template = "strict"`, set explicitly (Q308).
#![cfg(unix)]

use crate::support;

use std::io::{BufRead, BufReader, Write as _};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use purlis_core::enforcement::{Resolved, Settings};
use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV, WindowOnlyRefusal,
};
use purlis_core::sandbox::policy::Locks;
use purlis_core::windowonly::{self, Control};

fn git(dir: &std::path::Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command.arg("-C").arg(dir).args(args);
    assert!(command.output().unwrap().status.success(), "git {args:?}");
}

/// A project, strict, with workspace `alpha` holding the clone `api` of github.com/acme/api,
/// and no inventory: nothing records its repo.
fn a_project() -> tempfile::TempDir {
    a_project_with_folder("api")
}

/// As [`a_project`], the clone's folder named `folder`: the record is keyed by the origin's bare
/// name (`api`), so a folder named otherwise is not one Record this repo records.
fn a_project_with_folder(folder: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    let clone = root.join(format!("workspaces/alpha/repos/{folder}"));
    std::fs::create_dir_all(&clone).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .unwrap();
    git(&clone, &["init", "-q"]);
    git(
        &clone,
        &["remote", "add", "origin", "https://github.com/acme/api.git"],
    );
    // purlis cloned it: the config home records it at this address.
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    purlis_core::forge::gitcredential::remote::record_for_a_test(
        &home,
        &clone,
        "github.com/acme/api",
    )
    .unwrap();
    dir
}

fn root(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("project")
}

/// The app, answering a `WindowOnly` ask with `answer` of the project, and keeping what it was
/// asked.
struct App {
    _reading: hookwire::Reading,
    env: Vec<(String, String)>,
    asked: Arc<Mutex<Vec<Ask>>>,
}

fn an_app(
    tmp: &tempfile::TempDir,
    answer: impl Fn(&std::path::Path, &Ask) -> Answer + Send + Sync + 'static,
) -> App {
    let socket = tmp.path().join("app").join("hooks.sock");
    let listener = Listener::bind(tmp.path(), &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let project = root(tmp);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&asked);
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            kept.lock().unwrap().push(ask.clone());
            answer(&project, &ask)
        }),
    );
    App {
        _reading: reading,
        env: vec![
            (SOCKET_ENV.to_owned(), socket.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (TOKEN_ENV.to_owned(), token.expose().to_owned()),
            (
                purlis_core::active::WORKSPACE_ENV.to_owned(),
                "alpha".to_owned(),
            ),
        ],
        asked,
    }
}

/// What the real app answers a `WindowOnly` ask: `ask_to_record` for chat 3's workspace.
fn as_the_app(root: &std::path::Path, ask: &Ask) -> Answer {
    // The chat's own `purlis` asks the app how the project's policy runs before it decides
    // anything, and the real app answers from the project's `[policy]`: strict, which enforces
    // `guards.forge`, the refusal Record this repo follows.
    if matches!(ask, Ask::Policy(_)) {
        let settings = Settings::from_text(Some("[policy]\ntemplate = \"strict\"\n"));
        return Answer::Policy {
            spelled: Resolved::of(&settings, &Locks::default()).spelled(),
        };
    }
    let Ask::WindowOnly(asked) = ask else {
        return Answer::No {
            why: "not a window-only ask".to_owned(),
        };
    };
    let WindowOnlyRefusal::RecordRepo { clone } = &asked.refused;
    match windowonly::ask_to_record(
        &root.parent().unwrap().join("home"),
        root,
        asked.chat,
        "alpha",
        clone,
        windowonly::Reach::default(),
    ) {
        Ok(why) | Err(why) => Answer::No { why },
    }
}

/// `purlis mcp` in chat 3, initialized, and `forge_milestones` called once for the clone `api`:
/// the tool's text.
fn forge_milestones_over_mcp(tmp: &tempfile::TempDir, env: &[(String, String)]) -> String {
    forge_milestones_of_over_mcp(tmp, env, "api")
}

/// As [`forge_milestones_over_mcp`], for the clone folder `repo`.
fn forge_milestones_of_over_mcp(
    tmp: &tempfile::TempDir,
    env: &[(String, String)],
    repo: &str,
) -> String {
    let root = root(tmp);
    let mut child = support::purlis()
        .arg("mcp")
        .current_dir(root.join("workspaces/alpha"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHARTER_ROOT", &root)
        .env("HOME", tmp.path().join("home"))
        .env("TMPDIR", std::env::temp_dir())
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("purlis mcp starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut read = || {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).expect("JSON-RPC")
    };
    for message in [
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "test-harness", "version": "1" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "forge_milestones", "arguments": { "repo": repo } } }),
    ] {
        writeln!(stdin, "{message}").unwrap();
    }
    stdin.flush().unwrap();
    let _initialized = read();
    let called = read();
    let _ = child.kill();
    let _ = child.wait();
    called["result"].to_string()
}

#[test]
fn a_chat_s_unrecorded_clone_raises_record_this_repo_and_the_chat_is_told_the_person_was_asked() {
    let tmp = a_project();
    let root = root(&tmp);
    let home = tmp.path().join("home");
    // A window listens for the project.
    windowonly::telling(&root, Arc::new(|| {}));
    let app = an_app(&tmp, as_the_app);

    let said = forge_milestones_over_mcp(&tmp, &app.env);

    assert!(said.contains("purlis asked the person in Asks"), "{said}");
    // The app was handed the clone's name and the chat's number, and no workspace.
    let asked: Vec<Ask> = app
        .asked
        .lock()
        .unwrap()
        .iter()
        .filter(|ask| matches!(ask, Ask::WindowOnly(_)))
        .cloned()
        .collect();
    assert!(
        matches!(
            asked.as_slice(),
            [Ask::WindowOnly(asked)]
                if asked.chat == 3
                    && asked.refused == WindowOnlyRefusal::RecordRepo { clone: "api".to_owned() }
        ),
        "{asked:?}"
    );
    // One ask for the chat, with the press that records exactly the address shown.
    let raised = windowonly::every(&root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 3);
    let Control::RecordRepo { clone, place, .. } = &raised[0].control else {
        panic!("{:?}", raised[0].control);
    };
    assert_eq!(
        (clone.as_str(), place.as_str()),
        ("api", "github.com/acme/api")
    );
    // Nothing was recorded by asking.
    assert!(!root.join("inventory/repos.json").exists());

    // The person's press records it, and clears the ask.
    windowonly::record_this_repo(&home, &root, &raised[0].control, &[]).unwrap();
    assert_eq!(windowonly::every(&root), []);
    assert!(root.join("inventory/repos.json").exists());
    windowonly::project_ended(&root);
}

#[test]
fn where_no_window_listens_the_chat_is_told_nobody_was_asked_and_the_way_out() {
    let tmp = a_project();
    let root = root(&tmp);
    let app = an_app(&tmp, as_the_app);

    let said = forge_milestones_over_mcp(&tmp, &app.env);

    assert!(said.contains("Nobody was asked"), "{said}");
    assert!(!said.contains("asked the person in Asks"), "{said}");
    assert!(said.contains("purlis discover"), "{said}");
    windowonly::project_ended(&root);
}

#[test]
fn an_app_older_than_the_ask_is_told_apart_from_a_refusal_with_restart_words() {
    let tmp = a_project();
    let app = an_app(&tmp, |_, _| Answer::No {
        why: hookwire::UNKNOWN_LINE.to_owned(),
    });

    let said = forge_milestones_over_mcp(&tmp, &app.env);

    assert!(said.contains("different builds"), "{said}");
    assert!(said.contains("restart purlis"), "{said}");
    assert!(!said.contains("could not read this line"), "{said}");
}

#[test]
fn with_no_app_at_all_the_chat_is_told_nobody_was_asked() {
    let tmp = a_project();
    let env = vec![
        (
            SOCKET_ENV.to_owned(),
            tmp.path().join("gone.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (
            purlis_core::active::WORKSPACE_ENV.to_owned(),
            "alpha".to_owned(),
        ),
    ];

    let said = forge_milestones_over_mcp(&tmp, &env);

    assert!(said.contains("Nobody was asked"), "{said}");
    assert!(said.contains("purlis discover"), "{said}");
}

#[test]
fn a_clone_in_a_folder_named_other_than_its_origin_s_repo_is_refused_with_the_discover_way_out() {
    // The record is keyed by the origin's bare name (`api`); the folder `service` is not it.
    let tmp = a_project_with_folder("service");
    let root = root(&tmp);
    windowonly::telling(&root, Arc::new(|| {}));
    let app = an_app(&tmp, as_the_app);

    let said = forge_milestones_of_over_mcp(&tmp, &app.env, "service");

    assert!(
        said.contains("named other than its origin's repo"),
        "{said}"
    );
    assert!(said.contains("purlis discover"), "{said}");
    assert!(!said.contains("asked the person in Asks"), "{said}");
    assert_eq!(windowonly::every(&root), []);
    windowonly::project_ended(&root);
}
