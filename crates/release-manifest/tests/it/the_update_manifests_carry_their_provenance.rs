// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The update manifests (`latest.json`, `dev.json` and their weekly twins) carry the same SLSA
//! build provenance as every build file (#778), and the provenance itself is a release asset
//! (#1133).
//!
//! `publish` holds `contents: write`, so it may not also mint the OIDC token an attestation is
//! signed with. The manifest is therefore written BEFORE `publish`, by a `manifest` job that
//! holds nothing (no secret, no environment, `contents: read`), and handed on as a run
//! artifact. `provenance`, the one job that may sign, attests it with the builds; `publish`
//! downloads it, verifies it against this run with the builds, and uploads those exact bytes,
//! last. The attestation's bundle goes up beside the builds as `purlis.intoto.jsonl`, so a
//! person, or a scanner, finds the provenance on the release page.

use crate::workflow;

use workflow::{Job, job, release_jobs, run_lines, step_uses, step_value};

const MANIFEST: &str = "purlis-manifest-${{ needs.plan.outputs.version }}";
const BUNDLE: &str = "purlis-provenance-${{ needs.plan.outputs.version }}";
const BUNDLE_FILE: &str = "purlis.intoto.jsonl";

/// Each `download-artifact` step's `(name, path)`.
fn downloads(job: &Job) -> Vec<(String, String)> {
    job.steps()
        .iter()
        .filter(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/download-artifact@")))
        .map(|s| {
            (
                step_value(s, "with", "name").unwrap_or_default().to_owned(),
                step_value(s, "with", "path").unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// Each `upload-artifact` step's `(name, path)`.
fn uploads(job: &Job) -> Vec<(String, String)> {
    job.steps()
        .iter()
        .filter(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/upload-artifact@")))
        .map(|s| {
            (
                step_value(s, "with", "name").unwrap_or_default().to_owned(),
                step_value(s, "with", "path").unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn pair(name: &str, path: &str) -> (String, String) {
    (name.to_owned(), path.to_owned())
}

fn runs_the_manifest_tool(job: &Job) -> bool {
    job.steps()
        .iter()
        .flat_map(|s| run_lines(s))
        .any(|l| l.contains("-p release-manifest"))
}

#[test]
fn a_job_that_holds_nothing_writes_the_manifest_and_hands_it_on() {
    let jobs = release_jobs();
    let manifest = job(&jobs, "manifest");
    assert_eq!(
        manifest.permissions(),
        Some(vec![pair("contents", "read")]),
        "the manifest job reads the tree and nothing else"
    );
    assert!(
        !manifest.reads_a_secret()
            && !manifest.publishes()
            && manifest.environment().is_none()
            && !manifest.can_mint_an_oidc_token(),
        "the manifest job holds no key, no environment and no token"
    );
    assert!(
        manifest
            .condition()
            .is_some_and(|said| said.contains("needs.plan.outputs.publish == 'true'")),
        "a manifest is written only for a run that publishes: {:?}",
        manifest.condition()
    );
    assert!(manifest.needs().contains(&"build".to_owned()));
    // From the build files by their exact names, never a pattern (see `provenance`).
    assert_eq!(
        downloads(manifest),
        [
            pair(
                "purlis-macos-arm64-${{ needs.plan.outputs.version }}",
                "dist"
            ),
            pair(
                "purlis-linux-x86_64-${{ needs.plan.outputs.version }}",
                "dist"
            ),
            // The runner image's line and its signature (#2749).
            pair(
                "purlis-runner-image-${{ needs.plan.outputs.version }}",
                "runner"
            ),
        ]
    );
    assert!(runs_the_manifest_tool(manifest), "it runs release-manifest");
    // What it wrote goes on as the run artifact `provenance` attests, and nothing else does.
    assert_eq!(uploads(manifest), [pair(MANIFEST, "manifest/")]);
    // Its bytes are what the updater reads, so it restores no cache another job saved.
    for step in manifest.steps() {
        let action = step_uses(&step).unwrap_or_default();
        assert!(
            !action.starts_with("Swatinem/rust-cache@") && !action.starts_with("actions/cache"),
            "the manifest job restores a cache: {step:#?}"
        );
    }
}

#[test]
fn provenance_attests_the_manifest_with_the_builds() {
    let jobs = release_jobs();
    let provenance = job(&jobs, "provenance");
    assert!(
        provenance.needs().contains(&"manifest".to_owned()),
        "provenance needs {:?}; it attests the manifest, so it waits for it",
        provenance.needs()
    );
    // Into `dist`, beside the builds, so the one `subject-path: dist/*` and the one
    // verification loop cover it.
    assert!(
        downloads(provenance).contains(&pair(MANIFEST, "dist")),
        "{:?}",
        downloads(provenance)
    );
}

#[test]
fn publish_uploads_the_attested_manifest_and_writes_none_of_its_own() {
    let jobs = release_jobs();
    let publish = job(&jobs, "publish");
    assert!(
        !runs_the_manifest_tool(publish),
        "publish uploads the manifest provenance attested; one it wrote itself would carry none"
    );
    assert!(
        downloads(publish).contains(&pair(MANIFEST, "dist")),
        "publish takes the manifest into `dist`, where its verification reads every file: {:?}",
        downloads(publish)
    );
}

/// The lines of `publish`'s step called `name`.
fn publish_step(name: &str) -> Vec<String> {
    let jobs = release_jobs();
    let step = job(&jobs, "publish")
        .steps()
        .into_iter()
        .find(|s| s[0].trim() == format!("- name: {name}"))
        .unwrap_or_else(|| panic!("publish has no step `{name}`"));
    run_lines(&step)
}

const CHANNELS: [(&str, &str, &str); 2] = [
    (
        "Stable — the release for the operator's tag",
        "tools/replace-release-assets.sh \"$TAG\" dist/latest.json dist/latest-weekly.json",
        "gh release create",
    ),
    (
        "Dev — replace the assets of the one `dev` prerelease",
        "tools/replace-release-assets.sh dev dist/dev.json dist/dev-weekly.json",
        "tools/replace-release-assets.sh dev \"${assets[@]}\"",
    ),
];

#[test]
fn each_channel_uploads_the_provenance_bundle_with_its_assets_and_the_manifest_last() {
    for (name, manifest, assets) in CHANNELS {
        let lines = publish_step(name);
        let at = |want: &str| {
            lines
                .iter()
                .position(|l| l.contains(want))
                .unwrap_or_else(|| panic!("`{want}` is not in {name}: {lines:#?}"))
        };
        let bundle = at(&format!("assets+=(provenance/{BUNDLE_FILE})"));
        let manifest_at = at(manifest);
        assert!(
            bundle < at(assets),
            "{name}: the bundle goes up with the assets"
        );
        assert!(
            at(assets) < manifest_at,
            "{name}: the manifest goes after the assets"
        );
        // Nothing is uploaded after the manifest.
        assert!(
            !lines[manifest_at + 1..].iter().any(|l| {
                l.contains("replace-release-assets.sh")
                    || l.contains("gh release upload")
                    || l.contains("gh release create")
            }),
            "{name}: an upload follows the manifest: {lines:#?}"
        );
    }
}

#[test]
fn provenance_hands_its_bundle_on_and_publish_keeps_it_out_of_the_verified_files() {
    let jobs = release_jobs();
    let provenance = job(&jobs, "provenance");
    let steps = provenance.steps();
    let attest = steps
        .iter()
        .find(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/attest@")))
        .expect("provenance runs actions/attest");
    assert!(
        attest.iter().any(|l| l.trim() == "id: attest"),
        "{attest:#?}"
    );
    let written = steps
        .iter()
        .find(|s| {
            s.iter()
                .any(|l| l.contains("steps.attest.outputs.bundle-path"))
        })
        .expect("a step reads the attestation's bundle");
    let lines = run_lines(written);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(&format!("> provenance/{BUNDLE_FILE}"))),
        "the bundle is written as {BUNDLE_FILE}, one line per bundle: {lines:#?}"
    );
    assert_eq!(uploads(provenance), [pair(BUNDLE, "provenance/")]);

    // The bundle is not itself an attested file, so it lands outside `dist`, which
    // publish's verification reads file by file.
    assert!(
        downloads(job(&jobs, "publish")).contains(&pair(BUNDLE, "provenance")),
        "{:?}",
        downloads(job(&jobs, "publish"))
    );
}

#[test]
fn no_job_that_can_write_the_release_or_holds_a_secret_can_sign() {
    let jobs = release_jobs();
    for job in jobs.iter().filter(|j| j.can_mint_an_oidc_token()) {
        // The runner image's call signs the image keyless, and so writes its package (#2749):
        // the registry, never the release.
        let image = job.name == "runner-image";
        let writes = job.permissions().unwrap_or_default().iter().any(|(k, v)| {
            v == "write" && k != "id-token" && k != "attestations" && !(image && k == "packages")
        });
        assert!(
            !writes && !job.reads_a_secret() && !job.publishes() && job.environment().is_none(),
            "job `{}` can sign provenance and holds more",
            job.name
        );
    }
}
