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
    /// Whether the harness installed is its latest rather than the pin, as each night's is: a
    /// version the runs do not say is then [`LATEST_UNKNOWN`], never the pin.
    pub latest: bool,
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
    /// report the nightly ([`nightly`]) reads.
    Done {
        passed: bool,
        summary: String,
        card: Box<ci::Report>,
    },
}

/// **The version recorded for a night's latest harness that did not say its version**: never
/// the pin, which it may not be, and never proposed as one ([`nightly::bump`]).
pub const LATEST_UNKNOWN: &str = "latest (unknown)";

/// **A harness by its word**, as the pin file, the CLI and an artifact's name spell it:
/// `claude-code`, `codex` or `opencode`, its serde name in a report.
pub fn word(harness: ci::Harness) -> &'static str {
    match harness {
        ci::Harness::ClaudeCode => "claude-code",
        ci::Harness::Codex => "codex",
        ci::Harness::Opencode => "opencode",
    }
}

/// **The `skill-grades` environment's secret a harness runs on** (spec #5714, decision Q10):
/// Codex on an OpenAI key, Claude Code and opencode on the Anthropic one.
pub fn key_name(harness: ci::Harness) -> &'static str {
    match harness {
        ci::Harness::Codex => "OPENAI_API_KEY",
        ci::Harness::ClaudeCode | ci::Harness::Opencode => "ANTHROPIC_API_KEY",
    }
}

/// **`text` with each of `keys` replaced** by `[a key]`: what a run printed, before it is
/// written anywhere, and whatever the optimiser says. A blank key replaces nothing.
pub fn scrubbed(text: &str, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .fold(text.to_owned(), |text, key| text.replace(key, "[a key]"))
}

/// The line a grade of `harness` without its key says.
pub fn no_key(harness: ci::Harness) -> String {
    format!(
        "Skill trigger grades on {} skipped: no `{}` from the `skill-grades` environment (it is \
         not set up, or this is a fork's pull request, which is never handed it), so nothing was \
         graded.",
        harness.shown(),
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
    let graded: Vec<launch::Skill> = ask
        .skills
        .iter()
        .map(|skill| launch::graded(&skills_dir, skill))
        .collect();
    let briefed = launch::briefed(ask);
    let ran = launch::run_all(ask, key, &graded, &briefed);
    let (version, model) = launch::ran_as(ask, &ran);
    let on = ci::On {
        harness: ask.harness,
        version: version.unwrap_or_else(|| {
            if ask.latest {
                LATEST_UNKNOWN.to_owned()
            } else {
                ask.pinned.to_owned()
            }
        }),
        model: model.unwrap_or_else(|| launch::MODEL_NOT_SAID.to_owned()),
    };
    let runs: Vec<ci::Run> = ran.into_iter().map(|ran| ran.run).collect();
    let card = launch::score(on, &graded, &runs);
    Graded::Done {
        passed: summary::passed(&card),
        summary: summary::markdown(&card, ask.pinned),
        card: Box::new(card),
    }
}
