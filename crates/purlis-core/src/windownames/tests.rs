// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::fs::PermissionsExt;

use super::*;

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_roots_name_is_128_random_bits_kept_the_same_across_restarts() {
    let config = tempfile::tempdir().unwrap();
    let (a, b) = (Path::new("/projects/a"), Path::new("/projects/b"));

    let first = name_for(config.path(), a).unwrap();
    assert!(is_a_name(first.expose()), "{}", first.expose().len());
    // As the window that comes back asks again.
    assert_eq!(name_for(config.path(), a).unwrap(), first);
    assert_ne!(name_for(config.path(), b).unwrap(), first);

    let elsewhere = tempfile::tempdir().unwrap();
    assert_ne!(name_for(elsewhere.path(), a).unwrap(), first);
}

#[test]
fn the_names_are_this_users_alone_in_the_session_hosts_folder() {
    let config = tempfile::tempdir().unwrap();
    name_for(config.path(), Path::new("/projects/a")).unwrap();

    let folder = crate::machine::charterd(config.path());
    assert_eq!(mode(&folder), 0o700);
    assert_eq!(mode(&folder.join(FILE)), 0o600);
    assert_eq!(mode(&folder.join(LOCK)), 0o600);
}

#[test]
fn a_name_never_shows_in_debug() {
    let config = tempfile::tempdir().unwrap();
    let name = name_for(config.path(), Path::new("/projects/a")).unwrap();

    let shown = format!("{name:?}");
    assert!(!shown.contains(name.expose()), "{shown}");
}

#[test]
fn a_file_that_does_not_read_is_started_again() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::charterd(config.path())).unwrap();
    folder.replace_file(FILE, b"not json").unwrap();

    let name = name_for(config.path(), Path::new("/projects/a")).unwrap();
    assert!(is_a_name(name.expose()));
}

#[test]
fn a_file_another_user_could_read_is_refused() {
    let config = tempfile::tempdir().unwrap();
    name_for(config.path(), Path::new("/projects/a")).unwrap();
    let file = crate::machine::charterd(config.path()).join(FILE);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    assert!(name_for(config.path(), Path::new("/projects/a")).is_err());
}
