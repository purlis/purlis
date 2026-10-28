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
