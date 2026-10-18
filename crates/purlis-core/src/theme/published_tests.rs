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
        it["properties"]["flourish"] = json!({ "type": "object" });
        it["$defs"]["flourish"] = json!({ "type": "string" });
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
fn a_keyword_the_rule_cannot_judge_is_a_narrowing_wherever_the_earlier_schema_accepted_values() {
    let token = TOKENS[0];
    let inner = json!({ "type": "object" });
    for keyword in ["not", "if", "oneOf"] {
        let after = changed(|it| {
            // On a token the earlier schema had.
            it["$defs"]["tokens"]["properties"][token][keyword] = json!({ "type": "string" });
            // In place of what a key the earlier schema had asked of its value.
            it["$defs"]["appearance"]["properties"]["tokens"] = json!({ keyword: inner });
        });
        let found = narrowings(&schema(), &after);
        for at in [
            format!("/$defs/tokens/properties/{token}/{keyword}"),
            "/$defs/appearance/properties/tokens".to_owned(),
        ] {
            assert!(
                found.iter().any(|one| one.starts_with(&at)),
                "{keyword} at {at}: {found:?}"
            );
        }
    }
}

#[test]
fn a_keyword_the_rule_cannot_judge_inside_what_only_adds_is_not_a_narrowing() {
    // A new definition and a new key of a closed object: nothing an earlier theme could hold
    // reaches them, so what they accept is new and only adds.
    let after = changed(|it| {
        it["$defs"]["flourish"] = json!({ "oneOf": [{ "type": "string" }, { "type": "boolean" }] });
        it["properties"]["flourish"] =
            json!({ "type": "array", "items": { "$ref": "#/$defs/flourish" } });
        it["$defs"]["tokens"]["properties"]["brand.new"] = json!({ "not": { "type": "string" } });
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
}

#[test]
fn a_property_named_like_a_keyword_is_only_a_name() {
    let after = changed(|it| {
        it["properties"]["if"] = json!({ "type": "string" });
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
}

#[test]
fn a_keyword_the_rule_cannot_judge_that_was_already_there_unchanged_is_not_a_narrowing() {
    let before = changed(|it| {
        it["$defs"]["choice"] = json!({ "oneOf": [{ "type": "string" }, { "type": "boolean" }] });
    });
    assert_eq!(narrowings(&before, &before), Vec::<String>::new());
    let after = changed(|it| {
        it["$defs"]["choice"] = json!({ "oneOf": [{ "type": "string" }] });
    });
    let found = narrowings(&before, &after);
    assert!(
        found
            .iter()
            .any(|one| one.starts_with("/$defs/choice/oneOf: a keyword this check cannot judge")),
        "{found:?}"
    );
}

/// The schema with a definition `wide` that an earlier theme reaches through `holder`.
fn reached_through(holder: Value) -> Value {
    changed(|it| {
        it["$defs"]["wide"] = json!({ "type": "integer" });
        it["$defs"]["deeper"] = json!({ "$ref": "#/$defs/wide" });
        it["$defs"]["holder"] = holder;
        it["properties"]["held"] = json!({ "$ref": "#/$defs/holder" });
    })
}

#[test]
fn a_widened_definition_under_an_unchanged_inverting_keyword_is_a_narrowing() {
    // Inside each of these a wider schema can refuse a value an earlier theme held: `not`
    // refuses what it accepts, an `if` that accepts more sends a value to `then`, and a value
    // two `oneOf` branches accept is refused.
    for (keyword, holder) in [
        ("not", json!({ "not": { "$ref": "#/$defs/wide" } })),
        (
            "if",
            json!({ "if": { "$ref": "#/$defs/wide" }, "then": { "minimum": 0 } }),
        ),
        (
            "oneOf",
            json!({ "oneOf": [{ "$ref": "#/$defs/wide" }, { "type": "string" }] }),
        ),
        // Through a second reference.
        (
            "oneOf",
            json!({ "oneOf": [{ "$ref": "#/$defs/deeper" }, { "type": "string" }] }),
        ),
    ] {
        let before = reached_through(holder.clone());
        let mut after = before.clone();
        after["$defs"]["wide"] = json!({ "type": ["integer", "string"] });
        let found = narrowings(&before, &after);
        assert_eq!(
            found,
            [format!(
                "/$defs/wide: reached from inside /$defs/holder/{keyword}, where a wider schema \
                 can accept less; this check cannot judge a change here"
            )],
            "{holder}"
        );
    }
    // The same definition reached from nowhere inverting only adds.
    let plain =
        reached_through(json!({ "anyOf": [{ "$ref": "#/$defs/wide" }, { "type": "string" }] }));
    let mut wider = plain.clone();
    wider["$defs"]["wide"] = json!({ "type": ["integer", "string"] });
    assert_eq!(narrowings(&plain, &wider), Vec::<String>::new());
}

#[test]
fn a_definition_under_a_one_of_told_apart_by_a_constant_may_widen() {
    // The schema's own `option` is such a `oneOf`, and it reaches `tokens`: a new token only adds.
    assert!(
        schema()["$defs"]["option"]["oneOf"]
            .as_array()
            .is_some_and(|branches| branches.len() > 1)
    );
    let after = changed(|it| {
        it["$defs"]["tokens"]["properties"]["brand.new"] = json!({ "$ref": "#/$defs/colour" });
    });
    assert_eq!(narrowings(&schema(), &after), Vec::<String>::new());
    // A `oneOf` whose branches are not told apart by a constant they all require is held.
    let loose = |kind: Value| {
        json!({ "oneOf": [
            { "type": "object", "required": ["kind"],
              "properties": { "kind": { "const": "a" }, "on": { "$ref": "#/$defs/wide" } } },
            { "type": "object", "required": ["kind"], "properties": { "kind": kind } },
        ] })
    };
    for kind in [json!({ "const": "a" }), json!({ "type": "string" })] {
        let before = reached_through(loose(kind));
        let mut after = before.clone();
        after["$defs"]["wide"] = json!({ "type": ["integer", "string"] });
        assert_eq!(narrowings(&before, &after).len(), 1);
    }
    let told = reached_through(loose(json!({ "const": "b" })));
    let mut wider = told.clone();
    wider["$defs"]["wide"] = json!({ "type": ["integer", "string"] });
    assert_eq!(narrowings(&told, &wider), Vec::<String>::new());
}

#[test]
fn a_new_named_key_of_a_closed_object_is_judged_against_the_patterns_that_accepted_it() {
    let before = changed(|it| {
        it["properties"]["extras"] = json!({
            "type": "object",
            "additionalProperties": false,
            "patternProperties": { "^x-": { "type": "string" } }
        });
    });
    // A name the pattern accepted, now asked for less than the pattern asked: it narrows.
    let after = changed(|it| {
        it["properties"]["extras"] = before["properties"]["extras"].clone();
        it["properties"]["extras"]["properties"] = json!({ "x-brand": { "type": "integer" } });
    });
    assert_eq!(
        narrowings(&before, &after),
        ["/properties/extras/properties/x-brand/type: string is no longer accepted"]
    );
    // The same name asked for what the pattern asked, and a name it never accepted: they add.
    let after = changed(|it| {
        it["properties"]["extras"] = before["properties"]["extras"].clone();
        it["properties"]["extras"]["properties"] = json!({
            "x-brand": { "type": "string", "description": "The brand." },
            "brand": { "type": "integer" }
        });
    });
    assert_eq!(narrowings(&before, &after), Vec::<String>::new());
    // A named key the earlier schema had, now left to a pattern that asks for less.
    let named = changed(|it| {
        it["properties"]["extras"] = before["properties"]["extras"].clone();
        it["properties"]["extras"]["properties"] = json!({ "x-brand": { "type": "string" } });
    });
    let after = changed(|it| {
        it["properties"]["extras"] = before["properties"]["extras"].clone();
    });
    assert_eq!(narrowings(&named, &after), Vec::<String>::new());
    let mut narrower = after.clone();
    narrower["properties"]["extras"]["patternProperties"]["^x-"]["maxLength"] = json!(8);
    assert!(!narrowings(&named, &narrower).is_empty());
}

/// **The comparison CI's step makes with the last release** (#2524): `released` is that
/// release's `docs/schema/theme.v2.schema.json`, as its tag holds it.
fn since_release(released: &str) -> Vec<String> {
    let released: Value = serde_json::from_str(released).expect("the released schema is JSON");
    narrowings(&released, &schema())
}

#[test]
#[ignore = "CI's step `The theme schema only grows since the last release` runs it, with \
            PURLIS_THEME_SCHEMA_RELEASED naming the last release's schema \
            (tools/theme-schema-since-release.sh)"]
fn the_schema_only_grows_since_the_last_release() {
    let path = std::env::var_os("PURLIS_THEME_SCHEMA_RELEASED").expect(
        "PURLIS_THEME_SCHEMA_RELEASED names the last release's schema; \
         tools/theme-schema-since-release.sh sets it",
    );
    let released = std::fs::read_to_string(&path).expect("the released schema");
    let narrowed = since_release(&released);
    assert!(
        narrowed.is_empty(),
        "format 2's schema only grows, and the engine's takes away or narrows what the last \
         release's {SCHEMA_FILE} has; a theme valid against that release would stop being \
         valid. Keep it, or make the change a new format:\n{}",
        narrowed.join("\n")
    );
}

#[test]
fn a_removal_carried_by_the_committed_file_is_held_by_the_last_release_s() {
    // The engine drops a token and the committed file is edited to match: against the
    // committed file nothing is taken away, against the release it is.
    let released = schema_text();
    let gone = TOKENS[0];
    let engine = changed(|it| {
        it["$defs"]["tokens"]["properties"]
            .as_object_mut()
            .unwrap()
            .remove(gone);
    });
    let committed = engine.clone();
    assert_eq!(narrowings(&committed, &engine), Vec::<String>::new());
    let released: Value = serde_json::from_str(&released).unwrap();
    assert_eq!(
        narrowings(&released, &engine),
        [format!("/$defs/tokens/properties/{gone} is gone")]
    );
    // And the release's own schema, as this engine generates it, narrows nothing.
    assert_eq!(since_release(&schema_text()), Vec::<String>::new());
}
