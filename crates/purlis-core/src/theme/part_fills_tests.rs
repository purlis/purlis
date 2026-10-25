// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every part's words are read on the fill it sets** (#3099): a part that takes a fill and
//! words declares the words it is drawn in and, where its stylesheet gives it a ground of its
//! own, that fill, so the floors read its words on what is under them, from the one rule
//! [`super::drawn`] keeps, never a family's own.

use serde_json::json;

use super::super::vocabulary::TOKENS;
use super::super::{ResolvedTheme, load};
use super::{MANIFEST, Prop, fallen, family};

/// Every part but a root that takes both a fill and words.
fn filled_with_words() -> Vec<(&'static str, &'static str)> {
    MANIFEST
        .iter()
        .flat_map(|family| {
            family.parts.iter().skip(1).filter_map(move |part| {
                (part.properties.contains(&Prop::Fill) && part.properties.contains(&Prop::Text))
                    .then_some((family.name, part.name))
            })
        })
        .collect()
}

/// A part's declared words and fill.
fn declared_of(family_name: &str, part: &str) -> (Option<&'static str>, Option<&'static str>) {
    let family = family(family_name).expect("a family");
    let at = family
        .parts
        .iter()
        .position(|one| one.name == part)
        .expect("a part");
    (family.own_words(at), family.own_fill(at))
}

#[test]
fn every_part_that_takes_a_fill_and_words_declares_the_words_it_is_drawn_in() {
    let missing: Vec<String> = filled_with_words()
        .into_iter()
        .filter(|(family, part)| declared_of(family, part).0.is_none())
        .map(|(family, part)| format!("{family}.{part}"))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

/// A stylesheet with its comments left out.
fn uncommented(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..]
            .find("*/")
            .map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// What the rules `selectors` open in `css` declare for `property`, the last that declares it
/// winning as the cascade has it, as a token: `None` for none, and for `transparent`.
fn drawn_as(css: &str, selectors: &[&str], property: &str) -> Option<String> {
    let value = selectors
        .iter()
        .filter_map(|selector| {
            let at = css
                .find(&format!("\n{selector} {{"))
                .unwrap_or_else(|| panic!("no rule {selector}"));
            let body = &css[at..];
            let body = &body[..body.find('}').expect("the rule ends")];
            body.lines().map(str::trim).find_map(|line| {
                line.strip_prefix(&format!("{property}: "))
                    .map(|value| value.trim_end_matches(';'))
            })
        })
        .next_back()?;
    let name = value.strip_prefix("var(--")?.strip_suffix(')')?;
    TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .map(|token| (*token).to_owned())
}

/// A part, the stylesheet and rules that draw it, and the surface it stands on: see [`DRAWN`].
type Drawn = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<&'static str>,
);

/// **Where each part's words and fill are drawn**: its family, its part, the stylesheet and the
/// rules that draw it at rest, and the surface it stands on where its stylesheet gives it no
/// ground and that is not its root's (a switcher's row, on the popover it is drawn in).
const DRAWN: &[Drawn] = &[
    (
        "tabs",
        "closer",
        include_str!("../../../../app/src/ui/tabs/tabs.css"),
        &["[data-scope=\"tabs\"][data-part=\"tab\"] [data-part=\"closer\"][data-scope=\"tabs\"]"],
        None,
    ),
    (
        "tabs",
        "count",
        include_str!("../../../../app/src/App.css"),
        &[
            ".tabs .tab-tasks button",
            ".tabs .tab-tasks button.tab-tasks-counts",
        ],
        None,
    ),
    (
        "popover",
        "footer",
        include_str!("../../../../app/src/ui/popover/popover.css"),
        &["[data-scope=\"popover\"][data-part=\"footer\"][data-of=\"keys\"]"],
        None,
    ),
    (
        "popover",
        "key",
        include_str!("../../../../app/src/ui/popover/popover.css"),
        &["[data-scope=\"popover\"][data-part=\"key\"]"],
        None,
    ),
    (
        "rail",
        "badge",
        include_str!("../../../../app/src/ui/rail/rail.css"),
        &["[data-part=\"badge\"][data-scope=\"rail\"]"],
        None,
    ),
    (
        "activity",
        "count",
        include_str!("../../../../app/src/ui/activity/activity.css"),
        &["[data-scope=\"activity\"][data-part=\"count\"]"],
        None,
    ),
    (
        "titlebar",
        "count",
        include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        &["[data-part=\"count\"][data-scope=\"titlebar\"]"],
        None,
    ),
    (
        "titlebar",
        "crumb",
        include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        &["[data-part=\"crumb\"][data-scope=\"titlebar\"]"],
        None,
    ),
    (
        "titlebar",
        "all",
        include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        &["[data-part=\"all\"][data-scope=\"titlebar\"]"],
        None,
    ),
    (
        "titlebar",
        "pin",
        include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        &["[data-part=\"pin\"][data-scope=\"titlebar\"]"],
        Some("surface.overlay"),
    ),
    (
        "titlebar",
        "row",
        include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        &["[data-part=\"row\"][data-scope=\"titlebar\"]"],
        Some("surface.overlay"),
    ),
];

#[test]
fn every_part_that_takes_a_fill_and_words_is_drawn_where_its_stylesheet_says() {
    let listed: Vec<(&str, &str)> = DRAWN.iter().map(|one| (one.0, one.1)).collect();
    assert_eq!(
        listed,
        filled_with_words(),
        "every such part, in the manifest's order"
    );
    for (family, part, css, selectors, on) in DRAWN {
        let css = uncommented(css);
        let (words, fill) = declared_of(family, part);
        let colour = drawn_as(&css, selectors, "color");
        let ground = drawn_as(&css, selectors, "background-color")
            .or_else(|| drawn_as(&css, selectors, "background"));
        assert_eq!(words.map(str::to_owned), colour, "{family}.{part}'s words");
        assert_eq!(
            fill.map(str::to_owned),
            ground.or(on.map(str::to_owned)),
            "{family}.{part}'s fill"
        );
    }
}

/// A theme over Purlis Classic that sets `tokens` and `parts`, as read.
fn shaped(tokens: serde_json::Value, parts: serde_json::Value) -> ResolvedTheme {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }],
        "parts": parts,
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    loaded.theme.first().clone()
}

/// What fell, as `<part> <floor>`.
fn fell_in(theme: &ResolvedTheme) -> Vec<String> {
    fallen(theme)
        .into_iter()
        .map(|one| format!("{} {}", one.part, one.floor))
        .collect()
}

#[test]
fn a_footer_and_a_key_drawn_in_their_own_fill_fall_back_and_the_check_names_them() {
    for part in ["footer", "key"] {
        let theme = shaped(
            json!({}),
            json!({
                format!("popover.{part}.fill"): "surface.raised",
                format!("popover.{part}.text"): "surface.raised",
            }),
        );
        let fell = fell_in(&theme);
        assert!(
            fell.iter()
                .any(|one| one.contains(&format!("the {part}'s words contrast 4.5:1"))),
            "{part}: {fell:?}"
        );
        let report = super::super::check(&theme);
        assert!(
            report.findings.iter().any(|one| matches!(
                one,
                super::super::check::Finding::PartFallsBack { floor, .. }
                    if floor.contains(&format!("the {part}'s words"))
            )),
            "{part}: {:?}",
            report.findings
        );
    }
}

#[test]
fn a_key_s_words_are_read_on_its_raised_cap_not_on_the_popover() {
    // A light cap in a dark theme: the primary words read on the popover's surface, but not on
    // the key's own raised cap, which is what is under them.
    let theme = shaped(
        json!({ "surface.raised": "#eeeeee" }),
        json!({ "popover.key.text": "text.primary" }),
    );
    let fell = fell_in(&theme);
    assert!(
        fell.iter()
            .any(|one| one.contains("the key's words contrast 4.5:1")),
        "{fell:?}"
    );
}

#[test]
fn a_tab_s_closer_is_held_as_a_mark() {
    // A faint × that reads as a mark (3:1) but not as words (4.5:1) is drawn as written.
    let theme = shaped(json!({}), json!({ "tabs.closer.text": "text.faint" }));
    let fell = fell_in(&theme);
    assert!(!fell.iter().any(|one| one.contains("closer")), "{fell:?}");
}
