// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every release carries a CycloneDX SBOM for each platform it ships, and every Rust binary in
//! it carries its own dependency list (`cargo-auditable`) (#584).
//!
//! The SBOM is read off what was BUILT, not off the lockfile: `build` links each binary with
//! `cargo auditable`, which embeds the exact crates it was compiled from, and the unprivileged
//! `sbom` job reads those back out of the shipped bundles with syft, beside the web front
//! end's production packages from `app/package-lock.json`. So the SBOM names what is in the
//! download, and a binary built without its dependency list fails the release instead of
//! producing an SBOM that quietly lists nothing.
//!
//! The SBOM files are build files like any other: uploaded as a `purlis-*` artifact, attested
//! by `provenance`, verified and uploaded by `publish`.
//!
//! The workflow is read by `workflow/mod.rs`, which the other workflow tests share.

use crate::workflow;

use std::path::PathBuf;

use workflow::{Job, downloads_by_name, job, release_jobs, run_lines, step_uses, step_value};

/// The wrapper Tauri's `--runner` calls in place of `cargo`.
const RUNNER: &str = "tools/auditable-cargo";

#[test]
fn every_rust_binary_the_release_ships_is_built_with_its_dependency_list() {
    let jobs = release_jobs();
    let build = job(&jobs, "build");

    // One exact cargo-auditable, the pinned prebuilt `runner-binary-aarch64` installs too,
    // checked against the digest install-action's manifest holds for it: nothing in the job
    // that holds the signing keys compiles a tool.
    let install = step_with(build, "taiki-e/install-action");
    assert_eq!(
        step_value(&install, "with", "tool"),
        Some("cargo-auditable@0.7.6")
    );
    let arm = job(&jobs, "runner-binary-aarch64");
    assert_eq!(
        step_uses(&step_with(arm, "taiki-e/install-action")),
        step_uses(&install),
        "build and the aarch64 runner binary install the same pinned release"
    );
    let installs: Vec<String> = build
        .steps()
        .iter()
        .flat_map(|s| run_lines(s))
        .filter(|l| l.contains("cargo install"))
        .collect();
    assert!(installs.is_empty(), "build compiles a tool: {installs:#?}");

    // ONE cargo run, Tauri's own, through the wrapper so every binary it links carries its
    // dependency list. The app, the command line and the extensions are one feature set and
    // one compile (R1, #7380); `-- --profile` is the profile every later step reads (ADR 0092).
    assert_eq!(
        run_lines(&step(
            build,
            "Compile the app, the purlis binary and the built-in extensions"
        )),
        [format!(
            r#"npx tauri build --no-bundle --config src-tauri/tauri.compile.conf.json --runner "$GITHUB_WORKSPACE/{RUNNER}" -- --profile "$PROFILE" --locked -p purlis-app -p purlis-cli -p persona-statistics"#
        )]
    );

    // The bundles are made by `tauri bundle`, which runs no cargo.
    assert_eq!(
        run_lines(&step(build, "Bundle the app"))
            .last()
            .map(String::as_str),
        Some(r#"npx tauri bundle --config src-tauri/tauri.build.conf.json --bundles "$BUNDLES""#)
    );
    assert_eq!(
        run_lines(&step(build, "Bundle the extra bundle")),
        [
            r#"npx tauri bundle --config src-tauri/tauri.build.conf.json --bundles ${{ matrix.bundles }},${{ matrix.extra_bundles }}"#
        ]
    );

    // Nothing else in the job compiles: one `tauri build`, and no `cargo`, `--runner` or
    // `tauri build` in any other step's commands.
    let others: Vec<String> = build
        .steps()
        .iter()
        .filter(|s| {
            s[0].trim() != "- name: Compile the app, the purlis binary and the built-in extensions"
        })
        .flat_map(|s| run_lines(s))
        .filter(|l| l.contains("cargo") || l.contains("--runner") || l.contains("tauri build"))
        .collect();
    assert!(others.is_empty(), "a second compile in build: {others:#?}");
}

#[test]
fn the_runner_is_cargo_auditable_and_nothing_else() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(RUNNER);
    let script = workflow::read(&path);
    let code: Vec<&str> = script
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .collect();
    assert_eq!(code, [r#"exec cargo auditable "$@""#], "{script}");
    assert!(script.starts_with("#!/bin/sh\n"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "{RUNNER} is not executable");
    }
}

/// The step called `name` in `job`.
fn step(job: &Job, name: &str) -> Vec<String> {
    job.steps()
        .into_iter()
        .find(|s| s[0].trim() == format!("- name: {name}"))
        .unwrap_or_else(|| panic!("job `{}` has no step `{name}`", job.name))
}

/// The step of `job` that uses `action`.
fn step_with(job: &Job, action: &str) -> Vec<String> {
    job.steps()
        .into_iter()
        .find(|s| step_uses(s).is_some_and(|u| u.starts_with(action)))
        .unwrap_or_else(|| panic!("job `{}` has no step using `{action}`", job.name))
}

fn position(lines: &[String], want: &str) -> usize {
    lines
        .iter()
        .position(|l| l == want)
        .unwrap_or_else(|| panic!("`{want}` is not a command of {lines:#?}"))
}

#[test]
fn the_sbom_job_holds_nothing_and_reads_the_sbom_off_the_built_bundles() {
    let jobs = release_jobs();
    let sbom = job(&jobs, "sbom");

    assert!(
        sbom.needs().contains(&"build".to_owned()),
        "{:?}",
        sbom.needs()
    );
    assert_eq!(
        sbom.permissions(),
        Some(vec![("contents".to_owned(), "read".to_owned())]),
        "the SBOM is made in a job that holds nothing"
    );
    assert!(!sbom.reads_a_secret() && !sbom.publishes() && sbom.environment().is_none());
    assert!(!sbom.can_mint_an_oidc_token());
    assert_eq!(
        downloads_by_name(sbom),
        [
            "purlis-linux-x86_64-${{ needs.plan.outputs.version }}",
            "purlis-macos-arm64-${{ needs.plan.outputs.version }}",
        ]
    );

    // Only GitHub's own actions. A third-party action gets the run's artifact token, and with
    // it could upload an artifact that provenance would attest and publish would ship.
    let actions: Vec<String> = sbom
        .steps()
        .iter()
        .filter_map(|s| step_uses(s))
        .map(|u| u.split('@').next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        actions,
        [
            "actions/checkout",
            "actions/download-artifact",
            "actions/download-artifact",
            "actions/upload-artifact",
        ]
    );

    // syft, at one version, checked against a sha256 committed here BEFORE it is unpacked,
    // and run from a `run:` step, which is handed no runtime token.
    let syft = step(
        sbom,
        "syft, at one version and checksum, reads each platform's bundle",
    );
    let version = step_value(&syft, "env", "SYFT_VERSION").expect("SYFT_VERSION");
    assert!(
        version.split('.').count() == 3
            && version
                .split('.')
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
        "SYFT_VERSION is one exact version: {version}"
    );
    let sha = step_value(&syft, "env", "SYFT_SHA256").expect("SYFT_SHA256");
    assert!(
        sha.len() == 64
            && sha
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "SYFT_SHA256 is a sha256: {sha}"
    );
    let lines = run_lines(&syft);
    let fetch = position(
        &lines,
        r#"curl -fsSL --proto '=https' --retry 5 --retry-all-errors --retry-delay 20 -o "$RUNNER_TEMP/syft.tar.gz" "https://github.com/anchore/syft/releases/download/v${SYFT_VERSION}/syft_${SYFT_VERSION}_linux_amd64.tar.gz""#,
    );
    let check = position(
        &lines,
        r#"echo "$SYFT_SHA256  $RUNNER_TEMP/syft.tar.gz" | sha256sum -c -"#,
    );
    let unpack = position(
        &lines,
        r#"tar -xzf "$RUNNER_TEMP/syft.tar.gz" -C "$RUNNER_TEMP" syft"#,
    );
    assert!(
        fetch < check && check < unpack,
        "fetched, checked, THEN unpacked"
    );
    for platform in ["macos-arm64", "linux-x86_64"] {
        let scan = position(
            &lines,
            &format!(
                r#""$RUNNER_TEMP/syft" scan "dir:sbom/{platform}" -o "cyclonedx-json=out/purlis-{platform}.cdx.json""#
            ),
        );
        assert!(unpack < scan);
    }

    // What proves cargo-auditable worked: each shipped binary's own crate is in the SBOM.
    let names = step(
        sbom,
        "Each SBOM names every binary it ships, and the front end",
    );
    assert_eq!(
        step_value(&names, "env", "REQUIRED_PURLS"),
        Some("pkg:cargo/purlis-app@ pkg:cargo/purlis-cli@ pkg:cargo/persona-statistics@ pkg:npm/")
    );
    let lines = run_lines(&names);
    position(&lines, r#"for sbom in out/*.cdx.json; do"#);
    position(
        &lines,
        r#"purls=$(jq -r '.components[]?.purl // empty' "$sbom")"#,
    );
    position(&lines, r#"for want in $REQUIRED_PURLS; do"#);

    let upload = sbom
        .steps()
        .into_iter()
        .find(|s| step_uses(s).is_some_and(|u| u.starts_with("actions/upload-artifact@")))
        .expect("the sbom job uploads what it made");
    assert_eq!(
        step_value(&upload, "with", "name"),
        Some("purlis-sbom-${{ needs.plan.outputs.version }}")
    );
    assert_eq!(step_value(&upload, "with", "path"), Some("out/"));
}

#[test]
fn the_sbom_is_attested_and_published_like_every_build_file() {
    let jobs = release_jobs();
    let provenance = job(&jobs, "provenance");
    assert!(
        provenance.needs().contains(&"sbom".to_owned()),
        "provenance attests the SBOM, so it waits for it: {:?}",
        provenance.needs()
    );

    // `publish` leaves `.json` files out of the release assets so that a manifest is only ever
    // uploaded last and on its own. The SBOM is `.cdx.json` and has to get through.
    let publish = job(&jobs, "publish");
    let filters: Vec<&String> = publish
        .body
        .iter()
        .filter(|l| l.contains("== *.json"))
        .collect();
    assert_eq!(filters.len(), 2, "one per channel: {filters:#?}");
    for line in filters {
        assert!(
            line.contains("!= *.cdx.json"),
            "this filter drops the SBOM: {line}"
        );
    }
}
