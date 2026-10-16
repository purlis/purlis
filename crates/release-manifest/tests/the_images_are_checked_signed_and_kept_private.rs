// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's container images (spec #2041): the runner image (#2043, `runner-image.yml`) and the
//! cluster operator's image (#2238, `operator-image.yml`). Each is built for both architectures
//! on every green `main` of what it is built from, checked, pushed to the organisation's
//! private registry, and signed keyless; a pull request builds and checks it and pushes nothing.
//!
//! What these tests hold both workflows and the images' Dockerfiles to:
//! - only a green run of the image's gate on a push to this repository's `main` publishes, by
//!   the same rule as the dev channel's build (`release.yml`'s `plan`);
//! - the workflow grants nothing, the compile holds no write permission, the image jobs may
//!   write packages and nothing more, and only `index` may mint the OIDC token cosign signs
//!   with; no job restores a cache, since what it builds is signed;
//! - nothing is pushed, tagged or signed except on a publishing run; nothing is pushed into a
//!   package the packages API does not show private; nothing is tagged or signed before the
//!   image's check passed on the pushed bytes and the images were shown private again;
//! - `main` is tagged only when `tools/image-registry.mjs order` says it moves forward;
//! - each base image is pinned by digest, and each image's user is a number other than 0.

mod workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

/// One image and what differs between the workflows that make them.
struct Image {
    /// The workflow file.
    file: &'static str,
    /// The workflow whose green run on a push to `main` publishes the image.
    gate: &'static str,
    /// What a pull request has to change to run the workflow.
    paths: &'static [&'static str],
    /// The package, under the repository's owner on ghcr.io.
    package: &'static str,
    /// Each check of the image job, as its one `run:` line, run on the candidate image.
    checks: &'static [&'static str],
}

const RUNNER: Image = Image {
    file: "runner-image.yml",
    gate: "ci",
    paths: &[
        "runner-image/**",
        "tools/runner-image-*",
        "tools/image-registry*",
        ".github/workflows/runner-image.yml",
    ],
    package: "purlis-runner",
    checks: &[
        "node tools/runner-image-contract.mjs purlis-runner:candidate",
        "node tools/runner-image-contract.mjs purlis-runner-team-example:candidate",
    ],
};

const OPERATOR: Image = Image {
    file: "operator-image.yml",
    gate: "cluster operator",
    paths: &[
        "operator/**",
        "rust-toolchain.toml",
        ".github/actions/rust-toolchain/**",
        "tools/operator-image-*",
        "tools/image-registry*",
        ".github/workflows/operator-image.yml",
    ],
    package: "purlis-cluster-operator",
    checks: &["node tools/operator-image-check.mjs purlis-cluster-operator:candidate \"$ARCH\""],
};

const IMAGES: [Image; 2] = [RUNNER, OPERATOR];

fn workflow(image: &Image) -> (String, Vec<Job>) {
    let text = read(&workflows_dir().join(image.file));
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

/// The lines of the workflow's `on:` block, trimmed.
fn triggers(text: &str) -> Vec<&str> {
    text.lines()
        .skip_while(|l| *l != "on:")
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with(' '))
        .map(str::trim)
        .collect()
}

#[test]
fn only_a_green_gate_on_a_push_to_this_repositorys_main_publishes() {
    let release = workflow::release_jobs();
    let gate = folded_condition(job_named(&release, "plan", "release.yml"));
    assert!(gate.contains("workflow_run.event == 'push'"), "{gate}");
    for image in &IMAGES {
        let (text, jobs) = workflow(image);
        let on = triggers(&text);
        let run = format!("workflows: [{}]", image.gate);
        let after_run: Vec<&&str> = on.iter().skip_while(|l| **l != "workflow_run:").collect();
        assert!(
            after_run.contains(&&run.as_str()) && after_run.contains(&&"branches: [main]"),
            "{}: {on:?}",
            image.file
        );
        let plan = job_named(&jobs, "plan", image.file);
        assert_eq!(
            folded_condition(plan),
            gate,
            "{}: published by the dev channel's own rule",
            image.file
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
            job_named(&jobs, "index", image.file).condition(),
            Some("needs.plan.outputs.publish == 'true'")
        );
    }
}

#[test]
fn a_pull_request_runs_it_only_when_the_images_files_change() {
    for image in &IMAGES {
        let (text, _) = workflow(image);
        let on = triggers(&text);
        let paths: Vec<&str> = on
            .iter()
            .skip_while(|l| **l != "paths:")
            .skip(1)
            .take_while(|l| l.starts_with("- "))
            .map(|l| &l[2..])
            .collect();
        assert_eq!(paths, image.paths, "{}", image.file);
        assert!(on.contains(&"pull_request:"), "{on:?}");
        assert!(!on.contains(&"pull_request_target:"), "{on:?}");
        assert!(
            !on.contains(&"push:"),
            "{}: main publishes through its gate only: {on:?}",
            image.file
        );
    }
}

#[test]
fn the_workflow_grants_nothing_and_each_job_only_what_it_needs() {
    for image in &IMAGES {
        let (text, jobs) = workflow(image);
        let file = image.file;
        assert!(text.lines().any(|l| l == "permissions: {}"), "{file}");
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, ["plan", "binary", "image", "index"], "{file}");
        assert_eq!(job_named(&jobs, "plan", file).permissions(), None);
        assert_eq!(
            job_named(&jobs, "binary", file).permissions(),
            Some(pairs(&[("contents", "read")])),
            "{file}: the compile runs dependencies' build scripts and holds no write permission"
        );
        assert_eq!(
            job_named(&jobs, "image", file).permissions(),
            Some(pairs(&[("contents", "read"), ("packages", "write")]))
        );
        assert_eq!(
            job_named(&jobs, "index", file).permissions(),
            Some(pairs(&[
                ("contents", "read"),
                ("id-token", "write"),
                ("packages", "write")
            ]))
        );
        let minting: Vec<&str> = jobs
            .iter()
            .filter(|j| j.can_mint_an_oidc_token())
            .map(|j| j.name.as_str())
            .collect();
        assert_eq!(minting, ["index"], "{file}");
    }
}

#[test]
fn no_job_restores_a_cache() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        for job in &jobs {
            for step in job.steps() {
                match action(&step) {
                    Some("Swatinem/rust-cache" | "actions/cache" | "actions/cache/restore") => {
                        panic!(
                            "{}: job `{}` restores a cache: {step:#?}",
                            image.file, job.name
                        )
                    }
                    Some("actions/setup-node") => assert_eq!(
                        step_value(&step, "with", "package-manager-cache"),
                        Some("false"),
                        "{}: job `{}`",
                        image.file,
                        job.name
                    ),
                    Some("docker/build-push-action") => assert!(
                        step_value(&step, "with", "cache-from").is_none(),
                        "{}: job `{}` builds from a cache: {step:#?}",
                        image.file,
                        job.name
                    ),
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn every_checkout_is_the_planned_commit_without_credentials() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        for job in jobs.iter().filter(|j| j.name != "plan") {
            let checkouts: Vec<Vec<String>> = job
                .steps()
                .into_iter()
                .filter(|s| action(s) == Some("actions/checkout"))
                .collect();
            assert_eq!(checkouts.len(), 1, "{}: job `{}`", image.file, job.name);
            assert_eq!(
                step_value(&checkouts[0], "with", "ref"),
                Some("${{ needs.plan.outputs.sha }}"),
                "{}: job `{}`",
                image.file,
                job.name
            );
            assert_eq!(
                step_value(&checkouts[0], "with", "persist-credentials"),
                Some("false"),
                "{}: job `{}`",
                image.file,
                job.name
            );
        }
    }
}

#[test]
fn only_a_publishing_run_signs_in_pushes_or_signs() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        let job = job_named(&jobs, "image", image.file);
        for step in job.steps() {
            let pushes = step_value(&step, "with", "outputs")
                .is_some_and(|o| o.contains("push=true"))
                || step_value(&step, "with", "push") == Some("true")
                || action(&step) == Some("docker/login-action")
                || run_lines(&step)
                    .iter()
                    .any(|l| l.contains("ensure-private"));
            if pushes {
                assert_eq!(
                    step_condition(&step),
                    Some("env.PUBLISH == 'true'"),
                    "{}: {step:#?}",
                    image.file
                );
            }
            if action(&step) == Some("docker/build-push-action") {
                assert_ne!(step_value(&step, "with", "push"), Some("true"), "{step:#?}");
            }
        }
        // Everything `index` does is a publishing run's: the job's own `if:` says so.
        let index = job_named(&jobs, "index", image.file);
        assert!(index.needs().contains(&"image".to_owned()));
    }
}

#[test]
fn the_check_runs_on_the_pushed_bytes_before_any_tag_or_signature() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        let file = image.file;
        let job = job_named(&jobs, "image", file);
        let steps = job.steps();
        let position = |what: &dyn Fn(&[String]) -> bool, about: &str| {
            steps
                .iter()
                .position(|s| what(s))
                .unwrap_or_else(|| panic!("{file}: no step in `image` {about}"))
        };
        let push = position(
            &|s| {
                step_value(s, "with", "outputs").is_some_and(|o| o.contains("push-by-digest=true"))
            },
            "pushes by digest",
        );
        let pull = position(
            &|s| {
                run_lines(s)
                    .iter()
                    .any(|l| l == "docker pull \"$IMAGE@$DIGEST\"")
            },
            "pulls back",
        );
        let record = position(
            &|s| step_value(s, "with", "name") == Some("digest-${{ matrix.arch }}"),
            "records the digest",
        );
        for check in image.checks {
            let at = position(&|s| run_lines(s) == [*check], check);
            assert!(push < pull && pull < at && at < record, "{file}: {check}");
        }
        let private = position(
            &|s| s[0].contains("name: The package is private before anything of purlis is pushed"),
            "shows the package private",
        );
        assert_eq!(
            step_condition(&steps[private]),
            Some("env.PUBLISH == 'true'")
        );
        assert_eq!(
            run_lines(&steps[private]),
            ["node tools/image-registry.mjs ensure-private \"$ARCH\""],
            "{file}"
        );
        assert!(
            private < push,
            "{file}: the package is shown private before the push"
        );

        let index = job_named(&jobs, "index", file);
        let steps = index.steps();
        let named = |name: &str| {
            steps
                .iter()
                .position(|s| s[0].contains(&format!("name: {name}")))
                .unwrap_or_else(|| panic!("{file}: `index` has no step `{name}`"))
        };
        let private = named("The images are private");
        let tag = named("One index for both architectures");
        let sign = named("Sign by digest");
        let verify = named("The signature verifies");
        let record = named("Record the digest");
        assert!(private < tag && tag < sign && sign < verify && verify < record);
        let checks = run_lines(&steps[private]);
        assert!(
            checks.contains(&"state=$(node tools/image-registry.mjs visibility)".to_owned())
                && checks.contains(&"node tools/image-registry.mjs anonymous \\".to_owned()),
            "{file}: {checks:#?}"
        );
        let tagging = run_lines(&steps[tag]);
        assert!(
            tagging.contains(&"order=$(node tools/image-registry.mjs order \"$SHA\")".to_owned()),
            "{file}: {tagging:#?}"
        );
        let main_tags: Vec<&String> = tagging
            .iter()
            .filter(|l| l.contains("$IMAGE:main"))
            .collect();
        assert_eq!(
            main_tags,
            [&"main:*) tags+=(--tag \"$IMAGE:main\") ;;".to_owned()],
            "{file}: `main` is tagged only when the order check says it moves forward"
        );
        assert_eq!(
            run_lines(&steps[sign]),
            ["cosign sign --yes --recursive \"$IMAGE@$DIGEST\""],
            "{file}: signed by digest, never by tag"
        );
        let verifying = run_lines(&steps[verify]).join("\n");
        assert!(
            verifying.contains(&format!("/.github/workflows/{file}@refs/heads/main\" \\")),
            "{file}: verified as this workflow's signature on main: {verifying}"
        );
    }
}

#[test]
fn the_image_lives_in_the_organisations_own_registry() {
    for image in &IMAGES {
        let (text, _) = workflow(image);
        let name = format!(
            "  IMAGE: ghcr.io/${{{{ github.repository_owner }}}}/{}",
            image.package
        );
        assert!(
            text.lines().any(|l| l == name),
            "{}: the image's one name",
            image.file
        );
        let registries: Vec<&str> = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("registry:"))
            .map(str::trim)
            .collect();
        assert!(!registries.is_empty());
        assert!(registries.iter().all(|r| *r == "ghcr.io"), "{registries:?}");
    }
}

/// The Dockerfile's `FROM` lines and its `USER`, as written.
fn dockerfile(path: &str) -> (Vec<String>, Vec<String>) {
    let text = read(&workflows_dir().join("../..").join(path));
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
    for path in [
        "runner-image/Dockerfile",
        "runner-image/team-example.Dockerfile",
        "operator/Dockerfile",
    ] {
        let (from, user) = dockerfile(path);
        assert!(!from.is_empty(), "{path}");
        for base in &from {
            let digest = base.split_once("@sha256:").map(|(_, d)| d);
            assert!(
                digest.is_some_and(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit())),
                "{path}: {base} is not pinned by digest"
            );
        }
        assert_eq!(user.len(), 1, "{path}");
        let uid = user[0].split(':').next().unwrap_or_default();
        assert!(
            uid.parse::<u32>().is_ok_and(|n| n != 0),
            "{path}: USER {}",
            user[0]
        );
    }
}
