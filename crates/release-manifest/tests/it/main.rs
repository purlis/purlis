// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The release's guards, as one test binary (#3613, #3752).
//!
//! Cargo builds every `tests/*.rs` as a binary of its own, and each one links the crate's whole
//! dependency graph, the core's included: 19 of them here, until 2026-10-29. Here each former
//! file is a module, so the crate links one binary, as `purlis-cli` (#3614) and `purlis-core`
//! (#3749) do. A test's name is its old name behind its file's:
//! `the_workflows_run_only_what_they_pinned::a_test`, run with
//! `cargo test -p release-manifest --test it the_workflows_run_only_what_they_pinned::` or
//! nextest's `-E 'binary(it) & test(/^the_workflows_run_only_what_they_pinned::/)'`.
//!
//! **What one binary changes.** Under nextest, which CI's `rust` job runs, nothing: every test
//! still runs in a process of its own. Under `cargo test` the tests now run as threads of one
//! process instead of 19. Every test here reads files and changes nothing its process shares.

// The workflow reader, in the folder it always had; each module reaches it as `crate::workflow`.
#[path = "../workflow/mod.rs"]
mod workflow;

mod a_dev_channel_build_is_lighter_and_a_stable_build_is_not;
mod a_release_checks_the_notices_its_own_source_ships;
mod a_runner_image_failure_never_holds_back_a_release;
mod every_channel_publishes_its_weekly_manifest;
mod every_published_build_carries_its_provenance;
mod every_release_carries_its_sbom;
mod no_release_build_turns_on_the_test_fence;
mod the_app_keeps_the_hardened_runtime;
mod the_engine_manifest_is_signed_on_main_with_least_privilege;
mod the_forge_nightly_runs_live_with_least_privilege;
mod the_images_are_checked_signed_and_public;
mod the_profile_guard_builds_the_stable_profile_and_holds_nothing;
mod the_release_checks_the_signed_binary_keeps_the_runtime;
mod the_release_workflow_runs_in_the_release_environment;
mod the_runner_image_carries_its_sbom_and_provenance;
mod the_scorecard_runs_weekly_with_least_privilege;
mod the_update_manifests_carry_their_provenance;
mod the_workflow_reader_reads_anchored_steps;
mod the_workflows_run_only_what_they_pinned;

/// A `.rs` file straight under `tests/` is a test binary of its own, which links the whole
/// graph again: the cost this layout exists to remove. A new test file goes in this folder,
/// with a `mod` line above.
#[test]
fn every_test_file_is_a_module_of_this_one_binary() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut binaries: Vec<String> = std::fs::read_dir(&tests)
        .expect("the tests folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    binaries.sort();
    assert!(
        binaries.is_empty(),
        "each of these is a test binary of its own; move it into tests/it/ as a module: {binaries:?}"
    );
}

/// A file in this folder that no `mod` line above names is never compiled, so its tests would
/// quietly stop running.
#[test]
fn every_file_in_this_folder_is_a_module_above() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it");
    let declared = include_str!("main.rs");
    let mut missing: Vec<String> = std::fs::read_dir(&here)
        .expect("this folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
        .filter(|name| name != "main" && !declared.contains(&format!("\nmod {name};\n")))
        .collect();
    missing.sort();
    assert!(
        missing.is_empty(),
        "add `mod <name>;` above for: {missing:?}"
    );
}
