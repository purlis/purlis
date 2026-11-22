// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's reports, read back for the run-result gate step** (T1.1-54): the JSON lines the legs
//! wrote, the legs a run ran, and the trigger that started it.

use std::path::Path;

use purlis_scenarios::coverage::Ran;
use purlis_scenarios::legs::{ran_from, read_reports, trigger_from};
use purlis_scenarios::model::{EntryKind, Os, ReportLeg, ReportVerdict, TimingsMode};
use purlis_scenarios::plan::Trigger;

#[test]
fn a_leg_report_is_read_back_and_a_timings_file_beside_it_is_not() {
    let folder = tempfile::tempdir().expect("a folder");
    std::fs::write(
        folder.path().join("linux-fake.jsonl"),
        concat!(
            r#"{"kind":"cell","row":"M-A-01","dialect":"claude","protection":"full","os":"linux","leg":"fake","timings":"scaled","verdict":"pass"}"#,
            "\n\n",
            r#"{"kind":"fixture","row":"M-WAY-04","test":"a_race","os":"linux","leg":"fixture","verdict":"fail"}"#,
            "\n",
        ),
    )
    .expect("a report");
    // Each scenario's run time, in another shape: not a record.
    std::fs::write(
        folder.path().join("linux-timings.jsonl"),
        "{\"scenario\":\"a\",\"seconds\":3.2}\n",
    )
    .expect("timings");
    let records = read_reports(folder.path()).expect("the reports read");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind, EntryKind::Cell);
    assert_eq!(records[1].verdict, ReportVerdict::Fail);
}

#[test]
fn a_report_that_cannot_be_read_is_an_error_never_a_pass() {
    let folder = tempfile::tempdir().expect("a folder");
    std::fs::write(
        folder.path().join("linux-fake.jsonl"),
        "{\"kind\":\"cell\"}\n",
    )
    .expect("report");
    let why = read_reports(folder.path()).expect_err("an invalid record");
    assert!(why.contains("linux-fake.jsonl:1"), "{why}");
    let why = read_reports(&folder.path().join("missing")).expect_err("no folder");
    assert!(why.contains("missing"), "{why}");
}

#[test]
fn the_legs_a_run_ran_and_its_trigger_are_read_from_words() {
    assert_eq!(
        ran_from("fake:linux:scaled, fixture:macos:production").expect("legs"),
        vec![
            Ran {
                leg: ReportLeg::Fake,
                os: Os::Linux,
                timings: TimingsMode::Scaled
            },
            Ran {
                leg: ReportLeg::Fixture,
                os: Os::Macos,
                timings: TimingsMode::Production
            },
        ]
    );
    assert!(ran_from("fake:linux").is_err());
    assert!(ran_from("fake:windows:scaled").is_err());
    assert_eq!(trigger_from("merge_group"), Ok(Trigger::MergeQueue));
    assert_eq!(trigger_from("nightly"), Ok(Trigger::Nightly));
    assert!(trigger_from("push").is_err());
}

#[test]
fn the_run_result_gate_program_reads_a_job_s_reports_and_says_why_it_is_red() {
    let gate = |reports: &std::path::Path, trigger: &str, ran: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_scenarios-gate"))
            .args([reports.to_str().expect("a path"), trigger, ran])
            .output()
            .expect("the gate runs")
    };
    let folder = tempfile::tempdir().expect("a folder");
    // B0's linux rows are covered by scenario cells (#7322), so a job that ran the fake leg on
    // Linux and reported nothing is red, and says which rows did not run.
    let rows = [
        "M-PRB-01",
        "M-PRB-02",
        "M-PRB-03",
        "M-PRB-04",
        "M-PRB-04b",
        "M-PRB-05",
        "M-PRB-06",
    ];
    let silent = gate(folder.path(), "pull_request", "fake:linux:scaled");
    assert!(!silent.status.success(), "{silent:?}");
    let said = String::from_utf8_lossy(&silent.stdout).into_owned();
    for row in rows {
        assert!(
            said.contains(&format!(
                "{row} claude linux: did not run in the fake leg on linux"
            )),
            "{row}: {said}"
        );
    }
    // A job that ran only fixtures owes none of them: they are another pass's.
    let fixtures = gate(folder.path(), "pull_request", "fixture:linux:scaled");
    assert!(fixtures.status.success(), "{fixtures:?}");
    // A pass for each row, as the runner writes one, and the gate is green.
    let report: String = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "kind": "cell", "row": row, "dialect": "claude", "protection": "full",
                "os": "linux", "leg": "fake", "timings": "scaled", "verdict": "pass",
            })
            .to_string()
                + "\n"
        })
        .collect();
    std::fs::write(folder.path().join("linux-fake.jsonl"), report).expect("the report");
    let green = gate(folder.path(), "pull_request", "fake:linux:scaled");
    assert!(green.status.success(), "{green:?}");
    // A report folder that is not there, a trigger or a leg it does not know: red, with the reason.
    for (args, said) in [
        (
            ("missing-folder", "pull_request", "fake:linux:scaled"),
            "missing-folder",
        ),
        (
            (
                folder.path().to_str().expect("a path"),
                "push",
                "fake:linux:scaled",
            ),
            "no trigger",
        ),
        (
            (
                folder.path().to_str().expect("a path"),
                "nightly",
                "fake:linux",
            ),
            "leg:os:timings",
        ),
    ] {
        let red = gate(Path::new(args.0), args.1, args.2);
        assert!(!red.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&red.stdout).contains(said),
            "{args:?}: {red:?}"
        );
    }
}
