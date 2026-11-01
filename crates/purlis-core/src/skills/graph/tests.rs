// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

fn skill(root: &Path, folder: &str, name: &str, body: &str) {
    std::fs::create_dir_all(root.join(folder)).unwrap();
    std::fs::write(
        root.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name} for the team.\n---\n\n{body}\n"),
    )
    .unwrap();
}

#[test]
fn a_skill_is_shown_what_it_uses_and_every_level_that_uses_it() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    std::fs::write(
        root.join("personas/steward/persona.md"),
        "---\nname: steward\n---\n",
    )
    .unwrap();
    skill(
        root,
        "skills/deploy",
        "deploy",
        "Then skills:notify and skills:gone, and purlis:purlis-handoff.",
    );
    skill(root, "skills/notify", "notify", "Says it.");
    skill(
        root,
        "workspaces/ops/skills/release",
        "release",
        "Run skills:deploy.",
    );
    skill(
        root,
        "personas/steward/skills/daily",
        "daily",
        "Every day skills:deploy.",
    );
    let built_in = tempfile::tempdir().unwrap();
    skill(
        built_in.path(),
        "purlis-handoff",
        "purlis-handoff",
        "Hands off.",
    );

    let deploy = uses_of(root, Some(built_in.path()), "deploy");

    let uses: Vec<(&str, bool)> = deploy
        .uses
        .iter()
        .map(|one| (one.said.as_str(), one.broken))
        .collect();
    assert_eq!(
        uses,
        [
            ("skills:notify", false),
            ("skills:gone", true),
            ("purlis:purlis-handoff", false)
        ]
    );
    assert_eq!(deploy.used_by, ["daily", "release"]);
    assert!(!deploy.cut);
    assert_eq!(
        uses_of(root, Some(built_in.path()), "notify").used_by,
        ["deploy"]
    );
}
