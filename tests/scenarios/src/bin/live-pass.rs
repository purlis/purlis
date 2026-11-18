// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`tools/live-pass`** (T1.1-37, #6884): the live pass's brief generator and its
//! live-versus-suite diff. See `purlis_scenarios::livepass`.
//!
//! ```text
//! live-pass brief  --harness claude|codex|opencode --os linux|macos [--json]
//! live-pass report --cells <cells file>... --log <live log> --harness H --os OS
//! live-pass diff   <live report> <suite report> [--suite-leg L] [--suite-timings scaled|production]
//! ```
//!
//! `brief` writes the runner's brief. `report` judges a live log by the cells' marks and writes
//! the suite's report format, one JSON record per line; `--cells` is repeated for every cells
//! file the log has cells of. `diff` lists where the two reports differ. Exit codes: 0 when they
//! agree, 1 when they differ or when nothing was compared, 2 when a file or an argument is wrong.

use std::fs;
use std::process::ExitCode;

use purlis_scenarios::coverage::{Coverage, Repo};
use purlis_scenarios::livepass::{
    Sources, Switches, brief, check_live, diff, live_report, read_report, scope_suite,
};
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
            let files = opts.values("--cells")?;
            if files.is_empty() {
                return Err(format!("--cells is required\n{USAGE}"));
            }
            let batches: Vec<Batch> = files
                .iter()
                .map(|f| read(f))
                .collect::<Result<_, String>>()?;
            let log: Value = read(&opts.value("--log")?)?;
            for r in live_report(&batches, &log, opts.harness()?, opts.os()?)? {
                println!("{}", serde_json::to_string(&r).map_err(|e| e.to_string())?);
            }
            Ok(ExitCode::SUCCESS)
        }
        "diff" => {
            let [live_path, suite_path] = opts.positional()?[..] else {
                return Err(USAGE.to_owned());
            };
            let leg: Option<ReportLeg> = opts
                .optional("--suite-leg")?
                .map(|l| parse(&l))
                .transpose()?;
            let timings: Option<TimingsMode> = opts
                .optional("--suite-timings")?
                .map(|t| parse(&t))
                .transpose()?;
            let live = report(live_path)?;
            check_live(&live)?;
            let mut suite = report(suite_path)?;
            if let Some(leg) = leg {
                suite.retain(|r| r.leg.leg == leg);
            }
            if let Some(t) = timings {
                // a record with no `timings` ran at scaled ones, as the gate reads it
                suite.retain(|r| r.leg.timings.unwrap_or(TimingsMode::Scaled) == t);
            }
            let before = suite.len();
            let suite = scope_suite(&live, suite);
            let d = diff(&live, &suite);
            print!("{}", d.render());
            println!(
                "suite side: {} of {before} records in the live report's scope",
                suite.len()
            );
            Ok(if d.agrees() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        _ => Err(USAGE.to_owned()),
    }
}

const USAGE: &str = "usage: live-pass brief --harness H --os OS [--json]\n       live-pass report --cells FILE... --log FILE --harness H --os OS\n       live-pass diff LIVE.jsonl SUITE.jsonl [--suite-leg LEG] [--suite-timings scaled|production]";

struct Opts<'a>(&'a [String]);

impl<'a> Opts<'a> {
    fn new(args: &'a [String]) -> Self {
        Opts(args)
    }
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    /// Every value of `name`, in order. An option with no value after it is an error.
    fn values(&self, name: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for (i, a) in self.0.iter().enumerate() {
            if a == name {
                match self.0.get(i + 1) {
                    Some(v) if !v.starts_with("--") => out.push(v.clone()),
                    _ => return Err(format!("{name} needs a value\n{USAGE}")),
                }
            }
        }
        Ok(out)
    }
    fn optional(&self, name: &str) -> Result<Option<String>, String> {
        let mut all = self.values(name)?;
        match all.len() {
            0 | 1 => Ok(all.pop()),
            _ => Err(format!("{name} is given once\n{USAGE}")),
        }
    }
    fn value(&self, name: &str) -> Result<String, String> {
        self.optional(name)?
            .ok_or_else(|| format!("{name} is required\n{USAGE}"))
    }
    fn harness(&self) -> Result<Dialect, String> {
        parse(&self.value("--harness")?)
    }
    fn os(&self) -> Result<Os, String> {
        parse(&self.value("--os")?)
    }
    /// The arguments that are neither an option nor an option's value. An option this tool does
    /// not have is an error.
    fn positional(&self) -> Result<Vec<&str>, String> {
        let mut out = Vec::new();
        let mut skip = false;
        for a in self.0 {
            if skip {
                skip = false;
            } else if matches!(a.as_str(), "--suite-leg" | "--suite-timings") {
                skip = true;
            } else if a.starts_with("--") {
                return Err(format!("`{a}` is not an option of this command\n{USAGE}"));
            } else {
                out.push(a.as_str());
            }
        }
        Ok(out)
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

/// A report: one JSON record per line, each checked against the leg-report schema.
fn report(path: &str) -> Result<Vec<purlis_scenarios::livepass::Recorded>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    read_report(path, &text)
}
