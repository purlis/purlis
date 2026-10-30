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

#[test]
fn each_source_says_when_it_was_last_checked_and_its_counts_without_asking_it() {
    // #3840: what the last browse recorded, and none for a source nothing has checked yet.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("charter.toml"),
        "schema = 1\n\n[[skill_sources]]\nurl = \"https://github.com/obra/superpowers\"\n\n\
         [[skill_sources]]\nurl = \"https://git.example.com/platform/skills\"\nbranch = \"main\"\n",
    )
    .unwrap();
    let superpowers = Remote {
        url: "https://github.com/obra/superpowers".into(),
        branch: None,
    };
    source::checked::browsed(tmp.path(), &superpowers, 14);

    let shown = checked_shown(tmp.path()).unwrap();

    assert_eq!(shown.len(), 2, "{shown:?}");
    assert_eq!(shown[0].remote.url, superpowers.url);
    assert!(shown[0].at.is_some());
    assert_eq!(shown[0].skills, Some(14));
    assert_eq!(shown[0].installed, 0);
    assert_eq!(shown[1].remote.branch.as_deref(), Some("main"));
    assert_eq!((shown[1].at, shown[1].skills), (None, None));
}
