// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The tokens #2327 adds, at the engine's interface: syntax, markdown, the window's selection,
//! caret, placeholder and scrollbars, charts, the terminal's extras and the window's background.
//! Purlis Classic sets each to what the window drew before it existed, the typed ones are held to
//! their types, and the check measures the new pairs.

use serde_json::{Value, json};

use super::vocabulary::{FLAGS, PLATFORM, PLATFORM_DRAWN, TOKENS};
use super::*;

fn dark() -> &'static ResolvedTheme {
    classic().first()
}

fn light() -> &'static ResolvedTheme {
    classic().of(Appearance::Light).unwrap()
}

fn said(resolved: &Resolved) -> Vec<String> {
    resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect()
}

/// A format 2 theme over Purlis Classic whose dark appearance sets `tokens`.
fn dark_with(tokens: Value) -> Resolved {
    load(&json!({
        "name": "Probe",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": tokens }]
    }))
}

/// Which syntax role the editor drew in which terminal colour, and italic or bold, before the
/// editor read the syntax tokens: `app/src/editor/look.ts` as it was at 5e9de946.
const LOOK_BEFORE: &[(&str, &str, bool, bool)] = &[
    ("keyword", "terminal.ansi.magenta", false, false),
    ("string", "terminal.ansi.green", false, false),
    ("number", "terminal.ansi.yellow", false, false),
    ("constant", "terminal.ansi.yellow", false, false),
    ("comment", "terminal.ansi.bright-black", true, false),
    ("function", "terminal.ansi.blue", false, false),
    ("type", "terminal.ansi.cyan", false, false),
    ("property", "terminal.ansi.bright-blue", false, false),
    ("attribute", "terminal.ansi.bright-blue", false, false),
    ("tag", "terminal.ansi.red", false, true),
    ("heading", "terminal.ansi.red", false, true),
    ("invalid", "terminal.ansi.bright-red", false, false),
    // Not coloured by a rule of their own: drawn in the editor's own colour.
    ("variable", "terminal.foreground", false, false),
    ("operator", "terminal.foreground", false, false),
    ("punctuation", "terminal.foreground", false, false),
];

/// The roles Purlis Classic light draws a shade darker than the colour the editor borrowed, so
/// they read on the editor's current line (#2522), and the colour each is drawn in.
const LIGHT_DARKER: &[(&str, &str)] = &[
    ("syntax.property", "#1283ff"),
    ("syntax.attribute", "#1283ff"),
];

// ---------------------------------------------------------------------------------------
// Purlis Classic draws as it did
// ---------------------------------------------------------------------------------------

#[test]
fn purlis_classic_draws_its_code_in_the_colours_the_editor_borrowed_from_the_terminal() {
    for theme in [dark(), light()] {
        for (role, borrowed, italic, bold) in LOOK_BEFORE {
            let token = format!("syntax.{role}");
            let darker = LIGHT_DARKER
                .iter()
                .find(|(it, _)| *it == token && theme.appearance == Appearance::Light);
            match darker {
                Some((_, colour)) => assert_eq!(theme.value(&token), *colour, "{token}"),
                None => assert_eq!(theme.value(&token), theme.value(borrowed), "{token}"),
            }
            assert_eq!(
                theme.typed.flag(&format!("{token}.italic")),
                *italic,
                "{token}"
            );
            assert_eq!(theme.typed.flag(&format!("{token}.bold")), *bold, "{token}");
        }
        let roles = TOKENS.iter().filter(|it| it.starts_with("syntax.")).count();
        assert_eq!(roles, LOOK_BEFORE.len());
    }
}

#[test]
fn purlis_classic_draws_markdown_charts_and_the_window_as_it_did() {
    for theme in [dark(), light()] {
        // A markdown render's links were the accent, and its code, quotes and headings the
        // words' own colour (`.release-notes`).
        assert_eq!(theme.value("markdown.link"), theme.value("accent.base"));
        for token in [
            "markdown.code",
            "markdown.code-block",
            "markdown.quote",
            "markdown.heading",
        ] {
            assert_eq!(theme.value(token), theme.value("text.primary"), "{token}");
        }
        // A chart's one series was drawn in the accent.
        assert_eq!(theme.value("chart.1"), theme.value("accent.base"));
        // The window was painted in the window's surface.
        assert_eq!(
            theme.value("window.background"),
            theme.value("surface.base")
        );
    }
}

#[test]
fn purlis_classic_leaves_the_selection_caret_placeholder_and_scrollbars_to_the_platform() {
    for theme in [dark(), light()] {
        for token in PLATFORM_DRAWN {
            assert_eq!(theme.value(token), PLATFORM, "{token}");
        }
        // The track is drawn only with a thumb, and Classic's is clear.
        assert_eq!(theme.value("scrollbar.track"), "#00000000");
    }
}

#[test]
fn purlis_classic_draws_the_terminal_as_xterm_s_defaults_did() {
    // xterm 6.0's own defaults, which the terminal was drawn with before a theme could set
    // them: the inactive selection is the selection, the scrollbar is the foreground at 20%, 40%
    // and 50%, a block cursor that does not blink, a line height of 1, bold in the bright
    // colours.
    for theme in [dark(), light()] {
        assert_eq!(
            theme.value("terminal.selection-inactive"),
            theme.value("terminal.selection")
        );
        let fg = theme.value("terminal.foreground");
        assert_eq!(theme.value("terminal.scrollbar"), format!("{fg}33"));
        assert_eq!(theme.value("terminal.scrollbar-hover"), format!("{fg}66"));
        assert_eq!(theme.value("terminal.scrollbar-active"), format!("{fg}80"));
        assert_eq!(theme.typed.choice("terminal.cursor-style"), "block");
        assert!(!theme.typed.flag("terminal.cursor-blink"));
        assert!((theme.typed.number("terminal.line-height") - 1.0).abs() < f64::EPSILON);
        assert!(theme.typed.flag("terminal.bold-as-bright"));
    }
}

#[test]
fn purlis_classic_gives_every_typed_token() {
    for theme in [dark(), light()] {
        assert_eq!(theme.typed.flags.len(), FLAGS.len());
        assert_eq!(theme.typed.choices.len(), 1);
        assert_eq!(theme.typed.numbers.len(), 1);
    }
}

// ---------------------------------------------------------------------------------------
// A token that is not a colour is held to its type
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_sets_syntax_with_italic_and_bold_and_the_terminal_s_extras() {
    let loaded = dark_with(json!({
        "syntax.keyword": "#ff00ff",
        "syntax.keyword.italic": true,
        "syntax.comment.italic": false,
        "terminal.cursor-style": "bar",
        "terminal.cursor-blink": true,
        "terminal.line-height": 1.25,
        "terminal.bold-as-bright": false
    }));
    assert_eq!(said(&loaded), Vec::<String>::new());
    let drawn = loaded.theme.first();
    assert_eq!(drawn.value("syntax.keyword"), "#ff00ff");
    assert!(drawn.typed.flag("syntax.keyword.italic"));
    assert!(!drawn.typed.flag("syntax.comment.italic"));
    assert_eq!(drawn.typed.choice("terminal.cursor-style"), "bar");
    assert!(drawn.typed.flag("terminal.cursor-blink"));
    assert!((drawn.typed.number("terminal.line-height") - 1.25).abs() < f64::EPSILON);
    assert!(!drawn.typed.flag("terminal.bold-as-bright"));
    // What it does not say is Classic's.
    assert!(drawn.typed.flag("syntax.tag.bold"));
}

#[test]
fn a_typed_value_of_the_wrong_type_refuses_the_theme_and_names_the_token() {
    let cases = [
        (
            json!({ "syntax.comment.italic": "yes" }),
            "syntax.comment.italic in appearance dark is \"yes\", which is not true or false",
        ),
        (
            json!({ "terminal.cursor-style": "beam" }),
            "terminal.cursor-style in appearance dark is \"beam\", which is not one of \"block\", \"underline\", \"bar\"",
        ),
        (
            json!({ "terminal.line-height": 0.5 }),
            "terminal.line-height in appearance dark is 0.5, which is not a number from 1 to 2",
        ),
        (
            json!({ "terminal.line-height": 3 }),
            "terminal.line-height in appearance dark is 3, which is not a number from 1 to 2",
        ),
        (
            json!({ "terminal.line-height": "1.2" }),
            "terminal.line-height in appearance dark is \"1.2\", which is not a number from 1 to 2",
        ),
        (
            json!({ "syntax.keyword": true }),
            "syntax.keyword in appearance dark is true, which is neither a hex colour nor a name in this theme's palette",
        ),
    ];
    for (tokens, why) in cases {
        let loaded = dark_with(tokens);
        assert!(loaded.refused, "{why}");
        assert_eq!(said(&loaded), [why]);
    }
}

#[test]
fn a_typed_value_layers_over_the_parent_s_like_a_colour() {
    let library = Library::new().with(
        "parent",
        json!({
            "name": "Parent",
            "extends": "purlis-classic",
            "appearances": [{ "id": "dark", "tokens": { "terminal.cursor-style": "underline" } }]
        }),
    );
    let child = library.load(
        None,
        &json!({
            "name": "Child",
            "extends": "parent",
            "appearances": [{ "id": "dark", "tokens": { "terminal.cursor-blink": true } }]
        }),
    );
    assert!(!child.refused, "{:?}", said(&child));
    let drawn = child.theme.first();
    assert_eq!(drawn.typed.choice("terminal.cursor-style"), "underline");
    assert!(drawn.typed.flag("terminal.cursor-blink"));
}

#[test]
fn a_format_1_file_names_a_typed_token_and_is_told_it_belongs_in_a_theme_folder() {
    let loaded = load(&json!({
        "name": "Old",
        "tokens": { "terminal.cursor-blink": true, "syntax.keyword": "#ff00ff" }
    }));
    assert!(!loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "terminal.cursor-blink is set in a theme folder's theme.json (format 2), so it is left out"
        ]
    );
    let drawn = loaded.theme.first();
    assert_eq!(drawn.value("syntax.keyword"), "#ff00ff");
    assert!(!drawn.typed.flag("terminal.cursor-blink"));
}

// ---------------------------------------------------------------------------------------
// The check measures the new pairs
// ---------------------------------------------------------------------------------------

fn contrast_findings(report: &Report) -> Vec<(&'static str, &'static str)> {
    report
        .findings
        .iter()
        .filter_map(|one| match one {
            Finding::Contrast {
                tint: None, pair, ..
            } => Some((pair.front, pair.back)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_check_holds_code_drawn_too_close_to_the_editor_s_ground() {
    let ground = dark().value("terminal.background").to_owned();
    let theme = dark_with(json!({ "syntax.comment": ground })).theme;
    let found = contrast_findings(&check(theme.first()));
    assert!(found.contains(&("syntax.comment", "terminal.background")));
}

#[test]
fn the_check_does_not_measure_what_the_platform_draws() {
    // Classic leaves the selection, caret, placeholder and scrollbars to the platform, and passes.
    assert!(check(dark()).passes(), "{:?}", check(dark()).findings);
    assert!(check(light()).passes(), "{:?}", check(light()).findings);
    // A transparent caret is drawn as given, transparent, and the check says it cannot be seen.
    let theme = dark_with(json!({ "caret": "#0000" })).theme;
    let found = contrast_findings(&check(theme.first()));
    assert!(found.contains(&("caret", "surface.sunken")));
}

#[test]
fn the_check_measures_a_selection_the_theme_draws() {
    // A selection the theme colours, with the words left their own colour, is measured as the
    // window's words on it.
    let theme = dark_with(json!({ "selection.background": "#dddcd6" })).theme;
    let found = contrast_findings(&check(theme.first()));
    assert!(found.contains(&("selection.text", "selection.background")));
    // And with words it colours too, as those words.
    let theme = dark_with(json!({
        "selection.background": "#dddcd6",
        "selection.text": "#111111"
    }))
    .theme;
    assert!(check(theme.first()).passes());
}

#[test]
fn the_check_measures_the_terminal_s_selected_text_as_the_cells_when_the_platform_draws_it() {
    // Left to the platform, the selected cells keep the terminal's own foreground, which is
    // measured.
    let theme = dark_with(json!({ "terminal.selection": "#dddcd6ff" })).theme;
    let found = contrast_findings(&check(theme.first()));
    assert!(found.contains(&("terminal.selection-foreground", "terminal.selection")));
    let theme = dark_with(json!({
        "terminal.selection": "#dddcd6ff",
        "terminal.selection-inactive": "#dddcd6ff",
        "terminal.selection-foreground": "#191917"
    }))
    .theme;
    assert!(check(theme.first()).passes(), "{:?}", check(theme.first()));
}

#[test]
fn only_what_the_platform_draws_may_be_left_to_it() {
    let loaded = dark_with(json!({ "caret": "platform", "scrollbar.thumb": "#80808080" }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    assert_eq!(loaded.theme.first().value("caret"), "platform");
    assert_eq!(loaded.theme.first().value("scrollbar.thumb"), "#80808080");
    let loaded = dark_with(json!({ "accent.base": "platform" }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "accent.base in appearance dark is \"platform\", which is neither a hex colour nor a name in this theme's palette"
        ]
    );
}

#[test]
fn no_palette_may_name_a_colour_platform() {
    let loaded = load(&json!({
        "name": "Taken",
        "extends": "purlis-classic",
        "palette": { "platform": "#123456" },
        "appearances": [{ "id": "dark", "tokens": { "caret": "platform" } }]
    }));
    assert!(loaded.refused);
    assert_eq!(
        said(&loaded),
        [
            "palette name \"platform\" is purlis's own word for what the platform draws, so a palette cannot name a colour by it"
        ]
    );
}

#[test]
fn the_check_says_selected_text_on_the_platform_s_selection_is_never_drawn() {
    let theme = dark_with(json!({ "selection.text": "#111111" })).theme;
    assert!(
        check(theme.first())
            .findings
            .contains(&Finding::SelectionTextAlone)
    );
    let theme = dark_with(json!({
        "selection.background": "#dddcd6",
        "selection.text": "#111111"
    }))
    .theme;
    assert!(
        !check(theme.first())
            .findings
            .contains(&Finding::SelectionTextAlone)
    );
}

// ---------------------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------------------

#[test]
fn the_schema_types_each_token_that_is_not_a_colour() {
    let schema = schema();
    let tokens = &schema["$defs"]["tokens"]["properties"];
    for flag in FLAGS {
        assert_eq!(tokens[*flag]["type"], "boolean", "{flag}");
    }
    assert_eq!(
        tokens["terminal.cursor-style"]["enum"],
        json!(["block", "underline", "bar"])
    );
    assert_eq!(tokens["terminal.line-height"]["minimum"], 1.0);
    assert_eq!(tokens["terminal.line-height"]["maximum"], 2.0);
    assert_eq!(tokens["syntax.keyword"]["$ref"], "#/$defs/colour");
    assert_eq!(tokens["caret"]["anyOf"][1]["const"], "platform");
}

// ---------------------------------------------------------------------------------------
// A theme option carries typed tokens and "platform" as an appearance does
// ---------------------------------------------------------------------------------------

#[test]
fn a_theme_option_sets_typed_tokens_and_leaves_what_the_platform_draws_to_it() {
    let loaded = load(&json!({
        "name": "Options",
        "extends": "purlis-classic",
        "appearances": [{ "id": "dark", "tokens": { "caret": "#ff0000" } }],
        "options": [{
            "id": "terminal", "kind": "toggle", "label": "Thin cursor", "default": false,
            "on": { "tokens": {
                "terminal.cursor-style": "bar",
                "terminal.cursor-blink": true,
                "syntax.keyword.bold": true,
                "caret": "platform"
            } }
        }]
    }));
    assert!(!loaded.refused, "{:?}", said(&loaded));
    let theme = loaded.theme;
    let off = theme.first();
    assert_eq!(off.typed.choice("terminal.cursor-style"), "block");
    assert_eq!(off.value("caret"), "#ff0000");
    let values: Values = [("terminal".to_owned(), "on".to_owned())]
        .into_iter()
        .collect();
    let (on, complaints) = theme.with(&values);
    assert!(complaints.is_empty(), "{complaints:?}");
    let on = on.first();
    assert_eq!(on.typed.choice("terminal.cursor-style"), "bar");
    assert!(on.typed.flag("terminal.cursor-blink"));
    assert!(on.typed.flag("syntax.keyword.bold"));
    assert_eq!(on.value("caret"), PLATFORM);
}

#[test]
fn a_theme_option_s_typed_value_of_the_wrong_type_refuses_the_theme() {
    let loaded = load(&json!({
        "name": "Options",
        "extends": "purlis-classic",
        "options": [{
            "id": "terminal", "kind": "toggle", "label": "Tall lines", "default": false,
            "on": { "tokens": { "terminal.line-height": 9 } }
        }]
    }));
    assert!(loaded.refused);
    assert!(
        said(&loaded)
            .iter()
            .any(|one| one.starts_with("terminal.line-height in ")
                && one.ends_with("is 9, which is not a number from 1 to 2")),
        "{:?}",
        said(&loaded)
    );
}
