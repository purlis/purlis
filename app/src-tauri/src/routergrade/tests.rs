// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_scope_is_a_workspace_and_a_persona_by_name_or_none() {
    assert!(scope(Some("ops".into()), Some("reviewer".into())).is_ok());
    assert!(scope(None, None).is_ok());
    assert!(scope(Some("../ops".into()), None).is_err());
    assert!(scope(None, Some("re/viewer".into())).is_err());
}

#[test]
fn only_a_router_an_extension_s_can_be_graded() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    let ops = || Scope {
        workspace: Some("ops".into()),
        persona: None,
    };
    assert!(
        chosen(root, &ops())
            .unwrap_err()
            .contains("built-in router")
    );
    std::fs::write(
        root.join("workspaces/ops/workspace.json"),
        r#"{"settings": {"skills": {"router": "relevance"}}}"#,
    )
    .unwrap();
    assert_eq!(chosen(root, &ops()).unwrap(), "relevance");
}

#[test]
fn the_plan_says_the_library_its_tested_skills_and_the_chats_before_anything_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let folder = root.join("skills/deploy");
    std::fs::create_dir_all(folder.join("evals")).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships.\n---\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("evals/triggers.json"),
        r#"[{"query": "ship it", "should_trigger": true}, {"query": "-x", "should_trigger": false}]"#,
    )
    .unwrap();
    let planned = routers::planned(root, None, None, "relevance").unwrap();
    let on = crate::skillgrade::On {
        profile: Some("claude".into()),
        checked: Ok("Full sandbox".into()),
        priced: grades::price::of_model(None),
        models: Vec::new(),
        model: None,
    };
    let plan = RouterGradePlan::of(&planned, None, on, None);
    assert_eq!(plan.router, "relevance");
    assert_eq!(plan.library, 1);
    assert_eq!(plan.skills.len(), 1);
    assert_eq!(plan.skills[0].tests, 1);
    assert_eq!(
        plan.skills[0].left_out.len(),
        1,
        "a query a chat reads as a flag"
    );
    // One test, two arms, three runs.
    assert_eq!(plan.chats, 6);
    assert!(plan.consent.is_some());
    assert_eq!(plan.sandbox.as_deref(), Some("Full sandbox"));
}

/// A router's grade of two skills, `deploy` failing with the router and passing with the
/// built-in one, `rollback` passing with both: each test's three chats named `<skill>-<arm>-<n>`.
fn a_router_grade() -> RouterGraded {
    let score = |skill: &str, arm: &str, pass: bool| {
        serde_json::json!({
            "query": format!("{skill} it"), "should_trigger": true, "arm": arm, "runs": 3,
            "triggered": if pass { 3 } else { 0 }, "rate": if pass { 1.0 } else { 0.0 },
            "pass": pass, "asked": 0,
            "chats": (1..=3).map(|n| format!("{skill}-{arm}-{n}")).collect::<Vec<_>>(),
        })
    };
    let part = |skill: &str, router_passes: bool| {
        serde_json::json!({"skill": skill, "report": {
            "grader": "purlis", "delta": null, "ended": "whole", "not_started": null,
            "cap": 200_000, "spent_tokens": 6000,
            "triggers": [score(skill, "router", router_passes), score(skill, "built-in-router", true)],
            "arms": [
                {"arm": "router", "pass_rate": null, "tokens": null, "seconds": null, "chats": 3, "not_run": 0},
                {"arm": "built-in-router", "pass_rate": null, "tokens": null, "seconds": null, "chats": 3, "not_run": 0},
            ],
        }})
    };
    serde_json::from_value(serde_json::json!({
        "v": 1, "grade": "01GRADE", "router": "relevance", "workspace": "ops", "persona": null,
        "commit": "a".repeat(40), "library": [], "harness": "claude", "profile": "work",
        "ts": "2026-10-30T10:00:00+00:00", "runs": 3, "model": "sonnet",
        "spent": {"chats": 12, "tokens": 12000, "cost_usd": 0.12},
        "report": {
            "grader": "purlis", "router": "relevance", "ended": "whole", "not_started": null,
            "cap": 200_000, "spent_tokens": 12000, "cost_cap_usd": 1.5, "spent_usd": 0.12,
            "skills": [part("deploy", false), part("rollback", true)],
        },
    }))
    .expect("a router's grade")
}

/// #3617: a router's scorecard says each skill's trigger tests in both arms, each chat it ran
/// with whether this machine kept it, its totals and how it ended.
#[test]
fn a_router_s_scorecard_shows_each_skill_s_tests_in_both_arms_and_each_chat_3617() {
    let card = RouterGradeScorecard::of(&a_router_grade(), |chat| chat.starts_with("deploy"));
    assert_eq!(card.router, "relevance");
    assert_eq!(card.library, "the library at aaaaaaaaaaaa");
    assert_eq!(card.ran_on, "Claude Code · sonnet");
    assert_eq!(card.outcome, GradeOutcome::Graded);
    assert_eq!(
        card.arms.router,
        Some(50.0),
        "one of two tests passes with it"
    );
    assert_eq!(card.arms.built_in, Some(100.0));
    assert_eq!(card.spent.chats, 12);
    assert_eq!(card.spent.limit_usd, Some(1.5));
    let skills: Vec<&str> = card.skills.iter().map(|one| one.skill.as_str()).collect();
    assert_eq!(skills, ["deploy", "rollback"]);
    let deploy = &card.skills[0].tests;
    assert_eq!(deploy.len(), 1, "one test, in both arms");
    assert_eq!(deploy[0].arms.len(), 2);
    assert_eq!(deploy[0].arms[0].pass, Some(false));
    assert_eq!(deploy[0].arms[1].pass, Some(true));
    let chats = &deploy[0].arms[0].chats;
    assert_eq!(chats.len(), 3);
    assert_eq!(chats[0].id, "deploy-router-1");
    assert!(chats.iter().all(|chat| chat.kept));
    assert!(
        card.skills[1].tests[0].arms[0]
            .chats
            .iter()
            .all(|chat| !chat.kept),
        "not kept on this machine"
    );
}

/// Where a router's grade's chats run, as a test names it: `profile` and `model`.
fn on(profile: &str, model: Option<&str>) -> crate::skillgrade::On {
    crate::skillgrade::On {
        profile: Some(profile.into()),
        checked: Ok("Full sandbox".into()),
        priced: grades::price::of_model(model),
        models: vec!["sonnet".into(), "opus".into()],
        model: model.map(str::to_owned),
    }
}

/// #3618: pressing Grade consents to the profile, the model, the cap and the dollar limit the
/// plan shows, as a skill's does: a press on a plan that showed others starts nothing.
#[test]
fn a_router_s_plan_consents_to_its_profile_model_and_limits_3618() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let folder = root.join("skills/deploy");
    std::fs::create_dir_all(folder.join("evals")).unwrap();
    std::fs::write(
        folder.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships.\n---\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("evals/triggers.json"),
        r#"[{"query": "ship it", "should_trigger": true}]"#,
    )
    .unwrap();
    let planned = routers::planned(root, None, None, "relevance").unwrap();
    let consent = |on: &crate::skillgrade::On| consent_shown(&planned, on).unwrap();
    let first = consent(&on("work", None));
    assert_eq!(first, consent(&on("work", None)), "the same plan");
    assert_ne!(first, consent(&on("home", None)), "another profile");
    assert_ne!(first, consent(&on("work", Some("opus"))), "another model");
    let mut capped = planned.clone();
    capped.cap.tokens += 1;
    assert_ne!(
        first,
        consent_shown(&capped, &on("work", None)).unwrap(),
        "another cap"
    );
    let plan = RouterGradePlan::of(&planned, None, on("work", Some("opus")), None);
    assert_eq!(plan.models, ["sonnet", "opus"]);
    assert_eq!(plan.model.as_deref(), Some("opus"));
    assert_eq!(
        plan.consent.as_deref(),
        Some(consent(&on("work", Some("opus"))).as_str())
    );
    assert!(plan.estimate.is_none(), "never graded: none known");
    assert!(plan.estimate_same_library.is_none());
}

#[test]
fn a_skill_s_grade_running_is_no_router_s_step() {
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!("plane-of-a-skill-step")).unwrap();
    running().insert(
        "skill-step".to_owned(),
        Running {
            plane: plane.clone(),
            skill: "deploy".into(),
            level: None,
            stop: Arc::new(AtomicBool::new(false)),
            after_edit: None,
            step: RunningStep::Skill(crate::skillgrade::SkillGradeStep {
                plane: plane.clone(),
                skill: "deploy".into(),
                level: crate::skillchange::SkillLevel::Project { local: false },
                grade: "skill-step".into(),
                ran: 1,
                of: 2,
                tokens: None,
                cost_usd: None,
                limit_usd: None,
                cap: 1_000.0,
                asked: 0,
                ended: None,
                said: None,
            }),
        },
    );
    assert_eq!(step_of_running(&plane, "skill-step"), None);
    running().remove("skill-step");
}

#[test]
fn a_router_grade_s_page_finds_its_latest_step_while_it_runs_and_none_after() {
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!("plane-of-a-router-step")).unwrap();
    let other: PlaneId = serde_json::from_value(serde_json::json!("another-plane")).unwrap();
    let step = RouterGradeStep {
        plane: plane.clone(),
        router: "relevance".into(),
        workspace: Some("ops".into()),
        persona: None,
        grade: "router-step".into(),
        ran: 3,
        of: 8,
        tokens: Some(1200.0),
        cost_usd: Some(0.04),
        limit_usd: Some(1.5),
        cap: 200_000.0,
        ended: None,
        said: None,
        recorded: false,
    };
    running().insert(
        "router-step".to_owned(),
        Running {
            plane: plane.clone(),
            skill: String::new(),
            level: None,
            stop: Arc::new(AtomicBool::new(false)),
            after_edit: None,
            step: RunningStep::Router(step.clone()),
        },
    );
    assert_eq!(step_of_running(&plane, "router-step"), Some(step.clone()));
    // Another project's page never reads it, and an id nothing runs is no step.
    assert_eq!(step_of_running(&other, "router-step"), None);
    assert_eq!(step_of_running(&plane, "not-running"), None);
    running().remove("router-step");
    assert_eq!(step_of_running(&plane, "router-step"), None);
    // An end told before its page listened is still read, until the project's next end.
    let ended = RouterGradeStep {
        ended: Some(GradeEnded::ChatsDidNotStart),
        said: Some("no profile".into()),
        ..step
    };
    keep_end(&ended);
    assert_eq!(step_of_running(&plane, "router-step"), Some(ended.clone()));
    assert_eq!(step_of_running(&other, "router-step"), None);
    keep_end(&RouterGradeStep {
        grade: "router-next".into(),
        ..ended
    });
    assert_eq!(step_of_running(&plane, "router-step"), None);
}
