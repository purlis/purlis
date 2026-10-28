// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The macOS app a build signs keeps the hardened runtime and carries no entitlement, read off
//! the signed binary itself (ruling V90a, #1180).
//!
//! A keyring item is held to the app, which holds only while no other code can run as the app.
//! The hardened runtime is what refuses an injected library and an attaching debugger, and an
//! entitlement is how a build gives one of those back. `the_app_keeps_the_hardened_runtime.rs`
//! holds the configuration the bundler reads; this holds the bytes it signed: `codesign`'s own
//! answer, after the bundle's seal is verified, in both signing modes, on every macOS build
//! (the nightly profile guard's too, so a break is found on a night and not on a tag).

use crate::workflow;

use workflow::{Job, job, release_jobs, run_lines};

const CHECK: &str = "The app keeps the hardened runtime and carries no entitlement";

/// The index and lines of the build job's step called `name`.
fn step_named(build: &Job, name: &str) -> (usize, Vec<String>) {
    build
        .steps()
        .into_iter()
        .enumerate()
        .find(|(_, s)| s[0].trim() == format!("- name: {name}"))
        .unwrap_or_else(|| panic!("the build job has no step `{name}`"))
}

#[test]
fn every_macos_build_reads_the_runtime_and_the_entitlements_off_the_signed_app() {
    let jobs = release_jobs();
    let build = job(&jobs, "build");
    let (_, step) = step_named(build, CHECK);
    // Every macOS build, not only a publish: the profile guard and a build started by hand are
    // signed the same way (ad-hoc), and they are where a break shows first.
    assert!(
        step.iter().any(|l| l.trim() == "if: runner.os == 'macOS'"),
        "{step:#?}"
    );
    let lines = run_lines(&step);
    let at = |want: &str| {
        lines
            .iter()
            .position(|l| l.contains(want))
            .unwrap_or_else(|| panic!("`{want}` is not in {lines:#?}"))
    };
    // The seal first: without it, a linker's own signature would answer for a bundle nobody
    // signed.
    let verified = at("codesign --verify --deep --strict");
    let flags = at("codesign -dv --verbose=4");
    let runtime = at("*runtime*)");
    let entitlements = at("codesign -d --entitlements - --xml");
    let none = at("grep -q '<key>'");
    assert!(verified < flags && flags < runtime, "{lines:#?}");
    assert!(verified < entitlements && entitlements < none, "{lines:#?}");
    // In both signing modes alike: the check reads no mode, so neither can skip it.
    assert!(
        !step.iter().any(|l| l.contains("MACOS_SIGNING")),
        "the runtime check is the same for a Developer ID and an ad-hoc build: {step:#?}"
    );
}

#[test]
fn the_check_blocks_everywhere_but_a_dev_publish_until_a_runner_has_measured_it() {
    // Train 79 review: an unmeasured red must not stop the dev channel on every green main, so
    // a dev publish reports it; the nightly guard, a hand-started build and a stable tag block.
    let jobs = release_jobs();
    let build = job(&jobs, "build");
    let (_, step) = step_named(build, CHECK);
    let exempt: Vec<&String> = step
        .iter()
        .filter(|l| l.trim().starts_with("continue-on-error:"))
        .collect();
    assert_eq!(
        exempt.len(),
        1,
        "one exemption, and only for a dev publish: {step:#?}"
    );
    assert_eq!(
        exempt[0].trim(),
        "continue-on-error: ${{ needs.plan.outputs.publish == 'true' && needs.plan.outputs.channel == 'dev' }}"
    );
}

#[test]
fn the_check_runs_on_the_bundle_that_is_uploaded() {
    let jobs = release_jobs();
    let build = job(&jobs, "build");
    let (built, _) = step_named(build, "Build the app");
    let (checked, step) = step_named(build, CHECK);
    let (uploaded, _) = step_named(build, "Upload");
    assert!(built < checked && checked < uploaded);
    assert!(
        run_lines(&step)
            .iter()
            .any(|l| l == "bundle=target/$PROFILE/bundle/macos/purlis.app"),
        "{step:#?}"
    );
}
