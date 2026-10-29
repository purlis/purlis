// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A command run with what it prints bounded** (#2056): the one way purlis runs `docker` and
//! `kubectl` for a pool. What the command prints is read up to a limit, and a command that prints
//! more is ended; what it says on standard error is kept up to a limit of its own, only to be
//! shown, and read to the end so the command never waits on it. A run may be given a time too
//! ([`run_within`]): one still running when it is up is ended, and said not to have finished;
//! nothing of such a run (writing its input, reading what it said) waits longer than its time.

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
    run_within(command, input, most, most_said, None)
}

/// How long what a run given a time said is waited on at least, once it has ended.
const SAID_AT_LEAST: std::time::Duration = std::time::Duration::from_millis(100);

/// How often a run given a time is looked at, to see whether it has ended.
const LOOKED_AT_EVERY: std::time::Duration = std::time::Duration::from_millis(20);

/// [`run`], and **ended once `within` is up** where it is given one: a command still running
/// then is killed and answered [`Unanswered::NotFinished`], its error of kind `TimedOut`. A
/// command that answers in time is read exactly as [`run`] reads it.
pub fn run_within(
    command: &mut Command,
    input: Option<&str>,
    most: u64,
    most_said: u64,
    within: Option<std::time::Duration>,
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
    // Its input is written beside the reading, so a command that never reads it holds up
    // neither the reading nor the clock; the writing ends as the command does.
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        let input = input.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    let said = child.stderr.take().map(|stderr| {
        let (kept_at, said) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let mut stderr = stderr;
            let _ = (&mut stderr).take(most_said).read_to_end(&mut kept);
            // What is past the limit is read and let go of, so the command never waits.
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            let _ = kept_at.send(kept);
        });
        said
    });
    let deadline = within.map(|within| std::time::Instant::now() + within);
    let mut printed = Vec::new();
    if let (Some(within), Some(deadline)) = (within, deadline) {
        // Read beside the clock: a command still running when its time is up is ended, which
        // closes what it prints, and the reading ends with it.
        let mut reading = child.stdout.take().map(|stdout| {
            std::thread::spawn(move || {
                let mut printed = Vec::new();
                let _ = stdout.take(most + 1).read_to_end(&mut printed);
                printed
            })
        });
        loop {
            // What it printed, once all of it is read: past the limit, it is ended at once.
            if reading
                .as_ref()
                .is_some_and(std::thread::JoinHandle::is_finished)
            {
                printed = reading
                    .take()
                    .and_then(|reading| reading.join().ok())
                    .unwrap_or_default();
                if printed.len() as u64 > most {
                    break;
                }
            }
            if reading.is_none() {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {}
                    // One that cannot be looked at is ended and reaped, not left behind.
                    Err(why) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(Unanswered::NotFinished(why));
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unanswered::NotFinished(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "it did not answer within {} seconds, so it was ended",
                        within.as_secs().max(1)
                    ),
                )));
            }
            std::thread::sleep(LOOKED_AT_EVERY);
        }
    } else if let Some(stdout) = child.stdout.take() {
        let _ = stdout.take(most + 1).read_to_end(&mut printed);
    }
    if printed.len() as u64 > most {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Unanswered::PrintedTooMuch);
    }
    let status = child.wait().map_err(Unanswered::NotFinished)?;
    // What it said, read to its end; where what it left running holds its errors open, waited
    // on no longer than its time, and then not shown.
    let said = said
        .and_then(|said| match deadline {
            // At least a moment, so the words of a command that ended right at its time are kept.
            Some(deadline) => said
                .recv_timeout(
                    deadline
                        .saturating_duration_since(std::time::Instant::now())
                        .max(SAID_AT_LEAST),
                )
                .ok(),
            None => said.recv().ok(),
        })
        .unwrap_or_default();
    Ok(Ran {
        status,
        printed,
        said,
    })
}

#[cfg(test)]
mod tests;
