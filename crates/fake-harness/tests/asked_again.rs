// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `--again`: a line typed into an interactive fake harness that writes its output again, so a
//! benchmark can burst the same chat as many times as it switches to it (`felt.bench.ts`, row
//! L12). Run over plain pipes: what is tested is what the harness writes and when, not the
//! terminal it writes to (`in_a_session.rs` has that).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn a_line_asking_again_replays_the_output_the_sentinel_and_the_hooks_and_others_are_answered() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("corpus.bin");
    std::fs::write(&corpus, b"plain output\n").unwrap();
    let log = dir.path().join("hooks.log");
    let mut harness = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .args([
            "--corpus",
            corpus.to_str().unwrap(),
            "--wait-for-input",
            "--sentinel",
            "BURST-DONE",
            "--hook",
            &format!("echo done >> {}", log.to_str().unwrap()),
            "--interactive",
            "--again",
            "burst",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the fake harness starts");
    let mut typed = harness.stdin.take().unwrap();
    let mut written = BufReader::new(harness.stdout.take().unwrap()).lines();
    let mut until = |wanted: &str| -> Vec<String> {
        let mut seen = Vec::new();
        for line in written.by_ref() {
            let line = line.unwrap();
            let done = line.contains(wanted);
            seen.push(line);
            if done {
                return seen;
            }
        }
        panic!("{wanted:?} was never written: {seen:#?}");
    };

    typed.write_all(b"\n").unwrap();
    until("BURST-DONE");
    typed.write_all(b"burst\n").unwrap();
    let again = until("BURST-DONE");
    typed.write_all(b"hello\n").unwrap();
    let answered = until("you said: hello");
    typed.write_all(b"/quit\n").unwrap();

    assert!(harness.wait().unwrap().success());
    assert_eq!(again, ["plain output", "", "BURST-DONE"]);
    assert_eq!(
        answered,
        ["you said: hello"],
        "the line asking again was answered"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "done\ndone\n");
}
