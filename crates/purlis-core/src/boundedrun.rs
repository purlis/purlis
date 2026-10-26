// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A command run with what it prints bounded** (#2056): the one way purlis runs `docker` and
//! `kubectl` for a pool. What the command prints is read up to a limit, and a command that prints
//! more is ended; what it says on standard error is kept up to a limit of its own, only to be
//! shown, and read to the end so the command never waits on it.

use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};

/// What a bounded run printed, said and ended with.
pub struct Ran {
    pub status: ExitStatus,
    /// Its standard output, at most the limit it was run with.
    pub printed: Vec<u8>,
    /// Its standard error, at most the limit it was run with.
    pub said: Vec<u8>,
}

/// Why a bounded run gave no answer.
#[derive(Debug)]
pub enum Unanswered {
    /// The command could not be started.
    NotStarted(std::io::Error),
    /// It printed more than the limit, so it was ended.
    PrintedTooMuch,
    /// It could not be waited for.
    NotFinished(std::io::Error),
}

/// Runs `command`, handing it `input` on its standard input (or none), and reads at most `most`
/// bytes of its standard output and `most_said` of its standard error.
pub fn run(
    command: &mut Command,
    input: Option<&str>,
    most: u64,
    most_said: u64,
) -> Result<Ran, Unanswered> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = crate::forklock::spawn(command).map_err(Unanswered::NotStarted)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(input.as_bytes());
    }
    let said = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let mut stderr = stderr;
            let _ = (&mut stderr).take(most_said).read_to_end(&mut kept);
            // What is past the limit is read and let go of, so the command never waits.
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            kept
        })
    });
    let mut printed = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(most + 1).read_to_end(&mut printed);
    }
    if printed.len() as u64 > most {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Unanswered::PrintedTooMuch);
    }
    let status = child.wait().map_err(Unanswered::NotFinished)?;
    let said = said.and_then(|said| said.join().ok()).unwrap_or_default();
    Ok(Ran {
        status,
        printed,
        said,
    })
}
