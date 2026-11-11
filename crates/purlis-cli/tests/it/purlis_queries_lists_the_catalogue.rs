// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis queries` (ADR 0122 §7, #5951): every read-only entry of the one tool catalogue, with
//! its shape, scope and since, asked of the binary as a page's author would ask it. It reads no
//! plane, so it answers outside one.

use crate::support;

use serde_json::Value;

fn queries(args: &[&str]) -> (i32, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let out = support::purlis()
        .arg("queries")
        .args(args)
        .current_dir(dir.path())
        .output()
        .expect("purlis runs");
    let stdout = String::from_utf8(out.stdout).expect("UTF-8");
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), stdout)
}

#[test]
fn the_json_listing_is_every_query_with_its_shape_scope_and_since() {
    let (_, out) = queries(&["--json"]);
    let listing: Value = serde_json::from_str(&out).expect("JSON");
    assert_eq!(listing, purlis_core::catalogue::listing());
    assert_eq!(listing["api"], 1);
    let ids: Vec<&str> = listing["queries"]
        .as_array()
        .expect("queries")
        .iter()
        .map(|q| q["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(
        ids,
        [
            "purlis.todo_list",
            "purlis.memory_search",
            "purlis.session_record_list",
            "purlis.session_record_read",
            "purlis.change_status",
            "purlis.forge_milestones",
        ]
    );
    for query in listing["queries"].as_array().expect("queries") {
        assert_eq!(query["scope"], "workspace", "{query}");
        assert!(query["since"].is_string(), "{query}");
        assert_eq!(query["parameters"]["type"], "object", "{query}");
        assert!(
            query["result"]["type"] == "object"
                || query["result"]["type"] == serde_json::json!(["object", "null"]),
            "a query answers data, not text: {query}"
        );
    }
}

#[test]
fn the_plain_listing_says_each_id_with_its_scope_since_and_parameters() {
    let (_, out) = queries(&[]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "api 1");
    let todo = lines
        .iter()
        .find(|l| l.starts_with("purlis.todo_list"))
        .expect("todo_list is listed");
    assert!(
        todo.contains("workspace") && todo.contains("since 0.5.1"),
        "{todo}"
    );
    assert!(
        out.contains("done: boolean"),
        "the parameters are said: {out}"
    );
    assert!(
        out.contains("→ object") && out.contains("→ object | null"),
        "the result's shape is said: {out}"
    );
    assert!(
        out.contains("widens done"),
        "a parameter that widens what it reads is said: {out}"
    );
    for id in purlis_core::catalogue::CATALOGUE.iter().map(|q| q.id()) {
        assert!(out.contains(&id), "{id} is listed: {out}");
    }
}
