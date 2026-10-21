// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A JSON Schema draft-07 validator, for the design-tokens tests only** (#2926).
//!
//! The export is held to the W3C design-tokens format's own schema (`dtcg-2025.10/format.json`),
//! and purlis adds no crate to check it. This reads exactly the keywords that schema uses and
//! **fails closed**: a schema that names a keyword it does not read panics, so a newer schema
//! cannot pass an export by being half read. The official test suite vendored beside the schema
//! holds it honest: every fixture must get the verdict its manifest gives (`tokens_tests.rs`).
//!
//! Draft-07 rules it keeps: a `$ref` replaces the schema it sits in (its siblings are not read);
//! an `$id` sets the base a `#/…` reference inside it is resolved against; numbers compare by
//! value, so `1` and `1.0` are equal. `format` is asserted, as the suite's own validator does
//! (ajv with ajv-formats), for the two formats the schema names: `json-pointer-uri-fragment` by
//! ajv-formats' own pattern, `uri-reference` by the characters a URI reference may hold. Any
//! other format panics.

use std::collections::HashMap;

use serde_json::Value;

/// The keywords this validator reads.
const READ: &[&str] = &[
    "$ref",
    "type",
    "properties",
    "patternProperties",
    "additionalProperties",
    "required",
    "enum",
    "const",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "items",
    "additionalItems",
    "minItems",
    "maxItems",
    "minimum",
    "maximum",
    "exclusiveMaximum",
    "pattern",
    "format",
];

/// The keywords that only describe, which validate nothing.
const ANNOTATIONS: &[&str] = &[
    "$schema",
    "$id",
    "$comment",
    "title",
    "description",
    "definitions",
    "default",
    "examples",
];

/// Every schema document a bundle holds, by its `$id` without a fragment.
pub struct Schemas {
    by_id: HashMap<String, Value>,
}

impl Schemas {
    /// The schemas `bundle` holds: itself and every subschema that has an `$id`.
    pub fn bundle(bundle: &Value) -> Self {
        let mut by_id = HashMap::new();
        register(bundle, &mut by_id);
        Self { by_id }
    }

    /// **Why `instance` does not hold to the schema `id`**: empty when it does. Each line names
    /// where in the instance it failed.
    pub fn validate(&self, id: &str, instance: &Value) -> Vec<String> {
        let schema = self
            .by_id
            .get(id)
            .unwrap_or_else(|| panic!("no schema has the $id {id}"));
        let mut errors = Vec::new();
        self.check(schema, id, instance, "#", &mut errors, 0);
        errors
    }

    fn resolve(&self, base: &str, reference: &str) -> (&Value, String) {
        let (document, fragment) = match reference.split_once('#') {
            Some(("", fragment)) => (base.to_owned(), fragment.to_owned()),
            Some((document, fragment)) => (document.to_owned(), fragment.to_owned()),
            None => (reference.to_owned(), String::new()),
        };
        let root = self
            .by_id
            .get(&document)
            .unwrap_or_else(|| panic!("the reference {reference} names no schema this bundle has"));
        let target = if fragment.is_empty() {
            root
        } else {
            root.pointer(&fragment)
                .unwrap_or_else(|| panic!("the reference {reference} names nothing in {document}"))
        };
        let base = target
            .get("$id")
            .and_then(Value::as_str)
            .map_or(document, |id| id.split('#').next().unwrap_or(id).to_owned());
        (target, base)
    }

    fn check(
        &self,
        schema: &Value,
        base: &str,
        instance: &Value,
        at: &str,
        errors: &mut Vec<String>,
        depth: usize,
    ) {
        assert!(
            depth < 256,
            "the schema references itself without end at {at}"
        );
        let object = match schema {
            Value::Bool(true) => return,
            Value::Bool(false) => {
                errors.push(format!("{at}: no value is allowed here"));
                return;
            }
            Value::Object(object) => object,
            other => panic!("a schema is an object or a boolean, not {other}"),
        };
        let base = object.get("$id").and_then(Value::as_str).map_or_else(
            || base.to_owned(),
            |id| id.split('#').next().unwrap_or(id).to_owned(),
        );
        if let Some(reference) = object.get("$ref") {
            let reference = reference.as_str().expect("a $ref is a string");
            let (target, target_base) = self.resolve(&base, reference);
            self.check(target, &target_base, instance, at, errors, depth + 1);
            return;
        }
        for (keyword, value) in object {
            assert!(
                READ.contains(&keyword.as_str()) || ANNOTATIONS.contains(&keyword.as_str()),
                "the schema uses the keyword {keyword}, which this validator does not read: \
                 teach it the keyword before trusting its verdict"
            );
            assert!(
                well_formed(keyword, value),
                "the schema's {keyword} is {value}, which is not the shape draft-07 gives it"
            );
        }
        let sub = |schema: &Value, instance: &Value, at: &str| {
            let mut found = Vec::new();
            self.check(schema, &base, instance, at, &mut found, depth + 1);
            found
        };
        if let Some(kind) = object.get("type") {
            let kinds: Vec<&str> = match kind {
                Value::String(one) => vec![one.as_str()],
                Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
                other => panic!("a type is a name or a list of names, not {other}"),
            };
            if !kinds.iter().any(|kind| is_type(instance, kind)) {
                errors.push(format!("{at}: is not {}", kinds.join(" or ")));
            }
        }
        if let Some(constant) = object.get("const")
            && !equal(constant, instance)
        {
            errors.push(format!("{at}: is not {constant}"));
        }
        if let Some(Value::Array(allowed)) = object.get("enum")
            && !allowed.iter().any(|one| equal(one, instance))
        {
            errors.push(format!(
                "{at}: is none of {}",
                Value::Array(allowed.clone())
            ));
        }
        if let Some(Value::Array(all)) = object.get("allOf") {
            for one in all {
                errors.extend(sub(one, instance, at));
            }
        }
        if let Some(Value::Array(any)) = object.get("anyOf")
            && !any.iter().any(|one| sub(one, instance, at).is_empty())
        {
            errors.push(format!("{at}: matches none of anyOf"));
        }
        if let Some(Value::Array(one_of)) = object.get("oneOf") {
            let matched = one_of
                .iter()
                .filter(|one| sub(one, instance, at).is_empty())
                .count();
            if matched != 1 {
                errors.push(format!("{at}: matches {matched} of oneOf, not exactly one"));
            }
        }
        if let Some(not) = object.get("not")
            && sub(not, instance, at).is_empty()
        {
            errors.push(format!("{at}: matches what not forbids"));
        }
        if let Some(condition) = object.get("if") {
            let branch = if sub(condition, instance, at).is_empty() {
                object.get("then")
            } else {
                object.get("else")
            };
            if let Some(branch) = branch {
                errors.extend(sub(branch, instance, at));
            }
        }
        if let Value::Object(fields) = instance {
            if let Some(Value::Array(required)) = object.get("required") {
                for name in required.iter().filter_map(Value::as_str) {
                    if !fields.contains_key(name) {
                        errors.push(format!("{at}: has no {name}"));
                    }
                }
            }
            let properties = object.get("properties").and_then(Value::as_object);
            let patterns: Vec<(regex::Regex, &Value)> = object
                .get("patternProperties")
                .and_then(Value::as_object)
                .map(|patterns| {
                    patterns
                        .iter()
                        .map(|(pattern, schema)| (pattern_of(pattern), schema))
                        .collect()
                })
                .unwrap_or_default();
            for (name, value) in fields {
                let here = format!("{at}/{}", name.replace('~', "~0").replace('/', "~1"));
                let mut matched = false;
                if let Some(schema) = properties.and_then(|it| it.get(name)) {
                    matched = true;
                    errors.extend(sub(schema, value, &here));
                }
                for (pattern, schema) in &patterns {
                    if pattern.is_match(name) {
                        matched = true;
                        errors.extend(sub(schema, value, &here));
                    }
                }
                if !matched && let Some(additional) = object.get("additionalProperties") {
                    errors.extend(sub(additional, value, &here));
                }
            }
        }
        if let Value::Array(items) = instance {
            if let Some(least) = object.get("minItems").and_then(Value::as_u64)
                && (items.len() as u64) < least
            {
                errors.push(format!("{at}: has fewer than {least} items"));
            }
            if let Some(most) = object.get("maxItems").and_then(Value::as_u64)
                && (items.len() as u64) > most
            {
                errors.push(format!("{at}: has more than {most} items"));
            }
            match object.get("items") {
                Some(Value::Array(each)) => {
                    for (index, item) in items.iter().enumerate() {
                        let here = format!("{at}/{index}");
                        match each.get(index) {
                            Some(schema) => errors.extend(sub(schema, item, &here)),
                            None => {
                                if let Some(additional) = object.get("additionalItems") {
                                    errors.extend(sub(additional, item, &here));
                                }
                            }
                        }
                    }
                }
                Some(schema) => {
                    for (index, item) in items.iter().enumerate() {
                        errors.extend(sub(schema, item, &format!("{at}/{index}")));
                    }
                }
                None => {}
            }
        }
        if let Some(number) = instance.as_f64().filter(|_| instance.is_number()) {
            if let Some(least) = object.get("minimum").and_then(Value::as_f64)
                && number < least
            {
                errors.push(format!("{at}: is less than {least}"));
            }
            if let Some(most) = object.get("maximum").and_then(Value::as_f64)
                && number > most
            {
                errors.push(format!("{at}: is more than {most}"));
            }
            if let Some(below) = object.get("exclusiveMaximum").and_then(Value::as_f64)
                && number >= below
            {
                errors.push(format!("{at}: is not less than {below}"));
            }
        }
        if let (Some(format), Value::String(text)) = (object.get("format"), instance) {
            let format = format.as_str().expect("a format is a string");
            if !is_format(format, text) {
                errors.push(format!("{at}: is not a {format}"));
            }
        }
        if let (Some(pattern), Value::String(text)) = (object.get("pattern"), instance) {
            let pattern = pattern_of(pattern.as_str().expect("a pattern is a string"));
            if !pattern.is_match(text) {
                errors.push(format!("{at}: does not match {pattern}"));
            }
        }
    }
}

/// Whether a keyword this validator reads has the shape draft-07 gives it: one it would read
/// some other way panics instead of passing quietly.
fn well_formed(keyword: &str, value: &Value) -> bool {
    let schema = |value: &Value| value.is_object() || value.is_boolean();
    let names = |value: &Value| {
        value
            .as_array()
            .is_some_and(|it| it.iter().all(Value::is_string))
    };
    match keyword {
        "type" => value.is_string() || names(value),
        "properties" | "patternProperties" => {
            value.as_object().is_some_and(|it| it.values().all(schema))
        }
        "additionalProperties" | "additionalItems" | "not" | "if" | "then" | "else" => {
            schema(value)
        }
        "required" => names(value),
        "enum" => value.is_array(),
        "oneOf" | "anyOf" | "allOf" => value
            .as_array()
            .is_some_and(|it| !it.is_empty() && it.iter().all(schema)),
        "items" => schema(value) || value.as_array().is_some_and(|it| it.iter().all(schema)),
        "minItems" | "maxItems" => value.is_u64(),
        "minimum" | "maximum" | "exclusiveMaximum" => value.is_number(),
        "pattern" | "format" | "$ref" => value.is_string(),
        _ => true,
    }
}

/// Whether `text` is of the format `format`.
fn is_format(format: &str, text: &str) -> bool {
    match format {
        // ajv-formats' `json-pointer-uri-fragment`.
        "json-pointer-uri-fragment" => {
            pattern_of(r"(?i)^#(?:/(?:[a-z0-9_\-.!$&'()*+,;:=@]|%[0-9a-f]{2}|~0|~1)*)*$")
                .is_match(text)
        }
        // RFC 3986's characters, and each percent sign starting an escape: looser than
        // ajv-formats' grammar (it takes `http://[`), which purlis's exports never meet, since
        // they hold no URI.
        "uri-reference" => {
            pattern_of(r"^(?:[A-Za-z0-9\-._~:/?#\[\]@!$&'()*+,;=]|%[0-9A-Fa-f]{2})*$")
                .is_match(text)
        }
        other => panic!("the schema names the format {other}, which this validator does not read"),
    }
}

/// Every schema with an `$id` inside `node`, `node` included, by that id.
fn register(node: &Value, by_id: &mut HashMap<String, Value>) {
    match node {
        Value::Object(object) => {
            if let Some(id) = object.get("$id").and_then(Value::as_str) {
                by_id.insert(id.split('#').next().unwrap_or(id).to_owned(), node.clone());
            }
            for value in object.values() {
                register(value, by_id);
            }
        }
        Value::Array(items) => {
            for item in items {
                register(item, by_id);
            }
        }
        _ => {}
    }
}

/// A schema's pattern, which JSON Schema matches anywhere in the text unless it anchors itself.
fn pattern_of(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).unwrap_or_else(|why| panic!("the pattern {pattern}: {why}"))
}

/// Whether `instance` is of the JSON Schema type `kind`.
fn is_type(instance: &Value, kind: &str) -> bool {
    match kind {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        "number" => instance.is_number(),
        "integer" => instance
            .as_f64()
            .is_some_and(|n| instance.is_number() && n.fract() == 0.0),
        other => panic!("{other} is not a JSON Schema type"),
    }
}

/// JSON equality as JSON Schema has it: numbers by their value.
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| equal(value, other)))
        }
        _ => a == b,
    }
}
