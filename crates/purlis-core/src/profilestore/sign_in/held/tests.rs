// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{PipeWriter, Write};
use std::path::Path;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::process::{Pid, test_kill_process};

use super::*;

const SIZE: Size = Size {
    columns: 100,
    rows: 30,
};

fn stand_in(script: &str) -> Login {
    Login {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), script.to_owned()],
        env: Vec::new(),
        cleared: Vec::new(),
    }
}

/// `login`, run on a thread with an input held by the test.
fn held(login: Login) -> (PipeWriter, JoinHandle<Result<Option<i32>, String>>) {
    let (reader, writer) = std::io::pipe().expect("a pipe");
    (
        writer,
        std::thread::spawn(move || until_input_ends(&login, reader)),
    )
}

/// The process ids written to `file`, once they all are.
fn pids_in(file: &Path, how_many: usize) -> Vec<i32> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let read = std::fs::read_to_string(file).unwrap_or_default();
        let pids: Vec<i32> = read
            .split_whitespace()
            .map(|pid| pid.parse().expect("a pid"))
            .collect();
        if pids.len() == how_many {
            return pids;
        }
        assert!(Instant::now() < until, "the stand-in never said its pids");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether `pid` still runs, given a moment to be reaped by whoever took it over.
fn runs(pid: i32) -> bool {
    let pid = Pid::from_raw(pid).expect("a pid");
    let until = Instant::now() + Duration::from_secs(5);
    while test_kill_process(pid).is_ok() {
        if Instant::now() > until {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// **The login ends with its input, and so does what it started** (#4043): Claude Code's does
/// not stop when its input closes, and nothing else on the runner would end it.
#[test]
fn a_login_that_ignores_its_input_ends_when_its_input_closes() {
    let out = tempfile::tempdir().expect("a folder");
    let file = out.path().join("pids");
    let (input, ran) = held(stand_in(&format!(
        "sleep 60 & echo $! >> {f}; echo $$ >> {f}; wait",
        f = file.display()
    )));
    let pids = pids_in(&file, 2);

    drop(input);

    assert_eq!(ran.join().expect("ended"), Ok(None));
    assert!(pids.iter().all(|pid| !runs(*pid)), "still runs: {pids:?}");
}

/// **What a login started is ended with it when it ends by itself.**
#[test]
fn what_a_login_started_ends_when_it_ends_by_itself() {
    let out = tempfile::tempdir().expect("a folder");
    let file = out.path().join("pids");
    let (_input, ran) = held(stand_in(&format!(
        "sleep 60 & echo $! >> {}; exit 0",
        file.display()
    )));

    assert_eq!(ran.join().expect("ended"), Ok(Some(0)));
    let pids = pids_in(&file, 1);
    assert!(!runs(pids[0]), "still runs: {pids:?}");
}

/// **A login that will not be asked is made to end.**
#[test]
fn a_login_that_refuses_to_end_is_made_to() {
    let out = tempfile::tempdir().expect("a folder");
    let file = out.path().join("pids");
    let (input, ran) = held(stand_in(&format!(
        "trap '' TERM; echo $$ >> {}; while :; do sleep 1; done",
        file.display()
    )));
    let pids = pids_in(&file, 1);

    drop(input);

    assert_eq!(ran.join().expect("ended"), Ok(None));
    assert!(!runs(pids[0]));
}

/// **What comes in reaches the login, and a login that ends by itself says how.**
#[test]
fn a_login_is_passed_its_input_and_says_how_it_ended() {
    let out = tempfile::tempdir().expect("a folder");
    let file = out.path().join("said");
    let (mut input, ran) = held(stand_in(&format!(
        "read line; echo \"$line\" > {}; exit 3",
        file.display()
    )));

    input.write_all(b"code#state\n").expect("written");

    assert_eq!(ran.join().expect("ended"), Ok(Some(3)));
    assert_eq!(
        std::fs::read_to_string(&file).expect("written"),
        "code#state\n"
    );
    drop(input);
}

/// **A login in a terminal made here sees a terminal of the size asked for**, and what draws it
/// crosses the output.
#[test]
fn a_login_in_its_own_terminal_is_a_terminal_of_that_size() {
    let (reader, input) = std::io::pipe().expect("a pipe");
    let mut output = Vec::new();

    let ran = in_own_terminal(
        &stand_in("test -t 0 && stty size; exit 4"),
        SIZE,
        reader,
        &mut output,
    );

    drop(input);
    assert_eq!(ran, Ok(Some(4)));
    assert!(String::from_utf8_lossy(&output).contains("30 100"));
}

/// **A login in a terminal ends with its input too**: what a terminal `docker exec` made could
/// not promise.
#[test]
fn a_login_in_its_own_terminal_ends_when_its_input_closes() {
    let out = tempfile::tempdir().expect("a folder");
    let file = out.path().join("pids");
    let (reader, input) = std::io::pipe().expect("a pipe");
    let login = stand_in(&format!(
        "sleep 60 & echo $! >> {f}; echo $$ >> {f}; wait",
        f = file.display()
    ));
    let ran = std::thread::spawn(move || in_own_terminal(&login, SIZE, reader, std::io::sink()));
    let pids = pids_in(&file, 2);

    drop(input);

    assert_eq!(ran.join().expect("ended"), Ok(None));
    assert!(pids.iter().all(|pid| !runs(*pid)), "still runs: {pids:?}");
}
