// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The families that hold what the window says at the engine's interface (#2912): a card, a
//! Notice, a badge, a count and a separator in the manifest, their colours held to their
//! stylesheets, an ask's edge and a Notice's ground held for consent, a separator measured for no
//! words, `separator.width`, and Purlis's controls drawn with no border while Allow keeps its
//! edge.

use serde_json::{Value, json};

use super::parts;
use super::*;

/// A theme over Purlis Classic that sets `parts`.
fn with_parts(parts: Value) -> Value {
    json!({ "name": "Shaped", "extends": "purlis-classic", "parts": parts })
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// The stylesheet of a theme's first appearance, which must be drawn.
fn sheet_of(parts: Value) -> String {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    parts::stylesheet(loaded.theme.first())
}

// ---------------------------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------------------------

#[test]
fn the_content_families_are_in_the_manifest_at_version_10() {
    let manifest = parts::manifest_json();
    let families = manifest["families"].as_array().unwrap();
    let of = |name: &str| -> (Vec<String>, Vec<(String, bool)>) {
        let family = families
            .iter()
            .find(|one| one["name"] == name)
            .unwrap_or_else(|| panic!("{name} is in the manifest"));
        assert_eq!(family["since"], 10, "{name}");
        let parts = family["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect();
        let variants = family["variants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| {
                (
                    one["name"].as_str().unwrap().to_owned(),
                    one["consent"].as_bool().unwrap(),
                )
            })
            .collect();
        (parts, variants)
    };
    let named =
        |list: &[&str]| -> Vec<String> { list.iter().map(|one| (*one).to_owned()).collect() };
    let plain = |list: &[&str]| -> Vec<(String, bool)> {
        list.iter().map(|one| ((*one).to_owned(), false)).collect()
    };
    assert_eq!(
        of("card"),
        (
            named(&["root", "title"]),
            vec![("raised".into(), false), ("ask".into(), true)]
        )
    );
    assert_eq!(
        of("notice"),
        (
            named(&["root", "message", "whose"]),
            vec![("news".into(), true), ("trouble".into(), true)]
        )
    );
    assert_eq!(
        of("badge"),
        (
            named(&["root"]),
            plain(&["neutral", "accent", "waiting", "ok"])
        )
    );
    assert_eq!(
        of("count"),
        (
            named(&["root", "group"]),
            plain(&["neutral", "accent", "waiting", "hot"])
        )
    );
    assert_eq!(of("separator"), (named(&["root"]), Vec::new()));
}

#[test]
fn a_notice_s_edge_width_is_no_theme_s() {
    let loaded = load(&with_parts(json!({ "notice.root.border-width": 0 })));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("parts.notice.root.border-width is not")),
        "{:?}",
        said(&loaded)
    );
    assert!(!parts::stylesheet(loaded.theme.first()).contains("border-width"));
}

// ---------------------------------------------------------------------------------------
// Consent: an ask's edge, and the ground answers stand on
// ---------------------------------------------------------------------------------------

#[test]
fn an_ask_s_edge_cannot_be_turned_off() {
    let loaded = load(&with_parts(json!({ "card.root.ask.border-width": 0 })));
    assert!(loaded.refused);
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.contains("card.root.ask.border-width")
                && one.contains("an ask keeps its waiting edge")),
        "{:?}",
        said(&loaded)
    );
}

#[test]
fn a_theme_that_turns_every_card_s_border_off_still_draws_the_ask_s() {
    let sheet = sheet_of(json!({ "card.root.border-width": 0 }));
    assert!(
        sheet.contains(
            ":where([data-scope=\"card\"][data-part=\"root\"]) { border-width: 0px; border-style: solid; }"
        ),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            ":where([data-scope=\"card\"][data-part=\"root\"][data-variant=\"ask\"]) { border-width: 1px; border-style: solid; }"
        ),
        "{sheet}"
    );
}

#[test]
fn an_ask_whose_edge_is_lost_on_its_fill_draws_its_default() {
    let loaded = load(&with_parts(
        json!({ "card.root.ask.border-colour": "surface.raised" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    // Its edge is lost on its fill: the reading floor's border or the ask's own, whichever
    // finds it first, and either way the ask draws its waiting edge.
    assert!(
        fell.iter().any(|one| one.part == "card.root.ask"),
        "{fell:?}"
    );
}

#[test]
fn a_card_and_a_notice_stand_their_answers_only_on_a_ground_a_button_is_measured_on() {
    for key in ["card.root.fill", "notice.root.trouble.fill"] {
        let loaded = load(&with_parts(json!({ key: "accent.base" })));
        assert!(loaded.refused, "{key}");
        assert!(
            said(&loaded)
                .iter()
                .any(|one| one.contains(key) && one.contains("held apart from the primary")),
            "{key}: {:?}",
            said(&loaded)
        );
    }
    // The grounds a button is held on are theirs to take, and so is none.
    let sheet = sheet_of(json!({
        "card.root.fill": "surface.base",
        "notice.root.fill": "transparent",
    }));
    assert!(
        sheet.contains("background-color: var(--surface-base)"),
        "{sheet}"
    );
}

// ---------------------------------------------------------------------------------------
// A separator, a badge and a count
// ---------------------------------------------------------------------------------------

#[test]
fn a_separator_has_no_words_to_measure() {
    let loaded = load(&with_parts(
        json!({ "separator.root.fill": "text.primary" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert_eq!(parts::fallen(loaded.theme.first()), Vec::new());
    assert!(
        parts::stylesheet(loaded.theme.first()).contains(
            ":where([data-scope=\"separator\"][data-part=\"root\"]) { background-color: var(--text-primary); }"
        )
    );
}

#[test]
fn a_badge_whose_words_break_their_floor_draws_its_default() {
    let loaded = load(&with_parts(
        json!({ "badge.root.waiting.text": "wash.warning" }),
    ));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let fell = parts::fallen(loaded.theme.first());
    assert!(
        fell.iter().any(|one| one.part == "badge.root.waiting"),
        "{fell:?}"
    );
}

#[test]
fn a_count_is_no_target_and_is_not_held_to_one() {
    // A count is as tall as its pill: a small font is no breach of the least target.
    let sheet = sheet_of(json!({ "count.root.font-size": 10, "count.root.padding": [0, 4] }));
    assert!(
        sheet.contains("[data-scope=\"count\"][data-part=\"root\"]"),
        "{sheet}"
    );
}

#[test]
fn the_separator_width_is_a_geometry_step_from_0_to_2_px() {
    let written = geometry::properties(&geometry::classic());
    assert_eq!(written["--separator-width"], "1px");
    let none = load(&json!({
        "name": "Lineless",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "separator.width": 0 } }],
    }));
    assert!(!none.refused, "{:?}", said(&none));
    let wide = load(&json!({
        "name": "Heavy",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": { "separator.width": 3 } }],
    }));
    assert!(wide.refused);
    assert!(
        said(&wide)
            .iter()
            .any(|one| one.contains("separator.width") && one.contains("from 0 to 2 px")),
        "{:?}",
        said(&wide)
    );
}

// ---------------------------------------------------------------------------------------
// Purlis: no control takes a border, Allow keeps its edge, cards on the window's fill
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_s_controls_take_no_border_at_any_size_and_allow_keeps_its_edge() {
    for theme in &purlis().appearances {
        let sheet = parts::stylesheet(theme);
        let roots = [
            "[data-scope=\"button\"][data-part=\"root\"][data-variant=\"secondary\"]",
            "[data-scope=\"button\"][data-part=\"root\"][data-variant=\"primary\"]",
            "[data-scope=\"text-input\"][data-part=\"root\"]",
            "[data-scope=\"textarea\"][data-part=\"root\"]",
        ];
        for root in roots {
            // The edge is taken off by its colour, which reaches the small, the large and a
            // wrapping row's controls as it reaches the default size.
            let line = sheet
                .lines()
                .find(|line| line.contains(&format!(":where({root})")))
                .unwrap_or_else(|| panic!("Purlis {}: {root}: {sheet}", theme.id));
            assert!(line.contains("border-color: transparent"), "{line}");
            assert!(!line.contains(":not("), "{line}");
        }
        // Allow and danger, the consent variants, are untouched: each keeps the family's edge.
        assert!(!sheet.contains("[data-variant=\"grants\"]"), "{sheet}");
        assert!(!sheet.contains("[data-variant=\"danger\"]"), "{sheet}");
        assert!(
            sheet.contains(
                ":where([data-scope=\"card\"][data-part=\"root\"]) { background-color: var(--surface-base); }"
            ),
            "Purlis {}: {sheet}",
            theme.id
        );
        assert_eq!(parts::fallen(theme), Vec::new(), "Purlis {}", theme.id);
    }
}

// ---------------------------------------------------------------------------------------
// The family defaults are the window's own stylesheets
// ---------------------------------------------------------------------------------------

/// A stylesheet with prettier's wrapping of a long selector read back onto one line, and no
/// comments.
fn css_of(text: &str) -> String {
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

/// The declarations of the rule `selector` opens in `css`, if one does.
fn rule<'a>(css: &'a str, selector: &str) -> Option<&'a str> {
    let at = css.find(&format!("\n{selector} {{"))?;
    let body = &css[at..];
    Some(&body[..body.find('}').unwrap()])
}

fn declared<'a>(body: &'a str, property: &str) -> Option<&'a str> {
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&format!("{property}: ")))
        .map(|value| value.trim_end_matches(';'))
}

/// The token a declared value names: `var(--surface-base)` is `surface.base`; a border's colour
/// is its last word.
fn token(value: &str) -> String {
    if value == "transparent" {
        return value.to_owned();
    }
    let name = value
        .rsplit("var(--")
        .next()
        .and_then(|it| it.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{value} names no token"));
    vocabulary::TOKENS
        .iter()
        .find(|token| token.replace('.', "-") == name)
        .unwrap_or_else(|| panic!("--{name} is not a token"))
        .to_string()
}

/// `family`'s colours in `variant` as `css` draws them: its root's rule, then the variant's own.
fn drawn_in(css: &str, family: &str, variant: &str) -> [String; 3] {
    let root = format!("[data-scope=\"{family}\"][data-part=\"root\"]");
    let base = rule(css, &root).unwrap_or_else(|| panic!("no rule {root}"));
    let own = (!variant.is_empty())
        .then(|| rule(css, &format!("{root}[data-variant=\"{variant}\"]")))
        .flatten();
    let pick = |property: &str| {
        own.and_then(|body| declared(body, property))
            .or_else(|| declared(base, property))
    };
    let border = pick("border-color")
        .map(token)
        .or_else(|| {
            pick("border").map(|value| {
                let last = value.rsplit(' ').next().unwrap();
                if last.contains("var(") {
                    token(last)
                } else {
                    "transparent".to_owned()
                }
            })
        })
        .unwrap_or_else(|| "transparent".to_owned());
    [
        pick("background-color").map_or_else(|| "transparent".to_owned(), token),
        pick("color").map_or_else(|| "transparent".to_owned(), token),
        border,
    ]
}

#[test]
fn the_content_families_defaults_are_what_their_stylesheets_draw() {
    let card = css_of(include_str!("../../../../app/src/ui/card/card.css"));
    let notice = css_of(include_str!("../../../../app/src/ui/notice/notice.css"));
    let badge = css_of(include_str!("../../../../app/src/ui/badge/badge.css"));
    let separator = css_of(include_str!(
        "../../../../app/src/ui/separator/separator.css"
    ));
    let cases: [(&str, &str, &[&str]); 5] = [
        (&card, "card", &["raised", "ask"]),
        (&notice, "notice", &["news", "trouble"]),
        (&badge, "badge", &["neutral", "accent", "waiting", "ok"]),
        (&badge, "count", &["neutral", "accent", "waiting", "hot"]),
        (&separator, "separator", &[""]),
    ];
    for (css, family, variants) in cases {
        for variant in variants {
            let held = parts::look_of(family, variant, &[]);
            let [fill, text, border] = drawn_in(css, family, variant);
            assert_eq!(held[0], fill, "{family} {variant}: fill");
            // A separator has no words: its stylesheet names no colour for them.
            if family != "separator" {
                assert_eq!(held[1], text, "{family} {variant}: words");
            }
            assert_eq!(held[2], border, "{family} {variant}: border");
        }
    }
}

// ---------------------------------------------------------------------------------------
// A Notice's waiting edge, an ask's hue and the separator's colour
// ---------------------------------------------------------------------------------------

/// Whether `parts` over Purlis Classic draw `look` as its family default in every appearance.
fn falls(parts: Value, look: &str) -> bool {
    let loaded = load(&with_parts(parts));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    loaded
        .theme
        .appearances
        .iter()
        .all(|theme| parts::fallen(theme).iter().any(|one| one.part == look))
}

#[test]
fn a_notice_s_edge_that_would_not_show_draws_its_default() {
    // No edge at all, in either look.
    for look in ["notice.root.news", "notice.root.trouble"] {
        assert!(
            falls(json!({ "notice.root.border-colour": "transparent" }), look),
            "{look}"
        );
    }
    // An edge lost on a fill of its own colour.
    assert!(falls(
        json!({ "notice.root.fill": "border.subtle", "notice.root.border-colour": "border.subtle" }),
        "notice.root.news"
    ));
    // An edge lost on its ground, through a fill of none, in either look.
    for look in ["notice.root.news", "notice.root.trouble"] {
        assert!(
            falls(
                json!({ "notice.root.fill": "transparent", "notice.root.border-colour": "surface.raised" }),
                look
            ),
            "{look}"
        );
    }
}

#[test]
fn an_ask_s_edge_in_the_accent_draws_its_default() {
    assert!(falls(
        json!({ "card.root.ask.border-colour": "accent.base" }),
        "card.root.ask"
    ));
    assert!(falls(
        json!({ "notice.root.trouble.border-colour": "accent.base" }),
        "notice.root.trouble"
    ));
}

#[test]
fn the_separator_s_colour_reaches_every_line_between_groups() {
    let sheet = sheet_of(json!({ "separator.root.fill": "border.strong" }));
    for line in [
        "[data-scope=\"popover\"][data-part=\"rule\"]",
        "[data-scope=\"dialog\"][data-part=\"separator\"]",
        "[data-scope=\"status-bar\"][data-part=\"separator\"]",
    ] {
        assert!(sheet.contains(line), "{line}: {sheet}");
    }
    assert!(
        sheet.contains(":root { --separator-fill: var(--border-strong); }"),
        "{sheet}"
    );
    // A family's own key for its line is written after it, so it wins.
    let sheet = sheet_of(json!({
        "separator.root.fill": "border.strong",
        "dialog.separator.fill": "border.subtle",
    }));
    assert!(
        sheet.find("--separator-fill").unwrap() < sheet.find("var(--border-subtle)").unwrap(),
        "{sheet}"
    );
}

#[test]
fn a_family_measured_as_written_alone_falls_as_it_would_in_every_tint() {
    // The skip: a family whose rules set no colour, or only take an edge off a variant with no
    // consent, is measured as written alone. What falls is what falls in every tint.
    let themes = [
        json!({}),
        json!({ "button.root.primary.border-colour": "transparent", "button.root.secondary.border-colour": "transparent", "button.root.ghost.border-colour": "transparent" }),
        json!({ "button.root.border-width": 2, "button.root.grants.border-width": 2 }),
        json!({ "card.root.ask.border-width": 2, "notice.root.corner-radius": 4 }),
        json!({ "button.root.corner-radius": "full", "button.root.padding": [2, 12] }),
    ];
    for parts in themes {
        for theme in &load(&with_parts(parts.clone())).theme.appearances {
            assert_eq!(
                parts::fallen(theme),
                parts::fallen_in_every_tint(theme),
                "{parts} {}",
                theme.id
            );
        }
    }
    for theme in &purlis().appearances {
        assert_eq!(
            parts::fallen(theme),
            parts::fallen_in_every_tint(theme),
            "Purlis {}",
            theme.id
        );
    }
}
