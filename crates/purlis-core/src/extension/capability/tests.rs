// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_release_build_names_the_words_it_knows_and_not_the_test_one() {
    assert_eq!(
        unknown("teleport", &[Capability::Badges, Capability::RepoColumns]),
        "asks for the capability \"teleport\", which this purlis does not know, so it \
         loads none of this extension. This purlis knows badges, repo-columns. A newer \
         purlis may know it."
    );
}

#[test]
fn a_refusal_repeats_at_most_a_line_of_the_word_it_refuses() {
    let said = unknown(&"x".repeat(10_000), &[Capability::Probe]);
    assert!(said.len() < 300, "{said}");
    assert!(said.contains("This purlis knows probe."), "{said}");
}

#[test]
fn the_prompt_names_each_capability_by_its_word_and_says_what_it_does() {
    assert_eq!(
        Capability::Briefing.asks(),
        "the capability “briefing” — adds text to every chat's first message, quoted as \
         data under its name"
    );
}

#[test]
fn the_test_capability_declares_no_shape_and_every_other_one_does() {
    assert!(!Capability::Probe.has_shape());
    assert!(
        Capability::known()
            .into_iter()
            .filter(|it| *it != Capability::Probe)
            .all(Capability::has_shape)
    );
}

#[test]
fn a_release_build_knows_badges_and_repo_columns() {
    let known = Capability::known_where(false);
    assert_eq!(
        Capability::parse_in("badges", &known),
        Some(Capability::Badges)
    );
    assert_eq!(
        Capability::parse_in("repo-columns", &known),
        Some(Capability::RepoColumns)
    );
}

#[test]
fn the_studio_words_are_unknown_while_the_flag_is_off_and_known_where_it_is_on() {
    let off = Capability::known_where(false);
    let on = Capability::known_where(true);
    for studio in Capability::STUDIO {
        assert!(
            !off.contains(&studio),
            "{} known with the flag off",
            studio.as_str()
        );
        assert!(
            on.contains(&studio),
            "{} unknown with the flag on",
            studio.as_str()
        );
    }
    let words: Vec<&str> = Capability::STUDIO.iter().map(|it| it.as_str()).collect();
    assert_eq!(
        words,
        [
            "skill-source",
            "grader",
            "skill-lint",
            "skill-template",
            "studio-panel",
            "skill-router"
        ]
    );
    let refused = declared(&serde_json::json!(["skill-lint"]), &off).expect_err("unknown");
    assert!(
        refused.contains("\"skill-lint\", which this purlis does not know"),
        "{refused}"
    );
}

#[test]
fn a_studio_capability_needs_protocol_three_or_four_and_has_a_shape() {
    for studio in Capability::STUDIO {
        assert!(studio.is_studio());
        // The Grader's question is one of many lines, protocol 4's (#3236).
        let since = if studio == Capability::Grader { 4 } else { 3 };
        assert_eq!(studio.since(), since, "{}", studio.as_str());
        assert!(studio.has_shape());
    }
    assert!(!Capability::Briefing.is_studio());
    assert!(!Capability::Briefing.asks().contains("experimental"));
}

/// The operator's ruling of 2026-10-31 on #2786: capabilities not fully wired stay out of the
/// freeze. Each word is named here, not read from the list that decides it.
#[test]
fn the_three_wired_words_are_version_one_and_the_others_are_still_said_to_be_experimental() {
    let said = |word: Capability| word.asks().contains(", experimental — ");
    // #3089 and #3092: an extension's source neither installs nor shows in Browse.
    assert!(said(Capability::SkillSource));
    // #3090: an extension's lint is not in the install review.
    assert!(said(Capability::SkillLint));
    // #3091: an extension's template does not land as a draft.
    assert!(said(Capability::SkillTemplate));

    assert!(!said(Capability::Grader), "{}", Capability::Grader.asks());
    assert!(!said(Capability::StudioPanel));
    assert!(!said(Capability::SkillRouter));
}
