// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The live-pass tools** (T1.1-37, #6884): the brief generator over small lists written here and
//! over the repo's own, and the live-versus-suite diff on chat 682's log (T1.1-05's B0, judged
//! on 2026-11-15).

use std::collections::BTreeSet;
use std::fs;

use purlis_scenarios::coverage::{Coverage, Repo};
use purlis_scenarios::kinds::dialect_words;
use purlis_scenarios::livepass::{
    HELD_ASK_TIMING, RECEIVER_FIXTURE, Recorded, Sources, Switches, Why, brief, diff, live_report,
};
use purlis_scenarios::model::{
    Batch, Dialect, EntryKind, LegRecord, Os, Protection, ReportLeg, ReportVerdict,
};
use purlis_scenarios::root;
use serde_json::{Value, json};

// ---- helpers -----------------------------------------------------------------------------------

fn batch(cells: Value) -> (String, Batch) {
    let batch = Batch::from_value(json!({
        "batch": "T0",
        "protection": "full",
        "persona": "steward",
        "hosts": {"h1": "one.test"},
        "cells": cells,
    }))
    .expect("a batch");
    ("cells/T0.json".to_string(), batch)
}

/// A live-only cell: it names what it calls and what a live run must see.
fn live_cell(id: &str, dialects: Value) -> Value {
    json!({
        "id": id,
        "dialects": dialects,
        "live_only": {"reason": "model_behaviour"},
        "steps": [{
            "calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
            "require": [{"kind": "stop.block", "source": "sandbox"}]
        }]
    })
}

fn list(value: Value) -> Coverage {
    Coverage::from_value(value).expect("a coverage list")
}

fn row(row: &str, dialect: &str, os: &str, cover: &str) -> Value {
    json!({
        "row": row, "dialect": dialect, "os": os, "log": true, "window": false, "chat": true,
        "carried": "live", "kind": "live", "reason": "model_behaviour", "cover": cover,
    })
}

fn fixture(row: &str, os: &str, cover: &str) -> Value {
    json!({"row": row, "os": os, "reason": "machine_wide_state", "cover": cover})
}

/// The repo's, with these cells and escape rows in place of its own.
fn repo(batches: Vec<(String, Batch)>, escapes: Value) -> Repo {
    Repo {
        batches,
        escapes: serde_json::from_value(escapes).expect("escape rows"),
        ..Repo::read().expect("the repo's files")
    }
}

fn switches(value: Value) -> Switches {
    serde_json::from_value(value).expect("a switch list")
}

fn no_switches() -> Switches {
    switches(json!({}))
}

fn ids(brief: &purlis_scenarios::livepass::Brief, why: Why) -> Vec<String> {
    brief
        .checks
        .iter()
        .filter(|c| c.why == why)
        .map(|c| c.id.clone())
        .collect()
}

fn sources<'a>(c: &'a Coverage, r: &'a Repo, s: &'a Switches) -> Sources<'a> {
    Sources {
        coverage: c,
        repo: r,
        switches: s,
    }
}

// ---- the brief ---------------------------------------------------------------------------------

#[test]
fn the_brief_lists_every_live_cover() {
    let cells = batch(json!([
        live_cell("live-a", json!(["claude", "codex"])),
        live_cell("live-b", json!(["codex"])),
        live_cell("live-unlisted", json!(["claude"])),
    ]));
    let escapes = json!([
        {"id": "E-T-01", "ticket": "20", "Full": "refused: a", "Basic": "refused: b",
         "off": "allowed by design: no sandbox", "dialects": "claude, codex", "os": "both",
         "real_binary": "no", "live": "yes", "live_observable": "the command fails"},
        {"id": "E-T-02", "ticket": "20", "Full": "refused: a", "Basic": "refused: b",
         "off": "allowed by design: no sandbox", "dialects": "claude, codex, opencode",
         "os": "both", "real_binary": "no", "live": "no", "live_observable": ""},
        {"id": "E-T-03", "ticket": "20", "Full": "refused: a", "Basic": "n/a",
         "off": "allowed by design: no sandbox", "dialects": "opencode", "os": "macos",
         "real_binary": "no", "live": "yes", "live_observable": "a Blocked row"},
    ]);
    let coverage = list(json!({"rows": [
        row("M-T-01", "claude", "macos", "live: live-a"),
        row("M-T-01", "claude", "linux", "live: live-a"),
        row("M-T-02", "codex", "macos", "live: live-b"),
        row("M-T-03", "claude", "macos", "pending: 37"),
        {"row": "M-T-04", "dialect": "claude", "os": "macos", "log": true, "window": false,
         "chat": false, "carried": "scenario", "kind": "scenario",
         "cover": "scenario: cells/T0.json#live-unlisted"},
    ]}));
    let repo = repo(vec![cells], escapes);
    let none = no_switches();
    let src = sources(&coverage, &repo, &none);

    let claude = brief(Dialect::Claude, Os::Macos, &src).expect("a brief");
    assert_eq!(ids(&claude, Why::LiveCover), ["live-a"]);
    // the observable is the cell's own: what it calls, and what the live run must see
    let a = claude.checks.iter().find(|c| c.id == "live-a").unwrap();
    assert!(a.what.contains("get https://$h1 3"), "{}", a.what);
    assert!(a.observable.contains("stop.block"), "{}", a.observable);
    assert_eq!(
        ids(&claude, Why::Escape),
        ["escape:E-T-01:full", "escape:E-T-01:basic"]
    );
    let e = claude
        .checks
        .iter()
        .find(|c| c.id == "escape:E-T-01:full")
        .unwrap();
    assert_eq!(e.observable, "the command fails");

    let codex = brief(Dialect::Codex, Os::Macos, &src).expect("a brief");
    assert_eq!(ids(&codex, Why::LiveCover), ["live-b"]);
    assert_eq!(
        ids(&codex, Why::Escape),
        ["escape:E-T-01:full", "escape:E-T-01:basic"]
    );

    let linux = brief(Dialect::Claude, Os::Linux, &src).expect("a brief");
    assert_eq!(ids(&linux, Why::LiveCover), ["live-a"]);

    // OpenCode: an escape its table row applies on macOS alone, at Full alone
    let opencode = brief(Dialect::Opencode, Os::Macos, &src).expect("a brief");
    assert!(ids(&opencode, Why::LiveCover).is_empty());
    assert_eq!(ids(&opencode, Why::Escape), ["escape:E-T-03:full"]);
    assert!(
        ids(
            &brief(Dialect::Opencode, Os::Linux, &src).unwrap(),
            Why::Escape
        )
        .is_empty()
    );

    // one control per hook kind the harness is armed with, and no other
    for harness in Dialect::ALL {
        let b = brief(harness, Os::Linux, &src).expect("a brief");
        let controls: BTreeSet<String> = ids(&b, Why::HookControl).into_iter().collect();
        let armed: BTreeSet<String> = dialect_words(harness)
            .into_iter()
            .map(|w| format!("control:hook.{w}"))
            .collect();
        assert_eq!(controls, armed, "{harness:?}");
        assert!(!controls.is_empty());
    }
    assert!(
        dialect_words(Dialect::Codex).len() < dialect_words(Dialect::Claude).len(),
        "the dialects differ, so the controls do"
    );
}

#[test]
fn a_live_cover_naming_no_live_cell_is_refused() {
    let cells = batch(json!([{
        "id": "not-live", "steps": [{"calls": [], "require": [{"kind": "stop.block"}]}]
    }]));
    let coverage = list(json!({"rows": [
        row("M-T-01", "claude", "macos", "live: not-live"),
        row("M-T-02", "claude", "macos", "live: nowhere"),
    ]}));
    let repo = repo(vec![cells], json!([]));
    let none = no_switches();
    let err = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(&coverage, &repo, &none),
    )
    .expect_err("a cover that names no live cell");
    assert!(err.contains("not-live") && err.contains("nowhere"), "{err}");
}

#[test]
fn the_brief_lists_every_macos_release_switch_cover() {
    let list_of = switches(json!({
        "covers": {},
        "substitutions": [
            {"name": "the in-process host", "reason": "r1",
             "covers": {"linux": ["host-process"], "macos": ["macos-live-check"]}},
            {"name": "the mock runtime", "reason": "r2",
             "covers": {"linux": ["linux-release-profile"], "macos": ["macos-live-check"]}},
        ],
        "switches": [
            {"file": "a.rs", "at": "if FENCED {", "kind": "switch", "reason": "r3",
             "covers": {"linux": ["linux-release-profile"], "macos": ["macos-live-check"]}},
            {"file": "b.rs", "at": "if x {", "kind": "switch", "reason": "r4",
             "covers": {"linux": ["linux-release-profile"], "macos": ["host-process"]}},
            {"file": "c.rs", "at": "if y {", "kind": "switch", "reason": "r5",
             "covers": {"linux": ["linux-release-profile"]}},
            {"file": "d.rs", "at": "if z {", "kind": "switch", "reason": "r6",
             "covers": {"macos": ["host-process", "macos-live-check"]}},
        ],
    }));
    let coverage = list(json!({}));
    let repo = repo(vec![], json!([]));
    let src = sources(&coverage, &repo, &list_of);

    let mac = brief(Dialect::Claude, Os::Macos, &src).expect("a brief");
    let mut got = ids(&mac, Why::ReleaseSwitch);
    got.extend(ids(&mac, Why::ReleaseSubstitution));
    assert_eq!(
        got,
        [
            "release:switch:a.rs:if FENCED {",
            "release:switch:d.rs:if z {",
            "release:substitution:the in-process host",
            "release:substitution:the mock runtime",
        ]
    );
    // each check says why the entry exists, so a runner knows what a pass shows
    let one = mac
        .checks
        .iter()
        .find(|c| c.id == "release:substitution:the mock runtime")
        .unwrap();
    assert!(one.what.contains("r2"), "{}", one.what);

    // no Linux entry is a macOS cover, and none is listed on Linux
    let linux = brief(Dialect::Claude, Os::Linux, &src).expect("a brief");
    assert!(ids(&linux, Why::ReleaseSwitch).is_empty());
    assert!(ids(&linux, Why::ReleaseSubstitution).is_empty());
}

#[test]
fn the_brief_lists_one_check_for_every_macos_cover_of_the_real_switch_list() {
    let real = Switches::repo().expect("the scenario host's switch list");
    let wanted = real
        .switches
        .iter()
        .chain(&real.substitutions)
        .filter(|e| {
            e.covers
                .get("macos")
                .is_some_and(|c| c.iter().any(|c| c == "macos-live-check"))
        })
        .count();
    assert!(wanted > 0, "the list has macOS live checks");
    let coverage = Coverage::read().expect("coverage.json");
    let repo = Repo::read().expect("the repo's files");
    let mac = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(&coverage, &repo, &real),
    )
    .expect("a brief");
    let got = ids(&mac, Why::ReleaseSwitch).len() + ids(&mac, Why::ReleaseSubstitution).len();
    assert_eq!(got, wanted);
}

#[test]
fn the_brief_lists_the_receiver_check_only_when_its_row_is_carried() {
    let carried = format!("fixture: app/src-tauri/src/lib.rs::{RECEIVER_FIXTURE}");
    let repo = repo(vec![], json!([]));
    let none = no_switches();
    let on = |fixtures: Value, os: Os| {
        let coverage = list(json!({"fixtures": fixtures}));
        let b = brief(Dialect::Claude, os, &sources(&coverage, &repo, &none)).expect("a brief");
        ids(&b, Why::Receiver)
    };

    assert_eq!(
        on(json!([fixture("M-FILE-31", "macos", &carried)]), Os::Macos),
        [RECEIVER_FIXTURE]
    );
    // the row is not carried yet
    assert!(
        on(
            json!([fixture("M-FILE-31", "macos", "pending: 52")]),
            Os::Macos
        )
        .is_empty()
    );
    // carried on Linux only: the receiver is macOS's
    assert!(on(json!([fixture("M-FILE-31", "linux", &carried)]), Os::Macos).is_empty());
    // another row's fixture, or another test of the row
    assert!(on(json!([fixture("M-NET-01", "macos", &carried)]), Os::Macos).is_empty());
    assert!(
        on(
            json!([fixture(
                "M-FILE-31",
                "macos",
                "fixture: app/src-tauri/src/lib.rs::another"
            )]),
            Os::Macos
        )
        .is_empty()
    );
    // carried, but this is the Linux pass
    assert!(on(json!([fixture("M-FILE-31", "macos", &carried)]), Os::Linux).is_empty());
    // no fixtures at all
    assert!(on(json!([]), Os::Macos).is_empty());
}

#[test]
fn the_brief_carries_the_timed_held_ask_check_on_macos_only() {
    let coverage = list(json!({}));
    let repo = repo(vec![], json!([]));
    let none = no_switches();
    for harness in Dialect::ALL {
        let mac = brief(harness, Os::Macos, &sources(&coverage, &repo, &none)).unwrap();
        assert_eq!(
            ids(&mac, Why::HeldAskTiming),
            [HELD_ASK_TIMING],
            "{harness:?}"
        );
        let lin = brief(harness, Os::Linux, &sources(&coverage, &repo, &none)).unwrap();
        assert!(ids(&lin, Why::HeldAskTiming).is_empty(), "{harness:?}");
    }
}

#[test]
fn the_markdown_brief_names_every_check_and_how_to_run_it() {
    let cells = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let coverage = list(json!({"rows": [row("M-T-01", "claude", "macos", "live: live-a")]}));
    let repo = repo(vec![cells], json!([]));
    let none = no_switches();
    let b = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(&coverage, &repo, &none),
    )
    .unwrap();
    let text = b.markdown();
    for c in &b.checks {
        assert!(text.contains(&c.id), "the brief omits {}", c.id);
    }
    // record only, non-interactive, an installed release-profile build without test-support
    for word in [
        "record only",
        "non-interactive",
        "release",
        "test-support",
        "phase-1-proven",
    ] {
        assert!(text.contains(word), "the brief never says `{word}`");
    }
}

#[test]
fn the_release_checklist_names_the_live_pass() {
    let text = fs::read_to_string(root().join("../../docs/updating.md")).expect("docs/updating.md");
    let step = text
        .split("## Cutting a stable release")
        .nth(1)
        .expect("the release section");
    let step = step.split("\n## ").next().unwrap();
    assert!(
        step.contains("tools/live-pass"),
        "the checklist never names tools/live-pass"
    );
}

// ---- the report and the diff -------------------------------------------------------------------

fn b0() -> Batch {
    let text = fs::read_to_string(root().join("cells/B0.json")).expect("cells/B0.json");
    Batch::from_value(serde_json::from_str(&text).unwrap()).expect("B0")
}

fn chat_682() -> Value {
    let text = fs::read_to_string(root().join("tests/fixtures/chat-682-b0.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn suite(row: &str, verdict: ReportVerdict, leg: ReportLeg) -> Recorded {
    Recorded {
        leg: LegRecord {
            kind: EntryKind::Cell,
            row: row.to_string(),
            test: None,
            dialect: Some(Dialect::Claude),
            protection: Some(Protection::Full),
            os: Os::Macos,
            leg,
            timings: None,
            verdict,
            reason: None,
        },
        events: None,
    }
}

#[test]
fn chat_682_judged_by_the_live_report_is_what_matrix_check_said() {
    let live = live_report(&b0(), &chat_682(), Dialect::Claude, Os::Macos).expect("a report");
    let seen: Vec<(&str, ReportVerdict)> = live
        .iter()
        .map(|r| (r.leg.row.as_str(), r.leg.verdict))
        .collect();
    // every failure in chat 682 is pinned by B0's marks, and each mark pinned something
    assert_eq!(seen.len(), 7, "{seen:?}");
    assert!(
        seen.iter().all(|(_, v)| *v == ReportVerdict::ExpectedFail),
        "{seen:?}"
    );
    assert!(
        live.iter()
            .all(|r| r.leg.leg == ReportLeg::Live && r.leg.os == Os::Macos)
    );
    assert!(live.iter().all(|r| r.leg.dialect == Some(Dialect::Claude)));
    assert!(
        live.iter()
            .all(|r| r.leg.protection == Some(Protection::Full))
    );
}

#[test]
fn a_failure_in_the_log_that_no_mark_pins_is_a_failed_cell() {
    let mut log = chat_682();
    log["cells"]["M-PRB-03"]["failed"] = json!([["require[0]", 2], ["require[1]", 7]]);
    let live = live_report(&b0(), &log, Dialect::Claude, Os::Macos).expect("a report");
    let one = live.iter().find(|r| r.leg.row == "M-PRB-03").unwrap();
    assert_eq!(one.leg.verdict, ReportVerdict::Fail);
    assert!(one.leg.reason.as_deref().unwrap().contains("require[1]"));
}

#[test]
fn the_live_report_and_the_suite_report_diff_per_cell() {
    let live = live_report(&b0(), &chat_682(), Dialect::Claude, Os::Macos).expect("a report");
    let mut suite_report: Vec<Recorded> = live
        .iter()
        .map(|r| suite(&r.leg.row, r.leg.verdict, ReportLeg::Fake))
        .collect();

    // the suite agrees with chat 682 on every cell
    assert!(diff(&live, &suite_report).is_empty());

    // the suite says M-PRB-06 passes where the live run showed the pinned failures; it has no
    // M-PRB-05 at all, and one cell the live run never ran
    suite_report
        .iter_mut()
        .find(|r| r.leg.row == "M-PRB-06")
        .unwrap()
        .leg
        .verdict = ReportVerdict::Pass;
    suite_report.retain(|r| r.leg.row != "M-PRB-05");
    suite_report.push(suite("M-PRB-99", ReportVerdict::Pass, ReportLeg::Fake));

    let d = diff(&live, &suite_report);
    assert!(!d.is_empty());
    assert_eq!(d.verdicts.len(), 1, "{:?}", d.verdicts);
    assert_eq!(d.verdicts[0].key.row, "M-PRB-06");
    assert_eq!(d.verdicts[0].live, ReportVerdict::ExpectedFail);
    assert_eq!(d.verdicts[0].suite, ReportVerdict::Pass);
    assert_eq!(d.verdicts[0].suite_leg, ReportLeg::Fake);
    assert_eq!(
        d.only_live
            .iter()
            .map(|k| k.row.as_str())
            .collect::<Vec<_>>(),
        ["M-PRB-05"]
    );
    assert_eq!(
        d.only_suite
            .iter()
            .map(|k| k.row.as_str())
            .collect::<Vec<_>>(),
        ["M-PRB-99"]
    );
    assert!(d.events.is_empty());
}

#[test]
fn a_cell_is_compared_only_with_the_same_dialect_protection_and_os() {
    let live = live_report(&b0(), &chat_682(), Dialect::Claude, Os::Macos).unwrap();
    let mut other = suite("M-PRB-01", ReportVerdict::Pass, ReportLeg::Fake);
    other.leg.os = Os::Linux;
    let d = diff(&live[..1], &[other]);
    assert!(d.verdicts.is_empty(), "{:?}", d.verdicts);
    assert_eq!(d.only_live.len(), 1);
    assert_eq!(d.only_suite.len(), 1);
}

#[test]
fn the_diff_compares_event_sequences_per_cell() {
    let kinds = |ks: &[&str]| Some(ks.iter().map(|k| k.to_string()).collect::<Vec<_>>());
    let mut live = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live);
    live.events = kinds(&["hook.pretooluse", "ask.raised", "stop.block"]);
    let mut same = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    same.events = kinds(&["hook.pretooluse", "ask.raised", "stop.block"]);
    assert!(diff(&[live.clone()], &[same]).is_empty());

    let mut other = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    other.events = kinds(&["hook.pretooluse", "stop.block", "ask.raised"]);
    let d = diff(&[live.clone()], &[other]);
    assert_eq!(d.events.len(), 1);
    assert_eq!(d.events[0].at, 1);
    assert_eq!(d.events[0].live.as_deref(), Some("ask.raised"));
    assert_eq!(d.events[0].suite.as_deref(), Some("stop.block"));

    let mut shorter = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    shorter.events = kinds(&["hook.pretooluse", "ask.raised"]);
    let d = diff(&[live.clone()], &[shorter]);
    assert_eq!((d.events[0].at, d.events[0].suite.clone()), (2, None));

    // a side that recorded no events has nothing to compare, and is no difference
    let silent = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    assert!(diff(&[live], &[silent]).is_empty());
}

#[test]
fn a_live_record_with_events_validates_against_the_leg_report_schema() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(root().join("schema/leg-report.schema.json")).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
    let mut r = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live);
    r.events = Some(vec!["stop.block".to_string()]);
    let value = serde_json::to_value(&r).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?} in {value}");
    let mut bad = value.clone();
    bad["events"] = json!([1]);
    assert!(!validator.is_valid(&bad));
    // a live report of chat 682 validates as the suite's own records do
    for r in live_report(&b0(), &chat_682(), Dialect::Claude, Os::Macos).unwrap() {
        let value = serde_json::to_value(&r).unwrap();
        assert!(validator.is_valid(&value), "{value}");
    }
}
