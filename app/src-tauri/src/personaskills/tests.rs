// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn skill(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
    )
    .unwrap();
}

#[test]
fn a_personas_skills_are_one_summary_row_that_opens_the_studio_on_it() {
    let plane = tempfile::tempdir().unwrap();
    let dir = plane.path().join("personas/ops");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("persona.md"),
        "---\nskills: deploy, superpowers:tdd\n---\n",
    )
    .unwrap();
    skill(&dir.join("skills"), "triage");

    let blocks = blocks(plane.path(), "ops", 0);

    let [panel::Block::List { rows, .. }] = blocks.as_slice() else {
        panic!("one list: {blocks:?}");
    };
    let [row] = rows.as_slice() else {
        panic!("one row: {rows:?}");
    };
    assert_eq!(row.text, "Brings 3 skills into every workspace");
    assert_eq!(row.note.as_deref(), Some("Open Skill studio"));
    assert_eq!(row.runs.as_deref(), Some("skills.studio:persona/ops"));
}

#[test]
fn a_persona_with_no_skills_says_it_brings_none() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("personas/ops")).unwrap();
    std::fs::write(plane.path().join("personas/ops/persona.md"), "---\n---\n").unwrap();

    let blocks = blocks(plane.path(), "ops", 0);

    assert!(matches!(
        blocks.as_slice(),
        [panel::Block::List { rows, .. }] if rows[0].text == "Brings 0 skills into every workspace"
    ));
}

#[test]
fn what_needs_you_is_a_second_row_that_opens_needs_you_on_the_persona() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("personas/ops")).unwrap();
    std::fs::write(plane.path().join("personas/ops/persona.md"), "---\n---\n").unwrap();

    let blocks = blocks(plane.path(), "ops", 2);

    let [panel::Block::List { rows, .. }] = blocks.as_slice() else {
        panic!("one list: {blocks:?}");
    };
    let [counts, needing] = rows.as_slice() else {
        panic!("two rows: {rows:?}");
    };
    assert_eq!(counts.runs.as_deref(), Some("skills.studio:persona/ops"));
    assert_eq!(needing.text, "2 need you");
    assert_eq!(needing.runs.as_deref(), Some("skills.needs:persona/ops"));
}
