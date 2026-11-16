// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The nightly re-record's diff** (T1.1-03 item 5, [`purlis_recordings::drift`]):
//!
//! ```text
//! harness-drift <checked-in recording.json> <fresh recording.json> <which> --mode drift|escape
//!     [--dry-run] [--open <issue number>]
//! ```
//!
//! `which` is `pinned` or `latest`. Both recordings are masked again before they are compared.
//! The two nightly entries of a re-record each run one mode, since the scheduler files a red
//! `kind: escape` entry as a p0 security issue and any other red as an ordinary one:
//!
//! - `--mode drift` (an ordinary entry): drift opens a `harness-drift` issue in
//!   `$GITHUB_REPOSITORY` with `gh`, or comments on the one open with what differs now, and
//!   exits 0. A refused call that changed is only said: the escape entry fails on it.
//! - `--mode escape` (the `kind: escape` entry): a refused call that changed is printed and exits
//!   1, the only way this mode is red. It opens nothing.
//!
//! With `--dry-run` it prints the `gh` commands it would run (`--open` names the open drift
//! issue it would comment on). A usage, read or `gh` error exits 2.

use std::process::{Command, ExitCode};

use purlis_recordings::drift;
use purlis_recordings::mask::mask;

fn read(path: &str) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let after = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let mode = after("--mode");
    let open_given = after("--open");
    let valued: Vec<usize> = ["--mode", "--open"]
        .iter()
        .filter_map(|flag| args.iter().position(|a| a == flag))
        .map(|at| at + 1)
        .collect();
    let words: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(at, a)| !a.starts_with("--") && !valued.contains(at))
        .map(|(_, a)| a)
        .collect();
    let ([checked_in, fresh, which], Some(mode @ ("drift" | "escape"))) =
        (words.as_slice(), mode.as_deref())
    else {
        eprintln!(
            "usage: harness-drift <checked-in recording.json> <fresh recording.json> pinned|latest --mode drift|escape [--dry-run] [--open <issue>]"
        );
        return ExitCode::from(2);
    };
    let (old, new) = match (read(checked_in), read(fresh)) {
        (Ok(old), Ok(new)) => (mask(&old), mask(&new)),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("harness-drift: {e}");
            return ExitCode::from(2);
        }
    };
    let harness = new["harness"].as_str().unwrap_or("a harness").to_owned();
    let version = new["version"]
        .as_str()
        .unwrap_or("an unknown version")
        .to_owned();
    let diff = drift::compare(&old, &new);
    let repo = std::env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| "purlis/purlis".to_owned());
    let run = match (
        std::env::var("GITHUB_SERVER_URL"),
        std::env::var("GITHUB_RUN_ID"),
    ) {
        (Ok(server), Ok(id)) => format!("{server}/{repo}/actions/runs/{id}"),
        _ => "(not run in CI)".to_owned(),
    };
    if diff.drift.is_empty() && diff.escape.is_empty() {
        println!("{harness} {version} ({which}): the same recording, masked");
        return ExitCode::SUCCESS;
    }
    if mode == "escape" {
        if diff.escape.is_empty() {
            println!(
                "{harness} {version} ({which}): every refused call recorded as before; the drift entry reports the rest"
            );
            return ExitCode::SUCCESS;
        }
        println!(
            "{harness} {version} ({which}): a refused call's recording changed, which may be an escape (Q301):"
        );
        for difference in &diff.escape {
            println!("  {difference}");
        }
        return ExitCode::from(1);
    }
    if !diff.escape.is_empty() {
        println!(
            "{harness} {version} ({which}): a refused call's recording changed; the escape entry fails on it"
        );
    }
    if let Some(issue) = drift::issue(&harness, &version, which, &diff, &run) {
        // One open issue a harness, version and re-record: a later night comments on it.
        let open = if dry_run {
            open_given
        } else {
            match open_issue(&repo, &issue.title) {
                Ok(open) => open,
                Err(e) => {
                    eprintln!("harness-drift: gh issue list: {e}");
                    return ExitCode::from(2);
                }
            }
        };
        let commands = match open {
            Some(number) => vec![vec![
                "issue".to_owned(),
                "comment".to_owned(),
                number,
                "--repo".to_owned(),
                repo.clone(),
                "--body".to_owned(),
                drift::comment(&diff, &run),
            ]],
            None => drift::gh_commands(&issue, &repo),
        };
        for command in commands {
            if dry_run {
                println!("would run: gh {}", shell_words(&command));
                continue;
            }
            match Command::new("gh").args(&command).status() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    eprintln!(
                        "harness-drift: gh {} ended {status}",
                        command[..2].join(" ")
                    );
                    return ExitCode::from(2);
                }
                Err(e) => {
                    eprintln!("harness-drift: gh: {e}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    ExitCode::SUCCESS
}

/// The number of the open drift issue titled `title`, if there is one; an error when `gh`
/// could not say.
fn open_issue(repo: &str, title: &str) -> Result<Option<String>, String> {
    let output = Command::new("gh")
        .args([
            "issue", "list", "--repo", repo, "--state", "open", "--label",
        ])
        .arg(drift::DRIFT_LABEL)
        .args([
            "--search",
            &format!("\"{title}\" in:title"),
            "--json",
            "number,title",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let found: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(found.as_array().and_then(|issues| {
        issues
            .iter()
            .find(|issue| issue["title"] == title)
            .map(|issue| issue["number"].to_string())
    }))
}

fn shell_words(words: &[String]) -> String {
    words
        .iter()
        .map(|w| purlis_core::handoff::quote(w))
        .collect::<Vec<_>>()
        .join(" ")
}
