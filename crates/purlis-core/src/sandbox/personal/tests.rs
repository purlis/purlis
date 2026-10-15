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

fn levels(level: Option<Choice>, personas: &[(&str, Choice)]) -> Personal {
    Personal {
        level,
        personas: personas
            .iter()
            .map(|(name, choice)| ((*name).to_owned(), *choice))
            .collect(),
    }
}

#[test]
fn a_level_the_file_alone_says_is_honoured_only_down_to_what_you_confirmed_or_the_floor() {
    use Choice::{Floor, Off, On};
    let nothing = Personal::default();
    // Nothing confirmed: a stricter level, or the floor, is honoured; `off` is not.
    assert_eq!(honoured(&levels(Some(On), &[]), &nothing).level, Some(On));
    assert_eq!(
        honoured(&levels(Some(Floor), &[]), &nothing).level,
        Some(Floor)
    );
    assert_eq!(honoured(&levels(Some(Off), &[]), &nothing).level, None);
    // Confirmed `off`: honoured, and anything stricter too.
    assert_eq!(
        honoured(&levels(Some(Off), &[]), &levels(Some(Off), &[])).level,
        Some(Off)
    );
    assert_eq!(
        honoured(&levels(Some(Floor), &[]), &levels(Some(Off), &[])).level,
        Some(Floor)
    );
    // Confirmed `on`, the file says lower: what you confirmed.
    assert_eq!(
        honoured(&levels(Some(Off), &[]), &levels(Some(On), &[])).level,
        Some(On)
    );
    // Taken out of the file: not set, whatever the record says.
    assert_eq!(honoured(&nothing, &levels(Some(Off), &[])).level, None);
    // A persona's, the same way.
    let file = levels(None, &[("devops", Off), ("qa", On)]);
    let judged = honoured(&file, &nothing);
    assert_eq!(judged.for_persona(Some("devops")), None);
    assert_eq!(judged.for_persona(Some("qa")), Some(On));
    assert_eq!(
        awaiting(&file, &nothing),
        [Awaiting {
            persona: Some("devops".to_owned()),
            level: Off
        }]
    );
    assert!(awaiting(&file, &levels(None, &[("devops", Off)])).is_empty());
}

#[test]
fn an_off_written_into_the_file_is_not_confirmed_until_you_confirm_it() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(crate::profiles::LOCAL_FILE);
    std::fs::write(
        &file,
        "[sandbox]\nlevel = \"off\"\n[sandbox.personas]\nqa = \"off\"\n",
    )
    .unwrap();

    assert_eq!(of(root.path()).level, Some(Choice::Off));
    let now = in_force(root.path());
    assert_eq!(
        now.for_persona(None),
        None,
        "a level the file alone says turned the sandbox off"
    );
    assert_eq!(now.for_persona(Some("qa")), None);
    assert_eq!(waiting(root.path()).len(), 2);

    // Confirm, in Settings: the project's, then the persona's.
    assert_eq!(
        confirm(root.path(), None, Choice::Off).unwrap(),
        Choice::Off
    );
    assert_eq!(in_force(root.path()).for_persona(None), Some(Choice::Off));
    assert_eq!(
        in_force(root.path()).for_persona(Some("qa")),
        Some(Choice::Off)
    );
    confirm(root.path(), Some("qa"), Choice::Off).unwrap();
    assert!(waiting(root.path()).is_empty());

    // Taken out of the file by hand and written back later: not confirmed again.
    std::fs::write(&file, "").unwrap();
    forget_not_in(root.path(), Some(""));
    std::fs::write(&file, "[sandbox]\nlevel = \"off\"\n").unwrap();
    assert_eq!(in_force(root.path()).for_persona(None), None);
}

#[test]
fn a_level_you_set_in_settings_is_confirmed_as_you_set_it() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(crate::profiles::LOCAL_FILE);
    std::fs::write(&file, "[sandbox]\nlevel = \"off\"\n").unwrap();
    set_by_you(root.path(), None, Some(Choice::Off)).unwrap();
    assert_eq!(in_force(root.path()).for_persona(None), Some(Choice::Off));
    set_by_you(root.path(), None, None).unwrap();
    assert_eq!(in_force(root.path()).for_persona(None), None);
    assert!(set_by_you(root.path(), Some("../x"), Some(Choice::Off)).is_err());
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

#[test]
fn a_confirm_takes_the_level_it_showed_and_refuses_one_the_file_changed_since() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(crate::profiles::LOCAL_FILE);
    std::fs::write(&file, "[sandbox]\nlevel = \"floor\"\n").unwrap();
    // Shown `floor`, then the file swapped to `off` before the click.
    std::fs::write(&file, "[sandbox]\nlevel = \"off\"\n").unwrap();
    let refused = confirm(root.path(), None, Choice::Floor).unwrap_err();
    assert!(refused.contains("no sandbox"), "{refused}");
    assert_eq!(in_force(root.path()).for_persona(None), None);
    // Taken out of the file since: nothing to confirm.
    std::fs::write(&file, "").unwrap();
    assert!(confirm(root.path(), None, Choice::Off).is_err());
    assert_eq!(
        super::super::local::confirmed_levels(root.path()),
        Personal::default()
    );
}

#[test]
fn in_a_project_at_the_full_sandbox_a_floor_the_file_alone_says_waits_for_confirm() {
    use Choice::{Floor, Off, On};
    let nothing = Personal::default();
    // Measured from the full sandbox, the project's own: the floor is lower.
    assert_eq!(
        honoured_over(&levels(Some(Floor), &[]), &nothing, On).level,
        None
    );
    assert_eq!(
        honoured_over(&levels(Some(On), &[]), &nothing, On).level,
        Some(On)
    );
    assert_eq!(
        honoured_over(&levels(Some(Floor), &[]), &levels(Some(Floor), &[]), On).level,
        Some(Floor)
    );
    assert_eq!(
        honoured_over(&levels(Some(Floor), &[]), &levels(Some(Off), &[]), On).level,
        Some(Floor)
    );
    // Through the files: a project that turned the full sandbox on.
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    std::fs::write(
        root.path().join(crate::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"floor\"\n",
    )
    .unwrap();
    assert_eq!(in_force(root.path()).level, None);
    assert_eq!(waiting(root.path()).len(), 1);
    set_by_you(root.path(), None, Some(Floor)).unwrap();
    assert_eq!(in_force(root.path()).level, Some(Floor));
}
