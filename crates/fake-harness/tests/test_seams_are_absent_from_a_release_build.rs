// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No build that ships carries a test seam** (R-700, spec "Testing Decisions"). The resolver
//! hook, the test reach entry, the trust store's test root, the recording outbound and the
//! injected clock are purlis-core's, behind its `test-support` feature, which only tests turn on
//! (`release-manifest`'s `no_release_build_turns_on_the_test_fence` holds that). Here:
//!
//! - purlis-core is built as a release build has it, without `test-support`, and the library it
//!   makes is read for each seam's name: a name absent from the crate's metadata is one no
//!   build without the feature can call, and one the binaries that link it cannot carry. A name
//!   every build has must be there, so the read cannot pass by reading nothing;
//! - the files that hold the seams read no environment variable and no file, so nothing a
//!   release build reads could turn one on.
//!
//! A later seam adds its name to [`SEAMS`] (T1a-35's policy fixture path, T1a-69's recording
//! file-system and vault).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Each seam's name, as its item is called.
const SEAMS: [&str; 5] = [
    "TestReach",
    "with_test_reach",
    "trusting_test_root",
    "RecordingOutbound",
    "InjectedClock",
];

/// Names every build of purlis-core has, from the same files.
const ALWAYS: [&str; 3] = ["Destination", "client_config", "Monotonic"];

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

/// purlis-core's library, built with the features a release build has: none.
fn release_library() -> PathBuf {
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
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "purlis_core"
                && message["target"]["kind"][0] == "lib"
        })
        .find_map(|artifact| {
            assert_eq!(
                artifact["features"],
                serde_json::json!([]),
                "the build turned a feature on"
            );
            artifact["filenames"]
                .as_array()?
                .iter()
                .filter_map(|f| f.as_str())
                .find(|f| f.ends_with(".rlib"))
                .map(PathBuf::from)
        })
        .expect("cargo named purlis-core's library")
}

fn holds(haystack: &[u8], name: &str) -> bool {
    haystack
        .windows(name.len())
        .any(|window| window == name.as_bytes())
}

#[test]
fn test_seams_are_absent_from_a_release_build() {
    let library = std::fs::read(release_library()).unwrap();
    for name in ALWAYS {
        assert!(
            holds(&library, name),
            "`{name}` is missing from the release library: this read finds nothing"
        );
    }
    let carried: Vec<_> = SEAMS
        .iter()
        .filter(|name| holds(&library, name))
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
