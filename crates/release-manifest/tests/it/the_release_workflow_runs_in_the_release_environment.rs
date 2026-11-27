// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every job in `release.yml` that reads a signing secret or publishes a release runs in the
//! `release` environment whenever the run publishes (#344).
//!
//! The environment is where the secrets live, and its deployment policy is what keeps them off
//! any ref but `main` and `v*` tags. A job that reads `secrets.*` without naming it gets an
//! empty string: `keys` would read "no updater key" and skip every dev build, and `build` would
//! bundle unsigned. So the rule is checked on the file, the way it is written, rather than
//! found out on the next release.
//!
//! The workflow is read by `workflow/mod.rs`, which the pinning and cache tests share.

use crate::workflow;

use workflow::{Job, release_jobs};

/// What each guarded job's `environment:` says, exactly.
///
/// `plan` takes none (R2, #7381): it runs the commit's own tools. `keys` asks which secrets are
/// set, so it keys on the event, and NAMES the one that can publish: a push, of a `v*` tag or to
/// `main` (the jobs that sign, upload or publish run only once the gate saw that commit's `ci`
/// green, and `build` waits for it itself before it does). A hand-started build and the nightly
/// profile guard get no environment, and neither does an event added to `on:` later, until
/// this line says so (ADR 0092). `build` keys on what `keys` decided. `publish` only runs
/// when the run publishes, so it names the environment outright. And `x && 'release' || ''`,
/// never `x && '' || 'release'`: an empty string is falsy in a workflow expression, so the
/// second form is `release` every time.
const EXPECTED: [(&str, &str); 8] = [
    // Reads the secrets as booleans: is the updater key set, is a Developer ID.
    (
        "keys",
        "${{ needs.plan.outputs.publish == 'true' && 'release' || '' }}",
    ),
    (
        "build",
        "${{ needs.keys.outputs.publish == 'true' && 'release' || '' }}",
    ),
    // Holds the Developer ID key and the notarization key, on a GitHub-hosted runner, and the
    // updater's key for the macOS archive; runs only when the run publishes (ADR 0042, amended
    // 2026-11-25), so it names the environment outright.
    ("sign-macos", "release"),
    // Signs the runner image's line with the updater's key (#2749).
    ("runner-image-signature", "release"),
    // Builds the Linux aarch64 runner binary with the Report intake's address compiled in, as
    // `build` does, keyed on what `plan` decided as `build` is (#723).
    (
        "runner-binary-aarch64",
        "${{ needs.keys.outputs.publish == 'true' && 'release' || '' }}",
    ),
    // Signs each runner platform's purlis with the updater's key when the run publishes (#723).
    (
        "runner-binaries",
        "${{ needs.keys.outputs.publish == 'true' && 'release' || '' }}",
    ),
    ("publish", "release"),
    // Reads the Report intake's upload token, which lives in the environment beside the signing
    // keys and is held to the same refs (#5692).
    ("symbols", "release"),
];

#[test]
fn every_job_that_reads_a_signing_secret_or_publishes_runs_in_the_release_environment() {
    let jobs = release_jobs();
    let guarded: Vec<&Job> = jobs
        .iter()
        .filter(|job| job.reads_a_secret() || job.publishes())
        .collect();

    // Found by what they do, so a new job that reads a secret or publishes lands here and has
    // to be given its line in `EXPECTED`.
    let names: Vec<&str> = guarded.iter().map(|job| job.name.as_str()).collect();
    assert_eq!(names, EXPECTED.map(|(name, _)| name), "{names:?}");

    for (job, (_, expected)) in guarded.iter().zip(EXPECTED) {
        assert_eq!(
            job.environment(),
            Some(expected),
            "job `{}` reads a signing secret or publishes",
            job.name
        );
    }
}

#[test]
fn no_other_job_names_an_environment() {
    for job in release_jobs() {
        if EXPECTED.iter().all(|(name, _)| *name != job.name) {
            assert_eq!(job.environment(), None, "job `{}`", job.name);
        }
    }
}
