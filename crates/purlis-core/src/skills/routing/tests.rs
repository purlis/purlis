// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat whose skills outgrow its budget (SK-15, #2778; ADR 0102), through purlis-core: what its
//! harness is handed, what its pins keep listed, what the studio and Use skill… say, and that
//! nothing hidden is out of reach of `skill_find` and `skill_load`.

use std::path::{Path, PathBuf};

use super::*;
use crate::harness::Harness;
use crate::skills::{
    ChatSkills, DIR_IN_BUNDLE, INDEX, Running, for_use_skill, in_the_studio, of_a_chat, studio_rows,
};

/// The most skills a chat lists on every harness (ADR 0102 as amended 2026-10-21).
const MOST: usize = 30;

fn write(dir: &Path, text: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
}

/// A plane with a workspace `smart-ide`, a persona `reviewer`, one built-in skill and `count`
/// project skills granted to every workspace, `skill-00` to `skill-<count - 1>`.
fn a_plane_of(count: usize) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join("workspaces/smart-ide")).unwrap();
    std::fs::create_dir_all(root.join("personas/reviewer")).unwrap();
    std::fs::write(
        root.join("personas/reviewer/persona.md"),
        "---\nname: reviewer\nrole: Reviews.\n---\n\nReviews.\n",
    )
    .unwrap();
    for at in 0..count {
        let name = format!("skill-{at:02}");
        write(
            &root.join("skills").join(&name),
            &format!("---\nname: {name}\ndescription: Does thing {at}.\n---\n\nbody\n"),
        );
        std::fs::write(
            root.join("skills").join(format!("{name}.toml")),
            "grant = \"all\"\n",
        )
        .unwrap();
    }
    let bundle = root.join("bundle");
    write(
        &bundle.join("handoff"),
        "---\nname: handoff\ndescription: Hands off.\n---\n",
    );
    (tmp, bundle)
}

fn running<'a>(root: &'a Path, bundle: &'a Path, persona: Option<&'a str>) -> Running<'a> {
    Running {
        plane: root,
        workspace: Some("smart-ide"),
        persona,
        harness: Harness::ClaudeCode,
        built_in: Some(bundle),
        plane_skills: true,
    }
}

fn hidden(skills: &ChatSkills) -> Vec<&str> {
    skills
        .offered
        .iter()
        .filter(|one| one.hidden.is_some())
        .map(|one| one.effective.skill.name.as_str())
        .collect()
}

/// The set's folder laid out for a chat in `smart-ide`, as `persona` or on none.
fn the_folder(root: &Path, bundle: &Path, persona: Option<&str>) -> PathBuf {
    crate::skills::lay_out_for_chat(
        root,
        Some("smart-ide"),
        persona,
        Some(bundle),
        &root.join("sets"),
        Harness::ClaudeCode,
        &[],
    )
    .expect("a set's folder")
}

/// The names of the skills that folder holds.
fn laid_out(root: &Path, bundle: &Path, persona: Option<&str>) -> Vec<String> {
    let folder = the_folder(root, bundle, persona);
    let mut names: Vec<String> = std::fs::read_dir(folder.join(DIR_IN_BUNDLE))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn below_the_budget_a_chat_is_handed_every_skill_and_no_index() {
    let (tmp, bundle) = a_plane_of(MOST - 1);

    let names = laid_out(tmp.path(), &bundle, None);

    assert_eq!(names.len(), MOST - 1);
    assert!(!names.iter().any(|name| name == INDEX));
    assert!(hidden(&for_use_skill(&running(tmp.path(), &bundle, None))).is_empty());
}

#[test]
fn above_the_budget_a_chat_is_handed_the_index_in_place_of_the_hidden_skills() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();

    assert_eq!(laid_out(root, &bundle, None), [INDEX]);
    let index = std::fs::read_to_string(
        the_folder(root, &bundle, None)
            .join(DIR_IN_BUNDLE)
            .join(INDEX)
            .join("SKILL.md"),
    )
    .unwrap();
    assert!(index.contains("- `skill-00`: Does thing 0."), "{index}");
    assert!(index.contains("- `skill-34`: Does thing 34."), "{index}");
    let chat = for_use_skill(&running(root, &bundle, None));
    assert_eq!(hidden(&chat).len(), MOST + 5);
    assert!(chat.spent.over());
    assert!(!chat.pins_over);
}

#[test]
fn a_skill_the_router_hid_is_found_and_loaded_through_purlis_tools() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let asking = crate::skilltools::Asking {
        plane: root,
        workspace: Some("smart-ide"),
        persona: None,
        harness: Some("claude-code"),
        route: Some(crate::skills::Route::Plugin),
        built_in: Some(&bundle),
    };
    let args = |key: &str, value: &str| {
        let mut args = serde_json::Map::new();
        args.insert(key.to_owned(), serde_json::Value::from(value));
        args
    };

    let found =
        crate::skilltools::call(&asking, "skill_find", &args("query", "thing 7"), &|_| {}).unwrap();
    assert!(found.contains("`skill-07`"), "{found}");
    let started = std::cell::RefCell::new(Vec::new());
    let loaded =
        crate::skilltools::call(&asking, "skill_load", &args("name", "skill-07"), &|name| {
            started.borrow_mut().push(name.to_owned())
        })
        .unwrap();
    assert!(loaded.contains("Does thing 7."), "{loaded}");
    assert_eq!(*started.borrow(), ["skill-07"]);
}

#[test]
fn a_workspaces_pin_and_a_personas_pin_are_never_hidden() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let workspace = PinnedBy::Workspace("smart-ide".to_owned());
    let persona = PinnedBy::Persona("reviewer".to_owned());

    assert!(pin(root, &workspace, "skill-03", true).unwrap());
    assert!(pin(root, &persona, "skill-09", true).unwrap());
    assert!(!pin(root, &persona, "skill-09", true).unwrap());

    assert_eq!(pinned_in(root, "smart-ide"), ["skill-03"]);
    assert_eq!(
        crate::personaverbs::pinned_skills(root, "reviewer"),
        ["skill-09"]
    );
    assert_eq!(
        laid_out(root, &bundle, Some("reviewer")),
        ["skill-03", "skill-09", INDEX]
    );
    // A chat on no persona has the workspace's pin alone.
    assert_eq!(laid_out(root, &bundle, None), ["skill-03", INDEX]);
    let chat = for_use_skill(&running(root, &bundle, Some("reviewer")));
    assert!(!hidden(&chat).contains(&"skill-03"));
    assert!(!hidden(&chat).contains(&"skill-09"));

    assert!(pin(root, &workspace, "skill-03", false).unwrap());
    assert!(pinned_in(root, "smart-ide").is_empty());
}

#[test]
fn a_pin_of_no_skill_or_for_no_workspace_is_refused_and_writes_nothing() {
    let (tmp, _bundle) = a_plane_of(2);
    let root = tmp.path();

    let none = pin(
        root,
        &PinnedBy::Workspace("smart-ide".to_owned()),
        "ghost",
        true,
    )
    .unwrap_err();
    assert!(none.contains("no skill is named"), "{none}");
    let nowhere = pin(
        root,
        &PinnedBy::Workspace("nowhere".to_owned()),
        "skill-00",
        true,
    )
    .unwrap_err();
    assert!(nowhere.contains("no workspace"), "{nowhere}");
    assert!(!root.join("workspaces/smart-ide/workspace.json").exists());
}

#[test]
fn a_workspaces_pins_that_are_no_list_of_names_pin_nothing_and_a_save_refuses_them() {
    let (tmp, _bundle) = a_plane_of(2);
    let root = tmp.path();
    std::fs::write(
        root.join("workspaces/smart-ide/workspace.json"),
        r#"{"settings": {"skills": {"pinned": "skill-00"}}}"#,
    )
    .unwrap();

    assert!(pinned_in(root, "smart-ide").is_empty());
    let table: toml::Table = toml::from_str("[skills]\npinned = \"skill-00\"\n").unwrap();
    let refused = crate::settings::workspace::skills_refused(&table, "workspace.json");
    assert_eq!(refused.len(), 1, "{refused:?}");
    let table: toml::Table = toml::from_str("[skills]\npinned = [\"skill-00\"]\n").unwrap();
    assert!(crate::settings::workspace::skills_refused(&table, "workspace.json").is_empty());
}

#[test]
fn the_studio_says_why_the_router_hid_a_skill_and_which_are_pinned() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    pin(
        root,
        &PinnedBy::Workspace("smart-ide".to_owned()),
        "skill-03",
        true,
    )
    .unwrap();

    let set = in_the_studio(Some(&bundle), root, Some("smart-ide"), None);
    let rows = studio_rows(&set, root, &[]);

    let row = |name: &str| rows.iter().find(|row| row.name == name).expect(name);
    assert_eq!(
        row("skill-00").hidden.as_deref(),
        Some(
            "hidden by the built-in router: over this chat's skill budget, so it is in the index \
             skill"
        )
    );
    assert_eq!(row("skill-00").left_out, None, "hidden, never left out");
    assert!(row("skill-03").pinned);
    assert_eq!(row("skill-03").hidden, None);
    assert_eq!(
        row("handoff").hidden,
        None,
        "a built-in skill is never hidden"
    );
}

#[test]
fn use_skill_asks_a_chat_to_load_a_hidden_skill_and_holds_no_run_for_it() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    for harness in Harness::ALL {
        let chat = for_use_skill(&Running {
            harness,
            ..running(root, &bundle, None)
        });

        let typing = chat.typed("skill-07", &[]).unwrap();

        assert_eq!(
            typing.text, "Load the skill skill-07 with skill_load and follow it.",
            "{harness:?}"
        );
        assert!(typing.held.is_none(), "the load opens the run");
        let built_in = chat.typed("handoff", &[]).unwrap();
        assert!(built_in.held.is_some(), "a listed skill is typed as before");
    }
}

#[test]
fn the_stamp_of_a_chat_changes_when_what_its_index_names_changes() {
    let (tmp, _bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let before = crate::skills::stamp_of(root, Some("smart-ide"), None, Harness::Codex);

    write(
        &root.join("skills/skill-99"),
        "---\nname: skill-99\ndescription: Does thing 99.\n---\n",
    );
    std::fs::write(root.join("skills/skill-99.toml"), "grant = \"all\"\n").unwrap();

    assert_ne!(
        crate::skills::stamp_of(root, Some("smart-ide"), None, Harness::Codex),
        before
    );
}

#[test]
fn the_set_purlis_tools_answer_from_is_not_routed() {
    let (tmp, bundle) = a_plane_of(MOST + 5);

    let set = of_a_chat(
        Some(&bundle),
        tmp.path(),
        Some("smart-ide"),
        None,
        crate::skills::Route::Plugin,
    );

    assert!(set.routing.is_none());
    assert_eq!(set.effective.len(), MOST + 6);
}

#[test]
fn a_briefed_chat_over_the_budget_is_briefed_on_the_index_and_not_on_what_it_hides() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let folder = crate::skills::lay_out_for_chat(
        root,
        Some("smart-ide"),
        None,
        Some(&bundle),
        &root.join("sets"),
        Harness::Codex,
        &[],
    )
    .expect("a set's folder");
    let env = |key: &str| match key {
        crate::skills::LISTED_ENV => Some(bundle.to_string_lossy().into_owned()),
        crate::skills::PLANE_LISTED_ENV => Some(folder.to_string_lossy().into_owned()),
        _ => None,
    };

    let briefing = crate::skills::briefed(root, Some("smart-ide"), None, &env).expect("briefed");

    assert!(briefing.contains(INDEX), "{briefing}");
    assert!(briefing.contains("handoff"), "{briefing}");
    assert!(!briefing.contains("skill-07"), "{briefing}");
}
