// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A project's hook socket, under the state folder whose spool a sandboxed chat is denied.
fn a_projects_socket(root: &Path) -> std::path::PathBuf {
    let app = root.join(".purlis").join("app");
    std::fs::create_dir_all(&app).expect("the app folder");
    app.join("hooks.sock")
}

#[test]
fn the_hook_reads_back_the_key_its_chat_was_armed_with_and_no_other_chats() {
    let root = tempfile::tempdir().expect("a directory");
    let socket = a_projects_socket(root.path());
    let spool = crate::hookwire::spool::dir_for(&socket);
    let key = ArmedKey::new().expect("a key");

    arm(&spool, 4, &key).expect("written");

    assert_eq!(ArmedKey::read(&socket, 4), Some(key.clone()));
    assert_eq!(
        ArmedKey::read(&socket, 5),
        None,
        "chat 5 was armed with nothing"
    );
}

#[test]
fn a_key_is_written_for_its_owner_alone() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().expect("a directory");
    let spool = crate::hookwire::spool::dir_for(&a_projects_socket(root.path()));

    arm(&spool, 4, &ArmedKey::new().expect("a key")).expect("written");

    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&spool.join(DIR)), 0o700);
    assert_eq!(mode(&spool.join(DIR).join("4")), 0o600);
}

#[test]
fn a_disarmed_chat_reads_no_key() {
    let root = tempfile::tempdir().expect("a directory");
    let socket = a_projects_socket(root.path());
    let spool = crate::hookwire::spool::dir_for(&socket);
    arm(&spool, 4, &ArmedKey::new().expect("a key")).expect("written");
    arm(&spool, 6, &ArmedKey::new().expect("a key")).expect("written");

    disarm(&spool, 4);
    assert_eq!(ArmedKey::read(&socket, 4), None);

    arm(&spool, 7, &ArmedKey::new().expect("a key")).expect("written");
    disarm_ended(&spool, &|chat| chat == 6);
    assert_eq!(ArmedKey::read(&socket, 6), None, "an ended chat's key goes");
    assert!(
        ArmedKey::read(&socket, 7).is_some(),
        "a running chat's key stays"
    );
}

#[test]
fn nothing_is_armed_beside_a_socket_a_sandboxed_chat_is_not_denied() {
    let root = tempfile::tempdir().expect("a directory");
    // The fallback beside a socket elsewhere: no denial covers it, so no key is written there
    // and none read.
    let socket = root.path().join("hooks.sock");
    let spool = crate::hookwire::spool::dir_for(&socket);

    assert!(arm(&spool, 4, &ArmedKey::new().expect("a key")).is_err());
    assert!(!spool.join(DIR).exists());
    std::fs::create_dir_all(spool.join(DIR)).unwrap();
    std::fs::write(file(&spool, 4), "ab".repeat(A_KEY_IS)).unwrap();
    assert_eq!(ArmedKey::read(&socket, 4), None);
}

#[test]
fn a_file_that_is_not_a_key_reads_as_none() {
    let root = tempfile::tempdir().expect("a directory");
    let socket = a_projects_socket(root.path());
    let at = crate::hookwire::spool::dir_for(&socket).join(DIR);
    std::fs::create_dir_all(&at).unwrap();
    for (chat, text) in [
        (1, "short".to_owned()),
        (2, "zz".repeat(A_KEY_IS)),
        (3, "ab".repeat(4096)),
    ] {
        std::fs::write(at.join(chat.to_string()), &text).unwrap();
        assert_eq!(ArmedKey::read(&socket, chat), None, "{text:.20}");
    }
}

#[test]
fn a_key_shows_nothing_of_itself_when_printed() {
    let key = ArmedKey::new().expect("a key");
    assert_eq!(format!("{key:?}"), "ArmedKey(..)");
}

#[test]
fn a_key_is_matched_whole_and_only_by_its_own_value() {
    let key = ArmedKey::new().expect("a key");
    assert!(key.matches(Some(key.expose())));
    assert!(!key.matches(None));
    assert!(!key.matches(Some("")));
    assert!(!key.matches(Some(&key.expose()[1..])));
    assert!(!key.matches(Some(ArmedKey::new().expect("another").expose())));
}
