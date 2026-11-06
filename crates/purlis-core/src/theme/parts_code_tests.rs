// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The code family at the engine's interface (#4481): in the manifest since the next version.

use super::parts;

#[test]
fn the_code_box_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let code = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "code")
        .expect("the code family is in the manifest");
    assert_eq!(code["since"], 13);
    let parts_named: Vec<_> = code["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(parts_named, ["root", "label", "body"]);
}
