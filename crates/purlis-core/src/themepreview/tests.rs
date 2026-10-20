// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::{Map, Value, json};

use super::*;

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => unreachable!("arguments are an object"),
    }
}

fn specimen(theme: &str, appearance: Option<Appearance>) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
    }
}

#[test]
fn a_theme_alone_asks_for_one_specimen_in_its_first_appearance() {
    assert_eq!(
        asked(&args(json!({"theme": "purlis-classic"}))),
        Ok(vec![specimen("purlis-classic", None)])
    );
}

#[test]
fn purlis_classic_is_drawn_in_the_appearance_asked_for() {
    let dark = chosen(&specimen("purlis-classic", Some(Appearance::Dark))).expect("dark");
    let light = chosen(&specimen("purlis-classic", Some(Appearance::Light))).expect("light");
    let first = chosen(&specimen("purlis-classic", None)).expect("its first");
    assert_eq!(dark.name, "purlis-dark");
    assert_eq!(light.name, "purlis-light");
    assert_eq!(first.name, "purlis-dark");
}

#[test]
fn a_specimen_is_drawn_as_the_window_draws_its_theme_with_the_consent_floors_held() {
    for appearance in [None, Some(Appearance::Dark), Some(Appearance::Light)] {
        let asked = specimen("purlis-classic", appearance);
        let theme = chosen(&asked).expect("a theme");
        let drawn = drawn(&asked).expect("drawn");
        assert_eq!(drawn, theme::draw(theme));
        assert_eq!(
            drawn.held, None,
            "Purlis Classic clears every consent floor"
        );
    }
    let why = drawn(&specimen("no-such-theme", None)).expect_err("refused");
    assert_eq!(
        Err(why),
        chosen(&specimen("no-such-theme", None)).map(|_| ())
    );
}

#[test]
fn an_appearance_is_named_by_its_own_id_and_by_the_id_it_had_before_the_rename() {
    assert_eq!(
        chosen(&specimen("purlis-light", None)).map(|t| t.name.as_str()),
        Ok("purlis-light")
    );
    assert_eq!(
        chosen(&specimen("charter-dark", Some(Appearance::Dark))).map(|t| t.name.as_str()),
        Ok("purlis-dark")
    );
}

#[test]
fn an_appearance_a_one_appearance_theme_lacks_is_refused_naming_the_one_that_has_it() {
    let why = chosen(&specimen("purlis-light", Some(Appearance::Dark))).expect_err("refused");
    assert!(why.contains("only a light appearance"), "{why}");
    assert!(why.contains("purlis-classic"), "{why}");
}

#[test]
fn a_theme_this_purlis_does_not_have_is_refused_naming_the_ones_it_has() {
    let why = asked(&args(json!({"theme": "solarized"}))).expect_err("refused");
    assert!(why.contains("no theme `solarized`"), "{why}");
    for id in ["purlis-classic", "purlis-dark", "purlis-light"] {
        assert!(why.contains(id), "{why}");
    }
}

#[test]
fn no_theme_or_an_odd_value_is_refused_in_words_that_say_what_to_give() {
    assert!(
        asked(&args(json!({})))
            .expect_err("refused")
            .contains("give `theme`")
    );
    assert!(
        asked(&args(json!({"theme": 3})))
            .expect_err("refused")
            .contains("as text")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "appearance": "sepia"})
        ))
        .expect_err("refused")
        .contains("`dark`, `light` or `dark-high-contrast`")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "workspace": "alpha"})
        ))
        .expect_err("refused")
        .contains("not `workspace`")
    );
}

#[test]
fn side_by_side_asks_for_a_second_specimen_that_keeps_what_it_does_not_change() {
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis-classic",
            "appearance": "dark",
            "side_by_side": {"appearance": "light"},
        }))),
        Ok(vec![
            specimen("purlis-classic", Some(Appearance::Dark)),
            specimen("purlis-classic", Some(Appearance::Light)),
        ])
    );
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis-dark",
            "side_by_side": {"theme": "purlis-light"},
        }))),
        Ok(vec![
            specimen("purlis-dark", None),
            specimen("purlis-light", None)
        ])
    );
}

#[test]
fn side_by_side_that_changes_nothing_or_names_what_is_not_there_is_refused() {
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": {}})
        ))
        .expect_err("refused")
        .contains("say what the second specimen changes")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": {"theme": "nord"}})
        ))
        .expect_err("refused")
        .contains("no theme `nord`")
    );
    assert!(
        asked(&args(
            json!({"theme": "purlis-classic", "side_by_side": true})
        ))
        .expect_err("refused")
        .contains("is an object")
    );
}

#[test]
fn the_tool_takes_no_workspace_project_path_or_directory() {
    // The chat tools' boundary (`chattools`): names, never paths.
    let schema = (TOOLS[0].schema)();
    let text = schema.to_string();
    for word in ["workspace", "project", "path", "dir"] {
        assert!(!text.contains(&format!("\"{word}")), "{word} in {text}");
    }
    assert!(TOOLS[0].read_only);
    assert!(is_one(TOOL));
}

#[test]
fn the_model_is_told_what_each_picture_is_and_that_it_is_sample_content() {
    let painted = |theme: &str, appearance| Painted {
        theme: theme.to_owned(),
        appearance,
        high_contrast: false,
        preview: false,
        width: 2560,
        height: 1600,
        millis: 640,
        png: vec![1, 2, 3],
    };
    let one = said(&[painted("purlis-classic", Appearance::Dark)]);
    assert!(
        one.starts_with(
            "The theme specimen in purlis-classic (dark), 2560×1600 pixels, drawn in 640 ms."
        ),
        "{one}"
    );
    assert!(one.contains("never the person's window"), "{one}");
    let two = said(&[
        painted("purlis-classic", Appearance::Dark),
        painted("purlis-classic", Appearance::Light),
    ]);
    assert!(
        two.starts_with(
            "Two theme specimens, in the order of their pictures: purlis-classic (dark)"
        ),
        "{two}"
    );
    assert!(two.contains("then purlis-classic (light)"), "{two}");
}

#[test]
fn a_picture_never_travels_inside_the_answer_s_json() {
    let painted = Painted {
        theme: "purlis-classic".to_owned(),
        appearance: Appearance::Light,
        high_contrast: false,
        preview: false,
        width: 1,
        height: 1,
        millis: 1,
        png: vec![0x89, b'P', b'N', b'G'],
    };
    let wire = serde_json::to_value(&painted).expect("JSON");
    assert_eq!(
        wire,
        json!({"theme": "purlis-classic", "appearance": "light", "width": 1, "height": 1, "millis": 1})
    );
}

fn sharp(theme: &str) -> Specimen {
    Specimen {
        theme: theme.to_owned(),
        appearance: Some(Appearance::Dark),
        high_contrast: true,
    }
}

#[test]
fn purlis_is_drawn_in_its_high_contrast_dark_when_that_is_asked_for() {
    // #2709: the tool draws every appearance Purlis has.
    assert_eq!(
        asked(&args(
            json!({"theme": "purlis", "appearance": "dark-high-contrast"})
        )),
        Ok(vec![sharp("purlis")])
    );
    let theme = chosen(&sharp("purlis")).expect("its high-contrast dark");
    assert_eq!(
        (theme.name.as_str(), theme.id.as_str()),
        ("Purlis", "dark-high-contrast")
    );
    assert!(theme.high_contrast);
    assert_eq!(drawn(&sharp("purlis")).expect("drawn").held, None);
    // Its plain dark stays the one tagged dark and not high contrast.
    let dark = chosen(&specimen("purlis", Some(Appearance::Dark))).expect("dark");
    assert_eq!((dark.id.as_str(), dark.high_contrast), ("dark", false));
}

#[test]
fn a_theme_with_no_high_contrast_appearance_is_refused_naming_the_one_that_has_it() {
    for theme in ["purlis-classic", "purlis-dark"] {
        let why = chosen(&sharp(theme)).expect_err("refused");
        assert!(why.contains("no high-contrast appearance"), "{why}");
        assert!(
            why.contains("purlis with appearance `dark-high-contrast`"),
            "{why}"
        );
    }
}

#[test]
fn side_by_side_compares_purlis_s_dark_with_its_high_contrast_dark() {
    assert_eq!(
        asked(&args(json!({
            "theme": "purlis",
            "appearance": "dark",
            "side_by_side": {"appearance": "dark-high-contrast"},
        }))),
        Ok(vec![
            specimen("purlis", Some(Appearance::Dark)),
            sharp("purlis")
        ])
    );
    // A second specimen that changes only the theme keeps the first's appearance, high contrast
    // and all, and is refused when that theme has none.
    let why = asked(&args(json!({
        "theme": "purlis",
        "appearance": "dark-high-contrast",
        "side_by_side": {"theme": "purlis-classic"},
    })))
    .expect_err("refused");
    assert!(why.contains("no high-contrast appearance"), "{why}");
}

#[test]
fn the_tool_says_purlis_is_a_preview_and_names_its_high_contrast_dark() {
    let schema = (TOOLS[0].schema)();
    assert_eq!(
        schema["properties"]["appearance"]["enum"],
        json!(["dark", "light", "dark-high-contrast"])
    );
    assert_eq!(
        schema["properties"]["side_by_side"]["properties"]["appearance"]["enum"],
        json!(["dark", "light", "dark-high-contrast"])
    );
    let theme = schema["properties"]["theme"]["description"]
        .as_str()
        .unwrap();
    assert!(theme.contains("purlis, Purlis, a preview theme"), "{theme}");
    assert!(TOOLS[0].description.contains("preview theme"));
    let painted = Painted {
        theme: "purlis".to_owned(),
        appearance: Appearance::Dark,
        high_contrast: true,
        preview: true,
        width: 2560,
        height: 1600,
        millis: 640,
        png: Vec::new(),
    };
    let one = said(std::slice::from_ref(&painted));
    assert!(
        one.starts_with(
            "The theme specimen in purlis (a preview theme, dark-high-contrast), 2560×1600 pixels"
        ),
        "{one}"
    );
    assert_eq!(
        serde_json::to_value(&painted).expect("JSON"),
        json!({
            "theme": "purlis", "appearance": "dark", "high_contrast": true, "preview": true,
            "width": 2560, "height": 1600, "millis": 640
        })
    );
}

#[test]
fn an_ask_without_high_contrast_reads_as_it_did() {
    // The ask travels from the chat's tool to the app: one written before #2709 still reads.
    let old: Specimen =
        serde_json::from_value(json!({"theme": "purlis", "appearance": "dark"})).expect("read");
    assert_eq!(old, specimen("purlis", Some(Appearance::Dark)));
    assert_eq!(
        serde_json::to_value(&old).expect("JSON"),
        json!({"theme": "purlis", "appearance": "dark"})
    );
    assert_eq!(
        serde_json::to_value(sharp("purlis")).expect("JSON"),
        json!({"theme": "purlis", "appearance": "dark", "high_contrast": true})
    );
}
