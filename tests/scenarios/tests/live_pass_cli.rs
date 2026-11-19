// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`tools/live-pass`'s command line** (T1.1-37, #6884): its exit codes (0 agree, 1 differ or
//! nothing compared, 2 a wrong file or argument), the options, and the workflow
//! `docs/updating.md` step 1b gives, end to end on chat 682's B0 log and a suite report in the
//! form the scenario host writes (`LegRecord`, no events).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use purlis_scenarios::model::{Batch, Dialect, LegRecord, ReportLeg, ReportVerdict, TimingsMode};
use purlis_scenarios::root;
use serde_json::{Value, json};

fn live_pass(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_live-pass"))
        .args(args)
        .output()
        .expect("the live-pass binary runs")
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("an exit code")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn b0_path() -> PathBuf {
    root().join("cells/B0.json")
}

fn chat_682_path() -> PathBuf {
    root().join("tests/fixtures/chat-682-b0.json")
}

/// `report` on chat 682's log, written to `dir/live.jsonl`.
fn live_report_file(dir: &Path) -> PathBuf {
    let out = live_pass(&[
        "report",
        "--cells",
        p(&b0_path()),
        "--log",
        p(&chat_682_path()),
        "--harness",
        "claude",
        "--os",
        "macos",
    ]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let path = dir.join("live.jsonl");
    fs::write(&path, &out.stdout).unwrap();
    path
}

/// A suite report as `app/src-tauri/src/scenarios/report.rs` writes it: one serialized
/// `LegRecord` a line, leg `fake`, no `timings` unless the run had them, no events. Its verdicts
/// are the live report's, so it agrees; `edit` changes the records first.
fn suite_report_file(
    dir: &Path,
    live: &Path,
    leg: ReportLeg,
    timings: Option<TimingsMode>,
    edit: impl Fn(&mut Vec<LegRecord>),
) -> PathBuf {
    let mut records: Vec<LegRecord> = fs::read_to_string(live)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for r in &mut records {
        r.leg = leg;
        r.timings = timings;
        r.reason = r.reason.take().filter(|_| r.verdict != ReportVerdict::Pass);
    }
    edit(&mut records);
    let path = dir.join(format!("suite-{leg:?}-{timings:?}.jsonl"));
    let lines: Vec<String> = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

#[test]
fn a_live_report_that_agrees_with_the_suite_report_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |_| {});
    let out = live_pass(&["diff", p(&live), p(&suite)]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("compared 7 cells"), "{}", text(&out));
}

#[test]
fn a_verdict_that_differs_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |rs| {
        rs[0].verdict = ReportVerdict::Pass;
        rs[0].reason = None;
    });
    let out = live_pass(&["diff", p(&live), p(&suite)]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(text(&out).contains("verdict"), "{}", text(&out));
}

#[test]
fn a_diff_that_compared_nothing_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    // a suite report of other cells entirely: scoped to the live report's, it is empty
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |rs| {
        for r in rs.iter_mut() {
            r.row = format!("{}-other", r.row);
        }
    });
    let out = live_pass(&["diff", p(&live), p(&suite)]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(
        text(&out).contains("nothing was compared"),
        "{}",
        text(&out)
    );
}

#[test]
fn the_suite_report_of_every_other_cell_prints_no_only_suite_line() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |rs| {
        let mut more = rs.clone();
        for r in &mut more {
            r.row = format!("{}-elsewhere", r.row);
        }
        rs.extend(more);
        let mut codex = rs[0].clone();
        codex.dialect = Some(Dialect::Codex);
        rs.push(codex);
    });
    let out = live_pass(&["diff", p(&live), p(&suite)]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(!text(&out).contains("only suite"), "{}", text(&out));
}

#[test]
fn a_left_file_that_is_not_the_live_leg_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |_| {});
    let out = live_pass(&["diff", p(&suite), p(&suite)]);
    assert_eq!(code(&out), 2, "{}", text(&out));
}

#[test]
fn a_report_that_fails_the_schema_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let bad = dir.path().join("bad.jsonl");
    fs::write(&bad, "{\"kind\":\"cell\",\"row\":\"M-PRB-01\"}\n").unwrap();
    assert_eq!(code(&live_pass(&["diff", p(&live), p(&bad)])), 2);
    assert_eq!(code(&live_pass(&["diff", p(&bad), p(&live)])), 2);
    assert_eq!(
        code(&live_pass(&["diff", p(&live), "no-such-file.jsonl"])),
        2
    );
}

#[test]
fn suite_leg_keeps_only_that_leg() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let fake = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |_| {});
    // the same file with a second leg's records that disagree
    let mut text_ = fs::read_to_string(&fake).unwrap();
    let other = suite_report_file(dir.path(), &live, ReportLeg::RealWrap, None, |rs| {
        for r in rs.iter_mut() {
            r.verdict = ReportVerdict::Fail;
            r.reason = Some("x".into());
        }
    });
    text_.push_str(&fs::read_to_string(&other).unwrap());
    let both = dir.path().join("both.jsonl");
    fs::write(&both, text_).unwrap();
    assert_eq!(code(&live_pass(&["diff", p(&live), p(&both)])), 1);
    let out = live_pass(&["diff", p(&live), p(&both), "--suite-leg", "fake"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert_eq!(
        code(&live_pass(&[
            "diff",
            p(&live),
            p(&both),
            "--suite-leg",
            "nope"
        ])),
        2
    );
    assert_eq!(
        code(&live_pass(&["diff", p(&live), p(&both), "--suite-leg"])),
        2
    );
}

#[test]
fn suite_timings_keeps_only_that_mode_and_a_record_with_none_is_scaled() {
    let dir = tempfile::tempdir().unwrap();
    let live = live_report_file(dir.path());
    let none = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |_| {});
    let production = suite_report_file(
        dir.path(),
        &live,
        ReportLeg::Fake,
        Some(TimingsMode::Production),
        |_| {},
    );
    for (suite, mode, expect) in [
        (&none, "scaled", 0),
        (&none, "production", 1),
        (&production, "production", 0),
        (&production, "scaled", 1),
    ] {
        let out = live_pass(&["diff", p(&live), p(suite), "--suite-timings", mode]);
        assert_eq!(code(&out), expect, "{mode}: {}", text(&out));
    }
    // a value is required, and it is one of two words
    assert_eq!(
        code(&live_pass(&["diff", p(&live), p(&none), "--suite-timings"])),
        2
    );
    assert_eq!(
        code(&live_pass(&[
            "diff",
            p(&live),
            p(&none),
            "--suite-timings",
            "fast"
        ])),
        2
    );
}

#[test]
fn report_exits_2_on_a_malformed_log_and_takes_every_cells_file() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.json");
    fs::write(&bad, r#"{"cells": {"M-PRB-01": {}}}"#).unwrap();
    let out = live_pass(&[
        "report",
        "--cells",
        p(&b0_path()),
        "--log",
        p(&bad),
        "--harness",
        "claude",
        "--os",
        "macos",
    ]);
    assert_eq!(code(&out), 2, "{}", text(&out));

    // a second cells file, and a log with a cell of each
    let second = dir.path().join("T0.json");
    let batch: Value = json!({
        "batch": "T0", "protection": "full", "persona": "steward", "hosts": {"h1": "one.test"},
        "cells": [{"id": "live-a", "dialects": ["claude"], "live_only": {"reason": "model_behaviour"},
            "steps": [{"calls": [{"tool": "Bash", "input": {"command": "get https://$h1 3"}}],
                       "require": [{"kind": "stop.block", "source": "sandbox"}]}]}],
    });
    Batch::from_value(batch.clone()).expect("a batch");
    fs::write(&second, batch.to_string()).unwrap();
    let log = dir.path().join("log.json");
    fs::write(
        &log,
        json!({"cells": {"live-a": {"failed": [], "globals": []},
                          "M-PRB-01": {"failed": [], "globals": []}}})
        .to_string(),
    )
    .unwrap();
    let args = |cells: &[&str]| {
        let mut a = vec!["report"];
        for c in cells {
            a.extend(["--cells", c]);
        }
        a.extend(["--log", p(&log), "--harness", "claude", "--os", "macos"]);
        a.into_iter().map(str::to_owned).collect::<Vec<_>>()
    };
    let run = |cells: &[&str]| {
        let a = args(cells);
        live_pass(&a.iter().map(String::as_str).collect::<Vec<_>>())
    };
    let both = run(&[p(&b0_path()), p(&second)]);
    assert_eq!(code(&both), 0, "{}", text(&both));
    let out = String::from_utf8_lossy(&both.stdout).into_owned();
    assert!(out.contains("live-a") && out.contains("M-PRB-01"), "{out}");
    // with only one of the two files, the log names a cell nobody gave
    assert_eq!(code(&run(&[p(&b0_path())])), 2);
    // no cells file at all
    assert_eq!(
        code(&live_pass(&[
            "report",
            "--log",
            p(&log),
            "--harness",
            "claude",
            "--os",
            "macos"
        ])),
        2
    );
}

#[test]
fn a_command_or_option_it_does_not_have_exits_2() {
    assert_eq!(code(&live_pass(&[])), 2);
    assert_eq!(code(&live_pass(&["judge"])), 2);
    assert_eq!(code(&live_pass(&["diff", "a.jsonl"])), 2);
    assert_eq!(
        code(&live_pass(&[
            "diff",
            "a.jsonl",
            "b.jsonl",
            "--suite-leg=fake"
        ])),
        2
    );
    assert_eq!(code(&live_pass(&["brief", "--harness", "claude"])), 2);
    assert_eq!(
        code(&live_pass(&[
            "brief",
            "--harness",
            "claude",
            "--os",
            "plan9"
        ])),
        2
    );
}

#[test]
fn brief_writes_the_log_format_for_the_real_files() {
    let out = live_pass(&["brief", "--harness", "claude", "--os", "macos"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(text(&out).contains("\"globals\""));
    let json = live_pass(&["brief", "--harness", "codex", "--os", "linux", "--json"]);
    assert_eq!(code(&json), 0, "{}", text(&json));
    let v: Value = serde_json::from_slice(&json.stdout).expect("JSON");
    assert!(v["checks"].is_array());
}

#[test]
fn a_live_fail_exits_1_even_when_the_suite_fails_the_same_cell() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log.json");
    let mut value: Value =
        serde_json::from_str(&fs::read_to_string(chat_682_path()).unwrap()).unwrap();
    value["cells"]["M-PRB-03"]["failed"] = json!([["require[0]", 2], ["require[1]", 7]]);
    fs::write(&log, value.to_string()).unwrap();
    let out = live_pass(&[
        "report",
        "--cells",
        p(&b0_path()),
        "--log",
        p(&log),
        "--harness",
        "claude",
        "--os",
        "macos",
    ]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let live = dir.path().join("live.jsonl");
    fs::write(&live, &out.stdout).unwrap();
    // a suite report with the very same verdicts, fail included
    let suite = suite_report_file(dir.path(), &live, ReportLeg::Fake, None, |_| {});
    let out = live_pass(&["diff", p(&live), p(&suite)]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(
        text(&out).contains("FAIL") && text(&out).contains("M-PRB-03"),
        "{}",
        text(&out)
    );
}

#[test]
fn a_stray_or_misspelt_argument_exits_2_and_drops_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let b0 = p(&b0_path()).to_owned();
    let log = p(&chat_682_path()).to_owned();
    let base = |extra: &[&str]| {
        let mut a = vec!["report", "--cells", b0.as_str()];
        a.extend(extra);
        a.extend([
            "--log",
            log.as_str(),
            "--harness",
            "claude",
            "--os",
            "macos",
        ]);
        live_pass(&a)
    };
    assert_eq!(code(&base(&[])), 0);
    // `--cells a b`: the second file is no one's value
    let two = base(&[&b0]);
    assert_eq!(code(&two), 2, "{}", text(&two));
    assert!(text(&two).contains("unexpected argument"), "{}", text(&two));
    // a misspelt option
    let typo = base(&["--celss", &b0]);
    assert_eq!(code(&typo), 2, "{}", text(&typo));
    assert!(text(&typo).contains("--celss"), "{}", text(&typo));
    // brief and diff refuse them too
    for args in [
        vec!["brief", "--harness", "claude", "--os", "macos", "extra"],
        vec!["brief", "--harness", "claude", "--os", "macos", "--jsn"],
        vec!["diff", "a.jsonl", "b.jsonl", "c.jsonl"],
    ] {
        assert_eq!(code(&live_pass(&args)), 2, "{args:?}");
    }
    let _ = dir;
}

#[test]
fn a_cell_given_twice_in_the_log_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log.json");
    fs::write(
        &log,
        r#"{"cells": {"M-PRB-01": {"failed": [], "globals": []}, "M-PRB-01": {"failed": [], "globals": []}}}"#,
    )
    .unwrap();
    let out = live_pass(&[
        "report",
        "--cells",
        p(&b0_path()),
        "--log",
        p(&log),
        "--harness",
        "claude",
        "--os",
        "macos",
    ]);
    assert_eq!(code(&out), 2, "{}", text(&out));
    assert!(text(&out).contains("twice"), "{}", text(&out));
}
