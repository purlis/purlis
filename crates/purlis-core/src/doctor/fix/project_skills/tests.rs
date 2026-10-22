// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn only_a_skills_own_md_files_have_their_references_rewritten() {
    for path in [
        ".claude/skills/deploy/SKILL.md",
        ".claude/skills/deploy/notes/a.md",
        "skills/deploy/SKILL.md",
        "workspaces/ops/skills/deploy/SKILL.md",
        "personas/ops/skills/deploy/refs/b.md",
    ] {
        assert!(in_a_skill(path), "{path}");
    }
    for path in [
        ".claude/skills/README.md",
        "skills/deploy.toml",
        "skills/deploy/run.sh",
        "skills.local/deploy/SKILL.md",
        "workspaces/ops/workspace.md",
        "docs/skills/a/b.md",
    ] {
        assert!(!in_a_skill(path), "{path}");
    }
}

#[test]
fn a_path_is_under_a_root_only_past_its_slash() {
    assert!(under(".claude/skills/a", FROM));
    assert!(!under(".claude/skills", FROM));
    assert!(!under(".claude/skillset/a", FROM));
}

#[test]
fn the_message_lists_each_move() {
    let plan = Plan {
        folders: vec!["deploy".to_owned(), "review".to_owned()],
        skills: Vec::new(),
        make_to: true,
        workspaces: 0,
    };
    let body = body(&plan);
    assert!(body.starts_with(&format!("{SUBJECT}\n\n")), "{body}");
    assert!(
        body.contains("- .claude/skills/deploy -> skills/deploy\n"),
        "{body}"
    );
    assert!(
        body.contains("- .claude/skills/review -> skills/review\n"),
        "{body}"
    );
}

#[test]
fn a_workspaces_turn_off_or_a_narrower_copy_keeps_the_skill_and_a_built_in_name_does_not() {
    let folder = Path::new("/p/skills/deploy");
    let left = |reason: Reason| SkillSet {
        left_out: vec![crate::skills::LeftOut {
            name: "deploy".to_owned(),
            origin: Origin::Project,
            local: false,
            folder: folder.to_path_buf(),
            reason,
        }],
        ..SkillSet::default()
    };
    assert_eq!(lost(&left(Reason::TurnedOff), folder, "deploy"), None);
    let narrower = Reason::NameTaken {
        by: Origin::Workspace,
        local: false,
    };
    assert_eq!(lost(&left(narrower), folder, "deploy"), None);
    let built_in = Reason::NameTaken {
        by: Origin::BuiltIn,
        local: false,
    };
    assert!(lost(&left(built_in), folder, "deploy").is_some());
    assert!(lost(&left(Reason::NotGranted), folder, "deploy").is_some());
    assert!(lost(&SkillSet::default(), folder, "deploy").is_some());
}
