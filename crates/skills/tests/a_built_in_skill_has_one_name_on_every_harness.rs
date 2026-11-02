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
