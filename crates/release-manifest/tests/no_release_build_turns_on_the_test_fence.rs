// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! No build `release.yml` makes turns on the test fence (#1517).
//!
//! The fence is purlis-core's `fenced` feature (`src/fence.rs`): a build that carries it dies
//! rather than act on a project outside the tree its run was given, and it also relaxes which
//! folders a provider's program may be run from, because every test fixture lives in a temp
//! folder. Both are right for a test and wrong for anything a person installs. The app's `e2e`
//! feature carries the fence too, and `test-support` opens a seam only tests may call.
//!
//! Cargo turns a feature on in three ways, and each is held here:
//! - a build command names it (`--features`, `-F`, `--all-features`), in the workflow or in the
//!   runner script Tauri builds the app through;
//! - a crate's `[dependencies]` asks for it (the shipped crates ask only from
//!   `[dev-dependencies]`, which `cargo build` does not unify);
//! - a crate's `default` feature list names it.

mod workflow;

use std::path::PathBuf;

use workflow::{release_jobs, run_lines};

/// The features that make a build a test build.
const TEST_ONLY: [&str; 3] = ["fenced", "e2e", "test-support"];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Why a command line turns on a feature, or `None`.
fn turns_on_a_feature(line: &str) -> Option<&'static str> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.contains(&"--all-features") {
        return Some("it turns on every feature, the fence among them");
    }
    let names_features = words
        .iter()
        .any(|w| *w == "--features" || w.starts_with("--features=") || w.starts_with("-F"));
    names_features.then_some("a release build names no feature: the defaults are what ships")
}

/// Whether the line builds Rust: Cargo itself, or Tauri, which runs Cargo.
fn builds(line: &str) -> bool {
    line.contains("cargo ") || line.contains("tauri build")
}

#[test]
fn no_build_command_in_the_release_names_a_feature() {
    let mut problems = Vec::new();
    let mut seen = 0;
    for job in release_jobs() {
        for step in job.steps() {
            for line in run_lines(&step).iter().filter(|l| builds(l)) {
                seen += 1;
                if let Some(why) = turns_on_a_feature(line) {
                    problems.push(format!("job `{}`: `{line}`: {why}", job.name));
                }
            }
        }
    }
    // The CLI's build, the app's build and the extra bundle's, at least: a reader that found
    // none would pass for the wrong reason.
    assert!(seen >= 3, "only {seen} build lines read in release.yml");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_runner_tauri_builds_through_adds_nothing_to_the_command() {
    let script = workflow::read(&repo().join("tools/auditable-cargo"));
    let commands: Vec<&str> = script
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(commands, [r#"exec cargo auditable "$@""#]);
}

/// The workspace's members, from the root `Cargo.toml`'s one-line `members = [...]`.
fn members() -> Vec<String> {
    let manifest = workflow::read(&repo().join("Cargo.toml"));
    let line = manifest
        .lines()
        .find_map(|l| l.strip_prefix("members = "))
        .expect("the workspace lists its members on one line");
    line.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|m| m.trim().trim_matches('"').to_owned())
        .filter(|m| !m.is_empty())
        .collect()
}

/// What in one crate's manifest turns a test-only feature on for a plain build.
fn what_turns_the_fence_on(member: &str, text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut section = String::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            section = line.to_owned();
            continue;
        }
        let names_one = TEST_ONLY
            .iter()
            .any(|f| line.contains(&format!("\"{f}\"")) || line.contains(&format!("/{f}\"")));
        // `[dependencies]`, `[build-dependencies]` and each `[target.….dependencies]`.
        let a_dependency_section = section == "[dependencies]"
            || section == "[build-dependencies]"
            || section.ends_with(".dependencies]");
        if a_dependency_section && names_one {
            problems.push(format!(
                "{member}/Cargo.toml:{}: {section} asks for `{line}`",
                n + 1
            ));
        }
        if section == "[features]" && line.starts_with("default") && names_one {
            problems.push(format!(
                "{member}/Cargo.toml:{}: the default features name `{line}`",
                n + 1
            ));
        }
    }
    problems
}

#[test]
fn no_crate_turns_the_fence_on_outside_its_tests() {
    let members = members();
    for shipped in [
        "crates/purlis-core",
        "crates/purlis-cli",
        "crates/persona-statistics",
        "app/src-tauri",
    ] {
        assert!(
            members.iter().any(|m| m == shipped),
            "{shipped} not in {members:?}"
        );
    }
    let problems: Vec<String> = members
        .iter()
        .flat_map(|member| {
            let text = workflow::read(&repo().join(member).join("Cargo.toml"));
            what_turns_the_fence_on(member, &text)
        })
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_readers_see_a_fence_turned_on() {
    // Each way in, written the way a change would write it, is caught.
    for line in [
        "cargo auditable build --profile \"$PROFILE\" --locked -p purlis-cli --features e2e",
        "cargo build --all-features",
        "npx tauri build --bundles app -- --profile release -F purlis-core/fenced",
        "cargo build --features=fenced",
    ] {
        assert!(turns_on_a_feature(line).is_some(), "{line}");
    }
    assert!(
        turns_on_a_feature("cargo auditable build --profile \"$PROFILE\" --locked -p purlis-cli")
            .is_none()
    );

    let manifest = "[dependencies]\npurlis-core = { workspace = true, features = [\"fenced\"] }\n";
    assert_eq!(what_turns_the_fence_on("x", manifest).len(), 1);
    let targeted = "[target.'cfg(unix)'.dependencies]\npurlis-core = { workspace = true, features = [\"test-support\"] }\n";
    assert_eq!(what_turns_the_fence_on("x", targeted).len(), 1);
    let defaults = "[features]\ndefault = [\"e2e\"]\ne2e = [\"purlis-core/fenced\"]\n";
    assert_eq!(what_turns_the_fence_on("x", defaults).len(), 1);
    let tests_only =
        "[dev-dependencies]\npurlis-core = { workspace = true, features = [\"fenced\"] }\n";
    assert!(what_turns_the_fence_on("x", tests_only).is_empty());
}
