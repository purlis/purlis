// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a skill's grades (SK-16, #2779): the plan before a grade, and the eval record
//! after it, on a temporary plane that is a real git repository.

use std::path::Path;

use purlis_skills::grade::{ArmScore, GradeEnd, Stat, TriggerScore};

use super::*;
use crate::testgit::run;

const SKILL: &str = "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nStep one.\n";

const TRIGGERS: &str = r#"[
    {"query": "ship it to production", "should_trigger": true},
    {"query": "what is production", "should_trigger": false}
]"#;

/// A plane whose project skill `deploy` has two trigger tests and one outcome eval, committed.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
    std::fs::write(root.join("skills/deploy/SKILL.md"), SKILL).unwrap();
    std::fs::write(root.join("skills/deploy/evals/triggers.json"), TRIGGERS).unwrap();
    std::fs::write(
        root.join("skills/deploy/evals/evals.json"),
        r#"{"skill_name": "deploy", "evals": [{"id": 1, "prompt": "ship v2"}]}"#,
    )
    .unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "make deploy"]);
    tmp
}

fn report(passed_with: bool) -> GradeReport {
    let stat = |mean: f64| Stat {
        mean,
        stddev: 0.0,
        min: mean,
        max: mean,
    };
    GradeReport {
        grader: grade::BUILT_IN.to_owned(),
        triggers: vec![TriggerScore {
            query: "ship it to production".into(),
            should_trigger: true,
            arm: Arm::With,
            runs: 3,
            triggered: if passed_with { 3 } else { 0 },
            rate: Some(if passed_with { 1.0 } else { 0.0 }),
            pass: Some(passed_with),
            chats: vec![Some("01A".into()), Some("01B".into()), Some("01C".into())],
        }],
        arms: vec![ArmScore {
            arm: Arm::With,
            pass_rate: Some(stat(1.0)),
            tokens: None,
            seconds: Some(stat(4.0)),
            chats: 3,
            not_run: 0,
        }],
        delta: Some(0.5),
        ended: GradeEnd::Whole,
        not_started: None,
    }
}

fn graded(root: &Path, id: &str, ts: &str, spent: Spent) -> Graded {
    let level = Level::project();
    let (content, commit) = version_of(root, &level, "deploy");
    Graded {
        v: VERSION,
        grade: id.to_owned(),
        skill: "deploy".into(),
        level: level_word(root, &level),
        tree: content.word().to_owned(),
        commit,
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: ts.to_owned(),
        runs: 3,
        report: report(true),
        spent,
    }
}

#[test]
fn the_plan_comes_before_anything_runs_and_says_what_waits() {
    let plane = plane();
    let planned = planned(plane.path(), &Level::project(), "deploy").unwrap();
    let plan = planned.plan.unwrap();
    assert_eq!(plan.chats, 12, "two tests, two arms, three runs");
    assert_eq!(plan.outcome_evals_waiting, 1);
    assert_eq!(planned.request.unwrap().arms, ARMS.to_vec());
    assert_eq!(
        planned.estimate, None,
        "nothing is estimated that was never measured"
    );
    assert!(planned.unseen.is_empty());
}

#[test]
fn a_grade_is_recorded_against_the_version_its_folder_holds() {
    let plane = plane();
    let root = plane.path();
    let (content, commit) = version_of(root, &Level::project(), "deploy");
    assert!(matches!(content, Content::Tree(_)), "{content:?}");
    assert_eq!(commit.as_deref().map(str::len), Some(40));

    let written = record(
        root,
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    )
    .unwrap();
    assert!(
        written.starts_with(root.join("personas/_grades")),
        "{}",
        written.display()
    );
    let read = grades_of(root, "deploy", Some(&Level::project()));
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].commit, commit);
    assert_eq!(read[0].report, report(true));

    std::fs::write(root.join("skills/deploy/SKILL.md"), format!("{SKILL}More.\n")).unwrap();
    let (changed, none) = version_of(root, &Level::project(), "deploy");
    assert_eq!((changed, none), (Content::Uncommitted, None));
}

#[test]
fn the_record_is_read_newest_first_and_a_row_it_cannot_take_is_skipped() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &graded(root, "old", "2026-10-01T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    record(
        root,
        &graded(root, "new", "2026-10-25T10:00:00+00:00", Spent::default()),
        "desk",
    );
    let file = path_for(root, chrono::Utc::now(), "laptop");
    let mut newer = serde_json::to_value(graded(root, "later", "2026-10-26T00:00:00+00:00", Spent::default())).unwrap();
    newer["v"] = (VERSION + 1).into();
    let mut other = serde_json::to_value(graded(root, "other", "2026-10-27T00:00:00+00:00", Spent::default())).unwrap();
    other["skill"] = "release".into();
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("{not json\n");
    text.push_str(&format!("{newer}\n{other}\n"));
    text.push_str(&format!("\"{}\"\n", "a".repeat(MOST_LINE_BYTES)));
    std::fs::write(&file, text).unwrap();

    let ids: Vec<String> = grades_of(root, "deploy", None)
        .into_iter()
        .map(|one| one.grade)
        .collect();
    assert_eq!(ids, vec!["new".to_owned(), "old".to_owned()]);
}

#[cfg(unix)]
#[test]
fn a_record_file_that_is_a_link_is_never_read() {
    let plane = plane();
    let root = plane.path();
    let elsewhere = tempfile::tempdir().unwrap();
    let target = elsewhere.path().join("grades.jsonl");
    let row = serde_json::to_value(graded(root, "linked", "2026-10-25T10:00:00+00:00", Spent::default())).unwrap();
    std::fs::write(&target, format!("{row}\n")).unwrap();
    std::fs::create_dir_all(root.join("personas/_grades")).unwrap();
    std::os::unix::fs::symlink(&target, root.join("personas/_grades/2026-10.x.jsonl")).unwrap();
    assert!(grades_of(root, "deploy", None).is_empty());
}

#[test]
fn the_estimate_is_the_last_grades_spend_per_chat_times_the_chats() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &graded(
            root,
            "g1",
            "2026-10-25T10:00:00+00:00",
            Spent {
                chats: 6,
                tokens: Some(60_000),
                cost_usd: Some(0.6),
            },
        ),
        "laptop",
    );
    let estimate = planned(root, &Level::project(), "deploy")
        .unwrap()
        .estimate
        .unwrap();
    assert_eq!(estimate.tokens, Some(120_000));
    assert!((estimate.cost_usd.unwrap() - 1.2).abs() < 1e-9);
    assert_eq!(estimate.chats_then, 6);
}

#[test]
fn a_query_that_hides_characters_is_marked_in_the_plan() {
    let plane = plane();
    let root = plane.path();
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        "[{\"query\": \"ship it\u{202e}noitcudorp\", \"should_trigger\": true}]",
    )
    .unwrap();
    let planned = planned(root, &Level::project(), "deploy").unwrap();
    assert_eq!(planned.unseen, vec![0]);
}

#[test]
fn a_skill_with_no_trigger_tests_has_no_plan() {
    let plane = plane();
    let root = plane.path();
    std::fs::remove_file(root.join("skills/deploy/evals/triggers.json")).unwrap();
    let planned = planned(root, &Level::project(), "deploy").unwrap();
    assert_eq!(planned.plan, None);
    assert_eq!(planned.fixtures.triggers, grade::Read::Absent);
    assert!(planned.fixtures.outcomes.read().is_some());
}

#[test]
fn a_skills_history_shows_each_versions_newest_grade() {
    let plane = plane();
    let root = plane.path();
    let before = super::super::history::history(root, &Level::project(), "deploy").unwrap();
    assert_eq!(before.versions[0].grade, None);
    record(
        root,
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    let after = super::super::history::history(root, &Level::project(), "deploy").unwrap();
    assert_eq!(
        after.versions[0].grade,
        Some(super::super::history::Grade {
            passed: 1,
            fixtures: 1
        })
    );
}
