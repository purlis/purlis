// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn asked(personas: Value, focus: Option<&str>) -> Value {
    answer(&json!({
        "charter": 1, "extension": "persona-statistics", "view": "statistics",
        "about": "personas", "focus": focus,
        "given": { "now": "2026-09-23 12:00", "personas": personas, "truncated": false },
    }))
}

fn chart<'a>(said: &'a Value, title: &str) -> &'a Value {
    said["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|block| block["title"] == title)
        .unwrap_or_else(|| panic!("no chart called {title}: {said}"))
}

fn plane() -> Value {
    json!([
        { "name": "steward", "default": true, "refused": null,
          "written": ["2026-09-22 10:00", "2026-09-21 09:00", "2026-08-01 12:00"] },
        { "name": "release", "default": false, "refused": null,
          "written": ["2026-07-01 12:00"] },
        { "name": "forge", "default": false, "refused": null, "written": [] },
    ])
}

#[test]
fn memories_per_persona_are_bars_most_first_with_the_default_said() {
    let said = asked(plane(), None);
    let bars = chart(&said, "Memories per persona");

    assert_eq!(bars["shape"], "bars");
    let points = bars["points"].as_array().expect("points");
    assert_eq!(points[0]["label"], "steward");
    assert_eq!(points[0]["value"], 3);
    assert_eq!(points[0]["note"], "default · 75%");
    assert_eq!(points[1]["label"], "release");
    assert_eq!(points[2]["value"], 0);
}

#[test]
fn the_weekly_columns_are_twelve_weeks_ending_this_one() {
    let said = asked(plane(), None);
    let columns = chart(&said, "Written each week");
    let points = columns["points"].as_array().expect("points");

    assert_eq!(points.len(), 12);
    // 2026-09-23 is a Wednesday, so this week began on Monday the 21st.
    assert_eq!(points[11]["label"], "Sep 21");
    assert_eq!(
        points[11]["value"], 2,
        "the 21st and the 22nd are this week"
    );
    assert_eq!(points[10]["value"], 0);
    assert_eq!(points[10]["label"], "Sep 14");
}

#[test]
fn the_quietest_persona_comes_first_and_one_that_never_learned_is_said() {
    let said = asked(plane(), None);
    let quiet = chart(&said, "Days since each last learned something");
    let points = quiet["points"].as_array().expect("points");

    assert_eq!(points[0]["label"], "release");
    assert_eq!(points[0]["value"], 84);
    assert_eq!(points[1]["label"], "steward");
    let text = said.to_string();
    assert!(text.contains("Nothing remembered yet: forge."), "{text}");
}

#[test]
fn a_focus_answers_about_that_persona_first() {
    let said = asked(plane(), Some("steward"));
    let blocks = said["blocks"].as_array().expect("blocks");

    assert!(
        blocks[0]["text"]
            .as_str()
            .expect("a sentence")
            .starts_with("steward remembers 3 things · 2 in the last 14 days"),
        "{said}"
    );
    chart(&said, "steward — written each week");
}

#[test]
fn an_empty_plane_is_a_sentence_and_not_an_empty_chart() {
    let said = asked(json!([]), None);
    assert_eq!(said["blocks"].as_array().expect("blocks").len(), 1);
    assert!(said.to_string().contains("no personas yet"));
}

#[test]
fn a_request_in_another_protocol_is_answered_with_an_error_not_a_guess() {
    let said = answer(&json!({ "charter": 2, "about": "personas" }));
    assert!(
        said["error"]
            .as_str()
            .expect("an error")
            .contains("protocol 2")
    );
}

#[test]
fn a_persona_charter_could_not_read_is_said_as_trouble() {
    let said = asked(
        json!([{ "name": "x", "default": false, "written": [], "refused": "a link" }]),
        None,
    );
    assert!(
        said.to_string()
            .contains("could not read x's memories: a link")
    );
}

#[test]
fn more_personas_than_a_chart_holds_are_folded_into_one_bar() {
    let many: Vec<Value> = (0..40)
        .map(|n| json!({ "name": format!("p{n:02}"), "written": ["2026-09-01 00:00"] }))
        .collect();
    let said = asked(Value::Array(many), None);
    let points = chart(&said, "Memories per persona")["points"]
        .as_array()
        .expect("points")
        .clone();
    assert_eq!(points.len(), MOST_BARS + 1);
    assert_eq!(points[MOST_BARS]["label"], "16 others");
}

#[test]
fn the_manifest_it_is_installed_with_declares_this_program_and_one_view() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("JSON");
    assert_eq!(manifest["contributes"]["runs"], "bin/persona-statistics");
    assert_eq!(manifest["contributes"]["views"][0]["about"], "personas");
}
