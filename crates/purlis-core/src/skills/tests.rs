// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::machine::Flags;

const ON: Flags = Flags { skill_studio: true };

#[test]
fn the_skill_studio_flag_is_off_until_this_machine_turns_it_on() {
    let unset = |_: &str| None;
    assert!(!studio_on(&Flags::default(), &unset));
    assert!(studio_on(&ON, &unset));
}

#[test]
fn turning_the_skill_studio_on_is_kept_in_this_machines_store_and_off_again() {
    let config = tempfile::tempdir().unwrap();

    set_studio_on(config.path(), true).unwrap();
    assert!(crate::machine::read(config.path()).store.flags.skill_studio);
    set_studio_on(config.path(), false).unwrap();
    assert!(!crate::machine::read(config.path()).store.flags.skill_studio);
}

#[test]
fn the_variable_turns_the_skill_studio_flag_on_or_off_over_what_the_machine_says() {
    let set =
        |value: &'static str| move |name: &str| (name == STUDIO_ENV).then(|| value.to_owned());
    for flags in [Flags::default(), ON] {
        assert!(studio_on(&flags, &set("1")), "{flags:?}");
        assert!(!studio_on(&flags, &set("0")), "{flags:?}");
        // A value it does not know leaves the machine to decide.
        assert_eq!(
            studio_on(&flags, &set("yes")),
            flags.skill_studio,
            "{flags:?}"
        );
    }
    // Under its old name too, as every product variable is read.
    let old = |name: &str| (name == "CHARTER_SKILL_STUDIO").then(|| "1".to_owned());
    assert!(studio_on(&Flags::default(), &old));
}

#[test]
fn a_chat_is_handed_every_built_in_skill_by_its_harness_route() {
    let plugin = crate::harness::testing::bundled_plugin();
    let kit = Kit {
        skills: None,
        binary: Path::new("/bin/purlis"),
        plugin: Some(&plugin),
        persona: None,
    };
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let set = for_chat(kit, harness);

        assert!(
            set.effective.iter().any(|e| e.skill.name == "secrets"),
            "{harness:?}"
        );
        assert!(
            set.effective
                .iter()
                .all(|e| e.route == harness.skills() && e.origin == Origin::BuiltIn),
            "{harness:?}"
        );
        assert!(set.left_out.is_empty(), "{:?}", set.left_out);
        assert_eq!(set.built_in_dir(), Some(plugin.join("skills").as_path()));
    }
}

#[test]
fn only_a_chat_started_with_the_variable_is_briefed_on_its_skills() {
    let root = Path::new("/plane");
    let unset = |_: &str| None;
    assert_eq!(briefed(root, None, &unset), None);
    let empty = |name: &str| (name == LISTED_ENV).then(String::new);
    assert_eq!(briefed(root, None, &empty), None);

    let skills = crate::harness::testing::bundled_plugin().join("skills");
    let dir = skills.display().to_string();
    let set = move |name: &str| (name == LISTED_ENV).then(|| dir.clone());
    let text = briefed(root, None, &set).expect("listed");

    assert!(text.contains("`safe-remove`"), "{text}");
    assert!(text.contains(&skills.join("safe-remove/SKILL.md").display().to_string()));
}

#[test]
fn a_briefing_lists_what_the_chats_set_holds_and_not_a_folder_it_leaves_out() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    for (folder, text) in [
        ("kept", "---\nname: kept\ndescription: Is kept.\n---\n"),
        ("undescribed", "---\nname: undescribed\n---\n"),
    ] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
    }
    let named = dir.display().to_string();
    let env = move |name: &str| (name == LISTED_ENV).then(|| named.clone());

    let text = briefed(tmp.path(), None, &env).expect("listed");

    assert!(text.contains("- `kept` — Is kept."), "{text}");
    assert!(!text.contains("undescribed"), "{text}");
}

/// A plane for the grant and Share orchestration (#2767): its `charter.toml`, a workspace
/// `smart-ide`, and a project skill `deploy`. No git repository: a local skill in it is read.
fn granting_plane() -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(plane.path().join("workspaces/smart-ide")).unwrap();
    let deploy = plane.path().join("skills/deploy");
    std::fs::create_dir_all(&deploy).unwrap();
    std::fs::write(
        deploy.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships it.\n---\n",
    )
    .unwrap();
    plane
}

fn gets(plane: &Path, workspace: Option<&str>) -> Vec<String> {
    in_the_studio(None, plane, workspace)
        .effective
        .into_iter()
        .map(|e| e.skill.name)
        .collect()
}

#[test]
fn a_grant_reaches_the_workspace_it_names_and_the_studio_shows_it() {
    let plane = granting_plane();
    assert!(gets(plane.path(), Some("smart-ide")).is_empty());

    let folder = grant(
        plane.path(),
        "deploy",
        &Grant::Only(vec!["smart-ide".to_owned()]),
    )
    .unwrap();

    assert_eq!(folder, plane.path().join("skills/deploy"));
    assert_eq!(gets(plane.path(), Some("smart-ide")), ["deploy"]);
}

#[test]
fn a_grant_to_a_workspace_the_plane_does_not_have_is_refused_and_writes_nothing() {
    let plane = granting_plane();

    let refused = grant(
        plane.path(),
        "deploy",
        &Grant::Only(vec!["smart-id".into()]),
    )
    .unwrap_err();

    assert!(refused.contains("no workspace \"smart-id\""), "{refused}");
    assert!(!plane.path().join("skills/deploy.toml").exists());
}

#[test]
fn a_workspace_turns_a_granted_skill_off_and_back_on_in_its_own_settings() {
    let plane = granting_plane();
    grant(plane.path(), "deploy", &Grant::All).unwrap();

    assert_eq!(switch(plane.path(), "smart-ide", "deploy", false), Ok(true));
    assert!(gets(plane.path(), Some("smart-ide")).is_empty());
    let manifest =
        std::fs::read_to_string(plane.path().join("workspaces/smart-ide/workspace.json")).unwrap();
    assert!(manifest.contains("\"off\""), "{manifest}");
    assert_eq!(
        switch(plane.path(), "smart-ide", "deploy", false),
        Ok(false),
        "already off: nothing is written"
    );

    assert_eq!(switch(plane.path(), "smart-ide", "deploy", true), Ok(true));
    assert_eq!(gets(plane.path(), Some("smart-ide")), ["deploy"]);
    assert!(turned_off(plane.path(), "smart-ide").is_empty());
}

#[test]
fn a_workspace_cannot_turn_on_a_skill_the_project_did_not_grant_it_and_is_told_why() {
    let plane = granting_plane();

    let refused = switch(plane.path(), "smart-ide", "deploy", true).unwrap_err();

    assert_eq!(
        refused,
        "deploy is not granted to smart-ide, so it cannot be turned on there: a workspace only \
         turns off what the project granted it. The project grants it at its root: purlis skill \
         grant deploy smart-ide"
    );
    assert!(
        !plane
            .path()
            .join("workspaces/smart-ide/workspace.json")
            .exists()
    );
}

#[test]
fn a_local_skill_in_a_plane_that_is_no_git_repository_is_read_and_shared() {
    let plane = granting_plane();
    let scratch = plane.path().join("skills.local/scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("SKILL.md"),
        "---\nname: scratch\ndescription: Tries it.\n---\n",
    )
    .unwrap();
    let set = in_the_studio(None, plane.path(), None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && e.local),
        "{set:?}"
    );

    let moved = share_here(plane.path(), None, "scratch").unwrap();

    assert_eq!(moved, plane.path().join("skills/scratch"));
    let set = in_the_studio(None, plane.path(), None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && !e.local)
    );
}

#[test]
fn a_local_skill_is_read_only_while_git_ignores_it() {
    // The live/local rule of ADR 0048, for `skills.local/` (#2767): one git would commit is left
    // out with git's answer and the fix; ignored, it is in the set.
    let plane = granting_plane();
    let init = crate::testgit::run(plane.path(), &["init", "-q"]);
    assert_eq!(init.code, Some(0), "git init: {}", init.err);
    let scratch = plane.path().join("skills.local/scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("SKILL.md"),
        "---\nname: scratch\ndescription: Tries it.\n---\n",
    )
    .unwrap();

    let set = in_the_studio(None, plane.path(), None);
    let left = set
        .left_out
        .iter()
        .find(|l| l.name == "scratch")
        .expect("left out");
    assert_eq!(
        left.reason.to_string(),
        "a local skill is read only while git keeps it on this machine, and git would commit \
         skills.local/scratch/SKILL.md: add /skills.local/ to the plane's .gitignore"
    );

    std::fs::write(plane.path().join(".gitignore"), "/skills.local/\n").unwrap();
    let set = in_the_studio(None, plane.path(), None);
    assert!(
        set.effective
            .iter()
            .any(|e| e.skill.name == "scratch" && e.local),
        "{set:?}"
    );
}
