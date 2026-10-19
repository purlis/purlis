// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The theme check's pairs and fits of #2522 and #2706, at the engine's interface: a theme in,
//! the report out.

use serde_json::{Value, json};

use super::super::{
    Density, FITS, Finding, Floor, PAIRS, Report, ResolvedTheme, built_in, built_ins, check,
    check_every, draw, load, warnings_said,
};

/// The syntax roles the editor draws.
const ROLES: [&str; 15] = [
    "keyword",
    "string",
    "number",
    "constant",
    "comment",
    "function",
    "type",
    "property",
    "attribute",
    "tag",
    "heading",
    "invalid",
    "variable",
    "operator",
    "punctuation",
];

/// The words a selection keeps in their own colour, beside the primary ones.
const SELECTED_WORDS: [&str; 8] = [
    "text.secondary",
    "text.muted",
    "accent.base",
    "state.running",
    "state.waiting",
    "state.failed",
    "state.success",
    "state.unreadable",
];

/// The grounds a selection is drawn on (#2706): the window, a field, a pane and a Notice, a
/// dialog, and a trouble Notice.
const GROUNDS: [&str; 5] = [
    "surface.base",
    "surface.sunken",
    "surface.raised",
    "surface.overlay",
    "danger.surface",
];

/// The grounds the window's words are selected on in their own colour: the window, a pane and a
/// Notice, and a dialog.
const WORD_GROUNDS: [&str; 3] = ["surface.base", "surface.raised", "surface.overlay"];

/// The danger words, and the grounds they are selected on: those, and a trouble Notice's.
const DANGER_WORDS: [&str; 2] = ["danger.text", "danger.base"];
const DANGER_GROUNDS: [&str; 4] = [
    "surface.base",
    "surface.raised",
    "surface.overlay",
    "danger.surface",
];

fn classic_dark() -> &'static ResolvedTheme {
    built_in("purlis-dark").expect("Classic's dark appearance")
}

fn classic_light() -> &'static ResolvedTheme {
    built_in("purlis-light").expect("Classic's light appearance")
}

/// A format 2 theme with one dark appearance that sets `tokens`, and `more` beside it: its
/// appearance as read.
fn dark(tokens: Value, more: Value) -> ResolvedTheme {
    let mut theme = json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    });
    for (key, value) in more.as_object().cloned().unwrap_or_default() {
        theme[key] = value;
    }
    let resolved = load(&theme);
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

/// The ratio the check found `front` on `back` at, as the theme is.
fn found(report: &Report, front: &str, back: &str) -> Option<f64> {
    report.findings.iter().find_map(|one| match one {
        Finding::Contrast {
            tint: None,
            pair,
            ratio,
        } if pair.front == front && pair.back == back => Some(*ratio),
        _ => None,
    })
}

/// The ratio the check found `front` on a selection laid over `ground` at, as the theme is.
fn found_over(report: &Report, front: &str, ground: &str) -> Option<f64> {
    report.findings.iter().find_map(|one| match one {
        Finding::Contrast {
            tint: None,
            pair,
            ratio,
        } if pair.front == front
            && pair.back == "selection.background"
            && pair.under == Some(ground) =>
        {
            Some(*ratio)
        }
        _ => None,
    })
}

/// A dark theme whose selection is clear and sets `front` to `ground`'s colour: selected, it is
/// drawn on that ground alone, and vanishes into it.
fn clear_selection_with(front: &str, ground: &str) -> ResolvedTheme {
    let mut tokens = serde_json::Map::new();
    tokens.insert("selection.background".to_owned(), json!("#00000000"));
    tokens.insert(front.to_owned(), json!(classic_dark().value(ground)));
    dark(Value::Object(tokens), json!({}))
}

fn lines(report: &Report) -> Vec<&Finding> {
    report
        .findings
        .iter()
        .filter(|one| matches!(one, Finding::LineDoesNotFit { .. }))
        .collect()
}

#[test]
fn every_built_in_still_clears_the_check() {
    for theme in built_ins() {
        let report = check(theme);
        assert!(report.passes(), "{}: {:#?}", theme.name, report.findings);
    }
}

// ---------------------------------------------------------------------------------------
// Syntax on the editor's current line
// ---------------------------------------------------------------------------------------

#[test]
fn every_syntax_role_is_held_on_the_editor_s_current_line() {
    for role in ROLES {
        let token = format!("syntax.{role}");
        // The current line's own colour: the role vanishes into it.
        let mut tokens = serde_json::Map::new();
        tokens.insert(token.clone(), json!(classic_dark().value("surface.hover")));
        let report = check(&dark(Value::Object(tokens), json!({})));
        assert_eq!(
            found(&report, &token, "surface.hover"),
            Some(1.0),
            "{token}"
        );
        let pair = PAIRS
            .iter()
            .find(|pair| pair.front == token && pair.back == "surface.hover")
            .expect("the pair is held");
        assert_eq!(pair.floor, 3.0, "a syntax role is held at the syntax floor");
    }
}

#[test]
fn the_current_line_is_measured_over_the_editor_s_ground() {
    // A current line that is a clear wash is the editor's own ground: a role that reads on the
    // ground reads on it, and one the same as the ground does not.
    let clear = dark(
        json!({
            "surface.hover": "#ffffff00",
            "terminal.background": "#000000",
            "syntax.keyword": "#000000",
        }),
        json!({}),
    );
    let report = check(&clear);
    assert_eq!(found(&report, "syntax.keyword", "surface.hover"), Some(1.0));
    assert_eq!(found(&report, "syntax.string", "surface.hover"), None);
}

#[test]
fn purlis_classic_light_s_property_and_attribute_read_on_the_current_line() {
    // Darkened from #218bff (2.79 on the current line) to the nearest of its hue that clears
    // the floor; the rest of the light appearance is as it was.
    for token in ["syntax.property", "syntax.attribute"] {
        assert_eq!(classic_light().value(token), "#1283ff");
    }
    assert_eq!(
        classic_light().value("terminal.ansi.bright-blue"),
        "#218bff"
    );
    let report = check(classic_light());
    assert_eq!(found(&report, "syntax.property", "surface.hover"), None);
}

// ---------------------------------------------------------------------------------------
// Words on a theme's selection
// ---------------------------------------------------------------------------------------

#[test]
fn words_selected_in_their_own_colour_are_held_on_the_theme_s_selection() {
    for word in SELECTED_WORDS {
        // A selection the colour of the word: it vanishes, while selected text keeps its colour.
        let mut tokens = serde_json::Map::new();
        tokens.insert(
            "selection.background".to_owned(),
            json!(classic_dark().value(word)),
        );
        let report = check(&dark(Value::Object(tokens), json!({})));
        assert_eq!(
            found(&report, word, "selection.background"),
            Some(1.0),
            "{word}"
        );
    }
}

#[test]
fn a_theme_that_colours_selected_text_is_held_by_that_colour_alone() {
    // Every word selected is drawn in `selection.text`, so only that pair is measured.
    let report = check(&dark(
        json!({
            "selection.background": classic_dark().value("text.muted"),
            "selection.text": "#000000",
        }),
        json!({}),
    ));
    for word in SELECTED_WORDS {
        assert_eq!(found(&report, word, "selection.background"), None, "{word}");
    }
    assert!(found(&report, "selection.text", "selection.background").is_none());
}

#[test]
fn a_selection_left_to_the_platform_is_not_measured() {
    let report = check(classic_dark());
    for word in SELECTED_WORDS {
        assert_eq!(found(&report, word, "selection.background"), None);
    }
}

#[test]
fn a_see_through_selection_is_measured_over_every_ground_it_is_drawn_on() {
    // Selected text in the theme's colour, on a clear selection: it is drawn on the ground under
    // the selection, so each ground is measured on its own.
    for ground in GROUNDS {
        let report = check(&clear_selection_with("selection.text", ground));
        assert_eq!(
            found_over(&report, "selection.text", ground),
            Some(1.0),
            "{ground}"
        );
    }
}

#[test]
fn words_selected_in_their_own_colour_are_measured_over_every_ground() {
    for word in SELECTED_WORDS {
        for ground in WORD_GROUNDS {
            let report = check(&clear_selection_with(word, ground));
            assert_eq!(
                found_over(&report, word, ground),
                Some(1.0),
                "{word} over {ground}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Danger words on a theme's selection
// ---------------------------------------------------------------------------------------

#[test]
fn danger_words_are_held_on_the_theme_s_selection_at_the_text_floor() {
    for word in DANGER_WORDS {
        for ground in DANGER_GROUNDS {
            let pair = PAIRS
                .iter()
                .find(|pair| {
                    pair.front == word
                        && pair.back == "selection.background"
                        && pair.under == Some(ground)
                })
                .unwrap_or_else(|| panic!("{word} over {ground} is held"));
            assert_eq!(pair.floor, 4.5, "{word} over {ground}");
            assert!(
                pair.selected,
                "{word} over {ground} is drawn in its own colour"
            );
            let report = check(&clear_selection_with(word, ground));
            assert_eq!(
                found_over(&report, word, ground),
                Some(1.0),
                "{word} over {ground}"
            );
        }
    }
}

#[test]
fn danger_words_on_a_selection_that_colours_its_text_are_held_by_that_colour() {
    let report = check(&dark(
        json!({
            "selection.background": classic_dark().value("danger.text"),
            "selection.text": "#000000",
        }),
        json!({}),
    ));
    for word in DANGER_WORDS {
        assert_eq!(found(&report, word, "selection.background"), None, "{word}");
    }
}

#[test]
fn a_selection_danger_words_vanish_into_is_drawn_as_purlis_s_own() {
    // A selection the colour of the danger words: the danger floor holds, and the window draws
    // purlis's own selection, so a Notice's trouble stays readable while selected.
    let mine = load(&json!({
        "name": "Blush",
        "appearance": "dark",
        "tokens": { "selection.background": classic_dark().value("danger.text") }
    }))
    .theme;
    let drawn = draw(mine.first());
    let held = drawn.held.expect("the floor was held");
    assert!(held.floors.contains(&Floor::Danger), "{:?}", held.floors);
    let theme = drawn.theme.expect("a theme is always drawn");
    assert_eq!(
        theme.value("selection.background"),
        classic_dark().value("selection.background")
    );
    for word in DANGER_WORDS {
        assert_eq!(found(&check(&theme), word, "selection.background"), None);
    }
}

#[test]
fn purlis_and_purlis_classic_clear_every_selection_pair() {
    for theme in built_ins() {
        let report = check(theme);
        assert!(
            !report.findings.iter().any(|one| matches!(
                one,
                Finding::Contrast { pair, .. } if pair.back == "selection.background"
            )),
            "{}: {:#?}",
            theme.name,
            report.findings
        );
    }
}

// ---------------------------------------------------------------------------------------
// Charts
// ---------------------------------------------------------------------------------------

#[test]
fn every_chart_series_is_held_on_its_track() {
    for at in 1..=8 {
        let token = format!("chart.{at}");
        let mut tokens = serde_json::Map::new();
        tokens.insert(token.clone(), json!(classic_dark().value("surface.sunken")));
        let report = check(&dark(Value::Object(tokens), json!({})));
        assert_eq!(
            found(&report, &token, "surface.sunken"),
            Some(1.0),
            "{token}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Geometry across tokens
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_s_lines_fit_their_boxes_in_both_densities() {
    for theme in built_ins() {
        assert_eq!(
            lines(&check(theme)),
            Vec::<&Finding>::new(),
            "{}",
            theme.name
        );
    }
}

#[test]
fn a_line_taller_than_its_control_is_named_with_its_sizes() {
    // Each number within its own bound, and together a 32.4px line in a 28px control.
    let tall = dark(
        json!({ "font-size.base": 18, "line-height.body": 1.8 }),
        json!({}),
    );
    let report = check(&tall);
    let control = FITS
        .iter()
        .find(|fit| fit.height == "size.control")
        .copied()
        .expect("the control's fit");
    assert!(report.findings.contains(&Finding::LineDoesNotFit {
        density: Density::Comfortable,
        fit: control,
        line: 32.4,
        room: 28.0,
    }));
    // Compact's control is 24px, and its line core's compact 1.3, the smaller of that and the
    // theme's 1.8: 18 × 1.3 = 23.4 fits.
    assert!(!report.findings.iter().any(|one| matches!(
        one,
        Finding::LineDoesNotFit { density: Density::Compact, fit, .. } if fit.height == "size.control"
    )));
    // And a person reads it with the sizes named.
    let theme = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark",
            "tokens": { "font-size.base": 18, "line-height.body": 1.8 } }]
    }))
    .theme;
    let said = warnings_said(&theme, &check_every(&theme));
    assert!(
        said.iter().any(|line| line
            == "font-size.base × line-height.body is a 32.4px line, taller than \
                size.control's 28px (comfortable)"),
        "{said:#?}"
    );
}

#[test]
fn a_strip_s_line_is_held_in_compact_as_the_theme_tunes_it() {
    // A compact strip tuned down to 16px holds a tab's 13px title on a 1.3 line (16.9px) no
    // longer; comfortable is untouched.
    let tight = dark(json!({}), json!({ "compact": { "size.chrome": 16 } }));
    let report = check(&tight);
    let chrome = FITS
        .iter()
        .find(|fit| fit.height == "size.chrome")
        .copied()
        .expect("the strip's fit");
    assert_eq!(
        lines(&report),
        vec![&Finding::LineDoesNotFit {
            density: Density::Compact,
            fit: chrome,
            line: 16.9,
            room: 16.0,
        }]
    );
}

#[test]
fn a_line_that_exactly_fills_its_box_fits() {
    // 15 × 1.4 = 21, a strip's own height at comfortable.
    let exact = dark(
        json!({ "font-size.md": 15, "line-height.body": 1.4, "size.chrome": 21 }),
        json!({}),
    );
    let report = check(&exact);
    assert!(!report.findings.iter().any(|one| matches!(
        one,
        Finding::LineDoesNotFit {
            density: Density::Comfortable,
            ..
        }
    )));
    // Compact's 1.3 line of it is 19.5px in an 18px strip.
    assert!(report.findings.iter().any(|one| matches!(
        one,
        Finding::LineDoesNotFit { density: Density::Compact, line, room, .. }
            if *line == 19.5 && *room == 18.0
    )));
}
