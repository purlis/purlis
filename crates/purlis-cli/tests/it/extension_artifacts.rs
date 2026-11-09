// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact templates` and `purlis artifact pin --template` (ADR 0120 §9, #5973), asked
//! of the binary with the test-only `extension-probe` installed as `artifact-probe`. What a pin
//! of a template copies and refuses is proven in `extension-probe`'s own tests; what is here is
//! that the command line reaches it and says what it did.

#![cfg(unix)]

use crate::support;

use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::OnceLock;

use purlis_core::extension;

/// The probe's program, built beside this binary, as `extension_commands.rs` finds it.
fn probe_program() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let beside = Path::new(support::PURLIS).with_file_name("extension-probe");
            if !beside.is_file() {
                let status = support::command(env!("CARGO"))
                    .args(["build", "--quiet", "-p", "extension-probe"])
                    .status()
                    .expect("cargo runs");
                assert!(status.success(), "the probe did not build: {status}");
            }
            beside
        })
        .clone()
}

/// A plane with workspace alpha, and the artifact probe assembled, installed and approved.
struct Setup {
    _tmp: tempfile::TempDir,
    base: PathBuf,
}

impl Setup {
    fn approved() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(tmp.path()).unwrap();
        let setup = Self { _tmp: tmp, base };
        std::fs::create_dir_all(setup.plane().join("workspaces/alpha")).unwrap();
        std::fs::write(setup.plane().join("charter.toml"), "schema = 1\n").unwrap();
        let status = support::command(probe_program())
            .arg("assemble-artifacts")
            .arg(setup.base.join("ext"))
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        let found = extension::install(
            &setup.config(),
            &extension::BuiltIn::none(),
            &setup.base.join("ext"),
        )
        .expect("installed");
        extension::approve(&setup.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
        setup
    }

    fn plane(&self) -> PathBuf {
        self.base.join("plane")
    }

    fn config(&self) -> PathBuf {
        self.base.join("config")
    }

    fn purlis(&self, args: &[&str]) -> Output {
        support::purlis()
            .args(args)
            .current_dir(self.plane())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.base)
            .env("TMPDIR", std::env::temp_dir())
            .env("CHARTER_ROOT", self.plane())
            .env("CHARTER_WORKSPACE", "alpha")
            .env("CHARTER_CONFIG_HOME", self.config())
            .output()
            .expect("purlis runs")
    }
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn templates_lists_what_the_approved_extension_offers_and_pin_template_pins_one() {
    let setup = Setup::approved();

    let listed = setup.purlis(&["artifact", "templates"]);
    assert!(listed.status.success(), "{}", said(&listed));
    let lines = String::from_utf8_lossy(&listed.stdout).into_owned();
    assert!(
        lines.contains("artifact-probe.board  Probe board  rocket  Artifact probe"),
        "{lines}"
    );
    assert!(
        lines.contains("artifact-probe.plain  Probe notes  chart"),
        "{lines}"
    );

    let pinned = setup.purlis(&[
        "artifact",
        "pin",
        "--template",
        "artifact-probe.plain",
        "--name",
        "Notes",
    ]);
    assert!(pinned.status.success(), "{}", said(&pinned));
    assert!(
        said(&pinned).contains("Copied artifact-probe.plain into artifacts/artifact-probe.plain"),
        "{}",
        said(&pinned)
    );
    let list = setup.purlis(&["artifact", "list"]);
    assert!(
        said(&list).contains("artifact-probe.plain  chart  Notes"),
        "{}",
        said(&list)
    );
}

#[test]
fn pin_template_says_why_it_refuses_and_exits_1() {
    let setup = Setup::approved();

    let leaky = setup.purlis(&["artifact", "pin", "--template", "artifact-probe.leaky"]);
    assert_eq!(leaky.status.code(), Some(1), "{}", said(&leaky));
    assert!(
        said(&leaky).contains("shaped like a secret"),
        "{}",
        said(&leaky)
    );

    let both = setup.purlis(&[
        "artifact",
        "pin",
        "plain",
        "--template",
        "artifact-probe.plain",
    ]);
    assert!(!both.status.success(), "a folder or a template, never both");

    let with_command = setup.purlis(&[
        "artifact",
        "pin",
        "--template",
        "artifact-probe.plain",
        "--command",
        "make",
    ]);
    assert!(
        !with_command.status.success(),
        "an extension's template runs no command"
    );
    assert!(
        !setup
            .plane()
            .join("workspaces/alpha/artifacts/pins.toml")
            .exists()
    );
}
