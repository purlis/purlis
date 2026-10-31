// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat whose skills outgrow its budget (SK-15, #2778; ADR 0102), through purlis-core: what its
//! harness is handed, what its pins keep listed, what the studio and Use skill… say, and that
//! nothing hidden is out of reach of `skill_find` and `skill_load`.

use std::path::{Path, PathBuf};

use super::*;
use crate::harness::Harness;
use crate::skills::{
    ChatSkills, DIR_IN_BUNDLE, INDEX, Origin, Outcome, Running, for_use_skill, of_a_chat,
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
        start: None,
    }
}

/// The stamp of a Codex chat in `smart-ide`, on no persona, with the built-in skills in
/// `built_in`.
fn stamp(root: &Path, built_in: Option<&Path>) -> crate::skills::Stamp {
    crate::skills::stamp_of(&Running {
        plane: root,
        workspace: Some("smart-ide"),
        persona: None,
        harness: Harness::Codex,
        built_in,
        plane_skills: true,
        start: None,
    })
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
        Some(crate::skills::PlaneSkills {
            plane: root,
            workspace: Some("smart-ide"),
            repo: &[],
            routed: None,
        }),
        persona,
        Some(bundle),
        &root.join("sets"),
        Harness::ClaudeCode,
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
fn above_the_budget_an_opencode_chat_is_handed_the_index_as_purlis_skill_index() {
    // opencode reads every skill by its bare name, a clone's `skill-index` too, so purlis's index
    // is `purlis-skill-index` there, beside the built-in skills under their own names.
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let folder = crate::skills::lay_out_for_chat(
        Some(crate::skills::PlaneSkills {
            plane: root,
            workspace: Some("smart-ide"),
            repo: &[],
            routed: None,
        }),
        None,
        Some(&bundle),
        &root.join("sets"),
        Harness::Opencode,
    )
    .expect("a set's folder");
    let mut names: Vec<String> = std::fs::read_dir(folder.join(DIR_IN_BUNDLE))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["purlis-handoff", "purlis-skill-index"]);
    let index = std::fs::read_to_string(
        folder
            .join(DIR_IN_BUNDLE)
            .join("purlis-skill-index/SKILL.md"),
    )
    .unwrap();
    assert!(
        index.starts_with("---\nname: purlis-skill-index\n"),
        "{index}"
    );
    assert!(index.contains("- `skill-00`: Does thing 0."), "{index}");
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
    assert!(chat.spent.warns().is_some());
    assert_eq!(chat.still_over, None);
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

/// The studio's list of a chat in `smart-ide`, with no harness of this machine's read.
fn studio(root: &Path, bundle: &Path) -> crate::skills::StudioList {
    let env = crate::harness_plugin::Env {
        chat: &[],
        home: Some(root.join("home")),
        process: false,
    };
    crate::skills::studio_list(Some(bundle), root, Some("smart-ide"), None, &env)
}

#[test]
fn the_studio_says_why_each_harnesss_router_hid_a_skill_and_which_are_pinned() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    pin(
        root,
        &PinnedBy::Workspace("smart-ide".to_owned()),
        "skill-03",
        true,
    )
    .unwrap();

    let list = studio(root, &bundle);

    let row = |name: &str| list.rows.iter().find(|row| row.name == name).expect(name);
    let hidden = &row("skill-00").hidden;
    assert_eq!(
        hidden.len(),
        Harness::ALL.len(),
        "every harness's budget is past"
    );
    assert!(
        Harness::ALL
            .iter()
            .all(|harness| hidden.iter().any(|one| one.harness == *harness))
    );
    assert_eq!(
        hidden[0].hidden.to_string(),
        "hidden by the built-in router: over this chat's skill budget, so it is in the index skill"
    );
    assert_eq!(row("skill-00").left_out, None, "hidden, never left out");
    assert!(row("skill-03").pinned);
    assert!(row("skill-03").hidden.is_empty());
    assert!(
        row("handoff").hidden.is_empty(),
        "a built-in skill is never hidden"
    );
}

#[test]
fn the_studio_routes_each_harness_by_its_own_budget() {
    // Under 30 skills and 6,000 characters of descriptions, but not once Codex's briefing counts
    // each skill's path too.
    let (tmp, bundle) = a_plane_of(0);
    let root = tmp.path();
    for at in 0..28 {
        let name = format!("long-{at:02}");
        write(
            &root.join("skills").join(&name),
            &format!(
                "---\nname: {name}\ndescription: {}\n---\n\nbody\n",
                "w".repeat(210)
            ),
        );
        std::fs::write(
            root.join("skills").join(format!("{name}.toml")),
            "grant = \"all\"\n",
        )
        .unwrap();
    }

    let list = studio(root, &bundle);

    let row = list
        .rows
        .iter()
        .find(|row| row.name == "long-00")
        .expect("long-00");
    let on: Vec<Harness> = row.hidden.iter().map(|one| one.harness).collect();
    assert_eq!(on, [Harness::Codex]);
}

#[test]
fn the_studio_routes_a_harness_as_a_chat_s_start_does_and_never_counts_its_own_skills_4192() {
    // #4192 (ADR 0102 as amended 2026-10-31): a harness's own catalogue counts against nothing.
    // A chat's start routes only what purlis hands it, so the studio's harness page routes the
    // same set: the person's own skills put no plane skill behind the index skill.
    let (tmp, bundle) = a_plane_of(MOST - 2);
    let root = tmp.path();
    let own = root.join("claude-config/skills");
    for at in 0..12 {
        let name = format!("mine-{at:02}");
        write(
            &own.join(&name),
            &format!("---\nname: {name}\ndescription: My own {at}.\n---\n"),
        );
    }
    // One with a plane skill's name: its clash is still said.
    write(
        &own.join("skill-00"),
        "---\nname: skill-00\ndescription: My own copy.\n---\n",
    );
    let chat = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        root.join("claude-config").display().to_string(),
    )];
    let env = crate::harness_plugin::Env {
        chat: &chat,
        home: Some(root.join("home")),
        process: false,
    };

    let list = crate::skills::studio_list(Some(&bundle), root, Some("smart-ide"), None, &env);

    let claude = list
        .harnesses
        .iter()
        .find(|one| one.harness == Harness::ClaudeCode)
        .expect("Claude Code in the studio");
    assert_eq!(
        claude
            .outside
            .iter()
            .filter(|one| one.origin == Origin::Harness)
            .count(),
        13,
        "the person's own skills are read"
    );
    let start = for_use_skill(&running(root, &bundle, None));
    assert!(hidden(&start).is_empty(), "a chat's start hides none");
    for row in list.rows.iter().filter(|row| row.origin.of_the_plane()) {
        assert!(
            row.hidden
                .iter()
                .all(|one| one.harness != Harness::ClaudeCode),
            "{}: listed on Claude Code, as a chat's start lists it",
            row.name
        );
    }
    assert!(
        claude
            .set
            .clashes
            .iter()
            .any(|clash| clash.name == "skill-00"
                && clash.outside == Origin::Harness
                && clash.outcome == Outcome::BothLoad),
        "{:?}",
        claude.set.clashes
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

        let salt = crate::skillspan::PromptSalt::of_chat("01JB00000000000000000000C4").unwrap();
        let typing = chat.typed("skill-07", &[], &salt).unwrap();

        assert_eq!(
            typing.text, "Load the skill skill-07 with skill_load and follow it.",
            "{harness:?}"
        );
        assert_eq!(
            typing.start,
            crate::skills::Start::ByLoad,
            "the load opens the run"
        );
        let built_in = chat
            .typed(&harness.skills().built_in_name("handoff"), &[], &salt)
            .unwrap();
        assert!(
            matches!(built_in.start, crate::skills::Start::Held(_)),
            "a listed skill is typed as before"
        );
    }
}

#[test]
fn the_stamp_of_a_chat_changes_when_what_its_index_names_changes() {
    let (tmp, _bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let before = stamp(root, None);

    write(
        &root.join("skills/skill-99"),
        "---\nname: skill-99\ndescription: Does thing 99.\n---\n",
    );
    std::fs::write(root.join("skills/skill-99.toml"), "grant = \"all\"\n").unwrap();

    assert_ne!(stamp(root, None), before);
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
        Some(crate::skills::PlaneSkills {
            plane: root,
            workspace: Some("smart-ide"),
            repo: &[],
            routed: None,
        }),
        None,
        Some(&bundle),
        &root.join("sets"),
        Harness::Codex,
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

#[test]
fn a_chat_the_built_in_skills_put_past_the_budget_is_stamped_with_its_index() {
    // 26 plane skills fit the budget alone; with six built-in skills they do not.
    let (tmp, bundle) = a_plane_of(26);
    let root = tmp.path();
    for name in ["secrets", "theme", "browser", "update", "persona"] {
        write(
            &bundle.join(name),
            &format!("---\nname: {name}\ndescription: Purlis's {name}.\n---\n"),
        );
    }
    assert!(
        laid_out(root, &bundle, None)
            .iter()
            .any(|name| name == INDEX)
    );

    let with = stamp(root, Some(&bundle));
    let without = stamp(root, None);
    assert_ne!(with, without, "the built-in skills decide what is hidden");
}

#[test]
fn a_hidden_skills_description_edit_offers_no_restart_and_its_deprecation_does() {
    // SK-15 review (#3245): the harness lists the index by its fixed description, and a hidden
    // skill is loaded as it is now (`skill_load`), so only what the index names, and what it says
    // to use instead, is what a running chat would miss.
    let (tmp, _bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let before = stamp(root, None);

    write(
        &root.join("skills/skill-05"),
        "---\nname: skill-05\ndescription: Does something else entirely.\n---\n",
    );
    assert_eq!(
        stamp(root, None),
        before,
        "a description edit of a hidden skill"
    );

    std::fs::write(
        root.join("skills/skill-05.toml"),
        "grant = \"all\"\nlifecycle = \"deprecated\"\n",
    )
    .unwrap();
    let deprecated = stamp(root, None);
    assert_ne!(deprecated, before, "a hidden skill deprecated");

    std::fs::write(
        root.join("skills/skill-05.toml"),
        "grant = \"all\"\nlifecycle = \"deprecated\"\nreplacement = \"skill-06\"\n",
    )
    .unwrap();
    assert_ne!(stamp(root, None), deprecated, "its replacement named");
}

#[test]
fn pins_past_their_limits_are_none_and_say_why() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    let manifest = root.join("workspaces/smart-ide/workspace.json");
    let many: Vec<String> = (0..=MOST_PINS).map(|at| format!("\"pin-{at}\"")).collect();
    std::fs::write(
        &manifest,
        format!(
            r#"{{"settings": {{"skills": {{"pinned": [{}]}}}}}}"#,
            many.join(", ")
        ),
    )
    .unwrap();
    let too_many = pins_of(root, Some("smart-ide"), None).unwrap_err();
    assert!(too_many.contains("more than 64 pins"), "{too_many}");

    std::fs::write(
        &manifest,
        r#"{"settings": {"skills": {"pinned": ["skill-00", "Not A Name"]}}}"#,
    )
    .unwrap();
    let odd = pins_of(root, Some("smart-ide"), None).unwrap_err();
    assert!(odd.contains("not a skill's name"), "{odd}");

    std::fs::write(
        &manifest,
        r#"{"settings": {"skills": {"pinned": ["skill-00", "skill-00"]}}}"#,
    )
    .unwrap();
    assert_eq!(
        pins_of(root, Some("smart-ide"), None).unwrap(),
        ["skill-00"]
    );
}

#[test]
fn a_skill_says_every_workspace_and_persona_that_pins_it_sk15d() {
    let (tmp, _bundle) = a_plane_of(3);
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    let workspace = PinnedBy::Workspace("smart-ide".to_owned());
    let persona = PinnedBy::Persona("reviewer".to_owned());
    assert!(pinners_of(root, "skill-01").is_empty());

    pin(root, &workspace, "skill-01", true).unwrap();
    pin(root, &persona, "skill-01", true).unwrap();
    pin(
        root,
        &PinnedBy::Workspace("ops".to_owned()),
        "skill-02",
        true,
    )
    .unwrap();

    assert_eq!(
        pinners_of(root, "skill-01"),
        [workspace.clone(), persona.clone()]
    );
    assert_eq!(
        pinners_of(root, "skill-02"),
        [PinnedBy::Workspace("ops".to_owned())]
    );
    assert_eq!(pins_written(root, &workspace), ["skill-01"]);
    assert_eq!(pins_written(root, &persona), ["skill-01"]);
}

#[test]
fn a_chat_says_which_of_its_workspace_and_persona_pin_anything_sk15d() {
    let (tmp, bundle) = a_plane_of(MOST + 5);
    let root = tmp.path();
    let chat = |persona| for_use_skill(&running(root, &bundle, persona));
    assert!(chat(Some("reviewer")).pinned_by.is_empty());

    pin(
        root,
        &PinnedBy::Persona("reviewer".to_owned()),
        "skill-02",
        true,
    )
    .unwrap();
    assert_eq!(
        chat(Some("reviewer")).pinned_by,
        [PinnedBy::Persona("reviewer".to_owned())]
    );
    assert!(chat(None).pinned_by.is_empty());

    pin(
        root,
        &PinnedBy::Workspace("smart-ide".to_owned()),
        "skill-03",
        true,
    )
    .unwrap();
    assert_eq!(
        chat(Some("reviewer")).pinned_by,
        [
            PinnedBy::Workspace("smart-ide".to_owned()),
            PinnedBy::Persona("reviewer".to_owned())
        ]
    );
}

#[test]
fn a_pin_past_the_most_a_chat_reads_is_refused_and_says_why_sk15d() {
    let (tmp, _bundle) = a_plane_of(super::MOST_PINS + 1);
    let root = tmp.path();
    let workspace = PinnedBy::Workspace("smart-ide".to_owned());
    for at in 0..super::MOST_PINS {
        assert!(pin(root, &workspace, &format!("skill-{at:02}"), true).unwrap());
    }

    let past = format!("skill-{:02}", super::MOST_PINS);
    let refused = pin(root, &workspace, &past, true).unwrap_err();

    assert!(refused.contains("already pins 64 skills"), "{refused}");
    assert!(!pins_written(root, &workspace).contains(&past));
    assert_eq!(pins_refused(root, &workspace), None);
    // Unpinning one is still taken.
    assert!(pin(root, &workspace, "skill-00", false).unwrap());
}

#[test]
fn a_pinners_list_that_a_chat_would_not_read_says_why_and_names_a_bad_entry_sk15d() {
    let (tmp, _bundle) = a_plane_of(2);
    let root = tmp.path();
    let workspace = PinnedBy::Workspace("smart-ide".to_owned());
    std::fs::write(
        root.join("workspaces/smart-ide/workspace.json"),
        r#"{"settings": {"skills": {"pinned": ["skill-00", "Not A Name"]}}}"#,
    )
    .unwrap();

    let refused = pins_refused(root, &workspace).expect("refused");
    assert_eq!(refused.bad.as_deref(), Some("Not A Name"));
    assert!(
        refused.why.contains("\"Not A Name\" is not a skill's name"),
        "{}",
        refused.why
    );
    assert_eq!(refused.file, "workspaces/smart-ide/workspace.json");
    assert_eq!(
        pins_refused(root, &PinnedBy::Persona("reviewer".to_owned())),
        None
    );

    // The bad entry is unpinned like any other, and the list is read again.
    assert!(pin(root, &workspace, "Not A Name", false).unwrap());
    assert_eq!(pins_refused(root, &workspace), None);
    assert_eq!(pins_written(root, &workspace), ["skill-00"]);
}

#[test]
fn a_pinners_list_of_too_many_says_so_and_names_no_entry_sk15d() {
    let (tmp, _bundle) = a_plane_of(1);
    let root = tmp.path();
    let names: Vec<String> = (0..=super::MOST_PINS)
        .map(|at| format!("\"p-{at}\""))
        .collect();
    std::fs::write(
        root.join("workspaces/smart-ide/workspace.json"),
        format!(
            r#"{{"settings": {{"skills": {{"pinned": [{}]}}}}}}"#,
            names.join(",")
        ),
    )
    .unwrap();

    let refused = pins_refused(root, &PinnedBy::Workspace("smart-ide".to_owned())).unwrap();
    assert_eq!(refused.bad, None);
    assert!(refused.why.contains("more than 64 pins"), "{}", refused.why);
}

#[test]
fn use_skill_and_the_stamp_of_a_chat_in_a_clone_route_its_repo_skills_as_its_folder_did() {
    // #3245: a chat's start routes its set with the repo skills of the clone it works in, so
    // Use skill… and its restart's stamp read the same clone, and mark hidden what it was not
    // handed.
    let (tmp, bundle) = a_plane_of(MOST - 1);
    let root = tmp.path();
    let clone = root.join("workspaces/smart-ide/api");
    std::fs::create_dir_all(clone.join(".git")).unwrap();
    for at in 0..5 {
        let name = format!("repo-{at}");
        write(
            &clone.join(".agents/skills").join(&name),
            &format!("---\nname: {name}\ndescription: The repo's {at}.\n---\n"),
        );
    }
    let repo = crate::skills::repo_skills_of_chat(Harness::Codex, root, "smart-ide", &clone);
    assert_eq!(repo.len(), 5, "{repo:?}");
    let folder = crate::skills::lay_out_for_chat(
        Some(crate::skills::PlaneSkills {
            plane: root,
            workspace: Some("smart-ide"),
            repo: &repo,
            routed: None,
        }),
        None,
        Some(&bundle),
        &root.join("sets"),
        Harness::Codex,
    )
    .expect("a set's folder");
    let handed: Vec<String> = std::fs::read_dir(folder.join(DIR_IN_BUNDLE))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(handed.iter().any(|name| name == INDEX), "{handed:?}");

    // What its start read, once: the clone's repo skills among it.
    let start =
        crate::hoststart::ChatSkills::read(true, root, Some(Harness::Codex), Some(&clone), None);
    assert_eq!(start.repo.len(), 5);
    let in_the_clone = Running {
        harness: Harness::Codex,
        start: Some(&start),
        ..running(root, &bundle, None)
    };
    let chat = for_use_skill(&in_the_clone);
    assert!(!hidden(&chat).is_empty());
    for one in chat
        .offered
        .iter()
        .filter(|one| one.effective.origin.of_the_plane())
    {
        let name = &one.effective.skill.name;
        assert_eq!(
            handed.contains(name),
            one.hidden.is_none(),
            "{name}: listed by Use skill… exactly where its folder holds it"
        );
    }

    let elsewhere = Running {
        start: None,
        ..in_the_clone
    };
    assert!(hidden(&for_use_skill(&elsewhere)).is_empty());
    assert_ne!(
        crate::skills::stamp_of(&in_the_clone),
        crate::skills::stamp_of(&elsewhere),
        "the clone's skills decide what is hidden"
    );
}

/// #3391: a workspace's and a persona's pins, each under the most a chat reads and past it
/// together, are each accepted, and the chat says the two together are not read, naming both
/// and how many each holds.
#[test]
fn a_chat_whose_workspace_and_persona_pins_pass_64_together_says_so_3391() {
    let (tmp, bundle) = a_plane_of(70);
    let root = tmp.path();
    let workspace = PinnedBy::Workspace("smart-ide".to_owned());
    let persona = PinnedBy::Persona("reviewer".to_owned());
    for at in 0..40 {
        assert!(pin(root, &workspace, &format!("skill-{at:02}"), true).unwrap());
    }
    for at in 40..70 {
        assert!(pin(root, &persona, &format!("skill-{at:02}"), true).unwrap());
    }
    // Each pinner alone is accepted.
    assert_eq!(pins_refused(root, &workspace), None);
    assert_eq!(pins_refused(root, &persona), None);

    let chat = |persona| for_use_skill(&running(root, &bundle, persona));
    assert_eq!(
        chat(Some("reviewer")).pins_together,
        Some(PinsTogether {
            held: vec![
                PinsHeld {
                    by: workspace.clone(),
                    pins: 40
                },
                PinsHeld {
                    by: persona.clone(),
                    pins: 30
                },
            ],
            most: MOST_PINS,
        })
    );
    // The workspace's alone is read.
    assert_eq!(chat(None).pins_together, None);

    // Ten of the persona's the workspace pins too: sixty names, which a chat reads.
    for at in 60..70 {
        assert!(pin(root, &persona, &format!("skill-{at:02}"), false).unwrap());
    }
    for at in 30..40 {
        assert!(pin(root, &persona, &format!("skill-{at:02}"), true).unwrap());
    }
    assert_eq!(chat(Some("reviewer")).pins_together, None);
}
