// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice I: the end-to-end test that proves the scenario (ADR 0128).
//!
//! A person signs in once to a forge, clones a repo as a project, and a chat runs `gh`, `glab` and
//! `git` against it with no manual command and no token in its hands. The scenario is
//! `tests/one-login-e2e/run.mjs`: the real clients against a fake forge of each shape, with
//! `tests/one-login-e2e/seams.mjs` standing in for each slice of the ADR that has not merged
//! (`TODO(slice A)` to `TODO(slice G)`). It needs `gh`, `glab`, `git`, `openssl` and `node`, so
//! it runs only behind the `forge-wiring` feature, on the runners that have them
//! (`.github/workflows/one-login-e2e.yml`).
//!
//! As each slice merges, its stand-in is replaced by a call into this crate and its marker goes;
//! this file is where those calls belong.

use std::path::Path;
use std::process::Command;

#[test]
fn a_person_signs_in_once_and_a_chat_works_the_forge_without_a_token() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("node")
        .arg("tests/one-login-e2e/run.mjs")
        .current_dir(&repo)
        .output()
        .expect("node runs the scenario");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the scenario failed ({}):\n{stdout}\n{stderr}",
        out.status
    );
    assert!(
        stdout.contains("every step passed"),
        "the scenario did not say every step passed:\n{stdout}"
    );
}
