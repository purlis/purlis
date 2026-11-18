// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a project")
}

const CHAT: &str = "01JBROWSERPOSTURE0000000001";

#[test]
fn an_answer_is_found_again_by_the_same_chat_after_a_restart() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Allowed { open_web: true }).expect("kept");

    assert_eq!(
        kept(dir.path(), CHAT),
        Some(Answer::Allowed { open_web: true })
    );
    keep(dir.path(), CHAT, Answer::Off).expect("kept again");
    assert_eq!(kept(dir.path(), CHAT), Some(Answer::Off));
}

#[test]
fn another_chat_finds_no_answer_so_it_asks() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Allowed { open_web: false }).expect("kept");

    assert_eq!(kept(dir.path(), "01JBROWSERPOSTURE0000000002"), None);
}

#[test]
fn an_answer_ends_with_its_chat_and_the_file_goes_with_the_last() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Off).expect("kept");
    keep(dir.path(), "other-chat", Answer::Off).expect("kept");

    remove(dir.path(), CHAT).expect("removed");
    assert_eq!(kept(dir.path(), CHAT), None);
    assert_eq!(kept(dir.path(), "other-chat"), Some(Answer::Off));
    remove(dir.path(), "other-chat").expect("removed");
    assert!(!path(dir.path()).exists(), "nothing left to keep");
    remove(dir.path(), CHAT).expect("a second end is no failure");
}

#[test]
fn it_is_kept_in_the_state_folder_never_the_plane_and_privately() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Off).expect("kept");

    let path = path(dir.path());
    assert_eq!(
        path,
        crate::names::state(dir.path()).join("app/browser-postures.json"),
        "the clone's state folder, which is never committed or synced"
    );
    let written: Vec<_> = walk(dir.path());
    assert_eq!(
        written,
        std::slice::from_ref(&path),
        "nothing else is written, the plane's files least"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(CHAT) && !text.contains("http"),
        "an identity and a word, no site: {text}"
    );
}

#[test]
fn a_file_that_does_not_read_or_names_an_unknown_word_gives_no_answer() {
    let dir = project();
    let file = path(dir.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not json").unwrap();
    assert_eq!(kept(dir.path(), CHAT), None);

    std::fs::write(&file, format!("{{\"{CHAT}\": \"everything\"}}")).unwrap();
    assert_eq!(kept(dir.path(), CHAT), None, "an unknown word asks again");
}

#[cfg(unix)]
#[test]
fn a_link_in_the_files_place_is_never_followed() {
    let dir = project();
    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(&elsewhere, format!("{{\"{CHAT}\": \"allowed-open-web\"}}")).unwrap();
    let file = path(dir.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &file).unwrap();

    assert_eq!(kept(dir.path(), CHAT), None);
}

#[test]
fn an_identity_that_is_no_identity_is_refused_and_never_found() {
    let dir = project();
    assert!(keep(dir.path(), "../x", Answer::Off).is_err());
    assert!(keep(dir.path(), "", Answer::Off).is_err());
    assert_eq!(kept(dir.path(), "../x"), None);
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                todo.push(entry.path());
            } else {
                found.push(entry.path());
            }
        }
    }
    found
}
