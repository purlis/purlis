// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The CI grader's workflow (#5727 of spec #5714) runs Claude Code on built-in skills' trigger
//! tests with one model key, held in the GitHub environment `skill-grades` under one static
//! name, so the runner is sent that key and no other secret. Only the job that grades is in the
//! environment; the key reaches the step that checks it is there and the step that grades,
//! after the build, and no command prints it. That job restores no cache. Without the key
//! (not provisioned, or a fork's pull request) it says so in the run's summary and runs
//! nothing else, green. It starts on a pull request only for the paths that can move a grade.

use crate::workflow;

use workflow::{job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "skill-grades.yml";

const KEY: &str = "${{ secrets.ANTHROPIC_API_KEY }}";

fn workflow() -> (String, Vec<workflow::Job>) {
    let text = read(&workflows_dir().join(FILE));
    let jobs = jobs(&text);
    (text, jobs)
}

fn names_a_secret(line: &str) -> bool {
    line.contains("secrets.") || line.contains("secrets[") || line.contains("secrets)")
}

#[test]
fn the_only_secret_named_is_the_key_in_its_own_environments_grading_job() {
    let (text, jobs) = workflow();
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        if names_a_secret(line) {
            assert!(
                line.trim_end().ends_with(KEY),
                "a secret other than the key, or the key by another form: {line}"
            );
        }
    }
    assert!(
        text.contains("\npermissions: {}\n"),
        "the workflow grants nothing"
    );
    let select = job_named(&jobs, "select", FILE);
    let grade = job_named(&jobs, "grade", FILE);
    assert_eq!(select.environment(), None);
    assert!(
        !select.reads_a_secret(),
        "the selecting job holds no secret"
    );
    assert_eq!(grade.environment(), Some("skill-grades"));
    for job in [select, grade] {
        assert_eq!(
            job.permissions(),
            Some(vec![("contents".to_owned(), "read".to_owned())]),
            "{}",
            job.name
        );
    }
}

#[test]
fn the_key_reaches_the_check_and_the_grade_after_the_build_and_no_command_prints_it() {
    let (_, jobs) = workflow();
    let grade = job_named(&jobs, "grade", FILE);
    let steps = grade.steps();
    let handed: Vec<usize> = (0..steps.len())
        .filter(|&i| steps[i].iter().any(|l| l.contains(KEY)))
        .collect();
    assert_eq!(handed.len(), 2, "the key reaches two steps: {handed:?}");
    assert_eq!(handed[0], 0, "the first step checks the key is there");
    assert_eq!(step_value(&steps[0], "env", "KEY"), Some(KEY));
    let grading = &steps[handed[1]];
    assert_eq!(step_value(grading, "env", "ANTHROPIC_API_KEY"), Some(KEY));
    assert!(
        run_lines(grading).join(" ").contains("skill-grader grade"),
        "the second is the grade"
    );
    let build = steps
        .iter()
        .position(|s| run_lines(s).join(" ").contains("cargo build"))
        .expect("a build step");
    assert!(
        build < handed[1],
        "the build runs before the key is handed over"
    );
    for step in &steps {
        for line in run_lines(step) {
            let names = line.contains("$KEY") || line.contains("ANTHROPIC_API_KEY");
            assert!(
                !names || line.contains("[ -z \"$KEY\" ]") || line.starts_with("echo"),
                "a command reads the key: {line}"
            );
            if line.starts_with("echo") {
                assert!(
                    !line.contains("$KEY") && !line.contains("$ANTHROPIC_API_KEY"),
                    "{line}"
                );
            }
        }
        let action = step_uses(step).unwrap_or_default();
        assert!(
            ![
                "Swatinem/rust-cache@",
                "actions/cache",
                "actions/setup-node@"
            ]
            .iter()
            .any(|a| action.starts_with(a)),
            "the job holding the key restores no cache: {action}"
        );
        if action.starts_with("actions/checkout@") {
            assert_eq!(
                step_value(step, "with", "persist-credentials"),
                Some("false")
            );
        }
    }
}

#[test]
fn without_the_key_it_says_so_in_the_summary_and_runs_nothing_else() {
    let (_, jobs) = workflow();
    let grade = job_named(&jobs, "grade", FILE);
    let steps = grade.steps();
    let first = &steps[0];
    assert!(first.iter().any(|l| l.trim() == "id: provisioned"));
    let said = run_lines(first).join("\n");
    for words in [
        "GITHUB_STEP_SUMMARY",
        "::notice",
        "skipped",
        "skill-grades",
        "nothing was graded",
    ] {
        assert!(said.contains(words), "no {words:?} in: {said}");
    }
    for step in &steps[1..] {
        assert!(
            step.iter()
                .any(|l| l.contains("steps.provisioned.outputs.ready == 'true'")),
            "a step runs without the key: {step:#?}"
        );
    }
    assert_eq!(grade.condition(), Some("needs.select.outputs.skills != ''"));
}

#[test]
fn a_pull_request_starts_it_only_for_paths_that_can_move_a_grade() {
    let (text, _) = workflow();
    let on = &text[text.find("\non:\n").expect("an `on:`")..text.find("\npermissions:").unwrap()];
    assert!(
        on.contains("pull_request:\n    paths:"),
        "pull requests are filtered by path: {on}"
    );
    for path in [
        "app/src-tauri/plugin/**",
        "crates/purlis-core/src/briefing.rs",
        "ci/skill-grades/**",
        "crates/skill-grader/**",
    ] {
        assert!(
            on.contains(&format!("\"{path}\"")),
            "{path} does not start it"
        );
    }
    assert!(!on.contains("\"**\""), "every path starts it");
}
