// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn table(text: &str) -> toml::Table {
    text.parse::<toml::Table>().unwrap()["sandbox"]
        .as_table()
        .unwrap()
        .clone()
}

#[test]
fn a_persona_s_level_wins_over_the_project_s_and_neither_is_the_default() {
    let (personal, refused) = read(
        &table("[sandbox]\nlevel = \"floor\"\n[sandbox.personas]\ndevops = \"off\"\nqa = \"on\"\n"),
        "purlis.local.toml",
    );
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(personal.for_persona(None), Some(Choice::Floor));
    assert_eq!(personal.for_persona(Some("steward")), Some(Choice::Floor));
    assert_eq!(personal.for_persona(Some("devops")), Some(Choice::Off));
    assert_eq!(personal.for_persona(Some("qa")), Some(Choice::On));
    assert!(personal.any_off());
    assert_eq!(Personal::default().for_persona(Some("devops")), None);
}

#[test]
fn a_value_that_is_not_a_level_or_a_name_that_cannot_be_a_persona_is_refused_and_not_taken() {
    let (personal, refused) = read(
        &table(
            "[sandbox]\nlevel = \"loose\"\n[sandbox.personas]\n\"../x\" = \"off\"\nops = 3\nqa = \"floor\"\n",
        ),
        "purlis.local.toml",
    );
    assert_eq!(personal.level, None);
    assert_eq!(
        personal.personas,
        BTreeMap::from([("qa".to_owned(), Choice::Floor)])
    );
    let keys: Vec<Vec<String>> = refused.iter().map(|one| one.key.clone()).collect();
    assert!(
        keys.contains(&vec!["sandbox".to_owned(), "level".to_owned()]),
        "{refused:?}"
    );
    assert!(
        keys.contains(&vec![
            "sandbox".to_owned(),
            "personas".to_owned(),
            "../x".to_owned()
        ]),
        "{refused:?}"
    );
    assert!(
        keys.contains(&vec![
            "sandbox".to_owned(),
            "personas".to_owned(),
            "ops".to_owned()
        ]),
        "{refused:?}"
    );
    assert!(
        refused
            .iter()
            .all(|one| one.why.contains("purlis.local.toml"))
    );
}

#[test]
fn a_level_is_written_and_taken_out_with_every_other_line_kept() {
    let before = "# mine\n[sandbox]\nhosts = [\"example.com\"]\n";
    let set = with(before, None, Some(Choice::Off)).unwrap();
    assert!(set.contains("# mine"), "{set}");
    assert!(set.contains("hosts = [\"example.com\"]"), "{set}");
    assert_eq!(of_text(&set).level, Some(Choice::Off));
    let persona = with(&set, Some("devops"), Some(Choice::On)).unwrap();
    assert_eq!(
        of_text(&persona).for_persona(Some("devops")),
        Some(Choice::On)
    );
    let back = with(&persona, Some("devops"), None).unwrap();
    assert!(!back.contains("personas"), "{back}");
    let back = with(&back, None, None).unwrap();
    assert_eq!(of_text(&back), Personal::default());
    assert!(back.contains("hosts = [\"example.com\"]"), "{back}");
    assert!(with(before, Some("../x"), Some(Choice::Off)).is_err());
}

#[test]
fn a_local_file_git_would_carry_turns_nobody_s_sandbox_down() {
    let text = "[sandbox]\nlevel = \"off\"\n".to_owned();
    let left_out = crate::settings::LayerText::LeftOut {
        why: "git would commit it".to_owned(),
        text: text.clone(),
    };
    assert_eq!(of_layer(&left_out), Personal::default());
    assert_eq!(
        of_layer(&crate::settings::LayerText::Text(text)).level,
        Some(Choice::Off)
    );
}

#[test]
fn the_settings_save_takes_your_level_and_refuses_one_that_is_not() {
    let file = crate::profiles::LOCAL_FILE;
    let kept = crate::sandbox::refusals(
        "[sandbox]\nhosts = [\"example.com\"]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"floor\"\n",
        file,
    );
    assert!(kept.is_empty(), "{kept:?}");
    let refused = crate::sandbox::refusals("[sandbox]\nlevel = \"loose\"\nmode = \"on\"\n", file);
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert!(
        refused.iter().any(|why| why.contains("sandbox.level")),
        "{refused:?}"
    );
    assert!(
        refused.iter().any(|why| why.contains("sandbox.mode")),
        "{refused:?}"
    );
}
