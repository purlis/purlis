use super::*;

const HASH: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

#[test]
fn a_sealed_digest_opens_with_the_chats_key_and_is_not_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), 4).unwrap();
    let sealed = seal(&public, HASH).unwrap();
    assert!(is_sealed(&sealed) && !sealed.contains(HASH));
    assert_eq!(open(dir.path(), 4, &sealed).as_deref(), Some(HASH));
}

#[test]
fn equal_hashes_seal_to_different_text() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), 4).unwrap();
    assert_ne!(seal(&public, HASH), seal(&public, HASH));
}

#[test]
fn the_same_chat_number_has_the_same_key_on_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        public_for(dir.path(), 4).unwrap(),
        public_for(dir.path(), 4).unwrap()
    );
    assert_ne!(
        public_for(dir.path(), 4).unwrap(),
        public_for(dir.path(), 5).unwrap()
    );
}

#[test]
fn a_missing_key_or_another_chats_key_opens_nothing_and_makes_no_key() {
    let dir = tempfile::tempdir().unwrap();
    let public = public_for(dir.path(), 4).unwrap();
    let sealed = seal(&public, HASH).unwrap();
    assert_eq!(open(dir.path(), 5, &sealed), None);
    assert!(!key_path(dir.path(), 5).exists());
    let elsewhere = tempfile::tempdir().unwrap();
    assert_eq!(open(elsewhere.path(), 4, &sealed), None);
    assert_eq!(
        open(dir.path(), 4, HASH),
        None,
        "a plain digest is not sealed"
    );
}

#[test]
fn a_public_key_that_is_not_one_seals_nothing() {
    assert_eq!(seal("not a key", HASH), None);
}
