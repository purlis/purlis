// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Starting Claude Code headless** on each trigger test, as a chat in the window is armed:
//! purlis's plugin (its hooks, so the session-start briefing, and its skills), purlis's MCP
//! server as a grade's chat has it (the reads alone, SK-16) and no other MCP server; among the crowd, the crowd fixture's plugin beside it.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_skills::grade::{RUNS, ci};

use crate::Ask;

/// The most agentic turns a run may take: a run that loops is cut short, and a run cut short is
/// no run ([`ci::Parsed::ran`]), never a "did not trigger".
pub const MOST_TURNS: &str = "8";

/// Claude Code's limit on its skill listing, as a chat in the window is handed it: purlis's
/// least (`LEAST_LISTING_LIMIT` in `purlis_core::harness::claude`, #4610), which the built-ins and
/// the crowd fit under. #5736 counts a person's own skills into it; this follows.
pub const LISTING_LIMIT: (&str, &str) = ("SLASH_COMMAND_TOOL_CHAR_BUDGET", "40000");

/// `skill`'s trigger tests and sources file, as its folder under `skills_dir` holds them: none
/// where it has none, which the score reports rather than scores.
pub fn graded(skills_dir: &Path, skill: &str) -> ci::Graded {
    let fixtures = purlis_skills::grade::fixtures_of(skills_dir, Path::new(skill), skill);
    let tests = fixtures.triggers.read().cloned().unwrap_or_default();
    let sources = std::fs::read_to_string(skills_dir.join(skill).join(ci::sources_path())).ok();
    ci::Graded {
        skill: skill.to_owned(),
        tests,
        sources,
    }
}

/// Every built-in's name: each folder of `skills_dir` holding a `SKILL.md`, sorted.
pub fn built_ins(skills_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(skills_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().join("SKILL.md").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

/// One run to start.
struct Job<'a> {
    skill: &'a str,
    test: usize,
    run: u8,
    listing: ci::Listing,
    query: &'a str,
}

/// **The words `claude` is started with** for one run of `query`.
pub fn args(ask: &Ask<'_>, listing: ci::Listing, query: &str) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose"]
        .map(str::to_owned)
        .to_vec();
    args.push("--plugin-dir".to_owned());
    args.push(ask.plugin.display().to_string());
    if listing == ci::Listing::Crowded {
        args.push("--plugin-dir".to_owned());
        args.push(ask.crowd.display().to_string());
    }
    // purlis's server alone, as a grade's chat in the window (SK-16). Its settings files load as
    // that chat's do: the plane's own, and none of the runner's, which has none.
    args.push("--strict-mcp-config".to_owned());
    args.push("--mcp-config".to_owned());
    args.push(purlis_core::chattools::claude_code_config_as(
        ask.purlis, true,
    ));
    // After `--mcp-config`, which takes every word up to the next flag.
    args.extend(["--max-turns", MOST_TURNS].map(str::to_owned));
    args.push(query.to_owned());
    args
}

/// **Every run of `graded`'s trigger tests**, crowded and bare, `ask.parallel` at a time, each
/// one's output kept under `ask.records`.
pub fn run_all(ask: &Ask<'_>, key: &str, graded: &[ci::Graded]) -> Vec<ci::Recorded> {
    let mut jobs = Vec::new();
    for listing in [ci::Listing::Crowded, ci::Listing::Bare] {
        for skill in graded {
            for (test, one) in skill.tests.iter().enumerate() {
                for run in 0..RUNS {
                    jobs.push(Job {
                        skill: &skill.skill,
                        test,
                        run,
                        listing,
                        query: &one.query,
                    });
                }
            }
        }
    }
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for _ in 0..ask.parallel.max(1) {
            scope.spawn(|| {
                while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let recorded = run_one(ask, key, job);
                    done.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(recorded);
                }
            });
        }
    });
    done.into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One run: its output, kept on disk too. A run that could not start wrote nothing, and is
/// scored as cut short.
fn run_one(ask: &Ask<'_>, key: &str, job: &Job<'_>) -> ci::Recorded {
    let listing = match job.listing {
        ci::Listing::Crowded => "crowded",
        ci::Listing::Bare => "bare",
    };
    let output = Command::new(ask.claude)
        .args(args(ask, job.listing, job.query))
        .current_dir(ask.cwd)
        // The key in the environment alone, never on a command line.
        .env("ANTHROPIC_API_KEY", key)
        .env(purlis_core::plugin::BINARY_ENV, ask.purlis)
        .env(LISTING_LIMIT.0, LISTING_LIMIT.1)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output();
    let output = match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(why) => {
            eprintln!(
                "skill-grader: {} did not start: {why}",
                ask.claude.display()
            );
            String::new()
        }
    };
    let folder = ask.records.join(job.skill).join(listing);
    let kept = std::fs::create_dir_all(&folder).and_then(|()| {
        std::fs::write(
            folder.join(format!("{:02}-{}.jsonl", job.test + 1, job.run + 1)),
            &output,
        )
    });
    if let Err(why) = kept {
        eprintln!("skill-grader: a run's output was not kept: {why}");
    }
    ci::Recorded {
        skill: job.skill.to_owned(),
        test: job.test,
        listing: job.listing,
        output,
    }
}

/// **What the runs ran as**: the Claude Code version and model the first run that said so
/// named in its `init` line.
pub fn ran_as(runs: &[ci::Recorded]) -> (Option<String>, Option<String>) {
    for run in runs {
        for line in run.output.lines() {
            let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if event["type"] == "system" && event["subtype"] == "init" {
                let said = |key: &str| event[key].as_str().map(str::to_owned);
                return (said("claude_code_version"), said("model"));
            }
        }
    }
    (None, None)
}
