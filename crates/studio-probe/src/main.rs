// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `studio-probe` — read one request line on stdin, print its answer on stdout.
//!
//! `studio-probe assemble <dir>` puts this program and its manifest in `<dir>`. Test-only; see
//! `lib.rs`.

use std::io::{BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [verb, dir] = args.as_slice()
        && verb == "assemble"
    {
        return match studio_probe::assemble(std::path::Path::new(dir)) {
            Ok(_) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("studio-probe: could not assemble into {dir}: {why}");
                ExitCode::FAILURE
            }
        };
    }
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        // End of input before a question: purlis is gone, and there is nobody to answer.
        Ok(0) => return ExitCode::SUCCESS,
        Ok(_) => {}
        Err(why) => {
            eprintln!("studio-probe: could not read the request: {why}");
            return ExitCode::FAILURE;
        }
    }
    let request: serde_json::Value = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(why) => {
            eprintln!("studio-probe: the request was not JSON: {why}");
            return ExitCode::FAILURE;
        }
    };
    // A grade is a question of many lines: its answers and purlis's go on until its report.
    if studio_probe::is_a_grade(&request) {
        let mut lines = std::io::stdin().lock().lines();
        let mut out = std::io::stdout().lock();
        return match studio_probe::grade(&request, &mut lines, &mut out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("studio-probe: the grade could not go on: {why}");
                ExitCode::FAILURE
            }
        };
    }
    let (said, status) = studio_probe::answer(&request);
    let mut out = std::io::stdout().lock();
    let written = match status {
        Some(_) => write!(out, "{said}"),
        None => writeln!(out, "{said}"),
    };
    if written.and_then(|()| out.flush()).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::from(status.unwrap_or(0))
}
