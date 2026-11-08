// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grade in the run's summary**: per skill, its crowded and bare scores and its verdict,
//! under the harness, its version and the model they ran on.

use std::fmt::Write as _;

use purlis_skills::grade::ci::{ListingScore, Rate, Scorecard, SkillGrade};

/// Whether every skill was scored and met its trigger bar.
pub fn passed(card: &Scorecard) -> bool {
    card.skills.iter().all(|skill| match skill {
        SkillGrade::Scored(score) => score.verdict.passed,
        SkillGrade::NotScored { .. } => false,
    })
}

/// A rate as the summary says it: `50% (1 of 2)`.
pub fn rate(rate: Rate) -> String {
    format!(
        "{}% ({} of {})",
        rate.percent().unwrap_or(0),
        rate.count,
        rate.of
    )
}

fn row(out: &mut String, word: &str, score: &ListingScore) {
    let _ = writeln!(
        out,
        "| {word} | {} | {} |",
        rate(score.recall),
        rate(score.false_triggers)
    );
}

/// **`card` in Markdown**, saying the pin where the runs ran another version.
pub fn markdown(card: &Scorecard, pinned: &str) -> String {
    let mut out = format!(
        "### Skill trigger grades on {} {} ({})\n\n",
        card.harness.name(),
        card.version,
        card.model
    );
    if card.version != pinned {
        let _ = writeln!(
            out,
            "The runs said they ran {}; the pin file names {pinned}.\n",
            card.version
        );
    }
    for skill in &card.skills {
        match skill {
            SkillGrade::NotScored { skill, why } => {
                let _ = writeln!(out, "#### {skill}: not scored\n\n{why}\n");
            }
            SkillGrade::Scored(score) => {
                let mark = if score.verdict.passed {
                    "meets"
                } else {
                    "below"
                };
                let _ = writeln!(
                    out,
                    "#### {}: {mark} its {} bar\n\n| listing | recall | false triggers |\n|---|---|---|",
                    score.skill,
                    score.bar.word()
                );
                row(&mut out, "crowded", &score.crowded);
                if let Some(bare) = &score.bare {
                    row(&mut out, "bare", bare);
                }
                let _ = writeln!(out, "\n{}\n", score.verdict.said);
            }
        }
    }
    out
}
