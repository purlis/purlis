// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every waiting `purlis` command says where it waits** (R-354, ADR 0121 decision 16): with an
//! ask left open, each road a command takes to wait on the person, `purlis secret exec`'s among
//! them, says "waiting for the person: Asks (✋) in the purlis window, project …, for up to …"
//! as it starts to wait, before the answer can come.
//!
//! The roads, each tested here at the core's seam:
//!
//! - **a Secret ask** ([`Asks::hold`]): `purlis secret exec` of a key its vault doesn't hold, a
//!   runner chat's tool, a run's missing keys;
//! - **a Run grant** ([`Asks::allowed_or_held`]): `purlis secret exec` of a program no tool
//!   declares, or of a vault its persona may not use;
//! - **a confirmation in the window** ([`windowline::asked_over`], the exchange of [`windowline::ask`]): `purlis profile authenticate
//!   --pool`, `purlis pool` taking a profile off, `purlis signing check`. Every kind of ask the
//!   line carries is listed in [`every_window_ask`], by a match with no wildcard, so a kind added
//!   later is told here whether it waits on the person, and is tested either way.
#![cfg(unix)]

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use purlis_core::secrets::ask::run::{Bound, Hosts, Offer, RunNeed};
use purlis_core::secrets::ask::{Asks, Need};
use purlis_core::signingsetup;
use purlis_core::windowline::{self, Answered, Asked, Said};

/// How long a wait in this test may go on: past it, a road that said nothing ends unanswered.
const AT_MOST: Duration = Duration::from_secs(2);

/// Each kind of ask a command puts to the window, with what it says as it starts to wait for it:
/// the line of ADR 0121 decision 16 where the person answers it, nothing where only the window
/// does.
fn every_window_ask(project: &Path) -> Vec<(Asked, Option<&'static str>)> {
    let config = project.join("config");
    let sign_in = Asked::ApproveSignIn {
        name: "work".into(),
        pool: "builds".into(),
        code: "AB12CD".into(),
        config: config.clone(),
        project: Some(project.to_path_buf()),
    };
    let take_off = Asked::TakeOffAPool {
        name: "work".into(),
        pool: "builds".into(),
        code: "AB12CD".into(),
        config,
        project: Some(project.to_path_buf()),
    };
    let check = Asked::Signing(signingsetup::Asked::Check {
        repo: "purlis/purlis".into(),
        commit: "abc1234".into(),
        code: "AB12CD".into(),
        on: None,
    });
    let key = Asked::Signing(signingsetup::Asked::KeyOnAccount {
        key: "ssh-ed25519 AAAA".into(),
        on: None,
    });
    let forget = Asked::ForgetOffThePools {
        name: "work".into(),
    };
    let keep = Asked::KeepOnAPool {
        name: "work".into(),
        pool: "builds".into(),
    };
    let in_the_project = "waiting for the person: Asks (\u{270b}) in the purlis window, project \
                          smart-ide, for up to 2 minutes";
    // A signing check names no project, so the window lists it under the one in front.
    let in_the_window =
        "waiting for the person: Asks (\u{270b}) in the purlis window, for up to 2 minutes";
    [sign_in, take_off, check, key, forget, keep]
        .into_iter()
        .map(|asked| {
            let said = match &asked {
                Asked::ApproveSignIn { .. } | Asked::TakeOffAPool { .. } => Some(in_the_project),
                Asked::Signing(signingsetup::Asked::Check { .. }) => Some(in_the_window),
                Asked::Signing(signingsetup::Asked::KeyOnAccount { .. })
                | Asked::ForgetOffThePools { .. }
                | Asked::KeepOnAPool { .. } => None,
            };
            (asked, said)
        })
        .collect()
}

/// A window on the far end of `peer` that reads one ask and answers it only once the command has
/// said something, as a person answers only once they know where to look.
fn a_window_that_answers_once_told(peer: UnixStream, told: mpsc::Receiver<()>) {
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(&peer), &mut line);
        if told.recv_timeout(AT_MOST * 2).is_ok() {
            let mut answer = serde_json::to_string(&Said::Refused {
                why: "answered".into(),
            })
            .expect("an answer");
            answer.push('\n');
            let _ = std::io::Write::write_all(&mut &peer, answer.as_bytes());
        }
    });
}

fn a_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("a folder");
    let project = tmp.path().join("smart-ide");
    std::fs::create_dir(&project).expect("a project");
    (tmp, project)
}

/// What a road said, and whether it said it as it started to wait, and then waited.
struct Heard {
    lines: Vec<String>,
    at_the_start: bool,
}

/// `wait` waits on the person, who never answers, for at most [`AT_MOST`], telling what it says
/// to the `say` it is handed.
fn heard_from(wait: impl FnOnce(&mut dyn FnMut(&str))) -> Heard {
    let mut lines = Vec::new();
    let began = Instant::now();
    let mut said_at = None;
    wait(&mut |line: &str| {
        said_at.get_or_insert_with(|| began.elapsed());
        lines.push(line.to_owned());
    });
    let waited = began.elapsed();
    Heard {
        lines,
        at_the_start: said_at.is_some_and(|at| at < AT_MOST / 2) && waited >= AT_MOST,
    }
}

/// The ADR's line for the project `smart-ide`, with how long the wait is.
fn waits_in_the_project(line: &str, within: &str) -> bool {
    line.ends_with(&format!(
        "waiting for the person: Asks (\u{270b}) in the purlis window, project smart-ide, for up \
         to {within}"
    ))
}

#[test]
fn every_waiting_purlis_command_says_where_it_waits() {
    purlis_core::unsteered!();
    let mut silent: Vec<String> = Vec::new();

    // A Secret ask: what `purlis secret exec` waits on for a key its vault doesn't hold.
    let (_tmp, project) = a_project();
    let asks = Asks::new(&project);
    let need = Need {
        chat: Some(7),
        persona: "devops".into(),
        vault: "team".into(),
        key: "TOKEN".into(),
        tool: None,
        purpose: None,
        declared: None,
        names_no_vault: false,
        reason: None,
        no_allow: None,
    };
    let heard = heard_from(|say| {
        asks.hold(need, AT_MOST, say);
    });
    if !(heard.at_the_start
        && heard.lines.len() == 1
        && waits_in_the_project(&heard.lines[0], "2 seconds"))
    {
        silent.push(format!("a Secret ask said {:?}", heard.lines));
    }

    // A Run grant: what `purlis secret exec` waits on for a program no tool declares.
    let (_tmp, project) = a_project();
    let asks = Asks::new(&project);
    let bound = Bound {
        chat: 7,
        name: "curl".into(),
        program: "/usr/bin/curl".into(),
        vault: "team".into(),
        env: vec!["T=TOKEN".into()],
        file: Vec::new(),
        cwd: "/work".into(),
        args: vec!["get".into()],
    };
    let need = RunNeed {
        ahead: false,
        fills: Vec::new(),
        offer: Offer::Program,
        command: "curl get".into(),
        not_its_vault: false,
        bound,
        persona: "devops".into(),
        hosts: Hosts::Listed(vec!["api.example.com".into()]),
        caution: None,
    };
    let heard = heard_from(|say| {
        asks.allowed_or_held(need, AT_MOST, say);
    });
    if !(heard.at_the_start
        && heard.lines.len() == 1
        && waits_in_the_project(&heard.lines[0], "2 seconds"))
    {
        silent.push(format!("a Run grant said {:?}", heard.lines));
    }

    // Each confirmation a command waits on in the window, and each ask only the window answers.
    for (asked, waits) in every_window_ask(&project) {
        let (command, window) = UnixStream::pair().expect("a line to the window");
        let (tell, told) = mpsc::channel();
        a_window_that_answers_once_told(window, told);
        let mut lines = Vec::new();
        let answered = windowline::asked_over(&command, &asked, AT_MOST, &mut |line: &str| {
            lines.push(line.to_owned());
            let _ = tell.send(());
        });
        let heard_in_time = matches!(answered, Answered::Said(Said::Refused { .. }));
        match waits {
            Some(line) if !(heard_in_time && lines == [line]) => silent.push(format!(
                "{asked:?} waits on the person and said {lines:?}, answered {answered:?}"
            )),
            None if !lines.is_empty() => silent.push(format!(
                "{asked:?} waits on the window alone, and said {lines:?}"
            )),
            _ => {}
        }
    }

    assert!(
        silent.is_empty(),
        "a waiting command that does not say where it waits (ADR 0121 decision 16):\n{}",
        silent.join("\n")
    );
}
