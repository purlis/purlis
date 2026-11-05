// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `check-probe` — read one request line on stdin, print SARIF on stdout.
//!
//! `check-probe assemble <dir>` puts this program and its manifest in `<dir>`. Test-only; see
//! `lib.rs`.

use std::io::{BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [verb, dir] = args.as_slice()
        && verb == "assemble"
    {
        return match check_probe::assemble(std::path::Path::new(dir)) {
            Ok(_) => ExitCode::SUCCESS,
            Err(why) => {
                eprintln!("check-probe: could not assemble into {dir}: {why}");
                ExitCode::FAILURE
            }
        };
    }
    if !args.is_empty() {
        eprintln!("usage: check-probe | check-probe assemble <dir>");
        return ExitCode::FAILURE;
    }
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        // End of input before a question: purlis is gone, and there is nobody to answer.
        Ok(0) => return ExitCode::SUCCESS,
        Ok(_) => {}
        Err(why) => {
            eprintln!("check-probe: could not read the request: {why}");
            return ExitCode::FAILURE;
        }
    }
    let printed = check_probe::answer(&line);
    print!("{}", printed.stdout);
    eprint!("{}", printed.stderr);
    let _ = std::io::stdout().flush();
    ExitCode::from(printed.status)
}
