// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam: what a chat's skill set holds (ADR 0099 as amended 2026-11-01). A built-in skill is
//! `purlis-<name>` on every route, the old names still read as it, the prefix is purlis's, and a
//! plane skill may have any other name.

use std::path::{Path, PathBuf};

use purlis_skills::{Chat, HarnessSkills, Machine, Reason, Reference, Route, SkillSet, skill_set};

const ROUTES: [Route; 4] = [Route::Plugin, Route::Config, Route::Briefing, Route::Mcp];

fn skill(dir: &Path, folder: &str) {
    let text = format!("---\nname: {folder}\ndescription: Does {folder}.\n---\n\nbody\n");
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

/// A plane at `root` whose project grants nothing, and a bundle holding `purlis-handoff`.
fn bundle(root: &Path) -> PathBuf {
    let built_in = root.join("plugin/skills");
    skill(&built_in, "purlis-handoff");
    built_in
}

fn set_for(root: &Path, built_in: &Path, names: &[Reference], route: Route) -> SkillSet {
    skill_set(
        Some(root),
        &Machine {
            built_in: Some(built_in),
            ..Machine::default()
        },
        &Chat {
            workspace: None,
            persona: Some("ops"),
            names,
            turned_off: &[],
            harness: HarnessSkills { route },
        },
    )
}

fn names_of(set: &SkillSet) -> Vec<&str> {
    set.effective
        .iter()
        .map(|one| one.skill.name.as_str())
        .collect()
}

#[test]
fn a_built_in_skill_has_its_one_name_on_every_route() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    for route in ROUTES {
        let set = set_for(tmp.path(), &built_in, &[], route);
        assert_eq!(names_of(&set), ["purlis-handoff"], "{route:?}");
    }
}

#[test]
fn a_plane_skill_with_a_built_in_skills_old_name_loads_on_every_route() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    skill(&tmp.path().join("skills"), "handoff");
    for route in ROUTES {
        let set = set_for(tmp.path(), &built_in, &[], route);
        assert_eq!(names_of(&set), ["handoff", "purlis-handoff"], "{route:?}");
        assert!(set.left_out.is_empty(), "{route:?}: {:?}", set.left_out);
    }
}

#[test]
fn a_plane_skill_named_with_the_prefix_is_left_out_saying_so_and_stays_on_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    let plane = tmp.path().join("skills");
    // One a built-in skill has the name of, and one no built-in skill has: the prefix decides.
    skill(&plane, "purlis-handoff");
    skill(&plane, "purlis-mine");
    for route in ROUTES {
        let set = set_for(tmp.path(), &built_in, &[], route);
        assert_eq!(names_of(&set), ["purlis-handoff"], "{route:?}");
        let left: Vec<(&str, &Reason)> = set
            .left_out
            .iter()
            .map(|one| (one.name.as_str(), &one.reason))
            .collect();
        assert_eq!(
            left,
            [
                ("purlis-handoff", &Reason::ReservedName),
                ("purlis-mine", &Reason::ReservedName),
            ],
            "{route:?}"
        );
    }
    assert!(plane.join("purlis-mine/SKILL.md").is_file());
    assert_eq!(
        Reason::ReservedName.to_string(),
        "a name starting with purlis- is purlis's own, so this skill never loads: rename it"
    );
}

#[test]
fn a_persona_that_names_a_built_in_skill_by_its_old_name_gets_it() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    for old in ["purlis:handoff", "charter:handoff", "purlis:purlis-handoff"] {
        let reference = Reference::parse(old).unwrap();
        assert_eq!(reference.to_string(), "purlis:purlis-handoff", "{old}");
        for route in ROUTES {
            let set = set_for(
                tmp.path(),
                &built_in,
                std::slice::from_ref(&reference),
                route,
            );
            assert_eq!(names_of(&set), ["purlis-handoff"], "{old} {route:?}");
            assert!(
                set.left_out.is_empty(),
                "{old} {route:?}: {:?}",
                set.left_out
            );
        }
    }
}

#[test]
fn a_bare_old_name_is_an_ordinary_name_and_never_the_built_in_skill() {
    assert_eq!(
        Reference::parse("handoff"),
        Some(Reference::Named("handoff".to_owned()))
    );
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    let set = set_for(tmp.path(), &built_in, &[], Route::Plugin);
    assert!(purlis_skills::by_name(&set, "handoff").is_none());
    skill(&tmp.path().join("skills"), "handoff");
    let set = set_for(tmp.path(), &built_in, &[], Route::Plugin);
    assert_eq!(
        purlis_skills::by_name(&set, "handoff").map(|one| one.origin),
        Some(purlis_skills::Origin::Project)
    );
}

#[test]
fn a_name_given_with_the_namespace_finds_the_built_in_skill_even_beside_a_plane_skill_of_the_old_name()
 {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    skill(&tmp.path().join("skills"), "handoff");
    let set = set_for(tmp.path(), &built_in, &[], Route::Plugin);
    for given in [
        "purlis-handoff",
        "purlis:purlis-handoff",
        "purlis:handoff",
        "charter:handoff",
    ] {
        assert_eq!(
            purlis_skills::by_name(&set, given).map(|one| one.origin),
            Some(purlis_skills::Origin::BuiltIn),
            "{given}"
        );
    }
}

#[test]
fn only_a_purlis_prefixed_name_is_reserved() {
    for reserved in [
        "purlis-handoff",
        "purlis-mine",
        "purlis-skill-index",
        "PURLIS-x",
    ] {
        assert!(purlis_skills::name_reserved(reserved), "{reserved}");
    }
    for free in ["handoff", "skill-index", "purlish", "my-purlis-notes"] {
        assert!(!purlis_skills::name_reserved(free), "{free}");
    }
}

/// Every name `purlis-building-ui` had while purlis shipped it: its own, bare or under either
/// prefix, and the longer one before it (ADR 0099 as amended 2026-11-01).
const BUILDING_NAMES: [&str; 9] = [
    "purlis-building-ui",
    "purlis:purlis-building-ui",
    "purlis:building-ui",
    "charter:purlis-building-ui",
    "purlis-building-purlis-ui",
    "purlis:purlis-building-purlis-ui",
    "purlis:building-purlis-ui",
    "charter:building-purlis-ui",
    "charter:purlis-building-purlis-ui",
];

/// #5725: the person retired `purlis-building-ui` on 2026-11-12, with no skill in its place.
/// Each name it had still reads as it, so whoever names it is told it was removed, and a run
/// recorded under one keeps the skill's name; no chat's set has it.
#[test]
fn every_name_of_the_retired_building_skill_reads_as_retired_and_its_runs_keep_its_name_5725() {
    let tmp = tempfile::tempdir().unwrap();
    let built_in = bundle(tmp.path());
    for name in BUILDING_NAMES {
        let reference = Reference::parse(name).expect("a reference");
        assert_eq!(
            reference.retired().map(|(skill, _)| skill),
            Some("purlis-building-ui"),
            "{name}"
        );
        assert_eq!(
            purlis_skills::run_name(name),
            "purlis-building-ui",
            "{name}"
        );
        for route in ROUTES {
            let set = set_for(
                tmp.path(),
                &built_in,
                std::slice::from_ref(&reference),
                route,
            );
            assert!(
                !names_of(&set).contains(&"purlis-building-ui"),
                "{name} {route:?}"
            );
            assert!(
                purlis_skills::by_name(&set, name).is_none(),
                "{name} {route:?}"
            );
        }
    }
    for other in [
        "purlis:purlis-handoff",
        "building-ui",
        "building-purlis-ui",
        "acme:purlis-building-ui",
    ] {
        assert_eq!(Reference::parse(other).unwrap().retired(), None, "{other}");
    }
}

#[test]
fn a_bare_old_name_without_the_prefix_stays_an_ordinary_name() {
    assert_eq!(
        Reference::parse("building-purlis-ui"),
        Some(Reference::Named("building-purlis-ui".to_owned()))
    );
}

/// Four built-in skills took clearer names (#5724, spec #5714 Q32): each old name, bare or under
/// either prefix, reads as the new one, so a persona's list, a run and a miss carry over.
const RENAMED_BY_5724: [(&str, &str); 4] = [
    ("purlis-compact", "purlis-tidy-memory"),
    ("purlis-working-in-a-clone", "purlis-repo-work"),
    ("purlis-update", "purlis-upgrade"),
    ("purlis-add-curation-action", "purlis-curation-action"),
];

#[test]
fn every_old_name_of_the_four_renamed_skills_reads_as_its_name_now_5724() {
    for (old, new) in RENAMED_BY_5724 {
        let short = old.strip_prefix("purlis-").unwrap();
        for written in [
            old.to_owned(),
            format!("purlis:{old}"),
            format!("purlis:{short}"),
            format!("charter:{short}"),
            format!("charter:{old}"),
        ] {
            assert_eq!(
                Reference::parse(&written),
                Some(Reference::BuiltIn(new.to_owned())),
                "{written}"
            );
            assert_eq!(purlis_skills::run_name(&written), new, "{written}");
            assert_eq!(
                Reference::written_renamed(&written),
                Some((old, new)),
                "{written}"
            );
        }
        assert_eq!(purlis_skills::name_now(old), new);
        assert_eq!(purlis_skills::name_now(new), new);
        assert_eq!(Reference::written_renamed(&format!("purlis:{new}")), None);
    }
    assert_eq!(purlis_skills::name_now("deploy"), "deploy");
}

#[test]
fn an_old_name_in_a_prompt_is_found_only_as_a_whole_name_5724() {
    assert_eq!(
        purlis_skills::old_names_in("Use the purlis-compact skill, then purlis:update."),
        vec![
            ("purlis-compact", "purlis-tidy-memory"),
            ("purlis-update", "purlis-upgrade"),
        ]
    );
    assert!(purlis_skills::old_names_in("Use purlis-updater and purlis-tidy-memory.").is_empty());
}
