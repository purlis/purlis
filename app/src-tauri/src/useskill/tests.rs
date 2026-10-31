// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::harness::Harness;
use purlis_core::skills::{Running, for_use_skill};

fn skill(dir: &std::path::Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

#[test]
fn the_window_is_shown_the_chats_skills_with_what_each_takes_and_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("plugin/skills");
    let root = tmp.path().join("plane");
    skill(
        &built_in,
        "handoff",
        "---\nname: handoff\ndescription: Hands off.\n---\n",
    );
    skill(
        &root.join("skills"),
        "review",
        "---\nname: review\ndescription: Reviews.\narguments: [pr, focus]\n---\n",
    );
    skill(
        &root.join("skills"),
        "release",
        "---\nname: release\ndescription: Releases.\nargument-hint: <version>\n\
         disable-model-invocation: true\n---\n",
    );
    let skills = for_use_skill(&Running {
        plane: &root,
        workspace: None,
        persona: None,
        harness: Harness::ClaudeCode,
        built_in: Some(&built_in),
        plane_skills: true,
        start: None,
    });

    let shown = shown(Some(&skills));

    assert!(shown.on);
    assert!(shown.known);
    let names: Vec<&str> = shown.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["handoff", "release", "review"]);
    let release = &shown.skills[1];
    assert!(release.manual);
    assert!(release.takes_arguments, "it says what to pass");
    assert!(!shown.skills[0].takes_arguments);
    assert_eq!(release.hint.as_deref(), Some("<version>"));
    assert_eq!(release.level, Level::Project);
    assert_eq!(release.label, "project");
    assert_eq!(shown.skills[2].arguments, ["pr", "focus"]);
    assert_eq!(
        shown.budget,
        Some(SkillBudget {
            skills: 2,
            chars: u32::try_from("Hands off.".len() + "Reviews.".len()).unwrap(),
            manual: 1,
            most_skills: 30,
            most_chars: 6_000,
            warns: None,
            hidden: 0,
            still_over: None,
            pinned_by: Vec::new(),
            pins_together: None,
        })
    );
    assert!(shown.skills.iter().all(|one| one.hidden.is_none()));
}

#[test]
fn a_chat_on_no_tool_purlis_knows_is_offered_nothing_and_has_no_budget() {
    let shown = shown(None);
    assert!(shown.on);
    assert!(!shown.known);
    assert!(shown.skills.is_empty());
    assert_eq!(shown.budget, None);
}

#[test]
fn a_chat_with_no_id_yet_has_nothing_typed_and_says_so() {
    // SK-38: its prompts are digested under its id, so with none no run could ever be matched.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    skill(
        &root.join("skills"),
        "review",
        "---\nname: review\ndescription: Reviews.\n---\n",
    );
    let skills = for_use_skill(&Running {
        plane: &root,
        workspace: None,
        persona: None,
        harness: Harness::Codex,
        built_in: None,
        plane_skills: true,
        start: None,
    });

    let refused = typing_of(&skills, None, "review", &[]).unwrap_err();
    assert_eq!(refused, NOT_STARTED);

    let salt = purlis_core::skillspan::PromptSalt::of_key("01JB00000000000000000000C4").unwrap();
    let typing = typing_of(&skills, Some(&salt), "review", &[]).expect("typed with an id");
    assert_eq!(typing.text, "$review");
}

#[test]
fn the_budget_names_the_workspace_and_the_persona_that_pin_a_skill_sk15d() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::write(
        root.join("workspaces/ops/workspace.json"),
        r#"{"settings": {"skills": {"pinned": ["review"]}}}"#,
    )
    .unwrap();
    skill(
        &root.join("skills"),
        "review",
        "---\nname: review\ndescription: Reviews.\n---\n",
    );
    let skills = for_use_skill(&Running {
        plane: &root,
        workspace: Some("ops"),
        persona: None,
        harness: Harness::ClaudeCode,
        built_in: None,
        plane_skills: true,
        start: None,
    });

    let budget = shown(Some(&skills)).budget.unwrap();

    assert_eq!(
        budget.pinned_by,
        [crate::skillchange::SkillPinner::Workspace {
            name: "ops".to_owned()
        }]
    );
}

/// #3445: a chat whose start the plane's skills did not reach (the Skill studio's flag off) was
/// handed the built-in skills alone, so Use skill… offers those alone.
#[test]
fn a_chat_started_with_the_flag_off_is_offered_the_built_in_skills_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("plugin/skills");
    let root = tmp.path().join("plane");
    skill(
        &built_in,
        "handoff",
        "---\nname: handoff\ndescription: Hands off.\n---\n",
    );
    skill(
        &root.join("skills"),
        "review",
        "---\nname: review\ndescription: Reviews.\n---\n",
    );
    let skills = for_use_skill(&Running {
        plane: &root,
        workspace: None,
        persona: None,
        harness: Harness::ClaudeCode,
        built_in: Some(&built_in),
        plane_skills: false,
        start: None,
    });

    let shown = shown(Some(&skills));

    let names: Vec<&str> = shown.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["handoff"]);
}

/// #3391: forty pins of the workspace's and thirty of the persona's, each list read alone, are
/// past the 64 a chat reads together: the budget names both and how many each pins.
#[test]
fn the_budget_says_a_workspace_and_a_persona_pin_past_64_together_3391() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    let names = |from: usize, to: usize| -> Vec<String> {
        (from..to).map(|at| format!("pin-{at:02}")).collect()
    };
    let quoted: Vec<String> = names(0, 40).iter().map(|n| format!("{n:?}")).collect();
    std::fs::write(
        root.join("workspaces/ops/workspace.json"),
        format!(
            r#"{{"settings": {{"skills": {{"pinned": [{}]}}}}}}"#,
            quoted.join(", ")
        ),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("personas/reviewer")).unwrap();
    std::fs::write(
        root.join("personas/reviewer/persona.md"),
        format!(
            "---\nname: reviewer\nrole: Reviews.\npinned-skills: {}\n---\n\nReviews.\n",
            names(40, 70).join(", ")
        ),
    )
    .unwrap();
    let chat = |persona| {
        for_use_skill(&Running {
            plane: &root,
            workspace: Some("ops"),
            persona,
            harness: Harness::ClaudeCode,
            built_in: None,
            plane_skills: true,
            start: None,
        })
    };

    let budget = shown(Some(&chat(Some("reviewer")))).budget.unwrap();

    use crate::skillchange::{SkillPinner, SkillPinsHeld, SkillPinsTogether};
    assert_eq!(
        budget.pins_together,
        Some(SkillPinsTogether {
            held: vec![
                SkillPinsHeld {
                    by: SkillPinner::Workspace {
                        name: "ops".to_owned()
                    },
                    pins: 40
                },
                SkillPinsHeld {
                    by: SkillPinner::Persona {
                        name: "reviewer".to_owned()
                    },
                    pins: 30
                },
            ],
            most: 64,
        })
    );
    // Each pinner alone is accepted.
    for by in [
        purlis_core::skills::PinnedBy::Workspace("ops".to_owned()),
        purlis_core::skills::PinnedBy::Persona("reviewer".to_owned()),
    ] {
        assert_eq!(crate::skillchange::pins_refused(&root, &by), None);
    }
    assert_eq!(shown(Some(&chat(None))).budget.unwrap().pins_together, None);
}
