// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis check report <branch> <file.sarif>`** (#5407, ADR 0114): how a branch's check results
//! arrive. From a terminal it writes the store; a bad file is refused saying what to do.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const COMMITTER: [&str; 4] = ["-c", "user.name=t", "-c", "user.email=t@example.invalid"];

fn git(dir: &Path, args: &[&str]) {
    let done = Command::new("git")
        .args(COMMITTER)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(done.status.success(), "git {args:?}: {done:?}");
}

/// A plane whose workspace `alpha` has the clone `svc`, on `main`, with one commit.
fn a_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    let clone = root.join("workspaces/alpha/svc");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["commit", "-q", "--allow-empty", "-m", "first"]);
    (dir, root)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sarif")
        .join(name)
}

fn report(root: &Path, args: &[&str]) -> Output {
    support::purlis()
        .args(["check", "report"])
        .args(args)
        .current_dir(root.join("workspaces/alpha/svc"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("PURLIS_CONFIG_HOME", root)
        .env("TMPDIR", std::env::temp_dir())
        .env("NO_COLOR", "1")
        .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
        .stdin(Stdio::null())
        .output()
        .expect("purlis runs")
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn a_tools_real_output_is_recorded_for_the_branch_at_its_head() {
    let (_dir, root) = a_plane();
    let file = fixture("codeql-2.27.4-actions.sarif");
    let out = report(&root, &["main", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{}", said(&out));
    assert!(
        said(&out).contains("3 results from CodeQL recorded for main"),
        "{}",
        said(&out)
    );
    let record =
        std::fs::read_to_string(root.join("workspaces/alpha/pieces/checks/svc.json")).unwrap();
    let record: serde_json::Value = serde_json::from_str(&record).unwrap();
    assert_eq!(record["sets"][0]["tool"], "CodeQL");
    assert_eq!(record["sets"][0]["branch"], "main");
    assert_eq!(record["sets"][0]["head"].as_str().map(str::len), Some(40));
}

#[test]
fn invalid_sarif_exits_1_saying_what_is_wrong_and_what_to_do_and_records_nothing() {
    let (_dir, root) = a_plane();
    let file = fixture("malformed/wrong-version.sarif");
    let out = report(&root, &["main", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    let told = said(&out);
    assert!(
        told.contains("was not recorded") && told.contains("2.0.0"),
        "{told}"
    );
    assert!(told.contains("Have the tool write SARIF 2.1.0"), "{told}");
    assert!(
        !root
            .join("workspaces/alpha/pieces/checks/svc.json")
            .exists()
    );
}

#[test]
fn a_branch_no_folder_has_exits_1_naming_it() {
    let (_dir, root) = a_plane();
    let file = fixture("scorecard-5.6.0.sarif");
    let out = report(&root, &["nowhere", file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("nowhere") && said(&out).contains("purlis worktree list"),
        "{}",
        said(&out)
    );
}
