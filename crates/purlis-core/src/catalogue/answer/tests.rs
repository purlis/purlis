// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use chrono::{NaiveDate, TimeZone};
use serde_json::json;

use super::*;
use crate::catalogue::{CATALOGUE, find};

/// A project with `charter.toml` and workspace `alpha`, with charter's own template.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let found = crate::workspaces::Plane::open(dir.path())
        .workspace("alpha")
        .unwrap();
    std::fs::create_dir_all(found.dir()).unwrap();
    found.scaffold_charter().unwrap();
    dir
}

fn at() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc.with_ymd_and_hms(2026, 11, 9, 12, 0, 0).unwrap()
}

fn now() -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 9)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
}

fn query(name: &str) -> &'static Query {
    find(&format!("purlis.{name}")).unwrap().0
}

/// What a chat would do through the MCP tool, to set the workspace up.
fn tool(root: &Path, name: &str, args: Value) -> String {
    crate::chattools::call(
        root,
        &Place::Workspace("alpha".into()),
        name,
        args.as_object().unwrap(),
        now(),
    )
    .unwrap_or_else(|why| panic!("{name}: {why}"))
}

fn answered(root: &Path, name: &str, args: Value) -> Answer {
    ask(root, "alpha", query(name), &args, at(), now())
}

fn data(root: &Path, name: &str, args: Value) -> Value {
    let answer = answered(root, name, args);
    let data = answer.outcome.clone().unwrap_or_else(|kind| {
        panic!("{name} answered {}", kind.as_str());
    });
    fits(&(query(name).result)(), &data, name);
    data
}

/// Holds that `value` is of the shape `schema` describes, in the subset the catalogue uses:
/// types (one or a list), an object's properties, required and closed, and an array's items.
fn fits(schema: &Value, value: &Value, at: &str) {
    let types: Vec<&str> = match &schema["type"] {
        Value::String(one) => vec![one.as_str()],
        Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
        other => panic!("{at}: no type in {other}"),
    };
    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    assert!(types.contains(&kind), "{at}: {kind} is not {types:?}");
    if let Value::Object(fields) = value {
        let properties = schema["properties"].as_object().unwrap();
        for name in schema["required"].as_array().unwrap() {
            assert!(
                fields.contains_key(name.as_str().unwrap()),
                "{at}: {name} missing"
            );
        }
        for (name, field) in fields {
            let property = properties
                .get(name)
                .unwrap_or_else(|| panic!("{at}: {name} is not in its shape"));
            fits(property, field, &format!("{at}.{name}"));
        }
    }
    if let Value::Array(items) = value {
        for (i, item) in items.iter().enumerate() {
            fits(&schema["items"], item, &format!("{at}[{i}]"));
        }
    }
}

#[test]
fn todo_list_answers_the_open_todos_as_data_and_with_done_the_done_ones() {
    let p = project();
    assert_eq!(
        data(p.path(), "todo_list", json!({})),
        json!({ "done": false, "todos": [] })
    );
    tool(p.path(), "todo_add", json!({ "text": "Ship the page API" }));
    tool(p.path(), "todo_add", json!({ "text": "Write the docs" }));
    let open = data(p.path(), "todo_list", Value::Null);
    let titles: Vec<&str> = open["todos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Ship the page API", "Write the docs"]);
    assert!(
        open["todos"][0]["opened"]
            .as_str()
            .unwrap()
            .starts_with("2026-11-09")
    );

    let slug = open["todos"][0]["slug"].as_str().unwrap().to_owned();
    tool(p.path(), "todo_done", json!({ "slug": slug }));
    let done = data(p.path(), "todo_list", json!({ "done": true }));
    assert_eq!(done["done"], true);
    assert_eq!(done["todos"][0]["title"], "Ship the page API");
    assert_eq!(done["todos"].as_array().unwrap().len(), 1);
}

#[test]
fn memory_search_answers_memories_and_a_page_s_cursor_in_the_envelope() {
    let p = project();
    for n in 0..22 {
        tool(
            p.path(),
            "memory_add",
            json!({ "text": format!("Fact {n:02}\nabout widgets") }),
        );
    }
    let first = answered(p.path(), "memory_search", json!({}));
    let page = first.outcome.clone().unwrap();
    fits(&(query("memory_search").result)(), &page, "memory_search");
    assert_eq!(page["memories"].as_array().unwrap().len(), 20);
    let cursor = first.cursor.clone().expect("a cursor to the older ones");
    let rest = answered(p.path(), "memory_search", json!({ "cursor": cursor }));
    assert_eq!(rest.cursor, None);
    assert_eq!(
        rest.outcome.unwrap()["memories"].as_array().unwrap().len(),
        2
    );

    let found = data(p.path(), "memory_search", json!({ "query": "Fact 07" }));
    assert_eq!(found["memories"][0]["title"], "Fact 07");
    assert!(
        found["memories"][0]["text"]
            .as_str()
            .unwrap()
            .contains("about widgets")
    );
}

#[test]
fn session_records_are_listed_and_read_as_data_and_one_not_there_is_null() {
    let p = project();
    let sessions = p.path().join("workspaces/alpha/sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let file = "20261109-110000-built-the-api.md";
    std::fs::write(
        sessions.join(file),
        "---\ntitle: Built the API\n---\n\nIt answers.\n",
    )
    .unwrap();
    let listed = data(p.path(), "session_record_list", json!({}));
    assert_eq!(listed["records"][0]["file"], file);
    assert_eq!(listed["records"][0]["title"], "Built the API");
    assert_eq!(listed["records"][0]["when"], "2026-11-09 11:00");

    let read = data(p.path(), "session_record_read", json!({ "file": file }));
    assert!(read["text"].as_str().unwrap().contains("It answers."));
    let gone = data(
        p.path(),
        "session_record_read",
        json!({ "file": "20261109-120000-never.md" }),
    );
    assert_eq!(gone, Value::Null);
    let not_a_name = answered(
        p.path(),
        "session_record_read",
        json!({ "file": "../x.md" }),
    );
    assert_eq!(not_a_name.outcome, Err(ErrorKind::Refused));
}

#[test]
fn change_status_answers_each_change_record_as_data() {
    let p = project();
    assert_eq!(
        data(p.path(), "change_status", json!({})),
        json!({ "changes": [], "unreadable": [] })
    );
    let record = crate::change::Record::new(
        "page-api",
        "pages read purlis",
        "steward",
        "2026-11-09T12:00:00Z",
    );
    crate::change::store::write(p.path(), "alpha", &record).unwrap();
    let all = data(p.path(), "change_status", json!({}));
    assert_eq!(all["changes"][0]["change"], "page-api");
    assert_eq!(all["changes"][0]["why"], "pages read purlis");
    let one = data(p.path(), "change_status", json!({ "change": "other" }));
    assert_eq!(one["changes"], json!([]));
    let bad = answered(p.path(), "change_status", json!({ "change": "../x" }));
    assert_eq!(bad.outcome, Err(ErrorKind::Refused));
}

#[test]
fn a_parameter_the_query_does_not_take_or_out_of_its_type_is_refused() {
    let p = project();
    for (name, args) in [
        ("todo_list", json!({ "done": "yes" })),
        ("todo_list", json!({ "workspace": "beta" })),
        ("todo_list", json!([true])),
        ("memory_search", json!({ "query": 3 })),
        ("session_record_read", json!({})),
    ] {
        let answer = answered(p.path(), name, args.clone());
        assert_eq!(answer.outcome, Err(ErrorKind::Refused), "{name} {args}");
    }
}

#[test]
fn every_query_answers_data_of_its_own_shape_in_an_empty_workspace() {
    let p = project();
    for query in CATALOGUE.iter() {
        let args = match query.name {
            "session_record_read" => json!({ "file": "20261109-120000-x.md" }),
            // The app answers it, from its own record of its chats (#6235).
            "chats" => continue,
            _ => json!({}),
        };
        data(p.path(), query.name, args);
    }
}

#[test]
fn the_chats_are_the_app_s_to_answer_and_the_plane_alone_answers_failed() {
    let p = project();
    assert_eq!(
        answered(p.path(), "chats", json!({})).outcome,
        Err(ErrorKind::Failed)
    );
    assert_eq!(
        answered(p.path(), "chats", json!({ "history": "yes" })).outcome,
        Err(ErrorKind::Refused),
        "its parameters are checked before anything is asked"
    );
}

#[test]
fn the_chats_answer_is_of_its_own_shape() {
    use crate::catalogue::chats;
    let open = [chats::Chat {
        id: "01KA00000000000000000000A0".into(),
        name: "a chat".into(),
        persona: None,
        harness: None,
        workspace: Some("alpha".into()),
        parent: Some(chats::Parent {
            id: "chat-3".into(),
            relation: chats::Relation::HandedOffFrom,
            workspace: Some("alpha".into()),
        }),
        facts: chats::Facts::default(),
        started: Some(1_762_862_400),
        last_activity: None,
        session_record: Some("20261111-120000-x.md".into()),
    }];
    let asked = chats::Asked {
        history: false,
        limit: chats::HISTORY_DEFAULT,
    };
    let data = chats::answer(&open, &[], "alpha", asked);
    fits(&(query("chats").result)(), &data, "chats");
}
