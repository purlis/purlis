// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The checks family at the engine's interface (#4481): in the manifest since the next version,
//! with each part and each verdict a line draws.

use super::parts;

#[test]
fn the_checks_are_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let checks = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "checks")
        .expect("the checks family is in the manifest");
    assert_eq!(checks["since"], 13);
    let names = |key: &str| -> Vec<String> {
        checks[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("parts"), ["root", "mark", "title", "more", "trail"]);
    assert_eq!(
        names("variants"),
        ["ok", "missing", "running", "todo", "warn", "skip"]
    );
}
