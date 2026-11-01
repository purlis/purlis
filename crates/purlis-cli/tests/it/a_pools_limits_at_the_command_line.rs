// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool`** (#4284, spec #4275): the limits with the window's and the file's names and
//! words. In a chat it writes nothing: the value is handed to the app as a proposal, and a chat
//! that can't reach the app is refused.
#![cfg(unix)]

use crate::support;

use std::path::Path;
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::poollimits::LimitId;
use purlis_core::runners;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A config home keeping the Docker pool `builds`.
fn a_machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let pool = runners::Pool::new(
        "builds",
        purlis_core::dockerpool::Image::pinned(IMAGE).unwrap(),
        runners::Set::default(),
    )
    .unwrap();
    runners::record_pool(dir.path(), &pool).unwrap();
    dir
}

fn purlis(config: &Path, args: &[&str], env: &[(String, String)]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(config)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", config)
        .env("PURLIS_CONFIG_HOME", config)
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

fn set(config: &Path) -> Option<u64> {
    runners::chosen_limits_of(config, "builds")
        .unwrap()
        .get(LimitId::SignInWait)
}

#[test]
fn a_limit_is_set_listed_and_reset() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(
        config,
        &["pool", "limit", "builds", "signInWait", "30"],
        &[],
    );
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(set(config), Some(30));

    let listed = purlis(config, &["pool", "limits", "builds", "--json"], &[]);
    let json: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let rows = json["limits"].as_array().unwrap();
    let row = rows.iter().find(|row| row["key"] == "signInWait").unwrap();
    assert_eq!(row["set"], 30);
    assert_eq!(row["inForce"], 30);
    assert!(
        rows.iter().all(|row| row["key"] != "vouchWait"),
        "a Docker pool has no vouch wait"
    );

    let out = purlis(
        config,
        &["pool", "limit", "builds", "signInWait", "--reset"],
        &[],
    );
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(set(config), None);
}

/// The parity the ticket asks for: the command refuses in the check's own words, which are the
/// window's (`pool_limit_check`) and the file's (`Chosen::read_row`, tested beside it).
#[test]
fn a_bad_value_is_refused_in_the_checks_own_words_and_nothing_changes() {
    let machine = a_machine();
    let config = machine.path();
    let limit = LimitId::SignInWait.limit();
    for typed in ["0", "121", "lots", "-1", "1.5"] {
        let out = purlis(
            config,
            &["pool", "limit", "builds", "signInWait", typed],
            &[],
        );
        assert!(!out.status.success(), "{typed}");
        let why = limit.check_typed(typed, None).unwrap_err();
        assert!(said(&out).contains(&why), "{typed}: {}", said(&out));
        assert_eq!(set(config), None, "{typed}");
    }
    let unknown = purlis(config, &["pool", "limit", "builds", "nothing", "1"], &[]);
    assert!(
        said(&unknown).contains("no limit named"),
        "{}",
        said(&unknown)
    );
    let no_pool = purlis(config, &["pool", "limit", "none", "signInWait", "30"], &[]);
    assert!(
        said(&no_pool).contains("no pool named"),
        "{}",
        said(&no_pool)
    );
    let docker = purlis(config, &["pool", "limit", "builds", "vouchWait", "10"], &[]);
    let why = LimitId::VouchWait.limit().not_on_docker("builds").unwrap();
    assert!(said(&docker).contains(&why), "{}", said(&docker));
}

/// In a chat the value goes to the app, and nothing is written here.
#[test]
fn in_a_chat_the_value_is_handed_to_the_app_and_nothing_is_written() {
    let machine = a_machine();
    let config = machine.path();
    let socket = config.join("app").join("hooks.sock");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(config, &socket).unwrap();
    let token = listener.tokens().issue_to_this_process(3).unwrap();
    let _reading: hookwire::Reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done("shown".into())],
                code: 0,
            }
        }),
    );
    let env = [
        (SOCKET_ENV.to_owned(), socket.display().to_string()),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (TOKEN_ENV.to_owned(), token.expose().to_owned()),
    ];
    let out = purlis(
        config,
        &["pool", "limit", "builds", "signInWait", "30"],
        &env,
    );
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(set(config), None, "only the person applies it");
    match asked.try_recv() {
        Ok(Ask::ProposeLimit(ask)) => {
            assert_eq!(ask.chat, 3);
            assert_eq!(ask.pool, "builds");
            assert_eq!(ask.key, "signInWait");
            assert_eq!(ask.value, Some(30));
        }
        _ => panic!("the app was asked nothing"),
    }
}

/// A chat the app doesn't answer is refused: it never falls back to writing the file.
#[test]
fn a_chat_that_cannot_reach_the_app_writes_nothing() {
    let machine = a_machine();
    let config = machine.path();
    let env = [
        (
            SOCKET_ENV.to_owned(),
            config.join("none.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (TOKEN_ENV.to_owned(), "0".repeat(64)),
    ];
    let out = purlis(
        config,
        &["pool", "limit", "builds", "signInWait", "30"],
        &env,
    );
    assert!(!out.status.success(), "{}", said(&out));
    assert_eq!(set(config), None);
}
