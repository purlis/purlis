// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An extension's catalogue queries** (ADR 0120 §9, ADR 0053 as amended 2026-11-09, #5971):
//! the `queries` capability. An extension contributes queries into the one catalogue a page, the
//! CLI and an agent read through, each `<extension-id>.<name>`.
//!
//! # Declared
//!
//! `contributes.queries` lists each query with its `name`, a `description`, its `scope`
//! (`workspace` or `plane`), `fresh_seconds`, the `parameters` it takes (a JSON Schema object
//! whose properties are of the closed types a core query's are: string, boolean, integer), the
//! `result` its data is described by, and the parameters that widen what it reads (`widens`).
//! All of it is inside the manifest's bytes, so the fingerprint covers it and the approval
//! prompt names each query ([`declares`]).
//!
//! # Answered
//!
//! [`ask`] answers one: **from the facts file** while the answer there is fresher than the
//! query's `fresh_seconds` and the query was asked with no parameters, without starting the
//! program; otherwise **by asking the program** one question with the executor's deadline
//! (protocol 7, [`crate::executor::Executor::query`]). The facts file gains a key per workspace
//! for it:
//!
//! ```json
//! { "workspaces": { "<ws>": { "queries": { "<name>": { "data": …, "at": 1790000000 } } } },
//!   "queries": { "<plane-scoped name>": { "data": …, "at": 1790000000 } } }
//! ```
//!
//! Every answer is the catalogue's envelope. **An extension this machine does not have**, or has
//! not approved, turned off here, or one that does not declare the query, answers
//! `needs-extension`: the page is shared and each viewer's own extensions answer it. One that
//! changed since its approval answers `not-approved`. **On a platform where purlis starts no
//! extension's program** (Windows, ADR 0031) every extension query is `refused`, never
//! `needs-extension`.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};

use super::facts;
use super::project::Choices;
use super::{Capability, Extension, Standing};
use crate::catalogue::{Answer, ErrorKind, Scope};
use crate::executor::{Executor, QueryFailed};

/// The most queries one extension may declare.
const MOST_QUERIES: usize = 16;

/// The most bytes a query's description may be.
const MOST_DESCRIPTION_BYTES: usize = 200;

/// The most parameters one query may take.
const MOST_PARAMETERS: usize = 8;

/// The most bytes a query's `result` schema may be, as JSON.
const MOST_RESULT_BYTES: usize = 16 << 10;

/// The keys a declared query may carry.
const KEYS: [&str; 7] = [
    "name",
    "description",
    "scope",
    "fresh_seconds",
    "parameters",
    "result",
    "widens",
];

/// The closed types a parameter may be, as a core query's are
/// ([`crate::catalogue::answer::checked`]).
const PARAMETER_TYPES: [&str; 3] = ["string", "boolean", "integer"];

/// One query an extension declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The id after `<extension-id>.`.
    pub name: String,
    pub description: String,
    pub scope: Scope,
    /// How long an answer stays good: the facts file's answer is used while it is younger, and a
    /// subscribed page is asked again after it.
    pub fresh_seconds: u64,
    /// The JSON Schema of what it takes, normalised: an object of closed-type properties.
    pub parameters: Value,
    /// The JSON Schema of the answer's `data`.
    pub result: Value,
    /// Each parameter that widens what it reads, with what it then reads.
    pub widens: Vec<(String, String)>,
}

/// The queries `contributes.queries` declares, or why purlis will not read them.
pub(super) fn declared_of(value: &Value) -> Result<Vec<Declared>, String> {
    let list = facts::entries(value, "queries", MOST_QUERIES)?;
    let mut out: Vec<Declared> = Vec::with_capacity(list.len());
    for (at, object) in list.iter().enumerate() {
        facts::only(object, &KEYS, "a query", at)?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("declares a query at {at} with no name"))?;
        if !facts::id_ok(name) {
            return Err(format!(
                "declares the query name {}, and a query's name is letters, digits, '-' and \
                 '_', starting with a letter or a digit",
                crate::shown::readable(name, 64)
            ));
        }
        let name = name.to_owned();
        if out.iter().any(|it| it.name == name) {
            return Err(format!("declares two queries called {name:?}"));
        }
        let description = object
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("declares the query {name:?} with no description"))?;
        if description.len() > MOST_DESCRIPTION_BYTES
            || description.contains(crate::panel::undrawable)
        {
            return Err(format!(
                "declares the query {name:?} with a description purlis will not draw: it is \
                 longer than {MOST_DESCRIPTION_BYTES} bytes or holds a control or invisible \
                 formatting character"
            ));
        }
        let scope = match object.get("scope").and_then(Value::as_str) {
            Some("workspace") => Scope::Workspace,
            Some("plane") => Scope::Plane,
            _ => {
                return Err(format!(
                    "declares the query {name:?} without a scope purlis knows — 'scope' is \
                     workspace or plane"
                ));
            }
        };
        let fresh_seconds = facts::fresh(object, "query", &name)?;
        let parameters = parameters_of(&name, object.get("parameters"))?;
        let result = match object.get("result") {
            Some(Value::Object(schema)) if schema.contains_key("type") => {
                Value::Object(schema.clone())
            }
            _ => {
                return Err(format!(
                    "declares the query {name:?} without a 'result' — the JSON Schema of the \
                     data it answers, an object with a 'type'"
                ));
            }
        };
        if result.to_string().len() > MOST_RESULT_BYTES {
            return Err(format!(
                "declares the query {name:?} with a 'result' over {} KiB",
                MOST_RESULT_BYTES >> 10
            ));
        }
        let widens = widens_of(&name, object.get("widens"), &parameters)?;
        out.push(Declared {
            name,
            description: description.to_owned(),
            scope,
            fresh_seconds,
            parameters,
            result,
            widens,
        });
    }
    Ok(out)
}

/// A query's `parameters`, normalised to an object of closed-type properties; none is an object
/// of none.
fn parameters_of(name: &str, value: Option<&Value>) -> Result<Value, String> {
    let refused = |why: &str| format!("declares the query {name:?} with parameters {why}");
    let Some(value) = value else {
        return Ok(json!({ "type": "object", "properties": {}, "additionalProperties": false }));
    };
    let schema = value
        .as_object()
        .filter(|schema| schema.get("type").and_then(Value::as_str) == Some("object"))
        .ok_or_else(|| refused("that are not a JSON Schema object (\"type\": \"object\")"))?;
    if let Some(key) = schema.keys().find(|key| {
        !["type", "properties", "required", "additionalProperties"].contains(&key.as_str())
    }) {
        return Err(refused(&format!(
            "carrying {key:?}; they say type, properties, required and additionalProperties"
        )));
    }
    let empty = Map::new();
    let properties = match schema.get("properties") {
        None => &empty,
        Some(Value::Object(properties)) => properties,
        Some(_) => return Err(refused("whose 'properties' is not an object")),
    };
    if properties.len() > MOST_PARAMETERS {
        return Err(refused(&format!(
            "past the {MOST_PARAMETERS} a query may take"
        )));
    }
    let mut kept = Map::new();
    for (param, property) in properties {
        if !facts::id_ok(param) {
            return Err(refused(&format!(
                "naming {}, and a parameter is letters, digits, '-' and '_'",
                crate::shown::readable(param, 64)
            )));
        }
        let kind = property
            .get("type")
            .and_then(Value::as_str)
            .filter(|kind| PARAMETER_TYPES.contains(kind))
            .ok_or_else(|| {
                refused(&format!(
                    "where {param:?} is not one of the closed types: {}",
                    PARAMETER_TYPES.join(", ")
                ))
            })?;
        let mut one = Map::new();
        one.insert("type".into(), kind.into());
        if let Some(text) = property.get("description").and_then(Value::as_str) {
            one.insert("description".into(), text.into());
        }
        kept.insert(param.clone(), Value::Object(one));
    }
    let required: Vec<&str> = match schema.get("required") {
        None => Vec::new(),
        Some(Value::Array(names)) => names
            .iter()
            .map(|it| it.as_str().filter(|n| kept.contains_key(*n)))
            .collect::<Option<_>>()
            .ok_or_else(|| refused("requiring one it does not take"))?,
        Some(_) => return Err(refused("whose 'required' is not a list of names")),
    };
    Ok(json!({
        "type": "object",
        "properties": kept,
        "required": required,
        "additionalProperties": false,
    }))
}

/// A query's `widens`: each a parameter it takes, with what it then reads.
fn widens_of(
    name: &str,
    value: Option<&Value>,
    parameters: &Value,
) -> Result<Vec<(String, String)>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let said = value.as_object().ok_or_else(|| {
        format!("declares the query {name:?} with a 'widens' that is not an object of parameters")
    })?;
    said.iter()
        .map(|(param, what)| {
            let what = what
                .as_str()
                .map(str::trim)
                .filter(|what| {
                    !what.is_empty()
                        && what.len() <= MOST_DESCRIPTION_BYTES
                        && !what.contains(crate::panel::undrawable)
                })
                .ok_or_else(|| {
                    format!(
                        "declares the query {name:?} widened by {param:?} without saying, in \
                         words purlis draws, what it then reads"
                    )
                })?;
            if parameters["properties"].get(param).is_none() {
                return Err(format!(
                    "declares the query {name:?} widened by {param:?}, which it does not take"
                ));
            }
            Ok((param.clone(), what.to_owned()))
        })
        .collect()
}

/// The approval prompt's line for each query, in purlis's words.
pub(super) fn declares(manifest: &super::Manifest) -> Vec<String> {
    manifest
        .queries
        .iter()
        .map(|query| {
            let reads = match query.scope {
                Scope::Workspace => "reads one workspace",
                Scope::Plane => "reads the whole project, a grant of its own on each page",
            };
            let mut line = format!(
                "a query, “{}.{}” — {}; {reads}, and an answer is good for {}",
                manifest.id,
                query.name,
                query.description,
                facts::age(query.fresh_seconds)
            );
            for (param, what) in &query.widens {
                line.push_str(&format!("; “{param}” widens it to {what}"));
            }
            line
        })
        .collect()
}

/// **Whether `id` is an extension's query**: `<extension-id>.<name>`, of a source that is not
/// the core's. Says nothing about whether any extension here declares it.
pub fn is_extension_query(id: &str) -> bool {
    split(id).is_some()
}

/// `<extension-id>.<name>`, split: the name holds no dot, the extension's id may.
fn split(id: &str) -> Option<(&str, &str)> {
    let (extension, name) = id.rsplit_once('.')?;
    (extension != crate::catalogue::SOURCE && super::an_id(extension) && facts::id_ok(name))
        .then_some((extension, name))
}

/// **What this machine's extensions declare of query `id`**, for the approval's listing: its
/// scope and widening parameters, from an extension installed and in force here that declares
/// it under the `queries` capability. Whether it answers is [`find`]'s, asked at the call.
pub fn declared_here(
    config_root: &Path,
    built_in: &super::BuiltIn,
    id: &str,
) -> Option<crate::artifact::calls::ExtensionQuery> {
    let (extension, name) = split(id)?;
    let loaded = super::read(config_root, built_in);
    let entry = loaded
        .registry
        .entries
        .get(extension)
        .filter(|entry| entry.in_force())?;
    let manifest = super::manifest_at(&entry.path).ok()?;
    if !manifest.capabilities.contains(&Capability::Queries) {
        return None;
    }
    let query = manifest
        .queries
        .into_iter()
        .find(|query| query.name == name)?;
    Some((query.scope, query.widens))
}

/// One extension query found here: the extension, approved and unchanged, and what it declares.
pub struct Found {
    pub extension: Extension,
    pub declared: Declared,
}

/// **The extension query `id`, as this machine has it**: installed, approved, on in `choices`'
/// project and workspace, declaring it under the `queries` capability, and unchanged since its
/// approval; otherwise why it cannot answer.
pub fn find(
    config_root: &Path,
    built_in: &super::BuiltIn,
    choices: &Choices,
    id: &str,
) -> Result<Found, ErrorKind> {
    let (extension, name) = split(id).ok_or(ErrorKind::Refused)?;
    let loaded = super::read(config_root, built_in);
    if loaded.unreadable.is_some() {
        return Err(ErrorKind::NeedsExtension);
    }
    let entry = loaded
        .registry
        .entries
        .get(extension)
        .filter(|entry| entry.in_force())
        .ok_or(ErrorKind::NeedsExtension)?;
    let manifest = super::manifest_at(&entry.path).map_err(|_| ErrorKind::NeedsExtension)?;
    let declared = manifest
        .queries
        .iter()
        .find(|query| query.name == name)
        .filter(|_| manifest.capabilities.contains(&Capability::Queries))
        .cloned()
        .ok_or(ErrorKind::NeedsExtension)?;
    if !facts::on_here(extension, &manifest, choices) {
        return Err(ErrorKind::NeedsExtension);
    }
    let found = super::read_at(&entry.path).map_err(|_| ErrorKind::Failed)?;
    if found.id() != extension || loaded.standing(&found) != Standing::Approved {
        return Err(ErrorKind::NotApproved);
    }
    if found.manifest != manifest {
        return Err(ErrorKind::NotApproved);
    }
    Ok(Found {
        extension: found,
        declared,
    })
}

/// What a query is asked: its id, the workspace it reads (a plane-scoped one reads none), its
/// parameters, and whether the page holds the plane grant.
pub struct Asked<'a> {
    pub id: &'a str,
    pub workspace: &'a str,
    pub params: &'a Value,
    pub plane: bool,
}

/// **The answer to extension query `asked`** at `at`, in the catalogue's envelope. See the
/// module docstring for where it comes from.
pub fn ask(
    executor: &Executor,
    config_root: &Path,
    choices: &Choices,
    asked: &Asked<'_>,
    at: DateTime<Utc>,
) -> Answer {
    if !crate::executor::RUNS_PROGRAMS {
        return Answer::error(ErrorKind::Refused, at);
    }
    let found = match find(config_root, executor.built_in(), choices, asked.id) {
        Ok(found) => found,
        Err(kind) => return Answer::error(kind, at),
    };
    let args = match crate::catalogue::answer::checked(&found.declared.parameters, asked.params) {
        Ok(args) => args,
        Err(kind) => return Answer::error(kind, at),
    };
    let workspace = match found.declared.scope {
        Scope::Plane if !asked.plane => return Answer::error(ErrorKind::NotApproved, at),
        Scope::Plane => None,
        Scope::Workspace => Some(asked.workspace),
    };
    if args.is_empty()
        && let Some((data, when)) = from_facts(&found, workspace, at)
    {
        return sized(data, None, when);
    }
    match executor.query(
        config_root,
        choices,
        found.extension.id(),
        &found.declared.name,
        workspace,
        &args,
    ) {
        Ok((data, cursor)) => sized(data, cursor, at),
        Err(QueryFailed::Late(_)) => Answer::error(ErrorKind::Timeout, at),
        Err(QueryFailed::Failed(_)) => Answer::error(ErrorKind::Failed, at),
    }
}

/// `data` as an answer stamped `at`, or `too-large` past the catalogue's cap.
fn sized(data: Value, cursor: Option<String>, at: DateTime<Utc>) -> Answer {
    let size = serde_json::to_vec(&data).map_or(usize::MAX, |b| b.len());
    if size > crate::catalogue::answer::MOST_BYTES {
        return Answer::error(ErrorKind::TooLarge, at);
    }
    let answer = Answer::data(data, at);
    match cursor {
        Some(cursor) => answer.with_cursor(cursor),
        None => answer,
    }
}

/// **The query's answer in its facts file**, and when it was true, while it is younger than its
/// `fresh_seconds` at `now`: under `workspaces.<ws>.queries.<name>` for one workspace's, or
/// `queries.<name>` for a plane-scoped one. Anything else is no answer, never an error: the
/// program is asked instead.
fn from_facts(
    found: &Found,
    workspace: Option<&str>,
    now: DateTime<Utc>,
) -> Option<(Value, DateTime<Utc>)> {
    let state = found.extension.manifest.state.as_ref()?;
    let root = &found.extension.path;
    let text = super::slurp(root, &root.join(state).join(facts::FILE), facts::MOST_BYTES).ok()?;
    let doc: Value = serde_json::from_str(&text).ok()?;
    let name = &found.declared.name;
    let field = match workspace {
        Some(ws) => doc
            .get(facts::WORKSPACES)?
            .get(ws)?
            .get(facts::QUERIES)?
            .get(name)?,
        None => doc.get(facts::QUERIES)?.get(name)?,
    };
    let data = field.get("data")?.clone();
    let when = DateTime::from_timestamp(field.get("at")?.as_i64()?, 0)?;
    let age = now.signed_duration_since(when).num_seconds();
    let fresh = i64::try_from(found.declared.fresh_seconds).ok()?;
    (-facts::MOST_SKEW_SECONDS..fresh)
        .contains(&age)
        .then_some((data, when))
}

#[cfg(test)]
mod tests;
