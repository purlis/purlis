// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The runner image (#2043, spec #2041) is built for both architectures on every green `main`,
//! checked against its contract, pushed to the organisation's private registry, and signed
//! keyless; a pull request builds and checks it and pushes nothing.
//!
//! What these tests hold `runner-image.yml` and the image's Dockerfiles to:
//! - only a green `ci` on a push to this repository's `main` publishes, by the same rule as
//!   the dev channel's build (`release.yml`'s `plan`);
//! - the workflow grants nothing, the compile holds no write permission, the image jobs may
//!   write packages and nothing more, and only `index` may mint the OIDC token cosign signs
//!   with; no job restores a cache, since what it builds is signed;
//! - nothing is pushed, tagged or signed except on a publishing run, and nothing is tagged
//!   or signed before the contract passed and the images were shown private;
//! - each base image is pinned by digest, and each image's user is a number other than 0.

mod workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const FILE: &str = "runner-image.yml";

fn runner_image() -> (String, Vec<Job>) {
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

/// The job's `if:`, joined from a folded `if: >-` block when it is one.
fn folded_condition(job: &Job) -> String {
    let Some(start) = job.body.iter().position(|l| l.starts_with("    if:")) else {
        return String::new();
    };
    let first = job.body[start]["    if:".len()..].trim();
    if first != ">-" {
        return first.to_owned();
    }
    job.body[start + 1..]
        .iter()
        .take_while(|l| l.starts_with("      "))
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A step's own `if:` value.
fn step_condition(step: &[String]) -> Option<&str> {
    step.iter().find_map(|l| {
        let t = l.trim_start();
        t.strip_prefix("- ")
            .unwrap_or(t)
            .strip_prefix("if:")
            .map(str::trim)
    })
}

/// The `uses:` action of a step without its `@<sha>`.
fn action(step: &[String]) -> Option<&str> {
    step_uses(step).map(|u| u.split('@').next().unwrap_or(u))
}

#[test]
fn only_a_green_ci_on_a_push_to_this_repositorys_main_publishes() {
    let (_, jobs) = runner_image();
    let release = workflow::release_jobs();
    let gate = folded_condition(job_named(&release, "plan", "release.yml"));
    assert!(gate.contains("workflow_run.event == 'push'"), "{gate}");
    let plan = job_named(&jobs, "plan", FILE);
    assert_eq!(
        folded_condition(plan),
        gate,
        "the runner image is published by the dev channel's own rule"
    );
    let decide = &plan.steps()[0];
    let script = run_lines(decide).join("\n");
    assert!(
        script.contains("workflow_run) publish=true; sha=$RUN_SHA ;;"),
        "{script}"
    );
    assert!(
        script.contains("pull_request) publish=false;") && script.contains("*) publish=false;"),
        "{script}"
    );
    assert_eq!(
        script.matches("publish=true").count(),
        1,
        "`workflow_run` is the one event that publishes: {script}"
    );
    assert_eq!(
        step_value(decide, "env", "RUN_SHA"),
        Some("${{ github.event.workflow_run.head_sha }}")
    );
    assert_eq!(
        job_named(&jobs, "index", FILE).condition(),
        Some("needs.plan.outputs.publish == 'true'")
    );
}

#[test]
fn a_pull_request_runs_it_only_when_the_image_files_change() {
    let (text, _) = runner_image();
    let on: Vec<&str> = text
        .lines()
        .skip_while(|l| *l != "on:")
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with(' '))
        .map(str::trim)
        .collect();
    let paths: Vec<&str> = on
        .iter()
        .skip_while(|l| **l != "paths:")
        .skip(1)
        .take_while(|l| l.starts_with("- "))
        .map(|l| &l[2..])
        .collect();
    assert_eq!(
        paths,
        [
            "runner-image/**",
            "tools/runner-image-contract.*",
            ".github/workflows/runner-image.yml"
        ]
    );
    assert!(on.contains(&"pull_request:"), "{on:?}");
    assert!(!on.contains(&"pull_request_target:"), "{on:?}");
    assert!(
        !on.contains(&"push:"),
        "main publishes through `ci`'s gate only: {on:?}"
    );
}

#[test]
fn the_workflow_grants_nothing_and_each_job_only_what_it_needs() {
    let (text, jobs) = runner_image();
    assert!(text.lines().any(|l| l == "permissions: {}"));
    let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names, ["plan", "binary", "image", "index"]);
    assert_eq!(job_named(&jobs, "plan", FILE).permissions(), None);
    assert_eq!(
        job_named(&jobs, "binary", FILE).permissions(),
        Some(pairs(&[("contents", "read")])),
        "the compile runs dependencies' build scripts and holds no write permission"
    );
    assert_eq!(
        job_named(&jobs, "image", FILE).permissions(),
        Some(pairs(&[("contents", "read"), ("packages", "write")]))
    );
    assert_eq!(
        job_named(&jobs, "index", FILE).permissions(),
        Some(pairs(&[("id-token", "write"), ("packages", "write")]))
    );
    let minting: Vec<&str> = jobs
        .iter()
        .filter(|j| j.can_mint_an_oidc_token())
        .map(|j| j.name.as_str())
        .collect();
    assert_eq!(minting, ["index"]);
}

#[test]
fn no_job_restores_a_cache() {
    let (_, jobs) = runner_image();
    for job in &jobs {
        for step in job.steps() {
            match action(&step) {
                Some("Swatinem/rust-cache" | "actions/cache" | "actions/cache/restore") => {
                    panic!("job `{}` restores a cache: {step:#?}", job.name)
                }
                Some("actions/setup-node") => assert_eq!(
                    step_value(&step, "with", "package-manager-cache"),
                    Some("false"),
                    "job `{}`",
                    job.name
                ),
                Some("docker/build-push-action") => assert!(
                    step_value(&step, "with", "cache-from").is_none(),
                    "job `{}` builds from a cache: {step:#?}",
                    job.name
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn only_a_publishing_run_signs_in_pushes_or_signs() {
    let (_, jobs) = runner_image();
    let image = job_named(&jobs, "image", FILE);
    for step in image.steps() {
        let pushes = step_value(&step, "with", "outputs").is_some_and(|o| o.contains("push=true"))
            || step_value(&step, "with", "push") == Some("true")
            || action(&step) == Some("docker/login-action");
        if pushes {
            assert_eq!(
                step_condition(&step),
                Some("env.PUBLISH == 'true'"),
                "{step:#?}"
            );
        }
        if action(&step) == Some("docker/build-push-action") {
            assert_ne!(step_value(&step, "with", "push"), Some("true"), "{step:#?}");
        }
    }
    assert_eq!(
        step_value(&image.steps()[0], "with", "persist-credentials"),
        Some("false")
    );
    // Everything `index` does is a publishing run's: the job's own `if:` says so.
    let index = job_named(&jobs, "index", FILE);
    assert!(index.needs().contains(&"image".to_owned()));
}

#[test]
fn the_contract_runs_on_the_pushed_bytes_before_any_tag_or_signature() {
    let (_, jobs) = runner_image();
    let image = job_named(&jobs, "image", FILE);
    let steps = image.steps();
    let position = |what: &dyn Fn(&[String]) -> bool| {
        steps
            .iter()
            .position(|s| what(s))
            .unwrap_or_else(|| panic!("no such step in `image`"))
    };
    let push = position(&|s| {
        step_value(s, "with", "outputs").is_some_and(|o| o.contains("push-by-digest=true"))
    });
    let pull = position(&|s| {
        run_lines(s)
            .iter()
            .any(|l| l == "docker pull \"$IMAGE@$DIGEST\"")
    });
    let contract = position(&|s| {
        run_lines(s) == ["node tools/runner-image-contract.mjs purlis-runner:candidate"]
    });
    let example = position(&|s| {
        run_lines(s)
            == ["node tools/runner-image-contract.mjs purlis-runner-team-example:candidate"]
    });
    let record = position(&|s| step_value(s, "with", "name") == Some("digest-${{ matrix.arch }}"));
    assert!(
        push < pull && pull < contract && contract < record,
        "{push} {pull} {contract} {record}"
    );
    assert!(example < record);

    let index = job_named(&jobs, "index", FILE);
    let steps = index.steps();
    let named = |name: &str| {
        steps
            .iter()
            .position(|s| s[0].contains(&format!("name: {name}")))
            .unwrap_or_else(|| panic!("`index` has no step `{name}`"))
    };
    let private = named("The images are private");
    let tag = named("One index for both architectures");
    let sign = named("Sign by digest");
    let verify = named("The signature verifies");
    let record = named("Record the digest");
    assert!(private < tag && tag < sign && sign < verify && verify < record);
    assert_eq!(
        run_lines(&steps[sign]),
        ["cosign sign --yes --recursive \"$IMAGE@$DIGEST\""],
        "signed by digest, never by tag"
    );
}

#[test]
fn the_image_lives_in_the_organisations_own_registry() {
    let (text, _) = runner_image();
    assert!(
        text.lines()
            .any(|l| l == "  IMAGE: ghcr.io/${{ github.repository_owner }}/purlis-runner"),
        "the image's one name"
    );
    let registries: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("registry:"))
        .map(str::trim)
        .collect();
    assert!(!registries.is_empty());
    assert!(registries.iter().all(|r| *r == "ghcr.io"), "{registries:?}");
}

/// The Dockerfile's `FROM` lines and its `USER`, as written.
fn dockerfile(name: &str) -> (Vec<String>, Vec<String>) {
    let text = read(&workflows_dir().join("../../runner-image").join(name));
    let words = |key: &str| {
        text.lines()
            .filter_map(|l| l.strip_prefix(key))
            .map(|v| v.trim().to_owned())
            .collect::<Vec<_>>()
    };
    (words("FROM "), words("USER "))
}

#[test]
fn each_image_starts_from_a_base_pinned_by_digest_and_runs_as_a_numbered_user() {
    for name in ["Dockerfile", "team-example.Dockerfile"] {
        let (from, user) = dockerfile(name);
        assert!(!from.is_empty(), "{name}");
        for base in &from {
            let digest = base.split_once("@sha256:").map(|(_, d)| d);
            assert!(
                digest.is_some_and(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit())),
                "{name}: {base} is not pinned by digest"
            );
        }
        assert_eq!(user.len(), 1, "{name}");
        let uid = user[0].split(':').next().unwrap_or_default();
        assert!(
            uid.parse::<u32>().is_ok_and(|n| n != 0),
            "{name}: USER {}",
            user[0]
        );
    }
}
