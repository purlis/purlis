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
        &plane,
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

    assert_eq!(
        refused,
        Unmoved::Nothing("something of its name is already where it goes".to_owned())
    );
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
        refused.to_string(),
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

    assert_eq!(
        move_skill(
            &plane,
            &plane.join("skills.local/notes"),
            &plane,
            &plane.join("skills/notes"),
            1,
        ),
        Err(Unmoved::Nothing(
            "skills.local/notes.toml could not go with it (it is not a plain folder or file, so \
             purlis does not move it), so nothing was moved"
                .to_owned()
        ))
    );
    assert!(
        plane.join("skills.local/notes/SKILL.md").is_file(),
        "the folder is put back"
    );
    assert!(!plane.join("skills/notes").exists());
}

/// The race the look at the leaf leaves open: the folder swapped for a link after its look and
/// before its rename. The rename moves the link; the look after it sees another entry, so the
/// link is put back and the move refused.
#[cfg(unix)]
#[test]
fn a_leaf_swapped_for_a_link_after_its_look_is_put_back_and_the_move_refused() {
    use std::os::unix::fs::symlink;
    let (_tmp, plane, outside) = plane();
    folder(&plane.join("skills.local/notes"));
    folder(&outside.join("secret"));
    let (aside, secret) = (plane.join("aside"), outside.join("secret"));
    let mut first = true;
    let _hook = hook::set(move |moment, from, _| {
        if moment == hook::Moment::BeforeRename && std::mem::take(&mut first) {
            std::fs::rename(from, &aside).unwrap();
            symlink(&secret, from).unwrap();
        }
    });

    let refused = move_between(
        &plane,
        &plane.join("skills.local/notes"),
        &plane,
        &plane.join("skills/notes"),
        1,
    )
    .unwrap_err();

    assert_eq!(
        refused.to_string(),
        "it changed as it was moved, so it was put back"
    );
    assert!(
        plane
            .join("skills.local/notes")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink(),
        "the link is put back where it was"
    );
    assert!(plane.join("skills/notes").symlink_metadata().is_err());
    assert!(plane.join("aside/SKILL.md").is_file());
}

/// The swap above, and the leaf's place taken again before the put-back: the move says what is
/// where, not that nothing moved.
#[cfg(unix)]
#[test]
fn a_swapped_leaf_that_cannot_be_put_back_is_said_to_be_where_it_went() {
    use std::os::unix::fs::symlink;
    let (_tmp, plane, outside) = plane();
    folder(&plane.join("skills.local/notes"));
    folder(&outside.join("secret"));
    let (aside, secret) = (plane.join("aside"), outside.join("secret"));
    let mut call = 0;
    let _hook = hook::set(move |moment, from, to| {
        if moment != hook::Moment::BeforeRename {
            return;
        }
        call += 1;
        if call == 1 {
            std::fs::rename(from, &aside).unwrap();
            symlink(&secret, from).unwrap();
        } else {
            // The put-back, from where it went to where it was: something is there now.
            folder(to);
        }
    });

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
        Unmoved::Stranded(
            "it changed as it was moved and could not be put back (something of its name is \
             already where it goes): what is there now is at skills/notes"
                .to_owned()
        )
    );
    assert!(
        plane
            .join("skills/notes")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// The folder moved, the file beside it could not (a link), and the folder's way back is taken:
/// the move says the folder went and the file stayed, never that nothing moved.
#[cfg(unix)]
#[test]
fn a_failed_put_back_says_the_folder_moved_and_the_file_beside_it_did_not() {
    use std::os::unix::fs::symlink;
    let (_tmp, plane, outside) = plane();
    folder(&plane.join("skills.local/notes"));
    std::fs::write(outside.join("notes.toml"), "grant = \"all\"\n").unwrap();
    symlink(
        outside.join("notes.toml"),
        plane.join("skills.local/notes.toml"),
    )
    .unwrap();
    let back_to = plane.join("skills.local/notes");
    let _hook = hook::set(move |moment, _, to| {
        if moment == hook::Moment::BeforeRename && to == back_to {
            folder(to);
        }
    });

    let refused = move_skill(
        &plane,
        &plane.join("skills.local/notes"),
        &plane,
        &plane.join("skills/notes"),
        1,
    )
    .unwrap_err();

    assert_eq!(
        refused,
        Unmoved::Stranded(
            "its folder was moved to skills/notes and could not be put back (something of its \
             name is already where it goes), and skills.local/notes.toml was not moved (it is \
             not a plain folder or file, so purlis does not move it)"
                .to_owned()
        )
    );
    assert!(plane.join("skills/notes/SKILL.md").is_file());
}

/// A skill moved between two held roots (the project and its state folder, as delete and
/// restore move one), whose file beside it cannot move and whose way back is taken: said to be
/// where it went, each part named under its own root (SK-48).
#[cfg(unix)]
#[test]
fn a_move_between_two_roots_pairs_the_file_beside_it_and_says_what_was_stranded() {
    use std::os::unix::fs::symlink;
    let (_tmp, plane, state) = plane();
    folder(&plane.join("skills/notes"));
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("notes.toml"), "grant = \"all\"\n").unwrap();
    symlink(
        elsewhere.path().join("notes.toml"),
        plane.join("skills/notes.toml"),
    )
    .unwrap();
    let back_to = plane.join("skills/notes");
    let _hook = hook::set(move |moment, _, to| {
        if moment == hook::Moment::BeforeRename && to == back_to {
            folder(to);
        }
    });

    let refused = move_skill(
        &plane,
        &plane.join("skills/notes"),
        &state,
        &state.join("deleted/1-notes/notes"),
        1,
    )
    .unwrap_err();

    assert_eq!(
        refused,
        Unmoved::Stranded(
            "its folder was moved to deleted/1-notes/notes and could not be put back (something \
             of its name is already where it goes), and skills/notes.toml was not moved (it is \
             not a plain folder or file, so purlis does not move it)"
                .to_owned()
        )
    );
    assert!(state.join("deleted/1-notes/notes/SKILL.md").is_file());
    assert!(plane.join("skills/notes.toml").is_symlink());
}

/// A skill and the file beside it moved between two held roots, both moved.
#[test]
fn a_skill_and_the_file_beside_it_move_between_two_roots() {
    let (_tmp, plane, state) = plane();
    folder(&plane.join("skills/notes"));
    std::fs::write(plane.join("skills/notes.toml"), "grant = \"all\"\n").unwrap();

    move_skill(
        &plane,
        &plane.join("skills/notes"),
        &state,
        &state.join("deleted/1-notes/notes"),
        1,
    )
    .unwrap();

    assert!(state.join("deleted/1-notes/notes/SKILL.md").is_file());
    assert!(state.join("deleted/1-notes/notes.toml").is_file());
    assert!(!plane.join("skills/notes").exists());
    assert!(!plane.join("skills/notes.toml").exists());
}

/// The entry taken away from where it went, between the rename and the look that proves it:
/// nothing is there to put back, and the move says so, not that something took its place.
#[cfg(unix)]
#[test]
fn a_leaf_gone_from_where_it_was_moved_is_said_to_be_gone() {
    let (_tmp, plane, _) = plane();
    folder(&plane.join("skills.local/notes"));
    let aside = plane.join("aside");
    let _hook = hook::set(move |moment, _, to| {
        if moment == hook::Moment::AfterRename {
            std::fs::rename(to, &aside).unwrap();
        }
    });

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
        Unmoved::Stranded(
            "it was gone from where it was moved, so nothing could be put back".to_owned()
        )
    );
}

/// A file with a second name outside the plane reaches past it as a link does: never moved, so
/// never committed, and a skill whose `.toml` is one is put back whole.
#[cfg(unix)]
#[test]
fn a_hard_linked_file_is_never_moved() {
    let (_tmp, plane, outside) = plane();
    folder(&plane.join("skills.local/notes"));
    std::fs::write(outside.join("notes.toml"), "grant = \"all\"\n").unwrap();
    std::fs::hard_link(
        outside.join("notes.toml"),
        plane.join("skills.local/notes.toml"),
    )
    .unwrap();

    let refused = move_between(
        &plane,
        &plane.join("skills.local/notes.toml"),
        &plane,
        &plane.join("skills/notes.toml"),
        1,
    )
    .unwrap_err();

    assert_eq!(
        refused,
        Unmoved::Nothing(
            "it is a file with another name too (a hard link), so purlis does not move it"
                .to_owned()
        )
    );
    assert!(
        move_skill(
            &plane,
            &plane.join("skills.local/notes"),
            &plane,
            &plane.join("skills/notes"),
            1,
        )
        .is_err()
    );
    assert!(plane.join("skills.local/notes/SKILL.md").is_file());
    assert!(plane.join("skills.local/notes.toml").is_file());
    assert!(!plane.join("skills/notes").exists());
    assert!(!plane.join("skills/notes.toml").exists());
}
