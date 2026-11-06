// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A tick hook fires at a set rate while the harness streams (#4883), the way a busy chat's
//! tool calls fire its hooks while it writes, so the load bench measures the chat events the
//! app batches (#4746) and not only bytes.
#![cfg(unix)]

use std::process::Command;

#[test]
fn a_tick_hook_fires_at_its_rate_while_the_output_streams_and_numbers_each_firing() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let fired = dir.path().join("fired");
    // About a second of output: 200 writes of 1,000 bytes, 5 ms apart.
    let started = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .args([
            "--synthetic",
            "200000",
            "--chunk",
            "1000",
            "--interval-ms",
            "5",
        ])
        .args(["--tick-ms", "100", "--tick-hook"])
        .arg(format!(
            "echo \"$FAKE_HARNESS_TICK\" >> '{}'",
            fired.display()
        ))
        .args(["--sentinel", "DONE"])
        .output()
        .expect("the fake harness runs");
    let took = started.elapsed();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let ticks: Vec<u64> = std::fs::read_to_string(&fired)
        .expect("a tick hook fired")
        .lines()
        .map(|line| line.parse().expect("each firing is numbered"))
        .collect();
    // One a tick of the run, at most; a loaded runner may lose a few, never most.
    let most = (took.as_millis() / 100) as usize;
    assert!(
        ticks.len() >= 5,
        "only {} firings in {took:?}: {ticks:?}",
        ticks.len()
    );
    assert!(
        ticks.len() <= most,
        "{} firings in {took:?}: {ticks:?}",
        ticks.len()
    );
    assert_eq!(ticks, (1..=ticks.len() as u64).collect::<Vec<_>>());
}

#[test]
fn a_tick_hook_does_not_fire_once_the_output_is_done() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let fired = dir.path().join("fired");
    let out = Command::new(env!("CARGO_BIN_EXE_fake-harness"))
        .args(["--synthetic", "10", "--tick-ms", "50", "--tick-hook"])
        .arg(format!("echo tick >> '{}'", fired.display()))
        // Half a second after the output, by a hook of the ordinary kind.
        .args(["--hook", "sleep 0.5"])
        .output()
        .expect("the fake harness runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let firings = std::fs::read_to_string(&fired).unwrap_or_default();
    assert!(
        firings.lines().count() <= 1,
        "fired after the output: {firings:?}"
    );
}
