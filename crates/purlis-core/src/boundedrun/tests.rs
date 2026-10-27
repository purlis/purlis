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
