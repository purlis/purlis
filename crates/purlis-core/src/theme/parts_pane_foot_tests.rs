// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The pane foot at the engine's interface (#5202): in the manifest since the next version, with
//! each part look A draws, drawn on the terminal's fill.

use serde_json::json;

use super::*;

#[test]
fn the_pane_foot_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let foot = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "pane-foot")
        .expect("the pane foot is in the manifest");
    assert_eq!(foot["since"], 13);
    let names: Vec<&str> = foot["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "root",
            "start",
            "end",
            "separator",
            "gauge",
            "label",
            "value",
            "track",
            "fill"
        ]
    );
    assert_eq!(foot["variants"].as_array().map(Vec::len), Some(0));
}

#[test]
fn the_pane_foot_is_drawn_on_the_terminals_fill() {
    let foot = parts::MANIFEST
        .iter()
        .find(|one| one.name == "pane-foot")
        .expect("the pane foot is in the manifest");
    assert_eq!(foot.grounds, ["terminal.background"]);
}

// ---------------------------------------------------------------------------------------
// The foot's warning number, held as words on the terminal's fill (#5262)
// ---------------------------------------------------------------------------------------

/// The foot's stylesheet, whose toned rules say which colours its number is drawn in.
const SHEET: &str = include_str!("../../../../app/src/ui/pane-foot/pane-foot.css");

/// Every colour `pane-foot.css` draws a toned number's words in that the consent floors hold as
/// a state's: each `[data-part="value"][data-tone="…"]` rule's `color`.
fn toned_number_colours() -> Vec<String> {
    SHEET
        .split("[data-scope=\"pane-foot\"][data-part=\"value\"][data-tone=")
        .skip(1)
        .filter_map(|rule| {
            let body = rule.split('{').nth(1)?.split('}').next()?;
            let colour = body.split("color: var(--").nth(1)?.split(')').next()?;
            // `--state-waiting` is the token `state.waiting`.
            Some(colour.replacen('-', ".", 1))
        })
        .filter(|token| token.starts_with("state.") || token.starts_with("danger."))
        .collect()
}

/// The colours the foot's warning number is drawn in.
const WARNING: [&str; 2] = ["state.waiting", "state.failed"];

#[test]
fn the_foot_draws_its_warning_number_in_the_waiting_and_the_failed_colours() {
    assert_eq!(toned_number_colours(), WARNING);
}

#[test]
fn a_toned_number_is_held_as_words_on_the_terminals_fill() {
    let text = check::aa_text(14.0, false);
    let foot = parts::MANIFEST
        .iter()
        .find(|one| one.name == "pane-foot")
        .expect("the pane foot is in the manifest");
    let missing: Vec<String> = toned_number_colours()
        .iter()
        .flat_map(|front| foot.grounds.iter().map(move |back| (front.clone(), *back)))
        .filter(|(front, back)| {
            !check::PAIRS.iter().any(|pair| {
                pair.front == front
                    && pair.back == *back
                    && pair.under.is_none()
                    && pair.fade.is_none()
                    && pair.floor >= text
            })
        })
        .map(|(front, back)| format!("{front} on {back}"))
        .collect();
    assert!(missing.is_empty(), "held below {text}:1: {missing:?}");
}

/// Purlis Classic dark with a terminal a step lighter than its controls: the waiting colour
/// clears 3:1 on it (3.88:1) and every surface it was held on before, the failed one only 2.92:1.
fn a_lighter_terminal() -> ResolvedTheme {
    let resolved = load(&json!({
        "name": "Lighter terminal",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "terminal.background": "#44443f"
        } }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

#[test]
fn a_theme_whose_terminal_hides_the_foot_s_warning_is_warned_by_the_check() {
    let named: Vec<(&str, &str)> = check(&a_lighter_terminal())
        .findings
        .iter()
        .filter_map(|one| match one {
            Finding::Contrast {
                tint: None, pair, ..
            } if pair.back == "terminal.background" => Some((pair.front, pair.back)),
            _ => None,
        })
        .collect();
    for front in WARNING {
        assert!(
            named.contains(&(front, "terminal.background")),
            "{front}: {named:?}"
        );
    }
}

#[test]
fn a_theme_whose_terminal_hides_the_foot_s_warning_is_drawn_as_written() {
    // No consent floor is held on the terminal's fill: its amber and its terminal stay the
    // theme's, and the check above is what says so.
    let theme = a_lighter_terminal();
    let drawn = draw(&theme);
    let window = drawn.theme.expect("a theme is drawn");
    for token in ["terminal.background", "state.waiting", "state.failed"] {
        assert_eq!(window.value(token), theme.value(token), "{token}");
    }
    assert_eq!(drawn.held, None);
}

#[test]
fn every_shipped_theme_reads_the_foot_s_warning_on_its_terminal() {
    let text = check::aa_text(14.0, false);
    for theme in built_ins().iter().chain(&purlis().appearances) {
        for front in WARNING {
            let ratio = check::contrast(theme.value(front), theme.value("terminal.background"));
            assert!(ratio >= text, "{} ({}): {front} {ratio:.2}:1", theme.name, theme.id);
        }
    }
}

#[test]
fn a_theme_s_colour_for_the_foot_s_number_never_reaches_a_toned_one() {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis-classic",
        "parts": { "pane-foot.value.text": "text.primary" }
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let sheet = parts::stylesheet(loaded.theme.first());
    assert!(
        sheet.contains("[data-scope=\"pane-foot\"][data-part=\"value\"]:not([data-tone])"),
        "{sheet}"
    );
}

#[test]
fn the_foot_s_warning_never_repaints_a_terminal_on_the_other_side_of_the_window() {
    // A dark window with a light terminal, which the tier floors already hold apart: no waiting
    // colour of Classic's dark clears it, and the terminal is the person's to read a program's
    // output on, so it stays as written and the check says the foot's warning is hard to read.
    let resolved = load(&json!({
        "name": "Light terminal",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {
            "terminal.background": "#f4f4f0"
        } }]
    }));
    let theme = resolved.theme.first();
    let window = draw(theme).theme.expect("a theme is drawn");
    assert_eq!(window.value("terminal.background"), "#f4f4f0");
    assert!(
        check(theme).findings.iter().any(|one| matches!(
            one,
            Finding::Contrast { tint: None, pair, .. }
                if pair.front == "state.waiting" && pair.back == "terminal.background"
        )),
        "the check names the foot's warning on the light terminal"
    );
}

#[test]
fn only_the_foot_s_warning_is_left_out_of_the_consent_floors() {
    // Every state, danger and needs-you pair of the check is a consent floor but the two the
    // foot's number draws: a new one on the terminal is held as the rest are.
    let consent: Vec<&check::Pair> = floors::pairs().map(|(_, pair)| pair).collect();
    let left_out: Vec<(&str, &str)> = check::PAIRS
        .iter()
        .filter(|pair| ["state.", "danger.", "needs-you."].iter().any(|one| pair.front.starts_with(one)))
        .filter(|pair| !consent.contains(pair))
        .map(|pair| (pair.front, pair.back))
        .collect();
    assert_eq!(
        left_out,
        [
            ("state.waiting", "terminal.background"),
            ("state.failed", "terminal.background")
        ]
    );
}
