// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`tools/live-pass`** (T1.1-37, #6884): the live pass's brief generator and its
//! live-versus-suite diff. See `purlis_scenarios::livepass`.
//!
//! ```text
//! live-pass brief  --harness claude|codex|opencode --os linux|macos [--json]
//! live-pass report --cells <cells file> --log <live log> --harness H --os OS
//! live-pass diff   <live report> <suite report> [--suite-leg L] [--suite-timings scaled|production]
//! ```
//!
//! `brief` writes the runner's brief. `report` judges a live log by the cells' marks and writes
//! the suite's report format, one JSON record per line. `diff` lists where the two reports
//! differ and exits 1 when they do.

use std::fs;
use std::process::ExitCode;

use purlis_scenarios::coverage::{Coverage, Repo};
use purlis_scenarios::livepass::{Recorded, Sources, Switches, brief, diff, live_report};
use purlis_scenarios::model::{Batch, Dialect, Os, ReportLeg, TimingsMode};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("live-pass: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some((cmd, rest)) = args.split_first() else {
        return Err(USAGE.to_owned());
    };
    let opts = Opts::new(rest);
    match cmd.as_str() {
        "brief" => {
            let (harness, os) = (opts.harness()?, opts.os()?);
            let (coverage, repo, switches) = (Coverage::read()?, Repo::read()?, Switches::repo()?);
            let b = brief(
                harness,
                os,
                &Sources {
                    coverage: &coverage,
                    repo: &repo,
                    switches: &switches,
                },
            )?;
            if opts.flag("--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&b).map_err(|e| e.to_string())?
                );
            } else {
                print!("{}", b.markdown());
            }
            Ok(ExitCode::SUCCESS)
        }
        "report" => {
            let batch: Batch = read(&opts.value("--cells")?)?;
            let log: Value = read(&opts.value("--log")?)?;
            for r in live_report(&batch, &log, opts.harness()?, opts.os()?)? {
                println!("{}", serde_json::to_string(&r).map_err(|e| e.to_string())?);
            }
            Ok(ExitCode::SUCCESS)
        }
        "diff" => {
            let [live, suite] = opts.positional()[..] else {
                return Err(USAGE.to_owned());
            };
            let live = lines(live)?;
            let mut suite = lines(suite)?;
            if let Some(leg) = opts.optional("--suite-leg") {
                let leg: ReportLeg = parse(&leg)?;
                suite.retain(|r| r.leg.leg == leg);
            }
            if let Some(t) = opts.optional("--suite-timings") {
                let t: TimingsMode = parse(&t)?;
                suite.retain(|r| r.leg.timings == Some(t));
            }
            let d = diff(&live, &suite);
            print!("{}", d.render());
            Ok(if d.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        _ => Err(USAGE.to_owned()),
    }
}

const USAGE: &str = "usage: live-pass brief --harness H --os OS [--json]\n       live-pass report --cells FILE --log FILE --harness H --os OS\n       live-pass diff LIVE.jsonl SUITE.jsonl [--suite-leg LEG] [--suite-timings scaled|production]";

struct Opts<'a>(&'a [String]);

impl<'a> Opts<'a> {
    fn new(args: &'a [String]) -> Self {
        Opts(args)
    }
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    fn optional(&self, name: &str) -> Option<String> {
        self.0
            .iter()
            .position(|a| a == name)
            .and_then(|i| self.0.get(i + 1))
            .cloned()
    }
    fn value(&self, name: &str) -> Result<String, String> {
        self.optional(name)
            .ok_or_else(|| format!("{name} is required\n{USAGE}"))
    }
    fn harness(&self) -> Result<Dialect, String> {
        parse(&self.value("--harness")?)
    }
    fn os(&self) -> Result<Os, String> {
        parse(&self.value("--os")?)
    }
    /// The arguments that are neither an option nor an option's value.
    fn positional(&self) -> Vec<&str> {
        let mut out = Vec::new();
        let mut skip = false;
        for a in self.0 {
            if skip {
                skip = false;
            } else if a.starts_with("--") {
                skip = a != "--json";
            } else {
                out.push(a.as_str());
            }
        }
        out
    }
}

/// A word as the schema spells it (`macos`, `real_wrap`).
fn parse<T: DeserializeOwned>(word: &str) -> Result<T, String> {
    serde_json::from_value(Value::String(word.to_owned()))
        .map_err(|_| format!("`{word}` is not one of its values"))
}

fn read<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// A report: one JSON record per line.
fn lines(path: &str) -> Result<Vec<Recorded>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("{path}:{}: {e}", i + 1)))
        .collect()
}
