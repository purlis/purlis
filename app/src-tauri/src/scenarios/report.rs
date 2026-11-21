// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A leg's report, in ticket 5's schema** (`tests/scenarios/schema/leg-report.schema.json`):
//! one record per entry a leg ran or did not run, written by the scenario host as JSON lines
//! under `target/sandbox-scenarios/`, which the workflow uploads with a red run and ticket 8
//! judges. A cell a scaled run does not run for its timings (a hand `production_only` pair, or a
//! `restart: host_killed`) is reported `not_run`, skipped-for-timing, never as passed.
//!
//! And which `[policy]` template a cell's chats start under (Q308): `strict` unless it names
//! another.

use std::path::PathBuf;

use purlis_scenarios::livepass::Recorded;
use purlis_scenarios::model::{
    Batch, Cell, Dialect, EntryKind, LegRecord, Os, Protection, ReportLeg, ReportVerdict,
    TimingsMode,
};
use purlis_scenarios::plan::{Scheduled, Trigger, schedule};
use purlis_scenarios::timings::CoreTimings;

use super::host::{DEFAULT_TEMPLATE, Template};

/// The reason a record gives for a cell a run skipped for its timings.
pub(crate) const SKIPPED_FOR_TIMING: &str = "skipped-for-timing";

/// This machine's OS, as a report names it.
pub(crate) fn this_os() -> Os {
    if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        Os::Linux
    }
}

/// **The records of `batch`'s cells a run at `timings`, started by `trigger`, skips for their
/// timings**: one per cell and dialect, `not_run` and skipped-for-timing.
pub(crate) fn skipped_for_timing(
    batch: &Batch,
    timings: TimingsMode,
    trigger: Trigger,
) -> Vec<LegRecord> {
    batch
        .cells
        .iter()
        .filter(|cell| {
            schedule(cell, &CoreTimings, timings, trigger) == Scheduled::SkippedForTiming
        })
        .flat_map(|cell| {
            cell.dialects.iter().map(move |dialect| LegRecord {
                kind: if cell.escape {
                    EntryKind::Escape
                } else {
                    EntryKind::Cell
                },
                row: cell.id.clone(),
                test: None,
                dialect: Some(*dialect),
                protection: Some(batch.protection),
                os: this_os(),
                leg: ReportLeg::Fake,
                timings: Some(timings),
                verdict: ReportVerdict::NotRun,
                reason: Some(SKIPPED_FOR_TIMING.to_owned()),
                refusal_expected: cell.escape || cell.steps.iter().any(|s| s.refusal_expected),
            })
        })
        .collect()
}

/// **The record of a cell that ran** (#7224, #7229): a cell's, or an escape's when the cell is
/// one, with the kinds of the events the device's log wrote for it: those after `since` (the
/// log's newest `seq` when the cell began, so the canary's and earlier cells' events are left
/// out), ordered by `seq` whatever order they were read in. `tools/live-pass diff` compares them
/// with a live report's, and a comparison of runs reads `refusal_expected` from it rather than
/// from the cells.
pub(crate) fn ran(
    cell: &Cell,
    dialect: Dialect,
    protection: Protection,
    timings: TimingsMode,
    verdict: ReportVerdict,
    reason: Option<String>,
    events: &[purlis_core::eventlog::Event],
    since: u64,
) -> Recorded {
    let mut ordered: Vec<&purlis_core::eventlog::Event> =
        events.iter().filter(|event| event.seq > since).collect();
    ordered.sort_by_key(|event| event.seq);
    Recorded {
        leg: LegRecord {
            kind: if cell.escape {
                EntryKind::Escape
            } else {
                EntryKind::Cell
            },
            row: cell.id.clone(),
            test: None,
            dialect: Some(dialect),
            protection: Some(protection),
            os: this_os(),
            leg: ReportLeg::Fake,
            timings: Some(timings),
            verdict,
            reason,
            refusal_expected: cell.escape || cell.steps.iter().any(|s| s.refusal_expected),
        },
        events: Some(ordered.iter().map(|event| event.kind.clone()).collect()),
    }
}

/// This machine's OS, as a file name says it.
pub(crate) fn os_word() -> &'static str {
    match this_os() {
        Os::Linux => "linux",
        Os::Macos => "macos",
    }
}

/// `target/sandbox-scenarios/<name>`, what a red run uploads.
fn under_target(name: &str) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| super::host::repo().join("target"))
        .join("sandbox-scenarios")
        .join(name)
}

/// Where this leg's report is written: `target/sandbox-scenarios/<os>-fake.jsonl`.
pub(crate) fn report_path() -> PathBuf {
    under_target(&format!("{}-fake.jsonl", os_word()))
}

/// Appends each of `lines` to the file at `at`, one a line.
fn append(at: &std::path::Path, lines: &[String]) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(at)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

/// **Appends `records` to this leg's report**, one JSON object a line: [`LegRecord`]s, or
/// [`Recorded`]s that carry the events a cell's log wrote.
pub(crate) fn write<T: serde::Serialize>(records: &[T]) -> std::io::Result<PathBuf> {
    write_to(&report_path(), records)
}

/// [`write`], to the report at `at`.
fn write_to<T: serde::Serialize>(at: &std::path::Path, records: &[T]) -> std::io::Result<PathBuf> {
    let lines = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    append(at, &lines)?;
    Ok(at.to_path_buf())
}

/// **A scenario's run time** (T1.1-06), appended to `target/sandbox-scenarios/<os>-timings.jsonl`:
/// the per-scenario time on each OS that ticket 54 sets its budgets from.
pub(crate) fn time_of(record: &serde_json::Value) -> std::io::Result<()> {
    append(
        &under_target(&format!("{}-timings.jsonl", os_word())),
        &[record.to_string()],
    )
}

/// **The template `cell`'s chats start under** (Q308): the one it names, else `strict`.
pub(crate) fn template_of(cell: &Cell) -> Template {
    match cell.template.as_deref() {
        Some(word) => {
            Template::parse(word).unwrap_or_else(|| panic!("{}: {word:?} is no template", cell.id))
        }
        None => DEFAULT_TEMPLATE,
    }
}

/// Every batch in `tests/scenarios/cells`.
pub(crate) fn every_batch() -> Vec<Batch> {
    let folder = purlis_scenarios::root().join("cells");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .expect("the cells")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).expect("a batch");
            Batch::from_value(serde_json::from_str(&text).expect("JSON"))
                .unwrap_or_else(|why| panic!("{}: {why}", path.display()))
        })
        .collect()
}

/// A batch of `cells` (JSON values), at Full, for a test of the report.
#[cfg(test)]
pub(crate) fn a_batch(cells: serde_json::Value) -> Batch {
    Batch::from_value(serde_json::json!({
        "batch": "T", "protection": "full", "persona": "steward", "hosts": {}, "cells": cells,
    }))
    .expect("a batch")
}

/// **Production-only cells are skipped and reported on scaled runs** (T1.1-06): a scaled run
/// reports a cell with a hand `production_only` pair, and one that restarts with `host_killed`,
/// as `not_run`, skipped-for-timing, for each dialect, and never as passed; a cell without either
/// runs; a production run skips neither.
#[test]
fn production_only_cells_are_skipped_and_reported_on_scaled_runs() {
    let step = serde_json::json!({"calls": [{"tool": "bash", "input": {"command": "true"}}]});
    let batch = a_batch(serde_json::json!([
        {"id": "M-PLAIN", "steps": [step]},
        {"id": "M-PAIR", "production_only": ["hold", "hook_timeout"], "steps": [step]},
        {"id": "M-KILLED", "dialects": ["codex"],
         "steps": [{"restart": "host_killed", "calls": [{"tool": "bash", "input": {"command": "true"}}]}]},
    ]));

    let records = skipped_for_timing(&batch, TimingsMode::Scaled, Trigger::PullRequest);
    let mut rows: Vec<(String, String)> = records
        .iter()
        .map(|r| {
            (
                r.row.clone(),
                r.dialect.expect("a dialect").word().to_owned(),
            )
        })
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        [
            ("M-KILLED".to_owned(), "codex".to_owned()),
            ("M-PAIR".to_owned(), "claude".to_owned()),
            ("M-PAIR".to_owned(), "codex".to_owned()),
            ("M-PAIR".to_owned(), "opencode".to_owned()),
        ]
    );
    for record in &records {
        assert_eq!(record.verdict, ReportVerdict::NotRun, "{record:?}");
        assert_eq!(record.reason.as_deref(), Some(SKIPPED_FOR_TIMING));
        assert_eq!(record.timings, Some(TimingsMode::Scaled));
    }
    assert!(
        skipped_for_timing(&batch, TimingsMode::Production, Trigger::Nightly).is_empty(),
        "a production run skips neither"
    );

    // Written as a leg's report, one record a line, each read back in ticket 5's shape: to a
    // file of this test's own, never the leg's report a run uploads.
    let folder = tempfile::tempdir().expect("a folder");
    let at =
        write_to(&folder.path().join("linux-fake.jsonl"), &records).expect("the report is written");
    assert_ne!(at, report_path());
    let written: Vec<LegRecord> = std::fs::read_to_string(&at)
        .expect("the report")
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    for record in &records {
        assert!(
            written.contains(record),
            "{record:?} is in {}",
            at.display()
        );
    }
}

/// **A skipped cell's record says whether a refusal is expected of it** (#7175): an escape cell
/// and a cell with a step that expects a refusal carry `refusal_expected`, a plain cell does not.
#[test]
fn a_skipped_cells_record_says_whether_it_expects_a_refusal() {
    let call = serde_json::json!({"tool": "bash", "input": {"command": "true"}});
    let pair = serde_json::json!(["hold", "hook_timeout"]);
    let batch = a_batch(serde_json::json!([
        {"id": "M-PLAIN", "production_only": pair, "steps": [{"calls": [call]}]},
        {"id": "M-ESC", "escape": true, "production_only": pair, "steps": [{"calls": [call]}]},
        {"id": "M-REF", "production_only": pair,
         "steps": [{"calls": [call], "refusal_expected": true}]},
    ]));
    let records = skipped_for_timing(&batch, TimingsMode::Scaled, Trigger::PullRequest);
    for (row, expected) in [("M-PLAIN", false), ("M-ESC", true), ("M-REF", true)] {
        let ours: Vec<_> = records.iter().filter(|r| r.row == row).collect();
        assert_eq!(ours.len(), 3, "{row}: one a dialect");
        for r in ours {
            assert_eq!(r.refusal_expected, expected, "{row}");
            let value = serde_json::to_value(r).expect("a record serialises");
            assert_eq!(value.get("refusal_expected").is_some(), expected, "{row}");
        }
    }
}

/// **Every cell runs under `strict` unless it names a template** (T1.1-06, Q308): every cell in
/// `tests/scenarios/cells` that names none starts under `strict`, never the product's default
/// `watch`, a cell that names one starts under it, and each template's `[policy]` is written in
/// the project's manifest, `strict`'s too, and reads back as that template. That a chat's start
/// resolves it is `every_template_is_what_a_chat_starts_under`.
#[test]
fn every_cell_runs_under_strict_unless_it_names_a_template() {
    for batch in every_batch() {
        for cell in &batch.cells {
            let expected = cell
                .template
                .as_deref()
                .and_then(Template::parse)
                .unwrap_or(Template::Strict);
            assert_eq!(template_of(cell), expected, "{}", cell.id);
        }
    }
    let step = serde_json::json!({"calls": [{"tool": "bash", "input": {"command": "true"}}]});
    let batch = a_batch(serde_json::json!([
        {"id": "M-NONE", "steps": [step]},
        {"id": "M-WATCH", "template": "watch", "steps": [step]},
        {"id": "M-HOLD", "template": "hold", "steps": [step]},
        {"id": "M-OPEN", "template": "open", "steps": [step]},
    ]));
    assert_eq!(template_of(&batch.cells[0]), Template::Strict);
    assert_eq!(template_of(&batch.cells[1]), Template::Watch);
    assert_eq!(template_of(&batch.cells[2]), Template::Hold);
    assert_eq!(template_of(&batch.cells[3]), Template::Open);
    assert_eq!(
        DEFAULT_TEMPLATE,
        Template::Strict,
        "a scenario's own default"
    );
    assert_ne!(
        DEFAULT_TEMPLATE,
        Template::default(),
        "the product's default is not a scenario's"
    );
    for template in Template::ALL {
        let written =
            purlis_core::enforcement::Settings::from_text(Some(&super::host::policy_of(template)));
        assert_eq!(written.template, template, "{}", template.word());
        assert!(
            written.template_said,
            "{}: the template is said",
            template.word()
        );
    }
}

/// An event of the device's log with `kind`, as the log reads it back at `seq`.
#[cfg(test)]
fn an_event(seq: u64, kind: &str) -> purlis_core::eventlog::Event {
    serde_json::from_value(serde_json::json!({
        "v": 1, "device_id": "d", "seq": seq, "ulid": format!("U{seq}"), "chat": null,
        "run": null, "parent_run": null, "kind": kind, "body": {},
    }))
    .expect("an event")
}

/// **A cell that ran is reported with its event kinds in log order** (#7224, #7229): the record
/// is a cell's (an escape's when the cell is one) with its dialect, protection and timings, says
/// whether a refusal is expected of it, and holds the kinds the log wrote, ordered by `seq`
/// whatever order they were read in. `tools/live-pass diff` then compares them with a live
/// report's: equal sequences are one event comparison with no difference, and a differing one
/// names the first position.
#[test]
fn a_cell_that_ran_is_reported_with_its_event_kinds_in_log_order() {
    use purlis_scenarios::livepass::{Recorded, diff};
    use purlis_scenarios::model::{Dialect, Protection};

    let call = serde_json::json!({"tool": "bash", "input": {"command": "true"}});
    let batch = a_batch(serde_json::json!([
        {"id": "M-PLAIN", "steps": [{"calls": [call]}]},
        {"id": "M-ESC", "escape": true, "steps": [{"calls": [call]}]},
    ]));
    // The log holds the canary's and an earlier cell's events (seq 1 and 2) before this cell began
    // at seq 3: none of them is the cell's.
    let read_back = [
        an_event(5, "stop.block"),
        an_event(1, "canary.start"),
        an_event(2, "earlier.cell"),
        an_event(4, "hook.pretooluse"),
        an_event(6, "run.end"),
    ];
    let record = |cell: &Cell| {
        ran(
            cell,
            Dialect::Claude,
            Protection::Full,
            TimingsMode::Scaled,
            ReportVerdict::Pass,
            None,
            &read_back,
            3,
        )
    };

    let plain = record(&batch.cells[0]);
    assert_eq!(plain.leg.kind, EntryKind::Cell);
    assert_eq!(plain.leg.row, "M-PLAIN");
    assert!(!plain.leg.refusal_expected);
    assert_eq!(plain.leg.leg, ReportLeg::Fake);
    assert_eq!(plain.leg.timings, Some(TimingsMode::Scaled));
    assert_eq!(
        plain.events.as_deref(),
        Some(&["hook.pretooluse", "stop.block", "run.end"].map(str::to_owned)[..])
    );
    let escape = record(&batch.cells[1]);
    assert_eq!(escape.leg.kind, EntryKind::Escape);
    assert!(escape.leg.refusal_expected);

    // Written as a line a reader of the schema takes back, `events` among its keys.
    let folder = tempfile::tempdir().expect("a folder");
    let at = write_to(
        &folder.path().join("linux-fake.jsonl"),
        std::slice::from_ref(&plain),
    )
    .expect("the report is written");
    let line = std::fs::read_to_string(&at).expect("the report");
    let value: serde_json::Value = serde_json::from_str(line.trim()).expect("a line");
    assert_eq!(
        value["events"],
        serde_json::json!(["hook.pretooluse", "stop.block", "run.end"])
    );
    assert_eq!(
        purlis_scenarios::livepass::read_report("linux-fake.jsonl", &line).expect("a valid report"),
        [plain.clone()]
    );

    // The diff compares them.
    let live = |kinds: &[&str]| Recorded {
        leg: LegRecord {
            leg: ReportLeg::Live,
            ..plain.leg.clone()
        },
        events: Some(kinds.iter().map(|k| (*k).to_owned()).collect()),
    };
    let same = diff(
        &[live(&["hook.pretooluse", "stop.block", "run.end"])],
        std::slice::from_ref(&plain),
    );
    assert_eq!(same.events_compared, 1);
    assert!(same.events.is_empty(), "{same:?}");
    let differ = diff(
        &[live(&["hook.pretooluse", "run.end"])],
        std::slice::from_ref(&plain),
    );
    assert_eq!(differ.events_compared, 1);
    assert_eq!(differ.events.len(), 1);
    assert_eq!(differ.events[0].at, 1);
    assert!(differ.events_one_side.is_empty());
}
