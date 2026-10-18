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
//! engine generates to the one committed for its format with [`narrowings`], and CI holds it to
//! the one the last release's tag holds (`tools/theme-schema-since-release.sh`, #2524); a
//! removal or a narrowed type is a new format.

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

    /// Keywords whose effect on what a schema accepts this comparison cannot judge: one that is
    /// new or changed wherever the earlier schema accepted values is a narrowing until the
    /// comparison is taught it.
    const UNJUDGED: &[&str] = &["not", "if", "then", "else", "oneOf"];

    /// Keywords under which a wider schema can accept less: a value is refused by `not` when
    /// its schema accepts it, an `if` that accepts more sends values to its other branch, and a
    /// value two branches of a `oneOf` accept is refused. Whatever a reference inside one
    /// reaches is held to the earlier schema word for word ([`inverted`]).
    const INVERTING: &[&str] = &["not", "if", "oneOf"];

    /// Keywords whose value is a value a theme holds, not a schema.
    const DATA: &[&str] = &["const", "enum"];

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
        unjudged(after, Some(before), "", &mut found);
        inverted(before, after, &mut found);
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

    /// Every [`UNJUDGED`] keyword in `schema` that `before` does not hold, word for word, at the
    /// same place, wherever the two schemas both have one: one the earlier schema already had,
    /// unchanged, accepts what it accepted. Where `before` has nothing (a new definition, a new
    /// key) the walk in [`compare`] has judged whether anything an earlier theme held reaches
    /// it. The keys of `properties`, `patternProperties` and `$defs` are names, not keywords,
    /// so only their values are read.
    fn unjudged(schema: &Value, before: Option<&Value>, at: &str, found: &mut Vec<String>) {
        if before.is_none() {
            return;
        }
        let Value::Object(fields) = schema else {
            if let Value::Array(items) = schema {
                for (n, item) in items.iter().enumerate() {
                    let was = before.and_then(|it| it.get(n));
                    unjudged(item, was, &format!("{at}/{n}"), found);
                }
            }
            return;
        };
        for (key, value) in fields {
            let here = format!("{at}/{key}");
            let was = before.and_then(|it| it.get(key));
            if UNJUDGED.contains(&key.as_str()) && was != Some(value) {
                found.push(format!(
                    "{here}: a keyword this check cannot judge; teach \
                     `theme::published::narrowings` whether it narrows"
                ));
            }
            match (key.as_str(), value) {
                ("properties" | "patternProperties" | "$defs", Value::Object(named)) => {
                    for (name, one) in named {
                        let was_one = was.and_then(|it| it.get(name));
                        unjudged(one, was_one, &format!("{here}/{name}"), found);
                    }
                }
                _ => unjudged(value, was, &here, found),
            }
        }
    }

    /// **Every schema a `$ref` reaches from inside an [`INVERTING`] keyword**, in either schema
    /// and through every reference it holds in turn, must be the same in both. A `oneOf` whose
    /// branches are told apart by a constant ([`told_apart`]) accepts what one of its branches
    /// accepts, as `anyOf` does, so it inverts nothing by itself; what it holds is still read.
    fn inverted(before: &Value, after: &Value, found: &mut Vec<String>) {
        let mut reached: Vec<(String, String)> = Vec::new();
        for schema in [before, after] {
            inverting_sites(schema, "", &mut reached);
        }
        let mut seen: Vec<String> = Vec::new();
        while let Some((reference, site)) = reached.pop() {
            if seen.contains(&reference) {
                continue;
            }
            seen.push(reference.clone());
            let Some(pointer) = reference.strip_prefix('#') else {
                found.push(format!(
                    "{site}: a reference this check cannot follow, {reference}"
                ));
                continue;
            };
            let (was, now) = (before.pointer(pointer), after.pointer(pointer));
            if was.is_some() && was != now {
                found.push(format!(
                    "{}: reached from inside {site}, where a wider schema can accept less; \
                     this check cannot judge a change here",
                    shown(pointer)
                ));
            }
            for one in [was, now].into_iter().flatten() {
                let mut inner = Vec::new();
                references(one, &mut inner);
                reached.extend(inner.into_iter().map(|it| (it, site.clone())));
            }
        }
    }

    /// Each `$ref` inside an [`INVERTING`] keyword of `schema`, with where that keyword is.
    fn inverting_sites(schema: &Value, at: &str, reached: &mut Vec<(String, String)>) {
        match schema {
            Value::Array(items) => {
                for (n, item) in items.iter().enumerate() {
                    inverting_sites(item, &format!("{at}/{n}"), reached);
                }
            }
            Value::Object(fields) => {
                for (key, value) in fields {
                    let here = format!("{at}/{key}");
                    if is_data(key) {
                        continue;
                    }
                    if INVERTING.contains(&key.as_str()) && !(key == "oneOf" && told_apart(value)) {
                        let mut inner = Vec::new();
                        references(value, &mut inner);
                        reached.extend(inner.into_iter().map(|it| (it, here.clone())));
                    }
                    match (key.as_str(), value) {
                        ("properties" | "patternProperties" | "$defs", Value::Object(named)) => {
                            for (name, one) in named {
                                inverting_sites(one, &format!("{here}/{name}"), reached);
                            }
                        }
                        _ => inverting_sites(value, &here, reached),
                    }
                }
            }
            _ => {}
        }
    }

    /// Every `$ref` anywhere in `schema`.
    fn references(schema: &Value, out: &mut Vec<String>) {
        match schema {
            Value::Array(items) => items.iter().for_each(|item| references(item, out)),
            Value::Object(fields) => {
                for (key, value) in fields {
                    match (key.as_str(), value) {
                        ("$ref", Value::String(reference)) => out.push(reference.clone()),
                        (key, _) if is_data(key) => {}
                        ("properties" | "patternProperties" | "$defs", Value::Object(named)) => {
                            named.values().for_each(|one| references(one, out));
                        }
                        _ => references(value, out),
                    }
                }
            }
            _ => {}
        }
    }

    fn is_data(key: &str) -> bool {
        DATA.contains(&key) || ANNOTATIONS.contains(&key)
    }

    /// **Whether no value can satisfy two of these `oneOf` branches**: each is an object that
    /// requires one same key and holds it to a constant of its own, written in the branch
    /// itself, so nothing a reference reaches can change which branch a value meets.
    fn told_apart(branches: &Value) -> bool {
        let Some(branches) = branches.as_array().filter(|it| it.len() > 1) else {
            return false;
        };
        let constant = |branch: &Value, key: &str| -> Option<Value> {
            let object = branch.get("type") == Some(&Value::String("object".to_owned()));
            let required = branch
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|it| it.iter().any(|one| one.as_str() == Some(key)));
            let one = branch.get("properties")?.get(key)?.get("const")?;
            (object && required).then(|| one.clone())
        };
        let keys = branches[0]
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        for key in keys {
            let constants: Option<Vec<Value>> = branches
                .iter()
                .map(|branch| constant(branch, key))
                .collect();
            if let Some(constants) = constants
                && constants
                    .iter()
                    .enumerate()
                    .all(|(n, one)| !constants[..n].contains(one))
            {
                return true;
            }
        }
        false
    }

    fn shown(at: &str) -> &str {
        if at.is_empty() { "/" } else { at }
    }

    /// **What an object schema asks of the property `name`**, as the schemas that all hold for
    /// its value: its entry in `properties` and each `patternProperties` schema whose pattern
    /// matches the name, or else `additionalProperties`. A pattern this check cannot read is a
    /// narrowing, and is taken to match.
    fn property<'a>(
        schema: &'a Map<String, Value>,
        name: &str,
        at: &str,
        found: &mut Vec<String>,
    ) -> Vec<&'a Value> {
        const ANY: &Value = &Value::Bool(true);
        let mut holds: Vec<&Value> = schema
            .get("properties")
            .and_then(|it| it.get(name))
            .into_iter()
            .collect();
        let patterns = schema.get("patternProperties").and_then(Value::as_object);
        for (pattern, one) in patterns.into_iter().flatten() {
            match regex::Regex::new(pattern) {
                Ok(it) if !it.is_match(name) => {}
                Ok(_) => holds.push(one),
                Err(_) => {
                    found.push(format!(
                        "{at}/patternProperties/{pattern}: a pattern this check cannot read"
                    ));
                    holds.push(one);
                }
            }
        }
        if holds.is_empty() {
            holds.push(schema.get("additionalProperties").unwrap_or(ANY));
        }
        holds
    }

    /// [`compare`] for a value every schema of `before` holds and every schema of `after` must:
    /// each of `after` is to accept all that one of `before` accepts.
    fn compare_each(before: &[&Value], after: &[&Value], at: &str, found: &mut Vec<String>) {
        if let ([was], [now]) = (before, after) {
            return compare(was, now, at, found);
        }
        for now in after {
            let mut first: Option<Vec<String>> = None;
            for was in before {
                let mut here = Vec::new();
                compare(was, now, at, &mut here);
                if here.is_empty() {
                    first = Some(here);
                    break;
                }
                first.get_or_insert(here);
            }
            found.extend(first.unwrap_or_default());
        }
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
                        let kept = property(after, name, at, found);
                        if !now_named.contains_key(name) && kept == [&Value::Bool(false)] {
                            found.push(format!("{path} is gone"));
                        } else {
                            let held = property(before, name, at, found);
                            compare_each(&held, &kept, &path, found);
                        }
                    }
                    for name in now_named.keys() {
                        if !was_named.contains_key(name) {
                            let held = property(before, name, at, found);
                            let kept = property(after, name, at, found);
                            compare_each(&held, &kept, &format!("{here}/{name}"), found);
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
