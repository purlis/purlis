// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The macOS app keeps the hardened runtime and carries no entitlement (ruling V90a, #1180).
//!
//! A keyring item is held to the app: the Keychain lets the program that wrote it read it
//! without asking, and asks the person for any other. That holds only while no other code can
//! run as the app. The hardened runtime is what refuses a library injected through the dyld
//! environment, a library signed by someone else and a debugger attaching, and an entitlement
//! (`allow-dyld-environment-variables`, `disable-library-validation`, `get-task-allow`, or any
//! other) is how a build gives one of those back. So the app's build configuration may never
//! turn the runtime off, name an entitlements file, or have one beside it to be named.
//!
//! This is the configuration half. Tauri's bundler signs with the runtime on unless
//! `bundle.macOS.hardenedRuntime` is `false`, and with entitlements only from
//! `bundle.macOS.entitlements`. Every config Tauri may merge into a build is read: the base
//! file, each platform or channel file beside it (`tauri.<name>.conf.json`, which the release
//! and e2e builds name and which Tauri merges for a platform on its own), in JSON, JSON5 or TOML.
//! What the built binary carries (`codesign -d`, the runtime flag and no entitlement) is the
//! other half, for the release job's signature check (#1180).

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The app's Tauri folder.
fn app_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src-tauri")
}

/// Whether `name` is a file Tauri reads as the app's configuration, or merges into it.
fn is_a_tauri_config(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("tauri")
        && (lower.ends_with(".json") || lower.ends_with(".json5") || lower.ends_with(".toml"))
}

/// Every configuration file in the app's Tauri folder.
fn configs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(app_dir())
        .expect("the app's Tauri folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_a_tauri_config)
        })
        .collect();
    found.sort();
    found
}

/// What a configuration says about signing on macOS, or why it must not be read as saying
/// nothing.
fn what_it_turns_off(path: &Path) -> Vec<String> {
    let name = path.display().to_string();
    if !name.ends_with(".json") {
        // A JSON5 or TOML config is one this test cannot read, so it may not exist at all.
        return vec![format!(
            "{name} is a Tauri config this check cannot read; write it as JSON"
        )];
    }
    let text = std::fs::read_to_string(path).expect("a readable config");
    let doc: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut wrong = Vec::new();
    let mac = &doc["bundle"]["macOS"];
    if let Some(runtime) = mac.get("hardenedRuntime").or(mac.get("hardened-runtime"))
        && runtime != &Value::Bool(true)
    {
        wrong.push(format!(
            "{name} sets bundle.macOS.hardenedRuntime to {runtime}: the app must keep it"
        ));
    }
    if let Some(entitlements) = mac.get("entitlements").filter(|value| !value.is_null()) {
        wrong.push(format!(
            "{name} names entitlements {entitlements}: the app carries none"
        ));
    }
    wrong
}

#[test]
fn every_build_config_keeps_the_hardened_runtime_and_names_no_entitlements() {
    let configs = configs();
    let names: Vec<String> = configs
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    // The ones the builds name are read, so a rename cannot leave this reading nothing.
    for named in [
        "tauri.conf.json",
        "tauri.release.conf.json",
        "tauri.e2e.conf.json",
    ] {
        assert!(names.iter().any(|n| n == named), "{named} not in {names:?}");
    }
    let wrong: Vec<String> = configs
        .iter()
        .flat_map(|path| what_it_turns_off(path))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Every file under `dir`, the build's own output and installed packages left out.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a readable folder") {
        let path = entry.expect("an entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !matches!(name, "target" | "node_modules" | "gen") {
                files_under(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

#[test]
fn no_entitlements_file_lies_beside_the_app_to_be_named() {
    let mut files = Vec::new();
    files_under(&app_dir(), &mut files);
    let entitlements: Vec<&PathBuf> = files
        .iter()
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            name.contains("entitlements")
        })
        .collect();
    assert!(entitlements.is_empty(), "{entitlements:#?}");
}

#[test]
fn a_config_that_turns_the_runtime_off_or_names_entitlements_is_refused() {
    let dir = std::env::temp_dir().join(format!("hardened-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        what_it_turns_off(&path)
    };

    assert_eq!(
        write("plain.json", r#"{"bundle": {"active": true}}"#),
        Vec::<String>::new()
    );
    assert_eq!(
        write(
            "on.json",
            r#"{"bundle": {"macOS": {"hardenedRuntime": true}}}"#
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        write(
            "off.json",
            r#"{"bundle": {"macOS": {"hardenedRuntime": false}}}"#
        )
        .len(),
        1
    );
    assert_eq!(
        write(
            "kebab.json",
            r#"{"bundle": {"macOS": {"hardened-runtime": false}}}"#
        )
        .len(),
        1
    );
    assert_eq!(
        write(
            "named.json",
            r#"{"bundle": {"macOS": {"entitlements": "./App.entitlements"}}}"#
        )
        .len(),
        1
    );
    assert_eq!(write("other.toml", "").len(), 1);
    assert!(is_a_tauri_config("tauri.macos.conf.json"));
    assert!(is_a_tauri_config("Tauri.toml"));
    assert!(!is_a_tauri_config("Cargo.toml"));
    std::fs::remove_dir_all(&dir).unwrap();
}
