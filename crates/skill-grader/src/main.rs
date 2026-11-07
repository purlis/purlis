// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `skill-grader`: the CI grader's steps, as `.github/workflows/skill-grades.yml` runs them.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    about = "The CI grader: what a change grades, and a grade of built-in skills on Claude Code"
)]
struct Cli {
    #[command(subcommand)]
    command: Step,
}

#[derive(Subcommand)]
enum Step {
    /// Print the built-ins a change grades, one per line, given its changed paths one per line.
    Select {
        /// A file of the change's paths, one per line.
        #[arg(long)]
        changed: PathBuf,
        /// purlis's plugin folder.
        #[arg(long, default_value = "app/src-tauri/plugin")]
        plugin: PathBuf,
    },
    /// Print the version the pin file names for a harness.
    Pin {
        /// The harness: `claude-code`.
        harness: String,
        #[arg(long, default_value = "ci/skill-grades/pins.toml")]
        pins: PathBuf,
    },
    /// Grade built-ins on Claude Code, the key from `ANTHROPIC_API_KEY`, and append the result
    /// to the summary file. Exits 1 when a skill is below its bar or not scored.
    Grade {
        /// The built-ins to grade, as `select` printed them.
        #[arg(long, value_delimiter = ',', required = true)]
        skills: Vec<String>,
        #[arg(long, default_value = "claude")]
        claude: PathBuf,
        #[arg(long)]
        purlis: PathBuf,
        #[arg(long, default_value = "app/src-tauri/plugin")]
        plugin: PathBuf,
        #[arg(long, default_value = "ci/skill-grades/crowd")]
        crowd: PathBuf,
        /// The folder each run starts in.
        #[arg(long)]
        cwd: PathBuf,
        #[arg(long, default_value = "ci/skill-grades/pins.toml")]
        pins: PathBuf,
        /// Where each run's output is kept.
        #[arg(long)]
        records: PathBuf,
        /// The file the result is appended to: `$GITHUB_STEP_SUMMARY` in CI.
        #[arg(long)]
        summary: Option<PathBuf>,
        #[arg(long, default_value_t = 6)]
        parallel: usize,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Step::Select { changed, plugin } => select(&changed, &plugin),
        Step::Pin { harness, pins } => match pinned(&pins, &harness) {
            Ok(version) => {
                println!("{version}");
                ExitCode::SUCCESS
            }
            Err(why) => fail(&why),
        },
        Step::Grade {
            skills,
            claude,
            purlis,
            plugin,
            crowd,
            cwd,
            pins,
            records,
            summary,
            parallel,
        } => {
            let pin = match pinned(&pins, "claude-code") {
                Ok(pin) => pin,
                Err(why) => return fail(&why),
            };
            let key = std::env::var("ANTHROPIC_API_KEY").ok();
            let ask = skill_grader::Ask {
                claude: &claude,
                purlis: &purlis,
                plugin: &plugin,
                crowd: &crowd,
                cwd: &cwd,
                pinned: &pin,
                skills,
                key: key.as_deref(),
                records,
                parallel,
            };
            let (text, code) = match skill_grader::grade(&ask) {
                skill_grader::Graded::Skipped(line) => (line, ExitCode::SUCCESS),
                skill_grader::Graded::Done { passed, summary } => (
                    summary,
                    if passed {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    },
                ),
            };
            println!("{text}");
            if let Some(file) = summary {
                let appended = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file)
                    .and_then(|mut out| writeln!(out, "{text}"));
                if let Err(why) = appended {
                    return fail(&format!("the summary was not written: {why}"));
                }
            }
            code
        }
    }
}

fn fail(why: &str) -> ExitCode {
    eprintln!("skill-grader: {why}");
    ExitCode::FAILURE
}

/// The version `pins` names for `harness`, under `[harness]`.
fn pinned(pins: &std::path::Path, harness: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(pins)
        .map_err(|why| format!("{} could not be read: {why}", pins.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|why| format!("{} is not TOML: {why}", pins.display()))?;
    table
        .get("harness")
        .and_then(|harnesses| harnesses.get(harness))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} pins no version for {harness}", pins.display()))
}

/// The built-ins with trigger tests, of those the change touches.
fn select(changed: &std::path::Path, plugin: &std::path::Path) -> ExitCode {
    let text = match std::fs::read_to_string(changed) {
        Ok(text) => text,
        Err(why) => return fail(&format!("{} could not be read: {why}", changed.display())),
    };
    let paths: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let skills_dir = plugin.join("skills");
    let graded: Vec<String> = skill_grader::launch::built_ins(&skills_dir)
        .into_iter()
        .filter(|skill| {
            skills_dir
                .join(skill)
                .join(purlis_skills::grade::triggers_path())
                .is_file()
        })
        .collect();
    let touched = skill_grader::select::touched(&paths, &graded);
    match &touched {
        skill_grader::select::Touched::Nothing => {
            eprintln!("No built-in with trigger tests is touched: nothing to grade.");
        }
        skill_grader::select::Touched::These(_) => {
            eprintln!("Graded: the built-ins whose own folders the change touches.");
        }
        skill_grader::select::Touched::Every { because, .. } => {
            eprintln!("Every built-in with trigger tests is graded: {because}.");
        }
    }
    for skill in touched.skills() {
        println!("{skill}");
    }
    ExitCode::SUCCESS
}
