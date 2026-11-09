// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The licences and advisories checks install cargo-deny from a cache `main` saved, and ask
//! GitHub for it only when that cache misses (#5936).
//!
//! The release it comes from answered 5xx for hours on 2026-11-08 and 2026-11-09, and each time
//! a pull request's `rust (licences, advisories)` went red for a reason that was not its change.
//! The tarball is fixed by its version and its SHA-256, so the cache's key is both, a hit is
//! checked against the same SHA-256 as a download, and a run whose key `main` saved never leaves
//! the runner for it. One composite action, `.github/actions/cargo-deny`, holds the version, the
//! digest and the fetch, for `ci` and `operator` alike.

use crate::workflow;

use workflow::{action_steps, job_named, jobs, local_action, read, step_uses, workflows_dir};

const ACTION: &str = "./.github/actions/cargo-deny";

/// The workflows that check licences and advisories, and the job that does.
const CHECKS: [(&str, &str); 2] = [("ci.yml", "deny"), ("operator.yml", "checks")];

fn action() -> Vec<Vec<String>> {
    let path = local_action(ACTION).unwrap_or_else(|| panic!("{ACTION} has no action.yml"));
    action_steps(&read(&path))
}

/// The step whose `uses:` starts with `action`, or a panic naming it.
fn step_using<'a>(steps: &'a [Vec<String>], action: &str) -> &'a [String] {
    steps
        .iter()
        .find(|step| step_uses(step).is_some_and(|uses| uses.starts_with(action)))
        .unwrap_or_else(|| panic!("{ACTION} has no step using {action}"))
}

/// The step that runs `command`, or a panic naming it.
fn step_running<'a>(steps: &'a [Vec<String>], command: &str) -> &'a [String] {
    steps
        .iter()
        .find(|step| step.iter().any(|line| line.contains(command)))
        .unwrap_or_else(|| panic!("{ACTION} has no step running {command}"))
}

fn condition(step: &[String]) -> Option<&str> {
    step.iter().find_map(|line| {
        let line = line.trim_start();
        let line = line.strip_prefix("- ").unwrap_or(line);
        line.strip_prefix("if:").map(str::trim)
    })
}

#[test]
fn each_licences_check_installs_cargo_deny_through_the_one_action() {
    for (file, name) in CHECKS {
        let text = read(&workflows_dir().join(file));
        let jobs = jobs(&text);
        let job = job_named(&jobs, name, file);
        assert!(
            job.steps()
                .iter()
                .any(|step| step_uses(step) == Some(ACTION)),
            "{file}'s `{name}` job installs cargo-deny with {ACTION}"
        );
        assert!(
            !text.contains("-R EmbarkStudios/cargo-deny") && !text.contains("DENY_SHA256"),
            "{file} fetches cargo-deny itself: the version, its digest and the fetch are the \
             action's alone"
        );
    }
}

#[test]
fn the_cache_is_keyed_on_the_version_and_its_digest_and_restored_first() {
    let steps = action();
    let restore = step_using(&steps, "actions/cache/restore@");
    let key = restore
        .iter()
        .find_map(|line| line.trim().strip_prefix("key:"))
        .expect("the restore names a key");
    assert!(
        key.contains("DENY_VERSION") || key.contains("version"),
        "the key holds the version: {key}"
    );
    assert!(
        key.contains("DENY_SHA256") || key.contains("sha256"),
        "the key holds the digest: {key}"
    );
    let at = |step: &[String]| steps.iter().position(|one| one == step);
    assert!(
        at(restore) < at(step_running(&steps, "gh release download")),
        "the cache is restored before anything is fetched"
    );
}

#[test]
fn github_is_asked_only_when_the_cache_misses_and_a_hit_is_checked_as_a_download_is() {
    let steps = action();
    let restore = step_using(&steps, "actions/cache/restore@");
    let id = restore
        .iter()
        .find_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("- ").unwrap_or(line);
            line.strip_prefix("id:").map(str::trim)
        })
        .expect("the restore has an id its hit is read by");
    let fetch = step_running(&steps, "gh release download");
    let fetched_if = condition(fetch).expect("the fetch runs only on a miss");
    assert!(
        fetched_if.contains(&format!("steps.{id}.outputs.cache-hit")) && fetched_if.contains("!="),
        "the fetch runs only on a miss: {fetched_if}"
    );
    let check = step_running(&steps, "sha256sum --check");
    assert_eq!(
        condition(check),
        None,
        "the digest is checked whether the tarball was restored or fetched"
    );
    let at = |step: &[String]| steps.iter().position(|one| one == step);
    assert!(
        at(fetch) < at(check),
        "the check reads what the fetch wrote"
    );
}

#[test]
fn only_main_saves_the_cache_and_only_once_its_digest_was_checked() {
    let steps = action();
    let save = step_using(&steps, "actions/cache/save@");
    let saved_if = condition(save).expect("the save says when");
    assert!(
        saved_if.contains("github.ref == 'refs/heads/main'"),
        "only main saves: {saved_if}"
    );
    assert!(
        saved_if.contains("cache-hit != 'true'"),
        "a hit is not saved again: {saved_if}"
    );
    let at = |step: &[String]| steps.iter().position(|one| one == step);
    assert!(
        at(step_running(&steps, "sha256sum --check")) < at(save),
        "nothing is saved before its digest is checked"
    );
}
