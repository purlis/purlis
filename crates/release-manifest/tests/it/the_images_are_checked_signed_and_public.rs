// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's container images (spec #2041): the runner image (#2043, `runner-image.yml`) and the
//! cluster operator's image (#2238, `operator-image.yml`). Each is built for both architectures
//! on every green `main` of what it is built from, checked, pushed to the organisation's
//! registry as a public package (#4498), and signed keyless; a pull request builds and checks it and pushes nothing.
//!
//! What these tests hold both workflows and the images' Dockerfiles to:
//! - only a green run of the image's gate on a push to this repository's `main` publishes, by
//!   the same rule as the dev channel's build (`release.yml`'s `plan`); the runner image is
//!   published only by the release that calls it, with the release's version (#2749);
//! - the workflow grants nothing, the compile holds no write permission, the image jobs may
//!   write packages and nothing more, and only `index` may mint the OIDC token cosign signs
//!   with; no job restores a cache, since what it builds is signed;
//! - nothing is pushed, tagged or signed except on a publishing run; nothing is pushed into a
//!   package the packages API does not show public;
//! - each image is built once, into a file; the check runs on that file's image, and only then
//!   is the same file pushed, by the digest it was checked under (#4512), so an image that fails
//!   its check is never pushed at all; a pull request copies the file by its digest the same way,
//!   into a folder;
//! - nothing is tagged or signed before the images were shown public and served to a puller
//!   who has not signed in;
//! - each published image is then pulled, and its signature and provenance checked, by a job
//!   with no sign-in;
//! - `main` is tagged last, by a job of its own after that check from outside (#4512), only
//!   when `tools/image-registry.mjs order` says it moves forward, and only to the index that
//!   passed;
//! - each base image is pinned by digest, and each image's user is a number other than 0.

use crate::workflow;

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
    /// Published by `release.yml`'s call, for each release, rather than by its own gate's
    /// green run (#2749).
    released: bool,
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
        "node tools/runner-image-contract.mjs --carries gh,glab purlis-runner:candidate",
        "node tools/runner-image-contract.mjs purlis-runner-team-example:candidate",
    ],
    released: true,
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
    released: false,
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
    for image in IMAGES.iter().filter(|image| !image.released) {
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

/// **The runner image is the release's** (#2749, ruled 2026-10-20): `release.yml` calls
/// `runner-image.yml` for each release it publishes, with its commit and version, and nothing
/// else publishes it; the image's `purlis` is stamped with that version.
#[test]
fn the_runner_image_is_published_only_by_the_release_it_belongs_to() {
    let release = workflow::release_jobs();
    let gate = folded_condition(job_named(&release, "plan", "release.yml"));
    let call = job_named(&release, "runner-image", "release.yml");
    assert_eq!(
        call.condition(),
        Some("needs.plan.outputs.publish == 'true'")
    );
    assert!(
        call.body
            .iter()
            .any(|l| l.trim() == "uses: ./.github/workflows/runner-image.yml"),
        "{:#?}",
        call.body
    );
    for (input, from) in [
        ("sha", "${{ needs.plan.outputs.sha }}"),
        ("version", "${{ needs.plan.outputs.version }}"),
    ] {
        assert!(
            call.body
                .iter()
                .any(|l| l.trim() == format!("{input}: {from}")),
            "{input}: {:#?}",
            call.body
        );
    }

    let (text, jobs) = workflow(&RUNNER);
    let on = triggers(&text);
    assert!(on.contains(&"workflow_call:"), "{on:?}");
    assert!(!on.contains(&"workflow_run:"), "{on:?}");
    let plan = job_named(&jobs, "plan", RUNNER.file);
    assert_eq!(folded_condition(plan), gate, "the release's own rule");
    let decide = &plan.steps()[0];
    let script = run_lines(decide).join("\n");
    assert!(script.contains("publish=true; sha=$CALLED_SHA"), "{script}");
    assert_eq!(script.matches("publish=true").count(), 1, "{script}");
    assert_eq!(
        step_value(decide, "env", "VERSION"),
        Some("${{ inputs.version }}")
    );
    assert_eq!(
        step_value(decide, "env", "CALLED_SHA"),
        Some("${{ inputs.sha }}")
    );
    let binary = job_named(&jobs, "binary", RUNNER.file);
    let build = binary
        .steps()
        .into_iter()
        .find(|s| s[0].contains("name: The purlis binary"))
        .expect("the binary's build");
    assert_eq!(
        step_value(&build, "env", "PURLIS_VERSION_STAMP"),
        Some("${{ needs.plan.outputs.version }}")
    );
    assert_eq!(
        job_named(&jobs, "index", RUNNER.file).condition(),
        Some("needs.plan.outputs.publish == 'true'")
    );
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
fn the_operator_image_is_never_started_by_hand() {
    let (text, _) = workflow(&OPERATOR);
    let on = triggers(&text);
    assert!(!on.contains(&"workflow_dispatch:"), "{on:?}");
}

#[test]
fn the_workflow_grants_nothing_and_each_job_only_what_it_needs() {
    for image in &IMAGES {
        let (text, jobs) = workflow(image);
        let file = image.file;
        assert!(text.lines().any(|l| l == "permissions: {}"), "{file}");
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        // Each image's SBOM and attestations (#2191, #4513) are held to their own test,
        // `the_images_carry_their_sbom_and_provenance.rs`.
        let made = [
            "plan", "binary", "image", "index", "sbom", "attest", "outside", "tag-main",
        ];
        assert_eq!(names, made, "{file}");
        assert_eq!(
            job_named(&jobs, "tag-main", file).permissions(),
            Some(pairs(&[("contents", "read"), ("packages", "write")])),
            "{file}: it moves a tag and signs nothing"
        );
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
        assert_eq!(minting, ["index", "attest"], "{file}");
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
            // Each image's SBOM and attestations, and the check from outside, read the pushed
            // image and run nothing of the commit's, so they check nothing out (#2191, #4513).
            let checks_out = !matches!(job.name.as_str(), "sbom" | "attest" | "outside");
            assert_eq!(
                checkouts.len(),
                usize::from(checks_out),
                "{}: job `{}`",
                image.file,
                job.name
            );
            if !checks_out {
                continue;
            }
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
                || run_lines(&step).iter().any(|l| {
                    l.contains("image-registry.mjs visibility") || l.contains("docker://$IMAGE")
                });
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
        // Everything `index` and `tag-main` do is a publishing run's: each job's own `if:` says so.
        let index = job_named(&jobs, "index", image.file);
        assert!(index.needs().contains(&"image".to_owned()));
        assert_eq!(
            job_named(&jobs, "tag-main", image.file).condition(),
            Some("needs.plan.outputs.publish == 'true'")
        );
    }
}

/// The file every architecture's image is built into, loaded, checked and pushed from.
const CANDIDATE: &str = "\"oci-archive:$RUNNER_TEMP/candidate.tar\"";

/// #4512: the check runs before the push, on the image of the very file the push then sends,
/// so a candidate that fails is never pullable, not even untagged by its digest.
#[test]
fn the_image_is_checked_before_it_is_pushed_and_the_push_sends_the_bytes_checked() {
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
        let dockerfile = if image.released {
            "runner-image/Dockerfile"
        } else {
            "operator/Dockerfile"
        };
        let builds: Vec<&Vec<String>> = steps
            .iter()
            .filter(|s| step_value(s, "with", "file") == Some(dockerfile))
            .collect();
        assert_eq!(
            builds.len(),
            1,
            "{file}: the image is built once: {builds:#?}"
        );
        let candidate = image.checks[0]
            .split_whitespace()
            .find(|w| w.ends_with(":candidate"))
            .expect("the checks name the candidate");
        assert_eq!(
            step_value(builds[0], "with", "outputs"),
            Some(
                format!("type=docker,name={candidate},dest=${{{{ runner.temp }}}}/candidate.tar")
                    .as_str()
            ),
            "{file}: built into a file and nowhere else"
        );
        assert!(
            step_condition(builds[0]).is_none(),
            "{file}: a pull request builds it the way a publishing run does"
        );
        for step in &steps {
            assert!(
                step_value(step, "with", "outputs").is_none_or(|o| !o.contains("push=true"))
                    && step_value(step, "with", "push").is_none(),
                "{file}: the build pushes nothing: {step:#?}"
            );
            assert!(
                run_lines(step)
                    .iter()
                    .all(|l| !l.starts_with("docker pull")),
                "{file}: nothing is pulled back: {step:#?}"
            );
        }
        let build = position(
            &|s| step_value(s, "with", "file") == Some(dockerfile),
            "builds the image",
        );
        let load = position(
            &|s| {
                run_lines(s)
                    .iter()
                    .any(|l| l == "docker load --input \"$RUNNER_TEMP/candidate.tar\"")
            },
            "loads the file",
        );
        // The digest is the file's own manifest's, and the image loaded is that manifest's.
        let loading = run_lines(&steps[load]).join("\n");
        for line in [
            "digest=$(tar -xOf \"$RUNNER_TEMP/candidate.tar\" index.json \\",
            "config=$(tar -xOf \"$RUNNER_TEMP/candidate.tar\" \"blobs/sha256/${digest#sha256:}\" | jq -r .config.digest)",
            "[[ \"$digest\" =~ ^sha256:[0-9a-f]{64}$ ]] \\",
            "if [ \"$id\" != \"$digest\" ] && [ \"$id\" != \"$config\" ]; then",
        ] {
            assert!(
                loading.lines().any(|l| l == line),
                "{file}: {line}\n{loading}"
            );
        }
        let push_line =
            format!("skopeo copy --preserve-digests {CANDIDATE} \"docker://$IMAGE@$DIGEST\"");
        let push = position(&|s| run_lines(s) == [push_line.as_str()], "pushes the file");
        assert_eq!(step_condition(&steps[push]), Some("env.PUBLISH == 'true'"));
        assert_eq!(
            step_value(&steps[push], "env", "DIGEST"),
            Some("${{ steps.candidate.outputs.digest }}"),
            "{file}: pushed by the digest the check ran under"
        );
        let dry = position(
            &|s| {
                run_lines(s).iter().any(|l| {
                    l.starts_with(&format!(
                        "skopeo copy --preserve-digests {CANDIDATE} \"dir:"
                    ))
                })
            },
            "copies the file by its digest on a run that publishes nothing",
        );
        assert_eq!(step_condition(&steps[dry]), Some("env.PUBLISH != 'true'"));
        let record = position(
            &|s| step_value(s, "with", "name") == Some("digest-${{ matrix.arch }}"),
            "records the digest",
        );
        for check in image.checks {
            let at = position(&|s| run_lines(s) == [*check], check);
            assert!(
                build < load && load < at && at < push && at < dry && push < record,
                "{file}: {check}"
            );
        }
        let public = position(
            &|s| s[0].contains("name: The package is public before the image is pushed"),
            "shows the package public",
        );
        assert_eq!(
            step_condition(&steps[public]),
            Some("env.PUBLISH == 'true'")
        );
        assert_eq!(
            run_lines(&steps[public]),
            ["node tools/image-registry.mjs visibility"],
            "{file}"
        );
        assert!(
            public < push,
            "{file}: the package is shown public before the push"
        );

        let index = job_named(&jobs, "index", file);
        let steps = index.steps();
        let named = |name: &str| {
            steps
                .iter()
                .position(|s| s[0].contains(&format!("name: {name}")))
                .unwrap_or_else(|| panic!("{file}: `index` has no step `{name}`"))
        };
        let public = named("The images are public");
        let tag = named("One index for both architectures");
        let sign = named("Sign by digest");
        let verify = named("The signature verifies");
        let record = named("Record the digest");
        assert!(public < tag && tag < sign && sign < verify && verify < record);
        let checks = run_lines(&steps[public]);
        assert!(
            checks.contains(&"state=$(node tools/image-registry.mjs visibility)".to_owned())
                && checks.contains(&"node tools/image-registry.mjs anonymous \\".to_owned()),
            "{file}: {checks:#?}"
        );
        let tagging = run_lines(&steps[tag]);
        assert!(
            tagging.contains(
                &"docker buildx imagetools create --tag \"$IMAGE:sha-$SHA\" \\".to_owned()
            ),
            "{file}: {tagging:#?}"
        );
        assert_eq!(
            run_lines(&steps[sign]),
            ["cosign sign --yes --recursive \"$IMAGE@$DIGEST\""],
            "{file}: signed by digest, never by tag"
        );
        let verifying = run_lines(&steps[verify]).join("\n");
        // The runner image is signed on the ref its release releases (#2749).
        let on = if image.released {
            "$RELEASED_REF"
        } else {
            "refs/heads/main"
        };
        assert!(
            verifying.contains(&format!("/.github/workflows/{file}@{on}\" \\")),
            "{file}: verified as this workflow's signature on {on}: {verifying}"
        );
        if image.released {
            assert_eq!(
                step_value(&steps[verify], "env", "RELEASED_REF"),
                Some("${{ github.ref }}")
            );
            // A workflow any repository may call: the run must be this repository's own.
            assert!(
                verifying
                    .contains("--certificate-github-workflow-repository \"$GITHUB_REPOSITORY\" \\"),
                "{file}: {verifying}"
            );
        }
    }
}

/// #4512: `main` only ever names an image that passed every check, the one from outside
/// included: one job, the last, moves it, to the index `index` made, and to nothing else.
#[test]
fn main_is_moved_last_and_only_to_the_index_that_passed_the_check_from_outside() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        let file = image.file;
        let job = job_named(&jobs, "tag-main", file);
        let last = jobs.last().map(|j| j.name.as_str());
        assert_eq!(last, Some("tag-main"), "{file}");
        for need in ["plan", "index", "outside"] {
            assert!(job.needs().contains(&need.to_owned()), "{file}: {need}");
        }
        assert!(!job.can_mint_an_oidc_token(), "{file}");
        // No other job names the tag.
        for other in jobs.iter().filter(|j| j.name != "tag-main") {
            assert!(
                other.body.iter().all(|l| !l.contains(":main\"")),
                "{file}: `{}` names main",
                other.name
            );
        }
        let steps = job.steps();
        let step = steps
            .iter()
            .find(|s| s[0].contains("name: main moves to the checked index"))
            .unwrap_or_else(|| panic!("{file}: `tag-main` has no step that moves main"));
        assert_eq!(
            step_value(step, "env", "LINE"),
            Some("${{ needs.index.outputs.image }}"),
            "{file}"
        );
        let run = run_lines(step);
        let at = |line: &str| {
            run.iter()
                .position(|l| l == line)
                .unwrap_or_else(|| panic!("{file}: no `{line}` in {run:#?}"))
        };
        let shape = at("\"$IMAGE\"@sha256:*) ;;");
        let order = at("order=$(node tools/image-registry.mjs order \"$SHA\")");
        let stay =
            at("sha-only:*) echo \"::notice::main is not moved: ${order#sha-only: }\"; exit 0 ;;");
        let create = at("docker buildx imagetools create --tag \"$IMAGE:main\" \"$LINE\"");
        let read = at(
            "tagged=$(docker buildx imagetools inspect \"$IMAGE:main\" --format '{{json .Manifest}}' | jq -r .digest)",
        );
        let compare = at("if [ \"$tagged\" != \"${LINE#*@}\" ]; then");
        assert!(
            shape < order && order < stay && stay < create && create < read && read < compare,
            "{file}: {run:#?}"
        );
        let mains = run.iter().filter(|l| l.contains("--tag")).count();
        assert_eq!(mains, 1, "{file}: one tag, main: {run:#?}");
    }
}

/// #4498: "from outside" means it: the job that pulls and checks holds no write, can mint no
/// token, never signs in to the registry, and runs on what a person would run.
#[test]
fn each_published_image_is_pulled_and_checked_with_no_sign_in() {
    for image in &IMAGES {
        let (_, jobs) = workflow(image);
        let file = image.file;
        let outside = job_named(&jobs, "outside", file);
        assert_eq!(
            outside.permissions(),
            Some(pairs(&[("attestations", "read"), ("contents", "read")])),
            "{file}"
        );
        assert_eq!(
            outside.condition(),
            Some("needs.plan.outputs.publish == 'true'"),
            "{file}"
        );
        let steps = outside.steps();
        assert!(
            steps
                .iter()
                .all(|s| action(s) != Some("docker/login-action")),
            "{file}: it never signs in to the registry"
        );
        assert_eq!(outside.needs(), ["plan", "index", "attest"], "{file}");
        let commands = steps.iter().flat_map(|s| run_lines(s)).collect::<Vec<_>>();
        // #4784: one daemon refuses to pull the same index a second time for another platform,
        // so the index is read with no sign-in and each architecture's own image is pulled by
        // the digest the index gives it.
        for line in [
            "index=$(docker buildx imagetools inspect \"$LINE\" --format '{{json .Manifest}}')",
            "for platform in linux/amd64 linux/arm64; do",
            "'[.manifests[] | select(\"\\(.platform.os)/\\(.platform.architecture)\" == $platform) | .digest]",
            "| if length == 1 then .[0] else error(\"the index holds \\(length) images for \\($platform)\") end' \\",
            "docker pull --platform \"$platform\" \"${LINE%@*}@$digest\"",
            "cosign verify \"$LINE\" \\",
        ] {
            assert!(commands.iter().any(|l| l == line), "{file}: {line}");
        }
        assert!(
            commands
                .iter()
                .filter(|l| l.starts_with("docker pull"))
                .all(|l| !l.contains("\"$LINE\"") && !l.contains("\"${LINE}\"")),
            "{file}: the index itself is never pulled: {commands:#?}"
        );
        // The same exact signer `index` verifies, never a pattern.
        let exact = if image.released {
            "--certificate-identity \"$GITHUB_SERVER_URL/$GITHUB_REPOSITORY/.github/workflows/runner-image.yml@$RELEASED_REF\" \\"
        } else {
            "--certificate-identity \"$GITHUB_SERVER_URL/$GITHUB_REPOSITORY/.github/workflows/operator-image.yml@refs/heads/main\" \\"
        };
        assert!(commands.iter().any(|l| l == exact), "{file}: {commands:#?}");
        assert!(
            commands.iter().all(|l| !l.contains("regexp")),
            "{file}: {commands:#?}"
        );
        // Only the step that asks the forge's attestation API holds a token.
        for step in &steps {
            let asks_the_forge = run_lines(step)
                .iter()
                .any(|l| l.starts_with("gh attestation verify"));
            assert_eq!(
                step_value(step, "env", "GH_TOKEN").is_some(),
                asks_the_forge,
                "{file}: {step:#?}"
            );
        }
        // The provenance, pinned to the same signer (#4513 for the operator's).
        let identity = exact.replace("--certificate-identity", "--cert-identity");
        assert!(
            commands.contains(&identity)
                && commands.iter().any(|l| l == "--deny-self-hosted-runners"),
            "{file}: the provenance is pinned to the ref: {commands:#?}"
        );
        assert!(
            commands.iter().all(|l| !l.contains("docker login")),
            "{file}: {commands:#?}"
        );
        assert!(
            commands
                .iter()
                .any(|l| l.starts_with("gh attestation verify \"oci://$LINE\"")),
            "{file}: each image carries its attestations"
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
        // A stage of nothing, or a stage earlier in this file, brings nothing in. Read in order,
        // as Docker reads them: a name a later stage takes is still a registry's here.
        let mut stages: Vec<String> = Vec::new();
        for line in &from {
            let (base, alias) = match line.split_once(" AS ") {
                Some((base, alias)) => (base.trim(), Some(alias.trim())),
                None => (line.as_str(), None),
            };
            let of_this_file = base == "scratch"
                || stages.iter().any(|stage| stage == base)
                || base.strip_suffix("-${TARGETARCH}").is_some_and(|stem| {
                    ["amd64", "arm64"]
                        .iter()
                        .all(|arch| stages.contains(&format!("{stem}-{arch}")))
                });
            stages.extend(alias.map(str::to_owned));
            if of_this_file {
                continue;
            }
            let base = line;
            let digest = base.split_once("@sha256:").map(|(_, d)| d);
            assert!(
                digest.is_some_and(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit())),
                "{path}: {base} is not pinned by digest"
            );
        }
        // What is fetched is checked as it is fetched (#4354).
        let text = read(&workflows_dir().join("../..").join(path));
        for add in text.lines().filter(|line| line.starts_with("ADD ")) {
            assert!(
                add.contains("--checksum=sha256:"),
                "{path}: {add} is not checked"
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
