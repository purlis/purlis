// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The process adversarial suite's rule-to-cell mapping** (T1.1-22, #6869): every process deny
//! rule the macOS floor writes, and the allow of signals and process info it gives back, has an
//! escape cell aimed at it, or is named as debt with why.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

fn read(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The text of the `&str` constant `name` in `source`: from its opening quote to the `";` that
/// ends it, with Rust's `\"` unescaped.
fn constant(source: &str, name: &str) -> String {
    let head = format!("const {name}: &str = \"");
    let at = source
        .find(&head)
        .unwrap_or_else(|| panic!("`{name}` is no longer a constant of the source"))
        + head.len();
    let rest = &source[at..];
    let end = rest.find("\";").expect("the constant ends");
    rest[..end].replace("\\\"", "\"")
}

/// **The shapes of the process rules in `source`**:
/// - each `(deny word)` of the floor's processes, one for a bare word (`(deny lsopen)`);
/// - one for each program a `(deny process-exec` names (`(deny process-exec /usr/bin/open)`) and
///   each service a `(deny mach-lookup` names (`(deny mach-lookup <name>)`);
/// - the two allows the base gives back for the chat's own sandbox, whole.
fn shapes(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut inside: Option<String> = None;
    for line in constant(source, "FLOOR_PROCESSES").lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("(deny ") {
            let word = rest.trim_end_matches(')').split_whitespace().next();
            let word = word.expect("a rule has a word").to_owned();
            if word == "process-exec" || word == "mach-lookup" {
                inside = Some(word);
            } else {
                found.insert(format!("(deny {word})"));
                inside = None;
            }
            continue;
        }
        let Some(word) = &inside else { continue };
        let marker = if word == "process-exec" {
            "(literal \""
        } else {
            "(global-name \""
        };
        if let Some(at) = line.find(marker) {
            let name: String = line[at + marker.len()..]
                .chars()
                .take_while(|c| *c != '"')
                .collect();
            found.insert(format!("(deny {word} {name})"));
        }
    }
    for line in constant(source, "BASE").lines() {
        let line = line.trim();
        if line.starts_with("(allow signal") || line.starts_with("(allow process-info") {
            found.insert(line.to_owned());
        }
    }
    found
}

/// Which escapes the rules' file maps to, flattened, for the shapes it lists.
struct Map {
    mapped: Vec<(String, Vec<String>)>,
    /// Shape, escapes, why, and whether the debt is *partial*: its escape has cells, aimed at
    /// another rule, so the escape being covered does not retire it.
    debt: Vec<(String, Vec<String>, String, bool)>,
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
    let list = |key: &str| file[key].as_array().unwrap_or_else(|| panic!("{key}"));
    Map {
        mapped: list("rules")
            .iter()
            .map(|r| {
                (
                    r["shape"].as_str().expect("a shape").to_owned(),
                    escapes_of(r),
                )
            })
            .collect(),
        debt: list("debt")
            .iter()
            .map(|r| {
                (
                    r["shape"].as_str().expect("a shape").to_owned(),
                    escapes_of(r),
                    r["why"].as_str().expect("why").to_owned(),
                    r["partial"].as_bool().unwrap_or(false),
                )
            })
            .collect(),
    }
}

/// **What is wrong with `map` against `source`, the escape table and the coverage list**, one
/// line each: a shape in the source that is neither mapped nor debt, a listed shape no longer in
/// the source, a mapped escape that is not ticket 22's or has a Full or Basic cover that is not a
/// scenario, a debt that no longer owes, and a debt with no reason.
fn problems(map: &Map, source: &str, table: &Value, coverage: &Value) -> Vec<String> {
    let mut found = Vec::new();
    let in_source = shapes(source);
    let listed: BTreeSet<&str> = map
        .mapped
        .iter()
        .map(|(s, _)| s.as_str())
        .chain(map.debt.iter().map(|(s, _, _, _)| s.as_str()))
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
        .filter(|r| r["ticket"] == "22")
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
                    "`{shape}`: {id} is not a row of ticket 22 in the escape table"
                ));
            } else if !covered(id) {
                found.push(format!(
                    "`{shape}`: {id} has no scenario cell at Full and Basic"
                ));
            }
        }
    }
    for (shape, escapes, why, partial) in &map.debt {
        if why.trim().is_empty() {
            found.push(format!("`{shape}`: debt with no reason"));
        }
        if !partial && escapes.iter().all(|id| covered(id)) {
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
fn every_process_deny_rule_has_an_escape_cell() {
    let found = problems(
        &map_of(&read(here().join("process-rules.json"))),
        &source(),
        &read(here().join("escapes.json")),
        &read(here().join("coverage.json")),
    );
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_real_floor_gives_the_shapes_the_map_names() {
    // The source is read, not the map: a floor that grew a rule shows here first.
    let got = shapes(&source());
    for want in [
        "(deny appleevent-send)",
        "(deny lsopen)",
        "(deny job-creation)",
        "(deny process-exec /usr/bin/open)",
        "(deny mach-lookup com.apple.coreservices.launchservicesd)",
        "(allow signal (target same-sandbox))",
    ] {
        assert!(got.contains(want), "{want} is not among {got:?}");
    }
}

fn sample() -> &'static str {
    "pub const FLOOR_PROCESSES: &str = \"(deny lsopen)\n(deny mach-lookup\n  (global-name \\\"com.x.a\\\")\n  (global-name \\\"com.x.b\\\"))\n(deny process-exec\n  (literal \\\"/usr/bin/open\\\")\n  (literal \\\"/bin/launchctl\\\"))\n\";\n\
     const BASE: &str = \"(allow process-fork)\n(allow signal (target same-sandbox))\n(allow sysctl-read)\n\";"
}

fn empty() -> Map {
    Map {
        mapped: Vec::new(),
        debt: Vec::new(),
    }
}

#[test]
fn the_shapes_name_each_program_and_service_and_the_signal_allow() {
    let want: BTreeSet<String> = [
        "(deny lsopen)",
        "(deny mach-lookup com.x.a)",
        "(deny mach-lookup com.x.b)",
        "(deny process-exec /usr/bin/open)",
        "(deny process-exec /bin/launchctl)",
        "(allow signal (target same-sandbox))",
    ]
    .map(str::to_owned)
    .into();
    assert_eq!(shapes(sample()), want);
}

#[test]
fn a_process_rule_nobody_aimed_a_cell_at_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let found = problems(&empty(), sample(), &table, &coverage);
    assert_eq!(found.len(), 6, "{found:?}");
    assert!(
        found
            .iter()
            .any(|f| f.contains("(deny process-exec /usr/bin/open)")),
        "{found:?}"
    );
}

#[test]
fn a_rule_mapped_to_an_escape_with_no_cell_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: vec![("(deny lsopen)".into(), vec!["E-PROC-02".into()])],
        debt: Vec::new(),
    };
    let found = problems(
        &map,
        "const FLOOR_PROCESSES: &str = \"(deny lsopen)\";\nconst BASE: &str = \"\";",
        &table,
        &coverage,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("E-PROC-02 has no scenario cell"),
        "{found:?}"
    );
}

#[test]
fn a_partial_debt_is_not_retired_by_its_escape_having_cells() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: Vec::new(),
        debt: vec![(
            "(deny lsopen)".into(),
            vec!["E-PROC-03".into()],
            "another rule of the escape has the cell".into(),
            true,
        )],
    };
    let found = problems(
        &map,
        "const FLOOR_PROCESSES: &str = \"(deny lsopen)\";\nconst BASE: &str = \"\";",
        &table,
        &coverage,
    );
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_debt_whose_escapes_all_have_cells_is_found() {
    let table = read(here().join("escapes.json"));
    let coverage = read(here().join("coverage.json"));
    let map = Map {
        mapped: Vec::new(),
        debt: vec![(
            "(deny lsopen)".into(),
            vec!["E-PROC-03".into()],
            "late".into(),
            false,
        )],
    };
    let found = problems(
        &map,
        "const FLOOR_PROCESSES: &str = \"(deny lsopen)\";\nconst BASE: &str = \"\";",
        &table,
        &coverage,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("is no debt"), "{found:?}");
}
