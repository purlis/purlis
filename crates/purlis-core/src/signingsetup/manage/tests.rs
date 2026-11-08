// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::Kind;
use crate::forge::recorded::Recorded;
use crate::forge::signingkeys::recording_of;
use crate::secrets::keyring::FileStore;

/// A device with a signing key kept in the fenced file keychain.
fn a_device_with_a_key() -> (tempfile::TempDir, tempfile::TempDir, FileStore, Signer) {
    let config = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    crate::machine::device_id(config.path()).unwrap();
    let store = FileStore::at(keys.path().join("keyring.json"));
    let signer = commitsign::make(config.path(), &store).unwrap();
    (config, keys, store, signer)
}

fn on<'a>(kind: Kind, transport: &'a Recorded) -> OnAccount<'a> {
    OnAccount {
        forge: Forge::default_of(kind),
        transport,
    }
}

#[test]
fn remove_signing_leaves_no_key_in_the_keychain_and_none_on_either_account() {
    let (config, _keys, store, signer) = a_device_with_a_key();
    let line = signer.public_line();
    let github = recording_of("github", "remove_everywhere", &[("@KEY@", &line)]);
    let gitlab = recording_of("gitlab", "remove_everywhere", &[("@KEY@", &line)]);
    let done = remove_everywhere(
        config.path(),
        &store,
        &[on(Kind::GitHub, &github), on(Kind::GitLab, &gitlab)],
    )
    .unwrap();
    assert_eq!(done, [Outcome::Done, Outcome::Done]);
    assert_eq!(commitsign::kept(config.path(), &store).unwrap(), None);
    assert!(github.unspent().is_empty() && gitlab.unspent().is_empty());
}

#[test]
fn remove_signing_turns_signing_off_even_where_a_forge_refuses() {
    let (config, _keys, store, _signer) = a_device_with_a_key();
    let refusing = Recorded::parse(
        r#"{"source": "a forge that refuses the list", "exchanges": [
        {"call": {"endpoint": {"rest": {"method": null, "path": "user/ssh_signing_keys"}}, "fields": [{"typed": ["per_page", "100"]}]},
         "reply": {"code": 1, "out": "", "err": "Not Found (HTTP 404)", "status": 404}}]}"#,
    )
    .unwrap();
    let done = remove_everywhere(config.path(), &store, &[on(Kind::GitHub, &refusing)]).unwrap();
    assert!(matches!(&done[0], Outcome::Refused(why) if why.contains("may not")));
    assert_eq!(commitsign::kept(config.path(), &store).unwrap(), None);
}

#[test]
fn replace_key_adds_the_new_one_and_removes_the_old_one_where_it_was() {
    let (config, _keys, store, old) = a_device_with_a_key();
    let new = Signer::generate().unwrap();
    let github = recording_of(
        "github",
        "replaced",
        &[("@OLD@", &old.public_line()), ("@NEW@", &new.public_line())],
    );
    let (kept, done) = replace_with(
        config.path(),
        &store,
        &[on(Kind::GitHub, &github)],
        "purlis · studio",
        new.clone(),
    )
    .unwrap();
    assert_eq!(kept, new);
    assert_eq!(done, [Outcome::Done]);
    assert_eq!(commitsign::kept(config.path(), &store).unwrap(), Some(new));
    assert!(github.unspent().is_empty());
}

#[test]
fn another_devices_key_is_removed_from_this_device_and_nothing_else_is() {
    let (_config, _keys, _store, signer) = a_device_with_a_key();
    let github = recording_of(
        "github",
        "other_device_removed",
        &[("@KEY@", &signer.public_line())],
    );
    let account = on(Kind::GitHub, &github);
    remove_other_device(&account, Some(&signer), 17).unwrap();
    assert!(github.unspent().is_empty());

    // This device's own key, and the person's own key, are not another device's.
    for id in [4242, 9] {
        let github = recording_of(
            "github",
            "other_device_removed",
            &[("@KEY@", &signer.public_line())],
        );
        let account = on(Kind::GitHub, &github);
        assert!(
            remove_other_device(&account, Some(&signer), id).is_err(),
            "{id}"
        );
    }
}
