// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_project_is_offered_anthropics_skills_until_it_browses_them() {
    // SK-22 (#2785): the offered sources, as the window draws them, each kept with the project
    // unless the person picks this machine in Add source.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();

    let offered = offered_shown(tmp.path());
    let anthropic = offered
        .iter()
        .find(|one| one.source.remote.url == "https://github.com/anthropics/skills")
        .expect("Anthropic's skills are offered");
    assert_eq!(anthropic.offering, SourceOffering::WellKnown);
    assert_eq!(anthropic.source.folder.as_deref(), Some("skills"));
    assert_eq!(anthropic.source.kept, SourceKept::Shared);
    assert!(!anthropic.why.is_empty());

    std::fs::write(
        tmp.path().join("charter.toml"),
        "schema = 1\n\n[[skill_sources]]\nurl = \"https://github.com/anthropics/skills\"\nfolder = \"skills\"\n",
    )
    .unwrap();
    assert!(
        offered_shown(tmp.path())
            .iter()
            .all(|one| one.source.remote.url != "https://github.com/anthropics/skills")
    );
}
