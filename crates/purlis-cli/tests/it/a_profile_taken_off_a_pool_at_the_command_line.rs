// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool remove-profile`** (#5174, Q30): what Pool setup's Sign-ins button does to a
//! profile unticked, by hand. Only the window reaches the runner, so the command asks it, on the
//! person's yes there, and changes nothing itself.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::CHAT_ENV;
use purlis_core::profilestore::{self, Homes, MachineSettings};
use purlis_core::runners;
use purlis_core::windowline::{self, Asked, Said};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A config home keeping the Docker pool `builds`, which names `work`, and the profiles `work`
/// and `home`.
fn a_machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let pool = runners::Pool::new(
        "builds",
        purlis_core::dockerpool::Image::pinned(IMAGE).unwrap(),
        runners::Set::default(),
    )
    .unwrap();
    runners::record_pool(dir.path(), &pool).unwrap();
    let store = Homes::at(dir.path(), &dir.path().join("data"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = |_: &str| -> Option<String> { None };
    for name in ["work", "home"] {
        profilestore::create(
            &store,
            name,
            "claude",
            &MachineSettings {
                home: &home,
                env: &env,
            },
            1,
        )
        .expect("made");
    }
    runners::add_pool_profile(dir.path(), "builds", "work", None).unwrap();
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
        .envs(std::env::var_os("XDG_RUNTIME_DIR").map(|dir| ("XDG_RUNTIME_DIR", dir)))
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

fn named(config: &Path) -> Option<Vec<String>> {
    runners::profiles_of(config, "builds").unwrap()
}

/// A window on `config` that answers each ask with `answer`, and what it was asked.
struct AWindow {
    socket: PathBuf,
    asked: Arc<Mutex<Vec<Asked>>>,
}

impl AWindow {
    fn answering(config: &Path, answer: Said) -> Self {
        let socket = windowline::beside(&purlis_core::hostsocket::here_for(
            std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
            Some(config),
        ));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let heard = Arc::clone(&asked);
        windowline::serve_on(&socket, move |one| {
            heard.lock().unwrap().push(one);
            answer.clone()
        })
        .expect("it listens");
        Self { socket, asked }
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked.lock().unwrap().clone()
    }
}

impl Drop for AWindow {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

const TAKE_OFF: [&str; 4] = ["pool", "remove-profile", "builds", "work"];

#[test]
fn the_window_is_asked_to_take_it_off_and_what_it_notes_is_said() {
    let machine = a_machine();
    let config = machine.path();
    let window = AWindow::answering(
        config,
        Said::Asked {
            note: Some("its runner keeps it until it is made again".to_owned()),
        },
    );
    let out = purlis(config, &TAKE_OFF, &[]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("no longer keeps 'work'"),
        "{}",
        said(&out)
    );
    assert!(said(&out).contains("made again"), "{}", said(&out));

    let asked = window.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    let Asked::TakeOffAPool {
        name, pool, code, ..
    } = &asked[0]
    else {
        panic!("asked something else: {asked:?}");
    };
    assert_eq!((name.as_str(), pool.as_str()), ("work", "builds"));
    assert!(
        said(&out).contains(code.as_str()),
        "the terminal shows the code the window does: {}",
        said(&out)
    );
    // What the window keeps is the window's to change: the runner forgets it first.
    assert_eq!(named(config), Some(vec!["work".into()]));
}

#[test]
fn with_no_window_it_says_so_and_nothing_changes() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(config, &TAKE_OFF, &[]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("none is running"), "{}", said(&out));
    assert_eq!(named(config), Some(vec!["work".into()]));
}

#[test]
fn a_no_in_the_window_is_said_and_nothing_changes() {
    let machine = a_machine();
    let config = machine.path();
    let _window = AWindow::answering(
        config,
        Said::Refused {
            why: "it was declined in the window".to_owned(),
        },
    );
    let out = purlis(config, &TAKE_OFF, &[]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("declined"), "{}", said(&out));
    assert_eq!(named(config), Some(vec!["work".into()]));
}

#[test]
fn a_profile_the_pool_doesnt_keep_asks_the_window_nothing() {
    let machine = a_machine();
    let config = machine.path();
    let window = AWindow::answering(config, Said::Asked { note: None });
    let out = purlis(config, &["pool", "remove-profile", "builds", "home"], &[]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("doesn't keep"), "{}", said(&out));
    assert_eq!(window.asked(), Vec::new());
}

#[test]
fn in_a_chat_the_window_is_asked_nothing() {
    let machine = a_machine();
    let config = machine.path();
    let window = AWindow::answering(config, Said::Asked { note: None });
    let env = [(CHAT_ENV.to_owned(), "3".to_owned())];
    let out = purlis(config, &TAKE_OFF, &env);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("own terminal"), "{}", said(&out));
    assert_eq!(window.asked(), Vec::new());
}
