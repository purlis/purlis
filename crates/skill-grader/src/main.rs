// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `skill-grader`: the CI grader's steps, as `.github/workflows/skill-grades.yml` runs them.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use purlis_skills::grade::ci::Harness;

#[derive(Parser)]
#[command(
    about = "The CI grader: what a change grades, and a grade of built-in skills on one harness"
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
        /// The harness: `claude-code`, `codex` or `opencode`.
        harness: String,
        #[arg(long, default_value = "ci/skill-grades/pins.toml")]
        pins: PathBuf,
    },
    /// Grade built-ins on one harness, the key from its variable (`ANTHROPIC_API_KEY` for
    /// Claude Code and opencode, `OPENAI_API_KEY` for Codex), and append the result to the
    /// summary file. Exits 1 when a skill is below its bar or not scored.
    Grade {
        /// The built-ins to grade, as `select` printed them.
        #[arg(long, value_delimiter = ',', required = true)]
        skills: Vec<String>,
        /// The harness: `claude-code`, `codex` or `opencode`.
        #[arg(long, default_value = "claude-code", value_parser = harness)]
        harness: Harness,
        /// Its program: `claude`, `codex` or `opencode`.
        #[arg(long)]
        program: PathBuf,
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
            harness,
            program,
            purlis,
            plugin,
            crowd,
            cwd,
            pins,
            records,
            summary,
            parallel,
        } => {
            let pin = match pinned(&pins, harness.word()) {
                Ok(pin) => pin,
                Err(why) => return fail(&why),
            };
            // Each run starts in `cwd`, so every path it is handed is whole.
            let whole = |path: PathBuf| std::path::absolute(&path).unwrap_or(path);
            let (program, purlis, plugin, crowd, records) = (
                whole(program),
                whole(purlis),
                whole(plugin),
                whole(crowd),
                whole(records),
            );
            let key = std::env::var(skill_grader::key_name(harness)).ok();
            let ask = skill_grader::Ask {
                harness,
                program: &program,
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

/// A harness by its word in a result.
fn harness(word: &str) -> Result<Harness, String> {
    [Harness::ClaudeCode, Harness::Codex, Harness::Opencode]
        .into_iter()
        .find(|harness| harness.word() == word)
        .ok_or_else(|| format!("`{word}` is none of claude-code, codex, opencode"))
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
