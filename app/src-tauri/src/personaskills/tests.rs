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

    let blocks = blocks(plane.path(), "ops");

    let [panel::Block::List { rows, .. }] = blocks.as_slice() else {
        panic!("one list: {blocks:?}");
    };
    let [row] = rows.as_slice() else {
        panic!("one row: {rows:?}");
    };
    assert_eq!(row.text, "Brings 3 skills into every workspace");
    assert_eq!(row.note.as_deref(), Some("Open in Skill studio"));
    assert_eq!(row.runs.as_deref(), Some("skills.studio:persona/ops"));
}

#[test]
fn a_persona_with_no_skills_says_it_brings_none() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("personas/ops")).unwrap();
    std::fs::write(plane.path().join("personas/ops/persona.md"), "---\n---\n").unwrap();

    let blocks = blocks(plane.path(), "ops");

    assert!(matches!(
        blocks.as_slice(),
        [panel::Block::List { rows, .. }] if rows[0].text == "Brings 0 skills into every workspace"
    ));
}
