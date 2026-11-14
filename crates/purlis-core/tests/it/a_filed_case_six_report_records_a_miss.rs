// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A filed case-6 Report records a miss** (#5739, spec #5714): when the person files a Report
//! draft that says a purlis skill should have started and did not (case 6 of #5683, whose
//! draft names the skill, the harness and the words, #5698), purlis logs one `skill.missed` row
//! in the project's skill log with signal `report`, naming the skill and the harness the Report
//! names. Dismissing the draft logs none, and neither does any other Report. The miss is counted
//! in the built-in's row by harness, beside its runs, as a correction's is (#5738).
//!
//! Every case drives the Reports module through its interface, filing to the Report intake over
//! the recording `a_chats_report_draft_is_filed_from_the_inbox` uses.

use std::path::Path;

use purlis_core::forge::Caller;
use purlis_core::report::{AgentDraft, CameFrom, Evidence, Filing, Held, How, Kind, Reports};
use purlis_core::skilluse::{self, By, Group, Signal};

use crate::a_chats_report_draft_is_filed_from_the_inbox::{recorded_intake, reports};

const CASE_SIX: &str = "purlis-secrets did not start when I was asked for a token\n\n\
     The person asked for their API token and the chat asked them to paste it.\n\n\
     Skill: purlis-secrets\n\
     Harness: Claude Code 2.1.40\n\
     Words: \"use my deploy token for this\"\n";

/// A project the skill log may be written in.
fn project() -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join(purlis_core::plane::MANIFEST),
        "schema = 2\n",
    )
    .unwrap();
    plane
}

fn held(reports: &Reports, project: &Path, text: &str) -> Held {
    reports
        .hold(
            Evidence::AgentDraft(AgentDraft {
                kind: Kind::Bug,
                text: text.to_owned(),
                title: None,
                call: None,
                ran_as: purlis_core::report::RanAs::default(),
            }),
            &Caller::chat("4"),
            CameFrom {
                session: Some(4),
                chat: "steward 4".into(),
                project: project.display().to_string(),
            },
        )
        .expect("held")
}

fn filed(reports: &Reports, one: &Held) {
    let filing = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    assert!(
        matches!(filing, Filing::Sent { .. }),
        "not sent: {filing:?}"
    );
}

#[test]
fn filing_a_case_six_draft_logs_one_report_miss_of_the_skill_and_harness_it_names() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let plane = project();
    let reports = reports(config.path(), Some(true), &recorded_intake());
    let one = held(&reports, plane.path(), CASE_SIX);

    filed(&reports, &one);

    let misses = skilluse::misses(plane.path());
    let [miss] = misses.as_slice() else {
        panic!("one miss: {misses:#?}");
    };
    assert_eq!(miss.skill, "purlis-secrets");
    assert_eq!(miss.signal, Signal::Report);
    assert_eq!(miss.harness.as_deref(), Some("claude"));
    assert_eq!(miss.harness_version.as_deref(), Some("2.1.40"));
    assert_eq!(miss.persona, None, "a Report names no persona");
}

#[test]
fn a_case_six_draft_the_person_edited_logs_the_skill_and_harness_its_filed_words_name() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let plane = project();
    let reports = reports(config.path(), Some(true), &recorded_intake());
    let one = held(&reports, plane.path(), CASE_SIX);
    let one = reports
        .edit_held(
            &one.id,
            "purlis-handoff did not start",
            "Skill: purlis:handoff\nHarness: codex\nWords: \"open a chat for this\"",
        )
        .expect("edited");

    filed(&reports, &one);

    let misses = skilluse::misses(plane.path());
    let [miss] = misses.as_slice() else {
        panic!("one miss: {misses:#?}");
    };
    assert_eq!(
        (miss.skill.as_str(), miss.harness.as_deref()),
        ("purlis-handoff", Some("codex")),
        "the words that were filed, by the name the skill has now"
    );
    assert_eq!(miss.harness_version, None);
}

#[test]
fn dismissing_a_case_six_draft_logs_no_miss() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let plane = project();
    let reports = reports(config.path(), Some(true), &recorded_intake());
    let one = held(&reports, plane.path(), CASE_SIX);

    reports.dismiss_held(&one.id);

    assert_eq!(skilluse::misses(plane.path()), []);
}

#[test]
fn filing_a_report_of_another_case_logs_no_miss() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let plane = project();
    let reports = reports(config.path(), Some(true), &recorded_intake());
    let one = held(
        &reports,
        plane.path(),
        "purlis worktree update refuses a clean branch\n\nIt said the branch had uncommitted \
         changes. It had none. Harness: Claude Code",
    );

    filed(&reports, &one);

    assert_eq!(skilluse::misses(plane.path()), []);
}

#[test]
fn a_filed_case_six_miss_is_counted_in_the_built_ins_row_by_harness() {
    purlis_core::unsteered!();
    let config = tempfile::tempdir().unwrap();
    let plane = project();
    let reports = reports(config.path(), Some(true), &recorded_intake());
    let one = held(&reports, plane.path(), CASE_SIX);

    filed(&reports, &one);

    let misses: Vec<_> = skilluse::misses(plane.path())
        .into_iter()
        .filter(|miss| miss.is_of("purlis-secrets"))
        .collect();
    let rows: Vec<(Group, u64, u64)> = skilluse::stats_with_misses(&[], &misses, By::Harness)
        .into_iter()
        .map(|row| {
            (
                row.group.harness_titled(),
                row.totals.runs,
                row.totals.misses,
            )
        })
        .collect();
    assert_eq!(rows, [(Group::Named("Claude Code".into()), 0, 1)]);
}
