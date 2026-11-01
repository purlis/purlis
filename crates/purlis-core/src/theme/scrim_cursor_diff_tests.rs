// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The tokens the check did not hold to a floor until #2987: the modal scrim, the terminal's
//! cursor and the words on the diff's washes, as a theme writes them and as a person's own
//! colours set them.

use serde_json::{Value, json};

use super::super::{
    Appearance, Drawn, Person, ResolvedTheme, built_ins, check, classic, draw, load,
};
use super::{Finding, PAIRS, Pair, Report, SCRIM_DIM, drawn, ratio, scrim_dim};

/// Every pair #2987 holds: `front`, `back`, what `back` is laid over, and the floor.
const HELD: [(&str, &str, Option<&str>, f64); 6] = [
    ("terminal.cursor", "terminal.background", None, 3.0),
    ("terminal.cursor-accent", "terminal.cursor", None, 4.5),
    (
        "terminal.foreground",
        "diff.inserted",
        Some("terminal.background"),
        4.5,
    ),
    (
        "terminal.foreground",
        "diff.deleted",
        Some("terminal.background"),
        4.5,
    ),
    (
        "terminal.foreground",
        "diff.inserted-text",
        Some("diff.inserted"),
        4.5,
    ),
    (
        "terminal.foreground",
        "diff.deleted-text",
        Some("diff.deleted"),
        4.5,
    ),
];

fn pair_of(front: &str, back: &str) -> &'static Pair {
    PAIRS
        .iter()
        .find(|pair| pair.front == front && pair.back == back)
        .unwrap_or_else(|| panic!("{front} on {back} is held"))
}

fn classic_dark() -> &'static ResolvedTheme {
    classic().first()
}

fn dark(tokens: Value) -> ResolvedTheme {
    let resolved = load(&json!({
        "name": "Probe",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": tokens }]
    }));
    assert!(!resolved.refused, "{:?}", resolved.complaints);
    resolved.theme.first().clone()
}

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

fn thin_scrim(report: &Report) -> Option<f64> {
    report.findings.iter().find_map(|one| match one {
        Finding::ScrimTooThin {
            tint: None, ratio, ..
        } => Some(*ratio),
        _ => None,
    })
}

fn under_person(colours: Value) -> Drawn {
    let (person, complaints) = Person::read(Some(&json!({ "colours": colours })));
    assert!(complaints.is_empty(), "{complaints:?}");
    draw(classic_dark()).under(&person.layer())
}

/// A person's `token`, set to `colour`, is left out with a sentence that names it, and the
/// theme's is drawn.
fn left_out(token: &str, colour: &str) {
    let mut colours = serde_json::Map::new();
    colours.insert(token.to_owned(), json!(colour));
    let drawn = under_person(Value::Object(colours));
    assert_eq!(
        drawn.theme.expect("drawn").value(token),
        classic_dark().value(token),
        "{token}"
    );
    assert_eq!(drawn.mine.len(), 1, "{token}: {:?}", drawn.mine);
    assert_eq!(drawn.mine[0].token.as_deref(), Some(token));
    assert!(
        drawn.mine[0]
            .said
            .starts_with(&format!("your colour for {token}, {colour}, is left out")),
        "{}",
        drawn.mine[0].said
    );
}

// ---------------------------------------------------------------------------------------
// Each token is held, at the floor fit for what it draws
// ---------------------------------------------------------------------------------------

#[test]
fn each_pair_is_laid_as_it_is_drawn_at_the_floor_for_what_it_draws() {
    for (front, back, under, floor) in HELD {
        let pair = pair_of(front, back);
        assert_eq!(pair.under, under, "{front} on {back}");
        assert_eq!(pair.floor, floor, "{front} on {back}");
    }
    // A changed word's wash is laid on its line's, on the editor's ground.
    for back in ["diff.inserted-text", "diff.deleted-text"] {
        assert_eq!(
            pair_of("terminal.foreground", back).beneath,
            Some("terminal.background")
        );
    }
}

#[test]
fn a_cursor_the_colour_of_the_terminal_falls() {
    let theme = dark(json!({ "terminal.background": "#202020", "terminal.cursor": "#202020" }));
    assert_eq!(
        found(&check(&theme), "terminal.cursor", "terminal.background"),
        Some(1.0)
    );
}

#[test]
fn a_cursor_a_little_off_the_terminal_falls_and_at_its_floor_it_does_not() {
    let near = dark(json!({ "terminal.background": "#000000", "terminal.cursor": "#404040" }));
    let ratio = found(&check(&near), "terminal.cursor", "terminal.background");
    assert!(ratio.is_some_and(|it| it < 3.0), "{ratio:?}");
    let far = dark(json!({ "terminal.background": "#000000", "terminal.cursor": "#808080" }));
    assert_eq!(
        found(&check(&far), "terminal.cursor", "terminal.background"),
        None
    );
}

#[test]
fn the_cell_under_a_block_cursor_the_colour_of_the_cursor_falls() {
    let theme = dark(json!({ "terminal.cursor": "#dddddd", "terminal.cursor-accent": "#dddddd" }));
    assert_eq!(
        found(&check(&theme), "terminal.cursor-accent", "terminal.cursor"),
        Some(1.0)
    );
}

#[test]
fn words_the_colour_of_a_changed_line_s_wash_fall() {
    for back in ["diff.inserted", "diff.deleted"] {
        let mut tokens = serde_json::Map::new();
        tokens.insert("terminal.foreground".to_owned(), json!("#dddddd"));
        tokens.insert(back.to_owned(), json!("#dddddd"));
        let report = check(&dark(Value::Object(tokens)));
        assert_eq!(
            found(&report, "terminal.foreground", back),
            Some(1.0),
            "{back}"
        );
    }
}

#[test]
fn words_the_colour_of_a_changed_word_s_wash_fall() {
    for back in ["diff.inserted-text", "diff.deleted-text"] {
        let mut tokens = serde_json::Map::new();
        tokens.insert("terminal.foreground".to_owned(), json!("#dddddd"));
        tokens.insert(back.to_owned(), json!("#dddddd"));
        let report = check(&dark(Value::Object(tokens)));
        assert_eq!(
            found(&report, "terminal.foreground", back),
            Some(1.0),
            "{back}"
        );
    }
}

#[test]
fn a_changed_word_s_wash_is_measured_on_its_line_s_wash_and_not_on_the_ground_alone() {
    // Each wash alone leaves the words readable; only laid one on the other do they fall.
    let theme = dark(json!({
        "terminal.background": "#000000",
        "terminal.foreground": "#ffffff",
        "diff.inserted": "#ffffff80",
        "diff.inserted-text": "#ffffff80",
    }));
    let ratio = found(&check(&theme), "terminal.foreground", "diff.inserted-text");
    assert!(ratio.is_some_and(|it| it < 4.5), "{ratio:?}");
}

#[test]
fn a_scrim_that_lets_the_window_through_falls() {
    for scrim in ["#00000000", "#00000020"] {
        let report = check(&dark(json!({ "overlay.scrim": scrim })));
        let ratio = thin_scrim(&report);
        assert!(ratio.is_some_and(|it| it < SCRIM_DIM), "{scrim}: {ratio:?}");
    }
}

#[test]
fn a_scrim_is_measured_on_what_it_can_move_on_a_dark_window_and_on_a_light_one() {
    // On a dark window a black scrim cannot move the ground, so it is read on the words.
    let dark_window = dark(json!({
        "surface.base": "#111111",
        "text.primary": "#ffffff",
        "overlay.scrim": "#0000008c",
    }));
    assert_eq!(thin_scrim(&check(&dark_window)), None);
    assert!(scrim_dim(&dark_window) >= 3.0);
    // On a light window the words cannot move, so it is read on the ground.
    let light_window = dark(json!({
        "surface.base": "#ffffff",
        "text.primary": "#111111",
        "overlay.scrim": "#0000008c",
    }));
    assert_eq!(thin_scrim(&check(&light_window)), None);
    assert!(scrim_dim(&light_window) >= 3.0);
}

// ---------------------------------------------------------------------------------------
// The built-in themes, with a control that falls as written
// ---------------------------------------------------------------------------------------

#[test]
fn every_built_in_clears_each_pair_and_falls_when_its_front_takes_its_ground() {
    for theme in built_ins() {
        for (front, back, _, floor) in HELD {
            let pair = pair_of(front, back);
            let cleared = ratio(theme, pair).expect("measured");
            assert!(
                cleared >= floor,
                "{} {front} on {back}: {cleared}",
                theme.name
            );
            // The control: the same sweep, on the same theme, with the front as its ground.
            let (_, ground) = drawn(theme, pair).expect("drawn");
            let mut broken = theme.clone();
            broken.values.insert(front, ground);
            assert_eq!(
                ratio(&broken, pair),
                Some(1.0),
                "{} {front} on {back}",
                theme.name
            );
            assert!(
                check(&broken).findings.iter().any(|one| matches!(
                    one,
                    Finding::Contrast { pair: found, .. } if found == pair
                )),
                "{} {front} on {back} did not fall",
                theme.name
            );
        }
    }
}

#[test]
fn every_built_in_s_scrim_clears_its_floor_and_a_clear_one_falls() {
    for theme in built_ins() {
        assert!(
            scrim_dim(theme) >= SCRIM_DIM,
            "{}: {}",
            theme.name,
            scrim_dim(theme)
        );
        assert_eq!(thin_scrim(&check(theme)), None, "{}", theme.name);
        let mut broken = theme.clone();
        broken
            .values
            .insert("overlay.scrim", "#00000000".to_owned());
        assert_eq!(thin_scrim(&check(&broken)), Some(1.0), "{}", theme.name);
    }
}

#[test]
fn the_thinnest_shipped_scrim_is_purlis_classic_light_s() {
    // What the floor was chosen against (#2987): 3:1 is out of this measure's reach for this
    // appearance as it ships, and changing a shipped colour is not the check's to decide.
    let light = classic().of(Appearance::Light).expect("light");
    assert_eq!(scrim_dim(light), 2.44);
}

// ---------------------------------------------------------------------------------------
// A person's colour that breaks one is left out with a sentence
// ---------------------------------------------------------------------------------------

#[test]
fn a_person_s_cursor_on_the_terminal_s_colour_is_left_out_and_said() {
    left_out(
        "terminal.cursor",
        classic_dark().value("terminal.background"),
    );
}

#[test]
fn a_person_s_cursor_accent_on_the_cursor_s_colour_is_left_out_and_said() {
    left_out(
        "terminal.cursor-accent",
        classic_dark().value("terminal.cursor"),
    );
}

#[test]
fn a_person_s_diff_wash_under_the_words_is_left_out_and_said() {
    let words = classic_dark().value("terminal.foreground");
    for token in [
        "diff.inserted",
        "diff.deleted",
        "diff.inserted-text",
        "diff.deleted-text",
    ] {
        left_out(token, words);
    }
}

#[test]
fn a_person_s_thin_scrim_is_left_out_and_said() {
    left_out("overlay.scrim", "#00000010");
    let drawn = under_person(json!({ "overlay.scrim": "#00000010" }));
    assert!(
        drawn.mine[0].said.contains("under its floor of 2:1"),
        "{}",
        drawn.mine[0].said
    );
}

#[test]
fn a_person_s_scrim_that_dims_the_window_is_drawn() {
    let drawn = under_person(json!({ "overlay.scrim": "#000000cc" }));
    assert!(drawn.mine.is_empty(), "{:?}", drawn.mine);
    assert_eq!(
        drawn.theme.expect("drawn").value("overlay.scrim"),
        "#000000cc"
    );
}
