// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `artifacts` capability through the real registry** (ADR 0120 §9, #5973): the probe
//! installed as `artifact-probe`, in protocol 8, declares its templates, is fingerprinted and
//! approved, offers them, and a template pinned is copied into the workspace and pinned like any
//! page, under the same refusals and approvals.

#![cfg(unix)]

use std::path::PathBuf;

use purlis_core::artifact::{self, Pinned, approval};
use purlis_core::extension::{self, artifacts};

struct Probe {
    dir: tempfile::TempDir,
}

impl Probe {
    fn assembled() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let plane = dir.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).expect("a workspace");
        std::fs::write(plane.join("charter.toml"), "").expect("a manifest");
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_extension-probe"))
            .arg("assemble-artifacts")
            .arg(dir.path().join("ext"))
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        Self { dir }
    }

    fn installed() -> Self {
        let probe = Self::assembled();
        extension::install(&probe.config(), &extension::BuiltIn::none(), &probe.ext())
            .expect("installed");
        probe
    }

    fn approved() -> Self {
        let probe = Self::installed();
        probe.approve();
        probe
    }

    /// The person's yes in the Extensions dialog, to the bytes there now.
    fn approve(&self) {
        let found = extension::read_at(&self.ext()).expect("read");
        extension::approve(&self.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    fn ext(&self) -> PathBuf {
        self.dir.path().join("ext")
    }

    fn plane(&self) -> PathBuf {
        self.dir.path().join("plane")
    }

    fn alpha(&self) -> PathBuf {
        self.plane().join("workspaces/alpha")
    }

    fn choices(&self) -> extension::project::Choices {
        extension::project::Choices::read_in(&self.plane(), Some("alpha"))
    }

    fn offered(&self) -> Vec<String> {
        artifacts::offered(&self.config(), &extension::BuiltIn::none(), &self.choices())
            .into_iter()
            .map(|it| it.id)
            .collect()
    }

    fn pin(&self, id: &str) -> Result<artifacts::FromExtension, String> {
        artifacts::pin(
            &self.config(),
            &extension::BuiltIn::none(),
            &self.choices(),
            &self.plane(),
            &self.alpha(),
            &artifacts::Wanted {
                id,
                name: None,
                icon: None,
            },
        )
    }

    fn pins(&self) -> Vec<artifact::Pin> {
        artifact::read(&self.alpha()).expect("pins").pins
    }

    fn folder(&self, folder: &str) -> PathBuf {
        self.alpha().join(artifact::DIR).join(folder)
    }
}

#[test]
fn the_probe_declares_its_templates_in_protocol_8_and_the_approval_prompt_names_each() {
    let probe = Probe::assembled();
    let found = extension::read_at(&probe.ext()).expect("read");
    assert_eq!(
        found.manifest.capabilities,
        [extension::Capability::Artifacts]
    );
    assert_eq!(
        u64::from(found.manifest.protocol),
        extension_probe::ARTIFACT_PROTOCOL
    );
    let said = extension::prompt(&found, extension::Standing::New)
        .declares
        .join("\n");
    assert!(said.contains("the capability “artifacts”"), "{said}");
    assert!(
        said.contains(
            "an artifact template, “artifact-probe.board” — “Probe board”, from its folder \
             artifacts/board"
        ),
        "{said}"
    );
    assert!(said.contains("“artifact-probe.leaky”"), "{said}");
}

#[test]
fn a_template_s_bytes_are_in_the_extension_s_fingerprint() {
    let probe = Probe::assembled();
    let before = extension::read_at(&probe.ext()).expect("read").fingerprint;
    std::fs::write(
        probe.ext().join("artifacts/plain/index.html"),
        "<p>else</p>",
    )
    .unwrap();
    let after = extension::read_at(&probe.ext()).expect("read").fingerprint;
    assert_ne!(before, after);
}

#[test]
fn an_approved_extension_offers_its_templates_and_one_not_approved_offers_none() {
    let installed = Probe::installed();
    assert!(installed.offered().is_empty());
    let why = installed.pin("artifact-probe.board").unwrap_err();
    assert!(why.contains("not approved"), "{why}");
    assert!(!installed.alpha().join(artifact::DIR).exists());

    installed.approve();
    assert_eq!(
        installed.offered(),
        [
            "artifact-probe.board",
            "artifact-probe.plain",
            "artifact-probe.leaky"
        ]
    );
}

#[test]
fn a_pinned_template_is_copied_into_the_workspace_without_its_data_and_pinned_like_any_page() {
    let probe = Probe::approved();

    let done = probe.pin("artifact-probe.board").expect("pinned");

    assert_eq!(done.folder, "artifact-probe.board");
    let Pinned::New(pin) = done.pinned else {
        panic!("a new pin: {:?}", done.pinned);
    };
    assert_eq!(pin.name, "Probe board", "named by its title");
    assert_eq!(pin.icon, "rocket", "with the icon it declares");
    assert_eq!(pin.command, None, "an extension's template runs no command");
    assert_eq!(probe.pins(), std::slice::from_ref(&pin));
    let copied = probe.folder("artifact-probe.board");
    assert_eq!(
        std::fs::read_to_string(copied.join("index.html")).unwrap(),
        extension_probe::BOARD_PAGE
    );
    assert!(copied.join("board.js").is_file());
    assert!(
        !copied.join("data.json").exists(),
        "data is each person's own, never the extension's"
    );

    // Its script asks each person on their own machine, as any page's does (ADR 0120 §4): the
    // extension's approval is not the page's.
    let snapshot = artifact::serve::Snapshot::read(&copied, &pin.folder).unwrap();
    assert_eq!(
        approval::standing(
            Some(&probe.config()),
            &probe.alpha(),
            &pin.folder,
            &pin.id,
            &snapshot
        ),
        approval::Standing::New
    );

    let plain = probe.pin("artifact-probe.plain").expect("pinned");
    let Pinned::New(plain) = plain.pinned else {
        panic!("a new pin");
    };
    assert_eq!(plain.icon, artifact::DEFAULT_ICON);
    assert_eq!(probe.pins().len(), 2);
}

#[test]
fn a_template_is_never_copied_over_a_folder_that_is_there() {
    let probe = Probe::approved();
    probe.pin("artifact-probe.plain").expect("pinned");
    std::fs::write(
        probe.folder("artifact-probe.plain").join("index.html"),
        "<p>edited here</p>",
    )
    .unwrap();

    let why = probe.pin("artifact-probe.plain").unwrap_err();

    assert!(why.contains("already a folder"), "{why}");
    assert_eq!(
        std::fs::read_to_string(probe.folder("artifact-probe.plain").join("index.html")).unwrap(),
        "<p>edited here</p>"
    );
    assert_eq!(probe.pins().len(), 1);
}

#[test]
fn a_template_holding_a_secret_is_refused_like_any_pin_and_nothing_is_written() {
    let probe = Probe::approved();

    let why = probe.pin("artifact-probe.leaky").unwrap_err();

    assert!(why.contains("shaped like a secret"), "{why}");
    assert!(!probe.folder("artifact-probe.leaky").exists());
    assert!(probe.pins().is_empty());
}

#[test]
fn an_extension_changed_since_its_approval_or_turned_off_here_pins_nothing() {
    let probe = Probe::approved();
    std::fs::write(probe.ext().join("artifacts/plain/index.html"), "<p>new</p>").unwrap();
    let why = probe.pin("artifact-probe.plain").unwrap_err();
    assert!(why.contains("changed since you approved it"), "{why}");
    assert!(probe.offered().is_empty());

    probe.approve();
    std::fs::write(
        probe.plane().join("charter.toml"),
        "[extensions.artifact-probe]\nenabled = false\n",
    )
    .unwrap();
    let why = probe.pin("artifact-probe.plain").unwrap_err();
    assert!(why.contains("turned off"), "{why}");
    assert!(probe.offered().is_empty());
    assert!(!probe.alpha().join(artifact::DIR).exists());
}

#[test]
fn a_template_folder_that_leads_out_of_the_extension_pins_nothing() {
    let probe = Probe::installed();
    let outside = probe.dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("index.html"), "<p>not the extension's</p>").unwrap();
    let plain = probe.ext().join("artifacts/plain");
    std::fs::remove_dir_all(&plain).unwrap();
    std::os::unix::fs::symlink(&outside, &plain).unwrap();
    probe.approve();

    let why = probe.pin("artifact-probe.plain").unwrap_err();

    assert!(why.contains("not a folder inside its extension"), "{why}");
    assert!(!probe.folder("artifact-probe.plain").exists());
    assert!(probe.pins().is_empty());
}

#[test]
fn a_template_no_extension_here_offers_is_refused_with_a_sentence() {
    let probe = Probe::approved();
    for (id, said) in [
        ("board", "not an extension's template"),
        ("nobody.board", "no extension 'nobody'"),
        (
            "artifact-probe.missing",
            "offers no artifact template 'missing'",
        ),
    ] {
        let why = probe.pin(id).unwrap_err();
        assert!(why.contains(said), "{id}: {why}");
    }
    assert!(!probe.alpha().join(artifact::DIR).exists());
}
