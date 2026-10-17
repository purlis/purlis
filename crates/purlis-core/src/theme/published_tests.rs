// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The schema's address and the additive-only rule (#2448), at `theme::published`'s interface.
//! The committed schema is held to the engine's in `format_tests`
//! (`the_published_schema_is_the_one_the_engine_generates`), which runs the rule first.

use serde_json::{Value, json};

use super::published::narrowings;
use super::vocabulary::TOKENS;
use super::*;

/// The schema with `change` made to it.
fn changed(change: impl FnOnce(&mut Value)) -> Value {
    let mut after = schema();
    change(&mut after);
    after
}

/// Rewrites Purlis Classic's `$schema` to [`SCHEMA_URL`], in the folder's own text: only that
/// line changes, so the appearances' values (pinned by their sha-256) stay as they are. The
/// schema test's write step calls it, so a version bump is one command.
pub(super) fn name_this_release_in_the_classic_folder() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/theme/built-in/purlis-classic/theme.json"
    );
    let text = std::fs::read_to_string(path).expect("Purlis Classic's theme.json");
    let named = format!("  \"$schema\": \"{SCHEMA_URL}\",");
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("\"$schema\":") {
                named.clone()
            } else {
                line.to_owned()
            }
        })
        .collect();
    std::fs::write(path, rewritten.join("\n") + "\n").expect("Purlis Classic's theme.json");
}

#[test]
fn the_address_names_the_format_and_this_release() {
    assert_eq!(
        SCHEMA_URL,
        format!(
            "https://raw.githubusercontent.com/purlis/purlis/v{}/{SCHEMA_FILE}",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert!(SCHEMA_FILE.ends_with("/theme.v2.schema.json"));
}

#[test]
fn the_published_file_is_the_format_s_and_names_no_release() {
    let schema = schema();
    assert_eq!(schema.get("$id"), None);
    assert!(!schema_text().contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_schema_compared_with_itself_narrows_nothing() {
    assert_eq!(narrowings(&schema(), &schema()), Vec::<String>::new());
}

#[test]
fn a_removed_token_is_a_narrowing() {
    let gone = TOKENS[0];
    let after = changed(|it| {
        it["$defs"]["tokens"]["properties"]
            .as_object_mut()
            .unwrap()
            .remove(gone);
    });
    assert_eq!(
        narrowings(&schema(), &after),
        [format!("/$defs/tokens/properties/{gone} is gone")]
    );
}

#[test]
fn a_new_token_or_key_only_adds() {
    let after = changed(|it| {
        it["$defs"]["tokens"]["properties"]["brand.new"] = json!({ "$ref": "#/$defs/colour" });
        it["properties"]["options"] = json!({ "type": "object" });
        it["$defs"]["option"] = json!({ "type": "string" });
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
}

#[test]
fn a_narrowed_type_is_a_narrowing() {
    let token = TOKENS[1];
    let after = changed(|it| {
        // A token that took a palette name too now takes hex only.
        it["$defs"]["tokens"]["properties"][token] = json!({ "$ref": "#/$defs/hex" });
        // A string that is now an integer.
        it["properties"]["name"]["type"] = json!("integer");
    });
    let found = narrowings(&schema(), &after);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].starts_with("/$defs/tokens/properties/") && found[0].contains("/$ref: was"),
        "{found:?}"
    );
    assert_eq!(
        found[1],
        "/properties/name/type: string is no longer accepted"
    );
}

#[test]
fn a_lower_bound_raised_an_upper_lowered_or_a_new_one_are_narrowings() {
    let after = changed(|it| {
        it["properties"]["name"]["minLength"] = json!(2);
        it["properties"]["appearances"]["maxItems"] = json!(8);
        it["properties"]["extends"]["maxLength"] = json!(64);
    });
    let found = narrowings(&schema(), &after);
    assert_eq!(
        found,
        [
            "/properties/appearances/maxItems: was 64, now 8",
            "/properties/extends/maxLength: a new constraint",
            "/properties/name/minLength: was 1, now 2",
        ]
    );
}

#[test]
fn a_widened_bound_or_a_dropped_one_only_adds() {
    let after = changed(|it| {
        it["properties"]["appearances"]["maxItems"] = json!(128);
        it["properties"]["name"]
            .as_object_mut()
            .unwrap()
            .remove("minLength");
        it["$defs"]["appearance"]["properties"]["appearance"]["enum"] =
            json!(["dark", "light", "dim"]);
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
}

#[test]
fn a_value_no_longer_accepted_a_new_requirement_or_a_closed_object_are_narrowings() {
    let after = changed(|it| {
        it["$defs"]["appearance"]["properties"]["appearance"]["enum"] = json!(["dark"]);
        it["$defs"]["appearance"]["required"] = json!(["id", "appearance"]);
        it["properties"]["palette"]["additionalProperties"] = json!(false);
    });
    assert_eq!(
        narrowings(&schema(), &after),
        [
            "/$defs/appearance/properties/appearance/enum: \"light\" is no longer accepted",
            "/$defs/appearance/required: \"appearance\" is newly required",
            "/properties/palette/additionalProperties: accepts nothing now",
        ]
    );
}

#[test]
fn a_removed_definition_or_branch_is_a_narrowing() {
    let after = changed(|it| {
        it["$defs"].as_object_mut().unwrap().remove("motion");
        it["$defs"]["colour"]["anyOf"].as_array_mut().unwrap().pop();
    });
    assert_eq!(
        narrowings(&schema(), &after),
        ["/$defs/colour/anyOf/1 is gone", "/$defs/motion is gone"]
    );
}

#[test]
fn a_keyword_the_rule_does_not_know_is_a_narrowing_until_it_is_taught() {
    let after = changed(|it| {
        it["properties"]["name"]["dependentRequired"] = json!({ "name": ["extends"] });
    });
    let found = narrowings(&schema(), &after);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("/properties/name/dependentRequired: a keyword this check"),
        "{found:?}"
    );
}

#[test]
fn a_keyword_the_rule_cannot_judge_anywhere_is_a_narrowing() {
    for keyword in ["not", "if", "oneOf"] {
        let after = changed(|it| {
            // Inside a new token, where the walk itself has nothing to compare it with.
            it["$defs"]["tokens"]["properties"]["brand.new"] =
                json!({ keyword: { "type": "string" } });
        });
        let found = narrowings(&schema(), &after);
        assert_eq!(found.len(), 1, "{keyword}: {found:?}");
        assert!(
            found[0].starts_with(&format!(
                "/$defs/tokens/properties/brand.new/{keyword}: a keyword this check cannot judge"
            )),
            "{found:?}"
        );
    }
}

#[test]
fn a_property_named_like_a_keyword_is_only_a_name() {
    let after = changed(|it| {
        it["properties"]["if"] = json!({ "type": "string" });
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
}
