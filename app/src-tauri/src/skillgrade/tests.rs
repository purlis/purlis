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
            outcomes: Read::Refused(purlis_core::skills::grade::Refused {
                file: purlis_core::skills::grade::FixtureFile::Outcomes,
                item: None,
                wrong: purlis_core::skills::grade::Wrong::NotJson { line: 1, column: 2 },
            }),
        },
        plan: Some(grading::plan(&request)),
        request: Some(request),
        estimate: None,
        record_said: None,
        unseen: if query.contains('\u{202e}') {
            vec![0]
        } else {
            Vec::new()
        },
        left_out: vec![grades::LeftOut {
            at: 1,
            why: "it starts with a dash".into(),
        }],
        cap: grades::cap::Cap {
            tokens: 50_000,
            from: grades::cap::From::Yours,
        },
        consent: Some("abc".into()),
        copy: None,
    }
}

#[test]
fn the_plan_shows_each_query_as_the_chat_reads_it_and_what_is_left_out() {
    let plan = SkillGradePlan::of(
        &planned("ship it\u{202e}now"),
        None,
        (
            Some("work".into()),
            Ok("full sandbox".into()),
            grades::price::of_model(None),
        ),
    );
    assert_eq!(plan.chats, 6);
    assert_eq!(plan.runs, 3);
    assert_eq!(plan.tests.len(), 1);
    assert!(plan.tests[0].hides);
    assert_eq!(plan.tests[0].query, "ship it⟦U+202E⟧now");
    assert_eq!(
        plan.left_out,
        vec![GradeLeftOut {
            at: 1,
            why: "it starts with a dash".to_owned()
        }]
    );
    assert_eq!(
        plan.outcomes_refused.as_deref(),
        Some("The skill's outcome evals could not be read: they are not written right, at line 1.")
    );
    assert_eq!(plan.tests_refused, None);
    assert_eq!((plan.token_cap, plan.cap_set_here), (50_000.0, true));
    assert!(
        (plan.cost_cap_usd - 3.75).abs() < 1e-9,
        "{}",
        plan.cost_cap_usd
    );
    assert_eq!(
        plan.priced,
        GradePrice {
            usd_per_million: 75.0,
            of_the_model: false,
            model: None,
        },
        "no model said: the dearest known price"
    );
    assert_eq!(plan.chat_minutes, 5);
    assert_eq!(plan.sandbox.as_deref(), Some("full sandbox"));
    assert_eq!((plan.refused, plan.consent.as_deref()), (None, Some("abc")));

    let refused = SkillGradePlan::of(
        &planned("ship it"),
        None,
        (
            Some("work".into()),
            Err("the sandbox is off".into()),
            grades::price::of_model(None),
        ),
    );
    assert_eq!(refused.refused.as_deref(), Some("the sandbox is off"));
    assert_eq!(refused.sandbox, None);
    assert!(
        !refused.tests[0].hides,
        "the hidden mark is the plan's own reading"
    );
}

#[test]
fn the_plan_says_the_limit_at_the_price_of_the_model_its_chats_run_on() {
    let plan = SkillGradePlan::of(
        &planned("ship it"),
        None,
        (
            Some("work".into()),
            Ok("full sandbox".into()),
            grades::price::of_model(Some("claude-sonnet-4-6")),
        ),
    );
    // 50,000 tokens at $15 a million.
    assert!(
        (plan.cost_cap_usd - 0.75).abs() < 1e-9,
        "{}",
        plan.cost_cap_usd
    );
    assert_eq!(
        plan.priced,
        GradePrice {
            usd_per_million: 15.0,
            of_the_model: true,
            model: Some("claude-sonnet-4-6".into()),
        }
    );
}

#[test]
fn a_grade_that_is_not_running_cannot_be_stopped() {
    let plane = PlaneId::for_tests(std::path::Path::new("/planes/p"));
    let said = tauri::async_runtime::block_on(skill_grade_stop(plane, "nope".into()));
    assert!(said.is_err());
}

/// #3353 (L3): a panic inside a grade ends it as failed, and it is not running after.
#[test]
fn a_grade_that_panics_ends_as_failed_and_is_not_running_after() {
    let plane: PlaneId = serde_json::from_value(serde_json::json!("plane-of-a-panic")).unwrap();
    for (grade, panics) in [("panics", true), ("ends", false)] {
        running().insert(
            grade.to_owned(),
            Running {
                plane: plane.clone(),
                skill: "deploy".into(),
                stop: Arc::new(AtomicBool::new(false)),
            },
        );
        let failed = std::cell::Cell::new(false);
        held_to_its_end(
            grade,
            || {
                if panics {
                    panic!("a fault in the grade");
                }
            },
            || failed.set(true),
        );
        assert_eq!(failed.get(), panics, "{grade}");
        assert_eq!(running_of(&plane, "deploy"), None, "{grade} is not running");
    }
}
