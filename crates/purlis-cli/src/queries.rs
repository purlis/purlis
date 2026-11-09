// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis queries`: the read-only entries of the one tool catalogue
//! ([`purlis_core::catalogue`], ADR 0122 §7), for a page's author.

use std::process::ExitCode;

use purlis_core::catalogue::{API, CATALOGUE, PROTOCOL_KEY, Query, listing};
use serde_json::Value;

pub fn run(json: bool) -> ExitCode {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&listing()).unwrap_or_default()
        );
    } else {
        println!("{PROTOCOL_KEY} {API}");
        for query in CATALOGUE.iter() {
            print!("{}", plain(query));
        }
    }
    ExitCode::SUCCESS
}

/// One query in three lines: its id, scope and since (and what it replaces, or why it is
/// deprecated); what it takes and the type it answers; and what it is for.
fn plain(query: &Query) -> String {
    let mut head = format!(
        "{} · {} · since {}",
        query.id(),
        query.scope.as_str(),
        query.since
    );
    if let Some(old) = query.replaces {
        head.push_str(&format!(" · replaces {old}"));
    }
    if let Some(why) = query.deprecated {
        head.push_str(&format!(" · deprecated: {why}"));
    }
    for (name, what) in query.widens {
        head.push_str(&format!(" · widens {name}: {what}"));
    }
    let result = query_type(&(query.result)());
    format!(
        "{head}\n  {} → {result}\n  {}\n",
        parameters(&query.parameters()),
        purlis_core::shown::line(query.tool().description),
    )
}

/// `name: type` for each parameter, the required ones marked; or that it takes none.
fn parameters(schema: &Value) -> String {
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let said: Vec<String> = schema["properties"]
        .as_object()
        .map(|props| {
            props
                .iter()
                .map(|(name, prop)| {
                    let mark = if required.contains(&name.as_str()) {
                        " (required)"
                    } else {
                        ""
                    };
                    format!("{name}: {}{mark}", query_type(prop))
                })
                .collect()
        })
        .unwrap_or_default();
    if said.is_empty() {
        "no parameters".to_owned()
    } else {
        said.join(", ")
    }
}

/// A schema's type: `object`, or `object | null` for a list of them.
fn query_type(schema: &Value) -> String {
    match &schema["type"] {
        Value::String(one) => one.clone(),
        Value::Array(many) => many
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        _ => "any".to_owned(),
    }
}
