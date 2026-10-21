// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_core::harness::Harness;
use purlis_core::skills::{Running, of_a_chat};

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
    let skills = of_a_chat(&Running {
        plane: &root,
        workspace: None,
        persona: None,
        harness: Harness::Codex,
        built_in: Some(&built_in),
        plane_skills: true,
    });

    let shown = shown(Some(Harness::Codex), Some(&skills));

    assert!(shown.on);
    assert_eq!(shown.harness.as_deref(), Some("Codex"));
    let names: Vec<&str> = shown.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["handoff", "release", "review"]);
    let release = &shown.skills[1];
    assert!(release.manual);
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
            over: None,
        })
    );
}

#[test]
fn a_chat_on_no_harness_purlis_knows_is_offered_nothing_and_has_no_budget() {
    let shown = shown(None, None);
    assert!(shown.on);
    assert_eq!(shown.harness, None);
    assert!(shown.skills.is_empty());
    assert_eq!(shown.budget, None);
}
