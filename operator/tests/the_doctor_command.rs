// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator doctor` as a team admin runs it (#2236): without a cluster to ask it
//! says so in one line with the next step and exits 1; a command it does not know is a usage
//! error.

use std::process::Command;

fn operator(args: &[&str]) -> std::process::Output {
    let home = std::env::temp_dir().join(format!(
        "purlis-doctor-home-{}-{}",
        std::process::id(),
        args.join("-")
    ));
    std::fs::create_dir_all(&home).expect("a home of the test's own");
    let output = Command::new(env!("CARGO_BIN_EXE_purlis-cluster-operator"))
        .args(args)
        .env_clear()
        .env("HOME", &home)
        .env("KUBECONFIG", home.join("no-such-kubeconfig"))
        .output()
        .expect("the operator's binary");
    let _ = std::fs::remove_dir_all(&home);
    output
}

#[test]
fn doctor_without_a_cluster_names_the_next_step_and_exits_1() {
    let output = operator(&["doctor"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "{stdout}");
    assert!(lines[0].starts_with("missing: a kubeconfig"), "{stdout}");
    assert!(lines[0].contains("Next: "), "{stdout}");
}

#[test]
fn a_command_it_does_not_know_is_a_usage_error() {
    let output = operator(&["doctr"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("doctor"));
}
