// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The pane foot at the engine's interface (#5202): in the manifest since the next version, with
//! each part look A draws, drawn on the terminal's fill.

use super::parts;

#[test]
fn the_pane_foot_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let foot = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "pane-foot")
        .expect("the pane foot is in the manifest");
    assert_eq!(foot["since"], 13);
    let names: Vec<&str> = foot["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "root",
            "start",
            "end",
            "separator",
            "gauge",
            "label",
            "value",
            "track",
            "fill"
        ]
    );
    assert_eq!(foot["variants"].as_array().map(Vec::len), Some(0));
}

#[test]
fn the_pane_foot_is_drawn_on_the_terminals_fill() {
    let foot = parts::MANIFEST
        .iter()
        .find(|one| one.name == "pane-foot")
        .expect("the pane foot is in the manifest");
    assert_eq!(foot.grounds, ["terminal.background"]);
}
