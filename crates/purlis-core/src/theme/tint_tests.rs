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
                "accent.fill",
                "accent.fill-hover",
                "accent.fill-edge",
                "focus.ring",
                "tab.active",
                "list.selected",
                "list.selected-edge",
                "markdown.link",
                "chart.1"
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
    let handed = serde_json::to_value(&built_in_for(Appearance::Dark).tint).unwrap();
    assert_eq!(handed, cases["classic"]);
}

#[test]
fn a_theme_that_says_nothing_of_tint_has_classic_s_recipe() {
    let format_2 = drawn(&json!({
        "name": "Plain",
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    assert_eq!(format_2.tint, built_in_for(Appearance::Dark).tint);
    let format_1 = drawn(&json!({ "name": "Old", "tokens": {} }));
    assert_eq!(format_1.tint, built_in_for(Appearance::Dark).tint);
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
        assert!(
            !built_in_for(Appearance::Dark).tint.window().contains(token),
            "{token}"
        );
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
        tinted(built_in_for(Appearance::Dark), "teal").values
    );
    // And a workspace's own `#rrggbb` is its own hue whatever the theme says.
    assert_eq!(
        tinted(&theme, "#c0392b").values,
        tinted(built_in_for(Appearance::Dark), "#c0392b").values
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
            // Said of the theme as written, never of a tint.
            Finding::SelectionTextAlone
            | Finding::LineDoesNotFit { .. }
            | Finding::StyleWithoutAFace { .. }
            | Finding::PartFallsBack { .. }
            | Finding::TierFallsBack { .. } => None,
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
    // stood on its floor and the channels' rounding took a hundredth or two off. The check
    // says so.
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
            assert!(pair.floor - ratio <= 0.021, "{appearance}: {finding:?}");
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
                held.push(format!("{} {colour}", theme.id));
            }
        }
    }
    // The light appearance's red, orange and pink brought the accent near danger, and each
    // appearance's orange and yellow bring it near the waiting amber (#3130).
    assert_eq!(
        held,
        [
            "dark orange",
            "dark yellow",
            "light red",
            "light orange",
            "light yellow",
            "light pink"
        ]
    );
}

#[test]
fn a_held_colour_keeps_its_hue_and_gives_only_as_little_colour_as_it_must() {
    let light = classic().of(Appearance::Light).unwrap();
    // Red and orange are left out: there the accent gives way to the waiting amber too (#3130).
    {
        let (colour, at) = ("pink", 350.0);
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
        // It is the danger rule that binds: alone it asks the same, and the Keep blocked rule
        // alone asks for nothing.
        let rule = |from: &str| Rule {
            token: light.value("accent.base"),
            token_tinted: true,
            from: light.value(from),
            from_tinted: false,
            distance: 0.1,
        };
        assert_eq!(
            apart_chroma_of(&[rule("danger.base")], at),
            Some(most),
            "{colour}"
        );
        assert_eq!(
            apart_chroma_of(&[rule("border.control")], at),
            Some(f64::INFINITY),
            "{colour}"
        );
    }
    // A recipe that tints neither side of a rule has nothing to hold apart.
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
    // The path where no chroma keeps them apart, and the tint is dropped, is held too, and so
    // is a rule whose other colour the tint turns as well (Keep blocked's edge).
    assert!(cases.iter().any(|case| case["most"] == "none"));
    assert!(
        cases
            .iter()
            .any(|case| case["rules"].as_array().unwrap().len() == 2)
    );
    for case in cases {
        let field = |key: &str| case[key].as_str().unwrap();
        let hue = case["hue"].as_f64().unwrap();
        let rules: Vec<Rule<'_>> = case["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rule| Rule {
                // A rule may name a token of its own beside the accent: one of the accent's kin.
                token: rule["token"].as_str().unwrap_or(field("accent")),
                token_tinted: rule["token_tinted"]
                    .as_bool()
                    .unwrap_or(case["accent_tinted"].as_bool().unwrap()),
                from: rule["from"].as_str().unwrap(),
                from_tinted: rule["tinted"].as_bool().unwrap(),
                distance: rule["distance"].as_f64().unwrap(),
            })
            .collect();
        let most = apart_chroma_of(&rules, hue);
        let want = match &case["most"] {
            Value::Null => Some(f64::INFINITY),
            Value::String(none) if none == "none" => None,
            other => other.as_f64(),
        };
        assert_eq!(most, want, "{case}");
        let drawn = most.map(|most| {
            if case["accent_tinted"].as_bool().unwrap() {
                tint_hex_within(field("accent"), hue, most)
            } else {
                field("accent").to_owned()
            }
        });
        assert_eq!(drawn.as_deref(), case["tinted"].as_str(), "{case}");
    }
}

#[test]
fn a_theme_whose_accent_meets_danger_under_one_tint_is_drawn_with_them_apart_there() {
    // Danger drawn as the accent turned to red: apart as written, the same under a red tint.
    let accent = built_in_for(Appearance::Dark)
        .value("accent.base")
        .to_owned();
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
fn a_colour_no_amount_of_chroma_keeps_apart_from_danger_leaves_the_rules_sides_untinted() {
    // The accent and danger one grey: every tint of the accent is within the floor of it.
    let theme = drawn(&json!({
        "name": "Grey",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "accent.base": "#777777", "danger.base": "#777777" }
        }]
    }));
    assert_eq!(apart_chroma(&theme, 25.0), None);
    let tint = tinted(&theme, "red");
    // The sides of a rule are drawn as written; the strip's shades still take red (#3034).
    for token in theme.tint.tabs() {
        if held_apart(token) {
            assert_eq!(tint.value(token), theme.value(token), "{token}");
        }
    }
    let red = theme.tint.hue("red").expect("red");
    assert!(apart(hue(tint.value("layer.chat")), red) < 6.0);
}

#[test]
fn a_tint_holds_only_the_sides_of_a_rule_as_the_shared_fixture_says() {
    // The shared fixture's `held`: each token the recipe turns, and whether a tint gives it less
    // colour to keep the rules (#3034). `tint.ts`' `chromaFor` reads the same.
    let cases: Value = serde_json::from_str(TINT_CASES).unwrap();
    for case in cases["held"].as_array().unwrap() {
        let token = case["token"].as_str().unwrap();
        let held = case["held"].as_bool().unwrap();
        assert_eq!(held_apart(token), held, "{token}");
        let want = if held {
            Some(0.01)
        } else {
            Some(f64::INFINITY)
        };
        assert_eq!(chroma_for(token, Some(0.01)), want, "{token}");
    }
}

/// A theme whose accent is read against its controls from its own side of the middle, a dark
/// accent on darker controls, sitting on the floor of 3: rounding the accent away from the middle
/// takes it nearer the controls in a tinted workspace.
fn accent_on_its_own_side(on: bool) -> ResolvedTheme {
    drawn(&json!({
        "name": "Dim",
        "tint": { "on": on, "tokens": ["accent.base"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "accent.base": "#3365e6", "control.hover": "#262623" }
        }]
    }))
}

/// Every consent pair of `tint` at or above its floor.
fn floors_hold(tint: &ResolvedTheme) -> bool {
    let pairs = floors::pairs()
        .all(|(_, pair)| check::ratio(tint, pair).is_none_or(|ratio| ratio >= pair.floor));
    pairs && rules_hold(tint)
}

/// Every rule of [`APART`] held in `tint`.
fn rules_hold(tint: &ResolvedTheme) -> bool {
    APART
        .iter()
        .all(|rule| oklab_apart(tint.value(rule.token), tint.value(rule.from)) >= rule.distance)
}

/// The consent floors `theme` is drawn with, but for Allow against the primary (#2948): a
/// recipe that turns the controls or the primary's fills can bring the two together in a tint,
/// which that floor holds on its own (`floors/allow_primary_tests.rs`).
fn held_but_allow_from_primary(theme: &ResolvedTheme) -> Option<Held> {
    draw(theme)
        .held
        .filter(|held| held.floors != [Floor::AllowNotPrimary] || !held.tokens.is_empty())
}

/// Every colour a workspace can be tinted by, to sweep: the palette's names, and a hue of every
/// whole degree.
fn every_hue(theme: &ResolvedTheme) -> impl Iterator<Item = ResolvedTheme> + '_ {
    let named = PALETTE.iter().map(|(name, _)| tinted(theme, name));
    named.chain((0..360).map(|degree| tint::tinted_at(theme, f64::from(degree))))
}

#[test]
fn a_theme_on_the_danger_floor_keeps_it_under_every_tint_of_its_controls() {
    // Classic dark's danger colour on its control sits on the floor of 4.5. A tint keeps each
    // shade's luminance on its own side of the middle, so a recipe that turns the controls holds
    // the floor at every hue: the theme is drawn as written, with its tint.
    let controls = drawn(&json!({
        "name": "Controls",
        "tint": { "tokens": ["control.base", "control.hover", "accent.base"] },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    let on_the_floor = PAIRS
        .iter()
        .find(|pair| pair.front == "danger.base" && pair.back == "control.base")
        .expect("the danger colour is held on a control");
    assert_eq!(check::ratio(&controls, on_the_floor), Some(4.5));
    assert_eq!(held_but_allow_from_primary(&controls), None);
    for tint in every_hue(&controls) {
        assert!(floors_hold(&tint), "{:?}", tint.values.get("control.base"));
    }
}

#[test]
fn a_shade_a_tint_would_bring_under_a_floor_is_drawn_untinted_and_the_theme_as_written() {
    // As written the dim accent clears its floor on the controls; in a tinted workspace it would
    // fall a hundredth under it. The floors measure the pair at every whole degree, and purlis
    // draws the accent untinted there, the rest of the theme as written.
    let theme = accent_on_its_own_side(true);
    let pair = PAIRS
        .iter()
        .find(|pair| pair.front == "accent.base" && pair.back == "control.hover")
        .expect("the accent is held on a control");
    assert_eq!(check::ratio(&theme, pair), Some(3.0));
    assert!(every_hue(&theme).any(|tint| !floors_hold(&tint)));
    let on = draw(&theme);
    let held = on.held.clone().expect("the floor was held");
    assert_eq!(held.tokens, Vec::<&str>::new());
    assert_eq!(held.untinted, ["accent.base"]);
    assert!(
        held.said()
            .starts_with("In a tinted workspace, Dim would fall"),
        "{}",
        held.said()
    );
    let shown = on.theme.expect("a theme is always drawn");
    assert_eq!(shown.values, theme.values);
    assert!(!shown.tint.tabs().contains(&"accent.base"));
    for tint in every_hue(&shown) {
        assert!(floors_hold(&tint));
    }
    // With its tint off, the theme is only ever drawn as it is, and clears the floors so.
    assert_eq!(draw(&accent_on_its_own_side(false)).held, None);
}

#[test]
fn the_notice_says_what_purlis_draws_its_own_and_what_it_draws_untinted() {
    let held = |tokens: Vec<&'static str>, untinted: Vec<&'static str>| Held {
        theme: "Controls".to_owned(),
        floors: vec![Floor::Danger],
        tokens,
        untinted,
    };
    assert_eq!(
        held(vec!["danger.base", "danger.text"], vec![]).said(),
        "Controls breaks the consent floors (the danger colours contrast with what they are \
         drawn on), so purlis draws its own danger.base, danger.text; the rest of the theme \
         draws as written"
    );
    assert_eq!(
        held(vec![], vec!["control.base"]).said(),
        "In a tinted workspace, Controls would fall under the consent floors (the danger colours \
         contrast with what they are drawn on), so purlis draws control.base untinted there to \
         keep its words readable; the rest of the theme draws as written"
    );
    assert_eq!(
        held(vec!["danger.base"], vec!["control.base", "control.hover"]).said(),
        "Controls breaks the consent floors (the danger colours contrast with what they are \
         drawn on), so purlis draws its own danger.base, and control.base, control.hover \
         untinted in tinted workspaces; the rest of the theme draws as written"
    );
}

#[test]
fn an_option_s_tokens_are_tinted_as_the_recipe_says() {
    // An option that sets the accent and the raised surface: the recipe tints the accent and not
    // the surface, so a workspace's colour turns the option's accent and leaves its surface.
    let loaded = load(&json!({
        "name": "Chosen",
        "appearances": [{ "id": "dark", "appearance": "dark" }],
        "options": [{
            "id": "bold", "kind": "toggle", "label": "Bold", "default": false,
            "on": { "tokens": { "accent.base": "#2bb3a3", "surface.raised": "#2a2a33" } }
        }]
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let values: Values = [("bold".to_owned(), "on".to_owned())].into_iter().collect();
    let (on, complaints) = loaded.theme.with(&values);
    assert!(complaints.is_empty(), "{complaints:?}");
    let drawn = on.first();
    assert_eq!(drawn.tint, built_in_for(Appearance::Dark).tint);
    let tint = tinted(drawn, "red");
    let most = apart_chroma(drawn, 25.0).expect("apart");
    assert_eq!(
        tint.value("accent.base"),
        tint_hex_within("#2bb3a3", 25.0, most)
    );
    assert_eq!(tint.value("surface.raised"), "#2a2a33");
}

#[test]
fn a_value_left_to_the_platform_is_never_tinted() {
    // No token a theme may leave to the platform is one a tint may name, and the arithmetic
    // answers a value that is not a colour as it is.
    for token in vocabulary::PLATFORM_DRAWN {
        assert!(!vocabulary::TINTABLE.contains(token), "{token}");
    }
    assert_eq!(tint_hex(vocabulary::PLATFORM, 25.0), vocabulary::PLATFORM);
}

// ---------------------------------------------------------------------------------------
// Allow is held apart from Keep blocked under the theme's own tint
// ---------------------------------------------------------------------------------------

#[test]
fn allow_s_edge_is_held_apart_from_keep_blocked_s_under_the_theme_s_own_tint() {
    // Keep blocked's edge written as the accent turned red: apart from Allow's as written, and
    // one colour under any tint that turned both alike. The tint holds the two apart itself, so
    // the theme is drawn as written, and the edges stay apart at every hue of its recipe.
    let accent = built_in_for(Appearance::Dark)
        .value("accent.base")
        .to_owned();
    let border = tint_hex(&accent, 25.0);
    let edges = drawn(&json!({
        "name": "Edges",
        "tint": { "tokens": ["accent.base", "border.control"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "border.control": border }
        }]
    }));
    assert!(oklab_apart(&accent, &border) >= ALLOW_APART);
    // Turned alike, the two would be one colour.
    assert!(oklab_apart(&tint_hex(&accent, 255.0), &tint_hex(&border, 255.0)) < ALLOW_APART);
    let shown = draw(&edges);
    assert_eq!(shown.held, None);
    let shown = shown.theme.expect("a theme is always drawn");
    for one in &shown.tint.hues {
        let tint = tinted(&shown, one.name);
        let apart = oklab_apart(tint.value("accent.base"), tint.value("border.control"));
        assert!(apart >= ALLOW_APART, "{}: {apart}", one.name);
    }
}

#[test]
fn allow_s_edge_is_measured_on_its_ground_after_both_are_tinted() {
    // A recipe that turns the accent and both of the control's grounds: Allow's edge clears its
    // floor on each, at every hue, as the floors measure it.
    let theme = drawn(&json!({
        "name": "Grounds",
        "tint": { "tokens": ["accent.base", "control.base", "control.hover"] },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    }));
    let shown = draw(&theme).theme.expect("a theme is always drawn");
    let edges: Vec<&Pair> = PAIRS
        .iter()
        .filter(|pair| {
            pair.front == "accent.base" && matches!(pair.back, "control.base" | "control.hover")
        })
        .collect();
    assert_eq!(edges.len(), 2);
    for one in &shown.tint.hues {
        let tint = tinted(&shown, one.name);
        for pair in &edges {
            let ratio = check::ratio(&tint, pair).expect("both are colours");
            assert!(ratio >= pair.floor, "{} {}: {ratio}", one.name, pair.back);
        }
    }
}

/// Whether `tint` keeps Allow's edge apart from Keep blocked's and the accent apart from
/// danger, as the window draws it.
fn both_apart(tint: &ResolvedTheme) -> bool {
    oklab_apart(tint.value("accent.base"), tint.value("danger.base")) >= DANGER_APART
        && oklab_apart(tint.value("accent.base"), tint.value("border.control")) >= ALLOW_APART
}

/// A theme whose Keep blocked edge is apart from Allow's as written, and meets it under any
/// tint that turns both: the same luminance, half the colour, another hue.
fn edges_meet_under_tint() -> ResolvedTheme {
    let accent = built_in_for(Appearance::Dark)
        .value("accent.base")
        .to_owned();
    drawn(&json!({
        "name": "Edges",
        "tint": { "tokens": ["accent.base", "border.control"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "border.control": tint_hex_within(&accent, 25.0, 0.05) }
        }]
    }))
}

/// The same Keep blocked edge, with a recipe that turns it alone and leaves the accent as it is:
/// at the accent's own hue the two would meet.
fn only_keep_blocked_s_edge_tinted() -> ResolvedTheme {
    let accent = built_in_for(Appearance::Dark)
        .value("accent.base")
        .to_owned();
    drawn(&json!({
        "name": "Edge",
        "tint": { "tokens": ["border.control"] },
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "border.control": tint_hex_within(&accent, 25.0, 0.05) }
        }]
    }))
}

#[test]
fn every_rule_holds_when_a_theme_tints_only_keep_blocked_s_edge() {
    let theme = only_keep_blocked_s_edge_tinted();
    assert!(both_apart(&theme));
    let accent = theme.value("accent.base");
    let hue = hue_of(&accent[..7]).expect("a hue");
    // Turned to the accent's own hue, the edge would be the accent with less colour.
    assert!(oklab_apart(accent, &tint_hex(theme.value("border.control"), hue)) < ALLOW_APART);
    let mut turned = 0;
    for degree in 0..360 {
        let colour = tint_hex("#c0392b", f64::from(degree));
        let tint = tinted(&theme, &colour);
        // The accent is the theme's at every hue: only the edge turns.
        assert_eq!(tint.value("accent.base"), accent, "{degree}°");
        assert!(both_apart(&tint), "{degree}° ({colour})");
        if tint.value("border.control") != theme.value("border.control") {
            turned += 1;
        }
    }
    // It is a tint still: the edge turns at most hues.
    assert!(turned > 300, "{turned}");
}

#[test]
fn allow_and_keep_blocked_stay_apart_at_a_custom_colour_between_two_named_ones() {
    let theme = edges_meet_under_tint();
    assert!(both_apart(&theme));
    // A workspace's own colour at about 220°, between teal and blue.
    let custom = tint_hex("#c0392b", 220.0);
    let hue = hue_of(&custom).expect("a hue");
    assert!(hue > 195.0 && hue < 250.0, "{hue}");
    assert!(both_apart(&tinted(&theme, &custom)), "{custom}");
}

#[test]
fn both_apart_rules_hold_at_every_whole_degree_of_a_custom_colour() {
    let themes = [
        classic().first().clone(),
        classic().of(Appearance::Light).unwrap().clone(),
        edges_meet_under_tint(),
        only_keep_blocked_s_edge_tinted(),
    ];
    for theme in &themes {
        for degree in 0..360 {
            let colour = tint_hex("#c0392b", f64::from(degree));
            assert!(
                both_apart(&tinted(theme, &colour)),
                "{} at {degree}° ({colour})",
                theme.name
            );
        }
    }
}

#[test]
fn the_floors_hold_allow_apart_from_keep_blocked_through_each_tint_and_with_it_off() {
    // Keep blocked's edge written as Allow's: the floors find it in their pass over the theme
    // and its tints, draw both edges as purlis's, and those stay apart under every tint. The
    // same theme with its tint off is held the same way: the floor is the theme's, not its tint's.
    let accent = built_in_for(Appearance::Dark)
        .value("accent.base")
        .to_owned();
    let same = |on: bool| {
        drawn(&json!({
            "name": "Same",
            "tint": { "on": on, "tokens": ["accent.base", "border.control"] },
            "appearances": [{
                "id": "dark", "appearance": "dark",
                "tokens": { "accent.base": "#2bb3a3", "border.control": "#2bb3a3" }
            }]
        }))
    };
    let on = draw(&same(true));
    let held = on.held.clone().expect("the floor was held");
    assert!(held.floors.contains(&Floor::AllowApart), "{held:?}");
    assert!(held.tokens.contains(&"accent.base"), "{held:?}");
    let shown = on.theme.expect("a theme is always drawn");
    assert_eq!(shown.value("accent.base"), accent);
    for one in &shown.tint.hues {
        assert!(both_apart(&tinted(&shown, one.name)), "{}", one.name);
    }
    let off = draw(&same(false))
        .held
        .expect("the floor was held with the tint off too");
    assert_eq!(off.floors, held.floors);
    assert!(off.untinted.is_empty());
}

// ---------------------------------------------------------------------------------------
// The consent floors hold exactly under every tint (#2526)
// ---------------------------------------------------------------------------------------

/// WCAG's relative luminance of a `#rrggbb`.
fn luminance(hex: &str) -> f64 {
    let channel = |at: usize| {
        let v = f64::from(u8::from_str_radix(&hex[at..at + 2], 16).unwrap()) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
}

#[test]
fn a_tinted_shade_keeps_its_luminance_on_its_own_side_at_every_whole_degree() {
    // Where black and white contrast with a colour alike: a darker shade is never rounded
    // lighter than it was, and a lighter one never darker, so it contrasts at least as much with
    // whatever it is read against across the middle.
    let middle = (1.05_f64 * 0.05).sqrt() - 0.05;
    for theme in built_ins() {
        for token in vocabulary::TINTABLE {
            let value = theme.value(token);
            if !value.starts_with('#') {
                continue;
            }
            let was = luminance(&value[..7]);
            for degree in 0..360 {
                let now = luminance(&tint_hex(value, f64::from(degree))[..7]);
                let kept = if was >= middle {
                    now >= was
                } else {
                    now <= was
                };
                assert!(kept, "{} {token} {value} at {degree}°", theme.name);
            }
        }
    }
}

#[test]
fn every_tint_a_theme_may_name_keeps_classic_s_floors_exactly_at_every_hue() {
    // Every token a theme may tint, tinted at once on both of Classic's appearances: every pair
    // the check holds clears its floor at each palette hue and at every whole degree of a
    // workspace's own colour, as it does untinted.
    for appearance in ["dark", "light"] {
        let theme = drawn(&json!({
            "name": "Everything",
            "tint": { "tokens": vocabulary::TINTABLE },
            "appearances": [{ "id": appearance, "appearance": appearance }]
        }));
        assert_eq!(check(&theme).findings, [], "{appearance}");
        assert_eq!(held_but_allow_from_primary(&theme), None, "{appearance}");
        for tint in every_hue(&theme) {
            for pair in PAIRS {
                if let Some(ratio) = check::ratio(&tint, pair) {
                    assert!(ratio >= pair.floor, "{appearance}: {pair:?} {ratio}");
                }
            }
            assert!(rules_hold(&tint), "{appearance}");
        }
    }
}

#[test]
fn purlis_classic_s_tints_are_these() {
    // Classic's tint of each token its recipe turns, at each palette hue (D-2329-8 held the
    // accent of light red, orange and pink apart from danger). Since #2526 each channel is
    // rounded toward the luminance the shade had, on its own side of the middle, rather than to
    // the nearest byte: most of these moved by one step of one or two channels, and light red
    // and pink's accent by two steps of red, where that rounding let the accent keep one more
    // (red) or one less (pink) step of colour apart from danger. The operator ruled for the
    // rounding on 2026-10-19. Since #3034 only the tokens a rule holds apart from danger are
    // given less colour: light red, orange and pink's link and first chart series, which no rule
    // measures, take the colour's full hue. Since #3130 the accent and the edges drawn in it
    // give way alone to the waiting amber: dark orange and yellow and light red, orange and
    // yellow draw them with less colour, and their washes keep the colour danger leaves them.
    const CLASSIC: [(&str, &str, [&str; 14]); 16] = [
        (
            "purlis-dark",
            "red",
            [
                "#f69890", "#3b2625", "#3b2625", "#3f2c2b", "#f69890", "#f69890", "#3f2c2b",
                "#36211f", "#f69890", "#f69890", "#f69890", "#261413", "#2a1817", "#382523",
            ],
        ),
        (
            "purlis-dark",
            "orange",
            [
                "#cfaa91", "#3a271c", "#3a271c", "#3e2d24", "#cfaa91", "#cfaa91", "#3e2d24",
                "#352217", "#cfaa91", "#eca06b", "#eca06b", "#25150c", "#291910", "#37261c",
            ],
        ),
        (
            "purlis-dark",
            "yellow",
            [
                "#bab28d", "#312b18", "#312b18", "#363020", "#bab28d", "#bab28d", "#363020",
                "#2c2613", "#bab28d", "#c7b156", "#c7b156", "#1e1808", "#221c0c", "#2f2919",
            ],
        ),
        (
            "purlis-dark",
            "green",
            [
                "#7ec180", "#212f20", "#212f20", "#273427", "#7ec180", "#7ec180", "#273427",
                "#1b2a1c", "#7ec180", "#7ec180", "#7ec180", "#101b10", "#132013", "#202d1f",
            ],
        ),
        (
            "purlis-dark",
            "teal",
            [
                "#3ec4bc", "#14302e", "#14302e", "#1d3533", "#3ec4bc", "#3ec4bc", "#1d3533",
                "#0f2a2a", "#3ec4bc", "#3ec4bc", "#3ec4bc", "#051c1c", "#092020", "#172d2d",
            ],
        ),
        (
            "purlis-dark",
            "blue",
            [
                "#7fb5f7", "#212c3c", "#212c3c", "#273140", "#7fb5f7", "#7fb5f7", "#273140",
                "#1b2738", "#7fb5f7", "#7fb5f7", "#7fb5f7", "#101927", "#141d2b", "#202a39",
            ],
        ),
        (
            "purlis-dark",
            "purple",
            [
                "#c6a2ef", "#30283a", "#30283a", "#352e3d", "#c6a2ef", "#c6a2ef", "#352e3d",
                "#2b2335", "#c6a2ef", "#c6a2ef", "#c6a2ef", "#1d1624", "#201a2a", "#2d2737",
            ],
        ),
        (
            "purlis-dark",
            "pink",
            [
                "#ec98c0", "#39262f", "#39262f", "#3d2c34", "#ec98c0", "#ec98c0", "#3d2c34",
                "#342129", "#ec98c0", "#ec98c0", "#ec98c0", "#24141c", "#281820", "#37242d",
            ],
        ),
        (
            "purlis-light",
            "red",
            [
                "#5e5d5e", "#fde8e5", "#fde8e5", "#f9dbd6", "#5e5d5e", "#5e5d5e", "#f9dbd6",
                "#fde1df", "#5e5d5e", "#8b4b47", "#8b4b47", "#feeae7", "#fef4f2", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "orange",
            [
                "#5e5d5e", "#fbe9df", "#fbe9df", "#f5ddcc", "#5e5d5e", "#5e5d5e", "#f5ddcc",
                "#f8e4d5", "#5e5d5e", "#86502b", "#86502b", "#feebdd", "#fff4ee", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "yellow",
            [
                "#5e5d5e", "#f1edda", "#f1edda", "#e8e2c7", "#5e5d5e", "#5e5d5e", "#e8e2c7",
                "#ede8d1", "#5e5d5e", "#6d5c18", "#6d5c18", "#f5eed8", "#fbf7df", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "green",
            [
                "#39673c", "#e3f0e1", "#e3f0e1", "#d2e7d2", "#39673c", "#39673c", "#d2e7d2",
                "#dbecda", "#39673c", "#39673c", "#39673c", "#e1f3e1", "#e9fbe8", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "teal",
            [
                "#016964", "#dbf1ee", "#dbf1ee", "#c7e8e4", "#016964", "#016964", "#c7e8e4",
                "#d1edea", "#016964", "#016964", "#016964", "#d7f4f2", "#dffcf9", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "blue",
            [
                "#395f8e", "#e1eefc", "#e1eefc", "#d2e3f8", "#395f8e", "#395f8e", "#d2e3f8",
                "#dae9fb", "#395f8e", "#395f8e", "#395f8e", "#e3f0fe", "#f0f7fe", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "purple",
            [
                "#6c5288", "#f0eafa", "#f0eafa", "#e8ddf5", "#6c5288", "#6c5288", "#e8ddf5",
                "#ece4f8", "#6c5288", "#6c5288", "#6c5288", "#f3ebfe", "#f8f5fe", "#ffffff",
            ],
        ),
        (
            "purlis-light",
            "pink",
            [
                "#804e67", "#fae8ef", "#fae8ef", "#f6dae6", "#804e67", "#804e67", "#f6dae6",
                "#f9e1ec", "#804e67", "#854b68", "#854b68", "#fee9f1", "#fff3f8", "#ffffff",
            ],
        ),
    ];
    for (name, colour, values) in CLASSIC {
        // Classic's appearances, by the ids its tints were first pinned under.
        let theme = if name == "purlis-dark" {
            classic().first()
        } else {
            classic().of(Appearance::Light).unwrap()
        };
        let tint = tinted(theme, colour);
        let drawn: Vec<&str> = TINTED.iter().map(|token| tint.value(token)).collect();
        assert_eq!(drawn, values, "{name} {colour}");
    }
}

/// A dark theme whose `token` is the danger colour turned to teal: apart from danger as written,
/// and nearly danger itself under a red tint that turned it whole.
fn kin_near_danger_under_red(token: &str) -> ResolvedTheme {
    let danger = built_in_for(Appearance::Dark).value("danger.base");
    drawn(&json!({
        "name": "Kin",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { token: tint_hex(danger, 190.0) }
        }]
    }))
}

#[test]
fn every_accent_hued_token_beside_danger_stays_apart_from_it_at_every_whole_degree() {
    for token in ACCENT_KIN {
        let theme = kin_near_danger_under_red(token);
        let danger = theme.value("danger.base");
        assert!(
            oklab_apart(theme.value(token), danger) >= DANGER_APART,
            "{token}"
        );
        assert!(
            oklab_apart(&tint_hex(theme.value(token), 25.0), danger) < DANGER_APART,
            "{token}"
        );
        assert_eq!(held_but_allow_from_primary(&theme), None, "{token}");
        for tint in every_hue(&theme) {
            let apart = oklab_apart(tint.value(token), tint.value("danger.base"));
            assert!(apart >= DANGER_APART, "{token}: {apart}");
            assert!(rules_hold(&tint), "{token}");
        }
        // Still a tint where one keeps it apart.
        assert_ne!(
            tinted(&theme, "blue").value(token),
            theme.value(token),
            "{token}"
        );
    }
}

#[test]
fn the_floors_measure_danger_apart_from_the_accent_and_its_kin_as_written_and_per_tint() {
    // The focus ring written as the danger colour: the floors find danger and primary one colour,
    // with the tint on and off, and draw the ring as purlis's. Under every tint the drawn theme
    // keeps danger apart from the accent and from each token beside it.
    let ring = |on: bool| {
        drawn(&json!({
            "name": "Ring",
            "tint": { "on": on },
            "appearances": [{
                "id": "dark", "appearance": "dark",
                "tokens": { "focus.ring": built_in_for(Appearance::Dark).value("danger.base") }
            }]
        }))
    };
    for on in [true, false] {
        let shown = draw(&ring(on));
        let held = shown.held.expect("the floor was held");
        assert_eq!(held.floors, [Floor::DangerApart], "{on}");
        assert_eq!(held.tokens, ["focus.ring"], "{on}");
        let shown = shown.theme.expect("a theme is always drawn");
        assert_eq!(
            shown.value("focus.ring"),
            built_in_for(Appearance::Dark).value("focus.ring")
        );
        for one in &shown.tint.hues {
            assert!(rules_hold(&tinted(&shown, one.name)), "{on} {}", one.name);
        }
    }
    // Classic itself is drawn as it is.
    for theme in built_ins() {
        assert_eq!(draw(theme).held, None, "{}", theme.name);
    }
}

#[test]
fn a_grey_accent_keeps_a_tint_apart_from_danger_at_every_whole_degree() {
    // A grey accent is given a little colour, and where that would bring it near danger, as much
    // of it as keeps them apart rather than none.
    let theme = drawn(&json!({
        "name": "Grey",
        "appearances": [{
            "id": "dark", "appearance": "dark",
            "tokens": { "accent.base": "#808080", "focus.ring": "#808080", "list.selected-edge": "#808080" }
        }]
    }));
    let danger = theme.value("danger.base");
    assert!(oklab_apart("#808080", danger) >= DANGER_APART);
    assert!(oklab_apart(&tint_hex("#808080", 25.0), danger) < DANGER_APART);
    let most = apart_chroma(&theme, 25.0).expect("a chroma apart from danger");
    assert!(most > 0.0 && most < 0.03, "{most}");
    for degree in 0..360 {
        let tint = tint::tinted_at(&theme, f64::from(degree));
        let accent = tint.value("accent.base");
        assert_ne!(accent, "#808080", "{degree}°");
        assert!(rules_hold(&tint), "{degree}°: {accent}");
    }
}

#[test]
fn a_see_through_front_on_a_tinted_ground_keeps_its_floor_at_every_whole_degree() {
    // An ask's words written see-through over a dialog the recipe tints: the screen mixes the two
    // channel by channel, so the pair's contrast moves with the hue. The floors measure it at every
    // whole degree, between the palette's hues too, and purlis draws the ground untinted where a
    // tint would bring the words under their floor, the words as written.
    let veil = |on: bool| {
        drawn(&json!({
            "name": "Veil",
            "tint": { "on": on, "tokens": ["surface.overlay"] },
            "appearances": [{
                "id": "dark", "appearance": "dark",
                "tokens": { "text.primary": "#f5f3ee9b" }
            }]
        }))
    };
    let theme = veil(true);
    assert!(floors_hold(&theme));
    assert!(
        PALETTE
            .iter()
            .all(|(name, _)| floors_hold(&tinted(&theme, name)))
    );
    assert!(every_hue(&theme).any(|tint| !floors_hold(&tint)));
    let on = draw(&theme);
    let held = on.held.expect("the floor was held");
    assert_eq!(held.tokens, Vec::<&str>::new());
    assert_eq!(held.untinted, ["surface.overlay"]);
    let shown = on.theme.expect("a theme is always drawn");
    assert_eq!(shown.values, theme.values);
    for tint in every_hue(&shown) {
        assert!(floors_hold(&tint));
    }
    assert_eq!(draw(&veil(false)).held, None);
}
