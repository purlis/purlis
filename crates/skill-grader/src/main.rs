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
        /// The file the scorecard is written to, as JSON, for the nightly to read.
        #[arg(long)]
        scorecard: Option<PathBuf>,
    },
    /// Propose a rewritten description for one built-in from its failing trigger tests, grade
    /// it, and print both scores. A developer's tool, run on your own keys through `purlis
    /// secret exec`: the harness's key as `grade` reads it, and `ANTHROPIC_API_KEY` for the
    /// Claude Code that proposes. It writes nothing unless given `--write`.
    Optimise {
        /// The built-in whose description is proposed.
        skill: String,
        #[arg(long, default_value = "claude-code", value_parser = harness)]
        harness: Harness,
        /// The graded harness's program: `claude`, `codex` or `opencode`.
        #[arg(long)]
        program: PathBuf,
        /// The Claude Code that writes the proposal: `--program` where that is Claude Code.
        #[arg(long)]
        proposer: Option<PathBuf>,
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
        /// Where each run's output is kept: `current/` and `proposed/`.
        #[arg(long)]
        records: PathBuf,
        #[arg(long, default_value_t = 6)]
        parallel: usize,
        /// Put the proposal in the skill's `SKILL.md`.
        #[arg(long)]
        write: bool,
    },
    /// The nightly's decisions, from each harness's scorecard and the open `skill-triggers`
    /// issues on stdin (one JSON object per line, `number` and `title`): one line per act on
    /// stdout, `<verb>\t<issue>\t<file>\t<title>`, with `verb` one of `create`, `edit`,
    /// `close` (the file is the closing comment) and `bump` (the pin file is rewritten in
    /// place and the file is the pull request's description).
    Nightly {
        /// A folder of scorecards, one JSON file per harness graded; missing when none was.
        #[arg(long)]
        scorecards: PathBuf,
        #[arg(long, default_value = "ci/skill-grades/pins.toml")]
        pins: PathBuf,
        /// The run's page, named in every issue and in the bump.
        #[arg(long)]
        run_url: String,
        /// The folder each issue's body and the bump's description are written to.
        #[arg(long)]
        out: PathBuf,
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
            scorecard,
        } => {
            let pin = match pinned(&pins, skill_grader::word(harness)) {
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
                skill_grader::Graded::Done {
                    passed,
                    summary,
                    card,
                } => {
                    if let Some(file) = &scorecard {
                        let written = file
                            .parent()
                            .map_or(Ok(()), std::fs::create_dir_all)
                            .map_err(|why| why.to_string())
                            .and_then(|()| {
                                serde_json::to_string_pretty(&card).map_err(|why| why.to_string())
                            })
                            .map_err(|why| why.to_string())
                            .and_then(|json| {
                                std::fs::write(file, json).map_err(|why| why.to_string())
                            });
                        if let Err(why) = written {
                            return fail(&format!("the scorecard was not written: {why}"));
                        }
                    }
                    (
                        summary,
                        if passed {
                            ExitCode::SUCCESS
                        } else {
                            ExitCode::FAILURE
                        },
                    )
                }
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
        Step::Optimise {
            skill,
            harness,
            program,
            proposer,
            purlis,
            plugin,
            crowd,
            cwd,
            pins,
            records,
            parallel,
            write,
        } => {
            let pin = match pinned(&pins, skill_grader::word(harness)) {
                Ok(pin) => pin,
                Err(why) => return fail(&why),
            };
            let proposer = match (proposer, harness) {
                (Some(proposer), _) => proposer,
                (None, Harness::ClaudeCode) => program.clone(),
                (None, _) => {
                    return fail(&format!(
                        "{} does not propose: name a Claude Code with `--proposer`",
                        harness.shown()
                    ));
                }
            };
            let whole = |path: PathBuf| std::path::absolute(&path).unwrap_or(path);
            let (program, proposer, purlis, plugin, crowd, records) = (
                whole(program),
                whole(proposer),
                whole(purlis),
                whole(plugin),
                whole(crowd),
                whole(records),
            );
            let key = std::env::var(skill_grader::key_name(harness)).ok();
            let proposer_key = std::env::var("ANTHROPIC_API_KEY").ok();
            let asked = skill_grader::optimise::Optimise {
                ask: skill_grader::Ask {
                    harness,
                    program: &program,
                    purlis: &purlis,
                    plugin: &plugin,
                    crowd: &crowd,
                    cwd: &cwd,
                    pinned: &pin,
                    skills: vec![skill.clone()],
                    key: key.as_deref(),
                    records,
                    parallel,
                },
                skill: &skill,
                proposer: &proposer,
                proposer_key: proposer_key.as_deref(),
                write,
            };
            match skill_grader::optimise::optimise(&asked) {
                // Run by hand, a missing key is a mistake to say, not a grade to skip.
                Ok(skill_grader::optimise::Optimised::Skipped(said)) => fail(&said),
                Ok(
                    skill_grader::optimise::Optimised::Nothing(said)
                    | skill_grader::optimise::Optimised::Proposed { said, .. },
                ) => {
                    println!("{said}");
                    ExitCode::SUCCESS
                }
                Err(why) => fail(&why),
            }
        }
        Step::Nightly {
            scorecards,
            pins,
            run_url,
            out,
        } => match nightly(&scorecards, &pins, &run_url, &out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(why) => fail(&why),
        },
    }
}

/// The nightly's acts, as [`Step::Nightly`] says them.
fn nightly(
    scorecards: &std::path::Path,
    pins: &std::path::Path,
    run_url: &str,
    out: &std::path::Path,
) -> Result<(), String> {
    use skill_grader::nightly::{self, Act};

    let mut cards: Vec<purlis_skills::grade::ci::Report> = Vec::new();
    if scorecards.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(scorecards)
            .map_err(|why| format!("{} could not be read: {why}", scorecards.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file)
                .map_err(|why| format!("{} could not be read: {why}", file.display()))?;
            cards.push(
                serde_json::from_str(&text)
                    .map_err(|why| format!("{} is not a scorecard: {why}", file.display()))?,
            );
        }
    }
    let mut open = Vec::new();
    for line in std::io::stdin().lines() {
        let line = line.map_err(|why| format!("the open issues could not be read: {why}"))?;
        if !line.trim().is_empty() {
            open.push(
                serde_json::from_str::<nightly::OpenIssue>(&line)
                    .map_err(|why| format!("an open issue is not `number` and `title`: {why}"))?,
            );
        }
    }
    std::fs::create_dir_all(out)
        .map_err(|why| format!("{} could not be made: {why}", out.display()))?;
    let mut said = String::new();
    let write = |n: usize, text: &str| -> Result<PathBuf, String> {
        let file = out.join(format!("{n}.md"));
        std::fs::write(&file, text)
            .map_err(|why| format!("{} was not written: {why}", file.display()))?;
        Ok(file)
    };
    let acts = nightly::issues(&cards, &open, run_url);
    for (n, act) in acts.iter().enumerate() {
        let line = match act {
            Act::Create { title, body } => {
                format!("create\t-\t{}\t{title}", write(n, body)?.display())
            }
            Act::Edit {
                number,
                title,
                body,
            } => format!("edit\t{number}\t{}\t{title}", write(n, body)?.display()),
            Act::Close {
                number,
                title,
                comment,
            } => format!("close\t{number}\t{}\t{title}", write(n, comment)?.display()),
        };
        said.push_str(&line);
        said.push('\n');
    }
    let text = std::fs::read_to_string(pins)
        .map_err(|why| format!("{} could not be read: {why}", pins.display()))?;
    if let Some(bump) = nightly::bump(&text, &cards)? {
        std::fs::write(pins, &bump.pins)
            .map_err(|why| format!("{} was not written: {why}", pins.display()))?;
        let description = format!(
            "Every built-in skill met its trigger bar on the latest version of every harness \
             (spec #5714, decision Q23), so the pull requests' grades move to those versions:\n\n\
             {}\nThe nightly: {run_url}\n",
            bump.said
        );
        let file = write(acts.len(), &description)?;
        said.push_str(&format!(
            "bump\t-\t{}\tskill-grades: pin the harness versions the nightly graded green\n",
            file.display()
        ));
    }
    print!("{said}");
    Ok(())
}

/// A harness by its word in a result.
fn harness(word: &str) -> Result<Harness, String> {
    [Harness::ClaudeCode, Harness::Codex, Harness::Opencode]
        .into_iter()
        .find(|harness| skill_grader::word(*harness) == word)
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
