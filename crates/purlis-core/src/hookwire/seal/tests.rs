// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plane() -> &'static Path {
    Path::new("/projects/one")
}
const HASH: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

#[test]
fn a_sealed_digest_opens_with_the_chats_key_and_is_not_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), plane(), 4).unwrap();
    let sealed = seal(&public, HASH).unwrap();
    assert!(is_sealed(&sealed) && !sealed.contains(HASH));
    assert_eq!(open(dir.path(), plane(), 4, &sealed).as_deref(), Some(HASH));
}

#[test]
fn equal_hashes_seal_to_different_text() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), plane(), 4).unwrap();
    assert_ne!(seal(&public, HASH), seal(&public, HASH));
}

#[test]
fn the_same_chat_number_has_the_same_key_on_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        public_for(dir.path(), plane(), 4).unwrap(),
        public_for(dir.path(), plane(), 4).unwrap()
    );
    assert_ne!(
        public_for(dir.path(), plane(), 4).unwrap(),
        public_for(dir.path(), plane(), 5).unwrap()
    );
}

#[test]
fn a_missing_key_or_another_chats_key_opens_nothing_and_makes_no_key() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), plane(), 4).unwrap();
    let sealed = seal(&public, HASH).unwrap();
    assert_eq!(open(dir.path(), plane(), 5, &sealed), None);
    assert!(!key_path(dir.path(), plane(), 5).exists());
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(open(elsewhere.path(), plane(), 4, &sealed), None);
    assert_eq!(
        open(dir.path(), plane(), 4, HASH),
        None,
        "a plain digest is not sealed"
    );
}

#[test]
fn a_public_key_that_is_not_one_seals_nothing() {
    assert_eq!(seal("not a key", HASH), None);
}

#[test]
fn one_chat_number_has_a_key_for_each_plane() {
    let dir = tempfile::tempdir().unwrap();
    let other = Path::new("/projects/two");
    let public = public_for(dir.path(), plane(), 4).unwrap();
    assert_ne!(public, public_for(dir.path(), other, 4).unwrap());
    let sealed = seal(&public, HASH).unwrap();
    assert_eq!(open(dir.path(), plane(), 4, &sealed).as_deref(), Some(HASH));
    assert_eq!(
        open(dir.path(), other, 4, &sealed),
        None,
        "chat 4 of another project's key opens nothing sealed to this one's"
    );
}

#[test]
fn a_blob_of_32_bytes_or_fewer_opens_nothing() {
    let dir = tempfile::tempdir().unwrap();
    public_for(dir.path(), plane(), 4).unwrap();
    for length in [0, 1, 31, 32] {
        let blob = format!("{SEALED}{}", encode(&vec![7; length]));
        assert_eq!(open(dir.path(), plane(), 4, &blob), None, "{length} bytes");
    }
}

#[test]
fn a_chats_key_is_deleted_and_no_other_chats_or_planes() {
    let dir = tempfile::tempdir().unwrap();
    let other = Path::new("/projects/two");
    for (plane, chat) in [(plane(), 4), (plane(), 5), (other, 4)] {
        public_for(dir.path(), plane, chat).unwrap();
    }
    forget(dir.path(), plane(), 4).unwrap();
    assert!(!key_path(dir.path(), plane(), 4).exists());
    assert!(key_path(dir.path(), plane(), 5).exists());
    assert!(key_path(dir.path(), other, 4).exists());
    forget(dir.path(), plane(), 4).expect("a key that is gone is done");
}

#[test]
fn only_the_ended_chats_of_the_plane_lose_their_keys() {
    let dir = tempfile::tempdir().unwrap();
    let other = Path::new("/projects/two");
    for (plane, chat) in [(plane(), 4), (plane(), 5), (other, 4)] {
        public_for(dir.path(), plane, chat).unwrap();
    }
    forget_ended(dir.path(), plane(), |chat| chat == 4).unwrap();
    assert!(!key_path(dir.path(), plane(), 4).exists());
    assert!(key_path(dir.path(), plane(), 5).exists());
    assert!(key_path(dir.path(), other, 4).exists());
}

#[cfg(target_os = "macos")]
#[test]
fn the_seal_folder_is_left_out_of_backups() {
    let dir = tempfile::tempdir().unwrap();
    public_for(dir.path(), plane(), 4).unwrap();
    let set = xattr::get(dir.path().join(DIR), crate::browserlogins::BACKUP_EXCLUSION)
        .expect("readable")
        .expect("set");
    assert_eq!(set, crate::browserlogins::BACKUP_EXCLUDED);
}
