// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The hard floors' findings name every layer a pair is laid over (#2756), in their line and
//! their JSON, the way the check's warnings do.

use serde_json::json;

use super::*;
use crate::theme::{Theme, load};

/// A one-appearance dark theme that sets `tokens` over Classic's.
fn dark(tokens: serde_json::Value) -> Theme {
    load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }))
    .theme
}

/// Classic's light words on a clear selection over a clear danger wash over a light dialog.
fn under_two_clear_layers() -> Theme {
    dark(json!({
        "selection.background": "#00000000",
        "danger.wash": "#00000000",
        "surface.overlay": "#f2f2f2",
    }))
}

/// A pair laid over two layers that a hard floor holds. No pair of [`PAIRS`] is, so this one is
/// the test's own.
static OVER_TWO: [Pair; 1] = [Pair {
    front: "text.primary",
    back: "selection.background",
    floor: 4.5,
    under: Some("danger.wash"),
    beneath: Some("surface.overlay"),
    fade: None,
    selected: false,
}];

/// What `theme` breaks of `pairs` as it is, untinted.
fn untinted(theme: &Theme, pairs: &'static [Pair]) -> Vec<Broken> {
    broken_among(theme.first(), pairs)
        .into_iter()
        .filter(|one| one.tint.is_none())
        .collect()
}

#[test]
fn a_broken_pair_over_two_layers_is_said_with_both() {
    let found = untinted(&under_two_clear_layers(), &OVER_TWO);
    let [broken] = found.as_slice() else {
        panic!("one pair broken: {found:?}");
    };
    assert_eq!(
        (broken.floor, broken.under, broken.beneath),
        (
            HardFloor::PrimaryText,
            Some("danger.wash"),
            Some("surface.overlay")
        )
    );
    let said = broken.said();
    assert!(
        said.starts_with(
            "primary text is readable on every surface: text.primary on selection.background \
             over danger.wash over surface.overlay is "
        ),
        "{said}"
    );
    let json = serde_json::to_value(broken).expect("JSON");
    assert_eq!(json["under"], "danger.wash");
    assert_eq!(json["beneath"], "surface.overlay");
}

#[test]
fn a_broken_pair_over_one_layer_is_said_with_it_and_has_no_beneath() {
    // The danger wash laid clear over a light window: Classic's light words vanish into both.
    let theme = dark(json!({ "danger.wash": "#00000000", "surface.base": "#f2f2f2" }));
    let found = untinted(&theme, PAIRS);
    let broken = found
        .iter()
        .find(|one| one.back == "danger.wash" && one.under == Some("surface.base"))
        .unwrap_or_else(|| panic!("the wash over the window is broken: {found:?}"));
    assert!(
        broken
            .said()
            .contains(": text.primary on danger.wash over surface.base is "),
        "{}",
        broken.said()
    );
    let json = serde_json::to_value(broken).expect("JSON");
    assert_eq!(json["under"], "surface.base");
    assert!(json.get("beneath").is_none(), "{json}");
}
