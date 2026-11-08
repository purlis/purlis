// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The nightly (#5729 of spec #5714, decisions Q22 and Q23) grades every built-in on the latest
//! version of each harness. A skill below its bar on a harness opens one issue for that skill and
//! harness, or rewrites the one already open, naming the bar, the score, the failing prompts and
//! the harness version; a skill that meets its bar again closes it. A night on which every
//! harness was graded and every skill met its bar proposes the versions it ran as the new pins.

use purlis_skills::grade::ci::{Harness, Scorecard, SkillGrade};
use skill_grader::nightly::{self, Act, OpenIssue};
use skill_grader::{Graded, grade};

use crate::fixture::fixture;

const RUN: &str = "https://github.com/purlis/purlis/actions/runs/42";

const PINS: &str = "# The harness versions a pull request's skill grades run on.

[harness]
# npm's @anthropic-ai/claude-code.
claude-code = \"2.1.316\"
# npm's @openai/codex.
codex = \"0.167.0\"
# npm's opencode-ai.
opencode = \"1.19.5\"
";

/// A real grade's scorecard, from the stand-in harness: green when the skill starts among the
/// crowd, below its strict bar when it does not. Moved to `harness` at `version`.
fn card(green: bool, harness: Harness, version: &str) -> Scorecard {
    let fixture = fixture(green, false);
    let Graded::Done { card, .. } = grade(&fixture.ask(Some("sk-test"))) else {
        panic!("it graded nothing");
    };
    let mut card = *card;
    card.harness = harness;
    card.version = version.to_owned();
    card
}

fn open(number: u64, skill: &str, harness: Harness) -> OpenIssue {
    OpenIssue {
        number,
        title: nightly::title(skill, harness),
    }
}

#[test]
fn a_skill_below_its_bar_opens_one_issue_naming_the_bar_the_score_the_prompts_and_the_version() {
    let acts = nightly::issues(&[card(false, Harness::Codex, "0.170.0")], &[], RUN);
    let [Act::Create { title, body }] = acts.as_slice() else {
        panic!("not one issue opened: {acts:?}");
    };
    assert_eq!(title, &nightly::title("purlis-secrets", Harness::Codex));
    for said in ["purlis-secrets", "Codex"] {
        assert!(title.contains(said), "no {said:?} in the title {title:?}");
    }
    for said in [
        "strict trigger bar",
        "at least 95% recall",
        "recalled 0% (0 of 2)",
        "\"run the migration with the database password from the vault\" (missed)",
        "Codex 0.170.0",
        RUN,
    ] {
        assert!(body.contains(said), "no {said:?} in:\n{body}");
    }
}

#[test]
fn a_skill_still_below_its_bar_rewrites_its_open_issue_and_opens_no_other() {
    let acts = nightly::issues(
        &[card(false, Harness::Codex, "0.171.0")],
        &[
            open(7, "purlis-secrets", Harness::ClaudeCode),
            open(9, "purlis-secrets", Harness::Codex),
        ],
        RUN,
    );
    let [Act::Edit { number, body, .. }] = acts.as_slice() else {
        panic!("not one issue rewritten: {acts:?}");
    };
    assert_eq!(*number, 9, "the issue of that skill on that harness");
    assert!(body.contains("Codex 0.171.0"), "{body}");
}

#[test]
fn a_skill_that_meets_its_bar_again_closes_its_issue_and_one_that_never_dropped_files_nothing() {
    let recovered = nightly::issues(
        &[card(true, Harness::Opencode, "1.20.0")],
        &[open(11, "purlis-secrets", Harness::Opencode)],
        RUN,
    );
    let [
        Act::Close {
            number, comment, ..
        },
    ] = recovered.as_slice()
    else {
        panic!("its issue was not closed: {recovered:?}");
    };
    assert_eq!(*number, 11);
    for said in ["opencode 1.20.0", "meets its strict trigger bar", RUN] {
        assert!(comment.contains(said), "no {said:?} in:\n{comment}");
    }
    assert_eq!(
        nightly::issues(&[card(true, Harness::Opencode, "1.20.0")], &[], RUN),
        vec![]
    );
}

#[test]
fn each_harness_keeps_its_own_issue_for_the_same_skill() {
    let acts = nightly::issues(
        &[
            card(true, Harness::ClaudeCode, "2.1.400"),
            card(false, Harness::Codex, "0.170.0"),
        ],
        &[open(7, "purlis-secrets", Harness::ClaudeCode)],
        RUN,
    );
    assert!(
        matches!(acts.as_slice(), [Act::Close { number: 7, .. }, Act::Create { title, .. }]
            if *title == nightly::title("purlis-secrets", Harness::Codex)),
        "{acts:?}"
    );
}

#[test]
fn a_skill_that_was_not_scored_leaves_its_issue_as_it_is() {
    let mut not_scored = card(false, Harness::Codex, "0.170.0");
    not_scored.skills = vec![SkillGrade::NotScored {
        skill: "purlis-secrets".to_owned(),
        why: "its trigger tests do not parse".to_owned(),
    }];
    assert_eq!(
        nightly::issues(
            &[not_scored],
            &[open(9, "purlis-secrets", Harness::Codex)],
            RUN
        ),
        vec![]
    );
}

#[test]
fn a_green_night_on_every_harness_proposes_the_versions_it_ran_as_the_pins() {
    let cards = [
        card(true, Harness::ClaudeCode, "2.1.400"),
        card(true, Harness::Codex, "0.170.0"),
        card(true, Harness::Opencode, "1.19.5"),
    ];
    let bump = nightly::bump(PINS, &cards)
        .expect("the pins parse")
        .expect("a bump");
    assert_eq!(
        bump.pins,
        PINS.replace("2.1.316", "2.1.400")
            .replace("0.167.0", "0.170.0"),
        "only the versions move; every comment stays"
    );
    for said in ["Claude Code: 2.1.316 → 2.1.400", "Codex: 0.167.0 → 0.170.0"] {
        assert!(bump.said.contains(said), "no {said:?} in:\n{}", bump.said);
    }
    assert!(!bump.said.contains("opencode"), "{}", bump.said);
}

#[test]
fn no_bump_is_proposed_unless_every_harness_was_graded_green_on_a_newer_version() {
    let green = |harness, version| card(true, harness, version);
    let red_on_codex = [
        green(Harness::ClaudeCode, "2.1.400"),
        card(false, Harness::Codex, "0.170.0"),
        green(Harness::Opencode, "1.20.0"),
    ];
    let codex_not_graded = [
        green(Harness::ClaudeCode, "2.1.400"),
        green(Harness::Opencode, "1.20.0"),
    ];
    let nothing_newer = [
        green(Harness::ClaudeCode, "2.1.316"),
        green(Harness::Codex, "0.167.0"),
        green(Harness::Opencode, "1.19.5"),
    ];
    for cards in [&red_on_codex[..], &codex_not_graded[..], &nothing_newer[..]] {
        assert_eq!(nightly::bump(PINS, cards), Ok(None));
    }
}
