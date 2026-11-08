// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A `purlis` command that fails inside purlis exits 70, and only then** (#5697, spec #5683
//! Q23): the one code purlis's own hook drafts a Report of. A refusal and a usage error exit 1.

use crate::support;

use purlis_core::caught::INTERNAL_EXIT;

/// What makes a debug build's `purlis` panic before it answers (#349).
const PANICS: (&str, &str) = ("CHARTER_TEST_HOOK_PANICS", "1");

fn run(args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let mut command = support::purlis();
    command
        .args(args)
        .current_dir(dir.path())
        .env("HOME", dir.path());
    for (name, value) in env {
        command.env(name, value);
    }
    let out = command.output().expect("purlis runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_command_that_panics_exits_with_the_internal_failure_code() {
    let (code, err) = run(&["version"], &[PANICS]);
    assert_eq!(code, Some(i32::from(INTERNAL_EXIT)), "{err}");
    assert!(err.starts_with("purlis: internal failure: "), "{err}");
}

#[test]
fn a_usage_error_never_exits_with_the_internal_failure_code() {
    let (code, err) = run(&["worktree", "comit"], &[]);
    assert_eq!(code, Some(1), "{err}");
}

#[test]
fn a_hook_that_panics_keeps_its_own_end() {
    // A guard's crash is a refusal (exit 2), and a reporting hook's any other failure: neither is
    // a command a chat ran, and neither is drafted.
    let (code, err) = run(&["hook", "stop"], &[PANICS]);
    assert_ne!(code, Some(i32::from(INTERNAL_EXIT)), "{err}");
    assert_ne!(code, Some(0), "{err}");
}
