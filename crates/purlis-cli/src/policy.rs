// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis policy check [<file>]` (T1a-64, R-432): what this build reads in a policy file, before
//! an administrator rolls it out. The core (`purlis_core::sandbox::policy::check`) reads and
//! judges; this prints its lines and exits with its status: 0 when this build reads the file and
//! enforces every key in it, 1 when a key is unenforced or unknown or `min-version` is above this
//! build, 2 when the file can't be read or parsed.

use std::process::ExitCode;

use clap::Subcommand;
use purlis_core::sandbox::policy::check;

#[derive(Subcommand)]
pub enum PolicyCommand {
    /// Say what this build reads in a policy file: the lowest purlis version that reads it, the
    /// keys this version doesn't enforce or doesn't know, and whether chats that reach a model
    /// would be refused under it. One line per fact. Exits 0 when this build reads the file and
    /// enforces every key in it, 1 when a key is unenforced or unknown or `min-version` is above
    /// this build, and 2 when the file can't be read or parsed.
    Check {
        /// A draft to check before it is installed: its owner and permissions are not judged.
        /// Without it, this machine's /etc/purlis/policy.json is judged as purlis reads it.
        file: Option<std::path::PathBuf>,
    },
}

pub fn run(command: &PolicyCommand) -> ExitCode {
    let checked = match command {
        PolicyCommand::Check { file: Some(file) } => check::draft(file),
        PolicyCommand::Check { file: None } => check::installed(),
    };
    for line in &checked.lines {
        println!("{line}");
    }
    ExitCode::from(checked.status)
}
