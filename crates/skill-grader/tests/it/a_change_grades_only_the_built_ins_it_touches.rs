// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A pull request is graded only for the built-ins it touches: their skill folder, or the
//! plugin wiring, the session-start briefing or the grader itself, which touch every one.

use skill_grader::select::{Touched, touched};

fn graded() -> Vec<String> {
    vec!["purlis-secrets".to_owned(), "purlis-report".to_owned()]
}

#[test]
fn a_change_to_a_built_ins_folder_grades_that_one() {
    let changed = [
        "app/src-tauri/plugin/skills/purlis-secrets/SKILL.md",
        "docs/spec.md",
    ];
    assert_eq!(
        touched(&changed, &graded()),
        Touched::These(vec!["purlis-secrets".to_owned()])
    );
}

#[test]
fn a_change_to_a_built_ins_trigger_tests_grades_that_one() {
    let changed = ["app/src-tauri/plugin/skills/purlis-report/evals/triggers.json"];
    assert_eq!(
        touched(&changed, &graded()),
        Touched::These(vec!["purlis-report".to_owned()])
    );
}

#[test]
fn a_built_in_without_trigger_tests_is_not_graded_however_it_is_touched() {
    let changed = ["app/src-tauri/plugin/skills/purlis-theme/SKILL.md"];
    assert_eq!(touched(&changed, &graded()), Touched::Nothing);
}

#[test]
fn a_change_touching_nothing_skill_related_grades_nothing() {
    let changed = ["crates/purlis-core/src/forge.rs", "app/src/Notice.tsx"];
    assert_eq!(touched(&changed, &graded()), Touched::Nothing);
}

#[test]
fn the_plugin_wiring_the_briefing_or_the_grader_grades_every_built_in_and_says_why() {
    for (path, why) in [
        ("app/src-tauri/plugin/hooks/hooks.json", "the plugin wiring"),
        ("crates/purlis-core/src/chattools.rs", "the plugin wiring"),
        // How a Codex or opencode chat is armed is what their grades start them with (#5728).
        (
            "crates/purlis-core/src/harness/codex.rs",
            "the plugin wiring",
        ),
        (
            "crates/purlis-core/src/harness/opencode.rs",
            "the plugin wiring",
        ),
        ("crates/purlis-core/src/opencode.rs", "the plugin wiring"),
        (
            "crates/purlis-core/src/briefing.rs",
            "the session-start briefing",
        ),
        (
            "crates/purlis-core/src/briefing/memory.rs",
            "the session-start briefing",
        ),
        ("ci/skill-grades/crowd/skills/tdd/SKILL.md", "the CI grader"),
        ("crates/skills/src/grade/ci.rs", "the CI grader"),
        (".github/workflows/skill-grades.yml", "the CI grader"),
    ] {
        assert_eq!(
            touched(&[path], &graded()),
            Touched::Every {
                skills: graded(),
                because: format!("{path} is {why}"),
            },
            "{path}"
        );
    }
}

/// The paths `.github/workflows/skill-grades.yml` starts on for a pull request, as written.
fn workflow_paths() -> Vec<String> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github/workflows/skill-grades.yml");
    let text = std::fs::read_to_string(&file).expect("the workflow");
    let (_, after) = text
        .split_once("  pull_request:\n    paths:\n")
        .expect("pull requests filtered by path");
    after
        .lines()
        .map_while(|line| line.strip_prefix("      - "))
        .map(|path| path.trim_matches('"').to_owned())
        .collect()
}

#[test]
fn the_workflow_starts_on_exactly_the_paths_that_grade_a_built_in() {
    let mut written = workflow_paths();
    written.sort();
    let mut grading = skill_grader::select::workflow_paths();
    grading.sort();
    assert_eq!(written, grading);
}

#[test]
fn the_listing_and_how_a_chat_is_armed_grade_every_built_in() {
    for path in [
        "crates/purlis-core/src/hoststart.rs",
        "crates/purlis-core/src/skills.rs",
        "app/src-tauri/plugin/opencode/purlis.ts",
    ] {
        assert!(
            matches!(touched(&[path], &graded()), Touched::Every { .. }),
            "{path}"
        );
    }
}
