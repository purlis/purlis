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
    let plan = RouterGradePlan::of(&planned, None, on);
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
