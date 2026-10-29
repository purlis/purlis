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

use crate::workflow;

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

// ---------------------------------------------------------------------------------------------
// Train 79 review: the features Cargo would resolve, not only the lines that name one. A
// feature reaches a shipped crate through another feature (`default = ["x"]`, `x =
// ["purlis-core/fenced"]`), through a workspace member's dependency on another member, and
// through the root's `[workspace.dependencies]`, which no member's own manifest spells. Cargo's
// own `metadata --no-deps` says each of those with no network and no build, and the closure
// below follows them the way a `cargo build -p <shipped>` unifies them.

/// The packages the release builds: `-p purlis-cli -p persona-statistics`, and the app Tauri
/// builds.
const SHIPPED: [&str; 3] = ["purlis-cli", "persona-statistics", "purlis-app"];

/// `cargo metadata --no-deps` for this workspace.
fn metadata() -> serde_json::Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(repo())
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("cargo metadata prints JSON")
}

/// Every `(package, feature)` a plain build of `shipped` turns on among the workspace's own
/// packages, from `cargo metadata --no-deps`'s JSON.
fn resolved(
    meta: &serde_json::Value,
    shipped: &[&str],
) -> std::collections::BTreeSet<(String, String)> {
    use std::collections::BTreeSet;
    let packages: Vec<&serde_json::Value> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .collect();
    let find = |name: &str| packages.iter().copied().find(|p| p["name"] == name);
    let mut on: BTreeSet<(String, String)> = BTreeSet::new();
    let mut built: BTreeSet<String> = BTreeSet::new();
    // A work list of packages to build and features to turn on.
    enum Turn {
        Build(String),
        Feature(String, String),
        Dep(String, String),
    }
    let mut todo: Vec<Turn> = Vec::new();
    for name in shipped {
        todo.push(Turn::Build((*name).to_owned()));
        todo.push(Turn::Feature((*name).to_owned(), "default".to_owned()));
    }
    // A dependency of `pkg` by its key in `pkg`'s manifest (its rename, or its name).
    let dep_of = |pkg: &serde_json::Value, key: &str| -> Option<serde_json::Value> {
        pkg["dependencies"]
            .as_array()?
            .iter()
            .find(|d| {
                let kind_ok = d["kind"].is_null() || d["kind"] == "build";
                let name = d["rename"]
                    .as_str()
                    .or(d["name"].as_str())
                    .unwrap_or_default();
                kind_ok && name == key
            })
            .cloned()
    };
    let take = |d: &serde_json::Value, todo: &mut Vec<Turn>| {
        let name = d["name"].as_str().unwrap_or_default().to_owned();
        todo.push(Turn::Build(name.clone()));
        if d["uses_default_features"] != false {
            todo.push(Turn::Feature(name.clone(), "default".to_owned()));
        }
        for f in d["features"].as_array().into_iter().flatten() {
            todo.push(Turn::Feature(
                name.clone(),
                f.as_str().unwrap_or_default().to_owned(),
            ));
        }
    };
    while let Some(turn) = todo.pop() {
        match turn {
            Turn::Build(name) => {
                if !built.insert(name.clone()) {
                    continue;
                }
                let Some(pkg) = find(&name) else { continue };
                for d in pkg["dependencies"].as_array().into_iter().flatten() {
                    let kind_ok = d["kind"].is_null() || d["kind"] == "build";
                    if kind_ok && d["optional"] != true {
                        take(d, &mut todo);
                    }
                }
            }
            Turn::Dep(name, key) => {
                let Some(pkg) = find(&name) else { continue };
                if let Some(d) = dep_of(pkg, &key) {
                    take(&d, &mut todo);
                }
            }
            Turn::Feature(name, feature) => {
                let Some(pkg) = find(&name) else { continue };
                if !on.insert((name.clone(), feature.clone())) {
                    continue;
                }
                let Some(list) = pkg["features"][&feature].as_array() else {
                    // No such feature: an optional dependency's implicit one, or nothing.
                    todo.push(Turn::Dep(name.clone(), feature));
                    continue;
                };
                for entry in list.iter().filter_map(|e| e.as_str()) {
                    if let Some(key) = entry.strip_prefix("dep:") {
                        todo.push(Turn::Dep(name.clone(), key.to_owned()));
                    } else if let Some((key, feat)) = entry.split_once('/') {
                        let (key, weak) = match key.strip_suffix('?') {
                            Some(key) => (key, true),
                            None => (key, false),
                        };
                        if let Some(d) = dep_of(pkg, key) {
                            if !weak {
                                take(&d, &mut todo);
                            }
                            let target = d["name"].as_str().unwrap_or_default().to_owned();
                            todo.push(Turn::Feature(target, feat.to_owned()));
                        }
                    } else {
                        todo.push(Turn::Feature(name.clone(), entry.to_owned()));
                    }
                }
            }
        }
    }
    on
}

/// The test-only features among `on`.
fn test_only(on: &std::collections::BTreeSet<(String, String)>) -> Vec<String> {
    on.iter()
        .filter(|(_, f)| TEST_ONLY.contains(&f.as_str()))
        .map(|(p, f)| format!("{p}/{f}"))
        .collect()
}

#[test]
fn the_features_a_release_build_resolves_hold_no_test_feature() {
    let meta = metadata();
    let on = resolved(&meta, &SHIPPED);
    for shipped in SHIPPED {
        assert!(
            on.iter().any(|(p, _)| p == shipped),
            "{shipped} resolved nothing: {on:?}"
        );
    }
    // A reader that never reached purlis-core would pass for the wrong reason.
    assert!(on.iter().any(|(p, _)| p == "purlis-core"), "{on:?}");
    let found = test_only(&on);
    assert!(found.is_empty(), "a release build turns on {found:?}");
}

#[test]
fn the_resolver_follows_a_feature_through_a_default_a_dependency_and_the_workspace() {
    // Each indirect way in, as `cargo metadata` would say it, is caught.
    let core = serde_json::json!({
        "name": "purlis-core", "dependencies": [],
        "features": {"fenced": [], "test-support": [], "default": []}
    });
    let via_default = serde_json::json!({"packages": [core.clone(), {
        "name": "purlis-app",
        "dependencies": [{"name": "purlis-core", "kind": null, "optional": false, "uses_default_features": true, "features": [], "rename": null}],
        "features": {"default": ["x"], "x": ["purlis-core/fenced"]}
    }]});
    assert_eq!(
        test_only(&resolved(&via_default, &["purlis-app"])),
        ["purlis-core/fenced"]
    );
    // `[workspace.dependencies]` asking for it reaches the member's dependency entry.
    let via_workspace = serde_json::json!({"packages": [core.clone(), {
        "name": "purlis-cli",
        "dependencies": [{"name": "purlis-core", "kind": null, "optional": false, "uses_default_features": true, "features": ["test-support"], "rename": null}],
        "features": {}
    }]});
    assert_eq!(
        test_only(&resolved(&via_workspace, &["purlis-cli"])),
        ["purlis-core/test-support"]
    );
    // Through another member.
    let via_member = serde_json::json!({"packages": [core.clone(), {
        "name": "persona-statistics",
        "dependencies": [{"name": "purlis-core", "kind": null, "optional": false, "uses_default_features": true, "features": ["fenced"], "rename": null}],
        "features": {}
    }, {
        "name": "purlis-cli",
        "dependencies": [{"name": "persona-statistics", "kind": null, "optional": false, "uses_default_features": true, "features": [], "rename": null}],
        "features": {}
    }]});
    assert_eq!(
        test_only(&resolved(&via_member, &["purlis-cli"])),
        ["purlis-core/fenced"]
    );
    // A dev-dependency is not built by `cargo build`.
    let dev_only = serde_json::json!({"packages": [core.clone(), {
        "name": "purlis-cli",
        "dependencies": [{"name": "purlis-core", "kind": "dev", "optional": false, "uses_default_features": true, "features": ["fenced"], "rename": null}],
        "features": {}
    }]});
    assert!(test_only(&resolved(&dev_only, &["purlis-cli"])).is_empty());
}

#[test]
fn no_tauri_configuration_the_release_reads_names_a_cargo_feature() {
    // Tauri passes `build.features` to Cargo: a feature the command line never shows.
    for file in ["tauri.conf.json", "tauri.release.conf.json"] {
        let text = workflow::read(&repo().join("app/src-tauri").join(file));
        let conf: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        let features = &conf["build"]["features"];
        assert!(
            features.is_null() || features.as_array().is_some_and(Vec::is_empty),
            "{file} names Cargo features for every build: {features}"
        );
    }
}
