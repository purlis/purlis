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
        .contains("`dark` or `light`")
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
