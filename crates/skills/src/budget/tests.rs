// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The budget a chat's skill set spends, on sets worked out from planes in a temporary
//! directory.

use std::path::Path;

use super::*;
use crate::{Chat, HarnessSkills, Machine, PROJECT_DIR, Route, SkillSet, skill_set};

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
            made_since: None,
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

/// What every skill of `set` spends of `budget`, each read for what it declares.
fn spent_by(set: &SkillSet, budget: Budget) -> Spent {
    let declared: Vec<Declared> = set
        .effective
        .iter()
        .map(|effective| crate::declared(&effective.skill))
        .collect();
    spent(
        set.effective
            .iter()
            .map(|effective| &effective.skill)
            .zip(&declared),
        budget,
    )
}

const ROOMY: Budget = Budget {
    skills: 10,
    chars: 1_000,
    per_description: None,
    lists_paths: false,
    per_line: None,
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
    assert_eq!(spent.warns(), None);
}

#[test]
fn a_manual_only_skill_costs_nothing_only_where_the_harness_leaves_it_out() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    skill(&built_in, "handoff", "Hands off.", "");
    skill(
        &plane.join(PROJECT_DIR),
        "release",
        "Cuts a release.",
        "disable-model-invocation: true\n",
    );

    let set = set_at_the_root(&plane, &built_in);
    assert_eq!(set.effective.len(), 2, "still offered: {set:?}");
    let left_out = spent_by(&set, ROOMY);
    assert_eq!(
        (left_out.skills, left_out.chars, left_out.manual),
        (1, "Hands off.".len(), 1)
    );
}

#[test]
fn a_set_warns_at_four_fifths_of_either_limit_and_says_which() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&built_in).unwrap();
    for n in 0..4 {
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
        ..ROOMY
    };
    // 4 skills and 200 characters.
    assert_eq!(spent_by(&set, budget(6, 300)).warns(), None, "below both");
    assert_eq!(
        spent_by(&set, budget(5, 300)).warns(),
        Some(Over::Skills),
        "4 of 5 is four fifths"
    );
    assert_eq!(spent_by(&set, budget(6, 250)).warns(), Some(Over::Chars));
    assert_eq!(spent_by(&set, budget(3, 100)).warns(), Some(Over::Both));
}

#[test]
fn a_description_counts_only_as_far_as_the_harness_lists_it_and_a_listed_path_counts_too() {
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
    let with_paths = Budget {
        lists_paths: true,
        ..capped
    };
    let path = set.effective[0]
        .skill
        .path
        .to_string_lossy()
        .chars()
        .count();
    assert_eq!(spent_by(&set, with_paths).chars, 1_024 + path);
}

#[test]
fn where_a_harness_limit_counts_whole_lines_a_skill_costs_its_name_when_to_use_and_line() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&built_in).unwrap();
    skill(
        &plane.join(PROJECT_DIR),
        "deploy",
        "Ships it",
        "when_to_use: On a release\n",
    );
    let set = set_at_the_root(&plane, &built_in);
    // A harness whose limit counts descriptions alone is charged as it always was.
    assert_eq!(spent_by(&set, ROOMY).chars, "Ships it".len());
    let lines = Budget {
        per_line: Some(23),
        ..ROOMY
    };
    assert_eq!(
        spent_by(&set, lines).chars,
        "deploy".len() + "Ships it".len() + "On a release".len() + 23
    );
}

#[test]
fn a_when_to_use_shares_its_descriptions_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = tmp.path().join("bundle");
    let plane = tmp.path().join("plane");
    std::fs::create_dir_all(&built_in).unwrap();
    let more = format!("when_to_use: {}\n", "w".repeat(100));
    skill(&plane.join(PROJECT_DIR), "s", &"d".repeat(100), &more);
    let set = set_at_the_root(&plane, &built_in);
    let capped = Budget {
        per_line: Some(0),
        per_description: Some(150),
        ..ROOMY
    };
    assert_eq!(spent_by(&set, capped).chars, "s".len() + 150);
}

#[test]
fn a_set_written_over_a_budget_replaces_only_the_numbers_it_names() {
    let chars = Set {
        skills: None,
        chars: Some(9_000),
    };
    assert_eq!(ROOMY.with(chars).skills, 10);
    assert_eq!(ROOMY.with(chars).chars, 9_000);
    assert_eq!(ROOMY.with(Set::default()), ROOMY);
}

#[test]
fn a_huge_budget_warns_of_nothing_rather_than_overflowing() {
    let huge = Budget {
        skills: usize::MAX,
        chars: usize::MAX,
        ..ROOMY
    };
    let spent = Spent {
        skills: 1,
        chars: 1,
        manual: 0,
        budget: huge,
    };
    assert_eq!(spent.warns(), None);
}
