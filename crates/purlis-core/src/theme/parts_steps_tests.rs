// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The steps family at the engine's interface (#4480): in the manifest since the next version,
//! with each part and each state a step draws.

use super::parts;

#[test]
fn the_steps_are_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let steps = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "steps")
        .expect("the steps family is in the manifest");
    assert_eq!(steps["since"], parts::NEXT);
    let names = |key: &str| -> Vec<String> {
        steps[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        names("parts"),
        ["root", "count", "press", "mark", "name", "line"]
    );
    assert_eq!(
        names("variants"),
        ["done", "current", "attention", "skipped", "waiting", "later"]
    );
}
