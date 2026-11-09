// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::*;

fn one(template: Value) -> Result<Declared, String> {
    declared_of(&json!([template])).map(|mut all| all.remove(0))
}

fn board() -> Value {
    json!({
        "name": "board",
        "title": "Slack board",
        "folder": "artifacts/board",
        "icon": "mail",
    })
}

#[test]
fn a_template_declares_its_name_title_folder_and_icon() {
    let declared = one(board()).expect("declared");
    assert_eq!(declared.name, "board");
    assert_eq!(declared.title, "Slack board");
    assert_eq!(declared.folder, "artifacts/board");
    assert_eq!(declared.icon.as_deref(), Some("mail"));
    let bare = one(json!({ "name": "b", "title": "B", "folder": "b" })).expect("declared");
    assert_eq!(bare.icon, None, "no icon is purlis's default at the pin");
}

#[test]
fn a_template_s_folder_is_its_own_relative_path_and_never_leads_out() {
    for folder in [
        "../out",
        "/abs",
        "a/../b",
        "./a",
        "",
        "purlis-extension.json",
    ] {
        let why = one(json!({ "name": "b", "title": "B", "folder": folder })).unwrap_err();
        assert!(
            why.contains("declares the artifact template"),
            "{folder}: {why}"
        );
    }
}

#[test]
fn a_template_is_refused_for_a_bad_name_title_or_icon_an_unknown_key_or_a_twin() {
    let refused = |template: Value, said: &str| {
        let why = one(template).unwrap_err();
        assert!(why.contains(said), "{why}");
    };
    refused(
        json!({ "name": "a.b", "title": "B", "folder": "b" }),
        "a template's name",
    );
    refused(json!({ "name": "b", "folder": "b" }), "with no title");
    refused(
        json!({ "name": "b", "title": "x".repeat(200), "folder": "b" }),
        "a title purlis will not draw",
    );
    refused(
        json!({ "name": "b", "title": "B", "folder": "b", "icon": "skull-and-bones" }),
        "not one of purlis's icons",
    );
    refused(
        json!({ "name": "b", "title": "B", "folder": "b", "command": "rm -rf /" }),
        "\"command\"",
    );
    let why = declared_of(&json!([board(), board()])).unwrap_err();
    assert!(
        why.contains("two artifact templates called \"board\""),
        "{why}"
    );
    let many: Vec<Value> = (0..=MOST_TEMPLATES)
        .map(|n| json!({ "name": format!("t{n}"), "title": "T", "folder": "t" }))
        .collect();
    assert!(declared_of(&Value::Array(many)).is_err());
}

#[test]
fn the_prompt_names_each_template_by_its_id_title_and_folder() {
    let manifest = super::super::parse(
        &json!({
            "version": 8,
            "id": "slack",
            "name": "Slack",
            "capabilities": ["artifacts"],
            "contributes": { "artifacts": [board()] },
        })
        .to_string(),
    )
    .expect("parsed");
    assert_eq!(
        declares(&manifest),
        ["an artifact template, “slack.board” — “Slack board”, from its folder artifacts/board"]
    );
}

#[test]
fn a_template_needs_protocol_8_and_the_word_to_declare_one() {
    let manifest = |version: u64, capabilities: Value| {
        super::super::parse(
            &json!({
                "version": version,
                "id": "slack",
                "name": "Slack",
                "capabilities": capabilities,
                "contributes": { "artifacts": [board()] },
            })
            .to_string(),
        )
    };
    let why = manifest(7, json!(["artifacts"])).unwrap_err();
    assert!(why.contains("needs protocol 8"), "{why}");
    assert!(manifest(8, json!([])).is_err(), "a shape without its word");
}

#[test]
fn a_template_s_id_is_the_extension_s_then_its_name_and_its_folder_is_the_id() {
    assert_eq!(split("slack.board"), Some(("slack", "board")));
    assert_eq!(split("acme.slack.board"), Some(("acme.slack", "board")));
    assert_eq!(split("board"), None);
    assert_eq!(
        split("purlis.board"),
        None,
        "the core's source is no extension"
    );
    assert_eq!(split("slack.a/b"), None);
}

#[test]
fn where_purlis_starts_no_extension_a_template_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let why = pin_where(
        false,
        dir.path(),
        &BuiltIn::none(),
        &Choices::default(),
        dir.path(),
        &ws,
        &Wanted {
            id: "slack.board",
            name: None,
            icon: None,
        },
    )
    .unwrap_err();
    assert!(why.contains("not on this platform"), "{why}");
    assert!(!ws.join(crate::artifact::DIR).exists());
}
