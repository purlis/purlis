// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How every test in this folder starts the binary (#1284): with none of the variables that
//! choose a project, which a suite run inside a chat inherits under both names (V93k). The test
//! sets its own after. `every_test_starts_the_binary_unsteered.rs` fails on a test that starts
//! one any other way.
//!
//! Each test file declares `mod support;` and uses what it needs, so the rest is unused there.
#![allow(dead_code)]

use std::ffi::OsStr;
use std::process::Command;

/// The `purlis` binary cargo built for this run. Its path, for a test that writes it into a
/// hook or hands it to a shell: such a file starts every process through [`command`].
pub const PURLIS: &str = env!("CARGO_BIN_EXE_purlis");

/// The `charter` alias cargo built for this run (RN-3), which runs the `purlis` beside it.
pub const CHARTER: &str = env!("CARGO_BIN_EXE_charter");

/// `purlis`, unsteered.
pub fn purlis() -> Command {
    command(PURLIS)
}

/// `program`, unsteered: every spelling of every variable that chooses a project
/// (`purlis_core::envvar::SELECTING`) removed from what it inherits.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    for rest in purlis_core::envvar::SELECTING {
        for spelling in purlis_core::envvar::spellings(&format!("PURLIS_{rest}")) {
            command.env_remove(spelling);
        }
    }
    command
}
