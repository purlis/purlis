// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The message family at the engine's interface (#5649): in the manifest since the next version,
//! with each part a row draws and each state its mark says.

use super::parts;

#[test]
fn the_message_row_is_in_the_manifest_since_next() {
    let manifest = parts::manifest_json();
    let message = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "message")
        .expect("the message family is in the manifest");
    assert_eq!(message["since"], parts::NEXT);
    let names = |key: &str| -> Vec<String> {
        message[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        names("parts"),
        [
            "root", "mark", "main", "top", "title", "time", "meta", "state", "body", "links",
            "form", "dismiss"
        ]
    );
    assert_eq!(
        names("variants"),
        ["waiting", "done", "failed", "report", "plain"]
    );
}

/// **A theme sets a row's fill under the pointer and the keyboard, and its mark's tint** (#5649):
/// the states `message.css` draws are named, and so are the fills it draws in them.
#[test]
fn a_theme_sets_the_rows_hover_and_focus_fills_and_each_marks_tint() {
    let manifest = parts::manifest_json();
    let message = manifest["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["name"] == "message")
        .unwrap();
    let states: Vec<&str> = message["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|one| one.as_str().unwrap())
        .collect();
    assert_eq!(states, ["hover", "focused"]);
    let props = |part: &str| -> Vec<String> {
        message["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["name"] == part)
            .unwrap()["properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|one| one.as_str().unwrap().to_owned())
            .collect()
    };
    assert!(props("root").contains(&"fill".to_owned()));
    assert!(props("mark").contains(&"fill".to_owned()));
    assert!(props("state").contains(&"text".to_owned()));
}
