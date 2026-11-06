// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis check report <branch> <file.sarif>`** (#5407, ADR 0115): how a branch's check results
//! arrive. From a terminal it writes the store; a bad file is refused saying what to do.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

const COMMITTER: [&str; 4] = ["-c", "user.name=t", "-c", "user.email=t@example.invalid"];

fn git(dir: &Path, args: &[&str]) {
    let done = support::command("git")
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

/// **Through a Check reporter** (#5410): the check's output, handed to the `check-probe` extension
/// with the status the check exited with, its SARIF recorded as a file's is.
#[cfg(unix)]
mod through_a_check_reporter {
    use super::*;
    use purlis_core::extension;
    use std::sync::OnceLock;

    /// The probe's program, built beside the `purlis` binary, as `extension_commands.rs` builds
    /// its own.
    fn probe_program() -> PathBuf {
        static BUILT: OnceLock<PathBuf> = OnceLock::new();
        BUILT
            .get_or_init(|| {
                let beside = Path::new(support::PURLIS).with_file_name("check-probe");
                if !beside.is_file() {
                    let status = support::command(env!("CARGO"))
                        .args(["build", "--quiet", "-p", "check-probe"])
                        .status()
                        .expect("cargo runs");
                    assert!(status.success(), "the probe did not build: {status}");
                }
                beside
            })
            .clone()
    }

    /// The probe assembled under `root`, installed in its config home, and approved when
    /// `approved`.
    fn the_probe(root: &Path, approved: bool) {
        let ext = root.join("ext");
        let status = support::command(probe_program())
            .arg("assemble")
            .arg(&ext)
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        let found = extension::install(root, &extension::BuiltIn::none(), &ext).expect("installed");
        if approved {
            extension::approve(root, found.id(), &found.path, &found.fingerprint)
                .expect("approved");
        }
    }

    fn output(root: &Path, text: &str) -> PathBuf {
        let file = root.join("check.log");
        std::fs::write(&file, text).unwrap();
        file
    }

    fn record(root: &Path) -> serde_json::Value {
        let text =
            std::fs::read_to_string(root.join("workspaces/alpha/pieces/checks/svc.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn an_approved_reporter_s_sarif_is_recorded_for_the_branch() {
        let (_dir, root) = a_plane();
        the_probe(&root, true);
        let log = output(&root, "a.py:3: name 'x' is not defined\n");
        let out = report(
            &root,
            &[
                "main",
                log.to_str().unwrap(),
                "--reporter",
                "check-probe",
                "--status",
                "1",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", said(&out));
        assert!(
            said(&out).contains("2 results from probe-check recorded for main"),
            "{}",
            said(&out)
        );
        let record = record(&root);
        assert_eq!(record["sets"][0]["tool"], "probe-check");
        assert_eq!(
            record["sets"][0]["diagnostics"][1]["message"],
            "the check exited with status 1"
        );
    }

    #[test]
    fn a_reporter_not_approved_is_refused_and_nothing_is_recorded() {
        let (_dir, root) = a_plane();
        the_probe(&root, false);
        let log = output(&root, "a.py:3: x\n");
        let out = report(
            &root,
            &[
                "main",
                log.to_str().unwrap(),
                "--reporter",
                "check-probe",
                "--status",
                "1",
            ],
        );
        assert_eq!(out.status.code(), Some(1), "{}", said(&out));
        assert!(
            said(&out).contains("you have not approved 'check-probe'"),
            "{}",
            said(&out)
        );
        assert!(
            !root
                .join("workspaces/alpha/pieces/checks/svc.json")
                .exists()
        );
    }

    #[test]
    fn a_reporter_s_invalid_sarif_exits_1_naming_it_and_is_kept_as_its_fault() {
        let (_dir, root) = a_plane();
        the_probe(&root, true);
        let log = output(&root, "check-probe: answer garbage\n");
        let out = report(
            &root,
            &[
                "main",
                log.to_str().unwrap(),
                "--reporter",
                "check-probe",
                "--status",
                "1",
            ],
        );
        assert_eq!(out.status.code(), Some(1), "{}", said(&out));
        assert!(said(&out).contains("'check-probe'"), "{}", said(&out));
        let record = record(&root);
        assert_eq!(record["faults"][0]["extension"], "check-probe");
    }

    #[test]
    fn a_reporter_needs_the_check_s_exit_status() {
        let (_dir, root) = a_plane();
        let log = output(&root, "a.py:3: x\n");
        let out = report(
            &root,
            &["main", log.to_str().unwrap(), "--reporter", "check-probe"],
        );
        assert_eq!(out.status.code(), Some(1), "{}", said(&out));
        assert!(said(&out).contains("--status <STATUS>"), "{}", said(&out));
    }
}
