// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Starting a harness headless** on each trigger test, as a chat in the window is armed.
//!
//! - **Claude Code**: purlis's plugin (its hooks, so the session-start briefing, and its
//!   skills), purlis's MCP server as a grade's chat has it (the reads alone, SK-16) and no other
//!   MCP server; among the crowd, the crowd fixture's plugin beside it.
//! - **Codex** and **opencode** (#5728): whatever purlis arms a chat of theirs with
//!   ([`purlis_core::harness::Harness::state_hooks`]), so the grade's briefing is a real
//!   chat's by construction; among the crowd, the crowd's skills where each finds a person's
//!   own.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use purlis_core::harness::{Harness, Kit, StateHooks};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use purlis_skills::grade::{RUNS, TriggerTest, ci};

use crate::Ask;

/// The most agentic turns a run may take: a run that loops is cut short, and what it started
/// before then is what it started, as [`ci::started`] reads it.
pub const MOST_TURNS: &str = "8";

/// Claude Code's limit on its skill listing, as a chat in the window is handed it: purlis's
/// least (`LEAST_LISTING_LIMIT` in `purlis_core::harness::claude`, #4610), which the built-ins and
/// the crowd fit under. #5736 counts a person's own skills into it, and a CI runner has none.
pub const LISTING_LIMIT: (&str, &str) = ("SLASH_COMMAND_TOOL_CHAR_BUDGET", "40000");

/// **A built-in to grade**: its name, its trigger tests read, and its trigger tests' and sources
/// file's texts as [`ci::score`] takes them, each none where it has none.
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    /// The trigger tests each run is started on: none where they do not read.
    pub tests: Vec<TriggerTest>,
    pub triggers: Option<String>,
    pub sources: Option<String>,
}

/// `skill`'s trigger tests and sources file, as its folder under `skills_dir` holds them: none
/// where it has none, which the score reports rather than scores.
pub fn graded(skills_dir: &Path, skill: &str) -> Skill {
    let fixtures = purlis_skills::grade::fixtures_of(skills_dir, Path::new(skill), skill);
    let tests = fixtures.triggers.read().cloned().unwrap_or_default();
    let folder = skills_dir.join(skill);
    let text = |path: &str| std::fs::read_to_string(folder.join(path)).ok();
    Skill {
        name: skill.to_owned(),
        tests,
        triggers: text(&purlis_skills::grade::triggers_path()),
        sources: text(ci::SOURCES_PATH),
    }
}

/// **`skills` scored from `runs`** by [`ci::score`], in the order given: one without trigger
/// tests is reported, not scored.
pub fn score(on: ci::On, skills: &[Skill], runs: &[ci::Run]) -> ci::Report {
    let built_ins: Vec<ci::BuiltIn<'_>> = skills
        .iter()
        .filter_map(|skill| {
            skill.triggers.as_deref().map(|triggers| ci::BuiltIn {
                name: &skill.name,
                triggers,
                sources: skill.sources.as_deref(),
            })
        })
        .collect();
    let mut report = ci::score(on, &built_ins, runs);
    let mut scored = std::mem::take(&mut report.skills).into_iter();
    report.skills = skills
        .iter()
        .filter_map(|skill| match skill.triggers {
            Some(_) => scored.next(),
            None => Some(ci::Graded::NotScored {
                skill: skill.name.clone(),
                why: format!(
                    "it has no {}, so nothing to score",
                    purlis_skills::grade::triggers_path()
                ),
            }),
        })
        .collect();
    report
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
    setting: ci::Setting,
    query: &'a str,
}

/// **One run that ran**: what it read as, and what its harness printed.
pub struct Ran {
    pub run: ci::Run,
    pub output: String,
}

/// **The words `claude` is started with** for one run of `query`.
pub fn args(ask: &Ask<'_>, setting: ci::Setting, query: &str) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose"]
        .map(str::to_owned)
        .to_vec();
    args.push("--plugin-dir".to_owned());
    args.push(ask.plugin.display().to_string());
    if setting == ci::Setting::Crowded {
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
/// one's output kept under `ask.records` and read against `briefed`, the skills a Codex run's
/// briefing lists ([`briefed`]).
pub fn run_all(
    ask: &Ask<'_>,
    key: &str,
    graded: &[Skill],
    briefed: &[purlis_skills::Skill],
) -> Vec<Ran> {
    let mut jobs = Vec::new();
    for setting in [ci::Setting::Crowded, ci::Setting::Bare] {
        for skill in graded {
            for (test, one) in skill.tests.iter().enumerate() {
                for run in 0..RUNS {
                    jobs.push(Job {
                        skill: &skill.name,
                        test,
                        run,
                        setting,
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
                    let recorded = run_one(ask, key, job, briefed);
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

/// One run: its output, kept on disk too, and what it read as. A run that could not start
/// wrote nothing, which reads as no run.
fn run_one(ask: &Ask<'_>, key: &str, job: &Job<'_>, briefed: &[purlis_skills::Skill]) -> Ran {
    let name = format!("{:02}-{}", job.test + 1, job.run + 1);
    let started = match ask.harness {
        ci::Harness::ClaudeCode => Ok(claude_code(ask, key, job.setting, job.query)),
        ci::Harness::Codex => codex(ask, key, job.setting, job.query, &home_of(ask, job, &name)),
        ci::Harness::Opencode => opencode(ask, key, job.setting, job.query),
    };
    let output = started.and_then(|mut command| {
        command
            .current_dir(ask.cwd)
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
    });
    let output = match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(why) => {
            eprintln!(
                "skill-grader: {} did not start: {why}",
                ask.program.display()
            );
            String::new()
        }
    };
    let folder = ask.records.join(job.skill).join(job.setting.word());
    let kept = std::fs::create_dir_all(&folder)
        .and_then(|()| std::fs::write(folder.join(format!("{name}.jsonl")), &output));
    if let Err(why) = kept {
        eprintln!("skill-grader: a run's output was not kept: {why}");
    }
    let read = ci::started(
        ask.harness,
        &output,
        ci::Listing {
            cwd: ask.cwd,
            briefed,
        },
    );
    Ran {
        run: ci::Run {
            setting: job.setting,
            query: job.query.to_owned(),
            read,
        },
        output,
    }
}

/// Claude Code, started with [`args`]: the key in the environment alone, never on a command
/// line.
fn claude_code(ask: &Ask<'_>, key: &str, setting: ci::Setting, query: &str) -> Command {
    let mut command = Command::new(ask.program);
    command
        .args(args(ask, setting, query))
        .env("ANTHROPIC_API_KEY", key)
        .env(purlis_core::plugin::BINARY_ENV, ask.purlis)
        .env(LISTING_LIMIT.0, LISTING_LIMIT.1);
    command
}

/// **What a chat in the window on `harness` is armed with**: the arguments and environment
/// [`purlis_core::harness::Harness::state_hooks`] hands it, for a grade's chat (SK-16) on no
/// persona, with the built-in skills alone and none of the person's plugins turned off.
fn armed(ask: &Ask<'_>, harness: Harness) -> (Vec<String>, Vec<(String, String)>) {
    let kit = Kit {
        binary: ask.purlis,
        plugin: Some(ask.plugin),
        persona: None,
        skills: None,
        graded: true,
        judging: false,
        writes_its_piece: false,
        runs_a_shell: false,
        // No plane's skills, so none of the person's own are read into Claude Code's listing
        // limit (#5736): a CI runner has none, and the limit is purlis's alone.
        laid_out: None,
    };
    match harness.state_hooks(
        kit,
        Some(ask.cwd),
        &purlis_core::harness_plugin::Chosen::new(),
        None,
    ) {
        StateHooks::ThisSessionOnly { args, env, .. } => (args, env),
        StateHooks::None => (Vec::new(), Vec::new()),
    }
}

/// Codex's own words for a headless run: its events as JSON lines, no session kept, any folder,
/// and the hooks on its session flags run without the trust a person gives them in its TUI.
/// That trust is what keeps them inert otherwise (`codex exec` says nothing), and without the
/// `SessionStart` hook there is no briefing and no built-in is listed. Read from
/// `codex-rs/utils/cli/src/shared_options.rs` at `rust-v0.167.0`.
pub const CODEX_EXEC: [&str; 5] = [
    "exec",
    "--json",
    "--ephemeral",
    "--skip-git-repo-check",
    "--dangerously-bypass-hook-trust",
];

/// **Codex, armed as a chat in the window**: purlis's `-c` session flags before `exec`, which
/// takes the root's (`prepend_config_flags`, `cli/src/main.rs`), and their environment, which
/// names the built-in skills' folder its briefing lists. The key goes in `CODEX_API_KEY`, which
/// `codex exec` reads before any login (`login/src/auth/manager.rs`), and `home` is its
/// `CODEX_HOME`: a person's own skills are there, so the crowd is too where it is crowded.
fn codex(
    ask: &Ask<'_>,
    key: &str,
    setting: ci::Setting,
    query: &str,
    home: &Path,
) -> std::io::Result<Command> {
    let skills = home.join("skills");
    std::fs::create_dir_all(&skills)?;
    if setting == ci::Setting::Crowded {
        copy_crowd(&ask.crowd.join("skills"), &skills)?;
    }
    let (args, env) = armed(ask, Harness::Codex);
    let mut command = Command::new(ask.program);
    command
        .args(args)
        .args(CODEX_EXEC)
        .arg(query)
        .envs(env)
        .env("CODEX_API_KEY", key)
        .env("CODEX_HOME", home)
        .env(purlis_core::plugin::BINARY_ENV, ask.purlis);
    Ok(command)
}

/// Each crowd skill's folder, its files copied into `to`: a copy, not a link, so nothing Codex
/// writes in its home reaches the repo's crowd fixture.
fn copy_crowd(crowd: &Path, to: &Path) -> std::io::Result<()> {
    for skill in std::fs::read_dir(crowd)? {
        let skill = skill?;
        if !skill.file_type()?.is_dir() {
            continue;
        }
        let into = to.join(skill.file_name());
        std::fs::create_dir_all(&into)?;
        for file in std::fs::read_dir(skill.path())? {
            let file = file?;
            if file.file_type()?.is_file() {
                std::fs::copy(file.path(), into.join(file.file_name()))?;
            }
        }
    }
    Ok(())
}

/// The `CODEX_HOME` of one run: its own, so no two runs share Codex's state, and beside the
/// records rather than in them, so what Codex writes there is never uploaded.
fn home_of(ask: &Ask<'_>, job: &Job<'_>, name: &str) -> PathBuf {
    ask.records
        .with_extension("codex-homes")
        .join(job.skill)
        .join(job.setting.word())
        .join(name)
}

/// **opencode, armed as a chat in the window**: purlis's whole config in
/// `OPENCODE_CONFIG_CONTENT`, the shim that carries the hooks and adds the built-in skills to
/// those it discovers. Where crowded, the crowd's skills are in the same config's `skills.paths`,
/// where a person's own skill folders are named, and the shim appends purlis's after them.
fn opencode(
    ask: &Ask<'_>,
    key: &str,
    setting: ci::Setting,
    query: &str,
) -> std::io::Result<Command> {
    let (args, mut env) = armed(ask, Harness::Opencode);
    if setting == ci::Setting::Crowded {
        let crowd = std::path::absolute(ask.crowd.join("skills"))?;
        for (name, value) in &mut env {
            if name != purlis_core::opencode::CONFIG_ENV {
                continue;
            }
            let mut config: serde_json::Value = serde_json::from_str(value)?;
            config["skills"]["paths"] = serde_json::json!([crowd.display().to_string()]);
            *value = config.to_string();
        }
    }
    let mut command = Command::new(ask.program);
    command
        .args(args)
        .args(["run", "--format", "json"])
        .arg(query)
        .envs(env)
        .env("ANTHROPIC_API_KEY", key);
    Ok(command)
}

/// **The skills a Codex run's briefing lists, each by its `SKILL.md`**: the built-ins in the
/// folder its arming names, which is what a read is matched against ([`ci::Listing`]). Empty
/// for the harnesses that name the skill they start.
pub fn briefed(ask: &Ask<'_>) -> Vec<purlis_skills::Skill> {
    if ask.harness != ci::Harness::Codex {
        return Vec::new();
    }
    let (_, env) = armed(ask, Harness::Codex);
    env.iter()
        .find(|(name, _)| name == purlis_core::skills::LISTED_ENV)
        .map(|(_, dir)| purlis_skills::read(Path::new(dir)))
        .unwrap_or_default()
}

/// The model a grade says where its harness's output does not name one: Codex's and opencode's
/// JSON never do, and each runs on its default.
pub const MODEL_NOT_SAID: &str = "its default model, which its output does not name";

/// **What the runs ran as**: on Claude Code, the version and model the first run that said so
/// named in its `init` line; on Codex and opencode, which say neither, the version the program
/// says it is (`codex-cli 0.167.0`, `1.19.5`) and no model.
pub fn ran_as(ask: &Ask<'_>, runs: &[Ran]) -> (Option<String>, Option<String>) {
    if ask.harness != ci::Harness::ClaudeCode {
        let said = Command::new(ask.program)
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
        let version = said.and_then(|said| said.split_whitespace().last().map(str::to_owned));
        return (version, None);
    }
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
