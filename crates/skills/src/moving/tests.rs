// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;

/// A plane and a folder outside it, in one temporary directory.
fn plane() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let plane = tmp.path().join("plane");
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&plane).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    (tmp, plane, outside)
}

fn folder(at: &Path) {
    std::fs::create_dir_all(at).unwrap();
    std::fs::write(at.join("SKILL.md"), "body\n").unwrap();
}

#[test]
fn a_plain_folder_moves_and_the_level_folder_it_goes_into_is_made() {
    let (_tmp, plane, _) = plane();
    folder(&plane.join("skills.local/notes"));

    move_skill(
        &plane,
        &plane.join("skills.local/notes"),
        &plane.join("skills/notes"),
        1,
    )
    .unwrap();

    assert!(plane.join("skills/notes/SKILL.md").is_file());
    assert!(!plane.join("skills.local/notes").exists());
}

#[test]
fn nothing_is_moved_over_what_is_already_there() {
    let (_tmp, plane, _) = plane();
    folder(&plane.join("skills.local/notes"));
    folder(&plane.join("skills/notes"));

    let refused = move_between(
        &plane,
        &plane.join("skills.local/notes"),
        &plane,
        &plane.join("skills/notes"),
        usize::MAX,
    )
    .unwrap_err();

    assert_eq!(refused, "something of its name is already where it goes");
    assert!(plane.join("skills.local/notes/SKILL.md").is_file());
}

/// The race a look by path leaves open: any folder on the way swapped for a link after the look.
/// Each part is opened again through the one above with `O_NOFOLLOW`, so a link on the way
/// refuses the move whichever part it is, on either side.
#[cfg(unix)]
#[test]
fn a_link_at_any_folder_on_either_way_refuses_the_move_and_moves_nothing() {
    use std::os::unix::fs::symlink;
    for linked in ["workspaces", "workspaces/w", "workspaces/w/skills.local"] {
        let (_tmp, plane, outside) = plane();
        folder(&outside.join("workspaces/w/skills.local/notes"));
        let real = outside.join(linked);
        std::fs::create_dir_all(plane.join(linked).parent().unwrap()).unwrap();
        symlink(&real, plane.join(linked)).unwrap();
        // The rest of the way below the link, inside the plane where it is not the link.
        std::fs::create_dir_all(plane.join("workspaces/w/skills")).ok();

        let refused = move_between(
            &plane,
            &plane.join("workspaces/w/skills.local/notes"),
            &plane,
            &plane.join("workspaces/w/skills/notes"),
            usize::MAX,
        );

        assert!(refused.is_err(), "{linked}: {refused:?}");
        assert!(
            outside
                .join("workspaces/w/skills.local/notes/SKILL.md")
                .is_file(),
            "{linked}"
        );
    }
    for linked in ["workspaces", "workspaces/w", "workspaces/w/skills"] {
        let (_tmp, plane, outside) = plane();
        folder(&plane.join("skills.local/notes"));
        std::fs::create_dir_all(outside.join(linked)).unwrap();
        std::fs::create_dir_all(plane.join(linked).parent().unwrap()).unwrap();
        symlink(outside.join(linked), plane.join(linked)).unwrap();

        let refused = move_between(
            &plane,
            &plane.join("skills.local/notes"),
            &plane,
            &plane.join("workspaces/w/skills/notes"),
            1,
        );

        assert!(refused.is_err(), "{linked}: {refused:?}");
        assert!(
            plane.join("skills.local/notes/SKILL.md").is_file(),
            "{linked}"
        );
        assert!(
            !holds_notes(&outside),
            "{linked}: nothing was moved outside"
        );
    }
}

/// Whether anything named `notes` is anywhere under `dir`.
fn holds_notes(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|entry| {
        entry.file_name() == "notes"
            || (entry.file_type().unwrap().is_dir() && holds_notes(&entry.path()))
    })
}

#[cfg(unix)]
#[test]
fn a_link_in_the_skills_place_or_beside_it_is_never_moved() {
    use std::os::unix::fs::symlink;
    let (_tmp, plane, outside) = plane();
    folder(&outside.join("secret"));
    std::fs::create_dir_all(plane.join("skills.local")).unwrap();
    symlink(outside.join("secret"), plane.join("skills.local/notes")).unwrap();

    let refused = move_between(
        &plane,
        &plane.join("skills.local/notes"),
        &plane,
        &plane.join("skills/notes"),
        1,
    )
    .unwrap_err();

    assert_eq!(
        refused,
        "it is not a plain folder or file, so purlis does not move it"
    );

    std::fs::remove_file(plane.join("skills.local/notes")).unwrap();
    folder(&plane.join("skills.local/notes"));
    std::fs::write(outside.join("notes.toml"), "grant = \"all\"\n").unwrap();
    symlink(
        outside.join("notes.toml"),
        plane.join("skills.local/notes.toml"),
    )
    .unwrap();

    assert!(
        move_skill(
            &plane,
            &plane.join("skills.local/notes"),
            &plane.join("skills/notes"),
            1,
        )
        .is_err()
    );
    assert!(
        plane.join("skills.local/notes/SKILL.md").is_file(),
        "the folder is put back"
    );
    assert!(!plane.join("skills/notes").exists());
}
