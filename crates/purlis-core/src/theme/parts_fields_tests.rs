// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The textarea's hint (#5214): what a reply strip at an Inbox card's foot says while it holds
//! the caret. In the next manifest, in the quiet tier, as `field.css` draws it.

use super::parts;

#[test]
fn the_textareas_hint_is_a_part_of_the_next_manifest_in_the_quiet_tier() {
    let textarea = parts::MANIFEST
        .iter()
        .find(|one| one.name == "textarea")
        .expect("the textarea family");
    let at = textarea
        .parts
        .iter()
        .position(|one| one.name == "hint")
        .expect("the hint");
    assert_eq!(textarea.parts[at].since, 13);
    assert_eq!(textarea.own_words_of(at), Some("text.quiet"));
    let css = include_str!("../../../../app/src/ui/field/field.css");
    let drawn = css
        .split("\n[data-scope=\"textarea\"][data-part=\"hint\"] {")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("field.css draws the hint");
    assert!(drawn.contains("color: var(--text-quiet);"), "{drawn}");
}
