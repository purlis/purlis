// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `image-leftovers.yml` removes the untagged images a failed publishing run left in the runner's
//! and the operator's packages (#4930), and nothing can make it do more, or hold a release back.
//!
//! What it may remove is `tools/image-leftovers.mjs`, tested on recorded answers in
//! `tools/image-leftovers.test.mjs`. This holds the workflow around it: the workflow grants
//! nothing by default; `plan`, the one job a pull request runs, may read the package and never
//! write it; `remove`, the one job that may write it, runs only on `main`, from the schedule or a
//! dispatch that asks for it; neither can fail the run; and neither reads a secret, mints an OIDC
//! token or takes a token past the checkout.

use crate::workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "image-leftovers.yml";

fn leftovers() -> (String, Vec<Job>) {
    let text = read(&workflows_dir().join(FILE));
    let jobs = jobs(&text);
    (text, jobs)
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

/// The job's `if:`, folded onto one line whether it is written on one or as a `>-` block.
fn condition(job: &Job) -> String {
    let Some(start) = job.body.iter().position(|l| l.starts_with("    if:")) else {
        return String::new();
    };
    let mut lines = vec![job.body[start]["    if:".len()..].trim().to_owned()];
    lines.extend(
        job.body[start + 1..]
            .iter()
            .take_while(|l| l.starts_with("      "))
            .map(|l| l.trim().to_owned()),
    );
    lines.join(" ").trim_start_matches(">-").trim().to_owned()
}

#[test]
fn the_workflow_grants_nothing_by_default_and_only_remove_may_write_the_package() {
    let (text, jobs) = leftovers();
    assert!(
        text.lines().any(|l| l == "permissions: {}"),
        "the workflow grants nothing by default; each job names what it needs"
    );
    let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names, ["plan", "remove"], "{FILE}'s jobs");
    assert_eq!(
        job_named(&jobs, "plan", FILE).permissions(),
        Some(pairs(&[
            ("actions", "read"),
            ("contents", "read"),
            ("packages", "read"),
        ])),
        "plan reads the runs, their artifacts and the package, and writes nothing"
    );
    assert_eq!(
        job_named(&jobs, "remove", FILE).permissions(),
        Some(pairs(&[
            ("actions", "read"),
            ("contents", "read"),
            ("packages", "write"),
        ])),
    );
    for job in &jobs {
        assert!(
            !job.reads_a_secret(),
            "{}: the job's own token is enough",
            job.name
        );
        assert!(
            !job.can_mint_an_oidc_token(),
            "{}: nothing is signed",
            job.name
        );
    }
}

#[test]
fn a_pull_request_only_plans_and_remove_runs_only_on_main_when_asked() {
    let (text, jobs) = leftovers();
    assert!(
        text.lines().any(|l| l == "  pull_request:"),
        "a change to the selection runs its plan on its pull request"
    );
    assert!(
        !text
            .lines()
            .any(|l| l.starts_with("  pull_request_target:")),
        "never with the base's token on a pull request's code"
    );
    assert_eq!(
        condition(job_named(&jobs, "plan", FILE)),
        "",
        "plan runs on every way in"
    );
    assert_eq!(
        condition(job_named(&jobs, "remove", FILE)),
        "github.ref == 'refs/heads/main' && ( github.event_name == 'schedule' || (github.event_name == 'workflow_dispatch' && inputs.remove) )",
        "remove runs on main only, from the schedule or a dispatch that asks for it"
    );
}

#[test]
fn neither_job_can_fail_the_run_or_wait_on_a_release() {
    let (text, jobs) = leftovers();
    for job in &jobs {
        assert!(
            job.body.iter().any(|l| l == "    continue-on-error: true"),
            "{}: a leftover never turns a run red",
            job.name
        );
    }
    assert!(
        !text.contains("workflow_call") && !text.contains("workflow_run"),
        "no release calls it and it waits on none"
    );
}

#[test]
fn each_job_runs_the_tested_selection_with_its_own_command_and_holds_no_credential_in_git() {
    let (_, jobs) = leftovers();
    for (name, command) in [("plan", "plan"), ("remove", "remove")] {
        let job = job_named(&jobs, name, FILE);
        let steps = job.steps();
        let checkout = steps
            .iter()
            .find(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/checkout@")))
            .unwrap_or_else(|| panic!("{name} checks out the tool"));
        assert_eq!(
            step_value(checkout, "with", "persist-credentials"),
            Some("false")
        );
        let runs: Vec<String> = steps.iter().flat_map(|s| run_lines(s)).collect();
        let tool: Vec<&String> = runs
            .iter()
            .filter(|l| l.contains("tools/image-leftovers.mjs"))
            .collect();
        assert_eq!(tool.len(), 1, "{name}: {runs:?}");
        assert!(
            tool[0].contains(&format!(
                "tools/image-leftovers.mjs {command} \"$WORKFLOW\""
            )),
            "{name} runs `{command}`: {}",
            tool[0]
        );
        assert!(
            !runs
                .iter()
                .any(|l| l.contains("packages/container") || l.contains("-X DELETE")),
            "{name}: only the tool, which asks again before each removal, deletes"
        );
    }
}
