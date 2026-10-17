// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where the theme schema is published, and the promise each format keeps** (#2448, spec
//! #2313).
//!
//! [`super::schema`] says what format 2 is; this module says where an editor finds it. The
//! address names two things: the format, in the file's name (`theme.v2.schema.json`), and the
//! release of purlis that wrote it, as the release's tag in the repository (`v<version>`). A theme
//! written by one purlis names the schema that purlis reads, so an editor never completes a
//! token an older purlis does not know, and every release's schema stays at its address for as
//! long as the tag does. purlis itself never fetches it (ADR 0083 amendment of 2026-10-17).
//!
//! **A format only grows.** Within one format a release adds token names, keys and values to
//! the schema and never takes one away or narrows one: a theme valid against one release's
//! schema is valid against every later release's of that format. A test holds the schema the
//! engine generates to the one committed for its format with [`narrowings`]; a removal or a
//! narrowed type is a new format.

/// The format the published schema describes, as its file is named.
pub const SCHEMA_FILE: &str = "docs/schema/theme.v2.schema.json";

/// **The schema's address**: [`SCHEMA_FILE`] as GitHub serves it at this release's tag. A
/// theme's `$schema` names it; every theme purlis writes, Purlis Classic's folder included,
/// names this one.
pub const SCHEMA_URL: &str = concat!(
    "https://raw.githubusercontent.com/purlis/purlis/v",
    env!("CARGO_PKG_VERSION"),
    "/docs/schema/theme.v2.schema.json"
);

#[cfg(test)]
pub(super) use promise::narrowings;

/// The additive-only rule as a comparison of two JSON Schemas, over the keywords the theme
/// schema uses. A test needs it, nothing shipped does.
#[cfg(test)]
mod promise {
    use serde_json::{Map, Value};

    /// Keywords that say something about a value without constraining it.
    const ANNOTATIONS: &[&str] = &[
        "$schema",
        "$id",
        "$comment",
        "title",
        "description",
        "examples",
        "default",
        "deprecated",
        "markdownDescription",
        "enumDescriptions",
    ];

    /// Keywords whose effect on what a schema accepts this comparison cannot judge: one
    /// anywhere in the later schema is a narrowing until the comparison is taught it.
    const UNJUDGED: &[&str] = &["not", "if", "then", "else", "oneOf"];

    /// Bounds a later schema may lower or drop, never raise or add.
    const LOWER: &[&str] = &[
        "minimum",
        "exclusiveMinimum",
        "minLength",
        "minItems",
        "minProperties",
    ];

    /// Bounds a later schema may raise or drop, never lower or add.
    const UPPER: &[&str] = &[
        "maximum",
        "exclusiveMaximum",
        "maxLength",
        "maxItems",
        "maxProperties",
    ];

    /// **Each way `after` accepts less than `before`**, one line each, as `<pointer>: <what>`, sorted;
    /// none when every value `before` accepts, `after` accepts too. A keyword this comparison
    /// does not know is a narrowing until it is taught, so the rule never passes by not
    /// looking.
    pub fn narrowings(before: &Value, after: &Value) -> Vec<String> {
        let mut found = Vec::new();
        compare(before, after, "", &mut found);
        unjudged(after, "", &mut found);
        found.sort();
        found.dedup();
        found
    }

    fn compare(before: &Value, after: &Value, at: &str, found: &mut Vec<String>) {
        match (before, after) {
            (Value::Bool(false), _) | (_, Value::Bool(true)) => {}
            (_, Value::Bool(false)) => found.push(format!("{}: accepts nothing now", shown(at))),
            (Value::Bool(true), Value::Object(after)) => {
                for key in after
                    .keys()
                    .filter(|key| !ANNOTATIONS.contains(&key.as_str()))
                {
                    found.push(format!("{at}/{key}: a new constraint"));
                }
            }
            (Value::Object(before), Value::Object(after)) => objects(before, after, at, found),
            _ => found.push(format!("{}: not a schema", shown(at))),
        }
    }

    /// Every [`UNJUDGED`] keyword in `schema`, wherever it is. The keys of `properties`,
    /// `patternProperties` and `$defs` are names, not keywords, so only their values are read.
    fn unjudged(schema: &Value, at: &str, found: &mut Vec<String>) {
        let Value::Object(fields) = schema else {
            if let Value::Array(items) = schema {
                for (n, item) in items.iter().enumerate() {
                    unjudged(item, &format!("{at}/{n}"), found);
                }
            }
            return;
        };
        for (key, value) in fields {
            let here = format!("{at}/{key}");
            if UNJUDGED.contains(&key.as_str()) {
                found.push(format!(
                    "{here}: a keyword this check cannot judge; teach \
                     `theme::published::narrowings` whether it narrows"
                ));
            }
            match (key.as_str(), value) {
                ("properties" | "patternProperties" | "$defs", Value::Object(named)) => {
                    for (name, one) in named {
                        unjudged(one, &format!("{here}/{name}"), found);
                    }
                }
                _ => unjudged(value, &here, found),
            }
        }
    }

    fn shown(at: &str) -> &str {
        if at.is_empty() { "/" } else { at }
    }

    /// What a schema with `properties` and `additionalProperties` asks of the property `name`.
    fn property<'a>(schema: &'a Map<String, Value>, name: &str) -> &'a Value {
        const ANY: &Value = &Value::Bool(true);
        schema
            .get("properties")
            .and_then(|it| it.get(name))
            .or_else(|| schema.get("additionalProperties"))
            .unwrap_or(ANY)
    }

    fn or_any(value: Option<&Value>) -> &Value {
        const ANY: &Value = &Value::Bool(true);
        value.unwrap_or(ANY)
    }

    fn objects(
        before: &Map<String, Value>,
        after: &Map<String, Value>,
        at: &str,
        found: &mut Vec<String>,
    ) {
        let mut keys: Vec<&String> = before.keys().chain(after.keys()).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let here = format!("{at}/{key}");
            let (was, now) = (before.get(key), after.get(key));
            match key.as_str() {
                key if ANNOTATIONS.contains(&key) => {}
                "$defs" => {
                    let empty = Map::new();
                    let was = was.and_then(Value::as_object).unwrap_or(&empty);
                    let now = now.and_then(Value::as_object).unwrap_or(&empty);
                    for (name, def) in was {
                        match now.get(name) {
                            Some(kept) => compare(def, kept, &format!("{here}/{name}"), found),
                            None => found.push(format!("{here}/{name} is gone")),
                        }
                    }
                }
                "properties" => {
                    let empty = Map::new();
                    let was_named = was.and_then(Value::as_object).unwrap_or(&empty);
                    let now_named = now.and_then(Value::as_object).unwrap_or(&empty);
                    for name in was_named.keys() {
                        let path = format!("{here}/{name}");
                        let kept = property(after, name);
                        if !now_named.contains_key(name) && kept == &Value::Bool(false) {
                            found.push(format!("{path} is gone"));
                        } else {
                            compare(&was_named[name], kept, &path, found);
                        }
                    }
                    for (name, added) in now_named {
                        if !was_named.contains_key(name) {
                            compare(
                                property(before, name),
                                added,
                                &format!("{here}/{name}"),
                                found,
                            );
                        }
                    }
                }
                "additionalProperties" | "propertyNames" | "items" => {
                    compare(or_any(was), or_any(now), &here, found);
                }
                "prefixItems" => {
                    let empty = Vec::new();
                    let was_items = was.and_then(Value::as_array).unwrap_or(&empty);
                    let now_items = now.and_then(Value::as_array).unwrap_or(&empty);
                    let rest = |schema: &Map<String, Value>| or_any(schema.get("items")).clone();
                    for n in 0..was_items.len().max(now_items.len()) {
                        let was_one = was_items.get(n).cloned().unwrap_or_else(|| rest(before));
                        let now_one = now_items.get(n).cloned().unwrap_or_else(|| rest(after));
                        compare(&was_one, &now_one, &format!("{here}/{n}"), found);
                    }
                }
                "anyOf" => match (was.and_then(Value::as_array), now) {
                    (_, None) => {}
                    (None, Some(_)) => found.push(format!("{here}: a new constraint")),
                    (Some(was_branches), Some(now)) => {
                        let now_branches = now.as_array().cloned().unwrap_or_default();
                        for (n, branch) in was_branches.iter().enumerate() {
                            match now_branches.get(n) {
                                Some(kept) => compare(branch, kept, &format!("{here}/{n}"), found),
                                None => found.push(format!("{here}/{n} is gone")),
                            }
                        }
                    }
                },
                "type" => match (was, now) {
                    (_, None) => {}
                    (None, Some(_)) => found.push(format!("{here}: a new constraint")),
                    (Some(was), Some(now)) => {
                        let now_types = types(now);
                        for one in types(was) {
                            let kept = now_types.contains(&one)
                                || (one == "integer" && now_types.contains(&"number"));
                            if !kept {
                                found.push(format!("{here}: {one} is no longer accepted"));
                            }
                        }
                    }
                },
                "enum" => match (was.and_then(Value::as_array), now) {
                    (_, None) => {}
                    (None, Some(_)) => found.push(format!("{here}: a new constraint")),
                    (Some(was), Some(now)) => {
                        let now = now.as_array().cloned().unwrap_or_default();
                        for one in was.iter().filter(|one| !now.contains(one)) {
                            found.push(format!("{here}: {one} is no longer accepted"));
                        }
                    }
                },
                "required" => {
                    let was = was.and_then(Value::as_array).cloned().unwrap_or_default();
                    for one in now.and_then(Value::as_array).into_iter().flatten() {
                        if !was.contains(one) {
                            found.push(format!("{here}: {one} is newly required"));
                        }
                    }
                }
                "uniqueItems" => {
                    if now == Some(&Value::Bool(true)) && was != Some(&Value::Bool(true)) {
                        found.push(format!("{here}: a new constraint"));
                    }
                }
                key if LOWER.contains(&key) || UPPER.contains(&key) => match (was, now) {
                    (_, None) => {}
                    (None, Some(_)) => found.push(format!("{here}: a new constraint")),
                    (Some(was), Some(now)) => {
                        let narrower = match (was.as_f64(), now.as_f64()) {
                            (Some(was), Some(now)) if LOWER.contains(&key) => now > was,
                            (Some(was), Some(now)) => now < was,
                            _ => true,
                        };
                        if narrower {
                            found.push(format!("{here}: was {was}, now {now}"));
                        }
                    }
                },
                // Keywords whose change cannot be told wider or narrower: the same, or gone.
                "$ref" | "pattern" | "const" | "format" => match (was, now) {
                    (_, None) => {}
                    (Some(was), Some(now)) if was == now => {}
                    (None, Some(_)) => found.push(format!("{here}: a new constraint")),
                    (Some(was), Some(now)) => {
                        found.push(format!("{here}: was {was}, now {now}"));
                    }
                },
                _ => {
                    if was != now {
                        found.push(format!(
                            "{here}: a keyword this check does not know changed; teach \
                             `theme::published::narrowings` whether it narrows"
                        ));
                    }
                }
            }
        }
    }

    fn types(value: &Value) -> Vec<&str> {
        match value {
            Value::String(one) => vec![one.as_str()],
            Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        }
    }
}
