// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No build that ships carries a test seam** (R-700, spec "Testing Decisions"). The resolver
//! hook, the test reach entry, the trust store's test root, the recording outbound and the
//! injected clock are purlis-core's, behind its `test-support` feature, which only tests turn on
//! (`release-manifest`'s `no_release_build_turns_on_the_test_fence` holds that). Here:
//!
//! - purlis-core is built as a release build has it, without `test-support`, and a use of each
//!   seam is compiled against that library: each must fail to compile, while a use of what every
//!   build has compiles, so the check cannot pass by compiling nothing. (The library's bytes are
//!   not read for the names: rustc records the names of items a `cfg` left out, to say which
//!   feature they wait for, so a name there proves nothing.)
//! - the files that hold the seams read no environment variable and no file, so nothing a
//!   release build reads could turn one on.
//!
//! A later seam adds its use to [`SEAMS`], as T1a-69's recording file-system and vault have;
//! T1a-35's policy fixture path is next.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A use of each seam, by its name.
const SEAMS: [(&str, &str); 8] = [
    ("TestReach", "use purlis_core::upstream::TestReach;"),
    (
        "with_test_reach",
        "pub fn seam() { let _ = purlis_core::upstream::Direct::with_test_reach; }",
    ),
    (
        "trusting_test_root",
        "use purlis_core::upstream::trust::trusting_test_root;",
    ),
    (
        "RecordingOutbound",
        "use purlis_core::upstream::RecordingOutbound;",
    ),
    ("InjectedClock", "use purlis_core::clock::InjectedClock;"),
    // T1a-69's recording file-system and vault seam (R-697, `seam.rs`). Its file is not in
    // `SEAM_FILES`: its real `Files` reads files, which is what it is for.
    ("RecordingFiles", "use purlis_core::seam::RecordingFiles;"),
    (
        "RecordingSpawner",
        "use purlis_core::seam::RecordingSpawner;",
    ),
    (
        "RecordingCredentials",
        "use purlis_core::seam::RecordingCredentials;",
    ),
];

/// A use of what every build has, from the same files.
const ALWAYS: &str = "use purlis_core::clock::{Clock, Deadline, Monotonic};\n\
                      use purlis_core::upstream::{Destination, Direct, Outbound, trust};\n\
                      pub fn always() { let _ = (trust::client_config, Direct::new); }";

/// The files that hold the seams, under purlis-core's `src`.
const SEAM_FILES: [&str; 4] = [
    "clock.rs",
    "upstream.rs",
    "upstream/seams.rs",
    "upstream/trust.rs",
];

/// What in a seam's file would let something outside the build turn it on.
const READS_OUTSIDE: [&str; 7] = [
    "std::env",
    "env::var",
    "env!(",
    "std::fs",
    "File::open",
    "read_to_string",
    "dirs::",
];

fn core() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../purlis-core")
}

/// purlis-core's library as a release build has it, and what it links against.
struct Release {
    /// purlis-core's `.rlib`, built with the features a release build has: none.
    library: PathBuf,
    /// Every folder cargo put a crate of that build in, for `-L dependency=`. Cargo's layout of
    /// its build folder is its own (1.100 moved each crate into a folder of its own), so the
    /// folders are taken from what cargo says it built, never guessed.
    dependencies: BTreeSet<PathBuf>,
}

fn release_build() -> Release {
    let out = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "--lib", "-p", "purlis-core"])
        .args(["--message-format", "json"])
        .current_dir(core())
        .output()
        .expect("cargo runs");
    assert!(
        out.status.success(),
        "purlis-core did not build without test-support:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let artifacts: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .collect();
    let filenames = |artifact: &serde_json::Value| -> Vec<PathBuf> {
        artifact["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str())
            .map(PathBuf::from)
            .collect()
    };
    let library = artifacts
        .iter()
        .filter(|artifact| {
            artifact["target"]["name"] == "purlis_core" && artifact["target"]["kind"][0] == "lib"
        })
        .find_map(|artifact| {
            assert_eq!(
                artifact["features"],
                serde_json::json!([]),
                "the build turned a feature on"
            );
            filenames(artifact)
                .into_iter()
                .find(|f| f.extension().is_some_and(|e| e == "rlib"))
        })
        .expect("cargo named purlis-core's library");
    // Every artifact's folder, a proc-macro's library (`.dylib`, `.so`, `.dll`) included.
    let dependencies = artifacts
        .iter()
        .flat_map(filenames)
        .filter_map(|f| f.parent().map(Path::to_path_buf))
        .collect();
    Release {
        library,
        dependencies,
    }
}

/// Whether `source` compiles against purlis-core's release library and the crates it links.
fn compiles(release: &Release, name: &str, source: &str) -> (bool, String) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(format!("{name}.rs"));
    std::fs::write(&file, format!("#![allow(unused)]\n{source}\n")).unwrap();
    // The compiler cargo was told to use, when it was told one.
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut command = Command::new(rustc);
    command
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("--crate-name")
        .arg(format!("seam_{}", name.to_lowercase()))
        .arg("--out-dir")
        .arg(dir.path());
    for folder in &release.dependencies {
        command
            .arg("-L")
            .arg(format!("dependency={}", folder.display()));
    }
    let out = command
        .arg("--extern")
        .arg(format!("purlis_core={}", release.library.display()))
        .arg(&file)
        .output()
        .expect("rustc runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn test_seams_are_absent_from_a_release_build() {
    let release = release_build();
    let (always, why) = compiles(&release, "always", ALWAYS);
    assert!(
        always,
        "what every build has did not compile against {}, so this check proves nothing:\n{why}",
        release.library.display()
    );
    let carried: Vec<_> = SEAMS
        .iter()
        .filter(|(name, source)| {
            let (compiled, why) = compiles(&release, name, source);
            // A use refused for another reason (a typo here) would prove nothing.
            assert!(
                compiled || why.contains(name),
                "{name} was refused for another reason:\n{why}"
            );
            compiled
        })
        .map(|(name, _)| name)
        .collect();
    assert!(
        carried.is_empty(),
        "a build without test-support carries these test seams: {carried:?}"
    );
    for file in SEAM_FILES {
        let text = std::fs::read_to_string(core().join("src").join(file)).unwrap();
        for read in READS_OUTSIDE {
            assert!(
                !text.contains(read),
                "purlis-core/src/{file} uses `{read}`: a seam may be turned on only by a \
                 constructor argument or `test-support`"
            );
        }
    }
}
