// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn doctor(root: &std::path::Path) -> Doctor {
    std::fs::write(root.join(crate::plane::MANIFEST), "schema = 1\n").unwrap();
    Doctor::at(root, root, true, false)
}

#[test]
fn a_project_with_a_clean_log_or_none_has_no_row() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(work_links(&doctor(tmp.path())).is_none());
}

#[test]
fn a_skipped_line_warns_and_a_cycle_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = log::dir_for(tmp.path(), "alpha");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("01K6H0Z8Y3V1N3G4QK0A9T5B7C.jsonl");
    std::fs::write(&file, "{\"v\":1}\n").unwrap();
    let row = work_links(&doctor(tmp.path())).expect("a row");
    assert_eq!(row.status, super::super::Status::Warn);
    assert_eq!(
        row.detail,
        "1 line skipped, not one of the four closed key sets: 1 in \
         workspaces/alpha/work/01K6H0Z8Y3V1N3G4QK0A9T5B7C.jsonl"
    );

    let a = "todo:alpha/x";
    let b = "github:github.com/o/r#1";
    let line = |from: &str, to: &str| {
        format!(
            "{{\"v\":1,\"ts\":\"2026-10-02T08:00:00Z\",\"op\":\"alias\",\"from\":\"{from}\",\
             \"to\":\"{to}\",\"cause\":\"moved\"}}\n"
        )
    };
    std::fs::write(&file, format!("{}{}", line(a, b), line(b, a))).unwrap();
    let row = work_links(&doctor(tmp.path())).expect("a row");
    assert_eq!(row.status, super::super::Status::Fail);
    assert!(
        row.detail.contains(a) && row.detail.contains(b),
        "{}",
        row.detail
    );
}

#[test]
fn a_todo_promoted_but_left_open_is_reported_with_where_it_went() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = crate::workspaces::Plane::open(tmp.path())
        .workspace("alpha")
        .unwrap();
    let stamp = chrono::NaiveDate::from_ymd_opt(2026, 10, 2)
        .unwrap()
        .and_hms_opt(8, 0, 0)
        .unwrap();
    let path = ws.add_todo("Port the picker", stamp).unwrap();
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let d = doctor(tmp.path());
    log::append_alias(
        tmp.path(),
        "alpha",
        "01K6H0Z8Y3V1N3G4QK0A9T5B7C",
        stamp.and_utc(),
        crate::work::TrackerKey::todo("alpha", &stem).unwrap(),
        crate::work::TrackerKey::parse("github:github.com/o/r#1").unwrap(),
        log::Cause::Promoted,
    )
    .unwrap();
    let row = work_links(&d).expect("a row");
    assert_eq!(row.status, super::super::Status::Warn);
    assert_eq!(
        row.detail,
        "promoted but still open: alpha: 'Port the picker' → github:github.com/o/r#1"
    );
    assert!(row.hint.contains("purlis ws todo -w alpha"), "{}", row.hint);
}
