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
    HELD_ASK_TIMING, RECEIVER_FIXTURE, Recorded, Sources, Switches, Why, brief, check_live, diff,
    live_report, read_log, read_report, scope_suite,
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
    let on_macos = |covers: &std::collections::BTreeMap<String, Vec<String>>| {
        covers
            .get("macos")
            .is_some_and(|c| c.iter().any(|c| c == "macos-live-check"))
    };
    let wanted = real.switches.iter().filter(|e| on_macos(&e.covers)).count()
        + real
            .substitutions
            .iter()
            .filter(|e| on_macos(&e.covers))
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
    let live = live_report(&[b0()], &chat_682(), Dialect::Claude, Os::Macos).expect("a report");
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
    let live = live_report(&[b0()], &log, Dialect::Claude, Os::Macos).expect("a report");
    let one = live.iter().find(|r| r.leg.row == "M-PRB-03").unwrap();
    assert_eq!(one.leg.verdict, ReportVerdict::Fail);
    assert!(one.leg.reason.as_deref().unwrap().contains("require[1]"));
}

#[test]
fn the_live_report_and_the_suite_report_diff_per_cell() {
    let live = live_report(&[b0()], &chat_682(), Dialect::Claude, Os::Macos).expect("a report");
    let mut suite_report: Vec<Recorded> = live
        .iter()
        .map(|r| suite(&r.leg.row, r.leg.verdict, ReportLeg::Fake))
        .collect();

    // the suite agrees with chat 682 on every cell
    assert!(diff(&live, &suite_report).agrees());

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
    assert!(!d.no_differences());
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
    let live = live_report(&[b0()], &chat_682(), Dialect::Claude, Os::Macos).unwrap();
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
    assert!(diff(&[live.clone()], &[same]).agrees());

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
    let d = diff(&[live], &[silent]);
    assert!(d.agrees());
    assert_eq!(d.events_one_side.len(), 1, "one side's events are flagged");
    assert_eq!(d.events_compared, 0);
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
    for r in live_report(&[b0()], &chat_682(), Dialect::Claude, Os::Macos).unwrap() {
        let value = serde_json::to_value(&r).unwrap();
        assert!(validator.is_valid(&value), "{value}");
    }
}

// ---- the log is strict -------------------------------------------------------------------------

fn report_of(log: Value) -> Result<Vec<Recorded>, String> {
    live_report(&[b0()], &log, Dialect::Claude, Os::Macos)
}

#[test]
fn a_cell_the_log_does_not_mention_is_not_run_with_its_reason() {
    let log = json!({"cells": {"M-PRB-01": {"failed": [], "globals": []}}});
    let live = report_of(log).expect("a report");
    assert_eq!(
        live.len(),
        7,
        "every cell of B0 for the harness has a record"
    );
    let ran = live.iter().find(|r| r.leg.row == "M-PRB-01").unwrap();
    assert_ne!(ran.leg.verdict, ReportVerdict::NotRun);
    for r in live.iter().filter(|r| r.leg.row != "M-PRB-01") {
        assert_eq!(r.leg.verdict, ReportVerdict::NotRun, "{}", r.leg.row);
        assert_eq!(r.leg.reason.as_deref(), Some("not in the live log"));
        assert!(r.events.is_none());
    }
}

#[test]
fn a_malformed_log_is_an_error_never_a_pass() {
    let bad: [(&str, Value); 14] = [
        ("an empty entry", json!({"cells": {"M-PRB-01": {}}})),
        ("a null entry", json!({"cells": {"M-PRB-01": null}})),
        (
            "failed missing",
            json!({"cells": {"M-PRB-01": {"globals": []}}}),
        ),
        (
            "globals missing",
            json!({"cells": {"M-PRB-01": {"failed": []}}}),
        ),
        (
            "a typo'd key",
            json!({"cells": {"M-PRB-01": {"fail": [["require[0]", 2]], "failed": [], "globals": []}}}),
        ),
        (
            "failed as a string",
            json!({"cells": {"M-PRB-01": {"failed": "require[0]", "globals": []}}}),
        ),
        (
            "a failure of one item",
            json!({"cells": {"M-PRB-01": {"failed": [["require[0]"]], "globals": []}}}),
        ),
        (
            "a step of 0",
            json!({"cells": {"M-PRB-01": {"failed": [[0, "require[0]", 1]], "globals": []}}}),
        ),
        (
            "a global of one key",
            json!({"cells": {"M-PRB-01": {"failed": [], "globals": [{"check": "c"}]}}}),
        ),
        (
            "an unknown cell id",
            json!({"cells": {"M-PRB-1": {"failed": [], "globals": []}}}),
        ),
        (
            "events of numbers",
            json!({"cells": {"M-PRB-01": {"failed": [], "globals": [], "events": [1]}}}),
        ),
        (
            "events as a string",
            json!({"cells": {"M-PRB-01": {"failed": [], "globals": [], "events": "stop.block"}}}),
        ),
        (
            "events as null",
            json!({"cells": {"M-PRB-01": {"failed": [], "globals": [], "events": null}}}),
        ),
        ("a key beside cells", json!({"cell": {}, "cells": {}})),
    ];
    for (what, log) in bad {
        let found = report_of(log);
        assert!(found.is_err(), "{what} was read as a report: {found:?}");
    }
    // the cells key itself
    assert!(report_of(json!({})).is_err());
    assert!(report_of(json!({"cells": []})).is_err());
}

#[test]
fn a_well_formed_entry_keeps_its_events_and_three_item_failures() {
    let log = json!({"cells": {"M-PRB-03": {
        "failed": [[1, "require[0]", 2]], "globals": [], "events": ["hook.pretooluse", "stop.block"]
    }}});
    let live = report_of(log).expect("a report");
    let one = live.iter().find(|r| r.leg.row == "M-PRB-03").unwrap();
    assert_eq!(
        one.events.as_deref(),
        Some(&["hook.pretooluse".to_string(), "stop.block".to_string()][..])
    );
}

#[test]
fn the_live_report_takes_every_cells_file_it_is_given() {
    let (_, other) = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let log = json!({"cells": {
        "live-a": {"failed": [], "globals": []},
        "M-PRB-01": {"failed": [], "globals": []},
    }});
    let live = live_report(&[b0(), other.clone()], &log, Dialect::Claude, Os::Macos).unwrap();
    assert!(live.iter().any(|r| r.leg.row == "live-a"));
    assert!(live.iter().any(|r| r.leg.row == "M-PRB-01"));
    // a cell id in none of the files given is an error
    let err = live_report(&[other], &log, Dialect::Claude, Os::Macos).unwrap_err();
    assert!(err.contains("M-PRB-01"), "{err}");
}

// ---- the diff compares something ---------------------------------------------------------------

#[test]
fn a_diff_that_compared_nothing_does_not_agree() {
    let live = vec![suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live)];
    let d = diff(&live, &[]);
    assert_eq!(d.compared, 0);
    assert!(!d.agrees());
    assert!(d.render().contains("nothing was compared"));
    assert!(!diff(&[], &[]).agrees());
}

#[test]
fn a_cell_the_live_run_did_not_run_is_listed_even_when_the_suite_did_not_either() {
    let mut live = suite("M-T-01", ReportVerdict::NotRun, ReportLeg::Live);
    live.leg.reason = Some("not in the live log".to_owned());
    let mut other = suite("M-T-01", ReportVerdict::NotRun, ReportLeg::Fake);
    other.leg.reason = Some("skipped-for-timing".to_owned());
    let d = diff(&[live], &[other]);
    assert!(d.verdicts.is_empty(), "the verdicts are equal");
    assert_eq!(d.live_not_run.len(), 1);
    assert!(!d.agrees());
    assert!(d.render().contains("not in the live log"), "{}", d.render());
}

#[test]
fn the_diff_counts_the_cells_with_no_event_comparison() {
    let kinds = |ks: &[&str]| Some(ks.iter().map(|k| k.to_string()).collect::<Vec<_>>());
    let mut a = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live);
    a.events = kinds(&["stop.block"]);
    let mut a_suite = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    a_suite.events = kinds(&["stop.block"]);
    let b = suite("M-T-02", ReportVerdict::Pass, ReportLeg::Live);
    let mut b_suite = suite("M-T-02", ReportVerdict::Pass, ReportLeg::Fake);
    b_suite.events = kinds(&["stop.block"]);
    let c = suite("M-T-03", ReportVerdict::Pass, ReportLeg::Live);
    let c_suite = suite("M-T-03", ReportVerdict::Pass, ReportLeg::Fake);
    let d = diff(&[a, b, c], &[a_suite, b_suite, c_suite]);
    assert_eq!((d.compared, d.events_compared), (3, 1));
    assert_eq!(d.events_one_side.len(), 1);
    assert_eq!(d.events_one_side[0].row, "M-T-02");
    let text = d.render();
    assert!(
        text.contains("1 had an event comparison, 2 had none"),
        "{text}"
    );
    assert!(text.contains("only one side recorded events"), "{text}");
}

#[test]
fn the_suite_side_is_scoped_to_the_cells_the_live_side_has() {
    let live = vec![suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live)];
    let mut codex = suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake);
    codex.leg.dialect = Some(Dialect::Codex);
    let kept = scope_suite(
        &live,
        vec![
            suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake),
            suite("M-T-02", ReportVerdict::Pass, ReportLeg::Fake),
            codex,
        ],
    );
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].leg.row, "M-T-01");
}

#[test]
fn the_left_file_must_be_the_live_leg() {
    assert!(check_live(&[suite("M-T-01", ReportVerdict::Pass, ReportLeg::Live)]).is_ok());
    let err = check_live(&[suite("M-T-01", ReportVerdict::Pass, ReportLeg::Fake)]).unwrap_err();
    assert!(err.contains("live"), "{err}");
}

#[test]
fn a_report_is_checked_against_the_leg_report_schema_before_it_is_read() {
    let ok = r#"{"kind":"cell","row":"M-T-01","dialect":"claude","protection":"full","os":"macos","leg":"live","verdict":"pass"}"#;
    assert_eq!(read_report("r.jsonl", ok).unwrap().len(), 1);
    for (what, line) in [
        (
            "an unknown key",
            ok.replace("\"pass\"}", "\"pass\",\"extra\":1}"),
        ),
        ("a verdict out of its list", ok.replace("pass", "passed")),
        (
            "a cell with no dialect",
            ok.replace("\"dialect\":\"claude\",", ""),
        ),
        (
            "a not_run with no reason",
            ok.replace("\"pass\"", "\"not_run\""),
        ),
        (
            "events of numbers",
            ok.replace("\"pass\"}", "\"pass\",\"events\":[1]}"),
        ),
        (
            "an expected_fail escape",
            ok.replace("cell", "escape")
                .replace("\"pass\"", "\"expected_fail\""),
        ),
        ("not JSON", "{".to_owned()),
    ] {
        assert!(read_report("r.jsonl", &line).is_err(), "{what} was read");
    }
    assert!(read_report("r.jsonl", "\n\n").is_err(), "an empty report");
}

// ---- the brief refuses drift -------------------------------------------------------------------

#[test]
fn the_switch_list_refuses_a_key_it_does_not_know_and_an_entry_with_no_name() {
    let one = |v: Value| serde_json::from_value::<Switches>(v);
    assert!(one(json!({"extra": []})).is_err());
    assert!(
        one(json!({"substitutions": [{"reason": "r"}]})).is_err(),
        "no name"
    );
    assert!(one(json!({"substitutions": [{"name": "n", "reason": "r", "cover": {}}]})).is_err());
    assert!(
        one(json!({"switches": [{"file": "a.rs", "kind": "switch", "reason": "r"}]})).is_err(),
        "no `at`"
    );
    assert!(one(json!({"switches": [{"file": "a.rs", "at": "x", "kind": "switch", "reason": "r", "extra": 1}]})).is_err());
    assert!(
        one(json!({"switches": [{"file": "a.rs", "at": "x", "kind": "switch", "reason": "r"}]}))
            .is_ok()
    );
}

#[test]
fn an_escape_row_with_a_column_the_reader_does_not_know_is_refused() {
    let row = json!({"id": "E-T-09", "ticket": "20", "Full": "refused", "Basic": "refused",
        "off": "n/a", "dialects": "claude", "os": "both", "real_binary": "no", "lives": "yes"});
    assert!(serde_json::from_value::<purlis_scenarios::coverage::EscapeRow>(row).is_err());
}

#[test]
fn a_cover_that_cannot_be_read_is_a_problem_not_a_skip() {
    let cells = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let coverage = list(json!({"rows": [row("M-T-01", "claude", "macos", "live live-a")]}));
    let repo = repo(vec![cells], json!([]));
    let none = no_switches();
    let err = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(&coverage, &repo, &none),
    )
    .unwrap_err();
    assert!(err.contains("M-T-01"), "{err}");
}

#[test]
fn a_covered_escape_the_table_does_not_have_is_a_problem_not_a_skip() {
    let cells = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let coverage = list(json!({"escapes": [
        {"id": "E-T-77", "covers": {"full": "live: live-a"}, "control": "live: live-a"}
    ]}));
    let repo = repo(vec![cells], json!([]));
    let none = no_switches();
    let err = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(&coverage, &repo, &none),
    )
    .unwrap_err();
    assert!(err.contains("E-T-77"), "{err}");
}

#[test]
fn the_real_escape_table_makes_a_brief_for_every_harness_and_os() {
    let coverage = Coverage::read().expect("coverage.json");
    let repo = Repo::read().expect("the repo's files");
    let real = Switches::repo().expect("the switch list");
    let marked: Vec<&str> = repo
        .escapes
        .iter()
        .filter(|r| r.live == "yes")
        .map(|r| r.id.as_str())
        .collect();
    assert!(
        !marked.is_empty(),
        "the table marks escapes for the live pass"
    );
    for harness in Dialect::ALL {
        for os in [Os::Linux, Os::Macos] {
            let b = brief(harness, os, &sources(&coverage, &repo, &real))
                .unwrap_or_else(|e| panic!("{harness:?} on {os:?}: {e}"));
            for row in repo.escapes.iter().filter(|r| r.live == "yes") {
                let applies = row.applies().unwrap().contains(&(harness, os));
                let has = b.checks.iter().any(|c| {
                    c.why == Why::Escape && c.id.starts_with(&format!("escape:{}:", row.id))
                });
                assert_eq!(has, applies, "{} for {harness:?} on {os:?}", row.id);
            }
        }
    }
}

#[test]
fn the_brief_says_which_records_report_judges_and_which_a_person_does() {
    let b = brief(
        Dialect::Claude,
        Os::Macos,
        &sources(
            &Coverage::read().unwrap(),
            &Repo::read().unwrap(),
            &Switches::repo().unwrap(),
        ),
    )
    .unwrap();
    let text = b.markdown();
    assert!(text.contains("\"failed\""), "the log format is shown");
    assert!(text.contains("judged by a person"));
    assert!(text.contains("judged by `tools/live-pass report`"));
    assert!(!text.contains("judge nothing"));
    assert!(Why::LiveCover.judged_by_report());
    assert!(!Why::Escape.judged_by_report());
}

// ---- a live fail never reads as agreement ------------------------------------------------------

#[test]
fn a_plain_cell_that_fails_live_and_in_the_suite_is_listed_and_does_not_agree() {
    let mut log = chat_682();
    log["cells"]["M-PRB-03"]["failed"] = json!([["require[0]", 2], ["require[1]", 7]]);
    let live = live_report(&[b0()], &log, Dialect::Claude, Os::Macos).expect("a report");
    // the suite fails the very same cell
    let suite_report: Vec<Recorded> = live
        .iter()
        .map(|r| suite(&r.leg.row, r.leg.verdict, ReportLeg::Fake))
        .collect();
    let d = diff(&live, &suite_report);
    assert!(d.verdicts.is_empty(), "the two verdicts are equal");
    assert_eq!(d.live_fail.len(), 1, "{:?}", d.live_fail);
    assert_eq!(d.live_fail[0].0.row, "M-PRB-03");
    assert!(!d.no_differences());
    assert!(!d.agrees());
    let out = d.render();
    assert!(
        out.contains("FAIL") && out.contains("M-PRB-03") && out.contains("failed live"),
        "{out}"
    );
    assert!(!out.contains("ESCAPE"), "{out}");
}

#[test]
fn an_escape_cell_that_fails_live_and_in_the_suite_says_the_refused_action_happened() {
    let mut cell = live_cell("esc-a", json!(["claude"]));
    cell["escape"] = json!(true);
    let (_, escapes) = batch(json!([cell]));
    let log = json!({"cells": {"esc-a": {"failed": [["require[0]", 2]], "globals": []}}});
    let live = live_report(&[escapes], &log, Dialect::Claude, Os::Macos).expect("a report");
    assert_eq!(live[0].leg.kind, EntryKind::Escape);
    assert_eq!(live[0].leg.verdict, ReportVerdict::Fail);
    let mut other = suite("esc-a", ReportVerdict::Fail, ReportLeg::Fake);
    other.leg.kind = EntryKind::Escape;
    let d = diff(&live, &[other]);
    assert!(d.verdicts.is_empty());
    assert_eq!(d.live_fail.len(), 1);
    assert!(!d.agrees());
    let out = d.render();
    assert!(out.contains("ESCAPE") && out.contains("esc-a"), "{out}");
}

#[test]
fn a_live_fail_with_no_suite_record_is_listed_too() {
    let mut log = chat_682();
    log["cells"]["M-PRB-03"]["failed"] = json!([["require[0]", 2], ["require[1]", 7]]);
    let live = live_report(&[b0()], &log, Dialect::Claude, Os::Macos).expect("a report");
    let d = diff(
        &live,
        &[suite("M-PRB-01", ReportVerdict::Pass, ReportLeg::Fake)],
    );
    assert_eq!(d.live_fail.len(), 1);
}

// ---- a cell id is never in the log or the cells files twice ------------------------------------

#[test]
fn a_cell_id_given_twice_in_the_log_is_an_error() {
    let twice = r#"{"cells": {"M-PRB-01": {"failed": [["require[0]", 1]], "globals": []},
                              "M-PRB-01": {"failed": [], "globals": []}}}"#;
    let err = read_log("log.json", twice).unwrap_err();
    assert!(err.contains("M-PRB-01") && err.contains("twice"), "{err}");
    // a log with each id once reads, and reads as the same value serde_json gives
    let once = r#"{"note": "x", "cells": {"M-PRB-01": {"failed": [], "globals": []}}}"#;
    assert_eq!(
        read_log("log.json", once).unwrap(),
        serde_json::from_str::<Value>(once).unwrap()
    );
    // a repeated top-level key, trailing text and a non-object `cells` are errors too
    assert!(read_log("log.json", r#"{"cells": {}, "cells": {}}"#).is_err());
    assert!(read_log("log.json", r#"{"cells": {}} x"#).is_err());
    assert!(read_log("log.json", r#"{"cells": []}"#).is_err());
}

#[test]
fn the_same_cell_id_in_two_cells_files_is_an_error_not_one_shared_entry() {
    let (_, one) = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let (_, two) = batch(json!([live_cell("live-a", json!(["claude"]))]));
    let log = json!({"cells": {"live-a": {"failed": [], "globals": []}}});
    let err = live_report(
        &[one.clone(), two.clone()],
        &log,
        Dialect::Claude,
        Os::Macos,
    )
    .unwrap_err();
    assert!(
        err.contains("live-a") && err.contains("more than one"),
        "{err}"
    );
    // the same id for another harness is no clash
    let (_, other) = batch(json!([live_cell("live-a", json!(["codex"]))]));
    assert!(live_report(&[one, other], &log, Dialect::Claude, Os::Macos).is_ok());
}
