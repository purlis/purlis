// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! 1.77 (#4218): what a host says it armed a chat it composed with is names and levels alone,
//! and 1.81 (#2977) what it says the chat started without is names alone, and 1.83 (#6078)
//! names, levels and a why cut short, each bounded, as a window takes it.

use purlis_session_protocol::session::{
    ArmedSkill, MOST_ARMED_SKILL_NAME, MOST_ARMED_SKILLS, MOST_SKILL_WHY, MOST_SKILLS_NOT_LOADED,
    SkillNotLoaded, SkillsNotThere,
};

fn skill(name: &str, level: &str) -> ArmedSkill {
    ArmedSkill {
        name: name.into(),
        level: level.into(),
    }
}

/// **A host's word on a chat's skills is names and levels, bounded** (#4218): one with no
/// name, a long one or a control character is not taken, nor more than the most.
#[test]
fn a_hosts_armed_skills_are_taken_each_that_fits_and_at_most_the_most() {
    let said = vec![
        skill("deploy", "workspace"),
        skill("", "project"),
        skill("x".repeat(MOST_ARMED_SKILL_NAME + 1).as_str(), "project"),
        skill("bell\u{7}", "project"),
        skill("review", "a-level-far-too-long-to-be-any-level-word"),
        skill("purlis-secrets", "built-in"),
    ];
    assert_eq!(
        ArmedSkill::bounded(said),
        vec![
            skill("deploy", "workspace"),
            skill("purlis-secrets", "built-in")
        ]
    );
    let many = (0..MOST_ARMED_SKILLS + 5).map(|n| skill(&format!("s{n}"), "project"));
    let taken = ArmedSkill::bounded(many);
    assert_eq!(taken.len(), MOST_ARMED_SKILLS);
    assert_eq!(taken[0].name, "s0");
}

/// **1.81 (#2977): a host's word on the skills a chat started without is names alone,
/// bounded**: one with no name, a long one or a control character is not taken, nor more than
/// the most of each kind, and a word with none left says nothing.
#[test]
fn a_hosts_skills_not_there_are_taken_each_name_that_fits_and_at_most_the_most() {
    let said = SkillsNotThere {
        missing: vec![
            "deploy".into(),
            String::new(),
            "x".repeat(MOST_ARMED_SKILL_NAME + 1),
            "bell\u{7}".into(),
        ],
        no_built_in: (0..MOST_ARMED_SKILLS + 5)
            .map(|n| format!("b{n}"))
            .collect(),
        ..Default::default()
    };
    let taken = said.bounded().expect("names are left");
    assert_eq!(taken.missing, ["deploy"]);
    assert_eq!(taken.no_built_in.len(), MOST_ARMED_SKILLS);
    assert_eq!(taken.no_built_in[0], "b0");

    let none_fit = SkillsNotThere {
        missing: vec![String::new()],
        ..Default::default()
    };
    assert_eq!(none_fit.bounded(), None, "nothing is left to say");
}

fn not_loaded(name: &str, level: &str, why: &str) -> SkillNotLoaded {
    SkillNotLoaded {
        name: name.into(),
        level: level.into(),
        local: false,
        why: why.into(),
    }
}

/// **1.83 (#6078): a host's word on the skills a chat could not load there, and on another
/// workspace's it lacks, is bounded too**: a skill whose name or level does not fit is not
/// taken, its why loses its control characters and is cut at a character's edge, and each kind
/// holds at most the most.
#[test]
fn a_hosts_skills_not_loaded_there_are_taken_each_that_fits_with_its_why_cut_short() {
    let long_why = format!("{}é", "w".repeat(MOST_SKILL_WHY - 1));
    let said = SkillsNotThere {
        no_workspace: (0..MOST_SKILLS_NOT_LOADED + 5)
            .map(|n| format!("workspaces/ghost:s{n}"))
            .collect(),
        not_in_its_workspace: vec!["workspaces/ops:nope".into(), "bell\u{7}".into()],
        broken: vec![
            not_loaded("half-made", "project", "no frontmatter\nat all"),
            not_loaded("", "project", "no name"),
            not_loaded("review", "a-level-far-too-long-to-be-any-level-word", "why"),
            not_loaded("long", "workspace", &long_why),
        ],
        by_a_repo: (0..MOST_SKILLS_NOT_LOADED + 5)
            .map(|n| not_loaded(&format!("r{n}"), "project", "the repo has its own"))
            .collect(),
        beside_a_built_in: vec!["purlis-secrets".into(), String::new()],
        ..Default::default()
    };
    let taken = said.bounded().expect("names are left");
    assert_eq!(taken.no_workspace.len(), MOST_SKILLS_NOT_LOADED);
    assert_eq!(taken.not_in_its_workspace, ["workspaces/ops:nope"]);
    assert_eq!(taken.beside_a_built_in, ["purlis-secrets"]);
    assert_eq!(taken.by_a_repo.len(), MOST_SKILLS_NOT_LOADED);
    assert_eq!(
        taken.broken,
        [
            not_loaded("half-made", "project", "no frontmatterat all"),
            not_loaded("long", "workspace", &"w".repeat(MOST_SKILL_WHY - 1)),
        ]
    );
}
