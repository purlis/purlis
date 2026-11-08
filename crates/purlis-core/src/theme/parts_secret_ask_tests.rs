// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Secret ask's body, at the engine's interface (#5080): in the manifest since the next
//! version, with each part a theme can reach, and the value box's edge a theme's to set.

use super::parts;

#[test]
fn the_secret_ask_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "secret-ask")
        .expect("the secret ask is in the manifest");
    assert_eq!(family["since"], 14);
    let names: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "root",
            "facts",
            "facts-head",
            "who",
            "chat",
            "command",
            "control",
            "value",
            "scope-said",
            "tool-line",
            "foot"
        ]
    );
}

#[test]
fn a_theme_sets_the_edge_a_value_box_takes_on_the_card() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "secret-ask")
        .expect("listed");
    let value = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "value")
        .expect("the value part");
    let properties: Vec<&str> = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one.as_str().unwrap())
        .collect();
    assert_eq!(properties, ["border-colour"]);
}
