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
        record: grades::Read::default(),
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

/// Where a grade's chats run in a test: on `work`, checked as `checked`, priced at `priced`, its
/// harness offering Claude Code's three models and none picked.
fn on(checked: Result<String, String>, priced: grading::Priced) -> On {
    On {
        profile: Some("work".into()),
        checked,
        priced,
        models: ["opus", "sonnet", "haiku"].map(str::to_owned).to_vec(),
        model: None,
    }
}

#[test]
fn the_plan_shows_each_query_as_the_chat_reads_it_and_what_is_left_out() {
    let plan = SkillGradePlan::of(
        &planned("ship it\u{202e}now"),
        None,
        on(Ok("full sandbox".into()), grades::price::of_model(None)),
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
        on(
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
        on(
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

/// SK-60 (#3240): the plan offers the models the profile's harness can run, and says the one
/// picked.
#[test]
fn the_plan_offers_the_models_the_profiles_harness_can_run() {
    let plan = SkillGradePlan::of(
        &planned("ship it"),
        None,
        On {
            model: Some("sonnet".into()),
            ..on(
                Ok("full sandbox".into()),
                grades::price::of_model(Some("sonnet")),
            )
        },
    );
    assert_eq!(plan.models, ["opus", "sonnet", "haiku"]);
    assert_eq!(plan.model.as_deref(), Some("sonnet"));
    assert_eq!(plan.priced.model.as_deref(), Some("sonnet"));
}

/// SK-59 (#3239): a grade's scorecard has each trigger test once, with each arm's runs and each
/// of its chats, the ones this machine kept marked so the window opens them, and the asks said.
#[test]
fn a_scorecard_has_each_test_once_with_each_arms_chats_one_click_away() {
    use purlis_core::skills::grade::{ArmScore, GradeEnd, GradeReport, TriggerScore};
    let score = |arm: Arm, chats: &[&str], asked: u32| TriggerScore {
        query: "ship it\u{202e}".into(),
        should_trigger: true,
        arm,
        runs: 3 - asked,
        triggered: 1,
        rate: Some(1.0 / f64::from(3 - asked)),
        pass: Some(false),
        chats: chats.iter().map(|chat| Some((*chat).to_owned())).collect(),
        asked,
    };
    let graded = Graded {
        v: grades::VERSION,
        grade: "01GRADE".into(),
        skill: "deploy".into(),
        level: Some("skills".into()),
        tree: "uncommitted".into(),
        commit: None,
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: "2026-10-28T10:00:00+00:00".into(),
        runs: 3,
        report: GradeReport {
            grader: grading::BUILT_IN.into(),
            triggers: vec![
                score(Arm::With, &["01A", "01B", "01C"], 0),
                score(Arm::Without, &["01D", "01E", "01F"], 1),
            ],
            arms: vec![ArmScore {
                arm: Arm::With,
                pass_rate: None,
                tokens: None,
                seconds: None,
                chats: 3,
                not_run: 0,
            }],
            delta: Some(0.333),
            ended: GradeEnd::Whole,
            not_started: None,
            cap: None,
            spent_tokens: None,
            cost_cap_usd: None,
            spent_usd: None,
            counted_usd: None,
            priced: None,
        },
        spent: Spent::default(),
        plugins_off: None,
        model: Some("claude-sonnet-4-5".into()),
    };
    let card = GradeScorecard::of(&graded, |chat| chat != "01B");
    assert_eq!(card.ran_on, "Claude Code · claude-sonnet-4-5");
    assert_eq!(card.version, "Changes not saved");
    assert_eq!(card.ended, "Finished");
    assert_eq!(card.delta, Some(33.0));
    assert_eq!(card.asked, 1);
    let [test] = card.tests.as_slice() else {
        panic!("{:?}", card.tests);
    };
    assert_eq!(test.query, "ship it⟦U+202E⟧");
    assert_eq!(
        test.arms.iter().map(|arm| arm.arm).collect::<Vec<_>>(),
        [ScoredArm::With, ScoredArm::Without]
    );
    assert_eq!(
        test.arms[0].chats,
        [
            ScoredChat {
                id: "01A".into(),
                run: 1,
                kept: true
            },
            ScoredChat {
                id: "01B".into(),
                run: 2,
                kept: false
            },
            ScoredChat {
                id: "01C".into(),
                run: 3,
                kept: true
            },
        ]
    );
    assert_eq!(test.arms[1].asked, 1);
}

/// The review of #3239: a host sends a view's snapshot in pieces, after a reset where the view is
/// not its first, so the screen kept is every piece until the view is quiet.
#[test]
fn a_screen_sent_in_pieces_is_kept_whole() {
    let (send, pieces) = std::sync::mpsc::channel();
    for piece in ["", "\u{1b}c", "first half, ", "second half"] {
        send.send(piece.to_owned()).unwrap();
    }
    assert_eq!(
        gathered(&pieces, Instant::now()).as_deref(),
        Some("\u{1b}cfirst half, second half")
    );
    let (send, nothing) = std::sync::mpsc::channel::<String>();
    send.send(String::new()).unwrap();
    drop(send);
    assert_eq!(
        gathered(&nothing, Instant::now()),
        None,
        "nothing was shown"
    );
}
