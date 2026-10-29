// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::{Kept, bring, folded, relative, stage};

fn none() -> Kept {
    Kept::default()
}

fn chat_folder() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let made = tempfile::tempdir().unwrap();
    let root = made.path().join("chat");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/notes.txt"), "the notes").unwrap();
    let browser = made.path().join("browser/uploads");
    (made, root, browser)
}

#[test]
fn a_path_relative_to_the_chat_s_folder_is_its_names_and_nothing_else_is_one() {
    assert_eq!(relative("docs/./notes.txt").unwrap(), ["docs", "notes.txt"]);
    for refused in [
        "/etc/passwd",
        "~/x",
        "../x",
        "docs/../../x",
        "",
        ".",
        "a\0b",
    ] {
        assert!(relative(refused).is_err(), "{refused:?}");
    }
}

#[test]
fn an_upload_is_a_copy_in_the_browser_s_own_folder_of_a_file_in_the_chat_s_folder() {
    let (_made, root, browser) = chat_folder();
    let copies = stage(&root, &["docs/notes.txt".to_owned()], &browser, &none()).unwrap();
    assert_eq!(copies.len(), 1);
    assert!(copies[0].starts_with(&browser), "{copies:?}");
    assert_eq!(copies[0].file_name().unwrap(), "notes.txt");
    assert_eq!(std::fs::read_to_string(&copies[0]).unwrap(), "the notes");
}

#[test]
fn an_upload_from_outside_the_chat_s_folder_is_refused() {
    let (made, root, browser) = chat_folder();
    std::fs::write(made.path().join("secret"), "no").unwrap();
    for outside in ["../secret", "/etc/hosts"] {
        let refused = stage(&root, &[outside.to_owned()], &browser, &none()).unwrap_err();
        assert!(
            refused.contains("is not a path in the chat's folder"),
            "{refused}"
        );
    }
    assert!(!browser.exists(), "nothing was copied");
}

#[cfg(unix)]
#[test]
fn an_upload_through_a_link_out_of_the_chat_s_folder_is_refused() {
    let (made, root, browser) = chat_folder();
    let outside = made.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("key"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.join("key"), root.join("key")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("away")).unwrap();
    for through in ["key", "away/key"] {
        let refused = stage(&root, &[through.to_owned()], &browser, &none()).unwrap_err();
        assert!(refused.contains("link"), "{through}: {refused}");
    }
    // A link that stays inside is refused too: purlis follows none.
    std::os::unix::fs::symlink(root.join("docs/notes.txt"), root.join("same")).unwrap();
    assert!(stage(&root, &["same".to_owned()], &browser, &none()).is_err());
}

#[cfg(unix)]
#[test]
fn an_upload_of_a_file_with_a_second_name_or_of_no_plain_file_is_refused() {
    let (made, root, browser) = chat_folder();
    std::fs::write(made.path().join("elsewhere"), "secret").unwrap();
    std::fs::hard_link(made.path().join("elsewhere"), root.join("linked")).unwrap();
    let refused = stage(&root, &["linked".to_owned()], &browser, &none()).unwrap_err();
    assert!(refused.contains("more than one name"), "{refused}");
    let refused = stage(&root, &["docs".to_owned()], &browser, &none()).unwrap_err();
    assert!(refused.contains("not a plain file"), "{refused}");
}

#[test]
fn an_upload_past_its_limits_is_refused() {
    let (_made, root, browser) = chat_folder();
    let many: Vec<String> = (0..=super::MOST_UPLOAD_FILES)
        .map(|_| "docs/notes.txt".to_owned())
        .collect();
    assert!(
        stage(&root, &many, &browser, &none())
            .unwrap_err()
            .contains("at most")
    );
    let big = std::fs::File::create(root.join("big")).unwrap();
    big.set_len(super::MOST_UPLOAD_BYTES + 1).unwrap();
    assert!(
        stage(&root, &["big".to_owned()], &browser, &none())
            .unwrap_err()
            .contains("past the")
    );
    assert!(stage(&root, &[], &browser, &none()).is_err());
}

fn a_download(at: &Path) -> std::path::PathBuf {
    std::fs::create_dir_all(at).unwrap();
    let file = at.join("GUID-1");
    std::fs::write(&file, "downloaded").unwrap();
    file
}

#[test]
fn a_download_is_brought_into_the_chat_s_folder_as_a_new_file() {
    let (made, root, _) = chat_folder();
    let from = a_download(&made.path().join("held"));
    assert_eq!(bring(&from, &root, "docs/report.pdf", &none()).unwrap(), 10);
    assert_eq!(
        std::fs::read_to_string(root.join("docs/report.pdf")).unwrap(),
        "downloaded"
    );
    let refused = bring(&from, &root, "docs/report.pdf", &none()).unwrap_err();
    assert!(
        refused.contains("there is a file there already"),
        "{refused}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("docs/notes.txt")).unwrap(),
        "the notes"
    );
    assert!(bring(&from, &root, "../x", &none()).is_err());
    assert!(
        bring(&from, &root, "nowhere/x", &none())
            .unwrap_err()
            .contains("not there")
    );
}

#[cfg(unix)]
#[test]
fn a_download_is_never_brought_through_a_link() {
    let (made, root, _) = chat_folder();
    let from = a_download(&made.path().join("held"));
    let outside = made.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("away")).unwrap();
    std::os::unix::fs::symlink(outside.join("planted"), root.join("dangling")).unwrap();
    assert!(
        bring(&from, &root, "away/x", &none())
            .unwrap_err()
            .contains("link")
    );
    assert!(bring(&from, &root, "dangling", &none()).is_err());
    assert!(!outside.join("x").exists() && !outside.join("planted").exists());
}

/// A chat folder holding a folder purlis keeps, `vaults`, with a file in it.
fn with_a_kept_folder() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    Kept,
) {
    let (made, root, browser) = chat_folder();
    let vaults = root.join("vaults");
    std::fs::create_dir_all(&vaults).unwrap();
    std::fs::write(vaults.join("forge.json"), "secret").unwrap();
    let kept = Kept::of_paths(&[vaults.clone(), vaults.join("forge.json")]);
    (made, root, browser, kept)
}

#[test]
fn an_upload_of_what_purlis_keeps_is_refused_by_what_it_is_however_it_is_spelled() {
    let (_made, root, browser, kept) = with_a_kept_folder();
    // The spelling it has, and spellings a disk that folds names opens as the same file: each
    // is refused, whether this disk opens it (by what it is) or not (it is not there).
    for spelled in [
        "vaults/forge.json",
        "VAULTS/forge.json",
        "vault\u{17f}/forge.json",
        "vaults/forge.j\u{17f}on",
        "./vaults/./forge.json",
    ] {
        let refused = stage(&root, &[spelled.to_owned()], &browser, &kept).unwrap_err();
        assert!(
            refused.contains("keeps from every chat") || refused.contains("is not there"),
            "{spelled}: {refused}"
        );
    }
    // The spelling it has is refused by what it is, on every disk.
    let refused = stage(&root, &["vaults/forge.json".to_owned()], &browser, &kept).unwrap_err();
    assert!(refused.contains("keeps from every chat"), "{refused}");
    // And a chat folder inside a kept folder reads nothing.
    let refused = stage(
        &root.join("vaults"),
        &["forge.json".to_owned()],
        &browser,
        &kept,
    )
    .unwrap_err();
    assert!(refused.contains("keeps from every chat"), "{refused}");
    // What is not kept is read.
    assert!(stage(&root, &["docs/notes.txt".to_owned()], &browser, &kept).is_ok());
}

#[test]
fn a_download_is_never_brought_into_a_folder_purlis_keeps() {
    let (made, root, _browser, kept) = with_a_kept_folder();
    let from = a_download(&made.path().join("held"));
    let refused = bring(&from, &root, "vaults/planted.json", &kept).unwrap_err();
    assert!(refused.contains("keeps from every chat"), "{refused}");
    assert!(!root.join("vaults/planted.json").exists());
}

#[test]
fn a_spelling_is_judged_as_a_folding_disk_reads_it() {
    assert_eq!(folded("Vault\u{17f}"), "vaults");
    assert_eq!(folded("\u{fb01}le.TOML"), "file.toml");
    assert_eq!(folded("ｐｕｒｌｉｓ"), "purlis");
}
