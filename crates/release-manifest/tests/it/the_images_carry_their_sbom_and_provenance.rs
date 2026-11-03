// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every published image carries an SBOM and build provenance, verified in the run that
//! pushed it**, as the app's release files do (#583, #584): the runner image (#2191) and the
//! cluster operator's (#4513).
//!
//! What these tests hold `runner-image.yml` and `operator-image.yml` to:
//! - `sbom` reads each architecture's pushed image with the syft `release.yml` pins (same
//!   version, same checksum), holding nothing but `contents: read` and `packages: read`, and
//!   fails an SBOM that does not name the image's binary's own crate and the image's
//!   packages; a run that publishes nothing asks the same of the image it built;
//! - `attest` alone may store an attestation: SLSA provenance on the index the image is named
//!   by, and each architecture's SBOM on that architecture's image, each pushed to the registry
//!   beside the image; and it verifies each one, pinned to this workflow, this commit and a
//!   GitHub-hosted runner, before the run can succeed;
//! - `release.yml`'s call grants the attestation scope the runner image's workflow needs.

use crate::workflow;

use workflow::{Job, job_named, jobs, read, run_lines, step_uses, step_value, workflows_dir};

/// One image: its workflow, the name its SBOMs are handed on under, and what each SBOM names.
struct Image {
    file: &'static str,
    package: &'static str,
    sboms: &'static str,
    purls: &'static [&'static str],
}

const IMAGES: [Image; 2] = [
    Image {
        file: "runner-image.yml",
        package: "purlis-runner",
        sboms: "runner-image-sbom",
        purls: &[
            "pkg:cargo/purlis-cli@",
            "pkg:deb/",
            "pkg:deb/ubuntu/gh@",
            "pkg:deb/ubuntu/glab@",
        ],
    },
    Image {
        file: "operator-image.yml",
        package: "purlis-cluster-operator",
        sboms: "operator-image-sbom",
        purls: &[
            "pkg:cargo/purlis-cluster-operator@",
            "pkg:deb/",
            "pkg:deb/ubuntu/ca-certificates@",
        ],
    },
];

fn image_jobs(image: &Image) -> Vec<Job> {
    jobs(&read(&workflows_dir().join(image.file)))
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = list
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    pairs.sort();
    pairs
}

/// The `uses:` action of a step without its `@<sha>`.
fn action(step: &[String]) -> Option<&str> {
    step_uses(step).map(|u| u.split('@').next().unwrap_or(u))
}

/// Every command of the job's `run:` steps, in order.
fn commands(job: &Job) -> Vec<String> {
    job.steps().iter().flat_map(|s| run_lines(s)).collect()
}

/// The `env:` value `key` of the step that runs syft, in `job`.
fn syft_pin(job: &Job, key: &str) -> String {
    job.steps()
        .into_iter()
        .find(|s| run_lines(s).iter().any(|l| l.contains("syft_")))
        .and_then(|s| step_value(&s, "env", key).map(str::to_owned))
        .unwrap_or_else(|| panic!("{}: no syft step with {key}", job.name))
}

#[test]
fn each_images_sbom_is_read_from_the_pushed_bytes_with_the_releases_own_syft() {
    for image in &IMAGES {
        sbom_is_read_from_the_pushed_bytes(image);
    }
}

fn sbom_is_read_from_the_pushed_bytes(image: &Image) {
    let jobs = image_jobs(image);
    let sbom = job_named(&jobs, "sbom", image.file);
    assert_eq!(
        sbom.condition(),
        Some("needs.plan.outputs.publish == 'true'")
    );
    assert_eq!(
        sbom.needs(),
        ["plan", "image"],
        "the pushed images, read beside `index`, not after it"
    );
    assert_eq!(
        sbom.permissions(),
        Some(pairs(&[("contents", "read"), ("packages", "read")])),
        "it reads the image and writes nothing"
    );
    assert!(!sbom.can_mint_an_oidc_token());

    let release = workflow::release_jobs();
    let releases = job_named(&release, "sbom", "release.yml");
    for key in ["SYFT_VERSION", "SYFT_SHA256"] {
        assert_eq!(syft_pin(sbom, key), syft_pin(releases, key), "{key}");
    }
    let run = commands(sbom).join("\n");
    assert!(
        run.contains("sha256sum -c -"),
        "syft is checked before it runs"
    );
    for arch in ["amd64", "arm64"] {
        assert!(
            run.contains(&format!(
                "registry:$IMAGE@$(cat \"$RUNNER_TEMP/digests/{arch}\")"
            )),
            "{arch} is read from the image as pushed: {run}"
        );
    }
    for arch in ["amd64", "arm64"] {
        assert!(
            run.contains(&format!("out/{}-{arch}.cdx.json", image.package)),
            "{}: {arch}: {run}",
            image.file
        );
    }
    for want in image.purls {
        assert!(
            sbom.body.iter().any(|l| l.contains(want)),
            "{}: {want}",
            image.file
        );
    }
    let upload = sbom
        .steps()
        .into_iter()
        .find(|s| action(s) == Some("actions/upload-artifact"))
        .expect("the SBOMs are handed on");
    assert_eq!(step_value(&upload, "with", "name"), Some(image.sboms));
    let attest = job_named(&jobs, "attest", image.file);
    assert_eq!(
        crate::workflow::downloads_by_name(attest)
            .iter()
            .filter(|n| *n == image.sboms)
            .count(),
        1,
        "{}: `attest` takes the SBOMs by their one name",
        image.file
    );
}

/// What `sbom` asks of the pushed images, a run that publishes nothing asks of the image it
/// built, so a change that loses them is seen on its pull request: the same syft, checked the
/// same way, for the runner's own architecture.
#[test]
fn a_run_that_publishes_nothing_asks_the_same_of_each_architectures_sbom() {
    for image in &IMAGES {
        let jobs = image_jobs(image);
        let job = job_named(&jobs, "image", image.file);
        let step = job
            .steps()
            .into_iter()
            .find(|s| s[0].contains("SBOM names"))
            .unwrap_or_else(|| panic!("{}: `image` checks no SBOM", image.file));
        assert_eq!(
            step.iter()
                .find_map(|l| l.trim().strip_prefix("if:"))
                .map(str::trim),
            Some("env.PUBLISH != 'true'"),
            "{}",
            image.file
        );
        let sbom = job_named(&jobs, "sbom", image.file);
        assert_eq!(
            step_value(&step, "env", "SYFT_VERSION").map(str::to_owned),
            Some(syft_pin(sbom, "SYFT_VERSION")),
            "{}",
            image.file
        );
        assert_eq!(
            step_value(&step, "env", "SYFT_SHA256"),
            Some("${{ matrix.syft_sha256 }}"),
            "{}",
            image.file
        );
        let run = run_lines(&step).join("\n");
        assert!(run.contains("sha256sum -c -"), "{}: {run}", image.file);
    }
}

#[test]
fn only_attest_stores_attestations_and_it_pushes_provenance_and_each_sbom_beside_the_image() {
    for image in &IMAGES {
        attest_pushes_provenance_and_each_sbom(image);
    }
}

fn attest_pushes_provenance_and_each_sbom(image: &Image) {
    let jobs = image_jobs(image);
    let attest = job_named(&jobs, "attest", image.file);
    assert_eq!(
        attest.condition(),
        Some("needs.plan.outputs.publish == 'true'")
    );
    for need in ["plan", "index", "sbom"] {
        assert!(
            attest.needs().contains(&need.to_owned()),
            "{:?}",
            attest.needs()
        );
    }
    assert_eq!(
        attest.permissions(),
        Some(pairs(&[
            ("attestations", "write"),
            ("contents", "read"),
            ("id-token", "write"),
            ("packages", "write"),
        ]))
    );
    assert!(
        attest
            .body
            .iter()
            .filter(|l| l.contains("secrets."))
            .all(|l| l.contains("secrets.GITHUB_TOKEN")),
        "it holds no secret but the run's own token"
    );
    let attesters: Vec<&str> = jobs
        .iter()
        .filter(|j| {
            j.permissions()
                .is_some_and(|p| p.iter().any(|(k, v)| k == "attestations" && v == "write"))
        })
        .map(|j| j.name.as_str())
        .collect();
    assert_eq!(attesters, ["attest"]);

    let attests: Vec<Vec<String>> = attest
        .steps()
        .into_iter()
        .filter(|s| action(s) == Some("actions/attest"))
        .collect();
    let subjects: Vec<(Option<&str>, Option<&str>)> = attests
        .iter()
        .map(|s| {
            assert_eq!(
                step_value(s, "with", "push-to-registry"),
                Some("true"),
                "{s:#?}"
            );
            assert_eq!(
                step_value(s, "with", "subject-name"),
                Some("${{ env.IMAGE }}"),
                "{s:#?}"
            );
            (
                step_value(s, "with", "subject-digest"),
                step_value(s, "with", "predicate-type"),
            )
        })
        .collect();
    assert_eq!(
        subjects,
        [
            (Some("${{ steps.digests.outputs.index }}"), None),
            (
                Some("${{ steps.digests.outputs.amd64 }}"),
                Some("https://cyclonedx.org/bom")
            ),
            (
                Some("${{ steps.digests.outputs.arm64 }}"),
                Some("https://cyclonedx.org/bom")
            ),
        ],
        "provenance on the index, each architecture's SBOM on its own image"
    );
    for arch in ["amd64", "arm64"] {
        let path = format!(
            "${{{{ runner.temp }}}}/sbom/{}-{arch}.cdx.json",
            image.package
        );
        assert!(
            attests
                .iter()
                .any(|s| step_value(s, "with", "predicate-path") == Some(path.as_str())),
            "{}: {path}",
            image.file
        );
    }
}

#[test]
fn each_attestation_is_verified_in_the_run_that_pushed_it() {
    for image in &IMAGES {
        attestations_are_verified(image);
    }
}

fn attestations_are_verified(image: &Image) {
    let jobs = image_jobs(image);
    let attest = job_named(&jobs, "attest", image.file);
    let run = commands(attest);
    let verifies: Vec<&String> = run
        .iter()
        .filter(|l| l.starts_with("gh attestation verify"))
        .collect();
    assert_eq!(verifies.len(), 2, "provenance, then each SBOM: {run:#?}");
    let joined = run.join("\n");
    let signer = format!(
        "--signer-workflow \"$GITHUB_REPOSITORY/.github/workflows/{}\"",
        image.file
    );
    for pin in [
        "--repo \"$GITHUB_REPOSITORY\"",
        signer.as_str(),
        "--signer-digest \"$GITHUB_WORKFLOW_SHA\"",
        "--source-digest \"$GITHUB_SHA\"",
        "--deny-self-hosted-runners",
        "--predicate-type https://cyclonedx.org/bom",
        "oci://$IMAGE@",
    ] {
        assert!(joined.contains(pin), "{}: {pin}: {joined}", image.file);
    }
    let last_attest = attest
        .steps()
        .iter()
        .rposition(|s| action(s) == Some("actions/attest"))
        .expect("attested");
    let first_verify = attest
        .steps()
        .iter()
        .position(|s| {
            run_lines(s)
                .iter()
                .any(|l| l.starts_with("gh attestation verify"))
        })
        .expect("verified");
    assert!(
        last_attest < first_verify,
        "verified after every attestation is stored"
    );
}

#[test]
fn the_release_grants_its_runner_image_call_the_attestation_scope() {
    let release = workflow::release_jobs();
    let call = job_named(&release, "runner-image", "release.yml");
    assert_eq!(
        call.permissions(),
        Some(pairs(&[
            ("attestations", "write"),
            ("contents", "read"),
            ("id-token", "write"),
            ("packages", "write"),
        ])),
        "the most the called workflow's jobs ask for"
    );
}

/// The step that checks each architecture's digest file holds a digest and nothing else.
const DIGESTS_CHECKED: &str =
    "[[ \"$(cat \"$RUNNER_TEMP/digests/$arch\")\" =~ ^sha256:[0-9a-f]{64}$ ]] \\";

#[test]
fn each_job_that_reads_the_digests_checks_each_is_a_digest_before_using_it() {
    for image in &IMAGES {
        digests_are_checked(image);
    }
}

fn digests_are_checked(image: &Image) {
    let jobs = image_jobs(image);
    for name in ["index", "sbom", "attest"] {
        let job = job_named(&jobs, name, image.file);
        let steps = job.steps();
        let check = steps
            .iter()
            .position(|s| run_lines(s).iter().any(|l| l == DIGESTS_CHECKED))
            .unwrap_or_else(|| panic!("{}: `{name}` checks no digest file", image.file));
        let first_use = steps
            .iter()
            .position(|s| {
                run_lines(s)
                    .iter()
                    .any(|l| l.contains("$(cat \"$RUNNER_TEMP/digests/") && l != DIGESTS_CHECKED)
            })
            .unwrap_or_else(|| panic!("{}: `{name}` reads no digest file", image.file));
        assert!(
            check < first_use,
            "`{name}` checks the digests before it uses them"
        );
    }
}
