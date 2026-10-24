// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_level_the_studio_names_is_the_cores_level_and_back() {
    for level in [
        SkillLevel::Project { local: false },
        SkillLevel::Workspace {
            name: "ops".to_owned(),
            local: true,
        },
        SkillLevel::Persona {
            name: "reviewer".to_owned(),
            local: false,
        },
    ] {
        let core: Level = (&level).into();
        assert_eq!(SkillLevel::from(&core), level);
    }
    assert!(skill_move_needs_grant(
        SkillLevel::Workspace {
            name: "ops".to_owned(),
            local: false
        },
        SkillLevel::Project { local: true },
    ));
    // Promoting a persona's own skill to the project asks for its grant too (SK-32).
    assert!(skill_move_needs_grant(
        SkillLevel::Persona {
            name: "reviewer".to_owned(),
            local: false
        },
        SkillLevel::Project { local: false },
    ));
    assert!(!skill_move_needs_grant(
        SkillLevel::Project { local: false },
        SkillLevel::Persona {
            name: "reviewer".to_owned(),
            local: false
        },
    ));
    // As the window sends it.
    assert_eq!(
        serde_json::from_str::<SkillLevel>(r#"{"at":"persona","name":"reviewer","local":true}"#)
            .unwrap(),
        SkillLevel::Persona {
            name: "reviewer".to_owned(),
            local: true
        }
    );
}

#[test]
fn the_lint_and_the_templates_are_the_cores() {
    let templates = skill_templates();
    assert_eq!(
        templates.iter().map(|one| one.id).collect::<Vec<_>>(),
        [SkillTemplateId::Blank, SkillTemplateId::Steps]
    );
    let tmp = tempfile::tempdir().unwrap();
    for one in &templates {
        let text = Template::from(one.id).text("deploy", "Ships it to staging and checks it.");
        assert_eq!(
            findings(tmp.path(), &Level::project(), "deploy", &text),
            Vec::<SkillLintFinding>::new(),
            "{:?}",
            one.id
        );
    }
    assert_eq!(
        findings(tmp.path(), &Level::project(), "deploy", "no frontmatter"),
        [SkillLintFinding {
            level: SkillLintLevel::Blocks,
            says: purlis_core::skills::Problem::NoFrontmatter.to_string(),
        }]
    );
    // A short description warns, and Save does not wait on it.
    let short = Template::Blank.text("deploy", "Ships it.");
    let said = findings(tmp.path(), &Level::project(), "deploy", &short);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].level, SkillLintLevel::Warns);
}

#[test]
fn a_change_left_for_the_next_save_says_so_and_a_done_line_is_not_repeated() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();

    let changed = heard(tmp.path(), |cx| {
        verbs::create(
            cx,
            &Level::project(),
            "deploy",
            "Ships it.",
            Template::Blank,
        )
    })
    .unwrap();

    assert_eq!(changed.notes.len(), 1, "{changed:?}");
    assert!(changed.notes[0].starts_with("Not saved"), "{changed:?}");
    assert!(tmp.path().join("skills/deploy/SKILL.md").exists());
}

#[test]
fn a_deleted_skill_handed_to_the_window_comes_back_whole_and_restores() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    heard(tmp.path(), |cx| {
        verbs::create(
            cx,
            &Level::project(),
            "deploy",
            "Ships it.",
            Template::Blank,
        )
    })
    .unwrap();
    heard(tmp.path(), |cx| {
        verbs::delete(cx, &Level::project(), "deploy")
    })
    .unwrap();

    let listed = verbs::deleted(tmp.path());
    let id = serde_json::to_string(&listed[0]).unwrap();
    let back: verbs::Deleted = serde_json::from_str(&id).unwrap();
    heard(tmp.path(), |cx| verbs::restore(cx, &back)).unwrap();

    assert!(tmp.path().join("skills/deploy/SKILL.md").exists());
    assert_eq!(levels(tmp.path())[0].label, "project");
}
