// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The network adversarial suite's rule-to-cell mapping** (T1.1-20, #6867): every network deny
//! rule purlis writes has an escape cell aimed at it, or is named as debt with why.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

fn read(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **The shapes of the network deny rules in `source`**: from `(deny network` to the first of
/// `{`, `#`, `)` or `*` (kept, where it is `{` or `*`).
fn shapes(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in source.lines() {
        let mut rest = line;
        while let Some(at) = rest.find("(deny network") {
            let tail = &rest[at..];
            let end =
                tail.find(['{', '#', ')', '*'])
                    .map_or(tail.len(), |i| match tail.as_bytes()[i] {
                        b'{' | b'*' => i + 1,
                        _ => i,
                    });
            found.insert(tail[..end].to_owned());
            rest = &tail[end..];
        }
    }
    found
}

/// Which escapes the rules' file maps to, flattened, for the shapes it lists.
struct Map {
    mapped: Vec<(String, Vec<String>)>,
    debt: Vec<(String, Vec<String>, String)>,
}

fn escapes_of(entry: &Value) -> Vec<String> {
    entry["escapes"]
        .as_array()
        .expect("an escapes list")
        .iter()
        .map(|e| e.as_str().expect("an escape id").to_owned())
        .collect()
}

fn map_of(file: &Value) -> Map {
    Map {
        mapped: file["rules"]
            .as_array()
            .expect("rules")
            .iter()
            .map(|r| {
                (
                    r["shape"].as_str().expect("a shape").to_owned(),
                    escapes_of(r),
                )
            })
            .collect(),
        debt: file["debt"]
            .as_array()
            .expect("debt")
            .iter()
            .map(|r| {
                (
                    r["shape"].as_str().expect("a shape").to_owned(),
                    escapes_of(r),
                    r["why"].as_str().expect("why").to_owned(),
                )
            })
            .collect(),
    }
}

/// **What is wrong with `map` against `source`, the escape table and the coverage list**, one
/// line each: a shape in the source that is neither mapped nor debt, a listed shape no longer
/// in the source, a mapped escape that is not ticket 20's or has a Full or Basic cover that is
/// not a scenario, a debt that no longer owes, and a debt with no reason.
fn problems(map: &Map, source: &str, table: &Value, coverage: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let in_source = shapes(source);
    let listed: BTreeSet<&str> = map
        .mapped
        .iter()
        .map(|(s, _)| s.as_str())
        .chain(map.debt.iter().map(|(s, _, _)| s.as_str()))
        .collect();
    for shape in &in_source {
        if !listed.contains(shape.as_str()) {
            found.push(format!(
                "`{shape}` is written by the source and no cell is aimed at it, nor is it debt"
            ));
        }
    }
    for shape in &listed {
        if !in_source.contains(*shape) {
            found.push(format!(
                "`{shape}` is listed and the source no longer writes it"
            ));
        }
    }
    let ours: BTreeSet<&str> = table["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|r| r["ticket"] == "20")
        .filter_map(|r| r["id"].as_str())
        .collect();
    let covered = |id: &str| -> bool {
        coverage["escapes"]
            .as_array()
            .expect("escapes")
            .iter()
            .find(|e| e["id"] == id)
            .is_some_and(|e| {
                ["full", "basic"].iter().all(|p| {
                    e["covers"][*p]
                        .as_str()
                        .is_some_and(|c| c.starts_with("scenario: "))
                })
            })
    };
    for (shape, escapes) in &map.mapped {
        for id in escapes {
            if !ours.contains(id.as_str()) {
                found.push(format!(
                    "`{shape}`: {id} is not a row of ticket 20 in the escape table"
                ));
            } else if !covered(id) {
                found.push(format!(
                    "`{shape}`: {id} has no scenario cell at Full and Basic"
                ));
            }
        }
    }
    for (shape, escapes, why) in &map.debt {
        if why.trim().is_empty() {
            found.push(format!("`{shape}`: debt with no reason"));
        }
        if escapes.iter().all(|id| covered(id)) {
            found.push(format!(
                "`{shape}`: every escape it names now has its cells, so it is no debt: map it"
            ));
        }
    }
    found
}

fn here() -> PathBuf {
    purlis_scenarios::root()
}

fn source() -> String {
    let path = here().join("../../crates/purlis-core/src/sandbox/seatbelt.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn every_network_deny_rule_has_an_escape_cell() {
    let found = problems(
        &map_of(&read(here().join("network-rules.json"))),
        &source(),
        &read(here().join("escapes.json")),
        &read(here().join("coverage.json")),
    );
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn a_network_deny_rule_nobody_aimed_a_cell_at_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: Vec::new(),
        debt: Vec::new(),
    };
    let found = problems(
        &map,
        "x = \"(deny network-outbound (remote ip 1.2.3.4)\";",
        &table,
        &coverage,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("(deny network-outbound (remote ip 1.2.3.4"),
        "{found:?}"
    );
}

#[test]
fn a_rule_mapped_to_an_escape_with_no_cell_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: vec![("(deny network-outbound".into(), vec!["E-NET-19".into()])],
        debt: Vec::new(),
    };
    let found = problems(&map, "(deny network-outbound)", &table, &coverage);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("E-NET-19 has no scenario cell"),
        "{found:?}"
    );
}

#[test]
fn a_debt_whose_escapes_all_have_cells_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: Vec::new(),
        debt: vec![(
            "(deny network-outbound".into(),
            vec!["E-NET-22".into()],
            "late".into(),
        )],
    };
    let found = problems(&map, "(deny network-outbound)", &table, &coverage);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("is no debt"), "{found:?}");
}

#[test]
fn the_shapes_end_at_the_first_brace_hash_paren_or_star() {
    let got = shapes(
        "a(deny network-outbound (remote unix-socket (path-regex #\\\"^{regex}/\\\")))\n\
         (deny network-outbound (remote ip \\\"localhost:{port}\\\"))\n\
         (deny network-outbound (remote ip \\\"localhost:*\\\"))\n(deny file-write* (literal x))",
    );
    let want: BTreeSet<String> = [
        "(deny network-outbound (remote unix-socket (path-regex ",
        "(deny network-outbound (remote ip \\\"localhost:{",
        "(deny network-outbound (remote ip \\\"localhost:*",
    ]
    .map(str::to_owned)
    .into();
    assert_eq!(got, want);
}
