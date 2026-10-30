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

/// **Where a part is drawn** (#3099): its family and part, the stylesheet, the rules that draw
/// the part itself at rest, the rules of what it stands in (a tab's counts button around its
/// count), and the surface it stands on where no rule of its stylesheet paints that (a
/// switcher's row, on the popover it is drawn in).
struct Drawn {
    family: &'static str,
    part: &'static str,
    css: &'static str,
    own: &'static [&'static str],
    around: &'static [&'static str],
    on: Option<&'static str>,
}

/// Every part that takes a fill and words, in the manifest's order.
const DRAWN: &[Drawn] = &[
    Drawn {
        family: "tabs",
        part: "closer",
        css: include_str!("../../../../app/src/ui/tabs/tabs.css"),
        own: &[
            "[data-scope=\"tabs\"][data-part=\"tab\"] [data-part=\"closer\"][data-scope=\"tabs\"]",
        ],
        around: &[],
        on: None,
    },
    Drawn {
        family: "tabs",
        part: "count",
        css: include_str!("../../../../app/src/App.css"),
        own: &[".tab-tasks-counts .count"],
        // The chip's pill, which the counts' quiet ghost lets show through (#3264).
        around: &[".tabs .tab-tasks-pill"],
        on: None,
    },
    Drawn {
        family: "status-item",
        part: "dot",
        css: include_str!("../../../../app/src/ui/status/status.css"),
        own: &["[data-part=\"dot\"][data-scope=\"status-item\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "status-item",
        part: "dot-warning",
        css: include_str!("../../../../app/src/ui/status/status.css"),
        own: &["[data-part=\"dot-warning\"][data-scope=\"status-item\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "popover",
        part: "footer",
        css: include_str!("../../../../app/src/ui/popover/popover.css"),
        own: &["[data-scope=\"popover\"][data-part=\"footer\"][data-of=\"keys\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "popover",
        part: "key",
        css: include_str!("../../../../app/src/ui/popover/popover.css"),
        own: &["[data-scope=\"popover\"][data-part=\"key\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "rail",
        part: "badge",
        css: include_str!("../../../../app/src/ui/rail/rail.css"),
        own: &["[data-part=\"badge\"][data-scope=\"rail\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "activity",
        part: "count",
        css: include_str!("../../../../app/src/ui/activity/activity.css"),
        own: &["[data-scope=\"activity\"][data-part=\"count\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "count",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"count\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "crumb",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"crumb\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "all",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"all\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "pin",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"pin\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: Some("surface.overlay"),
    },
    Drawn {
        family: "titlebar",
        part: "row",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"row\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: Some("surface.overlay"),
    },
    Drawn {
        family: "titlebar",
        part: "dot",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"dot\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "dot-warning",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"dot-warning\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
    Drawn {
        family: "titlebar",
        part: "dot-needs",
        css: include_str!("../../../../app/src/ui/titlebar/titlebar.css"),
        own: &["[data-part=\"dot-needs\"][data-scope=\"titlebar\"]"],
        around: &[],
        on: None,
    },
];

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

#[test]
fn every_part_that_takes_a_fill_and_words_is_drawn_where_its_stylesheet_says() {
    let listed: Vec<(&str, &str)> = DRAWN.iter().map(|one| (one.family, one.part)).collect();
    assert_eq!(
        listed,
        filled_with_words(),
        "every such part, in the manifest's order"
    );
    for one in DRAWN {
        let css = uncommented(one.css);
        let named = format!("{}.{}", one.family, one.part);
        let family = family(one.family).expect("a family");
        let at = family
            .parts
            .iter()
            .position(|part| part.name == one.part)
            .expect("a part");
        let painted = |selectors: &[&str]| {
            drawn_as(&css, selectors, "background-color")
                .or_else(|| drawn_as(&css, selectors, "background"))
        };
        // Its words: its own colour, or the colour of what it stands in.
        let colour =
            drawn_as(&css, one.own, "color").or_else(|| drawn_as(&css, one.around, "color"));
        assert_eq!(
            family.own_words(at).map(str::to_owned),
            colour,
            "{named}'s words"
        );
        // A fill is what the part paints itself, and only that: a fallback paints it again.
        assert_eq!(
            family.own_fill(at).map(str::to_owned),
            painted(one.own),
            "{named}'s own fill"
        );
        // What it stands on otherwise is read, never painted.
        let under = family
            .under
            .iter()
            .find(|(part, _)| *part == one.part)
            .map(|(_, token)| (*token).to_owned());
        assert_eq!(
            under,
            painted(one.around).or(one.on.map(str::to_owned)),
            "{named}'s ground"
        );
    }
}

#[test]
fn a_fallback_paints_only_the_fills_a_part_paints_itself() {
    // A tab whose words vanish on its fill falls back; the count inside it, which paints no
    // ground of its own, is not given its chip's sunken pill.
    let theme = shaped(
        json!({}),
        json!({ "tabs.tab.fill": "surface.base", "tabs.tab.text": "surface.base" }),
    );
    assert!(!fell_in(&theme).is_empty(), "the tab falls back");
    let sheet = super::stylesheet(&theme);
    assert!(
        !sheet.lines().any(
            |line| line.contains("[data-scope=\"tabs\"][data-part=\"count\"]")
                && line.contains("background-color")
        ),
        "{sheet}"
    );
    // The key's cap, which the key paints, is painted again by a popover's fallback.
    let popover = shaped(
        json!({}),
        json!({ "popover.key.fill": "surface.raised", "popover.key.text": "surface.raised" }),
    );
    assert!(
        super::stylesheet(&popover).lines().any(|line| line
            .contains("[data-scope=\"popover\"][data-part=\"key\"]")
            && line.contains("background-color: var(--surface-raised)")),
        "the key's cap is painted again"
    );
}
