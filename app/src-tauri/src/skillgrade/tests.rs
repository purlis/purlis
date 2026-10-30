// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::skills::grade::{Fixtures, GradeRequest, Read, TriggerTest};

pub(super) fn planned(query: &str) -> Planned {
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
        previous: None,
        previous_refused: None,
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
    let dearest = grades::price::of_model(None);
    let shown = grades::consent_on(
        "abc",
        &grades::Terms {
            profile: Some("work"),
            model: None,
            token_cap: 50_000,
            limit_usd: grades::cap::Cap {
                tokens: 50_000,
                from: grades::cap::From::Yours,
            }
            .cost_usd(&dearest),
        },
    );
    assert_eq!(
        (plan.refused, plan.consent.as_deref()),
        (None, Some(shown.as_str())),
        "what it grades, and the profile, model and limits it shows"
    );

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
                after_edit: None,
                step: None,
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
        previous: None,
        after_edit: false,
    };
    let card = GradeScorecard::of(&graded, |chat| chat != "01B");
    assert_eq!(card.against, None, "no previous version was compared");
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

/// SK-57 (#3237): a grade against the skill's previous version shows all three arms, and its
/// difference against that version, with the difference against no skill beside it.
#[test]
fn a_scorecard_against_the_previous_version_shows_three_arms_and_both_differences() {
    use purlis_core::skills::grade::{ArmScore, GradeEnd, GradeReport, Stat, TriggerScore};
    let score = |arm: Arm| TriggerScore {
        query: "ship it".into(),
        should_trigger: true,
        arm,
        runs: 3,
        triggered: 3,
        rate: Some(1.0),
        pass: Some(true),
        chats: vec![None; 3],
        asked: 0,
    };
    let arm = |arm: Arm, mean: f64| ArmScore {
        arm,
        pass_rate: Some(Stat {
            mean,
            stddev: 0.0,
            min: mean,
            max: mean,
        }),
        tokens: None,
        seconds: None,
        chats: 3,
        not_run: 0,
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
            triggers: vec![score(Arm::With), score(Arm::Without), score(Arm::Previous)],
            arms: vec![
                arm(Arm::With, 1.0),
                arm(Arm::Without, 0.25),
                arm(Arm::Previous, 0.75),
            ],
            delta: Some(0.25),
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
        model: None,
        previous: Some("fedcba9876543210fedcba9876543210fedcba98".into()),
        after_edit: true,
    };
    let card = GradeScorecard::of(&graded, |_| false);
    assert_eq!(card.against.as_deref(), Some("fedcba987654"));
    assert_eq!(card.delta, Some(25.0), "against the previous version");
    assert_eq!(card.delta_without, Some(75.0), "against no skill");
    assert!(card.after_edit);
    let [test] = card.tests.as_slice() else {
        panic!("{:?}", card.tests);
    };
    assert_eq!(
        test.arms.iter().map(|arm| arm.arm).collect::<Vec<_>>(),
        [ScoredArm::With, ScoredArm::Without, ScoredArm::Previous]
    );
    // Its totals (S6): each arm's pass rate in points, and how it ended.
    let arms = GradeArms {
        with: Some(100.0),
        without: Some(25.0),
        previous: Some(75.0),
    };
    assert_eq!(card.arms, arms);
    assert_eq!(card.outcome, GradeOutcome::Graded);

    // Its row on the Grades tab says the same, with what it spent and the limit it was held to.
    let spent = Graded {
        spent: Spent {
            chats: 9,
            tokens: Some(90_000),
            cost_usd: Some(1.92),
        },
        report: GradeReport {
            ended: GradeEnd::OverLimit,
            cost_cap_usd: Some(3.0),
            ..graded.report.clone()
        },
        ..graded
    };
    let row = GradeRow::of(&spent);
    assert_eq!(row.grade, "01GRADE");
    assert_eq!(row.arms, arms);
    assert_eq!(row.outcome, GradeOutcome::AtLimit);
    assert_eq!(row.ended, "Stopped at its dollar limit");
    assert_eq!(
        row.spent,
        GradeSpent {
            chats: 9,
            tokens: Some(90_000.0),
            cost_usd: Some(1.92),
            limit_usd: Some(3.0),
        }
    );
    assert!(row.after_edit);
}

/// S6: a grade's list marks each end as one of four: graded, stopped by the person, stopped at its
/// limit, or interrupted.
#[test]
fn a_grade_ends_graded_stopped_at_its_limit_or_interrupted() {
    use purlis_core::skills::grade::GradeEnd;
    let outcome = GradeOutcome::from;
    assert_eq!(outcome(GradeEnd::Whole), GradeOutcome::Graded);
    assert_eq!(outcome(GradeEnd::Cancelled), GradeOutcome::Stopped);
    for end in [
        GradeEnd::OverCap,
        GradeEnd::OverLimit,
        GradeEnd::SpendNotReported,
    ] {
        assert_eq!(outcome(end), GradeOutcome::AtLimit, "{end:?}");
    }
    for end in [GradeEnd::ChatsDidNotStart, GradeEnd::Interrupted] {
        assert_eq!(outcome(end), GradeOutcome::Interrupted, "{end:?}");
    }
}

/// #3582: what a grade's marker is told its chats spent is what they said, and no more: every
/// chat that started counts its tokens and its cost, whatever ended it, and only a chat that ran
/// counts as one.
#[test]
fn what_a_grades_chats_spent_is_counted_from_what_each_said_3582() {
    let chat = |tokens: Option<u64>, cost_usd: Option<f64>, end: ChatEnd| ChatRan {
        chat: None,
        started: Vec::new(),
        tokens,
        cost_usd,
        ms: 0,
        end,
    };
    let mut spent = Spent::default();
    spent = counted(spent, &chat(Some(4_000), Some(0.04), ChatEnd::Turn));
    spent = counted(spent, &chat(Some(1_000), Some(0.01), ChatEnd::Asked));
    spent = counted(spent, &chat(Some(500), None, ChatEnd::TimedOut));
    spent = counted(spent, &chat(None, Some(f64::NAN), ChatEnd::Cancelled));
    spent = counted(
        spent,
        &chat(None, None, ChatEnd::NotStarted { why: "no".into() }),
    );
    assert_eq!(spent.chats, 2, "the turn and the time out ran");
    assert_eq!(spent.tokens, Some(5_500), "every chat that said its tokens");
    assert!((spent.cost_usd.unwrap() - 0.05).abs() < 1e-9, "{spent:?}");
    let past = counted(spent, &chat(Some(u64::MAX), Some(-1.0), ChatEnd::OverCap));
    assert_eq!(past.tokens, Some(u64::MAX), "held at the most a count says");
    assert_eq!(
        past.cost_usd, spent.cost_usd,
        "a cost no reader could hold is none"
    );
    assert_eq!(
        counted(Spent::default(), &chat(None, None, ChatEnd::Asked)),
        Spent::default(),
        "a chat that said nothing adds nothing"
    );
}

/// A press is held to the plan the person read: what it grades, and the profile, model, cap and
/// limit it shows.
#[test]
fn a_press_on_a_plan_that_changed_is_refused() {
    let dearest = grades::price::of_model(None);
    let planned = planned("ship it");
    let shown = consent_shown(&planned, &on(Ok("full sandbox".into()), dearest.clone())).unwrap();
    assert!(
        plan_holds(
            &planned,
            &on(Ok("full sandbox".into()), dearest.clone()),
            &shown,
            "deploy",
            "nothing was started"
        )
        .is_ok()
    );

    let another_profile = On {
        profile: Some("home".into()),
        ..on(Ok("full sandbox".into()), dearest.clone())
    };
    let another_model = On {
        model: Some("opus".into()),
        ..on(Ok("full sandbox".into()), dearest.clone())
    };
    let another_price = on(
        Ok("full sandbox".into()),
        grades::price::of_model(Some("claude-haiku-4-5")),
    );
    let mut another_cap = planned.clone();
    another_cap.cap.tokens = 400_000;
    for (planned, on) in [
        (&planned, another_profile),
        (&planned, another_model),
        (&planned, another_price),
        (&another_cap, on(Ok("full sandbox".into()), dearest.clone())),
    ] {
        let refused =
            plan_holds(planned, &on, &shown, "deploy", "nothing was started").unwrap_err();
        assert!(refused.contains("changed since it was shown"), "{refused}");
        assert!(refused.contains("nothing was started"), "{refused}");
    }
}

/// #3928: the Grades tab learns from its one read whether the skill has trigger tests, and the
/// grade of it running now at its latest step, so it asks no plan; a grade of another skill, or
/// in another project, is not this one's.
#[test]
fn a_skills_grades_say_its_trigger_tests_and_the_grade_of_it_running() {
    let plane: PlaneId = serde_json::from_value(serde_json::json!("plane-of-grades-3928")).unwrap();
    let elsewhere: PlaneId =
        serde_json::from_value(serde_json::json!("plane-elsewhere-3928")).unwrap();
    let tests = grades::TriggerTests {
        runs: 2,
        refused: None,
    };
    let none = SkillGrades::of(&grades::Read::default(), &tests, &plane, "ship-3928");
    assert_eq!(none.tests, 2);
    assert_eq!(none.tests_refused, None);
    assert_eq!(none.running, None, "no grade runs");

    let step = |plane: &PlaneId, skill: &str, grade: &str| SkillGradeStep {
        plane: plane.clone(),
        skill: skill.to_owned(),
        grade: grade.to_owned(),
        ran: 3,
        of: 12,
        tokens: Some(1_000.0),
        cost_usd: Some(0.25),
        limit_usd: Some(2.0),
        cap: 500_000.0,
        asked: 0,
        ended: None,
        said: None,
    };
    for (grade, at, skill) in [
        ("g-3928-other-skill", &plane, "other-3928"),
        ("g-3928-elsewhere", &elsewhere, "ship-3928"),
        ("g-3928", &plane, "ship-3928"),
    ] {
        running().insert(
            grade.to_owned(),
            Running {
                plane: at.clone(),
                skill: skill.to_owned(),
                stop: Arc::new(AtomicBool::new(false)),
                after_edit: None,
                step: Some(step(at, skill, grade)),
            },
        );
    }
    let now = SkillGrades::of(&grades::Read::default(), &tests, &plane, "ship-3928");
    for grade in ["g-3928-other-skill", "g-3928-elsewhere", "g-3928"] {
        running().remove(grade);
    }
    assert_eq!(now.running, Some(step(&plane, "ship-3928", "g-3928")));
}
