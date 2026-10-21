// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;

/// A folder of files with a folder of files in it, outside the holds: what a link may point a
/// note at. Answers every path in it, to tell nothing was written or removed there.
fn outside(tmp: &Path) -> (PathBuf, Vec<PathBuf>) {
    let outside = tmp.join("outside");
    std::fs::create_dir_all(outside.join("inner")).unwrap();
    std::fs::write(outside.join("file"), "mine").unwrap();
    std::fs::write(outside.join("inner/4242"), "mine").unwrap();
    let all = everything_in(&outside);
    (outside, all)
}

fn everything_in(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(next) = todo.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if std::fs::symlink_metadata(&path).unwrap().is_dir() {
                todo.push(path.clone());
            }
            all.push(path);
        }
    }
    all.sort();
    all
}

#[test]
fn a_hold_through_a_link_is_refused_and_nothing_is_written_there() {
    let tmp = tempfile::tempdir().unwrap();
    let (outside, before) = outside(tmp.path());
    // The folder a hold is in, a link.
    let sets = tmp.path().join("sets");
    std::fs::create_dir_all(&sets).unwrap();
    std::os::unix::fs::symlink(&outside, sets.join(".held")).unwrap();
    // A hold that is itself a link, in a folder that is not.
    let other = tmp.path().join("other");
    std::fs::create_dir_all(other.join(".held")).unwrap();
    std::os::unix::fs::symlink(outside.join("inner"), other.join(".held/digest")).unwrap();

    for hold in [sets.join(".held/digest"), other.join(".held/digest")] {
        let refused = open_all(std::slice::from_ref(&hold));
        assert!(refused.is_err(), "{}", hold.display());
    }
    assert_eq!(everything_in(&outside), before);
}

#[test]
fn a_link_put_in_place_of_a_hold_after_it_was_opened_gets_no_note_and_loses_none() {
    let tmp = tempfile::tempdir().unwrap();
    let (outside, before) = outside(tmp.path());
    std::fs::create_dir_all(tmp.path().join("sets")).unwrap();
    let hold = tmp.path().join("sets/.held/digest");
    let holds = open_all(std::slice::from_ref(&hold)).unwrap();
    // Between the check and the note: the hold moved aside, and a link to a folder holding a
    // file named like the note put in its place.
    let aside = tmp.path().join("sets/.held/aside");
    std::fs::rename(&hold, &aside).unwrap();
    std::os::unix::fs::symlink(outside.join("inner"), &hold).unwrap();

    let notes = write_all(holds, Some(4242)).unwrap();

    assert!(
        aside.join("4242").is_file(),
        "the note is in the folder that was checked"
    );
    assert_eq!(everything_in(&outside), before);
    drop(notes);
    assert!(!aside.join("4242").exists(), "and it goes from there");
    assert_eq!(everything_in(&outside), before);
}
