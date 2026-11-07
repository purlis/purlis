// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! 1.77 (#4218): what a host says it armed a chat it composed with is names and levels alone,
//! bounded, as a window takes it.

use purlis_session_protocol::session::{ArmedSkill, MOST_ARMED_SKILL_NAME, MOST_ARMED_SKILLS};

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
