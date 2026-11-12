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

use purlis_core::forge::Kind;
use purlis_core::forge::signingkeys::Checked;
use purlis_core::hookwire::CHAT_ENV;
use purlis_core::signingsetup::{self, Heard, OnForge};
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

/// The code the command's terminal showed, to compare with the window's.
fn shown_code(out: &Output) -> String {
    let text = said(out);
    let after = text
        .split("shows the code ")
        .nth(1)
        .expect("a code is shown");
    after.split('…').next().unwrap().trim().to_owned()
}

/// `config` signed in, as the window keeps its accounts (no token here: those are the window's),
/// to each of `accounts`: (forge, host, login).
fn signed_in(config: &Path, accounts: &[(&str, &str, &str)]) {
    let file = purlis_core::forge::accounts::path(config);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let listed: Vec<serde_json::Value> = accounts
        .iter()
        .enumerate()
        .map(|(n, (kind, host, login))| {
            serde_json::json!({
                "id": format!("01ACCOUNT{n}"),
                "kind": kind,
                "host": host,
                "login": login,
                "method": "pat",
                "state": "signed-in",
            })
        })
        .collect();
    std::fs::write(
        file,
        serde_json::json!({ "version": 1, "accounts": listed }).to_string(),
    )
    .unwrap();
}

fn on(kind: Kind, host: &str) -> Option<OnForge> {
    Some(OnForge {
        kind,
        host: host.to_owned(),
    })
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
            key: the_key_line(&out),
            on: on(Kind::GitHub, "github.com"),
        })]
    );
    gh_never_ran(config);
}

/// #5793: `status` asks about every signed-in account, GitLab's included, each by its forge and
/// host, and says of each whether it has this device's key.
#[test]
fn status_asks_the_window_about_each_signed_in_account_gitlabs_included() {
    let machine = a_machine();
    let config = machine.path();
    assert!(purlis(config, &["signing", "setup"], &[]).status.success());
    signed_in(
        config,
        &[
            ("github", "github.com", "octocat"),
            ("gitlab", "gitlab.example.org", "sam"),
        ],
    );
    let window = AWindow::answering(config, Said::Signing(Heard::KeyOnAccount { on: true }));
    let out = purlis(config, &["signing", "status"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(text.contains("GitHub has it as a signing key"), "{text}");
    assert!(
        text.contains("GitLab on gitlab.example.org has it as a signing key"),
        "{text}"
    );
    let key = the_key_line(&out);
    assert_eq!(
        window.asked(),
        [
            Asked::Signing(signingsetup::Asked::KeyOnAccount {
                key: key.clone(),
                on: on(Kind::GitHub, "github.com"),
            }),
            Asked::Signing(signingsetup::Asked::KeyOnAccount {
                key,
                on: on(Kind::GitLab, "gitlab.example.org"),
            }),
        ]
    );
    gh_never_ran(config);
}

/// #5793: setup for a GitLab account gives GitLab's page and *Signing* as the usage type.
#[test]
fn setup_for_a_gitlab_account_gives_gitlabs_page_and_the_signing_usage_type() {
    let machine = a_machine();
    let config = machine.path();
    signed_in(config, &[("gitlab", "gitlab.example.org", "sam")]);
    let out = purlis(config, &["signing", "setup"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(
        text.contains("https://gitlab.example.org/-/user_settings/ssh_keys"),
        "{text}"
    );
    assert!(text.contains("Usage type: Signing"), "{text}");
    assert!(!text.contains(signingsetup::NEW_KEY_PAGE), "{text}");
    assert!(!text.contains("Signing Key"), "{text}");
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

/// Q233: run in a project, `check` names it on the line to the window, whose Asks lists it there,
/// and says it waits there; run in none (the tests below), it names none.
#[test]
fn check_run_in_a_project_names_it_to_the_window_and_says_it_waits_there() {
    let machine = a_machine();
    let config = machine.path();
    let project = config.join("smart-ide");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("charter.toml"), "").unwrap();
    let window = AWindow::answering(
        config,
        Said::Signing(Heard::Checked(Checked {
            verified: true,
            reason: "valid".to_owned(),
            key: None,
        })),
    );
    let out = support::purlis()
        .args(["signing", "check", "octo/hello", "0123abc"])
        .current_dir(&project)
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
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs");
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(
        window.asked(),
        [Asked::Signing(signingsetup::Asked::Check {
            repo: "octo/hello".to_owned(),
            commit: "0123abc".to_owned(),
            code: shown_code(&out),
            on: on(Kind::GitHub, "github.com"),
            project: Some(project.canonicalize().unwrap()),
        })]
    );
    assert!(
        said(&out).contains(
            "waiting for the person: Asks (\u{270b}) in the purlis window, project smart-ide, for \
             up to 2 minutes"
        ),
        "{}",
        said(&out)
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
            key: None,
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
            commit: "0123abc".to_owned(),
            code: shown_code(&out),
            on: on(Kind::GitHub, "github.com"),
            project: None,
        })]
    );
    assert!(said(&out).contains("Confirm checking 0123abc in octo/hello in the purlis window"));
    gh_never_ran(config);
}

/// #5793: `check --host` asks the GitLab account signed in there, on the person's yes.
#[test]
fn check_on_a_gitlab_host_asks_gitlab_on_the_persons_yes() {
    let machine = a_machine();
    let config = machine.path();
    signed_in(config, &[("gitlab", "gitlab.example.org", "sam")]);
    let window = AWindow::answering(
        config,
        Said::Signing(Heard::Checked(Checked {
            verified: true,
            reason: "valid".to_owned(),
            key: None,
        })),
    );
    let out = purlis(
        config,
        &[
            "signing",
            "check",
            "--host",
            "gitlab.example.org",
            "team/billing",
            "0123abc",
        ],
        &[],
    );
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(
        window.asked(),
        [Asked::Signing(signingsetup::Asked::Check {
            repo: "team/billing".to_owned(),
            commit: "0123abc".to_owned(),
            code: shown_code(&out),
            on: on(Kind::GitLab, "gitlab.example.org"),
            project: None,
        })]
    );
    assert!(
        said(&out).contains("on GitLab on gitlab.example.org"),
        "{}",
        said(&out)
    );
    // A host with no account signed in is refused, naming it.
    let none = purlis(
        config,
        &[
            "signing",
            "check",
            "--host",
            "git.example.net",
            "a/b",
            "0123abc",
        ],
        &[],
    );
    assert_eq!(none.status.code(), Some(1), "{}", said(&none));
    assert!(said(&none).contains("git.example.net"), "{}", said(&none));
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

/// #5788, #5793: `status` names the address each account's chat commits carry, where one was
/// chosen for it in the window, with no window running.
#[test]
fn status_names_the_email_chosen_for_each_account() {
    let machine = a_machine();
    let config = machine.path();
    assert!(purlis(config, &["signing", "setup"], &[]).status.success());
    signed_in(
        config,
        &[
            ("github", "github.com", "octocat"),
            ("gitlab", "gitlab.example.org", "sam"),
        ],
    );
    signingsetup::set_commit_email_of(
        config,
        &signingsetup::AccountRef::new(Kind::GitLab, "gitlab.example.org", "sam"),
        Some("sam@example.org"),
    )
    .unwrap();
    let out = purlis(config, &["signing", "status"], &[]);
    assert!(out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(
        text.contains("on GitLab on gitlab.example.org carry sam@example.org"),
        "{text}"
    );
    assert!(!text.contains("on GitHub carry sam@example.org"), "{text}");
    gh_never_ran(config);
}
