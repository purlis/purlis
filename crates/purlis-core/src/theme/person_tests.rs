// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's layer beyond density and fonts (#2925): reduced motion, the minimum contrast and
//! their own colours, read from the layout file and drawn over any theme, held to the floors.

use serde_json::{Value, json};

use super::check::{self as measure, ansi_floor};
use super::*;

fn layer_of(raw: Value) -> Layer {
    let (person, complaints) = Person::read(Some(&raw));
    assert!(complaints.is_empty(), "{complaints:?}");
    person.layer()
}

fn said(raw: Value) -> Vec<String> {
    Person::read(Some(&raw))
        .1
        .into_iter()
        .map(|one| one.said)
        .collect()
}

/// What the window draws of `theme` with `layer` over it.
fn drawn_under(theme: &ResolvedTheme, layer: &Layer) -> Drawn {
    draw(theme).under(layer)
}

fn classic_dark() -> &'static ResolvedTheme {
    built_in_for(Appearance::Dark)
}

fn purlis_dark() -> &'static ResolvedTheme {
    purlis().first()
}

// ---------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------

#[test]
fn reduced_motion_more_contrast_and_colours_are_read_from_the_layout_file() {
    let layer = layer_of(json!({
        "motion": "reduced",
        "contrast": "more",
        "colours": { "accent.base": "#8F7BFF", "text.muted": "" }
    }));
    assert!(layer.reduce_motion);
    assert_eq!(layer.contrast, Least::More);
    // A colour is kept as the engine writes it, and an empty field is the theme's.
    assert_eq!(
        layer.colours.into_iter().collect::<Vec<_>>(),
        vec![("accent.base", "#8f7bff".to_owned())]
    );
}

#[test]
fn nothing_set_follows_the_system_at_the_standard_contrast_in_the_theme_s_colours() {
    let layer = layer_of(json!({ "motion": "system", "contrast": "standard", "colours": {} }));
    assert_eq!(layer, Layer::default());
    assert!(!layer.reduce_motion);
    assert_eq!(layer.contrast, Least::Standard);
    assert!(!layer.colours_anything());
}

#[test]
fn a_motion_contrast_or_colour_the_engine_cannot_use_is_the_default_and_said() {
    let (person, _) = Person::read(Some(&json!({
        "motion": "off",
        "contrast": 7,
        "colours": { "accent.base": "red; } body { color: red", "brand.hue": "#fff" }
    })));
    assert_eq!(person, Person::default());
    assert_eq!(
        said(json!({
            "motion": "off",
            "contrast": 7,
            "colours": { "accent.base": "red; } body { color: red", "brand.hue": "#fff" }
        })),
        [
            "motion is \"off\", which is \"system\" or \"reduced\", so it follows the system",
            "contrast is 7, which is \"standard\" or \"more\", so it is standard",
            "your colour for accent.base, \"red; } body { color: red\", is not #rgb, #rgba, \
             #rrggbb or #rrggbbaa, so the theme's is drawn",
            "brand.hue is not a colour token of the theme vocabulary, so it is left out",
        ]
    );
    assert_eq!(
        said(json!({ "colours": ["#fff"] })),
        ["colours is [\"#fff\"], which is not an object of tokens and colours"]
    );
}

#[test]
fn a_token_left_to_the_platform_takes_a_colour_but_not_the_platform_s_word() {
    let (person, complaints) = Person::read(Some(&json!({
        "colours": { "caret": "platform", "selection.background": "#334455" }
    })));
    assert_eq!(complaints.len(), 1);
    assert_eq!(
        person
            .colours
            .get("selection.background")
            .map(String::as_str),
        Some("#334455")
    );
    assert!(!person.colours.contains_key("caret"));
}

// ---------------------------------------------------------------------------------------
// Colours over any theme, held to the floors
// ---------------------------------------------------------------------------------------

#[test]
fn a_colour_of_the_person_s_is_drawn_over_every_theme() {
    let layer = layer_of(json!({ "colours": { "accent.base": "#9a86ff" } }));
    for theme in [classic_dark(), purlis_dark()] {
        let drawn = drawn_under(theme, &layer);
        assert!(drawn.mine.is_empty(), "{:?}", drawn.mine);
        assert_eq!(drawn.theme.unwrap().value("accent.base"), "#9a86ff");
    }
}

#[test]
fn a_colour_that_puts_a_pair_under_its_floor_is_left_out_and_said() {
    // Primary words drawn in the window's own colour cannot be read.
    let ground = classic_dark().value("surface.base").to_owned();
    let layer =
        layer_of(json!({ "colours": { "text.primary": ground, "accent.base": "#9a86ff" } }));
    let drawn = drawn_under(classic_dark(), &layer);
    let theme = drawn.theme.expect("drawn");
    assert_eq!(
        theme.value("text.primary"),
        classic_dark().value("text.primary")
    );
    // The other colour is still the person's.
    assert_eq!(theme.value("accent.base"), "#9a86ff");
    assert_eq!(drawn.mine.len(), 1);
    assert_eq!(drawn.mine[0].token.as_deref(), Some("text.primary"));
    assert!(
        drawn.mine[0].said.starts_with(&format!(
            "your colour for text.primary, {ground}, is left out in Purlis Classic: text.primary on \
             surface.base would contrast 1:1, under its floor of 4.5:1"
        )),
        "{}",
        drawn.mine[0].said
    );
}

#[test]
fn a_ground_of_the_person_s_is_held_to_every_word_drawn_on_it() {
    // A light window under a dark theme's light words.
    let layer = layer_of(json!({ "colours": { "surface.base": "#f4f4f4" } }));
    let drawn = drawn_under(classic_dark(), &layer);
    assert_eq!(
        drawn.theme.unwrap().value("surface.base"),
        classic_dark().value("surface.base")
    );
    assert_eq!(drawn.mine[0].token.as_deref(), Some("surface.base"));
}

#[test]
fn a_terminal_colour_of_the_person_s_is_held_to_its_floor() {
    let background = classic_dark().value("terminal.background").to_owned();
    let layer = layer_of(json!({ "colours": { "terminal.ansi.red": background } }));
    let drawn = drawn_under(classic_dark(), &layer);
    assert_eq!(
        drawn.theme.unwrap().value("terminal.ansi.red"),
        classic_dark().value("terminal.ansi.red")
    );
    assert_eq!(drawn.mine.len(), 1);
}

#[test]
fn the_consent_floors_hold_over_the_person_s_colours() {
    // Danger drawn in the accent clears every contrast pair and is not told from primary.
    let accent = classic_dark().value("accent.base").to_owned();
    let layer = layer_of(json!({ "colours": { "danger.base": accent } }));
    let drawn = drawn_under(classic_dark(), &layer);
    let held = drawn.held.expect("the consent floors were held");
    assert!(held.floors.contains(&Floor::DangerApart), "{held:?}");
    assert!(held.tokens.contains(&"danger.base"));
    assert_eq!(
        drawn.theme.unwrap().value("danger.base"),
        classic_dark().value("danger.base")
    );
}

#[test]
fn a_layer_with_nothing_to_draw_leaves_the_answer_as_it_is() {
    let layer = layer_of(json!({ "density": "compact", "motion": "reduced" }));
    assert_eq!(drawn_under(purlis_dark(), &layer), draw(purlis_dark()));
}

// ---------------------------------------------------------------------------------------
// The minimum contrast
// ---------------------------------------------------------------------------------------

#[test]
fn more_contrast_raises_words_to_seven_and_marks_to_four_and_a_half() {
    assert_eq!(Least::Standard.floor(4.5), 4.5);
    assert_eq!(Least::Standard.floor(3.0), 3.0);
    assert_eq!(Least::More.floor(4.5), WORDS_MORE);
    assert_eq!(Least::More.floor(3.0), MARKS_MORE);
    assert_eq!(Least::More.floor(1.5), 1.5);
    assert_eq!(WORDS_MORE, 7.0);
    assert_eq!(MARKS_MORE, 4.5);
}

/// Every pair of the check, and each ANSI colour on the terminal's background.
fn every_pair() -> Vec<Pair> {
    PAIRS.iter().copied().chain(measure::ansi_pairs()).collect()
}

/// Every pair the check measures that `theme` has under its raised floor: words under 7:1, marks
/// under 4.5:1, the consent colours' among them.
fn under_raised(theme: &ResolvedTheme) -> Vec<String> {
    let mut under = Vec::new();
    for pair in every_pair() {
        let floor = Least::More.floor(pair.floor);
        if let Some(ratio) = measure::ratio(theme, &pair)
            && ratio < floor
        {
            under.push(format!("{} on {} {ratio} < {floor}", pair.front, pair.back));
        }
    }
    under
}

#[test]
fn more_contrast_draws_every_built_in_appearance_at_the_raised_floors() {
    let layer = layer_of(json!({ "contrast": "more" }));
    for theme in [classic(), purlis()] {
        for appearance in &theme.appearances {
            let drawn = drawn_under(appearance, &layer).theme.expect("drawn");
            assert_eq!(drawn.least, Least::More);
            assert_eq!(
                under_raised(&drawn),
                Vec::<String>::new(),
                "{} {}",
                theme.name,
                appearance.id
            );
        }
    }
}

#[test]
fn more_contrast_never_moves_the_window_s_grounds() {
    let layer = layer_of(json!({ "contrast": "more" }));
    let drawn = drawn_under(classic_dark(), &layer).theme.expect("drawn");
    // The window's grounds are what words are measured on: they are not moved.
    for ground in TIER_GROUNDS {
        assert_eq!(
            drawn.value(ground),
            classic_dark().value(ground),
            "{ground}"
        );
    }
}

#[test]
fn more_contrast_lightens_words_on_a_dark_ground_and_darkens_them_on_a_light_one() {
    let layer = layer_of(json!({ "contrast": "more" }));
    let quiet = |theme: &ResolvedTheme| measure::luminance(theme.value("text.muted"));
    let dark = drawn_under(classic_dark(), &layer).theme.unwrap();
    assert!(quiet(&dark) > quiet(classic_dark()));
    let light_theme = built_in_for(Appearance::Light);
    let light = drawn_under(light_theme, &layer).theme.unwrap();
    assert!(quiet(&light) < quiet(light_theme));
}

#[test]
fn more_contrast_holds_the_person_s_own_colours_to_the_raised_floors_too() {
    let layer = layer_of(json!({ "contrast": "more", "colours": { "text.muted": "#c4c4c4" } }));
    let drawn = drawn_under(classic_dark(), &layer);
    assert!(drawn.mine.is_empty());
    let theme = drawn.theme.unwrap();
    assert_eq!(under_raised(&theme), Vec::<String>::new());
}

#[test]
fn more_contrast_keeps_a_wash_s_alpha() {
    let layer = layer_of(json!({ "contrast": "more" }));
    let drawn = drawn_under(purlis_dark(), &layer).theme.unwrap();
    for (token, value) in &purlis_dark().values {
        let alpha = |value: &str| measure::long(value).get(6..8).map(str::to_owned);
        if alpha(value).is_some() && !vocabulary::is_platform(token, value) {
            assert_eq!(alpha(drawn.value(token)), alpha(value), "{token}");
        }
    }
}

#[test]
fn more_contrast_raises_the_terminal_s_colours_on_its_background() {
    let layer = layer_of(json!({ "contrast": "more" }));
    let drawn = drawn_under(classic_dark(), &layer).theme.unwrap();
    let background = drawn.value("terminal.background");
    for token in TOKENS
        .iter()
        .filter(|token| token.starts_with("terminal.ansi."))
    {
        let ratio = measure::contrast(drawn.value(token), background);
        assert!(
            ratio >= Least::More.floor(ansi_floor(token)) - 0.01,
            "{token} {ratio}"
        );
    }
}

#[test]
fn the_consent_floors_hold_over_more_contrast() {
    let layer = layer_of(json!({ "contrast": "more" }));
    for theme in [classic(), purlis()] {
        for appearance in &theme.appearances {
            let drawn = drawn_under(appearance, &layer).theme.unwrap();
            let (_, held) = floors::hold(&drawn);
            assert_eq!(held, None, "{} {}", theme.name, appearance.id);
        }
    }
}

#[test]
fn more_contrast_raises_the_parts_reading_floors_in_the_stylesheet() {
    // A ghost button on a mid grey: its words clear 4.5:1 there and not 7:1.
    let loaded = load(&json!({
        "name": "Shaped", "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "chart.2": "#484848" } }],
        "parts": { "button.root.ghost.fill": "chart.2" }
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let standard = loaded.theme.first().clone();
    let fallen = |theme: &ResolvedTheme| -> Vec<String> {
        parts::fallen(theme)
            .into_iter()
            .map(|one| one.part)
            .collect()
    };
    assert_eq!(fallen(&standard), Vec::<String>::new());
    let more = ResolvedTheme {
        least: Least::More,
        ..standard.clone()
    };
    let at_more = fallen(&more);
    assert!(
        at_more
            .iter()
            .any(|part| part.starts_with("button.root.ghost")),
        "{at_more:?}"
    );
    // The stylesheet draws the family default there.
    assert_ne!(parts::stylesheet(&more), parts::stylesheet(&standard));
}

#[test]
fn more_contrast_keeps_the_grey_tiers_in_their_order() {
    let layer = layer_of(json!({ "contrast": "more" }));
    for theme in [classic(), purlis()] {
        for appearance in &theme.appearances {
            let drawn = drawn_under(appearance, &layer).theme.unwrap();
            let lum = |token: &str| measure::luminance(drawn.value(token));
            let tiers = [
                "text.primary",
                "text.secondary",
                "text.muted",
                "text.quiet",
                "text.faint",
            ];
            for pair in tiers.windows(2) {
                let (above, below) = (lum(pair[0]), lum(pair[1]));
                let ordered = match appearance.appearance {
                    Appearance::Dark => above >= below,
                    Appearance::Light => above <= below,
                };
                assert!(
                    ordered,
                    "{} {}: {} before {}",
                    theme.name, appearance.id, pair[0], pair[1]
                );
            }
        }
    }
}

#[test]
fn the_consent_floors_are_held_at_the_raised_floors_with_purlis_s_own_raised() {
    // A theme drawn for more contrast whose consent words were never raised: holding the floors
    // puts purlis's own raised values in, so the danger and needs-you words are held at 7:1 too.
    let unraised = ResolvedTheme {
        least: Least::More,
        ..classic_dark().clone()
    };
    let (held, said) = floors::hold(&unraised);
    let said = said.expect("the raised consent floors were held");
    assert!(!said.tokens.is_empty());
    for (_, pair) in floors::pairs() {
        if let Some(ratio) = measure::ratio(&held, pair) {
            assert!(
                ratio >= Least::More.floor(pair.floor),
                "{} on {} {ratio}",
                pair.front,
                pair.back
            );
        }
    }
}

#[test]
fn more_contrast_holds_the_consent_floors_raised_in_every_workspace_tint() {
    let layer = layer_of(json!({ "contrast": "more" }));
    for theme in [classic(), purlis()] {
        for appearance in &theme.appearances {
            let drawn = drawn_under(appearance, &layer).theme.unwrap();
            for hue in &drawn.tint.hues {
                let tinted = tint::tinted(&drawn, hue.name);
                for (_, pair) in floors::pairs() {
                    if let Some(ratio) = measure::ratio(&tinted, pair) {
                        assert!(
                            ratio >= Least::More.floor(pair.floor),
                            "{} {} in {}: {} on {} {ratio}",
                            theme.name,
                            appearance.id,
                            hue.name,
                            pair.front,
                            pair.back
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn of_two_colours_that_break_a_pair_the_one_that_breaks_it_is_left_out_and_both_are_named() {
    // A light ground with the theme's light words breaks them; a dark text alone would not.
    let layer = layer_of(json!({
        "colours": { "text.primary": "#f0f0f0", "surface.base": "#ececec" }
    }));
    let drawn = drawn_under(classic_dark(), &layer);
    let theme = drawn.theme.unwrap();
    let said = &drawn.mine[0].said;
    assert!(
        said.contains("text.primary") && said.contains("surface.base"),
        "{said}"
    );
    assert!(
        theme.value("surface.base") == classic_dark().value("surface.base")
            || theme.value("text.primary") == classic_dark().value("text.primary")
    );
}

// ---------------------------------------------------------------------------------------
// Allow never takes the primary look through the person's colours (#2948's floor)
// ---------------------------------------------------------------------------------------

/// What the window draws with nothing picked, the person's `layer` over it: resolved, then the
/// layer drawn and the consent floors held over it.
fn resolved_under(layer: &Layer) -> Drawn {
    resolve(
        &Inputs {
            pick: None,
            theirs: None,
            on: On::Every,
            system: Appearance::Dark,
            contrast: Contrast::Standard,
        },
        || std::sync::Arc::from(Vec::new()),
    )
    .under(layer)
}

/// The nearest Allow comes to the primary in `theme`, in any state.
fn allow_from_primary(theme: &ResolvedTheme) -> f64 {
    parts::allow_from_primary(theme)
        .into_iter()
        .map(|look| look.apart)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn no_colour_of_the_person_s_draws_allow_as_the_primary() {
    let base = default_theme();
    // Allow's fill as the primary's, the primary's as Allow's, and each under the pointer.
    let cases = [
        ("control.base", "accent.fill"),
        ("accent.fill", "control.base"),
        ("control.hover", "accent.fill-hover"),
        ("accent.fill-hover", "control.hover"),
    ];
    for contrast in ["standard", "more"] {
        for (token, like) in cases {
            let layer = layer_of(json!({
                "contrast": contrast,
                "colours": { token: base.value(like) }
            }));
            let drawn = resolved_under(&layer);
            let theme = drawn.theme.expect("drawn");
            let apart = allow_from_primary(&theme);
            assert!(
                apart >= ALLOW_FROM_PRIMARY,
                "{token} := {like} ({contrast}): Allow {apart} from the primary"
            );
            // The person's colour was either left out or held by the floor, and said either way.
            let held = drawn
                .held
                .as_ref()
                .is_some_and(|held| held.floors.contains(&Floor::AllowNotPrimary));
            let left_out = drawn
                .mine
                .iter()
                .any(|one| one.token.as_deref() == Some(token));
            assert!(
                held || left_out || theme.value(token) != base.value(like),
                "{token} := {like} ({contrast}): drawn as asked with nothing said"
            );
        }
    }
}
