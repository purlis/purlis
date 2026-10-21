// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The budget a chat's skill set spends, on sets worked out from planes in a temporary
//! directory.

use std::path::Path;

use super::*;
use crate::{Chat, HarnessSkills, Machine, PROJECT_DIR, Route, skill_set};

fn skill(dir: &Path, name: &str, description: &str, more: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n{more}---\n\nbody\n"),
    )
    .unwrap();
}

/// The set of a chat at the root of the plane at `plane`, with the built-in skills in `built_in`.
fn set_at_the_root(plane: &Path, built_in: &Path) -> SkillSet {
    skill_set(
        Some(plane),
        &Machine {
            built_in: Some(built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Plugin,
            },
        },
    )
}

const ROOMY: Budget = Budget {
    skills: 10,
    chars: 1_000,
    per_description: None,
};

#[test]
fn every_skill_a_chat_gets_costs_one_skill_and_its_descriptions_characters() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    skill(&built_in, "handoff", "Hands off.", "");
    skill(&plane.join(PROJECT_DIR), "deploy", "Deploys the app.", "");
    // Not a skill of this chat's: never counted.
    std::fs::create_dir_all(plane.join(PROJECT_DIR).join("half")).unwrap();

    let spent = spent_by(&set_at_the_root(&plane, &built_in), ROOMY);
    assert_eq!(spent.skills, 2);
    assert_eq!(spent.chars, "Hands off.".len() + "Deploys the app.".len());
    assert_eq!(spent.manual, 0);
    assert_eq!(spent.over(), None);
}

#[test]
fn a_manual_only_skill_is_in_the_set_and_costs_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    skill(&built_in, "handoff", "Hands off.", "");
    skill(
        &plane.join(PROJECT_DIR),
        "release",
        &"Cuts a release. ".repeat(40),
        "disable-model-invocation: true\n",
    );

    let set = set_at_the_root(&plane, &built_in);
    assert_eq!(set.effective.len(), 2, "still offered: {set:?}");
    let spent = spent_by(&set, ROOMY);
    assert_eq!(
        (spent.skills, spent.chars, spent.manual),
        (1, "Hands off.".len(), 1)
    );
    assert_eq!(spent.over(), None);
}

#[test]
fn a_set_past_the_budget_says_which_part_it_is_over() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&built_in).unwrap();
    for n in 0..3 {
        skill(
            &plane.join(PROJECT_DIR),
            &format!("s{n}"),
            &"x".repeat(50),
            "",
        );
    }
    let set = set_at_the_root(&plane, &built_in);
    let budget = |skills, chars| Budget {
        skills,
        chars,
        per_description: None,
    };
    assert_eq!(
        spent_by(&set, budget(3, 150)).over(),
        None,
        "at it is within"
    );
    assert_eq!(spent_by(&set, budget(2, 150)).over(), Some(Over::Skills));
    assert_eq!(spent_by(&set, budget(3, 149)).over(), Some(Over::Chars));
    assert_eq!(spent_by(&set, budget(2, 100)).over(), Some(Over::Both));
}

#[test]
fn a_description_counts_only_as_far_as_the_harness_lists_it() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&built_in).unwrap();
    skill(&plane.join(PROJECT_DIR), "long", &"é".repeat(2_000), "");
    let set = set_at_the_root(&plane, &built_in);
    let capped = Budget {
        per_description: Some(1_024),
        ..ROOMY
    };
    // Characters, not bytes.
    assert_eq!(spent_by(&set, ROOMY).chars, 2_000);
    assert_eq!(spent_by(&set, capped).chars, 1_024);
}
