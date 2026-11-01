// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A persona's page's skills, and adding and removing a name, on a plane in a temporary
//! directory (SK-5, #2768).

use std::path::{Path, PathBuf};

use super::*;

fn skill(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n"),
    )
    .unwrap();
}

struct Plane {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

/// A plane with the persona `ops`, whose definition says `frontmatter`.
fn plane(frontmatter: &str) -> Plane {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("plane");
    std::fs::create_dir_all(root.join("personas/ops")).unwrap();
    std::fs::write(
        root.join("personas/ops/persona.md"),
        format!("---\n{frontmatter}---\n\nThe body.\n"),
    )
    .unwrap();
    Plane { _tmp: tmp, root }
}

fn change_to(plane: &Path, persona: &str, how: Change, written: &str) -> Result<bool, String> {
    change(plane, persona, written, how)
}

fn definition(plane: &Plane) -> String {
    std::fs::read_to_string(plane.root.join("personas/ops/persona.md")).unwrap()
}

#[test]
fn it_brings_its_own_skills_and_each_other_one_its_list_names() {
    let plane = plane("skills: deploy, charter:handoff, superpowers:tdd, triage, purlis:handoff\n");
    skill(&plane.root.join("personas/ops/skills"), "triage");
    skill(&plane.root.join("skills"), "deploy");

    assert_eq!(brings(&plane.root, "ops"), 4, "triage once, handoff once");
    assert_eq!(brings(&plane.root, "nobody"), 0);
}

#[test]
fn adding_a_name_writes_the_skills_line_and_keeps_every_other_line() {
    let plane = plane("role: Ops\n");

    assert_eq!(
        change_to(&plane.root, "ops", Change::Add, "deploy"),
        Ok(true)
    );
    assert_eq!(
        change_to(&plane.root, "ops", Change::Add, "charter:handoff"),
        Ok(true)
    );

    assert_eq!(
        definition(&plane),
        "---\nrole: Ops\nskills: deploy, purlis:purlis-handoff\n---\n\nThe body.\n"
    );
}

#[test]
fn adding_a_name_the_list_already_gives_writes_nothing() {
    let plane = plane("skills: charter:handoff\n");
    let before = definition(&plane);

    assert_eq!(
        change_to(&plane.root, "ops", Change::Add, "purlis:handoff"),
        Ok(false)
    );
    assert_eq!(
        definition(&plane),
        before,
        "the old prefix names the same skill"
    );
}

#[test]
fn an_entry_that_names_no_skill_is_refused() {
    let plane = plane("");
    assert!(change_to(&plane.root, "ops", Change::Add, " : ").is_err());
}

#[test]
fn removing_a_name_takes_out_every_entry_of_it_and_the_line_when_it_empties() {
    let plane = plane("skills: deploy, charter:handoff\nrole: Ops\n");

    assert_eq!(
        change_to(&plane.root, "ops", Change::Remove, "purlis:handoff"),
        Ok(true)
    );
    assert_eq!(
        definition(&plane),
        "---\nskills: deploy\nrole: Ops\n---\n\nThe body.\n"
    );
    assert_eq!(
        change_to(&plane.root, "ops", Change::Remove, "deploy"),
        Ok(true)
    );
    assert_eq!(definition(&plane), "---\nrole: Ops\n---\n\nThe body.\n");
    assert_eq!(
        change_to(&plane.root, "ops", Change::Remove, "deploy"),
        Ok(false)
    );
}

#[test]
fn removing_never_touches_a_personas_own_skill() {
    let plane = plane("skills: triage\n");
    skill(&plane.root.join("personas/ops/skills"), "triage");

    assert_eq!(
        change_to(&plane.root, "ops", Change::Remove, "triage"),
        Ok(true)
    );

    assert!(
        plane
            .root
            .join("personas/ops/skills/triage/SKILL.md")
            .is_file()
    );
    assert_eq!(own(&plane.root, "ops"), ["triage"]);
}

#[test]
fn a_persona_that_is_not_one_is_refused_and_nothing_is_written() {
    let plane = plane("");
    assert!(change_to(&plane.root, "../ops", Change::Add, "deploy").is_err());
}
