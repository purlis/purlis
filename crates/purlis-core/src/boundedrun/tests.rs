// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;
use std::time::{Duration, Instant};

use super::*;

fn sh(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

#[test]
fn a_run_given_a_time_that_does_not_answer_in_it_is_ended_and_said_not_to_have_finished() {
    let started = Instant::now();
    let ran = run_within(
        &mut sh("sleep 30"),
        None,
        1024,
        1024,
        Some(Duration::from_millis(300)),
    );
    assert!(
        matches!(&ran, Err(Unanswered::NotFinished(why)) if why.kind() == std::io::ErrorKind::TimedOut),
        "it is ended for its time"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it is ended when its time is up"
    );
}

#[test]
fn a_run_given_a_time_that_answers_in_it_is_read_as_any_run_is() {
    let ran = run_within(
        &mut sh("printf answered; printf said >&2"),
        None,
        1024,
        1024,
        Some(Duration::from_secs(20)),
    )
    .expect("it answers");
    assert!(ran.status.success());
    assert_eq!(ran.printed, b"answered");
    assert_eq!(ran.said, b"said");
}

#[test]
fn a_run_given_a_time_that_prints_past_its_limit_is_ended_for_that() {
    let ran = run_within(&mut sh("yes"), None, 64, 64, Some(Duration::from_secs(20)));
    assert!(matches!(ran, Err(Unanswered::PrintedTooMuch)));
}

#[test]
fn a_run_given_a_time_that_never_reads_its_input_is_still_ended_when_its_time_is_up() {
    // More than a pipe holds, to a command that never reads it.
    let input = "x".repeat(1024 * 1024);
    let started = Instant::now();
    let ran = run_within(
        &mut sh("sleep 30"),
        Some(&input),
        1024,
        1024,
        Some(Duration::from_millis(300)),
    );
    assert!(
        matches!(&ran, Err(Unanswered::NotFinished(why)) if why.kind() == std::io::ErrorKind::TimedOut),
        "it is ended for its time"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "writing its input holds up nothing"
    );
}

#[test]
fn a_run_given_a_time_is_answered_when_its_command_ends_though_what_it_left_keeps_its_errors_open()
{
    let dir = tempfile::tempdir().unwrap();
    let left = dir.path().join("left");
    let started = Instant::now();
    // What the command leaves running holds its standard error open, and nothing else.
    let ran = run_within(
        &mut sh(&format!(
            "(sleep 30 >/dev/null </dev/null & echo $! > '{}'); printf answered",
            left.display()
        )),
        None,
        1024,
        1024,
        Some(Duration::from_millis(500)),
    );
    let elapsed = started.elapsed();
    if let Ok(pid) = std::fs::read_to_string(&left) {
        let _ = crate::forklock::status(Command::new("kill").arg(pid.trim()));
    }
    let ran = ran.expect("the command itself answered");
    assert_eq!(ran.printed, b"answered");
    assert!(
        elapsed < Duration::from_secs(10),
        "its errors are waited on no longer than its time"
    );
}
