// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam: what a chat's skill set is, and why. Every test builds a plane and a machine in a
//! temporary directory and reads only what the set says.

use std::path::{Path, PathBuf};

use purlis_skills::{Chat, HarnessSkills, Machine, Origin, Route, SkillSet, skill_set};

/// A bundle as the app ships it: a plugin directory whose `skills/` holds a folder per skill.
fn bundle(root: &Path) -> PathBuf {
    let plugin = root.join("plugin");
    std::fs::create_dir_all(plugin.join("skills")).unwrap();
    plugin
}

fn skill(dir: &Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

fn whole(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n")
}

fn chat(route: Route) -> Chat<'static> {
    Chat {
        workspace: Some("smart-ide"),
        persona: Some("steward"),
        names: &[],
        turned_off: &[],
        harness: HarnessSkills { route },
    }
}

fn set_for(plane: &Path, plugin: &Path, route: Route) -> SkillSet {
    skill_set(
        Some(plane),
        &Machine {
            built_in: Some(&plugin.join("skills")),
            ..Machine::default()
        },
        &chat(route),
    )
}

#[test]
fn every_built_in_skill_reaches_the_chat_by_its_harness_route() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = bundle(tmp.path());
    skill(&plugin.join("skills"), "secrets", &whole("secrets"));
    skill(&plugin.join("skills"), "compact", &whole("compact"));

    for route in [Route::Plugin, Route::Config, Route::Briefing] {
        let set = set_for(tmp.path(), &plugin, route);

        let got: Vec<(&str, Route, Origin)> = set
            .effective
            .iter()
            .map(|e| (e.skill.name.as_str(), e.route, e.origin))
            .collect();
        // The config route names each its own, `purlis-<name>` (opencode's route).
        let (compact, secrets) = if route == Route::Config {
            ("purlis-compact", "purlis-secrets")
        } else {
            ("compact", "secrets")
        };
        assert_eq!(
            got,
            [
                (compact, route, Origin::BuiltIn),
                (secrets, route, Origin::BuiltIn),
            ],
            "listed by name, each on the harness's route"
        );
        assert_eq!(
            set.effective[0].skill.path,
            plugin.join("skills/compact/SKILL.md")
        );
        assert!(set.left_out.is_empty(), "{:?}", set.left_out);
    }
}

#[test]
fn a_built_in_folder_that_is_not_a_whole_skill_is_left_out_with_a_one_line_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = bundle(tmp.path());
    let dir = plugin.join("skills");
    skill(&dir, "whole", &whole("whole"));
    skill(&dir, "nameless", "---\ndescription: Does it.\n---\n");
    skill(&dir, "undescribed", "---\nname: undescribed\n---\n");
    skill(&dir, "no-frontmatter", "# just a page\n");
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    // A file beside the folders is not a skill folder, and not a skill left out either.
    std::fs::write(dir.join("README.md"), "the skills\n").unwrap();

    let set = set_for(tmp.path(), &plugin, Route::Plugin);

    let names: Vec<&str> = set
        .effective
        .iter()
        .map(|e| e.skill.name.as_str())
        .collect();
    assert_eq!(names, ["whole"]);
    let left: Vec<(&str, Origin, String)> = set
        .left_out
        .iter()
        .map(|l| (l.name.as_str(), l.origin, l.reason.to_string()))
        .collect();
    assert_eq!(
        left,
        [
            (
                "empty",
                Origin::BuiltIn,
                "not a skill: its folder has no SKILL.md".to_owned()
            ),
            (
                "nameless",
                Origin::BuiltIn,
                "not a skill: its SKILL.md names no `name`".to_owned()
            ),
            (
                "no-frontmatter",
                Origin::BuiltIn,
                "not a skill: its SKILL.md names no `name`".to_owned()
            ),
            (
                "undescribed",
                Origin::BuiltIn,
                "not a skill: its SKILL.md names no `description`".to_owned()
            ),
        ]
    );
    for left in &set.left_out {
        assert!(!left.reason.to_string().contains('\n'), "one line");
    }
    assert_eq!(set.left_out[0].folder, dir.join("empty"));
}

#[test]
fn a_machine_without_the_bundle_gives_a_chat_no_skills_and_leaves_none_out() {
    let tmp = tempfile::tempdir().unwrap();

    for built_in in [None, Some(tmp.path().join("no-such-plugin/skills"))] {
        let set = skill_set(
            Some(tmp.path()),
            &Machine {
                built_in: built_in.as_deref(),
                ..Machine::default()
            },
            &chat(Route::Config),
        );

        assert!(set.effective.is_empty());
        assert!(set.left_out.is_empty());
        assert_eq!(set.built_in_dir(), None);
    }
}

#[test]
fn the_planes_harness_folders_do_not_change_the_set() {
    // The plane's `.claude/skills` is the harness's to read, never purlis's to hand over. The
    // project's `skills/` is `a_project_skill_reaches_chats_at_the_plane_root.rs`'s, and the
    // workspace's and the persona's are `src/tests.rs`'s (#2767, #2768).
    let tmp = tempfile::tempdir().unwrap();
    let plugin = bundle(tmp.path());
    skill(&plugin.join("skills"), "secrets", &whole("secrets"));
    let plane = tmp.path().join("plane");
    skill(&plane.join(".claude/skills"), "house", &whole("house"));

    let with_plane = set_for(&plane, &plugin, Route::Briefing);
    let without = skill_set(
        None,
        &Machine {
            built_in: Some(&plugin.join("skills")),
            ..Machine::default()
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Briefing,
            },
        },
    );

    let names = |set: &SkillSet| -> Vec<String> {
        set.effective.iter().map(|e| e.skill.name.clone()).collect()
    };
    assert_eq!(names(&with_plane), ["secrets"]);
    assert_eq!(names(&with_plane), names(&without));
    assert!(with_plane.left_out.is_empty());
}

#[test]
fn the_set_names_the_directory_its_skills_are_handed_over_in() {
    // What a harness on the config or briefing route is told: the folder the skills sit in.
    let tmp = tempfile::tempdir().unwrap();
    let plugin = bundle(tmp.path());
    skill(&plugin.join("skills"), "secrets", &whole("secrets"));
    skill(&plugin.join("skills"), "compact", &whole("compact"));

    let set = set_for(tmp.path(), &plugin, Route::Config);

    assert_eq!(set.built_in_dir(), Some(plugin.join("skills").as_path()));
}
