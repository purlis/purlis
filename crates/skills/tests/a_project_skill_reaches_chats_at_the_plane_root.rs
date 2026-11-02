// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam: what a chat's skill set is, and why, once the plane holds project skills (SK-2,
//! ADR 0099). Every test builds a plane and a machine in a temporary directory and reads only
//! what the set says.

use std::path::{Path, PathBuf};

use purlis_skills::{Chat, HarnessSkills, Machine, Origin, Route, SkillSet, skill_set};

fn skill(dir: &Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

fn whole(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n")
}

/// A plane and a bundle side by side: `plane/` with its `skills/`, `plugin/skills/`.
struct Here {
    _tmp: tempfile::TempDir,
    plane: PathBuf,
    built_in: PathBuf,
}

fn here() -> Here {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let built_in = tmp.path().join("plugin/skills");
    std::fs::create_dir_all(plane.join("skills")).unwrap();
    std::fs::create_dir_all(&built_in).unwrap();
    Here {
        _tmp: tmp,
        plane,
        built_in,
    }
}

fn set_in(here: &Here, workspace: Option<&str>, route: Route) -> SkillSet {
    skill_set(
        Some(&here.plane),
        &Machine {
            built_in: Some(&here.built_in),
            ..Machine::default()
        },
        &Chat {
            workspace,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills { route },
        },
    )
}

fn effective(set: &SkillSet) -> Vec<(&str, Origin)> {
    set.effective
        .iter()
        .map(|e| (e.skill.name.as_str(), e.origin))
        .collect()
}

fn left_out(set: &SkillSet) -> Vec<(&str, Origin, String)> {
    set.left_out
        .iter()
        .map(|l| (l.name.as_str(), l.origin, l.reason.to_string()))
        .collect()
}

#[test]
fn a_project_skill_reaches_a_chat_at_the_plane_root_on_every_route() {
    let here = here();
    skill(&here.built_in, "secrets", &whole("secrets"));
    skill(&here.plane.join("skills"), "deploy", &whole("deploy"));

    for route in [Route::Plugin, Route::Config, Route::Briefing] {
        let set = set_in(&here, None, route);

        assert_eq!(
            effective(&set),
            [("deploy", Origin::Project), ("secrets", Origin::BuiltIn)]
        );
        let deploy = &set.effective[0];
        assert_eq!(deploy.route, route);
        assert_eq!(
            deploy.skill.path,
            here.plane.join("skills/deploy/SKILL.md"),
            "the plane's own file, never a copy"
        );
        assert!(set.left_out.is_empty(), "{:?}", set.left_out);
    }
}

#[test]
fn a_chat_in_a_workspace_is_told_a_project_skill_was_not_granted_to_it() {
    let here = here();
    skill(&here.plane.join("skills"), "deploy", &whole("deploy"));

    let set = set_in(&here, Some("smart-ide"), Route::Plugin);

    assert!(set.effective.is_empty(), "{:?}", set.effective);
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            "not granted to this workspace: a project skill is for chats at the project's root \
             until the project grants it"
                .to_owned()
        )]
    );
}

#[test]
fn a_project_skill_with_a_built_in_skills_name_is_left_out_and_the_built_in_one_stays() {
    let here = here();
    skill(&here.built_in, "purlis-handoff", &whole("purlis-handoff"));
    // Its folder may be named anything: the name is what its `SKILL.md` says.
    skill(
        &here.plane.join("skills"),
        "my-handoff",
        &whole("purlis-handoff"),
    );

    let set = set_in(&here, None, Route::Config);

    assert_eq!(effective(&set), [("purlis-handoff", Origin::BuiltIn)]);
    assert_eq!(
        set.effective[0].skill.path,
        here.built_in.join("purlis-handoff/SKILL.md")
    );
    assert_eq!(
        left_out(&set),
        [(
            "purlis-handoff",
            Origin::Project,
            "a name starting with purlis- is purlis's own, so this skill never loads: rename it"
                .to_owned()
        )]
    );
}

#[test]
fn a_project_folder_that_is_not_a_whole_skill_is_left_out_with_what_it_lacks() {
    let here = here();
    let dir = here.plane.join("skills");
    skill(&dir, "nameless", "---\ndescription: Does it.\n---\n");
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    // purlis's own file beside a folder is not a folder, and not a skill left out.
    std::fs::write(dir.join("nameless.toml"), "grant = \"all\"\n").unwrap();

    let set = set_in(&here, None, Route::Plugin);

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [
            (
                "empty",
                Origin::Project,
                "not a skill: its folder has no SKILL.md".to_owned()
            ),
            (
                "nameless",
                Origin::Project,
                "not a skill: its SKILL.md names no `name`".to_owned()
            ),
        ]
    );
}

#[test]
fn a_chat_with_no_plane_gets_the_built_in_skills_alone() {
    let here = here();
    skill(&here.built_in, "secrets", &whole("secrets"));
    skill(&here.plane.join("skills"), "deploy", &whole("deploy"));

    let set = skill_set(
        None,
        &Machine {
            built_in: Some(&here.built_in),
            ..Machine::default()
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Plugin,
            },
        },
    );

    assert_eq!(effective(&set), [("secrets", Origin::BuiltIn)]);
}

#[test]
fn a_plane_without_skills_and_a_machine_without_built_in_skills_give_an_empty_set() {
    let tmp = tempfile::tempdir().unwrap();

    let set = skill_set(
        Some(tmp.path()),
        &Machine::default(),
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

    assert_eq!(set, SkillSet::default());
}

#[test]
fn of_two_project_folders_that_name_one_skill_the_first_by_folder_name_is_handed_over() {
    // One copy of a name reaches a harness, always the same one (ADR 0099 §3): opencode would
    // otherwise pick either at random.
    let here = here();
    skill(&here.plane.join("skills"), "b-deploy", &whole("deploy"));
    skill(&here.plane.join("skills"), "a-deploy", &whole("deploy"));

    let set = set_in(&here, None, Route::Config);

    assert_eq!(effective(&set), [("deploy", Origin::Project)]);
    assert_eq!(
        set.effective[0].skill.path,
        here.plane.join("skills/a-deploy/SKILL.md")
    );
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            "hidden by another project skill of this name".to_owned()
        )]
    );
    assert_eq!(set.left_out[0].folder, here.plane.join("skills/b-deploy"));
}
