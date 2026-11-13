// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A query's answer as data** (ADR 0122 §7, #5969): what a catalogue query answers a page, in
//! the shape its [`Query::result`] describes, never the text its MCP tool gives a chat.
//!
//! - **Its parameters are checked against their closed types** before anything is read
//!   ([`checked`]): a name its tool does not take, or a value out of its type, is `refused`.
//! - **It reads what the MCP tool reads**, through the same held stores of the same workspace
//!   (`chattools`), so a page, the CLI and a chat never see two versions of one fact.
//! - **Under a fixed cap** ([`MOST_BYTES`]): a bigger answer is `too-large`, never a part.
//! - A page of memories has a cursor to the next, carried in the envelope's `cursor`.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::{Answer, ErrorKind, Query, Scope};
use crate::active::Place;

/// The most one answer's data may be, as JSON: bigger is `too-large`.
pub const MOST_BYTES: usize = 4 << 20;

/// **The answer to `query` asked with `args`** in workspace `workspace` of the plane at `root`.
/// `at` stamps the answer, and `now` is the local time a todo's age is counted from. A query
/// that reads the forge asks it through `forge` ([`super::milestones`]).
pub fn ask(
    root: &Path,
    workspace: &str,
    query: &Query,
    args: &Value,
    forge: &super::milestones::Reach,
    at: chrono::DateTime<chrono::Utc>,
    now: chrono::NaiveDateTime,
) -> Answer {
    let args = match checked(&query.parameters(), args) {
        Ok(args) => args,
        Err(kind) => return Answer::error(kind, at),
    };
    if query.name == super::milestones::NAME {
        return match super::milestones::read(root, workspace, &args, forge) {
            Ok(data) => sized(data, None, at),
            Err(why) => Answer::error_saying(why.kind, why.message, at),
        };
    }
    let place = match query.scope {
        Scope::Workspace => Place::Workspace(workspace.to_owned()),
        Scope::Plane => Place::PlaneRoot,
    };
    match data(root, &place, query.name, &args, now) {
        Ok((data, cursor)) => sized(data, cursor, at),
        Err(kind) => Answer::error(kind, at),
    }
}

/// `data` as an answer with its cursor, or `too-large` over [`MOST_BYTES`].
fn sized(data: Value, cursor: Option<String>, at: chrono::DateTime<chrono::Utc>) -> Answer {
    let size = serde_json::to_vec(&data).map_or(usize::MAX, |b| b.len());
    if size > MOST_BYTES {
        return Answer::error(ErrorKind::TooLarge, at);
    }
    let answer = Answer::data(data, at);
    match cursor {
        Some(cursor) => answer.with_cursor(cursor),
        None => answer,
    }
}

/// **`args` as the parameters `schema` describes**, or `refused`: an object, every name one the
/// schema has, every value of its closed type, and every required one there. `null` is none.
pub fn checked(schema: &Value, args: &Value) -> Result<Map<String, Value>, ErrorKind> {
    let args = match args {
        Value::Null => Map::new(),
        Value::Object(args) => args.clone(),
        _ => return Err(ErrorKind::Refused),
    };
    let properties = schema["properties"].as_object();
    for (name, value) in &args {
        let Some(property) = properties.and_then(|p| p.get(name)) else {
            return Err(ErrorKind::Refused);
        };
        let fits = match property["type"].as_str() {
            Some("string") => value.is_string(),
            Some("boolean") => value.is_boolean(),
            Some("integer") => value.is_i64() || value.is_u64(),
            _ => false,
        };
        if !fits {
            return Err(ErrorKind::Refused);
        }
    }
    let required = schema["required"].as_array().into_iter().flatten();
    if required
        .filter_map(Value::as_str)
        .any(|name| !args.contains_key(name))
    {
        return Err(ErrorKind::Refused);
    }
    Ok(args)
}

#[cfg(not(unix))]
fn data(
    _root: &Path,
    _place: &Place,
    _name: &str,
    _args: &Map<String, Value>,
    _now: chrono::NaiveDateTime,
) -> Result<(Value, Option<String>), ErrorKind> {
    // Fail closed, as the MCP tools do: the stores are held only on unix yet.
    Err(ErrorKind::Failed)
}

/// The data of query `name`, and the cursor to its next page when there is one.
#[cfg(unix)]
fn data(
    root: &Path,
    place: &Place,
    name: &str,
    args: &Map<String, Value>,
    _now: chrono::NaiveDateTime,
) -> Result<(Value, Option<String>), ErrorKind> {
    use crate::chattools as tools;
    use crate::held::Store;
    let failed = |_: String| ErrorKind::Failed;
    let ws = || tools::workspace_name(place).map_err(|_| ErrorKind::Refused);
    let text = |key: &str| tools::optional_string(args, key).map_err(|_| ErrorKind::Refused);
    match name {
        "todo_list" => {
            ws()?;
            let done = args.get("done").and_then(Value::as_bool).unwrap_or(false);
            let todos = match Store::open(root, place, "todos", false).map_err(failed)? {
                None => Vec::new(),
                Some(store) if done => tools::done_todos(&store).map_err(failed)?,
                Some(store) => tools::open_todos(&store).map_err(failed)?,
            };
            let todos: Vec<Value> = todos
                .into_iter()
                .map(|t| json!({ "slug": t.slug, "title": t.title, "opened": t.stamp }))
                .collect();
            Ok((json!({ "done": done, "todos": todos }), None))
        }
        "memory_search" => {
            ws()?;
            let store = Store::open(root, place, "memory", false).map_err(failed)?;
            let (found, cursor) = match text("query")?.map(str::trim).filter(|q| !q.is_empty()) {
                Some(query) => {
                    let all = match &store {
                        Some(store) => tools::memories(store).map_err(failed)?,
                        None => Vec::new(),
                    };
                    (tools::best_memories(all, query), None)
                }
                None => tools::newest_page(store.as_ref(), text("cursor")?).map_err(failed)?,
            };
            let memories: Vec<Value> = found
                .into_iter()
                .map(|f| {
                    json!({
                        "file": f.path.to_string_lossy(),
                        "title": f.title,
                        "text": f.text,
                    })
                })
                .collect();
            Ok((json!({ "memories": memories }), cursor))
        }
        "session_record_list" => {
            let records: Vec<Value> = tools::session_records(root, place)
                .map_err(failed)?
                .into_iter()
                .map(|r| json!({ "file": r.file, "title": r.title, "when": r.when }))
                .collect();
            Ok((json!({ "records": records }), None))
        }
        "session_record_read" => {
            let file = text("file")?.ok_or(ErrorKind::Refused)?;
            if !crate::sessionrecord::is_record_name(file) {
                return Err(ErrorKind::Refused);
            }
            let read = Store::open(root, place, crate::sessionrecord::DIR, false)
                .map_err(failed)?
                .map(|sessions| sessions.read(file))
                .transpose()
                .map_err(failed)?
                .flatten();
            Ok((
                read.map_or(Value::Null, |text| json!({ "file": file, "text": text })),
                None,
            ))
        }
        "change_status" => {
            ws()?;
            let only = text("change")?;
            if only.is_some_and(|slug| !crate::change::name_ok(slug)) {
                return Err(ErrorKind::Refused);
            }
            let store =
                Store::open(root, place, crate::change::store::DIRNAME, false).map_err(failed)?;
            let mut changes = Vec::new();
            let mut unreadable = Vec::new();
            for read in tools::change_records(store.as_ref()).map_err(failed)? {
                match read {
                    Ok(record) if only.is_none_or(|slug| slug == record.change) => {
                        changes.push(change(&record));
                    }
                    Err((slug, why)) if only.is_none_or(|only| only == slug) => {
                        unreadable.push(json!({ "change": slug, "why": why }));
                    }
                    _ => {}
                }
            }
            Ok((
                json!({ "changes": changes, "unreadable": unreadable }),
                None,
            ))
        }
        _ => Err(ErrorKind::Failed),
    }
}

/// One change record as data.
#[cfg(unix)]
fn change(record: &crate::change::Record) -> Value {
    json!({
        "change": record.change,
        "why": record.why,
        "created": record.created,
        "by": record.by,
        "members": record.members.iter().map(|m| json!({
            "repo": m.repo,
            "branch": m.branch,
            "needs": m.needs,
        })).collect::<Vec<_>>(),
        "excluded": record.excluded.iter().map(|e| json!({
            "repo": e.repo,
            "why": e.why,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(all(test, unix))]
mod tests;
