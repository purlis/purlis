// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's half of grading several skills (#5356): what the estimate says before anything
//! runs, and what the person is told as the grades run.

use super::*;
use crate::skillgrade::tests::planned;
use crate::skillgrade::{GradeEval, On};

fn project() -> SkillLevel {
    SkillLevel::Project { local: false }
}

/// A plan of `name` as Grade would show it, its last grade worked out at `cost_usd`.
fn plan(
    name: &str,
    cost_usd: Option<Option<f64>>,
) -> (SkillLevel, String, Result<SkillGradePlan, String>) {
    let mut planned = planned("ship it");
    planned.skill = name.to_owned();
    let mut plan = SkillGradePlan::of(
        &planned,
        None,
        On {
            profile: Some("work".into()),
            checked: Ok("full sandbox".into()),
            priced: grades::price::of_model(None),
            models: Vec::new(),
            model: None,
            evals_refused: None,
        },
    );
    plan.estimate = cost_usd.map(|cost_usd| GradeEstimate {
        tokens: cost_usd.map(|_| 1_000.0),
        cost_usd,
        chats_then: 6,
        ts: "2026-11-01T10:00:00Z".into(),
        profile: Some("work".into()),
        harness: Some("claude".into()),
        model: None,
        same_profile: Some(true),
        interrupted: false,
    });
    (project(), name.to_owned(), Ok(plan))
}

#[test]
fn the_estimate_adds_each_last_grade_and_names_those_with_none_to_go_on() {
    let several = SkillGradesPlan::of(vec![
        plan("deploy", Some(Some(0.40))),
        plan("lint", None),
        plan("review", Some(None)),
        plan("ship", Some(Some(0.35))),
    ]);
    let names: Vec<_> = several.skills.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["deploy", "lint", "review", "ship"]);
    assert!(
        (several.known_usd - 0.75).abs() < 1e-9,
        "{}",
        several.known_usd
    );
    assert_eq!(several.known, 2);
    assert_eq!(several.unknown, ["lint", "review"]);
    assert_eq!(several.at_once, AT_ONCE);
    assert!(several.most_usd > 0.0, "each grade's own limit, added");
    assert!(several.left_out.is_empty());
}

#[test]
fn a_skill_with_no_tests_is_left_out_and_named_with_why() {
    let (level, name, no_tests) = plan("tidy", None);
    let no_tests = no_tests.map(|mut plan| {
        plan.tests.clear();
        plan.consent = None;
        plan
    });
    let (_, _, evals_only) = plan("forms", None);
    let evals_only = evals_only.map(|mut plan| {
        plan.tests.clear();
        plan.evals.push(GradeEval {
            id: "1".into(),
            prompt: "fill it".into(),
            hides: false,
            files: Vec::new(),
            not_text: Vec::new(),
            expectations: 1,
            on: false,
            chats: 12,
        });
        plan
    });
    let (_, _, running) = plan("busy", None);
    let running = running.map(|mut plan| {
        plan.running = Some("g1".into());
        plan
    });
    let several = SkillGradesPlan::of(vec![
        plan("deploy", None),
        (level.clone(), name, no_tests),
        (level.clone(), "forms".into(), evals_only),
        (level.clone(), "busy".into(), running),
        (
            level,
            "gone".into(),
            Err("There is no skill named gone.".into()),
        ),
    ]);
    assert_eq!(several.skills.len(), 1);
    let said: Vec<_> = several
        .left_out
        .iter()
        .map(|one| (one.name.as_str(), one.why.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            ("tidy", "It has no tests."),
            (
                "forms",
                "It has outcome evals but no trigger tests, and outcome evals run only from its own Grades tab."
            ),
            ("busy", "A grade of it is running already."),
            ("gone", "There is no skill named gone."),
        ]
    );
}

#[test]
fn a_profile_no_grade_can_run_on_refuses_the_whole_batch() {
    let (level, name, refused) = plan("deploy", None);
    let refused = refused.map(|mut plan| {
        plan.refused = Some("Its chats would run with the sandbox lifted.".into());
        plan
    });
    let several = SkillGradesPlan::of(vec![(level, name, refused)]);
    assert_eq!(
        several.refused.as_deref(),
        Some("Its chats would run with the sandbox lifted.")
    );
}

#[test]
fn each_step_says_where_every_skill_is_and_what_the_batch_spent() {
    let mut held = Driving::new(
        "b1".into(),
        PlaneId::for_tests(std::path::Path::new("/planes/p")),
        vec![
            BatchPick {
                level: project(),
                name: "deploy".into(),
                consent: "a".into(),
            },
            BatchPick {
                level: project(),
                name: "lint".into(),
                consent: "b".into(),
            },
        ],
        1.0,
    );
    let at = held.batch.start_next().unwrap();
    held.begun(at, "g1".into());
    held.told(at, 2, 6, Some(0.4), None);
    let step = held.step();
    assert_eq!(step.skills[0].grade.as_deref(), Some("g1"));
    assert_eq!((step.skills[0].ran, step.skills[0].of), (2, 6));
    assert_eq!(step.skills[0].state, BatchState::Running);
    assert_eq!(step.skills[1].state, BatchState::Waiting);
    assert!((step.spent_usd - 0.4).abs() < 1e-9);
    assert_eq!(step.ended, None);
    // Its grade reaches the limit: the batch is told to stop it, and the next never starts.
    assert!(held.told(at, 3, 6, Some(1.1), None));
    held.told(at, 3, 6, Some(1.1), Some(GradeEnded::Stopped));
    let step = held.step();
    assert_eq!(step.skills[0].state, BatchState::Ended);
    assert_eq!(step.skills[0].ended, Some(GradeEnded::Stopped));
    assert_eq!(
        step.skills[1].state,
        BatchState::NotRun {
            why: "The limit was reached first.".into()
        }
    );
    assert_eq!(step.ended, Some(BatchEnded::Limit));
}
