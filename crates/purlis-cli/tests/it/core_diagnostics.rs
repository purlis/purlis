// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A diagnostic `purlis-core` raises while the `charter` binary is running reaches standard
//! error (#647). The app keeps its diagnostics in a log file; the CLI has a terminal, so the
//! core's warnings go there, as the message alone and redacted the same way.

use crate::support;

#[test]
fn a_warning_raised_in_the_core_under_the_cli_is_said_on_standard_error() {
    let dir = tempfile::tempdir().expect("a directory");

    let said = support::purlis()
        .arg("--version")
        .current_dir(dir.path())
        .env_remove("CHARTER_ROOT")
        .env("CHARTER_CONFIG_HOME", dir.path())
        .env("CHARTER_TEST_CORE_WARNS", "1")
        .output()
        .expect("the binary runs");

    assert!(said.status.success(), "{said:?}");
    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(
        stderr, "purlis: a warning purlis-core raised on purpose, for a test\n",
        "the core's warning, as the message alone"
    );
}

/// `purlis serve` is a runner's host: its standard error is the service's journal or its
/// container's log, so the core's `info` is said there too, as the message alone (#2753).
#[cfg(unix)]
#[test]
fn an_info_line_raised_in_the_core_under_purlis_serve_is_said_on_standard_error() {
    let dir = tempfile::tempdir().expect("a directory");

    let said = support::purlis()
        .arg("serve")
        .current_dir(dir.path())
        .env_remove("CHARTER_ROOT")
        .env("CHARTER_CONFIG_HOME", dir.path())
        .env("PURLIS_TEST_CORE_INFORMS", "1")
        .output()
        .expect("the binary runs");

    assert!(said.status.success(), "{said:?}");
    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(
        stderr, "purlis: an info line purlis-core raised on purpose, for a test\n",
        "the core's info, as the message alone"
    );
}
