// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat on a profile this machine made, whose harness says it is not signed in on this
//! device, is refused before it starts, with Sign in as the way out (#2035, ADR 0095 §3). The
//! harness says so by its own status command; purlis reads nothing it prints, only how it
//! exits. A profile the harness says is signed in starts, and the person's own profiles (a
//! built-in, an adopted folder) are never asked.
//!
//! Each body runs in a child whose config and data homes are the test's own folder, with a
//! stand-in for each harness first on its `PATH` that answers `--version` as its harness does
//! and its status words with the exit status the test gives it.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::profilestore::{self, Homes, MachineSettings};
use purlis_core::start::{self, Start};

/// Set on the child: the data home it was started with.
const DATA: &str = "PROFILE_SIGN_IN_DATA_HOME";

/// Runs `test` again in a child whose data home is a folder of its own and whose harnesses
/// answer their status words with `status`; answers that data home where this is that child.
fn in_a_data_home_of_its_own(test: &str, status: u8) -> Option<PathBuf> {
    if let Some(data) = std::env::var_os(DATA) {
        return Some(PathBuf::from(data));
    }
    let data = tempfile::tempdir().unwrap();
    let programs = tempfile::tempdir().unwrap();
    for (harness, status_words, version) in [
        ("claude", "auth status", "2.1.288 (Claude Code)"),
        ("codex", "login status", "codex-cli 0.147.0"),
    ] {
        stand_in::program(
            programs.path(),
            harness,
            &format!(
                "#!/bin/sh\nif [ \"$1 $2\" = '{status_words}' ]; then exit {status}; fi\necho '{version}'\n"
            ),
        );
    }
    let path = std::env::join_paths(std::iter::once(programs.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    purlis_core::testrun::rerun_steered(
        &[test],
        &[
            (purlis_core::datahome::HOME_VAR, data.path().as_os_str()),
            (purlis_core::machine::HOME_VAR, data.path().as_os_str()),
            (DATA, data.path().as_os_str()),
            ("PATH", path.as_os_str()),
        ],
    );
    None
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("charter.toml"), "").unwrap();
    dir
}

fn start_on(name: &str, root: &Path) -> Start {
    Start {
        profile: Some(name.to_owned()),
        name: "ide.7".to_owned(),
        cwd: Some(root.to_path_buf()),
        ..Start::default()
    }
}

/// Makes the profile `name` of `kind`, with nothing to import.
fn made(name: &str, kind: &str) {
    let store = Homes::here().expect("the test's data home");
    let home = tempfile::tempdir().unwrap();
    let env = |_: &str| None;
    profilestore::create(
        &store,
        name,
        kind,
        &MachineSettings {
            home: home.path(),
            env: &env,
        },
        1,
    )
    .expect("the profile is made");
}

#[test]
fn a_claude_chat_on_a_made_profile_claude_says_is_signed_out_is_refused_with_sign_in() {
    purlis_core::unsteered!();
    let test = "a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in::a_claude_chat_on_a_made_profile_claude_says_is_signed_out_is_refused_with_sign_in";
    let Some(_data) = in_a_data_home_of_its_own(test, 1) else {
        return;
    };
    made("work", "claude");
    let plane = project();

    let why = start::ready(&start_on("work", plane.path()), plane.path())
        .expect_err("a profile that is not signed in does not start");

    assert!(why.contains("'work' is not signed in"), "{why}");
    assert!(why.contains("Sign in work"), "{why}");
    assert!(why.contains("purlis profile sign-in work"), "{why}");
    // One Ask waits for the profile, however many starts were refused.
    start::ready(&start_on("work", plane.path()), plane.path()).expect_err("refused again");
    let waits = purlis_core::profilestore::sign_in::waiting(plane.path());
    assert_eq!(waits.len(), 1, "{waits:?}");
    assert_eq!(waits[0].profile.name, "work");
}

#[test]
fn a_codex_chat_on_a_made_profile_codex_says_is_signed_out_is_refused_with_sign_in() {
    purlis_core::unsteered!();
    let test = "a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in::a_codex_chat_on_a_made_profile_codex_says_is_signed_out_is_refused_with_sign_in";
    let Some(_data) = in_a_data_home_of_its_own(test, 1) else {
        return;
    };
    made("client", "codex");
    let plane = project();

    let why = start::ready(&start_on("client", plane.path()), plane.path())
        .expect_err("a profile that is not signed in does not start");

    assert!(why.contains("'client' is not signed in"), "{why}");
    assert!(why.contains("Codex's own sign-in"), "{why}");
}

#[test]
fn a_chat_on_a_made_profile_its_harness_says_is_signed_in_starts() {
    purlis_core::unsteered!();
    let test = "a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in::a_chat_on_a_made_profile_its_harness_says_is_signed_in_starts";
    let Some(_data) = in_a_data_home_of_its_own(test, 0) else {
        return;
    };
    made("work", "claude");
    let plane = project();

    start::ready(&start_on("work", plane.path()), plane.path())
        .expect("a signed-in profile starts");
}

#[test]
fn a_chat_on_a_made_profile_whose_harness_cannot_tell_starts() {
    purlis_core::unsteered!();
    let test = "a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in::a_chat_on_a_made_profile_whose_harness_cannot_tell_starts";
    let Some(_data) = in_a_data_home_of_its_own(test, 7) else {
        return;
    };
    made("work", "claude");
    let plane = project();

    start::ready(&start_on("work", plane.path()), plane.path())
        .expect("a chat that might work is not refused");
}

#[test]
fn the_built_in_profile_runs_on_the_persons_own_sign_in_and_is_never_asked() {
    purlis_core::unsteered!();
    let test = "a_chat_on_a_profile_not_signed_in_is_refused_with_sign_in::the_built_in_profile_runs_on_the_persons_own_sign_in_and_is_never_asked";
    let Some(_data) = in_a_data_home_of_its_own(test, 1) else {
        return;
    };
    let plane = project();

    start::ready(&start_on("claude", plane.path()), plane.path())
        .expect("the person's own Claude Code starts as it always did");
}
