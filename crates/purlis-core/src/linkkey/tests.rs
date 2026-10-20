// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::secrets::keyring::Store as _;

fn a_keyring(dir: &Path) -> keyring::FileStore {
    keyring::FileStore::at(dir.join("keyring.json"))
}

#[test]
fn the_desktops_link_key_is_made_once_and_read_back_the_same() {
    let (config, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let store = a_keyring(keys.path());
    let first = desktop(config.path(), &store).unwrap();
    let again = desktop(config.path(), &store).unwrap();
    assert_eq!(first.pinned(), again.pinned());
    assert_eq!(
        first.pinned().device,
        crate::machine::device_id(config.path()).unwrap()
    );
    let service = format!("{SERVICE}/{}", first.pinned().device);
    let kept = store.get(&service, ACCOUNT).unwrap().unwrap().into_inner();
    assert!(
        crate::runnerlink::is_lower_hex(&kept, 64),
        "the item holds one key as hex"
    );
    assert_ne!(
        kept,
        crate::runnerlink::hex_of(first.pinned().key.as_bytes()),
        "the item holds the private half"
    );
}

#[test]
fn another_device_has_a_key_of_its_own() {
    let keys = tempfile::tempdir().unwrap();
    let store = a_keyring(keys.path());
    let one = desktop(tempfile::tempdir().unwrap().path(), &store).unwrap();
    let other = desktop(tempfile::tempdir().unwrap().path(), &store).unwrap();
    assert_ne!(one.pinned().device, other.pinned().device);
    assert_ne!(one.pinned().key, other.pinned().key);
}

#[test]
fn an_item_that_is_not_one_key_is_refused_and_never_replaced() {
    let (config, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let store = a_keyring(keys.path());
    let device = crate::machine::device_id(config.path()).unwrap();
    let service = format!("{SERVICE}/{device}");
    for held in ["", "not a key", &"A".repeat(64), &"a".repeat(63)] {
        store.set(&service, ACCOUNT, held).unwrap();
        assert!(
            desktop(config.path(), &store).is_err(),
            "{held:?} was taken"
        );
        assert_eq!(
            store.get(&service, ACCOUNT).unwrap().unwrap().into_inner(),
            held
        );
    }
}
