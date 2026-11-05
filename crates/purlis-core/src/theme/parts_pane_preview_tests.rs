// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a drag draws over the panes, at the engine's interface (#5291): in the manifest since the
//! next version, with each part a theme can reach.

use super::parts;

#[test]
fn the_pane_preview_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "pane-preview")
        .expect("the pane preview is in the manifest");
    assert_eq!(family["since"], parts::NEXT);
    let names: Vec<&str> = family["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["arriving", "name", "size", "refused", "held", "words"]
    );
}
