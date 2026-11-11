// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::*;

fn one(query: Value) -> Result<Declared, String> {
    declared_of(&json!([query])).map(|mut all| all.remove(0))
}

fn unread() -> Value {
    json!({
        "name": "unread",
        "description": "Unread messages in the workspace's channels.",
        "scope": "workspace",
        "fresh_seconds": 60,
        "parameters": {
            "type": "object",
            "properties": { "channel": { "type": "string", "description": "One channel." } },
        },
        "result": { "type": "object" },
        "widens": { "channel": "a channel the workspace does not follow" },
    })
}

#[test]
fn a_query_declares_its_name_description_scope_freshness_parameters_and_result() {
    let declared = one(unread()).expect("declared");
    assert_eq!(declared.name, "unread");
    assert_eq!(declared.scope, Scope::Workspace);
    assert_eq!(declared.fresh_seconds, 60);
    assert_eq!(
        declared.parameters,
        json!({
            "type": "object",
            "properties": { "channel": { "type": "string", "description": "One channel." } },
            "required": [],
            "additionalProperties": false,
        }),
        "normalised to what the catalogue checks a call against"
    );
    assert_eq!(
        declared.widens,
        [(
            "channel".to_owned(),
            "a channel the workspace does not follow".to_owned()
        )]
    );
    let bare = one(json!({
        "name": "everything",
        "description": "Everything.",
        "scope": "plane",
        "fresh_seconds": 1,
        "result": { "type": "object" },
    }))
    .expect("declared");
    assert_eq!(bare.scope, Scope::Plane);
    assert_eq!(bare.parameters["properties"], json!({}));
}

#[test]
fn a_query_said_wrong_is_refused_with_a_sentence_naming_it() {
    let with = |key: &str, value: Value| {
        let mut query = unread();
        query[key] = value;
        one(query).expect_err(key)
    };
    let without = |key: &str| {
        let mut query = unread();
        query.as_object_mut().unwrap().remove(key);
        one(query).expect_err(key)
    };
    assert!(without("description").contains("no description"));
    assert!(without("scope").contains("'scope' is workspace or plane"));
    assert!(with("scope", json!("machine")).contains("workspace or plane"));
    assert!(without("fresh_seconds").contains("fresh_seconds"));
    assert!(with("fresh_seconds", json!(0)).contains("fresh_seconds"));
    assert!(without("result").contains("'result'"));
    assert!(with("name", json!("a.b")).contains("query name"));
    assert!(with("colour", json!("red")).contains("\"colour\""));
    assert!(
        with(
            "parameters",
            json!({ "type": "object", "properties": { "n": { "type": "number" } } })
        )
        .contains("closed types")
    );
    assert!(
        with(
            "parameters",
            json!({ "type": "object", "properties": {}, "required": ["n"] })
        )
        .contains("requiring one it does not take")
    );
    assert!(with("widens", json!({ "team": "the team's" })).contains("does not take"));
    assert_eq!(
        declared_of(&json!([unread(), unread()])),
        Err("declares two queries called \"unread\"".to_owned())
    );
}

#[test]
fn an_extension_query_is_any_source_but_the_core_s() {
    assert!(is_extension_query("slack.unread"));
    assert!(is_extension_query("acme.slack.unread"));
    assert!(!is_extension_query("purlis.todo_list"));
    assert!(!is_extension_query("unread"));
    assert!(!is_extension_query(".unread"));
    assert!(!is_extension_query("slack."));
    assert!(!is_extension_query("../x.unread"));
}
