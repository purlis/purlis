// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The nightly's decisions** (#5729 of spec #5714, decisions Q22 and Q23): from each
//! harness's report (its scorecard), which `skill-triggers` issue to open, rewrite or close, and whether to
//! propose the versions the night ran as the new pins. `.github/workflows/skill-grades.yml`
//! fetches the open issues and carries the decisions out; nothing here calls GitHub.

use purlis_skills::grade::RUNS;
use purlis_skills::grade::ci::{Graded, Harness, Report, Score, Why};

/// **An open issue labelled `skill-triggers`**, as `gh api` prints it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct OpenIssue {
    pub number: u64,
    pub title: String,
}

/// **What to do with one skill's issue on one harness.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// The skill dropped below its bar and has no open issue.
    Create { title: String, body: String },
    /// The skill is still below its bar: its issue's body is rewritten, never commented on.
    Edit {
        number: u64,
        title: String,
        body: String,
    },
    /// The skill meets its bar again.
    Close {
        number: u64,
        title: String,
        comment: String,
    },
}

/// **The pins a green night proposes.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bump {
    /// The pin file with only the versions moved.
    pub pins: String,
    /// Each version that moved, one per line: `- Codex: 0.167.0 → 0.170.0`.
    pub said: String,
}

/// **The title of a skill's issue on a harness**: the same every night, so a night finds the
/// issue an earlier one opened.
pub fn title(skill: &str, harness: Harness) -> String {
    format!("{skill} is below its trigger bar on {}", harness.shown())
}

/// **What each scored skill's issue needs**, harness by harness in the order of `cards`. A skill
/// that was not scored says nothing about its bar, so its issue is left as it is.
pub fn issues(cards: &[Report], open: &[OpenIssue], run_url: &str) -> Vec<Act> {
    let mut acts = Vec::new();
    for card in cards {
        // Nothing about any skill's bar is known: its issues are left as they are.
        if unread_runs(card).is_some() {
            continue;
        }
        for skill in &card.skills {
            let Graded::Scored(score) = skill else {
                continue;
            };
            let title = title(&score.skill, card.on.harness);
            let existing = open.iter().find(|issue| issue.title == title);
            let said = skill.verdict(&card.on);
            match (score.meets_its_bar, existing) {
                (false, None) => acts.push(Act::Create {
                    body: body(&said, card, run_url),
                    title,
                }),
                (false, Some(issue)) => acts.push(Act::Edit {
                    number: issue.number,
                    body: body(&said, card, run_url),
                    title,
                }),
                (true, Some(issue)) => acts.push(Act::Close {
                    number: issue.number,
                    comment: format!(
                        "{}\n\n{}\n\nClosed by the nightly skill trigger grades: {run_url}",
                        said.trim_end(),
                        ran_on(card)
                    ),
                    title,
                }),
                (true, None) => {}
            }
        }
    }
    acts
}

/// **How many runs `card` had, where none of them said what it started**: a harness every run
/// of which failed (a spend limit reached, a harness that cannot start), so nothing was graded
/// on it. None where any run read, or nothing was scored.
fn unread_runs(card: &Report) -> Option<u32> {
    let counted = |score: Option<Score>| {
        score.is_some_and(|score| score.recall.of > 0 || score.false_triggers.of > 0)
    };
    let mut unread = 0_u32;
    let mut scored = false;
    for skill in &card.skills {
        let Graded::Scored(score) = skill else {
            continue;
        };
        if counted(score.crowded) || counted(score.bare) {
            return None;
        }
        scored = true;
        let prompts = score
            .failing
            .iter()
            .filter(|failing| failing.why == Why::NotRun)
            .count();
        unread = unread.saturating_add(
            u32::try_from(prompts)
                .unwrap_or(u32::MAX)
                .saturating_mul(u32::from(RUNS)),
        );
    }
    scored.then_some(unread)
}

/// **A line for each harness the night could not grade**, because none of its runs said what
/// it started: the run's summary says it, in place of an issue per skill.
pub fn not_graded(cards: &[Report]) -> Vec<String> {
    cards
        .iter()
        .filter_map(|card| {
            let runs = unread_runs(card)?;
            Some(format!(
                "Skill trigger grades on {} {}: not graded, since none of its {runs} runs said                  what it started. Its key's spend limit may be reached, or the harness may not                  start; the run's skill-grades-runs-{} artifact holds what each run printed.",
                card.on.harness.shown(),
                card.on.version,
                crate::word(card.on.harness)
            ))
        })
        .collect()
}

/// The harness version and model the night ran: its own line, as a verdict names the version
/// it was scored at.
fn ran_on(card: &Report) -> String {
    format!(
        "Graded on {} {} ({}).",
        card.on.harness.shown(),
        card.on.version,
        card.on.model
    )
}

fn body(said: &str, card: &Report, run_url: &str) -> String {
    let harness = card.on.harness;
    let said = said.trim_end();
    format!(
        "{said}\n\n{}\n\nLast graded: {run_url}\n\nThe nightly skill trigger grades \
         (`.github/workflows/skill-grades.yml`) rewrite this issue each night the skill stays \
         below its bar on {} and close it the night it meets it again. The run's \
         `skill-grades-runs-{}` artifact holds each run's output.\n",
        ran_on(card),
        harness.shown(),
        crate::word(harness)
    )
}

/// Whether every skill on `card` was scored and met its bar, and there was at least one.
fn green(card: &Report) -> bool {
    !card.skills.is_empty() && crate::summary::passed(card)
}

/// **The pins a night proposes**: only when every harness the pin file names was graded and
/// every skill met its bar on each, and some harness ran a version other than its pin. Only the
/// versions move; every other line of `pins` stays as it is.
pub fn bump(pins: &str, cards: &[Report]) -> Result<Option<Bump>, String> {
    let table: toml::Table =
        toml::from_str(pins).map_err(|why| format!("the pin file is not TOML: {why}"))?;
    let pinned = table
        .get("harness")
        .and_then(toml::Value::as_table)
        .ok_or("the pin file has no [harness] table")?;
    let mut moved: Vec<(&str, &str)> = Vec::new();
    let mut said = String::new();
    for (word, version) in pinned {
        let version = version
            .as_str()
            .ok_or_else(|| format!("the pin of {word} is not a string"))?;
        let Some(card) = cards
            .iter()
            .find(|card| crate::word(card.on.harness) == word)
        else {
            return Ok(None);
        };
        if !green(card) || card.on.version == crate::LATEST_UNKNOWN {
            return Ok(None);
        }
        if card.on.version != version {
            moved.push((crate::word(card.on.harness), &card.on.version));
            said.push_str(&format!(
                "- {}: {version} → {}\n",
                card.on.harness.shown(),
                card.on.version
            ));
        }
    }
    if moved.is_empty() {
        return Ok(None);
    }
    let mut section = String::new();
    let mut out = String::with_capacity(pins.len());
    for line in pins.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed.to_owned();
        }
        let key = trimmed.split('=').next().unwrap_or_default().trim();
        match moved.iter().find(|(word, _)| *word == key) {
            Some((word, version)) if section == "[harness]" && trimmed.contains('=') => {
                let ending = &line[line.trim_end().len()..];
                out.push_str(&format!("{word} = \"{version}\"{ending}"));
            }
            _ => out.push_str(line),
        }
    }
    Ok(Some(Bump { pins: out, said }))
}
