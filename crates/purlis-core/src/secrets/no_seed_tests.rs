// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The person's mark that a key's value is no 2FA seed (#5237), through the fill that reads it: a
//! marked value fills, and nothing but the person's mark on that very value makes it fill.

use std::path::Path;

use super::*;
use crate::secrets::Env;
use crate::secrets::brokered::{Asker, fill_value};

/// A value that looks like a seed and isn't: a 16-letter app password.
const APP_PASSWORD: &str = "abcdefghijklmnop";
const ADDRESS: &str = "otpauth://totp/Dash:alice?secret=GEZDGNBVGY3TQOJQ&issuer=Dash";

/// A project whose plain-file vault `team` holds `values`.
fn project(values: serde_json::Value) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    holding(tmp.path(), &values);
    tmp
}

/// The vault's file now holds `values`, as a change outside purlis leaves it.
fn holding(root: &Path, values: &serde_json::Value) {
    std::fs::write(root.join("team.json"), values.to_string()).unwrap();
}

fn ctx(root: &Path) -> Ctx {
    Ctx::new(root, Env::of(&[("PATH", "/usr/bin:/bin")]))
}

fn team(root: &Path) -> Vault {
    cmd::provider(&ctx(root), "team").unwrap()
}

/// What a fill of `key` answers: the value, or the refusal.
fn filled(root: &Path, key: &str) -> Result<String, String> {
    let asker = Asker {
        root: root.to_path_buf(),
        env: Env::of(&[("PATH", "/usr/bin:/bin")]),
        chat: 7,
        persona: None,
        folder: None,
        confines: None,
        harness: None,
    };
    fill_value(&asker, "team", key).map(|value| value.unwrap().expose().to_owned())
}

/// `seal` written as `key`'s row, however it was come by, as no window press writes one.
fn row(root: &Path, key: &str, seal: &str) {
    let c = ctx(root);
    std::fs::create_dir_all(c.vaults_dir()).unwrap();
    std::fs::write(
        path(&c),
        serde_json::json!({"marks": {"team": {key: seal}}}).to_string(),
    )
    .unwrap();
}

#[test]
fn a_value_that_looks_like_a_seed_is_refused_in_words_that_say_where_to_mark_it() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let refused = filled(tmp.path(), "APP").unwrap_err();
    assert!(refused.contains("looks like a 2FA seed"), "{refused}");
    assert!(refused.contains("vault's tab"), "{refused}");
    assert!(refused.contains(MARK_WORDS), "{refused}");
    assert!(refused.contains("A chat can't make this mark"), "{refused}");
    assert!(!refused.contains(APP_PASSWORD), "{refused}");
}

#[test]
fn a_key_the_person_marked_fills_as_a_value() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    mark(&ctx(tmp.path()), &team(tmp.path()), "APP").unwrap();
    assert_eq!(filled(tmp.path(), "APP").unwrap(), APP_PASSWORD);
}

/// Values that hold a seed in the forms the look takes in beyond a bare one.
const SEED_FORMS: [&str; 5] = [
    "secret=GEZDGNBVGY3TQOJQ",
    "Dash\nGEZDGNBVGY3TQOJQ",
    "Dash GEZD GNBV GY3T QOJQ",
    "GEZD_GNBV_GY3T_QOJQ",
    "GeZdGnBvGy3tQoJq",
];

#[test]
fn a_seed_in_any_form_is_refused_and_the_refusal_holds_none_of_it() {
    let tmp = project(serde_json::json!({}));
    for value in SEED_FORMS {
        holding(tmp.path(), &serde_json::json!({ "APP": value }));
        let refused = filled(tmp.path(), "APP").unwrap_err();
        assert!(refused.contains("looks like a 2FA seed"), "{refused}");
        assert!(!refused.contains("GEZD"), "{refused}");
        assert!(!refused.contains("GeZd"), "{refused}");
    }
}

#[test]
fn a_key_the_person_marked_fills_whatever_form_it_is_in() {
    let tmp = project(serde_json::json!({}));
    for value in SEED_FORMS {
        holding(tmp.path(), &serde_json::json!({ "APP": value }));
        mark(&ctx(tmp.path()), &team(tmp.path()), "APP").unwrap();
        assert_eq!(filled(tmp.path(), "APP").unwrap(), value);
    }
}

#[test]
fn an_ordinary_password_and_a_hex_key_fill_with_no_mark() {
    let tmp = project(serde_json::json!({}));
    for value in [
        "CorrectHorseBatteryStaple",
        // 16 of base32's letters alone is a seed in any case (the ruling of 2026-11-06): a 1 is
        // none of them.
        "MySecretPassword1",
        "abcdef2345672345",
        "AbCdEf2345672345",
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
    ] {
        holding(tmp.path(), &serde_json::json!({ "APP": value }));
        assert_eq!(filled(tmp.path(), "APP").unwrap(), value);
    }
}

#[test]
fn the_file_holds_a_seal_and_never_the_value() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), "APP").unwrap();
    let text = std::fs::read_to_string(path(&c)).unwrap();
    assert!(!text.contains(APP_PASSWORD), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(path(&c)).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
}

#[test]
fn a_mark_taken_off_refuses_again() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), "APP").unwrap();
    unmark(&c, "team", "APP").unwrap();
    assert!(filled(tmp.path(), "APP").is_err());
    assert!(marked_in(&c, "team").is_empty());
}

#[test]
fn a_new_value_under_a_marked_key_is_refused_and_says_the_value_changed() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    mark(&ctx(tmp.path()), &team(tmp.path()), "APP").unwrap();
    holding(
        tmp.path(),
        &serde_json::json!({"APP": "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"}),
    );
    let refused = filled(tmp.path(), "APP").unwrap_err();
    assert!(refused.contains("has changed since"), "{refused}");
    assert!(refused.contains(UNMARK_WORDS), "{refused}");
    assert!(refused.contains(MARK_WORDS), "{refused}");
}

#[test]
fn a_row_with_a_seal_purlis_did_not_make_is_no_mark() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    let forged = [
        "0".repeat(64),
        // A real seal, for another purpose.
        fingerprint::seal(&c, "another-purpose", &[APP_PASSWORD]).unwrap(),
        // The seal of the key's name, of another value, and the value itself.
        fingerprint::seal(&c, PURPOSE, &["APP"]).unwrap(),
        fingerprint::seal(&c, PURPOSE, &["another value"]).unwrap(),
        APP_PASSWORD.to_owned(),
    ];
    for seal in forged {
        row(tmp.path(), "APP", &seal);
        assert_eq!(of(&c, "team", "APP", APP_PASSWORD), Mark::ValueChanged);
        assert!(filled(tmp.path(), "APP").is_err(), "{seal}");
    }
}

#[test]
fn a_row_is_no_mark_with_no_plane_key_to_check_it_and_the_check_makes_no_key() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    row(tmp.path(), "APP", &"0".repeat(64));
    assert!(filled(tmp.path(), "APP").is_err());
    assert!(!fingerprint::key_path(&c).exists());
}

#[test]
fn marks_that_cannot_be_read_are_no_mark() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), "APP").unwrap();
    std::fs::write(path(&c), "not json").unwrap();
    assert!(filled(tmp.path(), "APP").is_err());
    // A write refuses too, and leaves what it could not read for the person to see.
    assert!(mark(&c, &team(tmp.path()), "APP").is_err());
    assert_eq!(std::fs::read_to_string(path(&c)).unwrap(), "not json");
}

#[cfg(unix)]
#[test]
fn a_marks_file_that_is_a_link_is_no_mark() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), "APP").unwrap();
    let elsewhere = tmp.path().join("marks.json");
    std::fs::rename(path(&c), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, path(&c)).unwrap();
    assert!(filled(tmp.path(), "APP").is_err());
}

#[test]
fn an_otpauth_address_is_refused_whatever_the_mark_says() {
    for address in [
        ADDRESS.to_owned(),
        "OTPAUTH://totp/x?secret=GEZDGNBVGY3TQOJQ".to_owned(),
        "otpauth-migration://offline?data=AAAA".to_owned(),
        format!("my login\n{ADDRESS}"),
    ] {
        let tmp = project(serde_json::json!({"APP": address}));
        let c = ctx(tmp.path());
        // A true seal of this very value, as no window press would ever write one.
        row(
            tmp.path(),
            "APP",
            &fingerprint::seal(&c, PURPOSE, &[address.as_str()]).unwrap(),
        );
        assert_eq!(of(&c, "team", "APP", &address), Mark::Holds);
        let refused = filled(tmp.path(), "APP").unwrap_err();
        assert!(refused.contains("no mark changes that"), "{refused}");
        assert!(!refused.contains("GEZDGNBV"), "{refused}");
    }
}

#[test]
fn an_otpauth_address_is_refused_with_no_mark_at_all_wherever_it_stands() {
    let tmp = project(serde_json::json!({"APP": format!("my login: {ADDRESS}")}));
    let refused = filled(tmp.path(), "APP").unwrap_err();
    assert!(refused.contains("otpauth address"), "{refused}");
}

#[test]
fn an_otpauth_address_cannot_be_marked() {
    let tmp = project(serde_json::json!({"APP": ADDRESS}));
    let c = ctx(tmp.path());
    let refused = mark(&c, &team(tmp.path()), "APP").unwrap_err();
    assert!(
        refused.message.contains("authenticator app's address"),
        "{refused:?}"
    );
    assert!(!path(&c).exists());
}

#[test]
fn a_sandboxed_chat_cannot_make_the_mark_and_is_told_it_is_the_persons() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let chat = Ctx::new(
        tmp.path(),
        Env::of(&[
            ("PATH", "/usr/bin:/bin"),
            (crate::hookwire::SANDBOXED_ENV, "1"),
        ]),
    );
    let refused = mark(&chat, &team(tmp.path()), "APP").unwrap_err();
    assert!(refused.message.contains("the person's"), "{refused:?}");
    assert!(!path(&chat).exists());
    assert!(filled(tmp.path(), "APP").is_err());
}

#[test]
fn nothing_a_chat_can_propose_is_read_for_a_mark() {
    // The project's own files, which a chat can write or propose: none is where a mark is read.
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    let seal = fingerprint::seal(&c, PURPOSE, &[APP_PASSWORD]).unwrap();
    let marks = serde_json::json!({"marks": {"team": {"APP": seal}}}).to_string();
    std::fs::write(tmp.path().join(FILE), &marks).unwrap();
    std::fs::write(
        tmp.path().join("charter.toml"),
        "[no-seed]\nteam = [\"APP\"]\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {
                "file": "team.json", "no-seed": ["APP"], "marks": {"APP": seal},
            }},
        }})
        .to_string(),
    )
    .unwrap();
    assert_eq!(of(&c, "team", "APP", APP_PASSWORD), Mark::None);
    let refused = filled(tmp.path(), "APP").unwrap_err();
    assert!(refused.contains("looks like a 2FA seed"), "{refused}");
}

#[test]
fn the_marks_live_in_the_vaults_folder_in_a_file_no_vault_can_name() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    assert_eq!(path(&c).parent().unwrap(), c.vaults_dir());
    assert!(!registry::name_ok(FILE.trim_end_matches(".json")));
    assert!(!registry::name_ok(LOCK_STEM));
}

#[test]
fn a_write_through_purlis_takes_the_mark_away() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    let v = team(tmp.path());
    mark(&c, &v, "APP").unwrap();
    cmd::set_value(&c, &v, "APP", APP_PASSWORD).unwrap();
    assert!(marked_in(&c, "team").is_empty());
    assert!(filled(tmp.path(), "APP").is_err());
}

#[test]
fn a_rename_moves_the_mark_and_a_delete_takes_it_away() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    let v = team(tmp.path());
    mark(&c, &v, "APP").unwrap();
    cmd::rename(&c, &v, "APP", "MAIL").unwrap();
    assert_eq!(filled(tmp.path(), "MAIL").unwrap(), APP_PASSWORD);
    assert_eq!(marked_in(&c, "team"), BTreeSet::from(["MAIL".to_owned()]));
    cmd::delete(&c, &v, "MAIL").unwrap();
    assert!(marked_in(&c, "team").is_empty());
}

#[test]
fn a_mark_on_one_key_says_nothing_of_another() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD, "OTHER": APP_PASSWORD}));
    mark(&ctx(tmp.path()), &team(tmp.path()), "APP").unwrap();
    assert!(filled(tmp.path(), "APP").is_ok());
    assert!(filled(tmp.path(), "OTHER").is_err());
}

#[test]
fn a_change_with_no_mark_to_change_writes_nothing() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    forget(&c, "team", "APP");
    renamed(&c, "team", "APP", "MAIL");
    forget_vault(&c, "team");
    assert!(!path(&c).exists());
}

/// `key` in `team` reads `to` in `team`, written straight into the links index.
fn linked(root: &Path, key: &str, to: &str) {
    let c = ctx(root);
    std::fs::create_dir_all(&c.state).unwrap();
    std::fs::write(
        crate::secrets::links::path(&c),
        serde_json::json!({"links": {"team": {key: {"vault": "team", "key": to}}}}).to_string(),
    )
    .unwrap();
}

const A_SEED: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

#[test]
fn a_linked_entry_is_not_marked_and_the_person_is_told_which_entry_to_mark() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD, "ELSE": A_SEED}));
    let c = ctx(tmp.path());
    linked(tmp.path(), "APP", "ELSE");
    let refused = mark(&c, &team(tmp.path()), "APP").unwrap_err();
    assert!(
        refused.message.contains("linked to ELSE in team"),
        "{refused:?}"
    );
    assert!(!path(&c).exists());
    assert!(filled(tmp.path(), "APP").is_err());
}

#[test]
fn a_marked_key_that_comes_to_read_another_entry_is_refused() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD, "ELSE": A_SEED}));
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), "APP").unwrap();
    linked(tmp.path(), "APP", "ELSE");
    assert_eq!(of(&c, "team", "APP", A_SEED), Mark::None);
    assert!(filled(tmp.path(), "APP").is_err());
}

#[test]
fn a_linked_entry_fills_on_the_mark_of_the_entry_it_reads() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    mark(&ctx(tmp.path()), &team(tmp.path()), "APP").unwrap();
    linked(tmp.path(), "MAIL", "APP");
    assert_eq!(filled(tmp.path(), "MAIL").unwrap(), APP_PASSWORD);
}

#[test]
fn the_persons_view_of_a_refusal_keeps_where_to_mark_the_key() {
    // As long a name as a Secret ask takes.
    let key = "K".repeat(128);
    let tmp = project(serde_json::json!({key.clone(): APP_PASSWORD}));
    let refused = filled(tmp.path(), &key).unwrap_err();
    let seen = crate::browsertimeline::fill_refused("b1", &key, &refused, 1);
    assert!(seen.moment.did.contains(MARK_WORDS), "{}", seen.moment.did);
    // So does the longer one, for a value that changed.
    let c = ctx(tmp.path());
    mark(&c, &team(tmp.path()), &key).unwrap();
    holding(tmp.path(), &serde_json::json!({key.clone(): A_SEED}));
    let refused = filled(tmp.path(), &key).unwrap_err();
    let seen = crate::browsertimeline::fill_refused("b1", &key, &refused, 1);
    assert!(seen.moment.did.contains(MARK_WORDS), "{}", seen.moment.did);
}

#[test]
fn an_empty_marks_file_holds_none_and_takes_a_mark() {
    let tmp = project(serde_json::json!({"APP": APP_PASSWORD}));
    let c = ctx(tmp.path());
    std::fs::create_dir_all(c.vaults_dir()).unwrap();
    std::fs::write(path(&c), "").unwrap();
    assert_eq!(of(&c, "team", "APP", APP_PASSWORD), Mark::None);
    mark(&c, &team(tmp.path()), "APP").unwrap();
    assert_eq!(filled(tmp.path(), "APP").unwrap(), APP_PASSWORD);
}

#[test]
fn an_empty_links_index_holds_no_link_and_a_vault_still_reads() {
    let tmp = project(serde_json::json!({"API": "fixture-not-a-seed-5237"}));
    let c = ctx(tmp.path());
    std::fs::create_dir_all(&c.state).unwrap();
    std::fs::write(crate::secrets::links::path(&c), "").unwrap();
    assert_eq!(
        cmd::get_value(&c, &team(tmp.path()), "API").unwrap(),
        "fixture-not-a-seed-5237"
    );
}

#[test]
fn a_next_step_is_said_only_where_it_would_work() {
    // A labelled address makes no code, so the refusal doesn't send the chat round again.
    let tmp = project(serde_json::json!({"APP": format!("my login: {ADDRESS}")}));
    let refused = filled(tmp.path(), "APP").unwrap_err();
    assert!(refused.contains("can't make a code"), "{refused}");
    assert!(!refused.contains("with `code`"), "{refused}");
    let tmp = project(serde_json::json!({"APP": ADDRESS}));
    assert!(
        filled(tmp.path(), "APP")
            .unwrap_err()
            .contains("with `code`")
    );
}
