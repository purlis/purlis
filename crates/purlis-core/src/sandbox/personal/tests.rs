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

/// R-010 (T1-24, Q236): **the Protection row at setup writes the person's own level**, the value
/// Settings › Project › Sandbox › Your level reads and changes. For each choice the local file
/// says it, a chat's start honours it (it is recorded as yours, so nothing waits for a Confirm),
/// and the committed file is left with no `mode`.
#[test]
fn the_setup_choice_is_the_value_settings_reads() {
    use crate::sandbox::policy::Locks;
    use crate::sandbox::{Made, your_level};
    use crate::settings::{self, Which};
    for choice in Choice::ALL {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("charter.toml"), "schema = 1\n").unwrap();

        assert_eq!(chosen_at_setup(root.path(), choice), AtSetup::Kept);

        // What Settings reads: this machine's file, whose `[sandbox] level` is the row.
        let local = settings::read(root.path(), Which::Local).unwrap();
        assert_eq!(of_text(&local.text).level, Some(choice), "{}", local.text);
        // What a chat's start takes, for a chat the person starts in the window.
        assert_eq!(in_force(root.path()).level, Some(choice));
        assert!(
            waiting(root.path()).is_empty(),
            "{choice:?} waits for a Confirm"
        );
        assert_eq!(
            your_level(root.path(), None, &Locks::none(), &Made::Window),
            Some(choice),
        );
        // Never the team's: the committed file keeps no mode.
        let shared = std::fs::read_to_string(root.path().join("charter.toml")).unwrap();
        assert!(!shared.contains("mode"), "{shared}");

        // Changed later in Settings, the same value reads back changed.
        let other = if choice == Choice::On {
            Choice::Off
        } else {
            Choice::On
        };
        let edited = settings::edited(
            &local.text,
            &[settings::Edit {
                path: vec![
                    settings::Step::Key("sandbox".to_owned()),
                    settings::Step::Key(LEVEL.to_owned()),
                ],
                value: Some(settings::Value::Text(other.word().to_owned())),
            }],
        )
        .unwrap();
        settings::save(root.path(), Which::Local, Some(&local.text), &edited).unwrap();
        set_by_you(root.path(), None, Some(other)).unwrap();
        let local = settings::read(root.path(), Which::Local).unwrap();
        assert_eq!(of_text(&local.text).level, Some(other));
        assert_eq!(in_force(root.path()).level, Some(other));
    }
}

/// Q241 (T1-24's review, 2): **the policy is checked before anything is written.** A level the
/// project's `policy = "never"` forbids is never kept as the person's: the local file is left
/// without it, and setup is told why in a sentence, never an error that would stop it after the
/// project was made.
#[test]
fn a_setup_choice_the_project_s_policy_forbids_writes_nothing_and_says_why() {
    use crate::settings::{self, Which};
    for choice in Choice::ALL {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("charter.toml"),
            "schema = 1\n[sandbox]\npolicy = \"never\"\n",
        )
        .unwrap();

        let kept = chosen_at_setup(root.path(), choice);

        let local = settings::read(root.path(), Which::Local).unwrap();
        let written = of_text(&local.text).level;
        if choice == Choice::On {
            assert_eq!(kept, AtSetup::Kept);
            assert_eq!(written, Some(Choice::On));
        } else {
            let AtSetup::NotKept(said) = kept else {
                panic!("{choice:?} was kept under policy = \"never\"");
            };
            assert!(said.contains("policy"), "{said}");
            assert!(said.contains("Settings"), "{said}");
            assert_eq!(written, None, "{}", local.text);
            assert!(
                !local.exists,
                "a refused choice made a file: {}",
                local.text
            );
        }
    }
}

/// Q241, the same under an administrator's lock on this machine: a level below it is never
/// written, and setup is told why.
#[test]
fn a_setup_choice_an_administrator_s_lock_forbids_writes_nothing_and_says_why() {
    use crate::sandbox::policy::{Locks, set_for_this_test};
    use crate::settings::{self, Which};
    let file = Path::new("/Library/Application Support/purlis/policy.json");
    set_for_this_test(Locks::parse(
        r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#,
        file,
    ));
    for choice in Choice::ALL {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("charter.toml"), "schema = 1\n").unwrap();

        let kept = chosen_at_setup(root.path(), choice);

        let written = of_text(&settings::read(root.path(), Which::Local).unwrap().text).level;
        if choice == Choice::On {
            assert_eq!(kept, AtSetup::Kept);
            assert_eq!(written, Some(Choice::On));
        } else {
            assert!(
                matches!(&kept, AtSetup::NotKept(said) if said.contains("Platform team")),
                "{kept:?}"
            );
            assert_eq!(written, None);
        }
    }
    set_for_this_test(Locks::none());
}
