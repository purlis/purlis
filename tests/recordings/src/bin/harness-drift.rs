// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The nightly re-record's diff** (T1.1-03 item 5, [`purlis_recordings::drift`]):
//!
//! ```text
//! harness-drift <checked-in recording.json> <fresh recording.json> <which> [--dry-run]
//! ```
//!
//! `which` is `pinned` or `latest`. Both recordings are masked again before they are compared.
//! Drift opens a `harness-drift` issue in `$GITHUB_REPOSITORY` with `gh` (or, with `--dry-run`,
//! prints the `gh` commands it would run) and exits 0. A difference in a refused call is printed
//! and exits 1, so the nightly entry, listed `kind: escape`, files it as a public
//! `area:security`, `priority:p0` issue (Q301). A usage or read error exits 2.

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
    let words: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [checked_in, fresh, which] = words.as_slice() else {
        eprintln!(
            "usage: harness-drift <checked-in recording.json> <fresh recording.json> pinned|latest [--dry-run]"
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
    if let Some(issue) = drift::issue(&harness, &version, which, &diff, &run) {
        // One open issue a harness, version and re-record: a later night comments on it.
        let open = if dry_run {
            None
        } else {
            open_issue(&repo, &issue.title)
        };
        let commands = match open {
            Some(number) => vec![vec![
                "issue".to_owned(),
                "comment".to_owned(),
                number,
                "--repo".to_owned(),
                repo.clone(),
                "--body".to_owned(),
                format!("Drifted again: {run}"),
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
    if !diff.escape.is_empty() {
        println!(
            "{harness} {version} ({which}): a refused call's recording changed, which may be an escape (Q301):"
        );
        for difference in &diff.escape {
            println!("  {difference}");
        }
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// The number of the open drift issue titled `title`, if there is one.
fn open_issue(repo: &str, title: &str) -> Option<String> {
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
        .ok()?;
    let found: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    found
        .as_array()?
        .iter()
        .find(|issue| issue["title"] == title)
        .map(|issue| issue["number"].to_string())
}

fn shell_words(words: &[String]) -> String {
    words
        .iter()
        .map(|w| purlis_core::handoff::quote(w))
        .collect::<Vec<_>>()
        .join(" ")
}
