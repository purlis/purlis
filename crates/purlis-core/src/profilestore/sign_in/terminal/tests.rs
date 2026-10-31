// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::mpsc::channel;

use super::super::driven::drive_with;
use super::super::{Login, Standing};
use super::*;

const PAGE: &str = "https://claude.ai/oauth/authorize?code=true&state=recorded";

/// A terminal that writes down what it was told, in order.
#[derive(Default)]
struct Written(Vec<String>);

impl Terminal for Written {
    fn say(&mut self, line: &str) {
        self.0.push(line.to_owned());
    }

    fn open(&mut self, url: &str) {
        self.0.push(format!("opened {url}"));
    }
}

/// A stand-in for Claude Code's login, running `then` once it printed its page and prompt.
fn claude(then: &str) -> Login {
    Login {
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            format!("printf 'visit: {PAGE}\\nPaste code here if prompted > '; {then}"),
        ],
        env: Vec::new(),
        cleared: Vec::new(),
    }
}

fn followed(login: &Login, lines: &[&str]) -> (Result<(), String>, Vec<String>) {
    followed_with(login, "claude", None, lines, Input::StaysOpen)
}

/// Whether the terminal's input stays open once `lines` were typed.
#[derive(PartialEq)]
enum Input {
    StaysOpen,
    Ends,
}

fn followed_with(
    login: &Login,
    kind: &str,
    plan: Option<&str>,
    lines: &[&str],
    input: Input,
) -> (Result<(), String>, Vec<String>) {
    let driving = drive_with(login, kind, plan, None, || Standing::SignedIn).unwrap();
    let (type_in, typed) = channel();
    for line in lines {
        type_in.send((*line).to_owned()).unwrap();
    }
    let held = (input == Input::StaysOpen).then_some(type_in);
    let mut terminal = Written::default();
    let ended = follow(&driving, &typed, &mut terminal, "work", "builds");
    drop(held);
    (ended, terminal.0)
}

/// **Claude Code's sign-in, from a terminal**: the page said, then opened; the code read and
/// passed on; signed in as its status check says.
#[cfg(unix)]
#[test]
fn a_pasted_code_signs_in_as_the_strip_does() {
    let login = claude("read code; [ \"$code\" = 'abc#recorded' ] || exit 3");

    let (ended, said) = followed(&login, &["abc#recorded"]);

    assert_eq!(ended, Ok(()));
    assert_eq!(
        said,
        [
            format!("Sign in on this page: {PAGE}"),
            format!("opened {PAGE}"),
            "Paste the code the page shows, then press Return:".to_owned(),
            "Checking it is signed in on the runner…".to_owned(),
            "'work' is signed in on pool builds".to_owned(),
        ]
    );
}

/// **opencode's Copilot sign-in**: the code before the page, the page before the browser.
#[cfg(unix)]
#[test]
fn a_device_code_is_said_before_its_page_opens() {
    let login = Login {
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            "echo 'Go to: https://github.com/login/device'; echo 'Enter code: WDJB-MJHT'; \
             echo 'Login successful'"
                .to_owned(),
        ],
        env: Vec::new(),
        cleared: Vec::new(),
    };

    let (ended, said) = followed_with(
        &login,
        "opencode",
        Some("github-copilot"),
        &[],
        Input::StaysOpen,
    );

    assert_eq!(ended, Ok(()));
    assert_eq!(
        said[..3],
        [
            "Enter this code on the page: WDJB-MJHT".to_owned(),
            "The page: https://github.com/login/device".to_owned(),
            "opened https://github.com/login/device".to_owned(),
        ]
    );
}

/// **A refused code is said, and another is read**.
#[cfg(unix)]
#[test]
fn a_refused_code_is_said_and_another_read() {
    let login = claude(
        "read code; echo 'Invalid code. Try again.'; read code; \
         [ \"$code\" = 'right' ] || exit 3",
    );

    let (ended, said) = followed(&login, &["wrong", "right"]);

    assert_eq!(ended, Ok(()));
    assert!(
        said.contains(&"The sign-in refused that code: Invalid code. Try again.".to_owned()),
        "{said:?}"
    );
}

/// **What purlis does not recognise says what to do**, and is an `Err` the command exits non-zero
/// on.
#[cfg(unix)]
#[test]
fn a_page_it_does_not_know_says_what_to_do() {
    let login = Login {
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            "echo 'visit: https://evil.example/oauth/authorize'; sleep 30".to_owned(),
        ],
        env: Vec::new(),
        cleared: Vec::new(),
    };

    let (ended, said) = followed(&login, &[]);

    let why = ended.unwrap_err();
    assert!(why.contains("Open in terminal"), "{why}");
    assert!(why.contains("https://evil.example"), "{why}");
    assert!(said.is_empty(), "{said:?}");
}

/// **A login that fails says why, and to run it again**.
#[cfg(unix)]
#[test]
fn a_failed_sign_in_says_why() {
    let login = claude("echo; echo 'Login failed: the vendor is down'; exit 1");

    let (ended, _) = followed(&login, &[]);

    assert_eq!(
        ended,
        Err(
            "'work' did not sign in on pool builds: the runner said: the vendor is down. Run this again to try \
             once more."
                .to_owned()
        )
    );
}

/// **An input that ends with no code ends the sign-in**: a script is never left waiting.
#[cfg(unix)]
#[test]
fn an_input_that_ends_with_no_code_says_so() {
    let login = claude("exec sleep 30");

    let (ended, _) = followed_with(&login, "claude", None, &[], Input::Ends);

    assert!(ended.unwrap_err().contains("input ended"));
}

/// **A code goes on only while what the sign-in was opened on still holds** (#4164): where it no
/// longer does, the sign-in is cancelled and the code goes nowhere, as in the window's strip.
#[cfg(unix)]
#[test]
fn a_code_is_not_passed_on_once_the_runner_is_no_longer_yours() {
    let dir = tempfile::tempdir().unwrap();
    let got = dir.path().join("got");
    let login = claude(&format!("read code; echo \"$code\" > '{}'", got.display()));
    let driving = drive_with(&login, "claude", None, None, || Standing::SignedOut).unwrap();
    let (typed, lines) = channel();
    typed.send("the-code".to_owned()).unwrap();
    let mut written = Written::default();

    // It holds as the sign-in starts, and no longer as the code is to go on.
    let asked = std::cell::Cell::new(0);
    let said = follow_while(&driving, &lines, &mut written, "work", "team", &|| {
        asked.set(asked.get() + 1);
        if asked.get() == 1 {
            Ok(())
        } else {
            Err("the mark was taken back".to_owned())
        }
    });

    assert_eq!(said, Err("the mark was taken back".to_owned()));
    assert!(!got.exists(), "the code went nowhere");
}

/// **A login that takes no code is cancelled too** once what it was opened on no longer holds
/// (#4164): one approved on the vendor's page would otherwise sign in all the same.
#[cfg(unix)]
#[test]
fn a_sign_in_waiting_on_no_code_ends_once_the_runner_is_no_longer_yours() {
    let login = claude("sleep 30");
    let driving = drive_with(&login, "claude", None, None, || Standing::SignedOut).unwrap();
    let (_typed, lines) = channel();
    let mut written = Written::default();

    let said = follow_while(&driving, &lines, &mut written, "work", "team", &|| {
        Err("the mark was taken back".to_owned())
    });

    assert_eq!(said, Err("the mark was taken back".to_owned()));
}
