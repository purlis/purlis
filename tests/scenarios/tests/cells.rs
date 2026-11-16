// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The cells format's validator** (T1.1-05, #6852): every cells file in the repo through its
//! JSON Schema and the checks a schema cannot make, and the rules for marks, legs and timings
//! against small cells written here.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use purlis_scenarios::judge::{Assertion, GlobalFailure, Observed, Run, Verdict, judge};
use purlis_scenarios::kinds::{Kinds, WRITTEN_AS_TEXT};
use purlis_scenarios::model::{
    Batch, Cell, Dialect, EntryKind, Leg, LegRecord, Os, PinsFile, Protection, ReportLeg,
    ReportVerdict, TimingsMode,
};
use purlis_scenarios::plan::{Scheduled, Trigger, production_set, profile, schedule};
use purlis_scenarios::root;
use purlis_scenarios::timings::catalogue;
use purlis_scenarios::validate::{Pins, RowTable, Validator};
use serde_json::{Value, json};

// ---- helpers ---------------------------------------------------------------------------------

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The schema of `file` in `schema/`, narrowed to its `$defs` entry `def` when given.
fn schema_errors(file: &str, def: Option<&str>, instance: &Value) -> Vec<String> {
    let mut schema = read_json(&root().join("schema").join(file));
    if let Some(def) = def {
        schema = json!({
            "$schema": schema["$schema"],
            "$defs": schema["$defs"],
            "$ref": format!("#/$defs/{def}"),
        });
    }
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
    validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect()
}

fn validator() -> Validator {
    Validator::repo().expect("the repo's pins read")
}

/// A batch around `cells`, after the schema has passed it.
fn batch(cells: Value) -> Batch {
    let value = json!({
        "batch": "T0",
        "protection": "full",
        "persona": "steward",
        "hosts": {"h1": "one.test"},
        "cells": cells,
    });
    let errors = schema_errors("cells.schema.json", None, &value);
    assert!(
        errors.is_empty(),
        "the test's own batch fails the schema: {errors:#?}"
    );
    Batch::from_value(value).expect("a batch")
}

fn cell(value: Value) -> Cell {
    batch(json!([value])).cells.remove(0)
}

fn problems(cells: Value) -> Vec<String> {
    validator().check(&batch(cells))
}

fn run(dialect: Dialect, leg: Leg, timings: TimingsMode) -> Run {
    let factor = if timings == TimingsMode::Scaled {
        0.5
    } else {
        1.0
    };
    Run {
        dialect,
        os: Os::Linux,
        leg,
        timings,
        seconds: catalogue().seconds_at(factor),
    }
}

fn fake() -> Run {
    run(Dialect::Claude, Leg::Fake, TimingsMode::Scaled)
}

fn fails(step: u32, assertion: &str, observed: Value) -> Assertion {
    Assertion {
        step,
        assertion: assertion.into(),
        observed: Observed::Fail(observed),
    }
}

fn passes(step: u32, assertion: &str) -> Assertion {
    Assertion {
        step,
        assertion: assertion.into(),
        observed: Observed::Pass,
    }
}

fn red(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Fail { .. })
}

/// A held GET with one sandbox ask and a block: a step any cell can start from.
fn held_get() -> Value {
    json!({
        "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
        "require": [
            {"kind": "ask.raised", "ask": "sandbox", "count": 1},
            {"kind": "stop.block", "source": "sandbox", "target.host": "$h1"}
        ],
        "forbid": [{"kind": "ask.answered"}]
    })
}

// ---- every file ------------------------------------------------------------------------------

#[test]
fn every_cells_file_validates() {
    let dir = root().join("cells");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .expect("tests/scenarios/cells exists")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no cells file in {}", dir.display());
    let check = validator();
    for path in files {
        let value = read_json(&path);
        let errors = schema_errors("cells.schema.json", None, &value);
        assert!(
            errors.is_empty(),
            "{} fails the schema: {errors:#?}",
            path.display()
        );
        let parsed = Batch::from_value(value).unwrap_or_else(|e| {
            panic!("{} passed the schema, not the reader: {e}", path.display())
        });
        let found = check.check(&parsed);
        assert!(found.is_empty(), "{}: {found:#?}", path.display());
    }
    let pins = root().join("pins");
    if pins.is_dir() {
        for entry in fs::read_dir(&pins).expect("pins reads") {
            let path = entry.expect("an entry").path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let errors = schema_errors("cells.schema.json", Some("pins_file"), &read_json(&path));
            assert!(
                errors.is_empty(),
                "{} fails the schema: {errors:#?}",
                path.display()
            );
        }
    }
}

#[test]
fn every_kind_written_as_text_is_still_written_by_purlis_core() {
    let src = root().join("../../crates/purlis-core/src");
    let mut seen = BTreeSet::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("purlis-core's source reads") {
            let path = entry.expect("an entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if name != "tests" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") && !name.contains("tests") {
                let text = fs::read_to_string(&path).expect("a source file reads");
                for kind in WRITTEN_AS_TEXT {
                    if text.contains(&format!("\"{kind}\"")) {
                        seen.insert(kind);
                    }
                }
            }
        }
    }
    let missing: Vec<_> = WRITTEN_AS_TEXT
        .iter()
        .filter(|k| !seen.contains(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "no longer written by purlis-core: {missing:?}"
    );
}

// ---- event names -----------------------------------------------------------------------------

#[test]
fn a_forbid_on_an_unknown_event_name_fails_validation() {
    let mut step = held_get();
    step["forbid"] = json!([{"kind": "ask.anwsered"}]);
    let found = problems(json!([{"id": "M-T-01", "steps": [step]}]));
    assert!(
        found
            .iter()
            .any(|p| p.contains("ask.anwsered") && p.contains("M-T-01")),
        "{found:#?}"
    );
    assert!(problems(json!([{"id": "M-T-01", "steps": [held_get()]}])).is_empty());
    assert!(Kinds::from_core().knows("hook.posttoolusefailure-blocked"));
    assert!(!Kinds::from_core().knows("hook.posttoolusefailure"));
}

#[test]
fn a_forbid_step_without_a_proof_of_run_fails_validation() {
    let step = json!({
        "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
        "require": [{"kind": "ask.answered", "answer": "allowed"}],
        "forbid": [{"kind": "stop.block"}]
    });
    let found = problems(json!([{"id": "M-T-02", "steps": [step]}]));
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-02") && p.contains("proves")),
        "{found:#?}"
    );
    let mut proven = step.clone();
    proven["require"] = json!([
        {"kind": "hook.pretooluse", "tool": "Bash"},
        {"kind": "ask.answered", "answer": "allowed"}
    ]);
    assert_eq!(
        problems(json!([{"id": "M-T-02", "steps": [proven]}])),
        Vec::<String>::new()
    );
}

#[test]
fn a_cell_naming_an_event_codex_lacks_must_say_what_codex_shows() {
    // Codex has no `PostToolUseFailure` (`plugin::CODEX`).
    let step = json!({
        "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
        "require": [{"kind": "hook.posttoolusefailure-blocked"}]
    });
    let bare = json!({"id": "M-T-03", "dialects": ["claude", "codex"], "steps": [step]});
    // Without the repo's pins: the rule itself.
    let unpinned = |cells: Value| validator().with_pins(Pins::default()).check(&batch(cells));
    let found = unpinned(json!([bare.clone()]));
    assert!(
        found
            .iter()
            .any(|p| p.contains("codex") && p.contains("hook.posttoolusefailure-blocked")),
        "{found:#?}"
    );
    assert!(
        problems(json!([{"id": "M-T-03", "dialects": ["claude"], "steps": [step]}])).is_empty()
    );
    // The repo's Codex pins file (ticket 3, drawn from Codex 0.169.0's recording) says it for
    // every cell.
    assert_eq!(problems(json!([bare.clone()])), Vec::<String>::new());

    // The cell says it, for one step...
    let mut per_step = bare.clone();
    per_step["per_dialect"] = json!({"codex": {"steps": {"1": {
        "require": [{"kind": "hook.posttooluse-blocked"}]
    }}}});
    assert_eq!(problems(json!([per_step])), Vec::<String>::new());

    // ... or for every step...
    let mut shows = bare.clone();
    shows["per_dialect"] = json!({"codex": {"shows": {
        "hook.posttoolusefailure-blocked": [{"kind": "hook.posttooluse-blocked"}]
    }}});
    assert_eq!(problems(json!([shows])), Vec::<String>::new());

    // ... or Codex's pins file (ticket 3) says it for every cell.
    let pins: PinsFile = serde_json::from_value(json!({
        "dialect": "codex",
        "shows": {"hook.posttoolusefailure-blocked": [{"kind": "hook.posttooluse-blocked"}]}
    }))
    .expect("a pins file");
    let pinned = validator().with_pins(Pins::from_files(vec![pins]));
    assert_eq!(pinned.check(&batch(json!([bare]))), Vec::<String>::new());

    // What Codex shows must itself be something Codex produces.
    let mut wrong = bare.clone();
    wrong["per_dialect"] = json!({"codex": {"shows": {
        "hook.posttoolusefailure-blocked": [{"kind": "hook.permissiondenied"}]
    }}});
    assert!(!problems(json!([wrong])).is_empty());
}

// ---- marks -----------------------------------------------------------------------------------

#[test]
fn an_expected_fail_pins_its_step_assertion_and_observed_value() {
    let marked = cell(json!({
        "id": "M-T-04",
        "steps": [held_get()],
        "expected_fail": [
            {"issue": 6674, "step": 1, "assertion": "require[0]", "observed": 2}
        ]
    }));
    let pinned = judge(&marked, &fake(), &[fails(1, "require[0]", json!(2))], &[]);
    assert_eq!(
        pinned,
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6674])
        }
    );

    let other_value = judge(&marked, &fake(), &[fails(1, "require[0]", json!(3))], &[]);
    assert!(
        red(&other_value),
        "another observed value is red: {other_value:?}"
    );

    let other_assertion = judge(
        &marked,
        &fake(),
        &[
            fails(1, "require[0]", json!(2)),
            fails(1, "require[1]", json!(0)),
        ],
        &[],
    );
    assert!(
        red(&other_assertion),
        "another failure in the cell is red: {other_assertion:?}"
    );

    let global = judge(
        &marked,
        &fake(),
        &[fails(1, "require[0]", json!(2))],
        &[GlobalFailure {
            check: "refusal_black_hole".into(),
            refusal: "step 1".into(),
        }],
    );
    assert!(red(&global), "a global check still runs: {global:?}");

    // A mark names an assertion the step has.
    let found = problems(json!([{
        "id": "M-T-04",
        "steps": [held_get()],
        "expected_fail": [{"issue": 6674, "step": 1, "assertion": "require[5]", "observed": 2}]
    }]));
    assert!(found.iter().any(|p| p.contains("require[5]")), "{found:#?}");

    // A `within` mark's observed value is a range in Timings units.
    let mut timed = held_get();
    timed["require"][0]["within"] = json!("group+1");
    let within = cell(json!({
        "id": "M-T-04",
        "steps": [timed],
        "expected_fail": [{"issue": 6672, "step": 1, "assertion": "require[0]",
                           "observed": {"from": "hold-20", "to": "hold"}}]
    }));
    let inside = judge(
        &within,
        &fake(),
        &[fails(1, "require[0]", json!(20.0))],
        &[],
    );
    assert_eq!(
        inside,
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6672])
        }
    );
    let outside = judge(
        &within,
        &fake(),
        &[fails(1, "require[0]", json!(45.0))],
        &[],
    );
    assert!(red(&outside), "outside the range is red: {outside:?}");
}

#[test]
fn a_pass_under_expected_fail_is_red() {
    let marked = cell(json!({
        "id": "M-T-05",
        "steps": [held_get()],
        "expected_fail": [
            {"issue": 6674, "step": 1, "assertion": "require[0]", "observed": 2},
            {"issue": 6675, "check": "refusal_black_hole", "refusal": "step 1"}
        ]
    }));
    let all_pass = [
        passes(1, "require[0]"),
        passes(1, "require[1]"),
        passes(1, "forbid[0]"),
    ];
    let globals = [GlobalFailure {
        check: "refusal_black_hole".into(),
        refusal: "step 1".into(),
    }];

    let step_mark_passes = judge(&marked, &fake(), &all_pass, &globals);
    match &step_mark_passes {
        Verdict::Fail { reasons } => {
            assert!(reasons.iter().any(|r| r.contains("6674")), "{reasons:#?}")
        }
        other => panic!("a pass under expected_fail is red, got {other:?}"),
    }

    let both_fail = judge(
        &marked,
        &fake(),
        &[fails(1, "require[0]", json!(2))],
        &globals,
    );
    assert_eq!(
        both_fail,
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6674, 6675])
        }
    );

    let global_mark_passes = judge(&marked, &fake(), &[fails(1, "require[0]", json!(2))], &[]);
    match &global_mark_passes {
        Verdict::Fail { reasons } => {
            assert!(reasons.iter().any(|r| r.contains("6675")), "{reasons:#?}")
        }
        other => panic!("a global check passing under its mark is red, got {other:?}"),
    }

    // A mark on a global check names the check and the refusal.
    let found = problems(json!([{
        "id": "M-T-05",
        "steps": [held_get()],
        "expected_fail": [{"issue": 6675, "check": "no_such_check", "refusal": "step 1"}]
    }]));
    assert!(
        found.iter().any(|p| p.contains("no_such_check")),
        "{found:#?}"
    );
}

#[test]
fn a_cell_with_two_marks_for_two_issues_stays_red_until_both_are_removed() {
    let mark_a = json!({"issue": 6674, "step": 1, "assertion": "require[0]", "observed": 2});
    let mark_b = json!({"issue": 6657, "step": 1, "assertion": "forbid[0]", "observed": 1});
    let with =
        |marks: Value| cell(json!({"id": "M-T-06", "steps": [held_get()], "expected_fail": marks}));
    let a_fails = fails(1, "require[0]", json!(2));
    let b_fails = fails(1, "forbid[0]", json!(1));
    let a_passes = passes(1, "require[0]");
    let b_passes = passes(1, "forbid[0]");

    let both = with(json!([mark_a, mark_b]));
    assert_eq!(
        judge(&both, &fake(), &[a_fails.clone(), b_fails.clone()], &[]),
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6657, 6674])
        }
    );
    // #6674 fixed: its mark is now a pass under expected_fail.
    assert!(red(&judge(
        &both,
        &fake(),
        &[a_passes.clone(), b_fails.clone()],
        &[]
    )));
    let only_b = with(json!([mark_b]));
    assert_eq!(
        judge(&only_b, &fake(), &[a_passes.clone(), b_fails.clone()], &[]),
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6657])
        }
    );
    // #6657 fixed too: red until its mark goes as well.
    assert!(red(&judge(
        &only_b,
        &fake(),
        &[a_passes.clone(), b_passes.clone()],
        &[]
    )));
    assert!(red(&judge(
        &both,
        &fake(),
        &[a_passes.clone(), b_passes.clone()],
        &[]
    )));
    let none = with(json!([]));
    assert_eq!(
        judge(&none, &fake(), &[a_passes, b_passes], &[]),
        Verdict::Pass
    );

    // One entry per failing assertion and issue: the same one twice is refused.
    let found = problems(json!([{"id": "M-T-06", "steps": [held_get()],
                                 "expected_fail": [mark_a, mark_a]}]));
    assert!(found.iter().any(|p| p.contains("twice")), "{found:#?}");
}

#[test]
fn a_mark_scoped_to_one_leg_and_timing_leaves_the_others_judged_normally() {
    let marked = cell(json!({
        "id": "M-T-07",
        "steps": [held_get()],
        "expected_fail": [{"issue": 6672, "step": 1, "assertion": "require[0]", "observed": 0,
                           "leg": "fake", "timings": "scaled"}]
    }));
    let failing = [fails(1, "require[0]", json!(0))];
    let passing = [passes(1, "require[0]")];

    let scoped = run(Dialect::Claude, Leg::Fake, TimingsMode::Scaled);
    assert_eq!(
        judge(&marked, &scoped, &failing, &[]),
        Verdict::ExpectedFail {
            issues: BTreeSet::from([6672])
        }
    );
    assert!(red(&judge(&marked, &scoped, &passing, &[])));

    for other in [
        run(Dialect::Claude, Leg::RealWrap, TimingsMode::Scaled),
        run(Dialect::Claude, Leg::Fake, TimingsMode::Production),
        run(Dialect::Claude, Leg::ClaudeSandbox, TimingsMode::Production),
    ] {
        assert!(
            red(&judge(&marked, &other, &failing, &[])),
            "unmarked there, so red"
        );
        assert_eq!(
            judge(&marked, &other, &passing, &[]),
            Verdict::Pass,
            "and a pass is a pass"
        );
    }
}

// ---- the leg report --------------------------------------------------------------------------

#[test]
fn a_leg_report_record_validates_for_each_kind() {
    let record = |kind, test: Option<&str>, leg| LegRecord {
        kind,
        row: "M-NET-05".into(),
        test: test.map(str::to_owned),
        dialect: matches!(kind, EntryKind::Cell | EntryKind::Escape).then_some(Dialect::Codex),
        protection: matches!(kind, EntryKind::Cell | EntryKind::Escape).then_some(Protection::Full),
        os: Os::Linux,
        leg,
        timings: Some(TimingsMode::Scaled),
        verdict: ReportVerdict::Pass,
        reason: None,
    };
    let good = [
        record(EntryKind::Cell, None, ReportLeg::Fake),
        record(EntryKind::Escape, None, ReportLeg::RealWrap),
        record(
            EntryKind::Fixture,
            Some("sandbox::asks::tests::a_held_ask_blocks"),
            ReportLeg::Fixture,
        ),
        record(EntryKind::E2e, Some("specs/asks.e2e.ts"), ReportLeg::E2e),
    ];
    for r in &good {
        let value = serde_json::to_value(r).expect("a record serialises");
        let errors = schema_errors("leg-report.schema.json", None, &value);
        assert!(errors.is_empty(), "{value}: {errors:#?}");
    }

    let not_run = LegRecord {
        verdict: ReportVerdict::NotRun,
        reason: Some("skipped-for-timing".into()),
        ..good[0].clone()
    };
    let bad = [
        // A fixture and an e2e entry name their test.
        LegRecord {
            test: None,
            ..good[2].clone()
        },
        LegRecord {
            test: None,
            ..good[3].clone()
        },
        // A cell and an escape name their dialect and protection.
        LegRecord {
            dialect: None,
            ..good[0].clone()
        },
        LegRecord {
            protection: None,
            ..good[1].clone()
        },
        // Not run says why.
        LegRecord {
            reason: None,
            ..not_run.clone()
        },
    ];
    let value = serde_json::to_value(&not_run).expect("a record serialises");
    assert!(schema_errors("leg-report.schema.json", None, &value).is_empty());
    for r in &bad {
        let value = serde_json::to_value(r).expect("a record serialises");
        assert!(
            !schema_errors("leg-report.schema.json", None, &value).is_empty(),
            "accepted: {value}"
        );
    }
}

// ---- timings ---------------------------------------------------------------------------------

#[test]
fn the_production_timing_set_is_derived_from_waits_and_timeouts() {
    let call = json!({"tool": "Bash", "input": {"command": "get https://$h1 3"}});
    let mut timed_call = call.clone();
    timed_call["timeout"] = json!("150s");
    let step = |extra: Value| {
        let mut s = held_get();
        s["calls"] = json!([call.clone()]);
        for (k, v) in extra.as_object().expect("an object") {
            s[k] = v.clone();
        }
        s
    };
    let parsed = batch(json!([
        {"id": "M-T-10", "steps": [step(json!({"wait": "hold+1"}))]},
        {"id": "M-T-11", "steps": [step(json!({"wait": "group+1"}))]},
        {"id": "M-T-12", "steps": [step(json!({"calls": [timed_call]}))]},
        {"id": "M-T-13", "steps": [step(json!({"restart": "host_killed"}))]},
        {"id": "M-T-14", "steps": [step(json!({"press": {
            "label": "Keep blocked", "target": "notice", "when": "ask.raised",
            "delay": "throttle_window-1"}}))]},
        {"id": "M-T-15", "production_only": ["hold", "HOOK_TIMEOUT"], "steps": [step(json!({}))]},
        {"id": "M-T-16", "production_only": ["hold", "HOOK_TIMEOUT"],
         "steps": [step(json!({"wait": "hold+1"}))]},
        {"id": "M-T-17", "steps": [{
            "calls": [call.clone()],
            "require": [{"kind": "stop.block", "within": "supervisor_window"}]
        }]}
    ]));
    let cat = catalogue();
    let set = production_set(std::slice::from_ref(&parsed), cat.as_ref());
    assert_eq!(
        set,
        BTreeSet::from([
            "M-T-10", "M-T-12", "M-T-13", "M-T-14", "M-T-15", "M-T-16", "M-T-17"
        ])
    );

    let by_id = |id: &str| parsed.cells.iter().find(|c| c.id == id).expect("the cell");
    // Derived membership adds a production run and keeps the scaled one.
    let derived = profile(by_id("M-T-10"), cat.as_ref());
    assert!(derived.in_production_set() && !derived.skips_scaled());
    assert_eq!(
        schedule(
            by_id("M-T-10"),
            cat.as_ref(),
            TimingsMode::Scaled,
            Trigger::PullRequest
        ),
        Scheduled::Run
    );
    // Only a hand tag and a host kill skip the scaled run.
    for id in ["M-T-13", "M-T-15", "M-T-16"] {
        assert!(profile(by_id(id), cat.as_ref()).skips_scaled(), "{id}");
        assert_eq!(
            schedule(
                by_id(id),
                cat.as_ref(),
                TimingsMode::Scaled,
                Trigger::PullRequest
            ),
            Scheduled::SkippedForTiming,
            "{id}"
        );
    }
    // A hand tag adds to what is derived and never takes it away.
    assert!(!profile(by_id("M-T-16"), cat.as_ref()).derived_by.is_empty());
    // A cell outside the set is not in a production run.
    assert_eq!(
        schedule(
            by_id("M-T-11"),
            cat.as_ref(),
            TimingsMode::Production,
            Trigger::Nightly
        ),
        Scheduled::NotInThisRun
    );

    // A pair that is not a `production_only` pair, or a name `Timings` lacks, is refused.
    let found = problems(json!([
        {"id": "M-T-18", "production_only": ["hold", "group"], "steps": [step(json!({}))]},
        {"id": "M-T-19", "steps": [step(json!({"wait": "holdd+1"}))]}
    ]));
    assert!(found.iter().any(|p| p.contains("M-T-18")), "{found:#?}");
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-19") && p.contains("holdd")),
        "{found:#?}"
    );
}

#[test]
fn a_production_nightly_only_cell_is_skipped_by_the_label_and_merge_queue_legs() {
    let nightly = cell(json!({
        "id": "M-NET-25",
        "production_nightly_only": "Q300",
        "steps": [{
            "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
            "wait": "quiet_hold+1",
            "require": [{"kind": "stop.block", "source": "sandbox"}]
        }]
    }));
    let cat = catalogue();
    assert!(profile(&nightly, cat.as_ref()).in_production_set());
    assert!(
        !profile(&nightly, cat.as_ref()).skips_scaled(),
        "it runs scaled on PRs (Q300)"
    );
    for trigger in [Trigger::PullRequest, Trigger::MergeQueue] {
        assert_eq!(
            schedule(&nightly, cat.as_ref(), TimingsMode::Scaled, trigger),
            Scheduled::Run
        );
    }
    for trigger in [Trigger::Label, Trigger::MergeQueue] {
        assert_eq!(
            schedule(&nightly, cat.as_ref(), TimingsMode::Production, trigger),
            Scheduled::SkippedForTiming,
            "{trigger:?}"
        );
    }
    assert_eq!(
        schedule(
            &nightly,
            cat.as_ref(),
            TimingsMode::Production,
            Trigger::Nightly
        ),
        Scheduled::Run
    );
}

// ---- the row table ---------------------------------------------------------------------------

#[test]
fn a_cell_missing_a_kind_its_row_states_fails_validation() {
    let rows: RowTable =
        serde_json::from_value(read_json(&root().join("tests/fixtures/rows.json")))
            .expect("the fixture row table");
    let check = validator().with_rows(rows);

    // The fixture's M-T-20 states Log and Window for Claude on Linux and macOS.
    let log_only = batch(json!([{"id": "M-T-20", "dialects": ["claude"], "steps": [held_get()]}]));
    let found = check.check(&log_only);
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-20") && p.contains("window")),
        "{found:#?}"
    );

    let mut with_window = held_get();
    with_window["window"] = json!([{"query": "notice_shown"}]);
    let both = batch(json!([{"id": "M-T-20", "dialects": ["claude"], "steps": [with_window]}]));
    assert_eq!(check.check(&both), Vec::<String>::new());

    // A cell with no row (an escape, a control) gets only the "no expectation at all" rule.
    let no_row = batch(json!([{"id": "E-T-21", "steps": [held_get()]}]));
    assert_eq!(check.check(&no_row), Vec::<String>::new());
    let nothing = batch(json!([{"id": "E-T-22", "steps": [
        {"calls": [{"tool": "Bash", "input": {"command": "true"}}]}
    ]}]));
    let found = check.check(&nothing);
    assert!(
        found
            .iter()
            .any(|p| p.contains("E-T-22") && p.contains("no expectation")),
        "{found:#?}"
    );
}

// ---- the timings catalogue is ticket 1's ---------------------------------------------------

#[test]
fn the_catalogue_keeps_holds_tie_with_the_hook_timeout_production_only() {
    let cat = catalogue();
    assert!(cat.production_only_members().contains("hold"));
    assert!(cat.production_only_members().contains("HOOK_TIMEOUT"));
    let scaled = cat.seconds_at(0.5);
    assert_eq!(scaled["hold"], 30.0);
    assert_eq!(
        scaled["HOOK_TIMEOUT"], 60.0,
        "out of process, so never scaled"
    );
}

/// **The catalogue is purlis-core's `Timings`** (T1.1-01): every field at its production value,
/// every out-of-process duration at its real one and never scaled, no name core does not have,
/// and the `production_only` pairs core declares.
#[test]
fn the_catalogue_is_purlis_cores_timings() {
    use purlis_core::sandbox::timings::{FIELDS, OUT_OF_PROCESS, PAIRS, Timings, id_of};
    let cat = catalogue();
    let production = Timings::production();
    let seconds = cat.production_seconds();
    let unscaled = cat.unscaled();
    for one in &FIELDS {
        let want = production
            .duration(one.field)
            .map_or(production.throttle_per_chat() as f64, |at| at.as_secs_f64());
        assert_eq!(seconds.get(one.field), Some(&want), "{}", one.field);
    }
    for (id, _, at) in OUT_OF_PROCESS {
        assert_eq!(seconds.get(id), Some(&at.as_secs_f64()), "{id}");
        assert!(unscaled.contains(id), "{id} runs at real time");
    }
    assert_eq!(
        seconds.len(),
        FIELDS.len() + OUT_OF_PROCESS.len(),
        "no name core does not have: {seconds:?}"
    );
    let pairs: Vec<(&str, &str)> = PAIRS
        .iter()
        .filter(|pair| pair.production_only)
        .map(|pair| (id_of(pair.left), id_of(pair.right)))
        .collect();
    assert_eq!(cat.production_only_pairs(), pairs);
}

// ---- review of #6911 -------------------------------------------------------------------------

#[test]
fn an_escape_cell_carries_no_expected_fail() {
    let mark = json!({"issue": 6674, "step": 1, "assertion": "require[0]", "observed": 2});
    let found = problems(json!([{
        "id": "E-T-30", "escape": true, "steps": [held_get()], "expected_fail": [mark]
    }]));
    assert!(
        found
            .iter()
            .any(|p| p.contains("E-T-30") && p.contains("escape")),
        "{found:#?}"
    );
    assert!(problems(json!([{"id": "E-T-30", "escape": true, "steps": [held_get()]}])).is_empty());
    assert!(
        problems(json!([{"id": "M-T-30", "steps": [held_get()], "expected_fail": [mark]}]))
            .is_empty()
    );
}

#[test]
fn an_escape_record_is_never_expected_fail() {
    let record = |kind: &str, verdict: &str| {
        json!({"kind": kind, "row": "E-NET-01", "dialect": "claude", "protection": "full",
               "os": "linux", "leg": "fake", "verdict": verdict})
    };
    assert!(schema_errors("leg-report.schema.json", None, &record("escape", "fail")).is_empty());
    assert!(
        schema_errors(
            "leg-report.schema.json",
            None,
            &record("cell", "expected_fail")
        )
        .is_empty()
    );
    assert!(
        !schema_errors(
            "leg-report.schema.json",
            None,
            &record("escape", "expected_fail")
        )
        .is_empty()
    );
}

#[test]
fn a_mark_for_a_run_the_cell_never_has_fails_validation() {
    let untimed = json!({"calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
                         "require": [{"kind": "stop.block", "source": "sandbox"}]});
    let mark = |timings: &str| json!([{"issue": 6672, "step": 1, "assertion": "require[0]", "observed": 0, "timings": timings}]);
    // Outside the production-timing set: never run at production timings.
    let found = problems(
        json!([{"id": "M-T-31", "steps": [untimed], "expected_fail": mark("production")}]),
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-31") && p.contains("production")),
        "{found:#?}"
    );
    assert!(
        problems(json!([{"id": "M-T-31", "steps": [untimed], "expected_fail": mark("scaled")}]))
            .is_empty()
    );
    // A hand production_only cell skips scaled runs.
    let found = problems(
        json!([{"id": "M-T-32", "production_only": ["hold", "HOOK_TIMEOUT"],
                                 "steps": [untimed], "expected_fail": mark("scaled")}]),
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-32") && p.contains("scaled")),
        "{found:#?}"
    );
    assert!(
        problems(
            json!([{"id": "M-T-32", "production_only": ["hold", "HOOK_TIMEOUT"],
                             "steps": [untimed], "expected_fail": mark("production")}])
        )
        .is_empty()
    );
}

#[test]
fn production_only_and_production_nightly_only_together_fail_validation() {
    let found = problems(
        json!([{"id": "M-T-33", "production_only": ["hold", "HOOK_TIMEOUT"],
                                 "production_nightly_only": "Q300", "steps": [held_get()]}]),
    );
    assert!(
        found
            .iter()
            .any(|p| p.contains("M-T-33") && p.contains("production_nightly_only")),
        "{found:#?}"
    );
}

#[test]
fn a_pins_file_naming_an_unknown_kind_fails_validation() {
    for shows in [
        json!({"hook.no-such-word": []}),
        json!({"hook.posttoolusefailure-blocked": [{"kind": "stop.blok"}]}),
    ] {
        let pins: PinsFile = serde_json::from_value(json!({"dialect": "codex", "shows": shows}))
            .expect("a pins file");
        let check = validator().with_pins(Pins::from_files(vec![pins]));
        let found = check.check(&batch(json!([{"id": "M-T-34", "steps": [held_get()]}])));
        assert!(
            found
                .iter()
                .any(|p| p.contains("pins") && p.contains("codex")),
            "{shows}: {found:#?}"
        );
    }
}

#[test]
fn a_hook_word_purlis_answers_with_nothing_is_not_a_kind() {
    // `hookreg::NO_OPS`: answered with exit 0 and never written.
    let kinds = Kinds::from_core();
    for word in purlis_core::hookreg::NO_OPS {
        assert!(!kinds.knows(&format!("hook.{word}")), "{word}");
    }
    assert!(kinds.knows("hook.posttooluse-blocked"));
}

/// Chat 682's run record (test-matrix-phase-1.md §8, B0, 2026-11-15) as the plane's live checker
/// judged its log (`tests/fixtures/chat-682-b0.json`): the step assertions that failed and the
/// global checks that failed, judged against B0's marks. Every cell is `expected_fail` on the
/// issues the record lists, M-PRB-03 included (its only failure is the doubled ask, #6674).
#[test]
fn b0_judges_chat_682s_record_as_the_record_says() {
    let b0 = Batch::from_value(read_json(&root().join("cells/B0.json"))).expect("B0 reads");
    let record = read_json(&root().join("tests/fixtures/chat-682-b0.json"));
    let live = run(Dialect::Claude, Leg::Live, TimingsMode::Production);
    let expected: [(&str, &[u64]); 7] = [
        ("M-PRB-01", &[6672, 6673, 6674]),
        ("M-PRB-02", &[6673, 6674]),
        ("M-PRB-03", &[6674]),
        ("M-PRB-04", &[6657]),
        ("M-PRB-04b", &[6657]),
        ("M-PRB-05", &[6675]),
        ("M-PRB-06", &[6672, 6673]),
    ];
    let recorded = record["cells"].as_object().expect("the record's cells");
    assert_eq!(
        recorded.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        expected.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>(),
        "the record covers B0's cells"
    );
    for (id, issues) in expected {
        let cell = b0.cells.iter().find(|c| c.id == id).expect("the cell");
        assert!(
            cell.expected_fail.iter().all(|m| m.note.is_some()),
            "{id}: every mark cites the record"
        );
        let failed: Vec<Assertion> = recorded[id]["failed"]
            .as_array()
            .expect("failed")
            .iter()
            .map(|f| fails(1, f[0].as_str().expect("an assertion"), f[1].clone()))
            .collect();
        let globals: Vec<GlobalFailure> = recorded[id]["globals"]
            .as_array()
            .expect("globals")
            .iter()
            .map(|g| GlobalFailure {
                check: g["check"].as_str().expect("a check").into(),
                refusal: g["refusal"].as_str().expect("a refusal").into(),
            })
            .collect();
        assert_eq!(
            judge(cell, &live, &failed, &globals),
            Verdict::ExpectedFail {
                issues: issues.iter().copied().collect()
            },
            "{id}"
        );
    }
}

/// **The Codex and OpenCode pins** (ticket 3): both files exist and pass their schema; each
/// names the recording it was drawn from, which is checked in; and each event it says the
/// dialect shows something else for is one the dialect does not produce, while what it shows
/// instead is.
#[test]
fn the_codex_and_opencode_pins_files_exist_and_are_drawn_from_their_recordings() {
    let kinds = Kinds::from_core();
    // Found by the dialect each names, as the loader reads them (`Pins::read`): a file named
    // `opencode.json` is OpenCode's own configuration's name, which a sandboxed chat may not
    // write, so OpenCode's is `opencode.pins.json`.
    let files: Vec<(Dialect, std::path::PathBuf)> = fs::read_dir(root().join("pins"))
        .expect("pins/ reads")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let pins: PinsFile = serde_json::from_value(read_json(&p)).expect("a pins file");
            (pins.dialect, p)
        })
        .collect();
    for (word, dialect) in [("codex", Dialect::Codex), ("opencode", Dialect::Opencode)] {
        let path = files
            .iter()
            .find(|(d, _)| *d == dialect)
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| panic!("no pins file in pins/ names {word}"));
        let value = read_json(&path);
        let errors = schema_errors("cells.schema.json", Some("pins_file"), &value);
        assert!(errors.is_empty(), "{}: {errors:#?}", path.display());
        let pins: PinsFile = serde_json::from_value(value).expect("a pins file");
        assert_eq!(pins.dialect, dialect, "{}", path.display());
        let recorded = pins.recorded.expect("the recording it was drawn from");
        let from = root().join("../..").join(&recorded.from);
        assert!(
            from.is_file(),
            "{} names {}, not checked in",
            path.display(),
            recorded.from
        );
        assert!(!recorded.facts.is_empty(), "{}", path.display());
        assert!(!pins.shows.is_empty(), "{}", path.display());
        for (kind, instead) in &pins.shows {
            assert!(
                kinds.knows(kind) && !kinds.produced_by(kind, dialect),
                "{}: {word} produces {kind}, so it needs no stand-in",
                path.display()
            );
            for e in instead {
                assert!(
                    kinds.produced_by(&e.kind, dialect),
                    "{}: {word} does not produce {}, shown for {kind}",
                    path.display(),
                    e.kind
                );
            }
        }
    }
}
