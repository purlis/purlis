// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The recording file-system and vault seam counts KP4's operations** (R-697, T1a-69).
//!
//! `purlis_core::seam` has three traits, `Files`, `Spawner` and `Credentials`, each with a real
//! implementation and a recording one. The recording ones share one [`Record`], which counts
//! each kind of operation separately and keeps its target (a path, a program, or a store's
//! service and entry id), never a value read or written. They exist only where the
//! `test-support` feature is on, which only `[dev-dependencies]` turn on.

use std::path::Path;
use std::process::Command;

use purlis_core::seam::{
    Credentials, Files, Kind, Record, RecordingCredentials, RecordingFiles, RecordingSpawner,
    Spawner, Target,
};
use purlis_core::secrets::keyring::{FileStore, Store};

/// A recording `Credentials` over the fenced stub in `dir`: the store every test build uses,
/// so no test here reaches the operating system's keychain.
fn stub_credentials(dir: &Path, record: &Record) -> RecordingCredentials {
    let stub = FileStore::at(dir.join(purlis_core::secrets::keyring::STUB_FILE));
    RecordingCredentials::new(Box::new(stub), record.clone())
}

#[test]
fn the_recording_seam_counts_each_kind_of_operation() {
    let dir = tempfile::tempdir().unwrap();
    let policy = dir.path().join("policy.toml");
    let index = dir.path().join("your-keys.json");
    std::fs::write(&policy, "min_version = \"0.5\"\n").unwrap();
    std::fs::write(&index, "{}").unwrap();

    let record = Record::default();
    let files = RecordingFiles::new(record.clone());
    let spawner = RecordingSpawner::new(record.clone());
    let credentials = stub_credentials(dir.path(), &record);
    // Written straight into the stub, so the read below finds a value and the write is not
    // one the seam counts.
    FileStore::at(dir.path().join(purlis_core::secrets::keyring::STUB_FILE))
        .set("purlis/models/0a1b2c3d", "anthropic", "stub-value-never-recorded")
        .unwrap();

    let files: &dyn Files = &files;
    let spawner: &dyn Spawner = &spawner;
    let credentials: &dyn Credentials = &credentials;
    files.stat(&policy).unwrap();
    files.stat(&index).unwrap();
    assert_eq!(files.read(&index).unwrap(), b"{}");
    let status = spawner
        .spawn(Command::new(env!("CARGO")).arg("-V").stdout(std::process::Stdio::null()))
        .unwrap()
        .wait()
        .unwrap();
    assert!(status.success());
    let got = credentials
        .get("purlis/models/0a1b2c3d", "anthropic")
        .unwrap()
        .expect("the stub holds the entry");
    assert_eq!(got.into_inner(), "stub-value-never-recorded");

    assert_eq!(record.count(Kind::Stat), 2);
    assert_eq!(record.count(Kind::Read), 1);
    assert_eq!(record.count(Kind::Spawn), 1);
    assert_eq!(record.count(Kind::CredentialRead), 1);
    assert_eq!(record.count(Kind::CredentialWrite), 0);
    assert_eq!(record.count(Kind::OutsideWrite), 0);

    let ops = record.ops();
    assert_eq!(ops.len(), 5, "{ops:?}");
    assert_eq!(ops[0].target, Target::Path(policy.clone()));
    assert_eq!(ops[1].target, Target::Path(index.clone()));
    assert_eq!(ops[2].target, Target::Path(index.clone()));
    assert_eq!(ops[3].target, Target::Program(env!("CARGO").into()));
    assert_eq!(
        ops[4].target,
        Target::Entry {
            service: "purlis/models/0a1b2c3d".to_owned(),
            account: "anthropic".to_owned(),
        }
    );
    // A record names where it read, never what it read.
    let printed = format!("{ops:?}");
    assert!(!printed.contains("stub-value-never-recorded"), "{printed}");
    assert!(!printed.contains("min_version"), "{printed}");
}

#[test]
fn an_outside_write_bypasses_the_window_command() {
    let dir = tempfile::tempdir().unwrap();
    let record = Record::default();
    let credentials = stub_credentials(dir.path(), &record);
    // The window's write: through the seam, as the command writes your keys.
    credentials
        .set("purlis/models/0a1b2c3d", "anthropic", "stub-value-first")
        .unwrap();
    assert_eq!(record.count(Kind::CredentialWrite), 1);

    credentials
        .write_outside("purlis/models/0a1b2c3d", "anthropic", "stub-value-from-elsewhere")
        .unwrap();

    // The item holds the outside value, and no further write went through the window's path.
    let stub = FileStore::at(dir.path().join(purlis_core::secrets::keyring::STUB_FILE));
    let held = stub
        .get("purlis/models/0a1b2c3d", "anthropic")
        .unwrap()
        .expect("the outside write left the entry");
    assert_eq!(held.into_inner(), "stub-value-from-elsewhere");
    assert_eq!(record.count(Kind::CredentialWrite), 1);
    assert_eq!(record.count(Kind::OutsideWrite), 1);
    let ops = record.ops();
    assert_eq!(ops.last().unwrap().kind, Kind::OutsideWrite);
    assert_eq!(
        ops.last().unwrap().target,
        Target::Entry {
            service: "purlis/models/0a1b2c3d".to_owned(),
            account: "anthropic".to_owned(),
        }
    );
    assert!(!format!("{ops:?}").contains("stub-value-"), "{ops:?}");
}

/// The three recording types, by name: what a release build must never carry.
const RECORDING: [&str; 3] = ["RecordingFiles", "RecordingSpawner", "RecordingCredentials"];

/// The recording implementations live only behind `test-support`, and nothing a release build
/// compiles turns that feature on: read from cargo's own feature resolution of the whole
/// workspace with dev-dependencies left out, which is what `cargo build --release` of any
/// shipped binary resolves. The seam reads no environment variable, file or setting to pick
/// an implementation: the caller passes one as a constructor argument.
#[test]
fn the_recording_seam_is_absent_from_a_release_build() {
    purlis_core::unsteered!();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join("crates/purlis-core/src/seam.rs")).unwrap();

    // Every recording type is defined inside the one gated module, and nowhere outside it.
    let gate = "#[cfg(any(test, feature = \"test-support\"))]\nmod recording {";
    let (open, gated) = source
        .split_once(gate)
        .expect("seam.rs keeps its recording implementations in one `mod recording` behind test-support");
    for name in RECORDING {
        assert!(
            gated.contains(&format!("pub struct {name}")),
            "{name} is defined in the gated `mod recording`"
        );
        assert!(
            !open.contains(&format!("struct {name}")),
            "{name} is defined outside the gated module"
        );
    }
    let reexport = "#[cfg(any(test, feature = \"test-support\"))]\npub use recording::";
    assert!(
        open.contains(reexport) || gated.contains(reexport),
        "the recording types are re-exported only behind test-support"
    );
    for picker in ["std::env", "env::var", "env!(", "option_env!("] {
        assert!(
            !source.contains(picker),
            "seam.rs reads `{picker}`: an implementation is a constructor argument, never \
             something the environment chooses"
        );
    }

    // Nothing but a dev-dependency turns test-support on, anywhere in the workspace.
    let out = purlis_core::forklock::output(
        Command::new(env!("CARGO"))
            .args(["tree", "--locked", "--offline", "--workspace", "--edges", "no-dev"])
            .args(["--invert", "purlis-core", "--depth", "0", "--format", "{p} [{f}]"])
            .current_dir(&root),
    )
    .expect("cargo tree runs");
    assert!(
        out.status.success(),
        "run `cargo fetch --locked` first: this guard reads cargo's resolution of the whole \
         workspace offline.\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    let line = tree
        .lines()
        .find(|line| line.starts_with("purlis-core "))
        .unwrap_or_else(|| panic!("cargo tree names purlis-core:\n{tree}"));
    assert!(
        !line.contains("test-support"),
        "a build without dev-dependencies turns on purlis-core's test-support, so a release \
         build would carry {RECORDING:?}: {line}"
    );
}
