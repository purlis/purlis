// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The CI grader's workflow (#5727 and #5728 of spec #5714) grades built-in skills' trigger tests
//! on Claude Code, Codex and opencode, one job each, so each harness is reported and gated on
//! its own. Each job runs on one model key, held in the GitHub environment `skill-grades` under
//! one static name, so its runner is sent that key and no other secret: `ANTHROPIC_API_KEY` for
//! Claude Code and opencode, `OPENAI_API_KEY` for Codex. Only the jobs that grade are in the
//! environment; a key reaches the step that checks it is there and the step that grades, after
//! the build, and no command prints it. Those jobs restore no cache. Without its key (not
//! provisioned, or a fork's pull request) a job says so in the run's summary and runs nothing
//! else, green. It starts on a pull request only for the paths that can move a grade.

use crate::workflow;

use workflow::{job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "skill-grades.yml";

/// Each grading job, the harness it grades and the one secret it is handed.
const GRADES: [(&str, &str, &str); 3] = [
    ("grade-claude-code", "claude-code", "ANTHROPIC_API_KEY"),
    ("grade-codex", "codex", "OPENAI_API_KEY"),
    ("grade-opencode", "opencode", "ANTHROPIC_API_KEY"),
];

fn secret(name: &str) -> String {
    format!("${{{{ secrets.{name} }}}}")
}

fn workflow() -> (String, Vec<workflow::Job>) {
    let text = read(&workflows_dir().join(FILE));
    let jobs = jobs(&text);
    (text, jobs)
}

fn names_a_secret(line: &str) -> bool {
    line.contains("secrets.") || line.contains("secrets[") || line.contains("secrets)")
}

#[test]
fn the_only_secret_each_grading_job_names_is_its_own_key_in_its_own_environment() {
    let (text, jobs) = workflow();
    assert!(
        text.contains("\npermissions: {}\n"),
        "the workflow grants nothing"
    );
    let select = job_named(&jobs, "select", FILE);
    assert_eq!(select.environment(), None);
    assert!(
        !select.reads_a_secret(),
        "the selecting job holds no secret"
    );
    for (name, _, key) in GRADES {
        let grade = job_named(&jobs, name, FILE);
        assert_eq!(grade.environment(), Some("skill-grades"), "{name}");
        for line in grade
            .body
            .iter()
            .filter(|l| !l.trim_start().starts_with('#'))
        {
            if names_a_secret(line) {
                assert!(
                    line.trim_end().ends_with(&secret(key)),
                    "{name} names a secret other than {key}, or {key} by another form: {line}"
                );
            }
        }
    }
    let graders: Vec<&str> = jobs
        .iter()
        .filter(|job| job.reads_a_secret())
        .map(|job| job.name.as_str())
        .collect();
    assert_eq!(
        graders,
        GRADES.map(|(name, _, _)| name),
        "the jobs holding a key"
    );
    for job in jobs.iter() {
        assert_eq!(
            job.permissions(),
            Some(vec![("contents".to_owned(), "read".to_owned())]),
            "{}",
            job.name
        );
    }
}

#[test]
fn each_harness_is_graded_by_its_own_job_on_its_own_pin() {
    let (_, jobs) = workflow();
    for (name, harness, _) in GRADES {
        let grade = job_named(&jobs, name, FILE);
        let said = grade
            .steps()
            .iter()
            .flat_map(|step| run_lines(step))
            .collect::<Vec<_>>()
            .join("\n");
        for words in [
            format!("skill-grader pin {harness}"),
            format!("--harness {harness}"),
        ] {
            assert!(said.contains(&words), "{name} has no {words:?}: {said}");
        }
        let kept = grade
            .steps()
            .into_iter()
            .find(|step| step_uses(step).is_some_and(|a| a.starts_with("actions/upload-artifact@")))
            .unwrap_or_else(|| panic!("{name} keeps no runs"));
        assert_eq!(
            step_value(&kept, "with", "name"),
            Some(format!("skill-grades-runs-{harness}").as_str()),
            "each job keeps its runs under its own name"
        );
    }
}

#[test]
fn the_key_reaches_the_check_and_the_grade_after_the_build_and_no_command_prints_it() {
    let (_, jobs) = workflow();
    for (name, _, key) in GRADES {
        its_key_reaches_the_check_and_the_grade(job_named(&jobs, name, FILE), key);
    }
}

fn its_key_reaches_the_check_and_the_grade(grade: &workflow::Job, key: &str) {
    let name = &grade.name;
    let handed_key = secret(key);
    let steps = grade.steps();
    let handed: Vec<usize> = (0..steps.len())
        .filter(|&i| steps[i].iter().any(|l| l.contains(&handed_key)))
        .collect();
    assert_eq!(
        handed.len(),
        2,
        "{name}: the key reaches two steps: {handed:?}"
    );
    assert_eq!(
        handed[0], 0,
        "{name}: the first step checks the key is there"
    );
    assert_eq!(
        step_value(&steps[0], "env", "KEY"),
        Some(handed_key.as_str())
    );
    let grading = &steps[handed[1]];
    assert_eq!(
        step_value(grading, "env", key),
        Some(handed_key.as_str()),
        "{name}"
    );
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
            let names = line.contains("$KEY") || line.contains(key);
            assert!(
                !names || line.contains("[ -z \"$KEY\" ]") || line.starts_with("echo"),
                "a command reads the key: {line}"
            );
            if line.starts_with("echo") {
                assert!(
                    !line.contains("$KEY") && !line.contains(&format!("${key}")),
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
fn without_its_key_each_job_says_so_in_the_summary_and_runs_nothing_else() {
    let (_, jobs) = workflow();
    for (name, _, key) in GRADES {
        without_its_key(job_named(&jobs, name, FILE), key);
    }
}

fn without_its_key(grade: &workflow::Job, key: &str) {
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
        key,
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
