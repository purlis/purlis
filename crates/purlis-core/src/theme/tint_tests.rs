// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A theme's `tint` at the engine's interface (#2329): on and off, which tokens a workspace's
//! colour turns, the theme's own hues, Purlis Classic's recipe kept as it was, and the check
//! measuring every tint of a theme that changes its recipe.

use serde_json::{Value, json};

use super::vocabulary::{TAB_ONLY, TINTED, TOKENS};
use super::*;

/// The shared fixture: its `classic` entry is the recipe the window is handed for Purlis Classic.
const TINT_CASES: &str = include_str!("tint-cases.json");

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// A format 2 theme with one dark appearance and `tint`.
fn tinting(tint: Value) -> Value {
    json!({
        "name": "Tinted",
        "tint": tint,
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    })
}

/// The first appearance of a theme that loads without a refusal.
fn drawn(document: &Value) -> ResolvedTheme {
    let loaded = load(document);
    assert!(!loaded.refused, "{:?}", said(&loaded));
    loaded.theme.first().clone()
}

/// The tokens `tint` changed in `theme`.
fn moved(theme: &ResolvedTheme, tint: &ResolvedTheme) -> Vec<&'static str> {
    TOKENS
        .iter()
        .copied()
        .filter(|token| theme.value(token) != tint.value(token))
        .collect()
}

/// The OKLCH hue a value has.
fn hue(hex: &str) -> f64 {
    hue_of(&hex[..7]).expect("a colour with a hue")
}

fn apart(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

// ---------------------------------------------------------------------------------------
// Purlis Classic's recipe is today's
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_tints_what_it_tinted_before_a_theme_could_say() {
    for theme in built_ins() {
        let recipe = &theme.tint;
        assert!(recipe.on);
        assert_eq!(recipe.tabs(), TINTED);
        assert_eq!(
            recipe.window(),
            [
                "accent.base",
                "accent.surface",
                "focus.ring",
                "tab.active",
                "list.selected",
                "list.selected-edge"
            ]
        );
        let hues: Vec<(&str, f64)> = recipe.hues.iter().map(|one| (one.name, one.hue)).collect();
        assert_eq!(
            hues,
            [
                ("red", 25.0),
                ("orange", 55.0),
                ("yellow", 95.0),
                ("green", 145.0),
                ("teal", 190.0),
                ("blue", 255.0),
                ("purple", 305.0),
                ("pink", 350.0),
            ]
        );
    }
}

#[test]
fn the_window_is_handed_classic_s_recipe_as_the_shared_fixture_holds_it() {
    let cases: Value = serde_json::from_str(TINT_CASES).unwrap();
    let handed = serde_json::to_value(&default_theme().tint).unwrap();
    assert_eq!(handed, cases["classic"]);
}

#[test]
fn a_theme_that_says_nothing_of_tint_has_classic_s_recipe() {
    let format_2 = drawn(&json!({
        "name": "Plain",
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    assert_eq!(format_2.tint, default_theme().tint);
    let format_1 = drawn(&json!({ "name": "Old", "tokens": {} }));
    assert_eq!(format_1.tint, default_theme().tint);
}

// ---------------------------------------------------------------------------------------
// On and off
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_that_turns_tint_off_is_drawn_as_itself_in_every_workspace_colour() {
    let theme = drawn(&tinting(json!({ "on": false })));
    assert!(!theme.tint.on);
    assert!(theme.tint.tabs().is_empty());
    assert!(theme.tint.window().is_empty());
    for colour in ["teal", "red", "#3a7bd5"] {
        assert_eq!(tinted(&theme, colour), theme, "{colour}");
    }
}

#[test]
fn on_is_true_or_false() {
    let loaded = load(&tinting(json!({ "on": "no" })));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        ["tint's on is \"no\", which is not true or false"]
    );
}

// ---------------------------------------------------------------------------------------
// Which tokens
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_names_the_tokens_a_workspace_s_colour_turns() {
    let theme = drawn(&tinting(
        json!({ "tokens": ["surface.raised", "accent.base"] }),
    ));
    // In the vocabulary's order, whatever order the file named them in.
    assert_eq!(theme.tint.tabs(), ["surface.raised", "accent.base"]);
    assert_eq!(theme.tint.window(), ["surface.raised", "accent.base"]);
    assert_eq!(
        moved(&theme, &tinted(&theme, "teal")),
        ["surface.raised", "accent.base"]
    );
}

#[test]
fn the_strip_shades_are_tinted_on_a_workspace_s_own_strip_and_not_on_the_window() {
    let theme = drawn(&tinting(
        json!({ "tokens": ["layer.divider", "text.muted"] }),
    ));
    assert_eq!(theme.tint.tabs(), ["text.muted", "layer.divider"]);
    assert_eq!(theme.tint.window(), ["text.muted"]);
    for token in TAB_ONLY {
        assert!(!default_theme().tint.window().contains(token), "{token}");
    }
}

#[test]
fn a_token_a_tint_may_not_turn_refuses_the_theme() {
    for token in [
        "danger.base",
        "needs-you.base",
        "state.failed",
        "terminal.background",
        "diff.inserted",
        "icon.red",
        "layer.project",
        "overlay.scrim",
    ] {
        let loaded = load(&tinting(json!({ "tokens": [token] })));
        assert!(loaded.refused, "{token}");
        assert_eq!(
            said(&loaded),
            [format!(
                "tint's tokens name {token}, which a workspace's colour never tints"
            )],
        );
    }
}

#[test]
fn a_token_name_purlis_does_not_have_is_said_and_left_out() {
    let loaded = load(&tinting(
        json!({ "tokens": ["accent.base", "accent.glow"] }),
    ));
    assert!(!loaded.refused);
    assert_eq!(
        said(&loaded),
        ["tint's tokens name accent.glow, which is not a purlis token, so it is left out"]
    );
    assert_eq!(loaded.theme.first().tint.tabs(), ["accent.base"]);
}

#[test]
fn tokens_is_a_list_of_token_names() {
    for wrong in [json!("accent.base"), json!([1]), json!({})] {
        let loaded = load(&tinting(json!({ "tokens": wrong })));
        assert!(loaded.refused, "{wrong}");
        assert_eq!(said(&loaded), ["tint's tokens is a list of token names"]);
    }
}

#[test]
fn an_empty_list_tints_nothing() {
    let theme = drawn(&tinting(json!({ "tokens": [] })));
    assert!(theme.tint.tabs().is_empty());
    assert_eq!(tinted(&theme, "teal"), theme);
}

#[test]
fn tint_is_an_object_of_on_tokens_and_hues() {
    let loaded = load(&tinting(json!(false)));
    assert!(loaded.refused);
    assert_eq!(said(&loaded), ["tint is an object of on, tokens and hues"]);
    let loaded = load(&tinting(json!({ "hue": {} })));
    assert!(loaded.refused);
    assert_eq!(said(&loaded), ["hue is not a key of tint"]);
}

// ---------------------------------------------------------------------------------------
// The theme's own hues
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_gives_a_workspace_colour_a_hue_of_its_own() {
    let theme = drawn(&tinting(json!({ "hues": { "red": "#3a7bd5" } })));
    let red = tinted(&theme, "red");
    let own = hue("#3a7bd5");
    assert!(apart(hue(red.value("accent.base")), own) < 4.0);
    // A colour the theme does not name keeps purlis's hue.
    assert_eq!(
        tinted(&theme, "teal").values,
        tinted(default_theme(), "teal").values
    );
    // And a workspace's own `#rrggbb` is its own hue whatever the theme says.
    assert_eq!(
        tinted(&theme, "#c0392b").values,
        tinted(default_theme(), "#c0392b").values
    );
    let named: Vec<&str> = theme.tint.hues.iter().map(|one| one.name).collect();
    assert_eq!(named, crate::extension::project::theme::PALETTE);
}

#[test]
fn a_hue_may_name_a_colour_of_the_theme_s_palette() {
    let theme = drawn(&json!({
        "name": "Branded",
        "palette": { "brand": "#3a7bd5" },
        "tint": { "hues": { "blue": "brand" } },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    let blue = theme
        .tint
        .hues
        .iter()
        .find(|one| one.name == "blue")
        .unwrap();
    assert!((blue.hue - hue("#3a7bd5")).abs() < 1e-9);
}

#[test]
fn a_hue_that_is_not_a_colour_with_a_hue_refuses_the_theme() {
    let cases = [
        (
            json!({ "mauve": "#3a7bd5" }),
            "tint's hues name mauve, which is not a workspace colour",
        ),
        (
            json!({ "red": "crimson" }),
            "tint's hue for red is \"crimson\", which is neither a hex colour nor a name in this theme's palette",
        ),
        (
            json!({ "red": "#808080" }),
            "tint's hue for red is #808080, a grey, which has no hue to give",
        ),
    ];
    for (hues, why) in cases {
        let loaded = load(&tinting(json!({ "hues": hues })));
        assert!(loaded.refused, "{why}");
        assert_eq!(said(&loaded), [why]);
    }
    let loaded = load(&tinting(json!({ "hues": ["#3a7bd5"] })));
    assert_eq!(
        said(&loaded),
        ["tint's hues is an object of workspace colours to colours"]
    );
}

// ---------------------------------------------------------------------------------------
// extends
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_starts_from_its_parent_s_recipe() {
    let parent = json!({
        "name": "Parent",
        "tint": { "tokens": ["accent.base"], "hues": { "red": "#3a7bd5" } },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    });
    let library = Library::new().with("parent", parent);
    let child = library.load(
        None,
        &json!({ "name": "Child", "extends": "parent", "tint": { "hues": { "blue": "#c0392b" } } }),
    );
    assert!(!child.refused, "{:?}", said(&child));
    let recipe = &child.theme.first().tint;
    assert_eq!(recipe.tabs(), ["accent.base"]);
    let of = |name: &str| recipe.hues.iter().find(|one| one.name == name).unwrap().hue;
    assert!((of("red") - hue("#3a7bd5")).abs() < 1e-9);
    assert!((of("blue") - hue("#c0392b")).abs() < 1e-9);
    assert!((of("teal") - 190.0).abs() < 1e-9);

    let off = library.load(
        None,
        &json!({ "name": "Off", "extends": "parent", "tint": { "on": false } }),
    );
    assert!(!off.theme.first().tint.on);
}

// ---------------------------------------------------------------------------------------
// The check measures the theme's own tints
// ---------------------------------------------------------------------------------------

/// The tints the check reported a finding under.
fn tints_found(report: &Report) -> Vec<Option<&'static str>> {
    let mut found: Vec<Option<&'static str>> = report
        .findings
        .iter()
        .map(|one| match one {
            Finding::Contrast { tint, .. }
            | Finding::Ansi { tint, .. }
            | Finding::SelectedNotApart { tint, .. } => *tint,
        })
        .collect();
    found.dedup();
    found
}

#[test]
fn the_check_measures_every_tint_of_a_theme_that_tints_its_own_tokens() {
    // Muted words that are too faint on the raised surface, which this theme tints: the check
    // finds them as the theme is and at every one of its hues.
    let faint = |tint: Value| {
        json!({
            "name": "Faint",
            "tint": tint,
            "appearances": [{
                "id": "dark", "appearance": "dark",
                "tokens": { "text.muted": "#333333" }
            }]
        })
    };
    let on = drawn(&faint(json!({
        "tokens": ["surface.raised", "text.muted"],
        "hues": { "red": "#3a7bd5" }
    })));
    let found = tints_found(&check(&on));
    assert_eq!(found.first(), Some(&None));
    for (colour, _) in PALETTE {
        assert!(found.contains(&Some(colour)), "{colour}");
    }
    // Off, the theme is only ever drawn as it is, so that is all the check measures.
    let off = drawn(&faint(json!({ "on": false })));
    assert_eq!(tints_found(&check(&off)), [None]);
}

#[test]
fn every_tint_a_theme_may_name_keeps_classic_s_contrast_up_to_one_channel_s_rounding() {
    // Every token a theme may tint, tinted at once on both of Classic's appearances, at each
    // palette hue: the tint keeps luminance, so a pair Classic clears is lost only where it
    // stood on its floor and one channel's rounding took a hundredth off. The check says so.
    for appearance in ["dark", "light"] {
        let theme = drawn(&json!({
            "name": "Everything",
            "tint": { "tokens": vocabulary::TINTABLE },
            "appearances": [{ "id": appearance, "appearance": appearance }]
        }));
        for finding in check(&theme).findings {
            let Finding::Contrast {
                tint: Some(_),
                pair,
                ratio,
            } = finding
            else {
                panic!("{appearance}: {finding:?}");
            };
            assert!(pair.floor - ratio <= 0.011, "{appearance}: {finding:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// A tinted accent is held apart from the danger colour
// ---------------------------------------------------------------------------------------

/// The OKLab distance between two theme colours.
fn oklab_apart(a: &str, b: &str) -> f64 {
    let lab = |hex: &str| {
        let [l, x, y] = tint::oklab_of(hex).expect("a colour");
        [l, x, y]
    };
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Every workspace colour there is to try: the palette's names, and a `#rrggbb` of every fifth
/// hue.
fn every_colour() -> Vec<String> {
    PALETTE
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .chain((0..72).map(|at| tint_hex("#c0392b", f64::from(at) * 5.0)))
        .collect()
}

#[test]
fn a_tinted_accent_stays_apart_from_danger_at_every_hue() {
    for theme in built_ins() {
        for colour in every_colour() {
            let tint = tinted(theme, &colour);
            let apart = oklab_apart(tint.value("accent.base"), tint.value("danger.base"));
            assert!(apart >= DANGER_APART, "{} {colour}: {apart}", theme.name);
        }
    }
}

#[test]
fn classic_keeps_today_s_tint_wherever_the_accent_was_already_apart_from_danger() {
    // Today's tint: every token of the recipe turned to the palette's own hue.
    let today = |theme: &ResolvedTheme, hue: f64| {
        let mut tint = theme.clone();
        for token in TINTED {
            let value = tint_hex(theme.value(token), hue);
            tint.values.insert(token, value);
        }
        tint
    };
    let mut held = Vec::new();
    for theme in built_ins() {
        for (colour, hue) in PALETTE {
            if tinted(theme, colour) != today(theme, hue) {
                held.push(format!("{} {colour}", theme.name));
            }
        }
    }
    // Only the light appearance's red, orange and pink brought the accent near danger.
    assert_eq!(
        held,
        [
            "purlis-light red",
            "purlis-light orange",
            "purlis-light pink"
        ]
    );
}

#[test]
fn a_held_colour_keeps_its_hue_and_gives_only_as_little_colour_as_it_must() {
    let light = built_in("purlis-light").unwrap();
    for (colour, at) in [("red", 25.0), ("orange", 55.0), ("pink", 350.0)] {
        let most = apart_chroma(light, at).expect("a chroma apart from danger");
        assert!(most.is_finite(), "{colour}");
        let accent = tinted(light, colour).value("accent.base").to_owned();
        assert!(apart(hue(&accent), at) < 6.0, "{colour}: {accent}");
        // One step more colour, and the accent would be nearer danger than the floor.
        let more = tint_hex_within(light.value("accent.base"), at, most + 0.005);
        assert!(
            oklab_apart(&more, light.value("danger.base")) < DANGER_APART,
            "{colour}"
        );
    }
    // A recipe that does not tint the accent has nothing to hold apart.
    let theme = drawn(&tinting(json!({ "tokens": ["surface.raised"] })));
    assert_eq!(apart_chroma(&theme, 25.0), Some(f64::INFINITY));
}

#[test]
fn the_accent_is_held_apart_exactly_as_the_window_holds_it() {
    // The shared fixture's `apart` cases: `most` is the chroma held to, `null` for no limit and
    // "none" for a colour no chroma holds apart; `tinted` the accent drawn.
    let cases: Value = serde_json::from_str(TINT_CASES).unwrap();
    let cases = cases["apart"].as_array().unwrap();
    assert!(cases.len() >= 40);
    // The path where no chroma keeps them apart, and the tint is dropped, is held too.
    assert!(cases.iter().any(|case| case["most"] == "none"));
    for case in cases {
        let field = |key: &str| case[key].as_str().unwrap();
        let hue = case["hue"].as_f64().unwrap();
        let most = apart_chroma_of(field("accent"), field("danger"), hue);
        let want = match &case["most"] {
            Value::Null => Some(f64::INFINITY),
            Value::String(none) if none == "none" => None,
            other => other.as_f64(),
        };
        assert_eq!(most, want, "{case}");
        let drawn = most.map(|most| tint_hex_within(field("accent"), hue, most));
        assert_eq!(drawn.as_deref(), case["tinted"].as_str(), "{case}");
    }
}

#[test]
fn a_theme_whose_accent_meets_danger_under_one_tint_is_drawn_with_them_apart_there() {
    // Danger drawn as the accent turned to red: apart as written, the same under a red tint.
    let accent = default_theme().value("accent.base").to_owned();
    let danger = tint_hex(&accent, 25.0);
    let theme = drawn(&json!({
        "name": "Close",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "danger.base": danger }
        }]
    }));
    assert!(oklab_apart(&accent, &danger) >= DANGER_APART);
    assert!(oklab_apart(&tint_hex(&accent, 25.0), &danger) < DANGER_APART);
    let custom = tint_hex("#c0392b", 25.0);
    for colour in ["red", custom.as_str()] {
        let tint = tinted(&theme, colour);
        let apart = oklab_apart(tint.value("accent.base"), tint.value("danger.base"));
        assert!(apart >= DANGER_APART, "{colour}: {apart}");
        // Still a tint of the colour, not the theme as it is.
        assert_ne!(tint.value("accent.base"), accent, "{colour}");
    }
}

#[test]
fn a_colour_no_amount_of_chroma_keeps_apart_from_danger_is_drawn_untinted() {
    // The accent and danger one grey: every tint of the accent is within the floor of it.
    let theme = drawn(&json!({
        "name": "Grey",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "accent.base": "#777777", "danger.base": "#777777" }
        }]
    }));
    assert_eq!(apart_chroma(&theme, 25.0), None);
    assert_eq!(tinted(&theme, "red"), theme);
}
