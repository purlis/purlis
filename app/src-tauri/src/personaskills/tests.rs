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
fn a_personas_skills_section_counts_them_and_lists_its_own_then_each_name() {
    let plane = tempfile::tempdir().unwrap();
    let dir = plane.path().join("personas/ops");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("persona.md"),
        "---\nskills: deploy, superpowers:tdd, purlis:gone\n---\n",
    )
    .unwrap();
    skill(&dir.join("skills"), "triage");
    skill(&plane.path().join("skills"), "deploy");
    let built_in = plane.path().join("plugin/skills");
    std::fs::create_dir_all(&built_in).unwrap();

    let blocks = blocks(plane.path(), "ops", Some(&built_in));

    let [
        panel::Block::Note { text, .. },
        panel::Block::List { rows, empty },
    ] = blocks.as_slice()
    else {
        panic!("a note and a list: {blocks:?}");
    };
    assert_eq!(text, "It brings 4 skills into every workspace.");
    let shown: Vec<(&str, &str, Option<&str>, panel::Tone)> = rows
        .iter()
        .map(|row| {
            (
                row.key.as_str(),
                row.text.as_str(),
                row.note.as_deref(),
                row.tone,
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("own:triage", "triage", Some("persona"), panel::Tone::Plain),
            (
                "named:deploy",
                "deploy",
                Some("project"),
                panel::Tone::Plain
            ),
            (
                "named:superpowers:tdd",
                "superpowers:tdd",
                Some("installed"),
                panel::Tone::Plain
            ),
            (
                "named:purlis:gone",
                "purlis:gone — its persona names it, and no built-in skill has this name",
                Some("built-in"),
                panel::Tone::Trouble
            ),
        ]
    );
    assert!(
        rows.iter()
            .all(|row| row.runs.as_deref() == Some("skills.studio:persona/ops")),
        "every row opens the studio on the persona's chats"
    );
    assert_eq!(empty.offer.as_deref(), Some("skills.studio:persona/ops"));
}

#[test]
fn a_persona_with_no_skills_says_so_and_offers_the_studio() {
    let plane = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(plane.path().join("personas/ops")).unwrap();
    std::fs::write(plane.path().join("personas/ops/persona.md"), "---\n---\n").unwrap();

    let blocks = blocks(plane.path(), "ops", None);

    assert!(matches!(
        blocks.as_slice(),
        [panel::Block::Note { text, .. }, panel::Block::List { rows, .. }]
            if text == "It brings 0 skills into every workspace." && rows.is_empty()
    ));
}
