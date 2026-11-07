// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Check reporter, end to end** (ADR 0117, #5410): the probe assembled as an operator would
//! install it, approved with the dialog's two clicks, handed a check's output and exit status
//! through purlis's real executor, and its SARIF recorded through the same path as `purlis check
//! report` — and refused when it was never approved, and kept as a fault naming it when what it
//! answers is not SARIF.

#![cfg(unix)]

use std::path::{Path, PathBuf};

use purlis_core::checks::{converted, of_branch, read_sets, report_converted};
use purlis_core::executor::Executor;
use purlis_core::extension;
use purlis_core::files::Branch;

/// How long the probe is given: not the real five seconds, because it is copied in fresh for each
/// test and macOS assesses a program the first time it runs (extension-probe's `PATIENT`).
const PATIENT: std::time::Duration = std::time::Duration::from_secs(30);

const WS: &str = "alpha";
const REPO: &str = "svc";

struct Probe {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn git(at: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

impl Probe {
    /// A plane with one clone, and the probe assembled beside it, not yet installed.
    fn assembled() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let root = std::fs::canonicalize(dir.path()).expect("its real path");
        let plane = root.join("plane");
        let clone = plane.join("workspaces").join(WS).join(REPO);
        std::fs::create_dir_all(&clone).expect("the clone");
        std::fs::write(plane.join("charter.toml"), "schema = 1\n").expect("a manifest");
        git(&clone, &["init", "-q", "-b", "main", "."]);
        std::fs::write(clone.join("a.py"), "one\n").expect("a file");
        git(&clone, &["add", "-A"]);
        git(&clone, &["commit", "-q", "-m", "one"]);
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_check-probe"))
            .arg("assemble")
            .arg(root.join("ext"))
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        Self { _dir: dir, root }
    }

    /// Assembled, installed and approved: the two clicks.
    fn approved() -> Self {
        let probe = Self::assembled();
        probe.install_and_approve();
        probe
    }

    fn install_and_approve(&self) {
        let found = extension::install(&self.config(), &extension::BuiltIn::none(), &self.ext())
            .expect("installed");
        extension::approve(&self.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
    }

    fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    fn ext(&self) -> PathBuf {
        self.root.join("ext")
    }

    fn plane(&self) -> PathBuf {
        self.root.join("plane")
    }

    fn branch(&self) -> Branch<'static> {
        Branch::repo(WS, REPO)
    }

    /// The probe handed `output` and `status`, and its answer recorded, as `purlis check report
    /// --reporter check-probe` does: `Err` for a refusal before it ran.
    fn report(&self, output: &str, status: i32) -> Result<Result<(), String>, String> {
        let plane = self.plane();
        let answered = converted(
            &Executor::default().with_deadline(PATIENT),
            &self.config(),
            &extension::project::Choices::read(&plane),
            "check-probe",
            output.as_bytes(),
            status,
        )?;
        Ok(report_converted(
            &plane,
            self.branch(),
            "check-probe",
            answered,
            chrono::Utc::now(),
        )
        .map(|_| ()))
    }
}

#[test]
fn the_probe_s_manifest_asks_for_the_check_reporter_in_protocol_6() {
    let probe = Probe::assembled();
    let found = extension::read_at(&probe.ext()).expect("it reads");
    assert_eq!(found.manifest.protocol, 6);
    assert!(found.manifest.reports_checks());
    let asked = extension::prompt(&found, extension::Standing::New).declares;
    assert!(
        asked
            .iter()
            .any(|line| line.contains("the capability “check-reporter”")),
        "{asked:?}"
    );
}

#[test]
fn approved_it_is_handed_the_output_and_status_and_its_sarif_is_recorded() {
    let probe = Probe::approved();
    probe
        .report("a.py:1: name 'x' is not defined\n", 1)
        .expect("not refused")
        .expect("recorded");
    let sets = read_sets(&probe.plane(), WS, REPO, None).expect("read");
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].tool, check_probe::TOOL);
    let said: Vec<(Option<&str>, Option<u32>, &str)> = sets[0]
        .diagnostics
        .iter()
        .map(|it| (it.path.as_deref(), it.line, it.message.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (Some("a.py"), Some(1), "name 'x' is not defined"),
            (None, None, "the check exited with status 1"),
        ]
    );
}

#[test]
fn never_approved_it_is_refused_and_nothing_is_recorded() {
    let probe = Probe::assembled();
    extension::install(&probe.config(), &extension::BuiltIn::none(), &probe.ext())
        .expect("installed");
    let why = probe
        .report("a.py:1: x\n", 1)
        .expect_err("refused before it ran");
    assert!(why.contains("you have not approved 'check-probe'"), "{why}");
    assert!(
        read_sets(&probe.plane(), WS, REPO, None)
            .expect("read")
            .is_empty()
    );
    let shown = of_branch(&probe.plane(), probe.branch(), "a.py", &[], None);
    assert!(shown.faults.is_empty(), "{:?}", shown.faults);
}

#[test]
fn changed_since_its_yes_it_is_refused() {
    let probe = Probe::approved();
    std::fs::write(probe.ext().join("extra.txt"), "new\n").expect("a change");
    let why = probe.report("a.py:1: x\n", 1).expect_err("refused");
    assert!(why.contains("check-probe"), "{why}");
}

#[test]
fn an_extension_that_does_not_ask_for_the_capability_is_never_handed_a_check() {
    let probe = Probe::assembled();
    let at = probe.ext().join(extension::MANIFEST);
    std::fs::write(
        &at,
        r#"{"version":6,"id":"check-probe","name":"Check probe","contributes":{"runs":"bin/check-probe"}}"#,
    )
    .expect("a manifest without the capability");
    probe.install_and_approve();
    let why = probe.report("a.py:1: x\n", 1).expect_err("refused");
    assert!(
        why.contains("does not ask for the capability \"check-reporter\""),
        "{why}"
    );
}

#[test]
fn invalid_sarif_is_a_fault_naming_it_and_the_results_before_stay() {
    let probe = Probe::approved();
    probe
        .report("a.py:1: kept\n", 1)
        .expect("not refused")
        .expect("recorded");
    let why = probe
        .report(&format!("{}\n", check_probe::ANSWER_GARBAGE), 1)
        .expect("not refused")
        .expect_err("not SARIF");
    assert!(why.contains("'check-probe'"), "{why}");
    assert!(why.contains("cannot read as SARIF 2.1.0"), "{why}");
    let shown = of_branch(&probe.plane(), probe.branch(), "a.py", &[], None);
    assert_eq!(shown.outside[0].diagnostic.message, "kept");
    assert_eq!(shown.faults.len(), 1);
    assert_eq!(shown.faults[0].extension, "check-probe");
}

#[test]
fn a_reporter_that_exits_with_a_failure_is_a_fault_saying_what_it_printed() {
    let probe = Probe::approved();
    let why = probe
        .report(&format!("{}3\n", check_probe::EXIT_WITH), 0)
        .expect("not refused")
        .expect_err("it failed");
    assert!(why.contains("'check-probe' exited with status 3"), "{why}");
    assert!(why.contains("told to exit with 3"), "{why}");
    let shown = of_branch(&probe.plane(), probe.branch(), "a.py", &[], None);
    assert_eq!(shown.faults[0].extension, "check-probe");
}
