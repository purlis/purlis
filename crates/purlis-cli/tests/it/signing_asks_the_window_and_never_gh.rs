// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis signing`** (#5625, ADR 0116, the person's ruling of 2026-11-07): the command holds no
//! forge sign-in and never runs `gh`. It makes the key in this machine's keychain (the fenced
//! file keychain here), asks the running window whether GitHub has it or shows a commit as
//! Verified, and gives the by-hand steps where no window answers. A `gh` first on `PATH` writes
//! down that it ran, and never may. In a chat, setup refuses.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::{Arc, Mutex};

use purlis_core::forge::signingkeys::Checked;
use purlis_core::hookwire::CHAT_ENV;
use purlis_core::signingsetup::{self, Heard};
use purlis_core::windowline::{self, Asked, Said};

/// A config home with a `gh` in `bin` that writes `gh-ran` if it is ever run.
fn a_machine() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    stand_in::program(
        &bin,
        "gh",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
            dir.path().join("gh-ran").display()
        ),
    );
    dir
}

fn purlis(config: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    support::purlis()
        .args(args)
        .current_dir(config)
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", config.join("bin").display()),
        )
        .env("HOME", config)
        .env("PURLIS_CONFIG_HOME", config)
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .envs(std::env::var_os("XDG_RUNTIME_DIR").map(|dir| ("XDG_RUNTIME_DIR", dir)))
        .envs(env.iter().copied())
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

fn gh_never_ran(config: &Path) {
    assert!(
        !config.join("gh-ran").exists(),
        "gh ran: {}",
        std::fs::read_to_string(config.join("gh-ran")).unwrap_or_default()
    );
}

/// The key's public line, as `status` prints it alone on a line.
fn the_key_line(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|line| line.starts_with("ssh-ed25519 "))
        .expect("the public line is printed")
        .to_owned()
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

#[test]
fn setup_in_a_chat_is_refused_and_makes_no_key() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(config, &["signing", "setup"], &[(CHAT_ENV, "01CHAT")]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(said(&out).contains("your own terminal"), "{}", said(&out));
    let status = purlis(config, &["signing", "status"], &[]);
    assert!(
        said(&status).contains("has no signing key"),
        "{}",
        said(&status)
    );
    gh_never_ran(config);
}

#[test]
fn setup_with_no_window_makes_the_key_and_gives_the_steps_by_hand_without_gh() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(config, &["signing", "setup"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(text.contains("a new key is made"), "{text}");
    assert!(text.contains("No purlis window is running"), "{text}");
    assert!(text.contains(signingsetup::NEW_KEY_PAGE), "{text}");
    assert!(text.contains("Title: purlis · "), "{text}");
    assert!(text.contains("Signing Key"), "{text}");
    gh_never_ran(config);

    let status = purlis(config, &["signing", "status"], &[]);
    assert!(
        said(&status).contains("this machine signs the commits chats make"),
        "{}",
        said(&status)
    );
    gh_never_ran(config);
}

#[test]
fn status_asks_the_window_whether_github_has_this_devices_key() {
    let machine = a_machine();
    let config = machine.path();
    assert!(purlis(config, &["signing", "setup"], &[]).status.success());
    let window = AWindow::answering(config, Said::Signing(Heard::KeyOnAccount { on: true }));
    let out = purlis(config, &["signing", "status"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("GitHub has it as a signing key"),
        "{}",
        said(&out)
    );
    assert_eq!(
        window.asked(),
        [Asked::Signing(signingsetup::Asked::KeyOnAccount {
            key: the_key_line(&out)
        })]
    );
    gh_never_ran(config);
}

#[test]
fn a_window_whose_sign_in_may_not_read_signing_keys_says_github_was_not_asked() {
    let machine = a_machine();
    let config = machine.path();
    assert!(purlis(config, &["signing", "setup"], &[]).status.success());
    let _window = AWindow::answering(config, Said::Signing(Heard::NeedsPermission));
    let out = purlis(config, &["signing", "status"], &[]);
    assert!(
        said(&out).contains("may not read your signing keys yet"),
        "{}",
        said(&out)
    );
    gh_never_ran(config);
}

#[test]
fn setup_goes_straight_to_its_check_where_github_has_the_key_already() {
    let machine = a_machine();
    let config = machine.path();
    let window = AWindow::answering(config, Said::Signing(Heard::KeyOnAccount { on: true }));
    let out = purlis(config, &["signing", "setup"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(text.contains("GitHub already has this key"), "{text}");
    assert!(text.contains("signed from now on"), "{text}");
    assert!(!text.contains(signingsetup::NEW_KEY_PAGE), "{text}");
    // Step 2 asks, and step 3 checks: twice, and nothing else.
    assert_eq!(window.asked().len(), 2, "{:?}", window.asked());
    gh_never_ran(config);
}

#[test]
fn remove_takes_the_key_off_this_machine_and_says_to_remove_it_from_github() {
    let machine = a_machine();
    let config = machine.path();
    assert!(purlis(config, &["signing", "setup"], &[]).status.success());
    let out = purlis(config, &["signing", "remove"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(
        said(&out).contains("Remove it from GitHub too"),
        "{}",
        said(&out)
    );
    let status = purlis(config, &["signing", "status"], &[]);
    assert!(
        said(&status).contains("has no signing key"),
        "{}",
        said(&status)
    );
    let again = purlis(config, &["signing", "remove"], &[]);
    assert!(
        said(&again).contains("nothing to remove"),
        "{}",
        said(&again)
    );
    gh_never_ran(config);
}

#[test]
fn check_asks_the_window_and_says_the_reason_in_plain_words() {
    let machine = a_machine();
    let config = machine.path();
    let window = AWindow::answering(
        config,
        Said::Signing(Heard::Checked(Checked {
            verified: false,
            reason: "unverified_email".to_owned(),
        })),
    );
    let out = purlis(config, &["signing", "check", "octo/hello", "0123abc"], &[]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("not a verified address"),
        "{}",
        said(&out)
    );
    assert_eq!(
        window.asked(),
        [Asked::Signing(signingsetup::Asked::Check {
            repo: "octo/hello".to_owned(),
            commit: "0123abc".to_owned()
        })]
    );
    gh_never_ran(config);
}

#[test]
fn check_with_no_window_says_to_open_purlis() {
    let machine = a_machine();
    let config = machine.path();
    let out = purlis(config, &["signing", "check", "octo/hello", "0123abc"], &[]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(said(&out).contains("Open purlis"), "{}", said(&out));
    gh_never_ran(config);
}
