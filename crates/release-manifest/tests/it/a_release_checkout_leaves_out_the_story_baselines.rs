// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every `actions/checkout` in `release.yml` leaves the story-test screenshot baselines out and
//! never downloads their blobs.
//!
//! About 950 MB of a ~1 GB tree is PNG under `**/__screenshots__/`, which no release job builds,
//! runs or reads; on a self-hosted Mac at 31 Mbit/s the full checkout took 30 minutes. So each
//! checkout is a partial clone (`filter: blob:none`: blobs only for files checked out), with a
//! non-cone sparse checkout of everything but `__screenshots__` folders. `fetch-depth` stays at
//! its default of 1. A job that needed a baseline would keep a full checkout and say why here.

use crate::workflow;

use workflow::{jobs, read, step_uses, step_value, workflows_dir};

/// The lines under `sparse-checkout: |`, trimmed.
fn sparse_patterns(step: &[String]) -> Vec<String> {
    let Some(at) = step
        .iter()
        .position(|l| l.trim_end() == "          sparse-checkout: |")
    else {
        return Vec::new();
    };
    step[at + 1..]
        .iter()
        .take_while(|l| l.starts_with("            "))
        .map(|l| l.trim().to_owned())
        .collect()
}

fn checkouts_are_partial_and_sparse(text: &str) -> Result<(), String> {
    let mut seen = 0;
    for job in jobs(text) {
        for step in job.steps() {
            if !step_uses(&step).is_some_and(|u| u.starts_with("actions/checkout@")) {
                continue;
            }
            seen += 1;
            let at = &job.name;
            let want = [
                ("filter", "blob:none"),
                ("sparse-checkout-cone-mode", "false"),
            ];
            for (key, value) in want {
                if step_value(&step, "with", key) != Some(value) {
                    return Err(format!("`{at}`: checkout does not set `{key}: {value}`"));
                }
            }
            if sparse_patterns(&step) != ["/*", "!**/__screenshots__/"] {
                return Err(format!(
                    "`{at}`: checkout's sparse-checkout is not `/*` then `!**/__screenshots__/`"
                ));
            }
            if step_value(&step, "with", "fetch-depth").is_some() {
                return Err(format!(
                    "`{at}`: checkout sets `fetch-depth`; it stays at 1"
                ));
            }
        }
    }
    if seen == 0 {
        return Err("release.yml has no checkout at all".to_owned());
    }
    Ok(())
}

#[test]
fn every_release_checkout_leaves_the_story_baselines_out() {
    let text = read(&workflows_dir().join("release.yml"));
    if let Err(why) = checkouts_are_partial_and_sparse(&text) {
        panic!("{why}");
    }
}

#[test]
fn a_checkout_that_downloads_every_blob_is_refused() {
    let text = read(&workflows_dir().join("release.yml")).replacen(
        "filter: blob:none",
        "filter: tree:0",
        1,
    );
    assert!(checkouts_are_partial_and_sparse(&text).is_err());
}

#[test]
fn a_checkout_that_keeps_the_baselines_is_refused() {
    let text = read(&workflows_dir().join("release.yml")).replacen(
        "            !**/__screenshots__/\n",
        "",
        1,
    );
    assert!(checkouts_are_partial_and_sparse(&text).is_err());
}

#[test]
fn a_checkout_in_cone_mode_is_refused() {
    let text = read(&workflows_dir().join("release.yml")).replacen(
        "sparse-checkout-cone-mode: false",
        "sparse-checkout-cone-mode: true",
        1,
    );
    assert!(checkouts_are_partial_and_sparse(&text).is_err());
}

#[test]
fn a_checkout_that_deepens_the_history_is_refused() {
    let text = read(&workflows_dir().join("release.yml")).replacen(
        "          filter: blob:none\n",
        "          fetch-depth: 0\n          filter: blob:none\n",
        1,
    );
    assert!(checkouts_are_partial_and_sparse(&text).is_err());
}
