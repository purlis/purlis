// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The button family's quiet link (#5214): a card's way on beside its one answer, Go to chat
//! in an Inbox card's head. In the next manifest, its default held to
//! `button.css`, and its words held to the floor.

use serde_json::{Value, json};

use super::parts;
use super::*;

fn with_parts(parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": "purlis-classic", "parts": parts })
}

#[test]
fn the_quiet_link_is_a_variant_of_the_next_manifest_held_to_its_words_floor() {
    // Additive, so a theme written against the released manifest still reads.
    let link = parts::MANIFEST[0]
        .variants
        .iter()
        .find(|one| one.name == "quiet-link")
        .expect("the quiet link");
    assert_eq!(link.since, parts::NEXT);
    // Its default: no box and quiet words; under the pointer, primary words and still no
    // ground, since it is underlined rather than filled.
    let held = parts::defaults_json("quiet-link");
    assert_eq!(held["fill"], "transparent");
    assert_eq!(held["text"], "text.quiet");
    assert_eq!(held["border"], "transparent");
    assert_eq!(held["hover"], "transparent");
    // A theme that takes its words under the floor is told so.
    let faded = load(&with_parts(
        json!({ "button.root.quiet-link.text": "surface.hover" }),
    ));
    let fallen = parts::fallen(faded.theme.first());
    assert!(
        fallen.iter().any(|one| one.part == "button.root.quiet-link"),
        "{fallen:?}"
    );
}
