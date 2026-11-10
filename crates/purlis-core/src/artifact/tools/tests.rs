// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use serde_json::{Map, Value, json};

use super::*;

/// A workspace at `ws` with folder `folder` holding `html`, pinned as `name`.
fn a_pinned_page(ws: &Path, folder: &str, name: &str, html: &str) {
    super::super::adopt(ws, ws, folder, html).unwrap();
    super::super::pin(
        ws,
        ws,
        &super::super::Request {
            folder: folder.into(),
            name: Some(name.into()),
            ..super::super::Request::default()
        },
    )
    .unwrap();
}

fn args(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn the_twins_are_read_only_and_each_is_one_of_these() {
    for tool in &TOOLS {
        assert!(tool.read_only, "{}", tool.name);
        assert!(is_one(tool.name));
    }
    assert!(!is_one("artifact_propose_pin"));
}

#[test]
fn artifact_list_says_each_pin_s_line_as_the_command_line_does_or_that_none_is_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    assert_eq!(
        call("alpha", ws, None, LIST, &Map::new()).unwrap(),
        "No artifact is pinned to 'alpha' (pin one: purlis artifact pin <folder>)."
    );
    a_pinned_page(ws, "board", "Board", "<title>Board</title>");
    let id = super::super::pinned(ws, "board").unwrap().id;
    assert_eq!(
        call("alpha", ws, None, LIST, &Map::new()).unwrap(),
        format!("1  {id}  board  chart  Board")
    );
    std::fs::remove_dir_all(ws.join(super::super::DIR).join("board")).unwrap();
    let said = call("alpha", ws, None, LIST, &Map::new()).unwrap();
    assert!(
        said.starts_with(&format!("1  {id}  board  chart  Board  (missing: ")),
        "{said}"
    );
}

#[test]
fn artifact_approve_status_says_where_each_pin_stands_and_refuses_a_pin_it_does_not_have() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    a_pinned_page(ws, "board", "Board", "<title>Board</title><p>12</p>");
    let said = call("alpha", ws, None, APPROVE_STATUS, &Map::new()).unwrap();
    assert!(said.starts_with("board  script-free  "), "{said}");
    assert!(
        said.ends_with("  opens without asking: no script"),
        "{said}"
    );
    assert_eq!(
        call(
            "alpha",
            ws,
            None,
            APPROVE_STATUS,
            &args(json!({ "pin": "board" }))
        )
        .unwrap(),
        said
    );
    let why = call(
        "alpha",
        ws,
        None,
        APPROVE_STATUS,
        &args(json!({ "pin": "nope" })),
    )
    .unwrap_err();
    assert_eq!(
        why,
        "no pin 'nope' in this workspace (`purlis artifact list` shows them)"
    );
    assert!(
        call(
            "alpha",
            ws,
            None,
            APPROVE_STATUS,
            &args(json!({ "pin": 3 }))
        )
        .is_err()
    );
}

#[test]
fn artifact_check_passes_a_page_that_calls_only_what_it_declares() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    super::super::adopt(ws, ws, "plain", "<title>Plain</title>").unwrap();
    assert_eq!(
        call(
            "alpha",
            ws,
            None,
            CHECK,
            &args(json!({ "folder": "plain" }))
        )
        .unwrap(),
        "plain declares nothing: it reads no data and calls no query."
    );
    super::super::adopt(
        ws,
        ws,
        "board",
        "<title>Board</title><script type=\"application/json\" id=\"purlis\">\
         {\"api\":1,\"queries\":{\"purlis.todo_list@1\":[\"done\"]}}</script>\
         <script>purlis.port.postMessage({ id: 1, query: \"purlis.todo_list@1\" });</script>",
    )
    .unwrap();
    let said = call(
        "alpha",
        ws,
        None,
        CHECK,
        &args(json!({ "folder": "board" })),
    )
    .unwrap();
    assert!(said.contains("  purlis.todo_list@1 · workspace"), "{said}");
    assert!(
        said.ends_with("board declares api 1: 0 data file(s) and 1 query(ies), and calls only what it declares."),
        "{said}"
    );
}

#[test]
fn artifact_check_refuses_an_undeclared_call_and_reads_nothing_outside_the_chat_s_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    super::super::adopt(
        ws,
        ws,
        "board",
        "<title>Board</title><script>purlis.port.postMessage({ id: 1, query: \
         \"purlis.todo_list@1\" });</script>",
    )
    .unwrap();
    let why = call(
        "alpha",
        ws,
        None,
        CHECK,
        &args(json!({ "folder": "board" })),
    )
    .unwrap_err();
    assert!(why.contains("refused: "), "{why}");
    assert!(why.contains("purlis.todo_list@1"), "{why}");

    for outside in ["..", "../..", "/etc", "board/../..", ""] {
        assert!(
            call(
                "alpha",
                ws,
                None,
                CHECK,
                &args(json!({ "folder": outside }))
            )
            .is_err(),
            "{outside}"
        );
    }
    assert!(call("alpha", ws, None, CHECK, &Map::new()).is_err());
}
