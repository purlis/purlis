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
    let after_edit_chats = grading::plan(&request).per_arm * 3;
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
        outcomes_left_out: Vec::new(),
        offered: Vec::new(),
        eval_chats: 12,
        consent: Some("abc".into()),
        previous: None,
        previous_refused: None,
        // Its previous version's arm counted, as a first saved edit brings one (#4035).
        after_edit_chats: Some(after_edit_chats),
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
        evals_refused: None,
    }
}

/// SK-55 (#3235): the plan shows each outcome eval as its chat reads it, counts its chats and
/// its judges' in the chats, and names each eval it leaves out.
#[test]
fn the_plan_offers_each_outcome_eval_and_counts_only_those_ticked() {
    let mut planned = planned("ship it");
    let eval = purlis_core::skills::grade::OutcomeEval {
        id: 4,
        prompt: "fill\u{200b} it".into(),
        expected_output: None,
        files: vec!["evals/files/form.txt".into()],
        expectations: vec!["It is filled".into(), "It is dated".into()],
    };
    let other = purlis_core::skills::grade::OutcomeEval {
        id: 5,
        prompt: "file it".into(),
        files: Vec::new(),
        ..eval.clone()
    };
    planned.offered = vec![eval.clone(), other];
    let request = planned.request.as_mut().unwrap();
    request.outcomes.push(eval);
    planned.plan = Some(grading::plan(request));
    planned.outcomes_left_out = vec![grades::LeftOutEval {
        id: 9,
        why: "it has no expectations, so a judge would have nothing to check".into(),
    }];
    let plan = SkillGradePlan::of(
        &planned,
        None,
        on(Ok("full sandbox".into()), grades::price::of_model(None)),
    );
    // A test: 3 runs × 2 arms; the ticked eval: 3 runs × 2 arms × its chat and its judge's.
    assert_eq!(plan.chats, 6 + 12);
    assert_eq!((plan.outcome_evals, plan.judge_chats), (1, 6));
    let eval = &plan.evals[0];
    assert_eq!(eval.id, "4");
    assert!(eval.on && eval.hides);
    assert_eq!(eval.chats, 12);
    assert!(
        eval.prompt.starts_with("fill⟦U+200B⟧ it"),
        "{}",
        eval.prompt
    );
    assert!(
        eval.prompt
            .ends_with("in this folder: evals/files/form.txt.")
    );
    assert_eq!((eval.files.len(), eval.expectations), (1, 2));
    assert!(!plan.evals[1].on, "off until it is ticked");
    assert_eq!(plan.outcomes_left_out[0].id, "9");
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
            grader: None,
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
                level: None,
                stop: Arc::new(AtomicBool::new(false)),
                after_edit: None,
                draft: None,
                step: RunningStep::Skill(SkillGradeStep {
                    plane: plane.clone(),
                    skill: "deploy".into(),
                    level: SkillLevel::Project { local: false },
                    grade: grade.to_owned(),
                    ran: 0,
                    of: 1,
                    tokens: None,
                    cost_usd: None,
                    limit_usd: None,
                    cap: 1_000.0,
                    asked: 0,
                    ended: None,
                    said: None,
                    draft: None,
                }),
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
                outcome_pass_rate: None,
                outcome_chats: 0,
                outcome_not_run: 0,
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
            outcomes: Vec::new(),
            outcome_delta: None,
            outcome_tokens: None,
            outcome_usd: None,
            failed: None,
        },
        spent: Spent::default(),
        plugins_off: None,
        model: Some("claude-sonnet-4-5".into()),
        previous: None,
        after_edit: false,
        draft: None,
    };
    let card = GradeScorecard::of(&graded, |chat| chat != "01B");
    assert_eq!(card.against, None, "no previous version was compared");
    // #3978: the copy it graded, so a scorecard opened where it is not listed finds its scope.
    assert_eq!(card.level, Some(SkillLevel::Project { local: false }));
    let in_ops = Graded {
        level: Some("workspaces/ops/skills.local".into()),
        ..graded.clone()
    };
    assert_eq!(
        GradeScorecard::of(&in_ops, |_| false).level,
        Some(SkillLevel::Workspace {
            name: "ops".into(),
            local: true
        })
    );
    let unsaid = Graded {
        level: None,
        ..graded.clone()
    };
    assert_eq!(GradeScorecard::of(&unsaid, |_| false).level, None);
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
        outcome_pass_rate: None,
        outcome_chats: 0,
        outcome_not_run: 0,
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
            outcomes: Vec::new(),
            outcome_delta: None,
            outcome_tokens: None,
            outcome_usd: None,
            failed: None,
        },
        spent: Spent::default(),
        plugins_off: None,
        model: None,
        previous: Some("fedcba9876543210fedcba9876543210fedcba98".into()),
        after_edit: true,
        draft: None,
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
            asked_for: None,
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
        answer: None,
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
            None,
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
            plan_holds(planned, &on, None, &shown, "deploy", "nothing was started").unwrap_err();
        assert!(refused.contains("changed since it was shown"), "{refused}");
        assert!(refused.contains("nothing was started"), "{refused}");
    }
}

/// #3928: the Grades tab learns from its one read whether the skill has trigger tests, and the
/// grade of that copy running now at its latest step, so it asks no plan; a grade of another
/// skill, of the same name's copy at another level, or in another project, is not this one's.
/// A copy whose tests cannot be counted keeps its grades, and says why.
#[test]
fn a_skills_grades_say_its_trigger_tests_and_the_grade_of_it_running() {
    let plane: PlaneId = serde_json::from_value(serde_json::json!("plane-of-grades-3928")).unwrap();
    let elsewhere: PlaneId =
        serde_json::from_value(serde_json::json!("plane-elsewhere-3928")).unwrap();
    let shared = Level::project();
    let local = Level::project().local(true);
    let tests = || {
        Ok(grades::TriggerTests {
            runs: 2,
            evals: 0,
            refused: None,
        })
    };
    let none = SkillGrades::of(
        &grades::Read::default(),
        tests(),
        &plane,
        &shared,
        "ship-3928",
    );
    assert_eq!(none.tests, 2);
    assert_eq!(none.tests_refused, None);
    assert_eq!(none.running, None, "no grade runs");
    let lost = SkillGrades::of(
        &grades::Read::default(),
        Err("no project skill is named \"ship-3928\"".to_owned()),
        &plane,
        &shared,
        "ship-3928",
    );
    assert_eq!(lost.tests, 0);
    assert!(lost.tests_refused.is_some());

    let step = |plane: &PlaneId, skill: &str, grade: &str| SkillGradeStep {
        plane: plane.clone(),
        skill: skill.to_owned(),
        level: (&shared).into(),
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
        draft: None,
    };
    let grades_running = [
        ("g-3928-other-skill", &plane, "other-3928", &shared),
        ("g-3928-elsewhere", &elsewhere, "ship-3928", &shared),
        ("g-3928-local", &plane, "ship-3928", &local),
    ];
    for (grade, at, skill, level) in grades_running {
        running().insert(
            grade.to_owned(),
            Running {
                plane: at.clone(),
                skill: skill.to_owned(),
                level: Some(level.clone()),
                stop: Arc::new(AtomicBool::new(false)),
                after_edit: None,
                draft: None,
                step: RunningStep::Skill(step(at, skill, grade)),
            },
        );
    }
    // A router's grade is no skill's, even one that names the skill and its level.
    running().insert(
        "g-3928-router".to_owned(),
        Running {
            plane: plane.clone(),
            skill: "ship-3928".to_owned(),
            level: Some(shared.clone()),
            stop: Arc::new(AtomicBool::new(false)),
            after_edit: None,
            draft: None,
            step: RunningStep::Router(crate::routergrade::RouterGradeStep {
                plane: plane.clone(),
                router: "relevance".to_owned(),
                workspace: None,
                persona: None,
                grade: "g-3928-router".to_owned(),
                ran: 0,
                of: 1,
                tokens: None,
                cost_usd: None,
                limit_usd: None,
                cap: 1_000.0,
                ended: None,
                said: None,
                recorded: false,
            }),
        },
    );
    assert_eq!(step_of_running(&plane, &shared, "ship-3928"), None);
    running().remove("g-3928-router");
    let now = |level: &Level| {
        SkillGrades::of(
            &grades::Read::default(),
            tests(),
            &plane,
            level,
            "ship-3928",
        )
        .running
    };
    let (shared_now, local_now) = (now(&shared), now(&local));
    for (grade, ..) in grades_running {
        running().remove(grade);
    }
    assert_eq!(
        shared_now, None,
        "the local copy's grade is not the shared copy's"
    );
    assert_eq!(local_now, Some(step(&plane, "ship-3928", "g-3928-local")));
}

fn acme() -> GradeGrader {
    GradeGrader {
        id: "acme.grader".into(),
        title: "Acme".into(),
        hosts: vec!["acme.example".into()],
    }
}

fn runs_on() -> On {
    on(Ok("full sandbox".into()), grades::price::of_model(None))
}

#[test]
fn the_plan_lists_each_grader_and_names_the_one_picked() {
    let planned = planned("ship it");
    let own = plan_by(&planned, None, runs_on(), vec![acme()], None);
    assert_eq!(own.graders, vec![acme()], "listed beside purlis's own");
    assert_eq!(own.grader, None);
    assert_eq!(
        own.consent,
        consent_shown(&planned, &runs_on()),
        "purlis's own runner consents as a plan with no Grader did"
    );

    let picked = plan_by(&planned, None, runs_on(), vec![acme()], Some("acme.grader"));
    assert_eq!(picked.grader, Some(acme()));
    assert_ne!(picked.consent, own.consent, "the pick is in the consent");
}

#[test]
fn a_press_on_a_plan_of_another_grader_starts_nothing() {
    let planned = planned("ship it");
    let by_acme = consent_by(&planned, &runs_on(), Some(&acme())).unwrap();
    assert_eq!(
        plan_holds(
            &planned,
            &runs_on(),
            Some(&acme()),
            &by_acme,
            "deploy",
            "nothing was started"
        ),
        Ok(())
    );
    let other = GradeGrader {
        id: "other.grader".into(),
        title: "Acme".into(),
        hosts: vec!["acme.example".into()],
    };
    for grader in [None, Some(&other)] {
        let refused = plan_holds(
            &planned,
            &runs_on(),
            grader,
            &by_acme,
            "deploy",
            "nothing was started",
        )
        .unwrap_err();
        assert!(refused.contains("nothing was started"), "{refused}");
    }
}

#[test]
fn a_pick_no_longer_on_offer_is_said_and_consents_to_nothing() {
    let plan = plan_by(
        &planned("ship it"),
        None,
        runs_on(),
        Vec::new(),
        Some("acme.grader"),
    );
    assert_eq!(plan.consent, None);
    assert_eq!(plan.grader, None);
    assert!(
        plan.grader_refused
            .as_deref()
            .is_some_and(|why| why.contains("'acme.grader'")),
        "{:?}",
        plan.grader_refused
    );
    assert!(grader_picked(&[], Some("acme.grader")).is_err());
    assert_eq!(grader_picked(&[], None), Ok(None));
}

#[test]
fn a_graders_failure_is_in_its_scorecard() {
    let report = grading::GradeReport {
        grader: "acme.grader".into(),
        ended: GradeEnd::GraderFailed,
        failed: Some("The Grader 'acme.grader' asked past its plan.".into()),
        ..grades::underway::interrupted_report(1, None, None)
    };
    let graded = Graded {
        v: grades::VERSION,
        grade: "01G".into(),
        skill: "deploy".into(),
        level: None,
        tree: "t".into(),
        commit: None,
        harness: None,
        profile: None,
        ts: "2026-10-31T00:00:00Z".into(),
        runs: 3,
        spent: Spent::default(),
        report,
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
        draft: None,
    };
    let card = GradeScorecard::of(&graded, |_| false);
    assert!(
        card.ended
            .ends_with("The Grader 'acme.grader' asked past its plan."),
        "{}",
        card.ended
    );
    assert!(
        card.ran_on.ends_with("graded by acme.grader"),
        "{}",
        card.ran_on
    );
}

#[test]
fn a_grader_is_handed_the_trigger_tests_alone() {
    let request = GradeRequest {
        skill: "deploy".into(),
        triggers: vec![TriggerTest {
            query: "ship it".into(),
            should_trigger: true,
        }],
        outcomes: vec![purlis_core::skills::grade::OutcomeEval {
            id: 1,
            prompt: "an eval it is not shown".into(),
            expected_output: None,
            files: Vec::new(),
            expectations: vec!["it holds".into()],
        }],
        arms: grades::ARMS.to_vec(),
        runs: grading::RUNS,
    };
    assert_eq!(request.outcomes.len(), 1);
    let handed = handed_to_a_grader(&request);
    assert!(handed.outcomes.is_empty());
    assert_eq!(handed.triggers, request.triggers);
    assert_eq!(handed.arms, request.arms);
    assert_eq!(handed.runs, request.runs);
}

/// SK-55: a Grader and ticked outcome evals are not one grade: the plan says so and consents to
/// nothing, and a start is refused by the same word.
#[test]
fn a_grader_is_refused_beside_ticked_outcome_evals() {
    let mut ticked = planned("ship it");
    let request = ticked.request.as_mut().unwrap();
    request
        .outcomes
        .push(purlis_core::skills::grade::OutcomeEval {
            id: 4,
            prompt: "fill it".into(),
            expected_output: None,
            files: Vec::new(),
            expectations: vec!["It is filled".into()],
        });
    assert!(grader_takes(&ticked, true).is_err());
    assert!(grader_takes(&ticked, false).is_ok());
    assert!(grader_takes(&planned("ship it"), true).is_ok());
}

/// SK-55: ticked outcome evals on a profile they cannot run on start nothing; left off, they cost
/// that profile nothing.
#[test]
fn ticked_evals_are_refused_on_a_profile_they_cannot_run_on() {
    let why = "it signs in another way".to_owned();
    let mut ticked = planned("ship it");
    assert!(evals_can_run(&ticked, Some(&why)).is_ok());
    ticked
        .request
        .as_mut()
        .unwrap()
        .outcomes
        .push(purlis_core::skills::grade::OutcomeEval {
            id: 4,
            prompt: "fill it".into(),
            expected_output: None,
            files: Vec::new(),
            expectations: vec!["It is filled".into()],
        });
    assert_eq!(evals_can_run(&ticked, Some(&why)), Err(why));
    assert!(evals_can_run(&ticked, None).is_ok());
}
