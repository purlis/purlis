// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The theme check's pairs and fits of #2522 and #2706, its pairs laid over two layers (#2724)
//! and its styles drawn without a face (#2736), at the engine's interface: a theme in, the
//! report out.

use serde_json::{Value, json};

use super::super::{
    Appearance, Checked, Density, FITS, Finding, Floor, PAIRS, Report, ResolvedTheme, Style,
    built_ins, check, check_every, classic, draw, load, purlis, warnings_said,
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

/// The grounds the window's words are selected on in their own colour: the window, a sunken box
/// (a release's notes, a finished report), a pane and a Notice, and a dialog.
const WORD_GROUNDS: [&str; 4] = [
    "surface.base",
    "surface.sunken",
    "surface.raised",
    "surface.overlay",
];

/// A markdown render's words.
const MARKDOWN_WORDS: [&str; 5] = [
    "markdown.link",
    "markdown.code",
    "markdown.code-block",
    "markdown.quote",
    "markdown.heading",
];

/// The danger words, and the grounds they are selected on: those, and a trouble Notice's.
const DANGER_WORDS: [&str; 2] = ["danger.text", "danger.base"];
const DANGER_GROUNDS: [&str; 4] = [
    "surface.base",
    "surface.raised",
    "surface.overlay",
    "danger.surface",
];

fn classic_dark() -> &'static ResolvedTheme {
    classic().first()
}

fn classic_light() -> &'static ResolvedTheme {
    classic()
        .of(Appearance::Light)
        .expect("Classic's light appearance")
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

#[test]
fn a_markdown_render_s_words_are_measured_on_the_selection_over_every_ground() {
    for word in MARKDOWN_WORDS {
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
fn selected_text_a_theme_colours_is_held_on_a_trouble_notice_and_a_dialog() {
    // Selected text barely apart from its selection: a trouble Notice's danger words and a
    // dialog's ask vanish while selected. The floors draw purlis's own selection, the
    // platform's, so the pair is not drawn, and the theme's own danger surface stays.
    let mine = load(&json!({
        "name": "Haze",
        "appearance": "dark",
        "tokens": {
            "selection.background": "#3a3a3a",
            "selection.text": "#404040",
            "danger.surface": "#241414",
        }
    }))
    .theme;
    let report = check(mine.first());
    for ground in ["danger.surface", "surface.overlay"] {
        assert!(
            found_over(&report, "selection.text", ground).is_some(),
            "{ground}"
        );
    }
    let drawn = draw(mine.first());
    let held = drawn.held.expect("the floors were held");
    assert_eq!(held.floors, [Floor::Ask, Floor::Danger]);
    assert_eq!(held.tokens, ["selection.background", "selection.text"]);
    let theme = drawn.theme.expect("a theme is always drawn");
    for token in ["selection.background", "selection.text"] {
        assert_eq!(theme.value(token), classic_dark().value(token), "{token}");
    }
    assert_eq!(theme.value("danger.surface"), "#241414");
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

// ---------------------------------------------------------------------------------------
// A pair laid over two layers (#2724)
// ---------------------------------------------------------------------------------------

/// The ratio the check found selected text at on the selection over the danger wash over the
/// dialog, as the theme is.
fn found_on_the_wash(report: &Report) -> Option<f64> {
    report.findings.iter().find_map(|one| match one {
        Finding::Contrast {
            tint: None,
            pair,
            ratio,
        } if pair.front == "selection.text"
            && pair.under == Some("danger.wash")
            && pair.beneath == Some("surface.overlay") =>
        {
            Some(*ratio)
        }
        _ => None,
    })
}

/// A dark theme with a clear selection, a half red danger wash and selected text in `text`.
fn washed_selection(text: &str) -> ResolvedTheme {
    dark(
        json!({
            "selection.background": "#00000000",
            "selection.text": text,
            "danger.wash": "#ff000080",
        }),
        json!({}),
    )
}

#[test]
fn the_command_on_the_danger_wash_is_held_on_a_selection_over_the_wash_over_the_dialog() {
    let pair = PAIRS
        .iter()
        .find(|pair| pair.under == Some("danger.wash") && pair.back == "selection.background")
        .expect("the selection on the danger wash is held");
    assert_eq!(
        (pair.front, pair.beneath, pair.floor),
        ("selection.text", Some("surface.overlay"), 4.5)
    );
    assert!(!pair.selected, "every selected word is drawn in its colour");
}

#[test]
fn a_pair_over_two_layers_lays_both_from_the_bottom_up() {
    // Half of #ff0000 laid over Classic's dialog (#1f1f1c) is #8f0f0e, and a clear selection
    // over that leaves it as it is: selected text of that colour vanishes into it.
    assert_eq!(
        found_on_the_wash(&check(&washed_selection("#8f0f0e"))),
        Some(1.0)
    );
    // The wash's own colour, which it would be read as if the dialog under it were left out,
    // is apart from what is drawn.
    assert_eq!(
        found_on_the_wash(&check(&washed_selection("#ff0000"))),
        Some(2.34)
    );
}

#[test]
fn a_pair_over_two_layers_is_said_with_both() {
    let theme = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "selection.background": "#00000000",
            "selection.text": "#8f0f0e",
            "danger.wash": "#ff000080",
        } }]
    }))
    .theme;
    let said = warnings_said(&theme, &check_every(&theme));
    assert!(
        said.iter().any(|line| line.starts_with(
            "selection.text on selection.background over danger.wash over surface.overlay: \
             1.00, under 4.5"
        )),
        "{said:#?}"
    );
    let finding = check(theme.first())
        .findings
        .into_iter()
        .find(|one| matches!(one, Finding::Contrast { pair, .. } if pair.beneath.is_some()))
        .expect("the pair over two layers");
    let json = serde_json::to_value(finding).expect("JSON");
    assert_eq!(json["pair"]["under"], "danger.wash");
    assert_eq!(json["pair"]["beneath"], "surface.overlay");
}

#[test]
fn selected_text_that_vanishes_into_the_danger_wash_is_held_by_the_ask_floor() {
    // Grey selected text reads on every ground but the red wash over the dialog: the Ask floor
    // holds it, and purlis draws its own selection, the platform's, so the wash stays the
    // theme's.
    let theme = washed_selection("#999999");
    let report = check(&theme);
    assert_eq!(found_on_the_wash(&report), Some(3.29));
    assert_eq!(
        found_over(&report, "selection.text", "surface.overlay"),
        None
    );
    let drawn = draw(&theme);
    let held = drawn.held.expect("the floor was held");
    assert_eq!(held.floors, [Floor::Ask]);
    assert_eq!(held.tokens, ["selection.background", "selection.text"]);
    let drawn = drawn.theme.expect("a theme is always drawn");
    for token in ["selection.background", "selection.text"] {
        assert_eq!(drawn.value(token), classic_dark().value(token), "{token}");
    }
    assert_eq!(drawn.value("danger.wash"), "#ff000080");
}

#[test]
fn purlis_s_see_through_selection_reads_on_the_danger_wash_in_every_appearance() {
    for one in &purlis().appearances {
        assert_eq!(found_on_the_wash(&check(one)), None, "{}", one.id);
    }
}

// ---------------------------------------------------------------------------------------
// A style drawn without a face (#2736)
// ---------------------------------------------------------------------------------------

fn faceless(report: &Report) -> Vec<&Finding> {
    report
        .findings
        .iter()
        .filter(|one| matches!(one, Finding::StyleWithoutAFace { .. }))
        .collect()
}

/// Purlis's dark appearance with its code font left only its upright regular.
fn thin_mono() -> ResolvedTheme {
    let mut thin = purlis().first().clone();
    thin.fonts
        .mono
        .faces
        .retain(|one| one.weight == 400 && one.style == Style::Normal);
    thin
}

#[test]
fn each_style_drawn_without_a_face_is_a_finding_once_naming_its_slot_and_style() {
    // Purlis is tinted by every workspace colour; the faces are found once, not in each tint.
    let report = check(&thin_mono());
    assert_eq!(
        faceless(&report),
        [
            &Finding::StyleWithoutAFace {
                slot: "mono",
                weight: 400,
                style: Style::Italic,
            },
            &Finding::StyleWithoutAFace {
                slot: "mono",
                weight: 700,
                style: Style::Normal,
            },
        ]
    );
}

#[test]
fn a_style_drawn_without_a_face_is_said_by_its_slot_and_style() {
    let thin = thin_mono();
    let checked = [Checked {
        appearance: thin.id.clone(),
        options: Default::default(),
        report: check(&thin),
        held: None,
        hard: Vec::new(),
    }];
    let said = warnings_said(purlis(), &checked);
    assert_eq!(
        said,
        [
            "mono draws italic text at weight 400 with no face of its own, so the browser makes \
             it from another face",
            "mono draws upright text at weight 700 with no face of its own, so the browser makes \
             it from another face",
        ]
    );
    let json = serde_json::to_value(faceless(&checked[0].report)[0]).expect("JSON");
    assert_eq!(
        json,
        json!({ "kind": "style-without-a-face", "slot": "mono", "weight": 400, "style": "italic" })
    );
}

#[test]
fn purlis_and_purlis_classic_draw_every_style_in_a_face_of_its_own() {
    for theme in [classic(), purlis()] {
        for one in check_every(theme) {
            assert_eq!(
                faceless(&one.report),
                Vec::<&Finding>::new(),
                "{} {}",
                theme.name,
                one.appearance
            );
        }
    }
}
