// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The CI grader** (#5727 of spec #5714; ADR 0105 as amended 2026-11-08): the job that runs
//! a harness headless on built-in skills' trigger tests, among the crowd fixture and bare, and
//! puts the scores in the run's summary. What a run started, and how a skill scores against its
//! trigger bar, is [`purlis_skills::grade::ci`]'s; this crate decides what to grade
//! ([`select`]), starts the harness ([`launch`]) and says the result ([`summary`]).

pub mod launch;
pub mod select;
pub mod summary;

use std::path::{Path, PathBuf};

use purlis_skills::grade::ci;

/// **A grade to run**: one harness, the built-ins to grade, and where the runs' output is kept.
pub struct Ask<'a> {
    /// The `claude` to start.
    pub claude: &'a Path,
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
    /// Every run ran: `passed` when every skill met its bar, and the summary in Markdown.
    Done { passed: bool, summary: String },
}

/// The line a grade without its key says.
pub const NO_KEY: &str = "Skill trigger grades skipped: no `ANTHROPIC_API_KEY` from the \
`skill-grades` environment (it is not set up, or this is a fork's pull request, which is never \
handed it), so nothing was graded.";

/// **Grade `ask`'s built-ins on Claude Code**: each trigger test [`purlis_skills::grade::RUNS`]
/// times among the crowd and as many bare, scored by [`ci::score`].
pub fn grade(ask: &Ask<'_>) -> Graded {
    let Some(key) = ask.key.map(str::trim).filter(|key| !key.is_empty()) else {
        return Graded::Skipped(NO_KEY.to_owned());
    };
    let skills_dir = ask.plugin.join("skills");
    let graded: Vec<ci::Graded> = ask
        .skills
        .iter()
        .map(|skill| launch::graded(&skills_dir, skill))
        .collect();
    let runs = launch::run_all(ask, key, &graded);
    let built_ins = launch::built_ins(&skills_dir);
    let (version, model) = launch::ran_as(&runs);
    let card = ci::score(&ci::Grade {
        harness: ci::Harness::ClaudeCode,
        version: version.as_deref().unwrap_or(ask.pinned),
        model: model.as_deref().unwrap_or("unknown"),
        briefed: ci::Briefed {
            cwd: ask.cwd,
            skills: &[],
        },
        built_ins: &built_ins,
        skills: &graded,
        runs: &runs,
    });
    Graded::Done {
        passed: summary::passed(&card),
        summary: summary::markdown(&card, ask.pinned),
    }
}
