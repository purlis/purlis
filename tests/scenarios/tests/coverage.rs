// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The matrix coverage gate** (T1.1-08, #6855): the repo's list over the real matrix, cells
//! and escape table, and each of the gate's rules against small lists written here.
//!
//! A change's own marks are judged against its base (`SCENARIOS_BASE`, a commit, or else the
//! merge base with `origin/phase-1-proven`): that test needs the base's history, so it is run
//! by `sandbox-scenarios-linux` (`--run-ignored all`), which sets it. The issues a pull request
//! closes come from `SCENARIOS_PR_CLOSES` (`7001,7002`), which the same job sets.

use std::process::Command;

use purlis_scenarios::coverage::{
    Coverage, Issues, Ran, Repo, at_gate, check, issues, judge_runs, ratchet, report, row_table,
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
        catalogue: catalogue(),
    }
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
    let out = Command::new("git")
        .args(args)
        .current_dir(root())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The commit this change's marks are judged against.
fn base() -> String {
    if let Ok(base) = std::env::var("SCENARIOS_BASE") {
        return base;
    }
    git(&["merge-base", "HEAD", "origin/phase-1-proven"])
        .expect("no base to judge the marks against: set SCENARIOS_BASE to a commit")
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
fn every_cell_carries_each_expectation_kind_its_row_states() {
    let repo = Repo::read().expect("the repo's cells");
    let list = Coverage::read().expect("coverage.json");
    assert!(
        !row_table(&list).entries.is_empty(),
        "the list carries no Log/Window/Chat flags"
    );
    // Every cell a scenario cover names is held to its row's flags; a cell no cover names yet
    // (B0 before ticket 11 gives it the Window and Chat expectations its rows state) is not
    // coverage, and its rows stay pending.
    let mut covering = list.clone();
    covering.rows.retain(|r| r.cover.starts_with("scenario: "));
    let table = row_table(&covering);
    let validator = Validator::repo().expect("the validator").with_rows(table);
    for (file, batch) in &repo.batches {
        let found = validator.check(batch);
        assert!(found.is_empty(), "{file}: {found:#?}");
    }
}

#[test]
#[ignore = "needs the change's base: sandbox-scenarios-linux runs it with SCENARIOS_BASE"]
fn the_marks_keep_their_ratchet_against_the_base() {
    let repo = Repo::read().expect("the repo's cells");
    let base = base();
    let found = ratchet(&cells_at(&base), &repo.batches);
    assert!(found.is_empty(), "against {base}: {found:#?}");
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
    let found = at_gate(&list, &repo.batches);
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
        json!({"caps": {"e2e": {"window": {"seed": 1, "cap": 3, "approved": "yes"}}}, "rows": [e2e]}),
    );
    let found = check(&not_a_ruling, &repo);
    assert!(mentions(&found, &["window", "Q-id"]), "{found:#?}");
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

    moved["moved"] = json!("Q331");
    let found = check(&list(json!({"caps": caps, "rows": [moved]})), &repo);
    assert_eq!(found, Vec::<String>::new());
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

#[test]
fn a_mark_on_an_existing_cell_without_a_new_issue_fails_the_gate() {
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
        "issues": {"7001": "open", "7002": "closed"}
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
    let found = at_gate(&covered(json!([])), &head);
    assert!(mentions(&found, &["M-T-01", "7001"]), "{found:#?}");
    assert!(mentions(&found, &["M-T-01", "7002"]), "{found:#?}");

    let one = covered(json!([{"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q340"}]));
    let found = at_gate(&one, &head);
    assert!(!mentions(&found, &["7001"]), "{found:#?}");
    assert!(mentions(&found, &["M-T-01", "7002"]), "{found:#?}");

    let both = covered(json!([
        {"issue": 7001, "cell": "cells/T.json#M-T-01", "approved": "Q340"},
        {"issue": 7002, "cell": "cells/T.json#M-T-01", "approved": "Q340"},
    ]));
    assert_eq!(at_gate(&both, &head), Vec::<String>::new());

    // A pending cover is no cover at the gate.
    let pending = list(json!({"rows": [entry("M-T-02", "codex", "linux", "pending: 11")]}));
    let found = at_gate(&pending, &[]);
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
