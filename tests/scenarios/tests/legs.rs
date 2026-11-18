// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's reports, read back for the run-result gate step** (T1.1-54): the JSON lines the legs
//! wrote, the legs a run ran, and the trigger that started it.

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
