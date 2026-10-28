// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A proposed change** (#1975, PC-2): `purlis propose <file> --from <draft>`, run in a chat
//! the app started, hands the app the draft as the whole new content of one of purlis's own
//! configuration files, worked out against the file as it is, and writes nothing. Anything but
//! purlis's own configuration, a draft outside the chat's own folder, and a run outside a chat
//! are refused with the way that works.
#![cfg(unix)]

use crate::support;

use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::sandboxblock::CHAT_DIR_ENV;

/// A project with a manifest, workspace `alpha` and persona `steward`.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(dir.path().join("home")).unwrap();
    dir
}

fn root(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("project")
}

/// The app: every ask is handed to the receiver and answered `answer`.
struct App {
    _reading: hookwire::Reading,
    token: hookwire::ChatToken,
    socket: PathBuf,
    asked: mpsc::Receiver<Ask>,
}

fn an_app(tmp: &tempfile::TempDir, answer: Answer) -> App {
    let socket = tmp.path().join("app").join("hooks.sock");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(tmp.path(), &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            answer.clone()
        }),
    );
    App {
        _reading: reading,
        token,
        socket,
        asked,
    }
}

impl App {
    /// The environment of chat 3, which the app started in `alpha`.
    fn chat_env(&self, tmp: &tempfile::TempDir) -> Vec<(String, String)> {
        vec![
            (SOCKET_ENV.to_owned(), self.socket.display().to_string()),
            (CHAT_ENV.to_owned(), "3".to_owned()),
            (TOKEN_ENV.to_owned(), self.token.expose().to_owned()),
            (
                CHAT_DIR_ENV.to_owned(),
                root(tmp).join("workspaces/alpha").display().to_string(),
            ),
        ]
    }
}

/// `purlis <args>` in `alpha`, with nothing of the caller's environment but what is named.
fn purlis(tmp: &tempfile::TempDir, args: &[&str], env: &[(String, String)]) -> Output {
    let root = root(tmp);
    support::purlis()
        .args(args)
        .current_dir(root.join("workspaces/alpha"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHARTER_ROOT", &root)
        .env("HOME", tmp.path().join("home"))
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn a_draft_in_the_chat_s_folder_is_handed_over_as_the_manifest_s_new_content() {
    let tmp = a_project();
    std::fs::write(
        root(&tmp).join("workspaces/alpha/draft.toml"),
        "schema = 1\nmode = \"pr\"\n",
    )
    .unwrap();
    let app = an_app(
        &tmp,
        Answer::Said {
            lines: vec![purlis_core::repocmd::Say::Done(
                "shown to the person".into(),
            )],
            code: 0,
        },
    );
    let out = purlis(
        &tmp,
        &["propose", "../../charter.toml", "--from", "draft.toml"],
        &app.chat_env(&tmp),
    );
    assert!(out.status.success(), "{}", said(&out));
    match app.asked.recv_timeout(Duration::from_secs(5)) {
        Ok(Ask::Propose(proposed)) => {
            assert_eq!(proposed.chat, 3);
            assert_eq!(proposed.path, "charter.toml");
            assert_eq!(
                proposed.change,
                purlis_core::proposed::Change::Whole {
                    content: "schema = 1\nmode = \"pr\"\n".to_owned()
                }
            );
            assert_eq!(
                proposed.base,
                purlis_core::proposed::base_of(Some(b"schema = 1\n"))
            );
        }
        other => panic!("the app was not handed a proposal: {other:?}"),
    }
    // Nothing was written here: only the person applies it.
    assert_eq!(
        std::fs::read_to_string(root(&tmp).join("charter.toml")).unwrap(),
        "schema = 1\n"
    );
}

#[test]
fn only_purlis_s_own_configuration_and_a_draft_of_the_chat_s_own_are_proposed() {
    let tmp = a_project();
    std::fs::write(root(&tmp).join("workspaces/alpha/draft.toml"), "x\n").unwrap();
    std::fs::write(tmp.path().join("outside.toml"), "x\n").unwrap();
    let app = an_app(
        &tmp,
        Answer::No {
            why: "never asked".to_owned(),
        },
    );
    let env = app.chat_env(&tmp);

    let not_ours = purlis(&tmp, &["propose", "notes.md", "--from", "draft.toml"], &env);
    assert!(!not_ours.status.success());
    assert!(said(&not_ours).contains("is not a file purlis lets a chat propose"));

    let outside = tmp.path().join("outside.toml").display().to_string();
    let not_its_draft = purlis(
        &tmp,
        &["propose", "../../charter.toml", "--from", &outside],
        &env,
    );
    assert!(!not_its_draft.status.success());
    assert!(said(&not_its_draft).contains("not in this chat's own folder"));
    assert!(app.asked.try_recv().is_err(), "nothing was handed over");

    let no_chat = purlis(
        &tmp,
        &["propose", "../../charter.toml", "--from", "draft.toml"],
        &[],
    );
    assert!(!no_chat.status.success());
    assert!(said(&no_chat).contains("is for a chat the app started"));
}
