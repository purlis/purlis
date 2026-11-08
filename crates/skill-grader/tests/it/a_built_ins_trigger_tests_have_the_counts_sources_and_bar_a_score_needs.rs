// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A built-in's trigger tests are enough to mean something (spec #5714, decision Q12): at least
//! 20 queries that should start it and 20 that should not, at least a third of those near misses
//! that belong to a neighbour its sources file names, each with a note saying where it came
//! from, and a trigger bar. A near miss's note opens `Near miss (<neighbour>)`.

use std::path::Path;

use purlis_skills::grade::ci::{Bar, sources_of};

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
            sources_of(&text, graded.tests.len()).unwrap_or_else(|why| panic!("{skill}: {why}"));
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
        let mut near = 0;
        for (test, note) in graded.tests.iter().zip(&sources.notes) {
            let Some(rest) = note.strip_prefix("Near miss (") else {
                continue;
            };
            assert!(
                !test.should_trigger,
                "{skill}: a near miss should not trigger: {:?}",
                test.query
            );
            let neighbour = rest.split_once(')').map(|(name, _)| name).unwrap_or("");
            assert!(
                sources.neighbours.iter().any(|n| n == neighbour),
                "{skill}: {neighbour:?} is not among its neighbours {:?}",
                sources.neighbours
            );
            near += 1;
        }
        assert!(
            near * 3 >= negatives,
            "{skill}: {near} of {negatives} negatives are near misses"
        );
        let mut queries: Vec<&str> = graded.tests.iter().map(|t| t.query.as_str()).collect();
        queries.sort_unstable();
        queries.dedup();
        assert_eq!(queries.len(), graded.tests.len(), "{skill} repeats a query");
        checked.push((skill, sources.bar));
    }
    for skill in STRICT {
        assert!(
            checked.contains(&((*skill).to_owned(), Bar::Strict)),
            "{skill} carries no trigger tests at the strict bar: {checked:?}"
        );
    }
    for skill in STANDARD {
        assert!(
            checked.contains(&((*skill).to_owned(), Bar::Standard)),
            "{skill} carries no trigger tests at the standard bar: {checked:?}"
        );
    }
}

/// The built-ins spec #5714 holds to the strict bar (decision Q3) that carry tests so far: where a
/// miss leaks a secret, loses work or loses context, and purlis-workspace (Q16), where one
/// publishes memory. purlis-report joins with #5699.
const STRICT: &[&str] = &[
    "purlis-secrets",
    "purlis-handoff",
    "purlis-smart-close",
    "purlis-safe-remove",
    "purlis-persona",
    "purlis-workspace",
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
];
