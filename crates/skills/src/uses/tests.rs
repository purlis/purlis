// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

#[test]
fn a_skill_names_project_and_built_in_skills_and_a_missing_one_is_broken() {
    let plane = names(&["deploy"]);
    let built_in = names(&["purlis-secrets"]);
    let uses = uses_in(
        "Run skills:deploy, then skills:gone and purlis:secrets. Again skills:deploy.",
        "skills",
        &|one| plane.iter().any(|known| known == one),
        &|one| built_in.iter().any(|known| known == one),
    );
    let seen: Vec<(&str, bool, bool)> = uses
        .iter()
        .map(|one| (one.said.as_str(), one.project, one.broken))
        .collect();
    assert_eq!(
        seen,
        [
            ("skills:deploy", true, false),
            ("skills:gone", true, true),
            ("purlis:secrets", false, false),
        ]
    );
}

#[test]
fn a_namespace_inside_a_longer_word_is_no_reference() {
    let uses = uses_in(
        "myskills:deploy a/skills:deploy",
        "skills",
        &|_| false,
        &|_| false,
    );
    assert!(uses.is_empty(), "{uses:?}");
}

#[test]
fn the_graph_says_what_each_skill_uses_and_what_uses_it() {
    let skills = vec![
        (
            "ship".to_owned(),
            "Use skills:deploy then skills:notify.".to_owned(),
        ),
        (
            "deploy".to_owned(),
            "Use skills:notify and skills:missing.".to_owned(),
        ),
        (
            "notify".to_owned(),
            "Plain text, and skills:notify itself.".to_owned(),
        ),
    ];
    let (graph, cut) = graph(&skills, "skills", &[]);
    assert!(!cut);
    assert_eq!(graph["notify"].used_by, ["deploy", "ship"]);
    assert_eq!(graph["deploy"].used_by, ["ship"]);
    assert!(graph["ship"].used_by.is_empty());
    let broken: Vec<&str> = graph["deploy"]
        .uses
        .iter()
        .filter(|one| one.broken)
        .map(|one| one.name.as_str())
        .collect();
    assert_eq!(broken, ["missing"]);
    assert!(!graph.contains_key("missing"));
}

#[test]
fn a_project_of_more_skills_than_the_graph_reads_says_it_was_cut() {
    let skills: Vec<(String, String)> = (0..=MOST_SKILLS)
        .map(|n| (format!("s{n}"), "skills:s0".to_owned()))
        .collect();
    let (graph, cut) = graph(&skills, "skills", &[]);
    assert!(cut);
    assert_eq!(graph["s0"].used_by.len(), MOST_SKILLS - 1);
}

#[test]
fn one_text_is_read_for_at_most_its_share_of_references() {
    let text: String = (0..MOST_REFERENCES + 10)
        .map(|n| format!("skills:s{n} "))
        .collect();
    assert_eq!(
        uses_in(&text, "skills", &|_| false, &|_| false).len(),
        MOST_REFERENCES
    );
}

/// #4510: a skill written for every harness names its siblings bare, as Claude Code's skill tool
/// takes them. A bare name is read as a hint only where it is the whole of a quote or a code
/// span, on a line that says "skill", and names a skill there is that is not the skill itself.
#[test]
fn a_bare_quoted_skill_name_beside_the_word_skill_is_a_hint() {
    let known = names(&["grilling", "to-tickets", "deploy"]);
    let text = "Use the Skill tool with \"grilling\", then `to-tickets`.\n\
                Load the skill “deploy” too, and \"grilling\" again.\n";
    let hints = hints_in(text, "ship", &|one| known.iter().any(|k| k == one));
    let seen: Vec<(&str, &str, bool, bool, bool)> = hints
        .iter()
        .map(|one| {
            (
                one.said.as_str(),
                one.name.as_str(),
                one.project,
                one.broken,
                one.hint,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("\"grilling\"", "grilling", true, false, true),
            ("`to-tickets`", "to-tickets", true, false, true),
            ("“deploy”", "deploy", true, false, true),
        ]
    );
}

#[test]
fn a_bare_name_is_no_hint_without_the_word_skill_a_quote_or_a_skill_of_that_name() {
    let known = names(&["grilling", "deploy", "ship"]);
    let known = |one: &str| known.iter().any(|k| k == one);
    for text in [
        // No "skill" on its line.
        "Then run \"grilling\" on the plan.",
        // "skill" only inside a longer word.
        "A skillful \"grilling\" of the skillset.",
        // Not the whole of the quote.
        "The skill \"grilling the plan\" is next.",
        // Not quoted at all.
        "The grilling skill is next.",
        // Single quotes are apostrophes as often as quotes.
        "The skill 'grilling' is next.",
        // No skill of that name.
        "The skill \"roasting\" is next.",
        // The skill itself.
        "This skill, \"ship\", ships.",
        // A quote that does not close on its line.
        "The skill \"grilling\nis next\".",
    ] {
        assert!(hints_in(text, "ship", &known).is_empty(), "{text:?}");
    }
}

#[test]
fn every_skill_name_a_skill_word_line_quotes_is_read_and_skills_prefixed_ones_are_not_hints() {
    let known = names(&["grilling", "deploy"]);
    let hints = hints_in(
        "Skills: `grilling`, `deploy`; skills:deploy is a reference, not a hint.",
        "ship",
        &|one| known.iter().any(|k| k == one),
    );
    let said: Vec<&str> = hints.iter().map(|one| one.said.as_str()).collect();
    assert_eq!(said, ["`grilling`", "`deploy`"]);
    assert!(
        uses_in("skills:deploy", "skills", &|_| true, &|_| true)
            .iter()
            .all(|one| !one.hint)
    );
}

#[test]
fn the_graph_marks_a_hint_and_says_which_skills_hint_at_one() {
    let skills = vec![
        (
            "ship".to_owned(),
            "Use the skill \"deploy\", then skills:notify.".to_owned(),
        ),
        ("deploy".to_owned(), "Deploys.".to_owned()),
        ("notify".to_owned(), "Notifies.".to_owned()),
    ];
    let (graph, _) = graph(&skills, "skills", &[]);
    let ship = &graph["ship"];
    let seen: Vec<(&str, bool)> = ship
        .uses
        .iter()
        .map(|one| (one.said.as_str(), one.hint))
        .collect();
    assert_eq!(seen, [("skills:notify", false), ("\"deploy\"", true)]);
    assert_eq!(graph["deploy"].hinted_by, ["ship"]);
    assert!(graph["deploy"].used_by.is_empty());
    assert_eq!(graph["notify"].used_by, ["ship"]);
}

#[test]
fn a_name_both_referenced_and_quoted_is_a_reference_only() {
    let skills = vec![
        (
            "ship".to_owned(),
            "Use skills:deploy, the skill \"deploy\".".to_owned(),
        ),
        ("deploy".to_owned(), "Deploys.".to_owned()),
    ];
    let (graph, _) = graph(&skills, "skills", &[]);
    assert_eq!(graph["ship"].uses.len(), 1);
    assert!(!graph["ship"].uses[0].hint);
    assert!(graph["deploy"].hinted_by.is_empty());
}

/// #5874: the skill's tab offers "Write skills:<name>" on a hint's row. It writes every quote
/// that hints at that name as a reference, inside its own marks, and leaves every other line,
/// quote and name as it was; the text then reads it as a reference and no longer as a hint.
#[test]
fn a_hint_written_as_a_reference_turns_every_quote_of_that_name_and_nothing_else() {
    let text = "# Ship\n\
                Use the Skill tool with \"grilling\" first.\n\
                Then run \"grilling\" on the plan.\n\
                Load the skill `grilling` again, and the skill `deploy`.\n";
    let written = written_as_reference(text, "ship", "grilling", "skills");
    assert_eq!(
        written,
        "# Ship\n\
         Use the Skill tool with \"skills:grilling\" first.\n\
         Then run \"grilling\" on the plan.\n\
         Load the skill `skills:grilling` again, and the skill `deploy`.\n"
    );
    let known = |one: &str| ["grilling", "deploy"].contains(&one);
    let hints: Vec<String> = hints_in(&written, "ship", &known)
        .into_iter()
        .map(|one| one.name)
        .collect();
    assert_eq!(hints, ["deploy"]);
    assert!(
        uses_in(&written, "skills", &known, &|_| false)
            .iter()
            .any(|one| one.name == "grilling" && !one.broken && !one.hint)
    );
}

/// #5874: the hint's row says the line it is on, 1 for the first, as the Text tab numbers them.
#[test]
fn a_hint_says_the_first_line_it_is_on() {
    let text =
        "---\nname: ship\n---\nShip it.\nUse the skill \"grilling\".\nThe skill \"grilling\".\n";
    assert_eq!(hint_line(text, "ship", "grilling"), Some(5));
    assert_eq!(hint_line(text, "ship", "deploy"), None);
    assert_eq!(hint_line("Run \"grilling\".", "ship", "grilling"), None);
}
