// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use chrono::TimeZone;
use serde_json::json;

use super::*;

/// The protocol as `tests/fixtures/queries/v1.json` pins it.
fn pinned() -> Value {
    let text = include_str!("../../../../tests/fixtures/queries/v1.json");
    serde_json::from_str(text).expect("the fixture is JSON")
}

#[test]
fn the_protocol_key_and_version_are_the_ones_the_fixture_pins() {
    let pinned = pinned();
    assert_eq!(pinned["key"], PROTOCOL_KEY);
    assert_eq!(pinned["api"], API);
    assert_eq!(pinned["source"], SOURCE);
}

#[test]
fn the_error_kinds_are_exactly_the_six_adr_0122_names() {
    let kinds: Vec<&str> = ErrorKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "needs-extension",
            "not-approved",
            "timeout",
            "too-large",
            "refused",
            "failed"
        ]
    );
    assert_eq!(pinned()["error_kinds"], json!(kinds));
}

#[test]
fn an_answer_is_data_at_and_a_cursor_only_when_there_is_one() {
    let at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 9, 13, 32, 0)
        .unwrap();
    assert_eq!(
        Answer::data(json!("No open todos in 'alpha'."), at).to_json(),
        json!({ "data": "No open todos in 'alpha'.", "at": "2026-11-09T13:32:00Z" })
    );
    assert_eq!(
        Answer::data(json!([1, 2]), at).with_cursor("20").to_json(),
        json!({ "data": [1, 2], "at": "2026-11-09T13:32:00Z", "cursor": "20" })
    );
}

#[test]
fn an_error_answer_carries_its_kind_and_no_data() {
    let at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 9, 13, 32, 0)
        .unwrap();
    assert_eq!(
        Answer::error(ErrorKind::NeedsExtension, at).to_json(),
        json!({ "error": { "kind": "needs-extension" }, "at": "2026-11-09T13:32:00Z" })
    );
    assert_eq!(
        pinned()["envelope"]["error"],
        Answer::error(ErrorKind::Refused, at).to_json()
    );
    assert_eq!(
        pinned()["envelope"]["data"],
        Answer::data(json!({ "n": 1 }), at)
            .with_cursor("c")
            .to_json()
    );
}

#[test]
fn every_query_is_a_read_only_mcp_tool_named_under_purlis() {
    for query in CATALOGUE.iter() {
        let tool = query.tool();
        assert!(
            tool.read_only,
            "{} answers from a tool that writes",
            query.id()
        );
        assert_eq!(query.id(), format!("purlis.{}", tool.name));
    }
}

#[test]
fn every_read_only_tool_is_a_query_or_says_why_it_is_not() {
    for tool in every_tool().filter(|t| t.read_only) {
        let query = CATALOGUE.iter().any(|q| q.name == tool.name);
        let not = NOT_QUERIES.iter().any(|(name, _)| *name == tool.name);
        assert!(
            query != not,
            "{}: a read-only tool is either a query or in NOT_QUERIES, never both or neither",
            tool.name
        );
    }
    for (name, why) in NOT_QUERIES {
        assert!(
            every_tool().any(|t| t.name == name && t.read_only),
            "{name} is not a read-only tool"
        );
        assert!(!why.is_empty(), "{name} says no reason");
    }
}

#[test]
fn a_released_query_id_is_never_taken_away_and_a_new_one_is_pinned_on_purpose() {
    let pinned: Vec<String> = pinned()["queries"]
        .as_array()
        .expect("queries")
        .iter()
        .map(|q| q.as_str().expect("an id").to_owned())
        .collect();
    let ours: Vec<String> = CATALOGUE.iter().map(Query::id).collect();
    assert_eq!(
        ours, pinned,
        "add a new id to tests/fixtures/queries/v1.json; never drop one"
    );
}

#[test]
fn ids_are_unique_and_what_a_query_replaces_is_another_id() {
    let ids: Vec<String> = CATALOGUE.iter().map(Query::id).collect();
    for (i, id) in ids.iter().enumerate() {
        assert!(!ids[..i].contains(id), "{id} twice");
    }
    for query in CATALOGUE.iter() {
        if let Some(old) = query.replaces {
            assert!(old.contains('.') && !ids.contains(&old.to_owned()), "{old}");
        }
    }
}

#[test]
fn the_listing_gives_each_query_its_shape_scope_and_since() {
    let listing = listing();
    assert_eq!(listing[PROTOCOL_KEY], API);
    let queries = listing["queries"].as_array().expect("queries");
    assert_eq!(queries.len(), CATALOGUE.len());
    let todo = &queries[0];
    assert_eq!(todo["id"], "purlis.todo_list");
    assert_eq!(todo["scope"], "workspace");
    assert_eq!(todo["since"], CATALOGUE[0].since);
    assert_eq!(todo["parameters"]["properties"]["done"]["type"], "boolean");
    assert_eq!(todo["result"]["type"], "object");
    assert_eq!(todo["widens"]["done"], CATALOGUE[0].widens[0].1);
    assert!(todo.get("deprecated").is_none() && todo.get("replaces").is_none());
}

#[test]
fn a_query_s_parameters_are_its_mcp_tool_s_schema() {
    for query in CATALOGUE.iter() {
        assert_eq!(
            query.parameters(),
            (query.tool().schema)(),
            "{}",
            query.id()
        );
    }
}

#[test]
fn every_query_s_result_shape_is_the_one_the_fixture_pins() {
    let pinned = pinned();
    let results = pinned["results"].as_object().expect("results");
    assert_eq!(results.len(), CATALOGUE.len(), "one shape per query");
    for query in CATALOGUE.iter() {
        assert_eq!(
            results.get(&query.id()),
            Some(&(query.result)()),
            "{}: a result shape moves only on purpose, with the fixture",
            query.id()
        );
        assert_ne!(
            (query.result)()["type"],
            "string",
            "{}: a query answers data, not text",
            query.id()
        );
    }
}

#[test]
fn a_widening_parameter_is_one_its_query_takes() {
    for query in CATALOGUE.iter() {
        for (name, what) in query.widens {
            assert!(
                query.parameters()["properties"].get(name).is_some(),
                "{}: {name} is no parameter of it",
                query.id()
            );
            assert!(!what.is_empty());
        }
    }
}

#[test]
fn an_id_is_found_as_written_and_only_a_deprecated_or_replaced_one_warns() {
    let (todo, warning) = find("purlis.todo_list").expect("found");
    assert_eq!(todo.name, "todo_list");
    assert_eq!(warning, None);
    assert!(find("purlis.todo_lists").is_none());
    assert!(find("todo_list").is_none());
    assert!(find("PURLIS.todo_list").is_none());
}

#[test]
fn only_a_query_no_plane_change_moves_lists_its_freshness() {
    let listing = listing();
    for listed in listing["queries"].as_array().expect("queries") {
        let fresh = &listed["fresh_seconds"];
        if listed["id"] == "purlis.forge_milestones" {
            assert_eq!(fresh, &json!(60), "asked again about once a minute");
        } else {
            assert!(fresh.is_null(), "{}: a plane change moves it", listed["id"]);
        }
    }
}

#[test]
fn forge_milestones_reads_a_repo_of_the_workspace_and_names_repo_as_widening() {
    let (query, warning) = find("purlis.forge_milestones").expect("it is a query");
    assert!(warning.is_none());
    assert_eq!(query.scope, Scope::Workspace);
    assert_eq!(query.since, "0.5.2");
    assert_eq!(query.widens.iter().map(|(n, _)| *n).collect::<Vec<_>>(), ["repo"]);
    let parameters = query.parameters();
    assert_eq!(parameters["properties"]["state"]["enum"], json!(["open", "all"]));
    assert!(
        parameters.get("required").is_none(),
        "the workspace's only repo, open milestones, with none given"
    );
}

#[test]
fn an_error_may_carry_a_sentence_for_the_reader_beside_its_kind() {
    let at = chrono::Utc
        .with_ymd_and_hms(2026, 11, 9, 13, 32, 0)
        .unwrap();
    let said = "purlis could not reach GitHub for the milestones of acme/api. It asks again in a \
                minute.";
    assert_eq!(
        Answer::error_saying(ErrorKind::Failed, said, at).to_json(),
        json!({ "error": { "kind": "failed", "message": said }, "at": "2026-11-09T13:32:00Z" })
    );
    assert_eq!(
        pinned()["envelope"]["error_with_message"],
        Answer::error_saying(ErrorKind::Failed, "It says why.", at).to_json()
    );
}
