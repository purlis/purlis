// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::skills::verbs::Level;

use super::super::{Running, step_of_running};
use super::*;

fn a_plane_id(name: &str) -> PlaneId {
    serde_json::from_value(serde_json::json!(name)).unwrap()
}

/// A grade of the draft `draft` of `skill` running in `plane`.
fn draft_grade_running(
    plane: &PlaneId,
    grade: &str,
    skill: &str,
    draft: Option<&str>,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let level = Level::project();
    running().insert(
        grade.to_owned(),
        Running {
            plane: plane.clone(),
            skill: skill.to_owned(),
            level: Some(level.clone()),
            stop: Arc::clone(&stop),
            after_edit: None,
            draft: draft.map(str::to_owned),
            step: RunningStep::Skill(SkillGradeStep {
                plane: plane.clone(),
                skill: skill.to_owned(),
                level: (&level).into(),
                grade: grade.to_owned(),
                ran: 3,
                of: 12,
                tokens: None,
                cost_usd: None,
                limit_usd: None,
                cap: 1.0,
                asked: 0,
                ended: None,
                said: None,
                draft: draft.map(str::to_owned),
            }),
        },
    );
    stop
}

#[test]
fn a_grade_of_a_draft_shows_on_the_drafts_page_and_never_as_the_skills_grade() {
    let plane = a_plane_id("draft-2780-shown");
    draft_grade_running(&plane, "g-2780-shown", "ship-2780-shown", Some("d-shown"));

    let ours = step_of(&plane, "d-shown");
    let skills = step_of_running(&plane, &Level::project(), "ship-2780-shown");
    let plan_says = super::super::running_of(&plane, "ship-2780-shown");
    running().remove("g-2780-shown");

    assert_eq!(ours.map(|step| step.grade), Some("g-2780-shown".to_owned()));
    assert_eq!(skills, None);
    assert_eq!(plan_says, None);
}

#[test]
fn a_stopped_grade_says_the_chats_its_plan_ran_and_only_what_it_measured() {
    use purlis_core::skills::grade::{Arm, ArmScore, GradeEnd, Stat};
    let rate = |mean: f64| Stat {
        mean,
        stddev: 0.0,
        min: mean,
        max: mean,
    };
    let arm = |arm, pass_rate, outcome_pass_rate| ArmScore {
        arm,
        pass_rate,
        tokens: None,
        seconds: None,
        chats: 0,
        not_run: 0,
        outcome_pass_rate,
        outcome_chats: 0,
        outcome_not_run: 0,
    };
    let mut report = grades::underway::interrupted_report(1000, None, None);
    report.ended = GradeEnd::Cancelled;
    report.outcome_delta = Some(0.25);
    report.arms = vec![
        arm(Arm::With, Some(rate(1.0)), Some(rate(0.75))),
        arm(Arm::Without, None, Some(rate(0.5))),
    ];
    let row = Graded {
        v: grades::VERSION,
        grade: "g-2780".to_owned(),
        skill: "deploy".to_owned(),
        level: Some("skills".to_owned()),
        tree: "uncommitted".to_owned(),
        commit: None,
        harness: None,
        profile: None,
        ts: "2026-11-02T00:00:00Z".to_owned(),
        runs: 3,
        report,
        spent: grades::Spent::default(),
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
        draft: Some(grades::draft::Mark {
            id: "mint".to_owned(),
            digest: "0".repeat(64),
            chats: 54,
        }),
    };

    let said = SkillDraftGrade::of(
        grades::draft::Said {
            only_evals: false,
            tests: Ok(grades::TriggerTests {
                runs: 2,
                evals: 1,
                refused: None,
            }),
            latest: Some(Box::new(row)),
            changed: false,
        },
        None,
    );

    assert_eq!(said.asked, 54, "the plan's, not the chats it got to");
    assert_eq!(said.scores.len(), 2);
    assert_eq!(said.scores[0].delta, None, "its baseline did not run");
    assert!(said.scores[1].evals);
    assert_eq!(said.scores[1].delta, Some(25.0));
}

#[test]
fn accepting_or_discarding_a_draft_stops_its_grade_and_no_other() {
    let plane = a_plane_id("draft-2780-stop");
    let its = draft_grade_running(&plane, "g-2780-its", "ship-2780-stop", Some("d-stop"));
    let other = draft_grade_running(&plane, "g-2780-other", "ship-2780-stop", None);

    stop(&plane, "d-stop");
    running().remove("g-2780-its");
    running().remove("g-2780-other");

    assert!(its.load(Ordering::SeqCst));
    assert!(!other.load(Ordering::SeqCst));
}
