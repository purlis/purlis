// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::skills::grade::{Fixtures, GradeRequest, Read, TriggerTest};

fn planned(query: &str) -> Planned {
    let tests = vec![TriggerTest {
        query: query.to_owned(),
        should_trigger: true,
    }];
    let request = GradeRequest {
        skill: "deploy".into(),
        triggers: tests.clone(),
        outcomes: Vec::new(),
        arms: grades::ARMS.to_vec(),
        runs: grading::RUNS,
    };
    Planned {
        skill: "deploy".into(),
        fixtures: Fixtures {
            triggers: Read::Read(tests),
            outcomes: Read::Refused("evals/evals.json is not JSON purlis can read".into()),
        },
        plan: Some(grading::plan(&request)),
        request: Some(request),
        estimate: None,
        unseen: vec![0],
        left_out: vec![(1, "it starts with a dash".into())],
    }
}

#[test]
fn the_plan_shows_each_query_as_the_chat_reads_it_and_what_is_left_out() {
    let plan = SkillGradePlan::of(&planned("ship it\u{202e}now"), None);
    assert_eq!(plan.chats, 6);
    assert_eq!(plan.runs, 3);
    assert_eq!(plan.tests.len(), 1);
    assert!(plan.tests[0].hides);
    assert_eq!(plan.tests[0].query, "ship it⟦U+202E⟧now");
    assert_eq!(
        plan.left_out,
        vec!["Test 2: it starts with a dash".to_owned()]
    );
    assert!(plan.outcomes_refused.is_some());
    assert_eq!(plan.tests_refused, None);
}

#[test]
fn a_grade_that_is_not_running_cannot_be_stopped() {
    let plane = PlaneId::for_tests(std::path::Path::new("/planes/p"));
    let said = tauri::async_runtime::block_on(skill_grade_stop(plane, "nope".into()));
    assert!(said.is_err());
}
