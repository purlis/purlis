// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis bridge --prune`** (#6688): a runner's purlis, installed as one version of several
//! (`<config>/server/<version>/purlis`), removes the folders of every version older than the one
//! before it whose host has drained, and prints what went. What decides which go, and when a host
//! has drained, is `purlis_core::runnerinstall::prune`'s, whose tests hold each case.
#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

/// `binary bridge --prune`, with `home` as its home and config home, and on Linux a runtime
/// folder of its own under it, where its hosts' sockets would be (macOS names the user's own
/// temporary folder, whatever the environment says).
fn run(binary: &Path, home: &Path) -> Output {
    let runtime = home.join("run");
    std::fs::create_dir_all(&runtime).unwrap();
    std::fs::set_permissions(
        &runtime,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    support::command(binary)
        .args(["bridge", "--prune"])
        .current_dir(home)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("PURLIS_CONFIG_HOME", home)
        .env("TMPDIR", std::env::temp_dir())
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

/// This build's purlis installed as `version` in `home`'s config home, with its host's folder.
fn installed(home: &Path, version: &str) -> PathBuf {
    let folder = purlis_core::machine::dir(home)
        .join(purlis_core::runnerinstall::SERVER_DIR)
        .join(version);
    std::fs::create_dir_all(&folder).unwrap();
    let binary = folder.join(purlis_core::runnerinstall::BINARY);
    if std::fs::hard_link(support::PURLIS, &binary).is_err() {
        std::fs::copy(support::PURLIS, &binary).unwrap();
    }
    let host =
        purlis_core::runnerinstall::host_folder_of(&purlis_core::machine::charterd(home), version);
    std::fs::create_dir_all(&host).unwrap();
    binary
}

#[test]
fn the_running_version_removes_a_drained_one_and_keeps_the_one_before_it() {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    let v1 = installed(&home, "1.0.0");
    let v2 = installed(&home, "2.0.0");
    let v3 = installed(&home, "3.0.0");

    let output = run(&v3, &home);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "removed=1.0.0\n");
    assert!(!v1.parent().unwrap().exists(), "version 1's folder is gone");
    assert!(
        !purlis_core::runnerinstall::host_folder_of(
            &purlis_core::machine::charterd(&home),
            "1.0.0"
        )
        .exists(),
        "version 1's host folder is gone"
    );
    assert!(
        v2.exists() && v3.exists(),
        "the one before and the running one stay"
    );
}

#[test]
fn a_purlis_not_installed_as_a_version_removes_nothing_and_says_so() {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    let v1 = installed(&home, "1.0.0");
    installed(&home, "2.0.0");
    let output = run(Path::new(support::PURLIS), &home);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("isn't installed as a version"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(v1.exists());
}
