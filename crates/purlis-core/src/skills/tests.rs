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
        assert_eq!(set.dir(), Some(plugin.join("skills").as_path()));
    }
}

#[test]
fn only_a_chat_started_with_the_variable_is_briefed_on_its_skills() {
    let root = Path::new("/plane");
    let unset = |_: &str| None;
    assert_eq!(briefed(root, &unset), None);
    let empty = |name: &str| (name == LISTED_ENV).then(String::new);
    assert_eq!(briefed(root, &empty), None);

    let skills = crate::harness::testing::bundled_plugin().join("skills");
    let dir = skills.display().to_string();
    let set = move |name: &str| (name == LISTED_ENV).then(|| dir.clone());
    let text = briefed(root, &set).expect("listed");

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

    let text = briefed(tmp.path(), &env).expect("listed");

    assert!(text.contains("- `kept` — Is kept."), "{text}");
    assert!(!text.contains("undescribed"), "{text}");
}
