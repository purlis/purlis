// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The CI grader** (#5727 of spec #5714; ADR 0105 as amended 2026-11-08): the job that runs
//! a harness headless on built-in skills' trigger tests, among the crowd fixture and bare, and
//! puts the scores in the run's summary. What a run started, and how a skill scores against its
//! trigger bar, is [`purlis_skills::grade::ci`]'s; this crate decides what to grade
//! ([`select`]), starts the harness ([`launch`]), says the result ([`summary`]) and, each
//! night, decides which issues to file and whether to propose new pins ([`nightly`]). For a
//! developer, it proposes a built-in's description from its failing tests and grades it
//! ([`optimise`]).

pub mod launch;
pub mod nightly;
pub mod optimise;
pub mod select;
pub mod summary;

use std::path::{Path, PathBuf};

use purlis_skills::grade::ci;

/// **A grade to run**: one harness, the built-ins to grade, and where the runs' output is kept.
pub struct Ask<'a> {
    /// The harness graded: each is its own grade, with its own key and its own result.
    pub harness: ci::Harness,
    /// Its program to start: `claude`, `codex` or `opencode`.
    pub program: &'a Path,
    /// The `purlis` the plugin's hooks and the MCP server run.
    pub purlis: &'a Path,
    /// purlis's plugin: its manifest, hooks and built-in skills.
    pub plugin: &'a Path,
    /// The crowd fixture, a plugin of its own.
    pub crowd: &'a Path,
    /// The folder each run starts in: a plane `purlis init` made, so the briefing has one.
    pub cwd: &'a Path,
    /// The harness version the pin file names, said where the runs ran another.
    pub pinned: &'a str,
    /// The built-ins to grade ([`select::touched`]).
    pub skills: Vec<String>,
    /// The model provider's key, from the `skill-grades` environment: none, or blank, skips.
    pub key: Option<&'a str>,
    /// Where each run's output is written, one file per run.
    pub records: PathBuf,
    /// How many runs at once.
    pub parallel: usize,
}

/// **What a grade came to.**
#[derive(Debug)]
pub enum Graded {
    /// Nothing ran, and the line says why.
    Skipped(String),
    /// Every run ran: `passed` when every skill met its bar, the summary in Markdown, and the
    /// scorecard the nightly ([`nightly`]) reads.
    Done {
        passed: bool,
        summary: String,
        card: Box<ci::Scorecard>,
    },
}

/// **The `skill-grades` environment's secret a harness runs on** (spec #5714, decision Q10):
/// Codex on an OpenAI key, Claude Code and opencode on the Anthropic one.
pub fn key_name(harness: ci::Harness) -> &'static str {
    match harness {
        ci::Harness::Codex => "OPENAI_API_KEY",
        ci::Harness::ClaudeCode | ci::Harness::Opencode => "ANTHROPIC_API_KEY",
    }
}

/// The line a grade of `harness` without its key says.
pub fn no_key(harness: ci::Harness) -> String {
    format!(
        "Skill trigger grades on {} skipped: no `{}` from the `skill-grades` environment (it is \
         not set up, or this is a fork's pull request, which is never handed it), so nothing was \
         graded.",
        harness.name(),
        key_name(harness)
    )
}

/// **Grade `ask`'s built-ins on its harness**: each trigger test [`purlis_skills::grade::RUNS`]
/// times among the crowd and as many bare, scored by [`ci::score`].
pub fn grade(ask: &Ask<'_>) -> Graded {
    let Some(key) = ask.key.map(str::trim).filter(|key| !key.is_empty()) else {
        return Graded::Skipped(no_key(ask.harness));
    };
    let skills_dir = ask.plugin.join("skills");
    let graded: Vec<ci::Graded> = ask
        .skills
        .iter()
        .map(|skill| launch::graded(&skills_dir, skill))
        .collect();
    let runs = launch::run_all(ask, key, &graded);
    let built_ins = launch::built_ins(&skills_dir);
    let (version, model) = launch::ran_as(ask, &runs);
    let briefed = launch::briefed(ask);
    let card = ci::score(&ci::Grade {
        harness: ask.harness,
        version: version.as_deref().unwrap_or(ask.pinned),
        model: model.as_deref().unwrap_or(launch::MODEL_NOT_SAID),
        briefed: ci::Briefed {
            cwd: ask.cwd,
            skills: &briefed,
        },
        built_ins: &built_ins,
        skills: &graded,
        runs: &runs,
    });
    Graded::Done {
        passed: summary::passed(&card),
        summary: summary::markdown(&card, ask.pinned),
        card: Box::new(card),
    }
}
