// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The chat mark family at the engine's interface (#5340): in the manifest since the next version
//! with its eight Chat states in the order that wins, and a theme reaching one state's badge.

use serde_json::json;

use super::parts;
use super::*;

const ROOT: &str = "[data-scope=\"chat-mark\"][data-part=\"root\"]";

#[test]
fn the_chat_mark_is_in_the_manifest_with_its_states_in_the_order_that_wins() {
    let manifest = parts::manifest_json();
    let family = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "chat-mark")
        .expect("the chat mark is in the manifest");
    assert_eq!(family["since"], parts::NEXT);
    let names = |key: &str| -> Vec<String> {
        family[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("parts"), ["root", "who", "badge", "word"]);
    assert_eq!(
        names("variants"),
        [
            "needs",
            "stuck",
            "failed",
            "working",
            "waiting",
            "idle",
            "ended",
            "cant-tell"
        ]
    );
}

#[test]
fn a_theme_reaches_each_state_s_badge_and_draws_no_rule_for_one_it_leaves_alone() {
    let loaded = load(&json!({
        "name": "Shaped",
        "extends": "purlis",
        "parts": { "chat-mark.badge.failed.text": "text.primary" },
    }));
    assert!(!loaded.refused, "{:?}", loaded.complaints);
    let sheet = parts::stylesheet(loaded.theme.first());
    let drawn = |variant: &str| sheet.contains(&format!("{ROOT}[data-variant=\"{variant}\"]"));
    assert!(drawn("failed"), "{sheet}");
    assert!(!drawn("idle"), "{sheet}");
}
