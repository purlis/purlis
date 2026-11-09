// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::{keep, rehome, remove};
use crate::ownwork::pieces::{Kind, Piece};

/// A chat's own temporary folder, as `own_tmp` makes one, with a shell's output in it.
fn a_chat_folder(base: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let tmp = base.join("charter-chat-AbC123");
    let output = tmp
        .join("claude-501")
        .join("-proj")
        .join("conv-1")
        .join("tasks")
        .join("b1.output");
    std::fs::create_dir_all(output.parent().expect("a folder")).expect("made");
    std::fs::write(&output, "test result: ok\n").expect("written");
    (tmp, output)
}

#[test]
fn a_kept_folder_holds_what_the_chat_s_folder_held_under_a_name_of_its_own() {
    let base = tempfile::tempdir().expect("a folder");
    let (tmp, _) = a_chat_folder(base.path());

    let kept = keep(&tmp).expect("kept");

    assert!(!tmp.exists(), "the chat's folder is moved, not copied");
    assert_eq!(kept.parent(), tmp.parent(), "beside it, on the same disk");
    assert!(
        kept.join("claude-501/-proj/conv-1/tasks/b1.output")
            .is_file(),
        "{}",
        kept.display()
    );
}

#[test]
fn what_drops_the_chat_s_folder_after_it_was_kept_removes_nothing_of_it() {
    let base = tempfile::tempdir().expect("a folder");
    let tmp = tempfile::Builder::new()
        .prefix("charter-chat-")
        .tempdir_in(base.path())
        .expect("a chat's folder");
    std::fs::write(tmp.path().join("b1.output"), "line\n").expect("written");

    let kept = keep(tmp.path()).expect("kept");
    drop(tmp);

    assert!(kept.join("b1.output").is_file());
}

#[test]
fn two_folders_kept_from_one_name_each_get_their_own() {
    let base = tempfile::tempdir().expect("a folder");
    let (tmp, _) = a_chat_folder(base.path());
    let first = keep(&tmp).expect("kept");
    let (again, _) = a_chat_folder(base.path());

    let second = keep(&again).expect("kept");

    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir());
}

#[test]
fn a_piece_whose_output_was_in_the_chat_s_folder_names_it_where_it_was_kept() {
    let base = tempfile::tempdir().expect("a folder");
    let (tmp, output) = a_chat_folder(base.path());
    let mut pieces = vec![
        Piece {
            output: Some(output.display().to_string()),
            ..Piece::new(Kind::Shell, "b1", "cargo test").expect("a piece")
        },
        Piece {
            output: Some("/elsewhere/conv-1/tasks/b2.output".to_owned()),
            ..Piece::new(Kind::Shell, "b2", "npm test").expect("a piece")
        },
    ];
    let kept = keep(&tmp).expect("kept");

    rehome(&mut pieces, &tmp, &kept);

    assert_eq!(
        pieces[0].output.as_deref(),
        Some(
            kept.join("claude-501/-proj/conv-1/tasks/b1.output")
                .display()
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        pieces[1].output.as_deref(),
        Some("/elsewhere/conv-1/tasks/b2.output"),
        "a path outside the chat's folder is left as it was"
    );
}

#[test]
fn a_kept_folder_is_removed() {
    let base = tempfile::tempdir().expect("a folder");
    let (tmp, _) = a_chat_folder(base.path());
    let kept = keep(&tmp).expect("kept");

    remove(&kept);

    assert!(!kept.exists());
}

#[test]
fn a_folder_that_is_no_kept_folder_is_never_removed() {
    let base = tempfile::tempdir().expect("a folder");
    let (tmp, _) = a_chat_folder(base.path());
    let other = base.path().join("someone-else");
    std::fs::create_dir_all(&other).expect("made");

    remove(&tmp);
    remove(&other);
    remove(base.path());

    assert!(tmp.is_dir(), "a chat's live folder is not a kept one");
    assert!(other.is_dir());
}

#[cfg(unix)]
#[test]
fn a_link_named_as_a_kept_folder_is_not_followed() {
    let base = tempfile::tempdir().expect("a folder");
    let target = base.path().join("precious");
    std::fs::create_dir_all(&target).expect("made");
    std::fs::write(target.join("file"), "keep me").expect("written");
    let link = base.path().join("charter-chat-AbC123-kept");
    std::os::unix::fs::symlink(&target, &link).expect("linked");

    remove(&link);

    assert!(target.join("file").is_file());
}
