// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Only the window's command layer makes the person's press** (M75-07b, #7016; spec §9.4; the
//! final check of #7234, item 1).
//!
//! `answers::Press::of_the_persons_press()` is what lets an answer be added, a never be lifted, the
//! deny state be mended or the device id be confirmed. A chat's request is never the person's
//! press, so no handler that serves a chat (a hook, a door, a dispatch, `purlis serve`, the CLI)
//! may be able to make one.
//!
//! Clippy's `disallowed-methods` says so in `purlis-core` and the app, each site allowing it by
//! name. Clippy reads one `clippy.toml` per crate, and the CLI, the host and the other crates have
//! none, so this reads every Rust file of the workspace and lists the files that may call it.
//! Making a press somewhere else is a new decision, not an edit to [`ALLOWED`].

use std::path::{Path, PathBuf};

/// The function.
const NAME: &str = "of_the_persons_press";

/// Where it may be named, and why.
const ALLOWED: [(&str, &str); 6] = [
    (
        "crates/purlis-core/src/secrets/answers.rs",
        "the definition",
    ),
    (
        "app/src-tauri/src/answers.rs",
        "the window's command layer: a press the person made in the window",
    ),
    (
        "crates/purlis-core/src/secrets/answers_import.rs",
        "the window's own never kept in step, until the press is passed down (#7251, minor)",
    ),
    (
        "crates/purlis-core/src/secrets/answers_tests.rs",
        "a test plays the person's press",
    ),
    (
        "crates/purlis-core/src/policy/deny_state_tests.rs",
        "a test plays the person's press",
    ),
    (
        "crates/purlis-core/src/localproject/tests.rs",
        "a test plays the person's press",
    ),
];

/// The only exception that is core's own, not a test's: tracked, so it cannot grow unseen.
const CORES_OWN: &str = "crates/purlis-core/src/secrets/answers_import.rs";

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target" || name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, into);
        } else if name.ends_with(".rs") {
            into.push(path);
        }
    }
}

/// Every non-comment line of the workspace that names the constructor: file, line, text.
fn mentions() -> Vec<(String, usize, Vec<String>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("app/src-tauri"), &mut files);
    assert!(files.len() > 50, "the walk found {} files", files.len());
    let this = Path::new(file!()).file_name().expect("this file's name");
    let mut found = Vec::new();
    for file in &files {
        if file.file_name() == Some(this) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let at = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_owned();
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if line.contains(NAME) && !line.trim_start().starts_with("//") {
                let before = lines[n.saturating_sub(12)..n]
                    .iter()
                    .map(|line| (*line).to_owned())
                    .collect();
                found.push((at.clone(), n + 1, before));
            }
        }
    }
    found
}

#[test]
fn the_press_is_made_only_by_the_windows_command_layer() {
    purlis_core::unsteered!();
    let found = mentions();
    let outside: Vec<String> = found
        .iter()
        .filter(|(at, _, _)| !ALLOWED.iter().any(|(file, _)| at == file))
        .map(|(at, n, _)| format!("{at}:{n}"))
        .collect();
    assert!(
        outside.is_empty(),
        "the person's press is made outside the window's command layer (a chat's request is never \
         a press; see `Press::of_the_persons_press`):\n{}",
        outside.join("\n")
    );
    // The window's command layer is where it is made: the app's, at least once.
    assert!(
        found
            .iter()
            .any(|(at, _, _)| at == "app/src-tauri/src/answers.rs"),
        "the window's command layer no longer makes a press"
    );
}

/// Clippy is the second lock: each call outside the definition names its allow, so a call that
/// slips past the list above is still refused at the crate's own build.
#[test]
fn each_call_names_its_clippy_allow() {
    purlis_core::unsteered!();
    for (at, n, before) in mentions() {
        if at.ends_with("secrets/answers.rs") {
            continue;
        }
        assert!(
            before
                .iter()
                .any(|line| line.contains("clippy::disallowed_methods")),
            "{at}:{n} makes a press without `#[allow(clippy::disallowed_methods, reason = ..)]`"
        );
    }
}

/// Core's own exception is one file, and it is the one tracked in #7251: it cannot grow.
#[test]
fn core_makes_a_press_in_one_file_only() {
    purlis_core::unsteered!();
    let cores: std::collections::BTreeSet<String> = mentions()
        .into_iter()
        .map(|(at, _, _)| at)
        .filter(|at| {
            at.starts_with("crates/purlis-core/src/")
                && !at.ends_with("answers.rs")
                && !at.ends_with("tests.rs")
        })
        .collect();
    assert_eq!(
        cores.into_iter().collect::<Vec<_>>(),
        vec![CORES_OWN.to_owned()]
    );
}
