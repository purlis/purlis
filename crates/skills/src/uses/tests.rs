// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

#[test]
fn a_skill_names_project_and_built_in_skills_and_a_missing_one_is_broken() {
    let uses = uses_in(
        "Run skills:deploy, then skills:gone and purlis:secrets. Again skills:deploy.",
        "skills",
        &names(&["deploy"]),
        &names(&["secrets"]),
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
    let uses = uses_in("myskills:deploy a/skills:deploy", "skills", &[], &[]);
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
    assert_eq!(uses_in(&text, "skills", &[], &[]).len(), MOST_REFERENCES);
}
