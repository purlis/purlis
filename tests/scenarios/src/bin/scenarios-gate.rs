// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The run-result gate step** (T1.1-54, #6901): `scenarios-gate <reports> <trigger> <ran>`
//! judges a job's leg reports with the matrix coverage list (`purlis_scenarios::legs::judge_job`)
//! and exits 1 on any covered entry the legs it ran owe a verdict for and no report holds.
//!
//! A program of its own, built into the scenario kit's target folder, because a cargo run after
//! the suite finds the workspace's test build stale and builds it again (14 minutes on Linux, 21 on
//! macOS), and the gate needs no build at all.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [reports, trigger, ran] = &args[..] else {
        eprintln!(
            "usage: scenarios-gate <reports folder> <pull_request|label|merge_group|nightly> <leg:os:timings,...>"
        );
        return ExitCode::from(2);
    };
    match purlis_scenarios::legs::judge_job(Path::new(reports), trigger, ran) {
        Ok(problems) if problems.is_empty() => {
            println!(
                "the run-result gate holds: every covered entry the legs that ran owe has its verdict"
            );
            ExitCode::SUCCESS
        }
        Ok(problems) => {
            for problem in &problems {
                println!("::error title=run-result gate::{problem}");
            }
            println!("{} covered entries have no verdict", problems.len());
            ExitCode::FAILURE
        }
        Err(why) => {
            println!("::error title=run-result gate::{why}");
            ExitCode::FAILURE
        }
    }
}
