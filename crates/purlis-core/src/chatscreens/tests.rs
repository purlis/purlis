// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    (dir, root)
}

/// A project whose local settings git ignores, as `purlis init` leaves one.
fn settable() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = project();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::write(root.join(".gitignore"), "charter.local.toml\n").unwrap();
    crate::testgit::run(&root, &["init", "-q", "-b", "main", "."]);
    (dir, root)
}

const CHAT: &str = "01K6W0RKER000000000000000B";
const OTHER: &str = "01K6W0RKER000000000000000C";

#[test]
fn a_kept_screen_reads_back_as_it_was_kept() {
    let (_dir, root) = project();
    keep(&root, CHAT, "\x1b[1mSigned all 12.\x1b[0m\r\n").unwrap();
    assert_eq!(
        kept(&root, CHAT).as_deref(),
        Some("\x1b[1mSigned all 12.\x1b[0m\r\n")
    );
    assert!(is_kept(&root, CHAT));
}

#[test]
fn nothing_kept_reads_as_none() {
    let (_dir, root) = project();
    assert_eq!(kept(&root, CHAT), None);
    assert!(!is_kept(&root, CHAT));
}

#[test]
fn a_name_that_is_not_a_ulid_is_refused() {
    let (_dir, root) = project();
    assert!(keep(&root, "../escape", "x").is_err());
    assert_eq!(kept(&root, "../escape"), None);
}

#[test]
fn a_screen_past_the_cap_is_not_kept() {
    let (_dir, root) = project();
    let big = "x".repeat(MOST_SCREEN_BYTES + 1);
    assert!(keep(&root, CHAT, &big).is_err());
    assert!(!is_kept(&root, CHAT));
}

#[cfg(unix)]
#[test]
fn a_kept_screen_is_private() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    keep(&root, CHAT, "x").unwrap();
    let file = std::fs::metadata(dir(&root).join(format!("{CHAT}.screen"))).unwrap();
    assert_eq!(file.permissions().mode() & 0o777, 0o600);
    let folder = std::fs::metadata(dir(&root)).unwrap();
    assert_eq!(folder.permissions().mode() & 0o777, 0o700);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_store_is_never_read() {
    let (_dir, root) = project();
    keep(&root, OTHER, "x").unwrap();
    let elsewhere = root.join("secret.txt");
    std::fs::write(&elsewhere, "not a screen").unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir(&root).join(format!("{CHAT}.screen"))).unwrap();
    assert_eq!(kept(&root, CHAT), None);
}

#[test]
fn the_sweep_takes_the_screens_no_record_names() {
    let (_dir, root) = project();
    keep(&root, CHAT, "kept").unwrap();
    keep(&root, OTHER, "gone").unwrap();
    assert_eq!(sweep(&root, |id| id == CHAT), 1);
    assert!(is_kept(&root, CHAT));
    assert!(!is_kept(&root, OTHER));
}

#[test]
fn forgetting_all_takes_every_screen() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    keep(&root, OTHER, "b").unwrap();
    assert_eq!(forget_all(&root), 2);
    assert!(!is_kept(&root, CHAT) && !is_kept(&root, OTHER));
}

#[test]
fn a_project_s_open_keeps_the_screens_its_records_name_and_takes_the_rest() {
    use crate::dispatchrecord::{self, ChatRef, Mode, Opening, Worker};
    let (_dir, root) = project();
    let opening = Opening {
        mode: Mode::Task,
        asker: Default::default(),
        persona: None,
        worker: Worker {
            chat: ChatRef {
                chat: 641,
                id: Some(CHAT.to_owned()),
                ..ChatRef::default()
            },
            ..Worker::default()
        },
        task: Some("task".to_owned()),
        place: Default::default(),
        brief: "brief".to_owned(),
        report_owed: true,
    };
    dispatchrecord::open(&root, opening, chrono::Utc::now()).unwrap();
    keep(&root, CHAT, "named").unwrap();
    keep(&root, OTHER, "named by nothing").unwrap();
    crate::retention::sweep(&root, std::time::SystemTime::now(), &[]);
    assert!(is_kept(&root, CHAT));
    assert!(!is_kept(&root, OTHER));
}

#[test]
fn screens_are_kept_where_nothing_is_set() {
    let (_dir, root) = project();
    assert!(keeps(&root));
}

#[test]
fn turning_keeping_off_takes_every_kept_screen_and_keeps_no_more() {
    let (_dir, root) = settable();
    keep(&root, CHAT, "a").unwrap();
    set_keep(&root, false).unwrap();
    assert!(!keeps(&root));
    assert!(!is_kept(&root, CHAT));
    assert!(keep(&root, OTHER, "b").is_err());
    assert!(!is_kept(&root, OTHER));
    set_keep(&root, true).unwrap();
    keep(&root, OTHER, "b").unwrap();
    assert!(is_kept(&root, OTHER));
}

#[test]
fn a_hand_edit_that_turns_it_off_is_caught_by_the_sweep() {
    let (_dir, root) = project();
    keep(&root, CHAT, "a").unwrap();
    std::fs::write(
        root.join(crate::settings::Which::Local.file_at(&root)),
        "[task_screens]\nkeep = false\n",
    )
    .unwrap();
    assert_eq!(sweep(&root, |_| true), 1);
    assert!(!is_kept(&root, CHAT));
}

#[test]
fn the_setting_in_the_committed_file_is_refused() {
    let (_dir, root) = project();
    let said = keyed_at(
        &root,
        "[task_screens]\nkeep = false\n",
        crate::settings::Which::Shared,
    );
    assert_eq!(said.len(), 1);
    assert!(
        keyed_at(
            &root,
            "[task_screens]\nkeep = false\n",
            crate::settings::Which::Local
        )
        .is_empty()
    );
    assert_eq!(
        keyed_at(
            &root,
            "[task_screens]\nkeep = 1\n",
            crate::settings::Which::Local
        )
        .len(),
        1
    );
}
