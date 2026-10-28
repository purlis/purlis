// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// What each harness answered `--version` with, recorded on 2026-10-15.
const RECORDED: [(&str, &str, &str); 3] = [
    (
        "claude",
        include_str!("../../../../../tests/fixtures/harness-versions/claude.txt"),
        "2.1.300",
    ),
    (
        "codex",
        include_str!("../../../../../tests/fixtures/harness-versions/codex.txt"),
        "0.147.0",
    ),
    (
        "opencode",
        include_str!("../../../../../tests/fixtures/harness-versions/opencode.txt"),
        "1.18.33",
    ),
];

#[test]
fn each_harness_version_is_read_from_what_it_answered_version_with() {
    for (kind, said, version) in RECORDED {
        assert_eq!(
            version_in(kind, said).map(|v| v.to_string()).as_deref(),
            Some(version),
            "{kind} answered {said:?}"
        );
    }
}

#[test]
fn an_answer_in_another_harness_shape_is_no_version_of_it() {
    // Each harness's answer, asked of the others: a program that answers as another harness is
    // not the one the profile runs.
    assert_eq!(version_in("claude", "codex-cli 0.147.0\n"), None);
    assert_eq!(version_in("claude", "1.18.33\n"), None);
    assert_eq!(version_in("codex", "2.1.300 (Claude Code)\n"), None);
    assert_eq!(version_in("opencode", "codex-cli 0.147.0\n"), None);
    assert_eq!(version_in("opencode", "2.1.300 (Claude Code)\n"), None);
    assert_eq!(version_in("opencode", ""), None);
    assert_eq!(version_in("aider", "1.0.0\n"), None);
}

#[test]
fn a_line_printed_before_the_version_is_passed_over() {
    assert_eq!(
        version_in("codex", "a notice of its own\ncodex-cli 0.48.0-alpha.2\n")
            .map(|v| v.to_string()),
        Some("0.48.0-alpha.2".to_owned())
    );
}

/// A store under a folder of its own, holding one profile `name` of Claude Code: made by purlis,
/// or the harness's own folder adopted in place, and pinned to `pinned` where given.
fn store_with(name: &str, adopted: bool, pinned: Option<&str>) -> tempfile::TempDir {
    store_of(name, "claude", adopted, pinned)
}

/// [`store_with`], of the harness `harness`.
fn store_of(name: &str, harness: &str, adopted: bool, pinned: Option<&str>) -> tempfile::TempDir {
    let config = tempfile::tempdir().unwrap();
    super::super::update(config.path(), |store| {
        store.profiles.push(MachineProfile {
            name: name.to_owned(),
            harness: harness.to_owned(),
            title: if adopted {
                "Claude Code (this Mac)".to_owned()
            } else {
                name.to_owned()
            },
            folders: super::super::Folders {
                config: config.path().join("folder"),
                data: None,
            },
            adopted: adopted.then(|| super::super::Adopted {
                at: 1,
                variables: Vec::new(),
            }),
            imported: None,
            sign_in: None,
            sandbox_only: false,
            kept_for: None,
            pinned: pinned.map(|version| Pinned {
                version: version.to_owned(),
                at: 1,
            }),
            rest: serde_json::Map::new(),
        });
    })
    .unwrap();
    config
}

/// The record `name`, as the store lists it: what a start hands [`at_start`].
fn rec(config: &tempfile::TempDir, name: &str) -> MachineProfile {
    super::super::list(config.path())
        .unwrap()
        .into_iter()
        .find(|p| p.name == name)
        .unwrap()
}

/// The version `name` is pinned to now, as the store lists it.
fn pin_of(config: &tempfile::TempDir, name: &str) -> Option<String> {
    super::super::list(config.path())
        .unwrap()
        .into_iter()
        .find(|p| p.name == name)
        .and_then(|p| p.pinned)
        .map(|p| p.version)
}

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn the_first_chat_on_a_profile_pins_the_version_installed() {
    let config = store_with("work", false, None);

    let said = at_start(
        config.path(),
        &rec(&config, "work"),
        Some(&v("2.1.300")),
        false,
        7,
    );

    assert_eq!(said, Ok(None));
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.300"));
}

#[test]
fn a_chat_on_the_pinned_version_runs_and_leaves_the_pin_as_it_was() {
    let config = store_with("work", false, Some("2.1.300"));

    assert_eq!(
        at_start(
            config.path(),
            &rec(&config, "work"),
            Some(&v("2.1.300")),
            false,
            7
        ),
        Ok(None)
    );
    assert_eq!(
        at_start(
            config.path(),
            &rec(&config, "work"),
            Some(&v("2.1.300")),
            true,
            7
        ),
        Ok(None)
    );
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.300"));
}

#[test]
fn a_made_profile_follows_a_minor_or_patch_update_and_its_tab_says_so() {
    let config = store_with("work", false, Some("2.1.283"));

    let patch = at_start(
        config.path(),
        &rec(&config, "work"),
        Some(&v("2.1.300")),
        false,
        7,
    );
    assert_eq!(
        patch.as_ref().map(Option::as_deref),
        Ok(Some(
            "The profile 'work' moved from Claude Code 2.1.283 to 2.1.300, and its pin with it: \
             its chats follow its harness to a newer version of the same major version."
        ))
    );
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.300"));

    // A resumed conversation follows a minor update too, and the next start runs on the pin.
    let minor = at_start(
        config.path(),
        &rec(&config, "work"),
        Some(&v("2.2.0")),
        true,
        8,
    )
    .unwrap();
    assert!(minor.is_some_and(|said| said.contains("from Claude Code 2.1.300 to 2.2.0")));
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.2.0"));
    assert_eq!(
        at_start(
            config.path(),
            &rec(&config, "work"),
            Some(&v("2.2.0")),
            true,
            9
        ),
        Ok(None)
    );
}

#[test]
fn a_new_chat_on_another_major_version_is_refused_with_the_way_out() {
    let config = store_with("work", false, Some("2.1.283"));

    let refused = at_start(
        config.path(),
        &rec(&config, "work"),
        Some(&v("3.0.0")),
        false,
        7,
    )
    .unwrap_err();

    assert_eq!(
        refused,
        "The profile 'work' is pinned to Claude Code 2.1.283, and Claude Code 3.0.0 is \
         installed: a profile's chats follow its harness only to a newer version of the same \
         major version, so nothing was started. To run its chats on 3.0.0, press Update the \
         pin in Settings › You › Profiles, or install Claude Code 2.1.283 again."
    );
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.283"));
}

#[test]
fn a_conversation_resumes_only_on_the_pinned_version_or_a_newer_one_of_its_major() {
    let config = store_with("work", false, Some("2.1.283"));

    // Another major, or an older version that may not read what a newer one wrote.
    for installed in ["3.0.0", "1.9.0", "2.1.200"] {
        let refused = at_start(
            config.path(),
            &rec(&config, "work"),
            Some(&v(installed)),
            true,
            7,
        )
        .unwrap_err();
        assert_eq!(
            refused,
            format!(
                "This chat resumes a conversation of the profile 'work', which is pinned to \
                 Claude Code 2.1.283, and Claude Code {installed} is installed. purlis resumes a \
                 profile's conversations only on its pinned version or a newer one of the same \
                 major version, so nothing was started. To resume it on {installed}, press \
                 Update the pin in Settings › You › Profiles, or install Claude Code 2.1.283 \
                 again."
            )
        );
    }
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.283"));
}

#[test]
fn once_the_person_updates_the_pin_the_profiles_chats_run_and_resume_on_it() {
    let config = store_with("work", false, Some("2.1.283"));

    let record = update_pin(config.path(), "work", "3.0.0", 9).unwrap();

    assert_eq!(
        record.pinned,
        Some(Pinned {
            version: "3.0.0".to_owned(),
            at: 9
        })
    );
    assert_eq!(
        at_start(
            config.path(),
            &rec(&config, "work"),
            Some(&v("3.0.0")),
            true,
            10
        ),
        Ok(None)
    );
}

#[test]
fn a_pin_is_a_version_and_names_a_profile_this_machine_has() {
    let config = store_with("work", false, Some("2.1.283"));

    assert!(update_pin(config.path(), "work", "latest", 9).is_err());
    assert!(update_pin(config.path(), "play", "2.1.300", 9).is_err());
    assert_eq!(pin_of(&config, "work").as_deref(), Some("2.1.283"));
}

#[test]
fn an_adopted_profile_follows_its_harness_to_a_new_version_and_its_tab_says_so() {
    let config = store_with("claude", true, Some("2.1.283"));

    for resuming in [false, true] {
        let said = at_start(
            config.path(),
            &rec(&config, "claude"),
            Some(&v("2.1.300")),
            resuming,
            7,
        );
        let notice = said.expect("an adopted profile never refuses a version");
        if resuming {
            // The pin moved with the first chat; the second runs on it.
            assert_eq!(notice, None);
        } else {
            assert_eq!(
                notice.as_deref(),
                Some(
                    "Claude Code (this Mac) moved from 2.1.283 to 2.1.300, and its pin with it: \
                     it is the harness's own folder, so its chats run on the version installed."
                )
            );
        }
    }
    assert_eq!(pin_of(&config, "claude").as_deref(), Some("2.1.300"));
}

#[test]
fn a_version_purlis_cannot_read_holds_a_pinned_made_profile_and_never_an_adopted_one() {
    let made = store_with("work", false, Some("2.1.283"));
    let refused = at_start(made.path(), &rec(&made, "work"), None, false, 7).unwrap_err();
    assert!(refused.contains("2.1.283"), "{refused}");
    assert!(refused.contains("claude --version"), "{refused}");

    let unpinned = store_with("work", false, None);
    assert_eq!(
        at_start(unpinned.path(), &rec(&unpinned, "work"), None, false, 7),
        Ok(None)
    );
    assert_eq!(pin_of(&unpinned, "work"), None);

    let adopted = store_with("claude", true, Some("2.1.283"));
    assert_eq!(
        at_start(adopted.path(), &rec(&adopted, "claude"), None, true, 7),
        Ok(None)
    );
    assert_eq!(pin_of(&adopted, "claude").as_deref(), Some("2.1.283"));
}

#[test]
fn the_pin_is_kept_in_the_profiles_record_as_pinned() {
    // A build before #2038 keeps it among the fields it does not know and writes it back as it was.
    let config = store_with("work", false, Some("2.1.283"));
    let text = std::fs::read_to_string(super::super::path(config.path())).unwrap();
    assert!(
        text.contains("\"pinned\": {\n        \"version\": \"2.1.283\""),
        "{text}"
    );
}

#[test]
fn a_made_codex_profile_below_1_0_follows_each_release_and_its_tab_says_so() {
    // Below 1.0 every release is a minor one, so a weekly Codex release follows.
    let config = store_of("work", "codex", false, Some("0.147.0"));

    let said = at_start(
        config.path(),
        &rec(&config, "work"),
        Some(&v("0.148.0")),
        true,
        7,
    );

    assert_eq!(
        said.as_ref().map(Option::as_deref),
        Ok(Some(
            "The profile 'work' moved from Codex 0.147.0 to 0.148.0, and its pin with it: its \
             chats follow its harness to a newer version of the same major version."
        ))
    );
    assert_eq!(pin_of(&config, "work").as_deref(), Some("0.148.0"));
}

/// Makes the store under `config` one a newer purlis wrote, which this one never writes over.
fn written_by_a_newer_purlis(config: &tempfile::TempDir) {
    let at = super::super::path(config.path());
    let text = std::fs::read_to_string(&at).unwrap();
    std::fs::write(&at, text.replacen("\"version\": 1", "\"version\": 99", 1)).unwrap();
}

#[test]
fn an_adopted_profile_is_never_refused_for_a_store_purlis_cannot_write() {
    let adopted = store_with("claude", true, Some("2.1.283"));
    let record = rec(&adopted, "claude");
    written_by_a_newer_purlis(&adopted);

    for installed in [None, Some(v("3.0.0"))] {
        assert_eq!(
            at_start(adopted.path(), &record, installed.as_ref(), true, 7),
            Ok(None)
        );
    }

    // A profile purlis made, on the same store, is refused and says why.
    let made = store_with("work", false, None);
    let record = rec(&made, "work");
    written_by_a_newer_purlis(&made);
    let refused = at_start(made.path(), &record, Some(&v("2.1.300")), false, 7).unwrap_err();
    assert!(refused.contains("newer purlis"), "{refused}");
    assert!(refused.ends_with("so nothing was started."), "{refused}");
}

#[test]
fn update_the_pin_is_offered_only_where_the_next_chat_would_not_start() {
    let pinned = |adopted: bool, pin: Option<&str>| {
        let config = store_with("work", adopted, pin);
        rec(&config, "work")
    };
    let made = pinned(false, Some("2.1.283"));
    // Another major, or an older version of the same major: offered.
    assert_eq!(offered(&made, &v("3.0.0")), Some(v("3.0.0")));
    assert_eq!(offered(&made, &v("1.9.0")), Some(v("1.9.0")));
    assert_eq!(offered(&made, &v("2.1.200")), Some(v("2.1.200")));
    // The pinned one, or a newer one its chats follow: not.
    assert_eq!(offered(&made, &v("2.1.283")), None);
    assert_eq!(offered(&made, &v("2.2.0")), None);
    // An adopted profile follows every version, and one not pinned yet is pinned by its chat.
    assert_eq!(offered(&pinned(true, Some("2.1.283")), &v("3.0.0")), None);
    assert_eq!(offered(&pinned(false, None), &v("3.0.0")), None);
}
