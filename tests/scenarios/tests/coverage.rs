// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The matrix coverage gate** (T1.1-08, #6855): the repo's list over the real matrix, cells
//! and escape table, and each of the gate's rules against small lists written here.
//!
//! A change's own marks and list are judged against its base (`SCENARIOS_BASE`, a commit, or else
//! the merge base with `origin/phase-1-proven`), and its rulings are the base's `rulings.json`,
//! never its own: those tests need the base's history, so they are run by
//! `sandbox-scenarios-linux` (`--run-ignored all`), which sets it. A new issue is one opened after
//! the change began (`SCENARIOS_SINCE`, the change's merge base's commit time, or else read from
//! git), and that job refreshes `issues.json` from GitHub first. The issues a pull request closes
//! come from `SCENARIOS_PR_CLOSES` (`7001,7002`), which the same job sets.

use std::process::Command;

use purlis_scenarios::coverage::{
    Coverage, Issues, Kind, RULINGS_ALONE, Ran, Repo, Required, Rulings, Target, against_base,
    at_gate, check, expectations, issues, judge_ran_legs, judge_runs, ratchet, report, row_table,
};
use purlis_scenarios::model::{
    Batch, EntryKind, LegRecord, Os, Protection, ReportLeg, ReportVerdict, TimingsMode,
};
use purlis_scenarios::plan::Trigger;
use purlis_scenarios::root;
use purlis_scenarios::timings::catalogue;
use purlis_scenarios::validate::{RowTable, Validator};
use serde_json::{Value, json};

// ---- helpers -----------------------------------------------------------------------------------

/// A step that proves its call ran, with a Log expectation.
fn step() -> Value {
    json!({
        "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
        "require": [{"kind": "stop.block", "source": "sandbox"}]
    })
}

fn cell(id: &str) -> Value {
    json!({"id": id, "steps": [step()]})
}

fn batch(file: &str, protection: &str, cells: Value) -> (String, Batch) {
    let batch = Batch::from_value(json!({
        "batch": file.trim_start_matches("cells/").trim_end_matches(".json"),
        "protection": protection,
        "persona": "steward",
        "hosts": {"h1": "one.test"},
        "cells": cells,
    }))
    .expect("a batch");
    (file.to_string(), batch)
}

fn rows(value: Value) -> RowTable {
    serde_json::from_value(value).expect("a row table")
}

fn repo(batches: Vec<(String, Batch)>, table: RowTable, escapes: Value) -> Repo {
    Repo {
        root: root().join("../.."),
        batches,
        rows: table,
        escapes: serde_json::from_value(escapes).expect("escape rows"),
        issues: Issues::default(),
        rulings: rulings(),
        required: Required::default(),
        catalogue: catalogue(),
    }
}

/// The rulings the small lists here cite, each naming what it approves: Q330 two caps, its point
/// Q330.2 one, Q331 the row M-T-01, Q340 the marks of #7001 and #7002, and Q341 other things.
fn rulings() -> Rulings {
    serde_json::from_value(json!({"rulings": {
        "Q330": {"date": "2026-11-16", "caps": ["e2e.window", "fixture.race"]},
        "Q330.2": {"date": "2026-11-16", "caps": ["e2e.window"]},
        "Q331": {"date": "2026-11-16", "rows": ["M-T-01"]},
        "Q340": {"date": "2026-11-16", "marks": [7001, 7002]},
        "Q341": {"date": "2026-11-17", "rows": ["M-T-02"], "caps": ["fixture.time"], "marks": [7003]},
    }}))
    .expect("rulings")
}

fn list(value: Value) -> Coverage {
    Coverage::from_value(value).expect("a coverage list")
}

/// A row entry: `M-T-01` for `dialect` on `os`, stating a Log expectation only.
fn entry(row: &str, dialect: &str, os: &str, cover: &str) -> Value {
    json!({
        "row": row, "dialect": dialect, "os": os, "log": true, "window": false, "chat": false,
        "carried": "scenario", "kind": "scenario", "cover": cover,
    })
}

fn table_entry(row: &str, dialect: &str, os: &str) -> Value {
    json!({"row": row, "dialect": dialect, "os": os, "log": true, "window": false, "chat": false})
}

fn mentions(problems: &[String], words: &[&str]) -> bool {
    problems.iter().any(|p| words.iter().all(|w| p.contains(w)))
}

fn record(
    kind: EntryKind,
    row: &str,
    dialect: Option<&str>,
    protection: Option<Protection>,
    os: Os,
    leg: ReportLeg,
    verdict: ReportVerdict,
) -> LegRecord {
    LegRecord {
        kind,
        row: row.into(),
        test: None,
        dialect: dialect.map(|d| serde_json::from_value(json!(d)).expect("a dialect")),
        protection,
        os,
        leg,
        timings: Some(TimingsMode::Scaled),
        verdict,
        reason: (verdict == ReportVerdict::NotRun).then(|| "filtered out".into()),
    }
}

fn scaled(leg: ReportLeg, os: Os) -> Ran {
    Ran {
        leg,
        os,
        timings: TimingsMode::Scaled,
    }
}

/// `SCENARIOS_PR_CLOSES`, as numbers.
fn closing() -> Vec<u64> {
    std::env::var("SCENARIOS_PR_CLOSES")
        .unwrap_or_default()
        .split([',', ' '])
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches('#').parse().expect("an issue number"))
        .collect()
}

fn git(args: &[&str]) -> Option<String> {
    try_git(args).ok()
}

/// `git <args>` in `tests/scenarios`: its output, or why it failed.
fn try_git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// `path` (in `tests/scenarios`) at `commit`: `None` only when the commit has no such path, and
/// an error for anything else (a commit that isn't there, a shallow clone, a git that fails).
fn show_at(commit: &str, path: &str) -> Result<Option<String>, String> {
    if try_git(&["ls-tree", "--name-only", commit, "--", path])?.is_empty() {
        return Ok(None);
    }
    try_git(&["show", &format!("{commit}:./{path}")]).map(Some)
}

/// The commit this change's marks are judged against.
fn base() -> String {
    if let Ok(base) = std::env::var("SCENARIOS_BASE") {
        return base;
    }
    git(&["merge-base", "HEAD", "origin/phase-1-proven"])
        .expect("no base to judge the marks against: set SCENARIOS_BASE to a commit")
}

/// When this change began: `SCENARIOS_SINCE`, or else when its merge base with `base` was
/// committed. A mark's issue is new when it was opened after it.
fn since(base: &str) -> String {
    if let Some(since) = std::env::var("SCENARIOS_SINCE")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return since;
    }
    let fork = git(&["merge-base", "HEAD", base])
        .unwrap_or_else(|| panic!("no merge base with {base}: set SCENARIOS_SINCE"));
    committed_at(&fork)
}

/// When `commit` was committed, in UTC, as issues.json writes `opened`.
fn committed_at(commit: &str) -> String {
    let out = Command::new("git")
        .args([
            "show",
            "-s",
            "--date=format-local:%Y-%m-%dT%H:%M:%SZ",
            "--format=%cd",
            commit,
        ])
        .env("TZ", "UTC")
        .current_dir(root())
        .output()
        .expect("git runs");
    assert!(out.status.success(), "{commit}'s date can't be read");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The coverage list at `commit`, if it has one.
fn coverage_at(commit: &str) -> Option<Coverage> {
    let text = show_at(commit, "coverage.json").unwrap_or_else(|e| panic!("{e}"))?;
    Some(list(serde_json::from_str(&text).expect("json")))
}

/// The person's rulings at `commit`: none if it has no `rulings.json`.
fn rulings_at(commit: &str) -> Rulings {
    match show_at(commit, "rulings.json").unwrap_or_else(|e| panic!("{e}")) {
        Some(text) => {
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("rulings.json at {commit}: {e}"))
        }
        None => Rulings::default(),
    }
}

/// The cells files at `commit`.
fn cells_at(commit: &str) -> Vec<(String, Batch)> {
    let names = git(&["ls-tree", "--name-only", commit, "cells/"])
        .unwrap_or_else(|| panic!("the cells at {commit} can't be listed"));
    names
        .lines()
        .filter(|n| n.ends_with(".json"))
        .map(|name| {
            let text = git(&["show", &format!("{commit}:./{name}")])
                .unwrap_or_else(|| panic!("{name} at {commit} can't be read"));
            let batch = Batch::from_value(serde_json::from_str(&text).expect("json"))
                .unwrap_or_else(|e| panic!("{name} at {commit}: {e}"));
            (name.to_string(), batch)
        })
        .collect()
}

// ---- the repo's list ---------------------------------------------------------------------------

#[test]
fn every_matrix_row_names_a_cover_that_exists() {
    let repo = Repo::read().expect("the repo's cells, rows, escapes and issues");
    let list = Coverage::read().expect("coverage.json");
    let found = check(&list, &repo);
    assert!(found.is_empty(), "{found:#?}");
    println!("{}", report(&list, &repo));

    // A cover naming a cell no cells file has, or a test no file holds, is no cover.
    let mut broken = list.clone();
    broken.rows[0].cover = "scenario: cells/B0.json#M-NO-SUCH-01".into();
    broken.fixtures[0].cover = "fixture: tests/scenarios/tests/coverage.rs::no_such_test".into();
    let found = check(&broken, &repo);
    assert!(mentions(&found, &["M-NO-SUCH-01"]), "{found:#?}");
    assert!(mentions(&found, &["no_such_test"]), "{found:#?}");

    // A fixture cover naming a test that exists is a cover.
    let mut fixed = list.clone();
    fixed.fixtures[0].cover =
        "fixture: tests/scenarios/tests/coverage.rs::every_matrix_row_names_a_cover_that_exists"
            .into();
    let found = check(&fixed, &repo);
    assert!(found.is_empty(), "{found:#?}");

    // A row × dialect × OS of the matrix with no entry in the list has no cover.
    let mut short = list.clone();
    let gone = short.rows.remove(0);
    let found = check(&short, &repo);
    assert!(mentions(&found, &[&gone.row, "no cover"]), "{found:#?}");
}

#[test]
fn deleting_a_fixture_row_from_the_list_alone_fails_the_gate() {
    let repo = Repo::read().expect("the repo's cells, rows, escapes and issues");
    let list = Coverage::read().expect("coverage.json");
    for row in ["M-POL-01", "M-OBS-11", "R-006"] {
        let mut short = list.clone();
        short.fixtures.retain(|f| f.row != row);
        short.caps.clear();
        let found = check(&short, &repo);
        assert!(
            mentions(&found, &[row, "no cover", "fixtures"]),
            "{row}: {found:#?}"
        );
    }
    // A fixture row the matrix doesn't leave to fixtures is no cover of one.
    let mut extra = list.clone();
    let mut stray = extra.fixtures[0].clone();
    stray.row = "M-NET-01".into();
    extra.fixtures.push(stray);
    let found = check(&extra, &repo);
    assert!(
        mentions(
            &found,
            &["M-NET-01", "not a row the matrix leaves to fixtures"]
        ),
        "{found:#?}"
    );
}

#[test]
fn a_list_at_version_0_still_owes_every_real_binary_probe() {
    let repo = Repo::read().expect("the repo's cells, rows, escapes and issues");
    let list = Coverage::read().expect("coverage.json");
    assert_eq!(
        list.real_binary.len(),
        24,
        "three dialects × two OSes × two probes × Full and Basic"
    );
    let mut none = list.clone();
    none.version = 0;
    none.real_binary.clear();
    let found = check(&none, &repo);
    assert_eq!(
        found
            .iter()
            .filter(|p| p.contains("probe") && p.contains("no cover"))
            .count(),
        24,
        "{found:#?}"
    );
}

#[test]
fn every_cell_carries_each_expectation_kind_its_row_states() {
    let repo = Repo::read().expect("the repo's cells");
    let the_list = Coverage::read().expect("coverage.json");
    assert!(
        !row_table(&the_list).entries.is_empty(),
        "the list carries no Log/Window/Chat flags"
    );
    // Every cell a scenario cover names is held to its row's flags; a cell no cover names yet
    // (B0 before ticket 11 gives it the Window and Chat expectations its rows state) is not
    // coverage, and its rows stay pending.
    let mut validator = Validator::repo().expect("the validator");
    let found = expectations(&the_list, &repo, &mut validator);
    assert!(found.is_empty(), "{found:#?}");
    let plain = Validator::repo().expect("the validator");
    for (file, batch) in &repo.batches {
        let found = plain.check(batch);
        assert!(found.is_empty(), "{file}: {found:#?}");
    }

    // A scenario cover naming a cell with a Log expectation only, for a row that states a
    // Window and a Chat expectation too, is refused; the cell's id need not be the row's.
    let cells = vec![batch("cells/T.json", "full", json!([cell("M-T-01-cell")]))];
    let fixture = repo_with(cells, &repo_rows_only());
    let mut stated = entry(
        "M-T-01",
        "codex",
        "linux",
        "scenario: cells/T.json#M-T-01-cell",
    );
    stated["window"] = json!(true);
    stated["chat"] = json!(true);
    let found = expectations(
        &list(json!({"rows": [stated.clone()]})),
        &fixture,
        &mut validator,
    );
    assert!(mentions(&found, &["M-T-01-cell", "window"]), "{found:#?}");
    assert!(mentions(&found, &["M-T-01-cell", "chat"]), "{found:#?}");
    // The same row while its cover is pending holds no cell to anything.
    stated["cover"] = json!("pending: 11");
    let found = expectations(&list(json!({"rows": [stated]})), &fixture, &mut validator);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
#[ignore = "needs the change's base: sandbox-scenarios-linux runs it with SCENARIOS_BASE"]
fn the_marks_keep_their_ratchet_against_the_base() {
    let repo = Repo::read().expect("the repo's cells");
    let base = base();
    let found = ratchet(&cells_at(&base), &repo.batches, &repo.issues, &since(&base));
    assert!(found.is_empty(), "against {base}: {found:#?}");
}

#[test]
#[ignore = "needs the change's base: sandbox-scenarios-linux runs it with SCENARIOS_BASE"]
fn the_list_keeps_its_anchors_against_the_base() {
    let head = Coverage::read().expect("coverage.json");
    let base = base();
    // A base with no list is the change that brings it: the generator and `diff` are its anchor.
    let Some(was) = coverage_at(&base) else {
        println!("{base} has no coverage.json: nothing to anchor against");
        return;
    };
    // The base's rulings, never this change's: a ruling is added in a pull request of its own.
    let found = against_base(&was, &head, &rulings_at(&base));
    assert!(found.is_empty(), "against {base}: {found:#?}");
}

#[test]
fn a_base_that_can_t_be_read_fails_the_anchored_test() {
    // A path the base doesn't have is the only failure that reads as "nothing there".
    let head = git(&["rev-parse", "HEAD"]).expect("this checkout's HEAD");
    assert_eq!(show_at(&head, "no-such-file.json"), Ok(None));
    assert!(matches!(show_at(&head, "rulings.json"), Ok(Some(_))));
    // A commit that isn't there is an error, never "no list".
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let found = show_at(missing, "coverage.json");
    assert!(found.is_err(), "{found:?}");
    let caught = std::panic::catch_unwind(|| coverage_at(missing));
    assert!(
        caught.is_err(),
        "a base that can't be read reads as no list"
    );
}

#[test]
fn no_mark_names_a_closed_issue_or_one_this_change_closes() {
    let repo = Repo::read().expect("the repo's cells and issue cache");
    let found = issues(&repo.batches, &repo.issues, &closing());
    assert!(found.is_empty(), "{found:#?}");
}

/// M74.1's gate itself: run at the gate merge with `--ignored`.
#[test]
#[ignore = "M74.1's gate: run with --ignored at the gate merge"]
fn at_m74_1_s_gate_nothing_is_pending_and_every_mark_is_approved() {
    let repo = Repo::read().expect("the repo's cells");
    let list = Coverage::read().expect("coverage.json");
    let found = at_gate(&list, &repo.batches, &repo.rulings);
    assert!(found.is_empty(), "{found:#?}");
}

// ---- the list's rules ----------------------------------------------------------------------------

#[test]
fn a_row_covered_for_one_dialect_only_fails_the_gate() {
    let table = rows(json!([
        table_entry("M-T-01", "claude", "linux"),
        table_entry("M-T-01", "codex", "linux"),
        table_entry("M-T-01", "opencode", "linux"),
    ]));
    let cells = vec![batch("cells/T.json", "full", json!([cell("M-T-01")]))];
    let repo = repo(cells, table, json!([]));
    let one = list(
        json!({"rows": [entry("M-T-01", "claude", "linux", "scenario: cells/T.json#M-T-01")]}),
    );
    let found = check(&one, &repo);
    assert!(
        mentions(&found, &["M-T-01", "codex", "no cover"]),
        "{found:#?}"
    );
    assert!(
        mentions(&found, &["M-T-01", "opencode", "no cover"]),
        "{found:#?}"
    );

    let all = list(json!({"rows": [
        entry("M-T-01", "claude", "linux", "scenario: cells/T.json#M-T-01"),
        entry("M-T-01", "codex", "linux", "scenario: cells/T.json#M-T-01"),
        entry("M-T-01", "opencode", "linux", "pending: 11"),
    ]}));
    assert_eq!(check(&all, &repo), Vec::<String>::new());

    // A cell that leaves the dialect out does not cover it.
    let claude_only = vec![batch(
        "cells/T.json",
        "full",
        json!([{"id": "M-T-01", "dialects": ["claude"], "steps": [step()]}]),
    )];
    let repo = repo_with(claude_only, &repo);
    let found = check(&all, &repo);
    assert!(mentions(&found, &["M-T-01", "codex"]), "{found:#?}");
}

fn repo_with(batches: Vec<(String, Batch)>, like: &Repo) -> Repo {
    Repo {
        root: like.root.clone(),
        batches,
        rows: like.rows.clone(),
        escapes: like.escapes.clone(),
        issues: like.issues.clone(),
        rulings: like.rulings.clone(),
        required: like.required.clone(),
        catalogue: catalogue(),
    }
}

#[test]
fn a_live_cover_over_its_reasons_cap_fails_the_gate() {
    let table = rows(json!([
        table_entry("M-T-01", "claude", "linux"),
        table_entry("M-T-02", "claude", "linux"),
    ]));
    let repo = repo(vec![], table, json!([]));
    let live = |row: &str, reason: &str| {
        let mut e = entry(row, "claude", "linux", "pending: 37");
        e["carried"] = json!("live");
        e["kind"] = json!("live");
        e["reason"] = json!(reason);
        e
    };
    let over = list(json!({
        "caps": {"live": {"model_behaviour": {"seed": 1, "cap": 1}}},
        "rows": [live("M-T-01", "model_behaviour"), live("M-T-02", "model_behaviour")],
    }));
    let found = check(&over, &repo);
    assert!(mentions(&found, &["model_behaviour", "cap"]), "{found:#?}");

    let within = list(json!({
        "caps": {"live": {"model_behaviour": {"seed": 2, "cap": 2}}},
        "rows": [live("M-T-01", "model_behaviour"), live("M-T-02", "model_behaviour")],
    }));
    assert_eq!(check(&within, &repo), Vec::<String>::new());

    // A live reason off the closed list is refused, capped or not.
    let off_list = list(json!({
        "caps": {"live": {"model_behaviour": {"seed": 1, "cap": 1}, "too slow": {"seed": 1, "cap": 1}}},
        "rows": [live("M-T-01", "model_behaviour"), live("M-T-02", "too slow")],
    }));
    let found = check(&off_list, &repo);
    assert!(
        mentions(&found, &["too slow", "not on the list"]),
        "{found:#?}"
    );
}

#[test]
fn an_unrecorded_cap_raise_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let e2e = {
        let mut e = entry("M-T-01", "claude", "linux", "pending: 18");
        e["carried"] = json!("e2e");
        e["kind"] = json!("e2e");
        e["reason"] = json!("window");
        e
    };
    let raised =
        list(json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3}}}, "rows": [e2e.clone()]}));
    let found = check(&raised, &repo);
    assert!(mentions(&found, &["window", "seed", "Q-id"]), "{found:#?}");

    let approved = list(
        json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": "Q330"}}}, "rows": [e2e.clone()]}),
    );
    assert_eq!(check(&approved, &repo), Vec::<String>::new());

    let not_a_ruling = list(
        json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": "yes"}}}, "rows": [e2e.clone()]}),
    );
    let found = check(&not_a_ruling, &repo);
    assert!(mentions(&found, &["window", "Q-id"]), "{found:#?}");

    // A Q-id of the right shape that is none of the person's rulings approves nothing; a point of
    // one of them does.
    let unknown = list(
        json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": "Q999"}}}, "rows": [e2e.clone()]}),
    );
    let found = check(&unknown, &repo);
    assert!(mentions(&found, &["Q999", "rulings.json"]), "{found:#?}");
    let point = list(
        json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": "Q330.2"}}}, "rows": [e2e.clone()]}),
    );
    assert_eq!(check(&point, &repo), Vec::<String>::new());
    // A point that isn't listed exactly, and a ruling that names other things, approve nothing.
    for (q, why) in [("Q330.9", "no ruling"), ("Q331", "does not name")] {
        let raised = list(
            json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": q}}}, "rows": [e2e.clone()]}),
        );
        let found = check(&raised, &repo);
        assert!(mentions(&found, &["window", q, why]), "{q}: {found:#?}");
    }
}

#[test]
fn the_repo_s_rulings_are_the_person_s_m74_1_rulings() {
    let repo = Repo::read().expect("rulings.json");
    assert_eq!(repo.rulings.problems(), Vec::<String>::new());
    let wide = Target::Cap(Kind::Fixture, "machine_wide_state");
    assert_eq!(repo.rulings.refuses("Q321", wide), None, "matrix v5's cap");
    assert!(
        repo.rulings
            .refuses("Q321", Target::Cap(Kind::Fixture, "race"))
            .is_some(),
        "a cap it does not name"
    );
    assert!(
        repo.rulings.refuses("Q321.8", wide).is_some(),
        "a point not listed"
    );
    assert!(
        repo.rulings.refuses("Q999", wide).is_some(),
        "no such ruling"
    );
    assert!(
        repo.rulings
            .comment
            .as_deref()
            .is_some_and(|c| c.contains("pull request of its own")),
        "rulings.json says a ruling is added on its own"
    );
}

#[test]
fn moving_a_row_off_scenario_without_a_q_id_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let mut moved = entry("M-T-01", "claude", "linux", "pending: 19");
    moved["kind"] = json!("fixture");
    moved["reason"] = json!("race");
    let caps = json!({"fixture": {"race": {"seed": 1, "cap": 1}}});
    let found = check(&list(json!({"caps": caps, "rows": [moved.clone()]})), &repo);
    assert!(
        mentions(&found, &["M-T-01", "off scenario", "Q-id"]),
        "{found:#?}"
    );

    moved["moved"] = json!("Q999");
    let found = check(&list(json!({"caps": caps, "rows": [moved.clone()]})), &repo);
    assert!(
        mentions(&found, &["M-T-01", "Q999", "rulings.json"]),
        "{found:#?}"
    );

    // A ruling that names another row, or a point of the right one that isn't listed.
    for (q, why) in [("Q341", "does not name `M-T-01`"), ("Q331.99", "no ruling")] {
        moved["moved"] = json!(q);
        let found = check(&list(json!({"caps": caps, "rows": [moved.clone()]})), &repo);
        assert!(mentions(&found, &["M-T-01", q, why]), "{q}: {found:#?}");
    }

    moved["moved"] = json!("Q331");
    let found = check(&list(json!({"caps": caps, "rows": [moved.clone()]})), &repo);
    assert_eq!(found, Vec::<String>::new());

    // A kind off what the row carries needs its ruling whatever the row carries.
    moved["carried"] = json!("live");
    moved.as_object_mut().map(|o| o.remove("moved"));
    let found = check(&list(json!({"caps": caps, "rows": [moved]})), &repo);
    assert!(
        mentions(&found, &["M-T-01", "off live to fixture", "Q-id"]),
        "{found:#?}"
    );
}

// ---- the list against its base ------------------------------------------------------------------

fn capped(seed: usize, cap: usize, approved: Option<&str>) -> Coverage {
    let mut c = json!({"seed": seed, "cap": cap});
    if let Some(q) = approved {
        c["approved"] = json!(q);
    }
    list(json!({"caps": {"fixture": {"race": c}}}))
}

#[test]
fn raising_a_seed_with_its_cap_against_the_base_needs_a_q_id() {
    let base = capped(2, 2, None);
    // Seed and cap raised together pass the list's own rule, and not the base's.
    let raised = capped(4, 4, None);
    assert_eq!(check(&raised, &repo_rows_only()), Vec::<String>::new());
    let found = against_base(&base, &raised, &rulings());
    assert!(
        mentions(
            &found,
            &["fixture cap", "race", "4", "base allowed", "Q-id"]
        ),
        "{found:#?}"
    );
    // With a ruling the base's cap already named, it doesn't hold either. (Which rulings approve
    // it is `each_way_round_a_ruling_against_the_base_is_refused`'s.)
    let reused = against_base(
        &capped(2, 3, Some("Q330")),
        &capped(4, 4, Some("Q330")),
        &rulings(),
    );
    assert!(
        mentions(&reused, &["race", "Q330", "already cited"]),
        "{reused:#?}"
    );
    // A cap the base already allowed, or one lowered, needs nothing.
    assert_eq!(
        against_base(
            &capped(2, 3, Some("Q330")),
            &capped(3, 3, Some("Q330")),
            &rulings()
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        against_base(&base, &capped(1, 1, None), &rulings()),
        Vec::<String>::new()
    );
    // A reason the base had no cap for starts from nothing.
    let found = against_base(&list(json!({})), &capped(1, 1, None), &rulings());
    assert!(mentions(&found, &["race", "Q-id"]), "{found:#?}");
}

fn row_entry(carried: &str, kind: &str, binary: Option<&str>, moved: Option<&str>) -> Value {
    let mut e = entry("M-T-01", "codex", "linux", "pending: 19");
    e["carried"] = json!(carried);
    e["kind"] = json!(kind);
    if kind != "scenario" {
        e["reason"] = json!("race");
    }
    if let Some(b) = binary {
        e["binary"] = json!(b);
    }
    if let Some(q) = moved {
        e["moved"] = json!(q);
    }
    e
}

#[test]
fn moving_carried_with_kind_against_the_base_needs_moved() {
    let base = list(json!({"rows": [row_entry("scenario", "scenario", None, None)]}));
    // `carried` and `kind` moved together pass the list's own rule, and not the base's.
    let both = list(json!({
        "caps": {"fixture": {"race": {"seed": 1, "cap": 1}}},
        "rows": [row_entry("fixture", "fixture", None, None)],
    }));
    let table = rows(json!([table_entry("M-T-01", "codex", "linux")]));
    assert_eq!(
        check(&both, &repo(vec![], table, json!([]))),
        Vec::<String>::new()
    );
    let found = against_base(&base, &both, &rulings());
    assert!(
        mentions(
            &found,
            &[
                "M-T-01 codex linux",
                "carried",
                "scenario to fixture",
                "moved"
            ]
        ),
        "{found:#?}"
    );
    let ruled = list(json!({"rows": [row_entry("fixture", "fixture", None, Some("Q331"))]}));
    assert_eq!(
        against_base(&base, &ruled, &rulings()),
        Vec::<String>::new()
    );
    let not_a_ruling = list(json!({"rows": [row_entry("fixture", "fixture", None, Some("Q999"))]}));
    assert!(!against_base(&base, &not_a_ruling, &rulings()).is_empty());
}

#[test]
fn dropping_a_binary_leg_against_the_base_is_refused() {
    let base = list(json!({"rows": [row_entry("scenario", "scenario", Some("both"), None)]}));
    for fewer in ["fake", "real"] {
        let head = list(json!({"rows": [row_entry("scenario", "scenario", Some(fewer), None)]}));
        let found = against_base(&base, &head, &rulings());
        assert!(
            mentions(
                &found,
                &["M-T-01 codex linux", "both binaries", "a leg is dropped"]
            ),
            "{fewer}: {found:#?}"
        );
    }
    // Leaving `binary` out is the fake binary alone.
    let mut bare = row_entry("scenario", "scenario", None, None);
    bare.as_object_mut().map(|o| o.remove("binary"));
    let found = against_base(&base, &list(json!({"rows": [bare]})), &rulings());
    assert!(mentions(&found, &["a leg is dropped"]), "{found:#?}");
    // Adding a leg is not dropping one.
    let fake = list(json!({"rows": [row_entry("scenario", "scenario", Some("fake"), None)]}));
    assert_eq!(against_base(&fake, &base, &rulings()), Vec::<String>::new());
}

/// The base's rulings, for the tests below: every ruling of [`rulings`] but Q341, which only the
/// change's own `rulings.json` holds.
fn base_rulings() -> Rulings {
    let mut r = rulings();
    r.rulings.remove("Q341");
    r
}

#[test]
fn each_way_round_a_ruling_against_the_base_is_refused() {
    let refused = |found: &[String], words: &[&str]| {
        assert!(mentions(found, words), "{words:?}: {found:#?}");
        assert!(
            found.iter().all(|p| p.contains(RULINGS_ALONE)),
            "{found:#?}"
        );
    };
    // A cap raised by a ruling that names a row, by a point not listed, by one only the change's
    // rulings hold; and by the ruling of the base's that names the cap.
    let base = capped(2, 2, None);
    let raise = |q: &str| against_base(&base, &capped(4, 4, Some(q)), &base_rulings());
    refused(
        &raise("Q331"),
        &["race", "Q331", "does not name `fixture.race`"],
    );
    refused(&raise("Q330.9"), &["race", "Q330.9", "no ruling"]);
    refused(&raise("Q341"), &["race", "Q341", "no ruling"]);
    assert_eq!(raise("Q330"), Vec::<String>::new());

    // A row's carried kind moved, a leg dropped, by a ruling that names another row, a point not
    // listed, or one only the change holds.
    let base = list(json!({"rows": [row_entry("scenario", "scenario", Some("both"), None)]}));
    for q in ["Q330", "Q331.99", "Q341"] {
        let carried =
            list(json!({"rows": [row_entry("fixture", "fixture", Some("both"), Some(q))]}));
        refused(
            &against_base(&base, &carried, &base_rulings()),
            &["M-T-01 codex linux", "carried moves", q],
        );
        let dropped =
            list(json!({"rows": [row_entry("scenario", "scenario", Some("fake"), Some(q))]}));
        refused(
            &against_base(&base, &dropped, &base_rulings()),
            &["M-T-01 codex linux", "a leg is dropped", q],
        );
    }
    let ruled =
        list(json!({"rows": [row_entry("fixture", "fixture", Some("fake"), Some("Q331"))]}));
    assert_eq!(
        against_base(&base, &ruled, &base_rulings()),
        Vec::<String>::new()
    );

    // A mark approved for the gate by a ruling that names another issue, or one only the change
    // holds; and by the base's ruling that names it. A mark the base already approved stays.
    let approving = |q: &str| {
        list(
            json!({"approved_marks": [{"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": q}]}),
        )
    };
    let none = list(json!({}));
    refused(
        &against_base(&none, &approving("Q330"), &base_rulings()),
        &["cells/T.json#M-T-01", "#7001", "does not name 7001"],
    );
    refused(
        &against_base(&none, &approving("Q341"), &base_rulings()),
        &["#7001", "Q341", "no ruling"],
    );
    assert_eq!(
        against_base(&none, &approving("Q340"), &base_rulings()),
        Vec::<String>::new()
    );
    assert_eq!(
        against_base(&approving("Q341"), &approving("Q341"), &base_rulings()),
        Vec::<String>::new()
    );
}

#[test]
fn moving_a_row_s_kind_off_what_it_carries_against_the_base_needs_moved() {
    let base = list(json!({"rows": [row_entry("scenario", "scenario", None, None)]}));
    // `carried` left alone, `kind` moved off it: refused without a ruling of the base's.
    let off = |q: Option<&str>| list(json!({"rows": [row_entry("scenario", "fixture", None, q)]}));
    let found = against_base(&base, &off(None), &base_rulings());
    assert!(
        mentions(
            &found,
            &[
                "M-T-01 codex linux",
                "kind moves from scenario to fixture",
                "no Q-id"
            ]
        ),
        "{found:#?}"
    );
    let found = against_base(&base, &off(Some("Q330")), &base_rulings());
    assert!(mentions(&found, &["kind moves", "Q330"]), "{found:#?}");
    assert_eq!(
        against_base(&base, &off(Some("Q331")), &base_rulings()),
        Vec::<String>::new()
    );
    // Back to the kind it carries needs nothing.
    assert_eq!(
        against_base(&off(Some("Q331")), &base, &base_rulings()),
        Vec::<String>::new()
    );
}

#[test]
fn a_real_cover_going_back_to_pending_needs_a_ruling() {
    let real = "scenario: cells/T.json#M-T-01";
    let row = |cover: &str, q: Option<&str>| {
        let mut e = row_entry("scenario", "scenario", None, q);
        e["cover"] = json!(cover);
        list(json!({"rows": [e]}))
    };
    let found = against_base(&row(real, None), &row("pending: 11", None), &base_rulings());
    assert!(
        mentions(
            &found,
            &["M-T-01 codex linux", "goes back to `pending: 11`"]
        ),
        "{found:#?}"
    );
    assert_eq!(
        against_base(
            &row(real, None),
            &row("pending: 11", Some("Q331")),
            &base_rulings()
        ),
        Vec::<String>::new()
    );
    // A pending cover that stays pending, or becomes real, needs nothing.
    assert_eq!(
        against_base(
            &row("pending: 11", None),
            &row("pending: 12", None),
            &base_rulings()
        ),
        Vec::<String>::new()
    );

    let fixture = |cover: &str| {
        list(
            json!({"fixtures": [{"row": "M-T-01", "os": "linux", "reason": "race", "cover": cover}]}),
        )
    };
    let found = against_base(
        &fixture("fixture: a.rs::a_test"),
        &fixture("pending: 19"),
        &base_rulings(),
    );
    assert!(
        mentions(&found, &["M-T-01", "on linux", "goes back"]),
        "{found:#?}"
    );

    let escape = |cover: &str, control: &str| {
        list(json!({"escapes": [{"id": "E-T-01", "covers": {"full": cover}, "control": control}]}))
    };
    let found = against_base(
        &escape(real, real),
        &escape("pending: 20", "pending: 20"),
        &base_rulings(),
    );
    assert!(
        mentions(&found, &["E-T-01", "at full", "back to pending"]),
        "{found:#?}"
    );
    assert!(
        mentions(&found, &["E-T-01", "control", "back to pending"]),
        "{found:#?}"
    );

    let probe = |cover: &str| {
        list(
            json!({"real_binary": [{"probe": "network", "dialect": "codex", "os": "linux", "protection": "full", "cover": cover}]}),
        )
    };
    let found = against_base(&probe(real), &probe("pending: 53"), &base_rulings());
    assert!(
        mentions(
            &found,
            &["network-probe", "codex linux at full", "goes back"]
        ),
        "{found:#?}"
    );
}

#[test]
fn a_fixture_row_s_test_leaving_an_os_needs_a_ruling() {
    let fixture = |os: &str, q: Option<&str>| {
        let mut f =
            json!({"row": "M-T-01", "os": os, "reason": "race", "cover": "fixture: a.rs::a_test"});
        if let Some(q) = q {
            f["moved"] = json!(q);
        }
        list(json!({"fixtures": [f]}))
    };
    let found = against_base(
        &fixture("linux", None),
        &fixture("macos", None),
        &base_rulings(),
    );
    assert!(
        mentions(&found, &["M-T-01", "leaves linux", "no Q-id"]),
        "{found:#?}"
    );
    let found = against_base(
        &fixture("linux", None),
        &fixture("macos", Some("Q341")),
        &base_rulings(),
    );
    assert!(mentions(&found, &["leaves linux", "Q341"]), "{found:#?}");
    assert_eq!(
        against_base(
            &fixture("linux", None),
            &fixture("macos", Some("Q331")),
            &base_rulings()
        ),
        Vec::<String>::new()
    );
    // Running it on one more OS is not leaving one.
    let both = list(json!({"fixtures": [
        {"row": "M-T-01", "os": "linux", "reason": "race", "cover": "fixture: a.rs::a_test"},
        {"row": "M-T-01", "os": "macos", "reason": "race", "cover": "fixture: a.rs::a_test"},
    ]}));
    assert_eq!(
        against_base(&fixture("linux", None), &both, &base_rulings()),
        Vec::<String>::new()
    );
}

// ---- the list's other rules ----------------------------------------------------------------------

#[test]
fn a_row_whose_flags_are_not_the_matrix_s_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let mut flagged = entry("M-T-01", "claude", "linux", "pending: 11");
    flagged["window"] = json!(true);
    let found = check(&list(json!({"rows": [flagged]})), &repo);
    assert!(
        mentions(&found, &["M-T-01 claude linux", "Log/Window/Chat flags"]),
        "{found:#?}"
    );
}

#[test]
fn a_row_the_matrix_s_table_does_not_have_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let found = check(
        &list(json!({"rows": [
            entry("M-T-01", "claude", "linux", "pending: 11"),
            entry("M-T-01", "claude", "macos", "pending: 11"),
        ]})),
        &repo,
    );
    assert!(
        mentions(
            &found,
            &["M-T-01 claude macos", "not in the matrix's row table"]
        ),
        "{found:#?}"
    );
}

#[test]
fn a_row_listed_twice_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let found = check(
        &list(json!({"rows": [
            entry("M-T-01", "claude", "linux", "pending: 11"),
            entry("M-T-01", "claude", "linux", "pending: 11"),
        ]})),
        &repo,
    );
    assert!(
        mentions(&found, &["M-T-01 claude linux", "listed twice"]),
        "{found:#?}"
    );
}

#[test]
fn a_cover_of_another_kind_than_its_entry_fails_the_gate() {
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let repo = repo(vec![], table, json!([]));
    let mut e2e = entry(
        "M-T-01",
        "claude",
        "linux",
        "fixture: tests/scenarios/tests/coverage.rs::a_cover_of_another_kind_than_its_entry_fails_the_gate",
    );
    e2e["carried"] = json!("e2e");
    e2e["kind"] = json!("e2e");
    e2e["reason"] = json!("window");
    let found = check(
        &list(json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 1}}}, "rows": [e2e]})),
        &repo,
    );
    assert!(
        mentions(&found, &["M-T-01", "a fixture cover", "kind is e2e"]),
        "{found:#?}"
    );
}

#[test]
fn an_e2e_cover_names_its_test_by_its_exact_title() {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(
        dir.path().join("window.spec.ts"),
        "test(\"the marker counts every block\", async () => {});\ntest(\n  'a long title, broken by the formatter',\n  async () => {},\n);\n",
    )
    .expect("a spec");
    let table = rows(json!([table_entry("M-T-01", "claude", "linux")]));
    let mut fixture = repo(vec![], table, json!([]));
    fixture.root = dir.path().to_path_buf();
    let e2e = |title: &str| {
        let mut e = entry(
            "M-T-01",
            "claude",
            "linux",
            &format!("e2e: window.spec.ts#{title}"),
        );
        e["carried"] = json!("e2e");
        e["kind"] = json!("e2e");
        e["reason"] = json!("window");
        list(json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 1}}}, "rows": [e]}))
    };
    assert_eq!(
        check(&e2e("the marker counts every block"), &fixture),
        Vec::<String>::new()
    );
    assert_eq!(
        check(&e2e("a long title, broken by the formatter"), &fixture),
        Vec::<String>::new()
    );
    for part in ["the marker", "counts every block", "marker counts every"] {
        let found = check(&e2e(part), &fixture);
        assert!(
            mentions(&found, &["has no test", part]),
            "{part}: {found:#?}"
        );
    }
}

#[test]
fn a_fixture_or_e2e_cover_without_a_listed_reason_fails_the_gate() {
    let table = rows(json!([
        table_entry("M-T-01", "claude", "linux"),
        table_entry("M-T-02", "claude", "linux"),
    ]));
    let repo = repo(vec![], table, json!([]));
    let mut no_reason = entry("M-T-01", "claude", "linux", "pending: 18");
    no_reason["carried"] = json!("e2e");
    no_reason["kind"] = json!("e2e");
    let mut wrong_reason = entry("M-T-02", "claude", "linux", "pending: 18");
    wrong_reason["carried"] = json!("e2e");
    wrong_reason["kind"] = json!("e2e");
    wrong_reason["reason"] = json!("fault");
    let found = check(
        &list(json!({
            "caps": {"e2e": {"fault": {"seed": 1, "cap": 1}}},
            "rows": [no_reason, wrong_reason],
            "fixtures": [{"row": "M-T-F1", "os": "linux", "reason": "flaky", "cover": "pending: 19"}],
        })),
        &repo,
    );
    assert!(mentions(&found, &["M-T-01", "reason"]), "{found:#?}");
    assert!(
        mentions(&found, &["M-T-02", "fault", "not on the list"]),
        "{found:#?}"
    );
    assert!(
        mentions(&found, &["M-T-F1", "flaky", "not on the list"]),
        "{found:#?}"
    );

    let listed = list(json!({
        "caps": {"fixture": {"race": {"seed": 1, "cap": 1}}},
        "fixtures": [{"row": "M-T-F1", "os": "linux", "reason": "race", "cover": "pending: 19"}],
    }));
    let repo = repo_with(vec![], &repo_rows_only());
    assert_eq!(check(&listed, &repo), Vec::<String>::new());
}

fn repo_rows_only() -> Repo {
    repo(vec![], RowTable::default(), json!([]))
}

// ---- marks -------------------------------------------------------------------------------------

fn marked(id: &str, marks: Value) -> Value {
    json!({"id": id, "steps": [step()], "expected_fail": marks})
}

fn mark(issue: u64, scope: Value) -> Value {
    let mut m = json!({"issue": issue, "step": 1, "assertion": "require[0]", "observed": 0});
    if let (Some(m), Some(scope)) = (m.as_object_mut(), scope.as_object()) {
        m.extend(scope.clone());
    }
    m
}

/// The issues the marks here name: 7001 and 7003 opened before [`BASE_TIME`], 7002 after it.
fn cache() -> Issues {
    serde_json::from_value(json!({"issues": {
        "7001": {"state": "open", "opened": "2026-11-01T09:00:00Z"},
        "7002": {"state": "open", "opened": "2026-11-20T09:00:00Z"},
        "7003": {"state": "open", "opened": "2026-11-02T09:00:00Z"},
    }}))
    .expect("an issue cache")
}

const BASE_TIME: &str = "2026-11-10T00:00:00Z";

#[test]
fn a_mark_on_an_existing_cell_without_a_new_issue_fails_the_gate() {
    let ratchet = |base: &[(String, Batch)], head: &[(String, Batch)]| {
        ratchet(base, head, &cache(), BASE_TIME)
    };
    let base = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(7001, json!({"dialect": "claude"}))])
        )]),
    )];

    // The same issue, on the same cell, for a dialect it did not mark: refused.
    let wider = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([
                mark(7001, json!({"dialect": "claude"})),
                mark(7001, json!({"dialect": "codex", "os": "linux"})),
            ])
        )]),
    )];
    let found = ratchet(&base, &wider);
    assert!(
        mentions(&found, &["M-T-01", "7001", "codex", "linux"]),
        "{found:#?}"
    );
    assert!(!mentions(&found, &["for claude "]), "{found:#?}");

    // A new issue's mark on the existing cell: allowed.
    let new_issue = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([
                mark(7001, json!({"dialect": "claude"})),
                mark(7002, json!({"dialect": "codex"})),
            ])
        )]),
    )];
    assert_eq!(ratchet(&base, &new_issue), Vec::<String>::new());

    // A cell new in the change may carry a mark for an existing issue.
    let new_cell = vec![batch(
        "cells/T.json",
        "full",
        json!([
            marked("M-T-01", json!([mark(7001, json!({"dialect": "claude"}))])),
            marked("M-T-02", json!([mark(7001, json!({}))])),
        ]),
    )];
    assert_eq!(ratchet(&base, &new_cell), Vec::<String>::new());

    // Marks are only ever removed from an existing cell otherwise.
    let removed = vec![batch("cells/T.json", "full", json!([cell("M-T-01")]))];
    assert_eq!(ratchet(&base, &removed), Vec::<String>::new());

    // Widening a mark's leg or timings is adding one.
    let leg_scoped = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(
                7001,
                json!({"dialect": "claude", "leg": "fake", "timings": "scaled"})
            )])
        )]),
    )];
    let widened = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(
                7001,
                json!({"dialect": "claude", "timings": "scaled"})
            )])
        )]),
    )];
    let found = ratchet(&leg_scoped, &widened);
    assert!(
        mentions(&found, &["M-T-01", "7001", "real_wrap"]),
        "{found:#?}"
    );
}

#[test]
fn a_mark_for_an_issue_opened_before_the_base_is_not_a_new_issue_s() {
    let base = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(7001, json!({"dialect": "claude"}))])
        )]),
    )];
    let with = |issue: u64| {
        vec![batch(
            "cells/T.json",
            "full",
            json!([marked(
                "M-T-01",
                json!([
                    mark(7001, json!({"dialect": "claude"})),
                    mark(issue, json!({"dialect": "codex"})),
                ])
            )]),
        )]
    };
    // No mark of the base names #7003, and it was opened before the base: not new.
    let found = ratchet(&base, &with(7003), &cache(), BASE_TIME);
    assert!(
        mentions(&found, &["M-T-01", "7003", "opened after the base"]),
        "{found:#?}"
    );
    // An issue the cache doesn't have is not known to be new.
    let found = ratchet(&base, &with(7004), &cache(), BASE_TIME);
    assert!(mentions(&found, &["M-T-01", "7004"]), "{found:#?}");
    // #7002 was opened after the base.
    assert_eq!(
        ratchet(&base, &with(7002), &cache(), BASE_TIME),
        Vec::<String>::new()
    );
}

#[test]
fn an_expected_fail_on_a_closed_issue_fails_the_gate() {
    let head = vec![batch(
        "cells/T.json",
        "full",
        json!([
            marked("M-T-01", json!([mark(7001, json!({}))])),
            marked("M-T-02", json!([mark(7002, json!({}))])),
            marked("M-T-03", json!([mark(7003, json!({}))])),
        ]),
    )];
    let cache: Issues = serde_json::from_value(json!({
        "issues": {
            "7001": {"state": "open", "opened": "2026-11-01T09:00:00Z"},
            "7002": {"state": "closed", "opened": "2026-11-01T09:00:00Z"},
        }
    }))
    .expect("an issue cache");
    let found = issues(&head, &cache, &[]);
    assert!(
        mentions(&found, &["M-T-02", "7002", "closed"]),
        "{found:#?}"
    );
    assert!(
        mentions(&found, &["M-T-03", "7003", "issues.json"]),
        "{found:#?}"
    );
    assert!(!mentions(&found, &["7001"]), "{found:#?}");

    // A change that closes an issue its marks still name.
    let found = issues(&head[..], &cache, &[7001]);
    assert!(
        mentions(&found, &["M-T-01", "7001", "closes"]),
        "{found:#?}"
    );
}

#[test]
fn the_gate_fails_on_any_expected_fail_the_person_did_not_approve() {
    let head = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(7001, json!({})), mark(7002, json!({}))])
        )]),
    )];
    let covered = |approved: Value| {
        list(json!({
            "approved_marks": approved,
            "rows": [entry("M-T-01", "claude", "linux", "scenario: cells/T.json#M-T-01")],
        }))
    };
    let found = at_gate(&covered(json!([])), &head, &rulings());
    assert!(mentions(&found, &["M-T-01", "7001"]), "{found:#?}");
    assert!(mentions(&found, &["M-T-01", "7002"]), "{found:#?}");

    // A Q-id that is none of the person's rulings approves nothing.
    let unknown =
        covered(json!([{"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q999"}]));
    let found = at_gate(&unknown, &head, &rulings());
    assert!(mentions(&found, &["M-T-01", "7001"]), "{found:#?}");

    // Nor does a ruling that names another issue's mark.
    let other =
        covered(json!([{"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q341"}]));
    let found = at_gate(&other, &head, &rulings());
    assert!(mentions(&found, &["M-T-01", "7001"]), "{found:#?}");

    let one = covered(json!([{"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q340"}]));
    let found = at_gate(&one, &head, &rulings());
    assert!(!mentions(&found, &["7001"]), "{found:#?}");
    assert!(mentions(&found, &["M-T-01", "7002"]), "{found:#?}");

    let both = covered(json!([
        {"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q340"},
        {"issue": 7002, "cell": "cells/T.json#M-T-01", "approved": "Q340"},
    ]));
    assert_eq!(at_gate(&both, &head, &rulings()), Vec::<String>::new());

    // A pending cover is no cover at the gate.
    let pending = list(json!({"rows": [entry("M-T-02", "codex", "linux", "pending: 11")]}));
    let found = at_gate(&pending, &[], &rulings());
    assert!(mentions(&found, &["M-T-02", "pending"]), "{found:#?}");
}

#[test]
fn the_report_counts_each_cover_kind_and_the_marks_per_issue() {
    let cells = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([
                mark(7001, json!({})),
                mark(7002, json!({"dialect": "codex"}))
            ])
        )]),
    )];
    let repo = repo(cells, RowTable::default(), json!([]));
    let mut e2e = entry("M-T-03", "claude", "linux", "pending: 18");
    e2e["kind"] = json!("e2e");
    let covered = list(json!({
        "rows": [
            entry("M-T-01", "claude", "linux", "scenario: cells/T.json#M-T-01"),
            entry("M-T-01", "claude", "macos", "scenario: cells/T.json#M-T-01"),
            entry("M-T-02", "claude", "linux", "pending: 11"),
            e2e,
        ],
        "fixtures": [{"row": "M-T-F1", "os": "linux", "reason": "race", "cover": "pending: 19"}],
    }));
    let text = report(&covered, &repo);
    assert!(text.contains("| scenario | 2 |"), "{text}");
    assert!(text.contains("| pending | 2 | 1 |"), "{text}");
    assert!(text.contains("#7001: 1"), "{text}");
    assert!(text.contains("#7002: 1"), "{text}");
}

// ---- run results -------------------------------------------------------------------------------

#[test]
fn a_covered_cell_that_did_not_run_in_its_required_leg_fails_the_gate() {
    let cells = vec![batch(
        "cells/T.json",
        "full",
        json!([marked(
            "M-T-01",
            json!([mark(7001, json!({"dialect": "codex", "leg": "fake"}))])
        )]),
    )];
    let table = rows(json!([table_entry("M-T-01", "codex", "linux")]));
    let repo = repo(cells, table, json!([]));
    let covered =
        list(json!({"rows": [entry("M-T-01", "codex", "linux", "scenario: cells/T.json#M-T-01")]}));
    let ran = [scaled(ReportLeg::Fake, Os::Linux)];
    let judge =
        |records: &[LegRecord]| judge_runs(&covered, &repo, Trigger::PullRequest, &ran, records);
    let cell = |verdict| {
        record(
            EntryKind::Cell,
            "M-T-01",
            Some("codex"),
            Some(Protection::Full),
            Os::Linux,
            ReportLeg::Fake,
            verdict,
        )
    };

    let found = judge(&[]);
    assert!(
        mentions(&found, &["M-T-01", "codex", "linux", "fake", "did not run"]),
        "{found:#?}"
    );
    let found = judge(&[cell(ReportVerdict::NotRun)]);
    assert!(mentions(&found, &["M-T-01", "filtered out"]), "{found:#?}");
    let found = judge(&[cell(ReportVerdict::Fail)]);
    assert!(mentions(&found, &["M-T-01", "fail"]), "{found:#?}");
    assert_eq!(judge(&[cell(ReportVerdict::Pass)]), Vec::<String>::new());
    // Its mark pins a fake-leg failure for Codex, so that expected_fail is judged as pinned.
    assert_eq!(
        judge(&[cell(ReportVerdict::ExpectedFail)]),
        Vec::<String>::new()
    );

    // A leg that did not run in this run owes nothing.
    let none = judge_runs(
        &covered,
        &repo,
        Trigger::PullRequest,
        &[scaled(ReportLeg::Fake, Os::Macos)],
        &[],
    );
    assert_eq!(none, Vec::<String>::new());

    // An expected_fail no mark pins is a failure.
    let unpinned =
        list(json!({"rows": [entry("M-T-01", "codex", "linux", "scenario: cells/T.json#M-T-01")]}));
    let bare = repo_with(
        vec![batch("cells/T.json", "full", json!([cell_value("M-T-01")]))],
        &repo,
    );
    let found = judge_runs(
        &unpinned,
        &bare,
        Trigger::PullRequest,
        &ran,
        &[cell(ReportVerdict::ExpectedFail)],
    );
    assert!(
        mentions(&found, &["M-T-01", "expected_fail", "no mark"]),
        "{found:#?}"
    );
}

fn cell_value(id: &str) -> Value {
    cell(id)
}

#[test]
fn a_macos_claude_cell_outside_ticket_10s_leg_is_not_coverage() {
    let cells = vec![batch("cells/T.json", "full", json!([cell("M-T-01")]))];
    let table = rows(json!([table_entry("M-T-01", "claude", "macos")]));
    let repo = repo(cells, table, json!([]));
    let covered = list(
        json!({"rows": [entry("M-T-01", "claude", "macos", "scenario: cells/T.json#M-T-01")]}),
    );
    let ran = [
        scaled(ReportLeg::Fake, Os::Macos),
        scaled(ReportLeg::RealWrap, Os::Macos),
        scaled(ReportLeg::ClaudeSandbox, Os::Macos),
    ];
    let on = |leg| {
        record(
            EntryKind::Cell,
            "M-T-01",
            Some("claude"),
            Some(Protection::Full),
            Os::Macos,
            leg,
            ReportVerdict::Pass,
        )
    };
    let found = judge_runs(
        &covered,
        &repo,
        Trigger::MergeQueue,
        &ran,
        &[on(ReportLeg::Fake), on(ReportLeg::RealWrap)],
    );
    assert!(
        mentions(&found, &["M-T-01", "claude", "macos", "claude_sandbox"]),
        "{found:#?}"
    );
    let found = judge_runs(
        &covered,
        &repo,
        Trigger::MergeQueue,
        &ran,
        &[on(ReportLeg::ClaudeSandbox)],
    );
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_run_that_left_out_a_leg_its_cover_requires_owes_that_leg() {
    let cells = vec![batch("cells/T.json", "full", json!([cell("M-T-01")]))];
    let table = rows(json!([table_entry("M-T-01", "codex", "linux")]));
    let repo = repo(cells, table, json!([]));
    let mut both = entry("M-T-01", "codex", "linux", "scenario: cells/T.json#M-T-01");
    both["binary"] = json!("both");
    let covered = list(json!({"rows": [both]}));
    let fake = record(
        EntryKind::Cell,
        "M-T-01",
        Some("codex"),
        Some(Protection::Full),
        Os::Linux,
        ReportLeg::Fake,
        ReportVerdict::Pass,
    );
    // The run ran the scenario pass on Linux without ticket 53's leg: it still owes it.
    let found = judge_runs(
        &covered,
        &repo,
        Trigger::PullRequest,
        &[scaled(ReportLeg::Fake, Os::Linux)],
        std::slice::from_ref(&fake),
    );
    assert!(
        mentions(
            &found,
            &["M-T-01", "codex", "linux", "real_wrap", "did not run"]
        ),
        "{found:#?}"
    );
    let mut real = fake.clone();
    real.leg = ReportLeg::RealWrap;
    assert_eq!(
        judge_runs(
            &covered,
            &repo,
            Trigger::PullRequest,
            &[scaled(ReportLeg::Fake, Os::Linux)],
            &[fake, real],
        ),
        Vec::<String>::new()
    );
}

#[test]
fn an_escape_the_table_runs_on_the_real_binary_owes_ticket_53_s_leg() {
    let escape = json!({"id": "E-T-03", "escape": true, "dialects": ["codex"], "steps": [step()]});
    let cells = vec![batch(
        "cells/E.json",
        "full",
        json!([escape, cell("E-T-03/control")]),
    )];
    let mut row = escape_row("E-T-03", "codex", "linux", "yes");
    row["Basic"] = json!("n/a: Full only");
    let repo = repo(cells, RowTable::default(), json!([row]));
    let covered = list(json!({"escapes": [{
        "id": "E-T-03",
        "covers": {"full": "scenario: cells/E.json#E-T-03"},
        "control": "scenario: cells/E.json#E-T-03/control",
    }]}));
    let on = |kind, row: &str, leg| {
        record(
            kind,
            row,
            Some("codex"),
            Some(Protection::Full),
            Os::Linux,
            leg,
            ReportVerdict::Pass,
        )
    };
    let ran = [
        scaled(ReportLeg::Fake, Os::Linux),
        scaled(ReportLeg::RealWrap, Os::Linux),
    ];
    let fake_only = [
        on(EntryKind::Escape, "E-T-03", ReportLeg::Fake),
        on(EntryKind::Cell, "E-T-03/control", ReportLeg::Fake),
    ];
    let found = judge_runs(&covered, &repo, Trigger::PullRequest, &ran, &fake_only);
    assert!(
        mentions(
            &found,
            &["E-T-03", "codex", "linux", "real_wrap", "did not run"]
        ),
        "{found:#?}"
    );
    let mut both = fake_only.to_vec();
    both.push(on(EntryKind::Escape, "E-T-03", ReportLeg::RealWrap));
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::PullRequest, &ran, &both),
        Vec::<String>::new()
    );
}

#[test]
fn a_job_that_ran_one_leg_owes_that_leg_alone() {
    let escape = json!({"id": "E-T-03", "escape": true, "dialects": ["codex"], "steps": [step()]});
    let cells = vec![batch(
        "cells/E.json",
        "full",
        json!([escape, cell("E-T-03/control")]),
    )];
    let mut row = escape_row("E-T-03", "codex", "linux", "yes");
    row["Basic"] = json!("n/a: Full only");
    let repo = repo(cells, RowTable::default(), json!([row]));
    let covered = list(json!({"escapes": [{
        "id": "E-T-03",
        "covers": {"full": "scenario: cells/E.json#E-T-03"},
        "control": "scenario: cells/E.json#E-T-03/control",
    }]}));
    let on = |kind, row: &str, leg| {
        record(
            kind,
            row,
            Some("codex"),
            Some(Protection::Full),
            Os::Linux,
            leg,
            ReportVerdict::Pass,
        )
    };
    let fake_leg = [
        on(EntryKind::Escape, "E-T-03", ReportLeg::Fake),
        on(EntryKind::Cell, "E-T-03/control", ReportLeg::Fake),
    ];
    let fake = [scaled(ReportLeg::Fake, Os::Linux)];
    // The scenario job ran the fake leg: the whole pass judges it against ticket 53's leg too,
    // and the leg's own judge owes the fake leg's entries alone.
    let wide = judge_runs(&covered, &repo, Trigger::Nightly, &fake, &fake_leg);
    assert!(mentions(&wide, &["E-T-03", "real_wrap"]), "{wide:#?}");
    assert_eq!(
        judge_ran_legs(&covered, &repo, Trigger::Nightly, &fake, &fake_leg),
        Vec::<String>::new()
    );
    // And a leg that ran owes every entry it is required for: no record, no pass.
    let found = judge_ran_legs(&covered, &repo, Trigger::Nightly, &fake, &[]);
    assert!(
        mentions(&found, &["E-T-03", "fake", "did not run"]),
        "{found:#?}"
    );
    let real = [scaled(ReportLeg::RealWrap, Os::Linux)];
    let found = judge_ran_legs(&covered, &repo, Trigger::Nightly, &real, &fake_leg);
    assert!(mentions(&found, &["E-T-03", "real_wrap"]), "{found:#?}");
}

/// An escape table row for `id`, refused at Full and Basic and n/a with no sandbox.
fn escape_row(id: &str, dialects: &str, os: &str, real_binary: &str) -> Value {
    json!({
        "id": id, "ticket": "20", "Full": "refused", "Basic": "refused", "off": "n/a: no wrap",
        "dialects": dialects, "os": os, "real_binary": real_binary,
    })
}

#[test]
fn an_escape_cell_that_did_not_run_under_a_protection_fails_the_gate() {
    let escape =
        |id: &str| json!({"id": id, "escape": true, "dialects": ["codex"], "steps": [step()]});
    let cells = vec![
        batch(
            "cells/E-full.json",
            "full",
            json!([escape("E-T-01"), cell("E-T-01/control")]),
        ),
        batch("cells/E-basic.json", "basic", json!([escape("E-T-01")])),
    ];
    let repo = repo(
        cells,
        RowTable::default(),
        json!([escape_row("E-T-01", "codex", "linux", "no")]),
    );
    let covered = list(json!({"escapes": [{
        "id": "E-T-01",
        "covers": {"full": "scenario: cells/E-full.json#E-T-01", "basic": "scenario: cells/E-basic.json#E-T-01"},
        "control": "scenario: cells/E-full.json#E-T-01/control",
    }]}));
    assert_eq!(check(&covered, &repo), Vec::<String>::new());

    let ran = [scaled(ReportLeg::Fake, Os::Linux)];
    let escape_at = |p| {
        record(
            EntryKind::Escape,
            "E-T-01",
            Some("codex"),
            Some(p),
            Os::Linux,
            ReportLeg::Fake,
            ReportVerdict::Pass,
        )
    };
    let control = record(
        EntryKind::Cell,
        "E-T-01/control",
        Some("codex"),
        Some(Protection::Full),
        Os::Linux,
        ReportLeg::Fake,
        ReportVerdict::Pass,
    );
    let found = judge_runs(
        &covered,
        &repo,
        Trigger::PullRequest,
        &ran,
        &[escape_at(Protection::Full), control.clone()],
    );
    assert!(
        mentions(&found, &["E-T-01", "basic", "did not run"]),
        "{found:#?}"
    );
    assert!(!mentions(&found, &["E-T-01", "full"]), "{found:#?}");

    let found = judge_runs(
        &covered,
        &repo,
        Trigger::PullRequest,
        &ran,
        &[escape_at(Protection::Full), escape_at(Protection::Basic)],
    );
    assert!(
        mentions(&found, &["E-T-01/control", "did not run"]),
        "{found:#?}"
    );

    let all = [
        escape_at(Protection::Full),
        escape_at(Protection::Basic),
        control,
    ];
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::PullRequest, &ran, &all),
        Vec::<String>::new()
    );

    // The list must cover every protection the table expects anything at.
    let short = list(json!({"escapes": [{
        "id": "E-T-01",
        "covers": {"full": "scenario: cells/E-full.json#E-T-01"},
        "control": "scenario: cells/E-full.json#E-T-01/control",
    }]}));
    let found = check(&short, &repo);
    assert!(
        mentions(&found, &["E-T-01", "basic", "no cover"]),
        "{found:#?}"
    );
}

#[test]
fn a_macos_claude_escape_outside_ticket_10s_leg_is_not_coverage() {
    let escape = json!({"id": "E-T-02", "escape": true, "dialects": ["claude"], "steps": [step()]});
    let cells = vec![batch(
        "cells/E.json",
        "full",
        json!([escape, cell("E-T-02/control")]),
    )];
    let mut row = escape_row("E-T-02", "claude", "macos", "yes");
    row["Basic"] = json!("n/a: Full only");
    let repo = repo(cells, RowTable::default(), json!([row]));
    let covered = list(json!({"escapes": [{
        "id": "E-T-02",
        "covers": {"full": "scenario: cells/E.json#E-T-02"},
        "control": "scenario: cells/E.json#E-T-02/control",
    }]}));
    let ran = [
        scaled(ReportLeg::Fake, Os::Macos),
        scaled(ReportLeg::RealWrap, Os::Macos),
        scaled(ReportLeg::ClaudeSandbox, Os::Macos),
    ];
    let on = |kind, row: &str, leg| {
        record(
            kind,
            row,
            Some("claude"),
            Some(Protection::Full),
            Os::Macos,
            leg,
            ReportVerdict::Pass,
        )
    };
    let elsewhere = [
        on(EntryKind::Escape, "E-T-02", ReportLeg::Fake),
        on(EntryKind::Escape, "E-T-02", ReportLeg::RealWrap),
        on(EntryKind::Cell, "E-T-02/control", ReportLeg::ClaudeSandbox),
    ];
    let found = judge_runs(&covered, &repo, Trigger::MergeQueue, &ran, &elsewhere);
    assert!(
        mentions(&found, &["E-T-02", "claude", "macos", "claude_sandbox"]),
        "{found:#?}"
    );
    let in_ten = [
        on(EntryKind::Escape, "E-T-02", ReportLeg::ClaudeSandbox),
        on(EntryKind::Cell, "E-T-02/control", ReportLeg::ClaudeSandbox),
    ];
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::MergeQueue, &ran, &in_ten),
        Vec::<String>::new()
    );
}

#[test]
fn a_fixture_cover_that_did_not_run_on_its_named_os_fails_the_gate() {
    let test = "every_matrix_row_names_a_cover_that_exists";
    let covered = list(json!({
        "fixtures": [{"row": "M-T-F1", "os": "macos", "reason": "race",
                      "cover": format!("fixture: tests/scenarios/tests/coverage.rs::{test}")}],
    }));
    let repo = repo_rows_only();
    let ran = [
        scaled(ReportLeg::Fixture, Os::Linux),
        scaled(ReportLeg::Fixture, Os::Macos),
    ];
    let on = |os| LegRecord {
        kind: EntryKind::Fixture,
        row: "M-T-F1".into(),
        test: Some(test.into()),
        dialect: None,
        protection: None,
        os,
        leg: ReportLeg::Fixture,
        timings: None,
        verdict: ReportVerdict::Pass,
        reason: None,
    };
    let found = judge_runs(&covered, &repo, Trigger::MergeQueue, &ran, &[on(Os::Linux)]);
    assert!(
        mentions(&found, &["M-T-F1", test, "macos", "did not run"]),
        "{found:#?}"
    );
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::MergeQueue, &ran, &[on(Os::Macos)]),
        Vec::<String>::new()
    );
}

#[test]
fn a_nightly_only_entry_skipped_in_the_merge_queue_is_not_a_failure() {
    let nightly = json!({"id": "M-T-25", "production_nightly_only": "Q300", "steps": [step()]});
    let cells = vec![batch("cells/T.json", "full", json!([nightly]))];
    let table = rows(json!([table_entry("M-T-25", "claude", "linux")]));
    let repo = repo(cells, table, json!([]));
    let covered = list(
        json!({"rows": [entry("M-T-25", "claude", "linux", "scenario: cells/T.json#M-T-25")]}),
    );
    let ran = [
        scaled(ReportLeg::Fake, Os::Linux),
        Ran {
            leg: ReportLeg::Fake,
            os: Os::Linux,
            timings: TimingsMode::Production,
        },
    ];
    let at = |timings, verdict| {
        let mut r = record(
            EntryKind::Cell,
            "M-T-25",
            Some("claude"),
            Some(Protection::Full),
            Os::Linux,
            ReportLeg::Fake,
            verdict,
        );
        r.timings = Some(timings);
        if verdict == ReportVerdict::NotRun {
            r.reason = Some("skipped-for-timing".into());
        }
        r
    };
    let records = [
        at(TimingsMode::Scaled, ReportVerdict::Pass),
        at(TimingsMode::Production, ReportVerdict::NotRun),
    ];
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::MergeQueue, &ran, &records),
        Vec::<String>::new()
    );
    assert_eq!(
        judge_runs(&covered, &repo, Trigger::Label, &ran, &records),
        Vec::<String>::new()
    );
    let found = judge_runs(&covered, &repo, Trigger::Nightly, &ran, &records);
    assert!(
        mentions(&found, &["M-T-25", "production", "skipped-for-timing"]),
        "{found:#?}"
    );
}
