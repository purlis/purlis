// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash leaves one minidump for the next launch** (#5691, spec #5683, ruling O9).
//!
//! Each case runs `crash-probe`, a program that watches itself with the handler and then crashes
//! the way it is told, and reads the folder the handler keeps its minidumps in.
//!
//! macOS only, as the app watches itself only there for now: Linux and Windows are follow-ups.
//! The handler binds a Unix socket, so a sandbox that refuses one cannot run these.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn crash(how: &str, crashes: &Path) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_crash-probe"))
        .arg(how)
        .arg(crashes)
        .status()
        .expect("the probe ran")
}

fn minidumps(crashes: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(crashes) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "dmp"))
        .collect()
}

fn is_a_minidump(path: &Path) -> bool {
    std::fs::read(path)
        .expect("the minidump")
        .starts_with(b"MDMP")
}

#[test]
fn a_process_that_aborts_leaves_one_minidump() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("abort", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(crashes.path());
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(is_a_minidump(&left[0]));
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&left[0]), 0o600, "a minidump is this user's alone");
}

#[test]
fn a_process_that_overflows_its_stack_leaves_one_minidump() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("overflow", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    let left = minidumps(crashes.path());
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(is_a_minidump(&left[0]));
}

#[test]
fn a_panic_already_told_leaves_no_minidump_since_its_own_report_has_it() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("panic", crashes.path());

    assert!(!ended.success(), "the probe crashed: {ended:?}");
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}

#[test]
fn a_process_that_ends_well_leaves_nothing() {
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("exit", crashes.path());

    assert!(ended.success(), "{ended:?}");
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}

#[test]
#[ignore = "red on minidumper-child 0.7, whose crash-handler 0.7 answers a child's exception as handled: on once a release depends on crash-handler 0.8 (#5691)"]
fn a_child_of_a_watched_process_that_crashes_dies_of_it_and_leaves_no_minidump() {
    // A child inherits its parent's exception port on macOS. A handler that answers the child's
    // exception as handled resumes its faulting thread, which faults again, for ever.
    let crashes = tempfile::tempdir().unwrap();

    let ended = crash("child", crashes.path());

    assert_eq!(
        ended.code(),
        Some(0),
        "the child died of its crash: {ended:?}"
    );
    assert_eq!(minidumps(crashes.path()), Vec::<PathBuf>::new());
}
