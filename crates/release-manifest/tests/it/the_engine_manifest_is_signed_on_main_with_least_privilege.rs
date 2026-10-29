// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The browser engine's manifest (#2073) is signed keyless by `engine-manifest.yml` on `main`
//! of purlis/purlis, and a build trusts exactly that identity
//! (`purlis_browser_engine::sigstore::MANIFEST_WORKFLOW`). So the workflow runs on `main` only:
//! on its schedule, or on a push to `main` that changes it, never by hand, from a pull request
//! or after another workflow. It grants nothing at the top; the job that can mint an OIDC token
//! cannot write to the repository, the job that writes the release assets cannot mint one, and
//! the job that keeps its issue does neither. No job restores a cache, every checkout is without
//! credentials, and what it publishes is first verified against the trusted root the builds ship
//! as exactly the identity they accept.

use crate::workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "engine-manifest.yml";

fn manifest_workflow() -> (String, Vec<Job>) {
    let text = read(&workflows_dir().join(FILE));
    let jobs = jobs(&text);
    (text, jobs)
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = list
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    pairs.sort();
    pairs
}

/// The lines of the workflow's `on:` block, trimmed, blank ones dropped.
fn triggers(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|l| *l != "on:")
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with(' '))
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn it_runs_on_its_schedule_and_on_a_push_to_main_that_changes_it_and_nothing_else() {
    let (text, jobs) = manifest_workflow();
    assert_eq!(
        triggers(&text),
        [
            "schedule:",
            "- cron: \"17 5 * * *\"",
            "push:",
            "branches: [main]",
            "paths:",
            "- .github/workflows/engine-manifest.yml",
            "- tools/engine-manifest.mjs",
        ]
    );
    assert_eq!(
        job_named(&jobs, "sign", FILE).condition(),
        Some("github.repository == 'purlis/purlis' && github.ref == 'refs/heads/main'")
    );
    assert_eq!(job_named(&jobs, "publish", FILE).needs(), ["sign"]);
}

#[test]
fn the_workflow_grants_nothing_and_each_job_only_what_it_needs() {
    let (text, jobs) = manifest_workflow();
    assert!(text.lines().any(|l| l == "permissions: {}"));
    let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names, ["sign", "publish", "notice"]);
    assert_eq!(
        job_named(&jobs, "sign", FILE).permissions(),
        Some(pairs(&[("contents", "read"), ("id-token", "write")]))
    );
    assert_eq!(
        job_named(&jobs, "publish", FILE).permissions(),
        Some(pairs(&[("contents", "write")]))
    );
    assert_eq!(
        job_named(&jobs, "notice", FILE).permissions(),
        Some(pairs(&[("issues", "write")]))
    );
    for job in &jobs {
        assert!(!job.reads_a_secret(), "job `{}` reads a secret", job.name);
    }
}

#[test]
fn no_job_restores_a_cache_and_every_checkout_is_without_credentials() {
    let (_, jobs) = manifest_workflow();
    for job in &jobs {
        for step in job.steps() {
            let action = step_uses(&step).map(|u| u.split('@').next().unwrap_or(u));
            match action {
                Some("Swatinem/rust-cache" | "actions/cache" | "actions/cache/restore") => {
                    panic!("job `{}` restores a cache: {step:#?}", job.name)
                }
                Some("actions/setup-node") => assert_eq!(
                    step_value(&step, "with", "package-manager-cache"),
                    Some("false"),
                    "job `{}`",
                    job.name
                ),
                Some("actions/checkout") => assert_eq!(
                    step_value(&step, "with", "persist-credentials"),
                    Some("false"),
                    "job `{}`",
                    job.name
                ),
                _ => {}
            }
        }
    }
}

/// What a run script reads from the workflow's expressions comes in through `env:`, never
/// written into the script.
#[test]
fn no_run_script_holds_an_expression() {
    let (_, jobs) = manifest_workflow();
    for job in &jobs {
        for step in job.steps() {
            for line in run_lines(&step) {
                assert!(
                    !line.contains("${{"),
                    "job `{}` writes an expression into a script: {line}",
                    job.name
                );
            }
        }
    }
}

#[test]
fn what_is_published_is_verified_first_as_the_builds_verify_it() {
    let (_, jobs) = manifest_workflow();
    let sign = job_named(&jobs, "sign", FILE);
    let script: Vec<String> = sign.steps().iter().flat_map(|s| run_lines(s)).collect();
    let script = script.join("\n");
    let signed = script.find("cosign sign-blob").expect("it signs");
    let verified = script.find("cosign verify-blob").expect("it verifies");
    assert!(signed < verified);
    for flag in [
        "--trusted-root crates/browser-engine/sigstore-trusted-root.json",
        "--certificate-identity \"$IDENTITY\"",
        "--certificate-oidc-issuer https://token.actions.githubusercontent.com",
    ] {
        assert!(script.contains(flag), "the verification lacks {flag}");
    }
    let identity = sign
        .body
        .iter()
        .find_map(|l| l.trim().strip_prefix("IDENTITY:"))
        .map(str::trim);
    assert_eq!(
        identity,
        Some(
            "${{ github.server_url }}/${{ github.repository }}/.github/workflows/engine-manifest.yml@refs/heads/main"
        )
    );
    // The file the builds trust is this one.
    let trusted = read(
        &workflows_dir()
            .join("../../crates/browser-engine/src/sigstore.rs")
            .canonicalize()
            .unwrap(),
    );
    assert!(trusted.contains(&format!("file: \"{FILE}\"")));
}
