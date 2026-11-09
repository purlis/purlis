// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grade in the run's summary**: per skill, its crowded and bare scores and its verdict,
//! under the harness, its version and the model they ran on.

use std::fmt::Write as _;

use purlis_skills::grade::ci::{Graded, Ratio, Report, Score};

/// Whether every skill was scored and met its trigger bar.
pub fn passed(card: &Report) -> bool {
    card.skills.iter().all(|skill| match skill {
        Graded::Scored(scored) => scored.meets_its_bar,
        Graded::NotScored { .. } => false,
    })
}

/// A share as the summary says it, as a verdict does: `50% (1 of 2)`, or `none (0 of 0)` where
/// nothing counted.
pub fn rate(ratio: Ratio) -> String {
    match ratio.share() {
        Some(share) => format!("{:.0}% ({} of {})", share * 100.0, ratio.hits, ratio.of),
        None => "none (0 of 0)".to_owned(),
    }
}

fn row(out: &mut String, word: &str, score: Option<Score>) {
    let _ = match score {
        Some(score) => writeln!(
            out,
            "| {word} | {} | {} |",
            rate(score.recall),
            rate(score.false_triggers)
        ),
        None => writeln!(out, "| {word} | not run | not run |"),
    };
}

/// **`card` in Markdown**, saying the pin where the runs ran another version.
pub fn markdown(card: &Report, pinned: &str) -> String {
    let on = &card.on;
    let mut out = format!(
        "### Skill trigger grades on {} {} ({})\n\n",
        on.harness.shown(),
        on.version,
        on.model
    );
    if on.version == crate::LATEST_UNKNOWN {
        let _ = writeln!(
            out,
            "The latest {} ran, and did not say its version; the pin file names {pinned}.\n",
            on.harness.shown()
        );
    } else if on.version != pinned {
        let _ = writeln!(
            out,
            "The runs said they ran {}; the pin file names {pinned}.\n",
            on.version
        );
    }
    for skill in &card.skills {
        match skill {
            Graded::NotScored { skill: name, .. } => {
                let _ = writeln!(out, "#### {name}: not scored\n\n{}", skill.verdict(on));
            }
            Graded::Scored(scored) => {
                let mark = if scored.meets_its_bar {
                    "meets"
                } else {
                    "below"
                };
                let _ = writeln!(
                    out,
                    "#### {}: {mark} its {} bar\n\n| listing | recall | false triggers |\n|---|---|---|",
                    scored.skill,
                    scored.tier.word()
                );
                row(&mut out, "crowded", scored.crowded);
                if scored.bare.is_some() {
                    row(&mut out, "bare", scored.bare);
                }
                let _ = writeln!(out, "\n{}", skill.verdict(on));
            }
        }
    }
    out
}
