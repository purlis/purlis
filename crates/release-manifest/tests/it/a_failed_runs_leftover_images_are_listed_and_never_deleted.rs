// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `image-leftovers.yml` lists the untagged images a failed publishing run left in the runner's
//! and the operator's packages (#4930), for a person to delete, and can do nothing more.
//!
//! What it lists is `tools/image-leftovers.mjs`, tested on recorded answers in
//! `tools/image-leftovers.test.mjs`. This holds the workflow around it: it reports and never
//! deletes (the person's ruling of 2026-11-03, D-4930-7), so its one job can read the runs, their
//! artifacts and the package and write nothing; the workflow grants nothing by default; the job
//! cannot fail the run; no release calls it or waits on it; and it reads no secret, mints no OIDC
//! token and keeps no token in git.

use crate::workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "image-leftovers.yml";

fn leftovers() -> (String, Vec<Job>) {
    let text = read(&workflows_dir().join(FILE));
    let jobs = jobs(&text);
    (text, jobs)
}

#[test]
fn its_one_job_reads_the_runs_and_the_package_and_writes_nothing() {
    let (text, jobs) = leftovers();
    assert!(
        text.lines().any(|l| l == "permissions: {}"),
        "the workflow grants nothing by default; the job names what it needs"
    );
    let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names, ["report"], "{FILE}'s jobs");
    let report = job_named(&jobs, "report", FILE);
    assert_eq!(
        report.permissions(),
        Some(
            [
                ("actions", "read"),
                ("contents", "read"),
                ("packages", "read"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .to_vec()
        ),
        "deleting a version is a person's, in the package's settings (D-4930-7)"
    );
    assert!(!report.reads_a_secret(), "the job's own token is enough");
    assert!(!report.can_mint_an_oidc_token(), "nothing is signed");
}

#[test]
fn it_never_fails_its_run_and_no_release_calls_it_or_waits_on_it() {
    let (text, jobs) = leftovers();
    let report = job_named(&jobs, "report", FILE);
    assert!(
        report
            .body
            .iter()
            .any(|l| l == "    continue-on-error: true"),
        "a leftover never turns a run red"
    );
    assert!(
        !text.contains("workflow_call") && !text.contains("workflow_run"),
        "no release calls it and it waits on none"
    );
    assert!(
        !text
            .lines()
            .any(|l| l.starts_with("  pull_request_target:")),
        "never with the base's token on a pull request's code"
    );
}

#[test]
fn it_runs_the_tested_report_and_deletes_nothing_itself() {
    let (_, jobs) = leftovers();
    let steps = job_named(&jobs, "report", FILE).steps();
    let checkout = steps
        .iter()
        .find(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/checkout@")))
        .expect("the job checks out the tool");
    assert_eq!(
        step_value(checkout, "with", "persist-credentials"),
        Some("false")
    );
    let runs: Vec<String> = steps.iter().flat_map(|s| run_lines(s)).collect();
    let tool: Vec<&String> = runs
        .iter()
        .filter(|l| l.contains("tools/image-leftovers.mjs"))
        .collect();
    assert_eq!(tool.len(), 1, "{runs:?}");
    assert!(
        tool[0].contains("node tools/image-leftovers.mjs \"$WORKFLOW\""),
        "{}",
        tool[0]
    );
    assert!(
        !runs
            .iter()
            .any(|l| l.contains("DELETE") || l.contains("packages/container")),
        "nothing in the job deletes: {runs:?}"
    );
}
