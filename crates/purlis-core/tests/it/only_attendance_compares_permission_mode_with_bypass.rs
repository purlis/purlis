// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Only the attendance fact compares a chat's `permission_mode` with the bypass** (T1-22,
//! #6413; R-109 to R-112): A4, dispatch, the briefing and the commit gate read the fact
//! ([`purlis_core::attendance`]), and none keeps a copy of the comparison. This walks the shipped
//! source of the core, the CLI and the app, and fails where the comparison turns up anywhere
//! else.
//!
//! What it looks for, in code a shipped build compiles (no comment, no test module, no test
//! file):
//!
//! - **the mode's own word**, the literal `"bypassPermissions"`: in the attendance module, and
//!   in two places that are not a chat's report: the briefing's recorded stop words (ADR 0046),
//!   and a person's own Claude Code settings file, whose `defaultMode` a grade's start reads;
//! - **the attendance module's own name for it**, `PROMPTS_OFF`, and its comparison,
//!   `says_prompts_off(`: in the attendance module, and in the hook channel, which hands each
//!   report's bit to the fact ([`purlis_core::hookwire::Report::read`]);
//! - **the old readers**, `floorguard::unattended(` and `floorguard::UNATTENDED_MODE`, and the
//!   one-way mark (`dispatchunattended::Mark`): nowhere.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|x| x == "rs") {
            into.push(path);
        }
    }
}

/// Whether `line`, trimmed, opens a module.
fn opens_a_module(line: &str) -> bool {
    ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
        .iter()
        .any(|opening| line.starts_with(opening))
}

/// **The lines of `text` a shipped build compiles**, as far as a reading by lines can tell: no
/// comment line, and no module behind `#[cfg(test)]`, skipped to its closing brace.
fn shipped(text: &str) -> Vec<&str> {
    let mut kept = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let code = line.trim();
        if code.starts_with("//") {
            continue;
        }
        if code == "#[cfg(test)]" {
            // Past any other attribute, to what the gate is on.
            while lines
                .peek()
                .is_some_and(|next| next.trim().starts_with("#["))
            {
                lines.next();
            }
            if let Some(next) = lines.peek().copied().map(str::trim)
                && opens_a_module(next)
            {
                let opening = lines.next().unwrap_or_default();
                if opening.trim_end().ends_with('{') {
                    let mut depth = 1_usize;
                    for inside in lines.by_ref() {
                        depth += inside.matches('{').count();
                        depth = depth.saturating_sub(inside.matches('}').count());
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            continue;
        }
        kept.push(line);
    }
    kept
}

/// Every shipped source file of the core, the CLI and the app, by its path under the
/// workspace, with the lines a shipped build compiles.
fn shipped_sources() -> Vec<(String, String)> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for tree in [
        "crates/purlis-core/src",
        "crates/purlis-cli/src",
        "app/src-tauri/src",
    ] {
        let dir = workspace.join(tree);
        assert!(dir.is_dir(), "{} is not there", dir.display());
        sources(&dir, &mut files);
    }
    assert!(files.len() > 100, "the walk found {} files", files.len());
    files
        .iter()
        .filter(|f| {
            let name = f.to_string_lossy().replace('\\', "/");
            !name.ends_with("/tests.rs")
                && !name.ends_with("_tests.rs")
                && !name.contains("/tests/")
        })
        .map(|f| {
            let at = f
                .strip_prefix(&workspace)
                .unwrap_or(f)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(f).unwrap();
            (at, shipped(&text).join("\n"))
        })
        .collect()
}

/// The files whose shipped lines hold any of `needles`, sorted.
fn naming(sources: &[(String, String)], needles: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = sources
        .iter()
        .filter(|(_, text)| needles.iter().any(|needle| text.contains(needle)))
        .map(|(at, _)| at.clone())
        .collect();
    found.sort();
    found
}

/// **The guard**: the comparison lives in the attendance module alone.
///
/// Red before T1-22: `floorguard.rs` held the old comparison and the old constant, and
/// `dispatchunattended.rs`, `hookwire.rs`, `toolgate.rs`, `dispatchmode.rs`, the app's
/// `handoff.rs` and the CLI's `hooks.rs` read them, each a copy of its own.
#[test]
fn only_attendance_compares_permission_mode_with_bypass() {
    purlis_core::unsteered!();
    let sources = shipped_sources();

    assert_eq!(
        naming(&sources, &["\"bypassPermissions\""]),
        [
            // The one comparison.
            "crates/purlis-core/src/attendance.rs",
            // The stop words a chat is told, recorded from the Python charter (ADR 0046): words,
            // never a reading of the chat's mode.
            "crates/purlis-core/src/briefing.rs",
            // A person's own Claude Code settings file, its `permissions.defaultMode`: what a
            // grade's start refuses, never a chat's report.
            "crates/purlis-core/src/harness/claude.rs",
        ],
        "a module outside the attendance fact spells the bypass mode"
    );
    assert_eq!(
        naming(&sources, &["PROMPTS_OFF"]),
        ["crates/purlis-core/src/attendance.rs"],
        "a module outside the attendance fact names its bypass mode"
    );
    assert_eq!(
        naming(&sources, &["says_prompts_off("]),
        [
            "crates/purlis-core/src/attendance.rs",
            // A report's bit, handed to the fact (`Fact::heard`) and to its ask
            // (`Fact::hearing`): read there, never decided on.
            "crates/purlis-core/src/hookwire.rs",
        ],
        "a module outside the attendance fact asks whether a line is in the bypass"
    );
    assert_eq!(
        naming(
            &sources,
            &[
                "floorguard::unattended(",
                "floorguard::UNATTENDED_MODE",
                "dispatchunattended::Mark",
            ]
        ),
        Vec::<String>::new(),
        "an old reader of the bypass mode is back"
    );
}
