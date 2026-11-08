// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A built-in's trigger tests are enough to mean something (spec #5714, decision Q12): at least
//! 20 queries that should start it and 20 that should not, at least a third of those near misses
//! that belong to a neighbour, and a sources file that reads ([`sources_of`]): its tier, and a
//! source note for each prompt. A near miss is a prompt that should not start it whose source
//! names the neighbour it `belongs_to`.

use std::path::Path;

use purlis_skills::grade::ci::{Tier, sources_of};

const LEAST_EACH_WAY: usize = 20;

fn skills_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri/plugin/skills")
}

#[test]
fn every_built_in_with_trigger_tests_has_enough_of_them_with_sources_and_near_misses() {
    let skills = skills_dir();
    let mut checked = Vec::new();
    for skill in skill_grader::launch::built_ins(&skills) {
        let graded = skill_grader::launch::graded(&skills, &skill);
        if graded.tests.is_empty() {
            continue;
        }
        let text = graded
            .sources
            .unwrap_or_else(|| panic!("{skill} has trigger tests and no sources file"));
        let sources =
            sources_of(&text, &skill, &graded.tests).unwrap_or_else(|why| panic!("{skill}: {why}"));
        let positives = graded.tests.iter().filter(|t| t.should_trigger).count();
        let negatives = graded.tests.len() - positives;
        assert!(
            positives >= LEAST_EACH_WAY,
            "{skill} has {positives} positive tests"
        );
        assert!(
            negatives >= LEAST_EACH_WAY,
            "{skill} has {negatives} negative tests"
        );
        // `sources_of` holds each `belongs_to` to a neighbour and to a prompt that should not
        // start the skill.
        let near = graded
            .tests
            .iter()
            .filter(|test| {
                sources
                    .of(&test.query)
                    .is_some_and(|source| source.belongs_to.is_some())
            })
            .count();
        assert!(
            near * 3 >= negatives,
            "{skill}: {near} of {negatives} negatives are near misses"
        );
        let mut queries: Vec<&str> = graded.tests.iter().map(|t| t.query.as_str()).collect();
        queries.sort_unstable();
        queries.dedup();
        assert_eq!(queries.len(), graded.tests.len(), "{skill} repeats a query");
        checked.push((skill, sources.tier));
    }
    for skill in STRICT {
        assert!(
            checked.contains(&((*skill).to_owned(), Tier::Strict)),
            "{skill} carries no trigger tests at the strict bar: {checked:?}"
        );
    }
    for skill in STANDARD {
        assert!(
            checked.contains(&((*skill).to_owned(), Tier::Standard)),
            "{skill} carries no trigger tests at the standard bar: {checked:?}"
        );
    }
}

/// The built-ins spec #5714 holds to the strict bar (decision Q3) that carry tests so far: where a
/// miss leaks a secret, loses work or loses context, purlis-workspace (Q16), where one
/// publishes memory, and purlis-permissions (Q19), where one changes security settings.
/// purlis-report joins with #5699.
const STRICT: &[&str] = &[
    "purlis-secrets",
    "purlis-handoff",
    "purlis-smart-close",
    "purlis-safe-remove",
    "purlis-persona",
    "purlis-workspace",
    "purlis-permissions",
];

/// The built-ins spec #5714 holds to the standard bar (decision Q3, #5731): the rest.
const STANDARD: &[&str] = &[
    "purlis-browser",
    "purlis-theme",
    "purlis-upgrade",
    "purlis-tidy-memory",
    "purlis-repo-work",
    "purlis-curation-action",
    "purlis-skill-studio",
    "purlis-change",
    "purlis-remember",
];
