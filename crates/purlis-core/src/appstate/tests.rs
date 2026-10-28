// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for the app's state of a project in the data home (#3718): its folder, and the one
//! move of an older purlis's copies out of the plane's state folder.

use super::*;

/// A project, with a state folder of its own under the purlis name.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(tmp.path().join(".purlis")).unwrap();
    tmp
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn a_projects_app_state_is_its_clone_key_under_the_data_homes_app_state() {
    let tmp = project();
    let root = tmp.path();
    let dir = dir(root).expect("a fenced build always has one");
    let key = crate::plane::CloneKey::of(root);
    assert!(
        dir.ends_with(Path::new(DIR).join(key.as_str())),
        "{}",
        dir.display()
    );
    // Made private, as a folder of its own, inside the fence and never in the plane.
    assert_eq!(made(root).unwrap(), dir);
    assert!(crate::fence::inside(&dir, &crate::fence::fence()));
    assert!(!dir.starts_with(root));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        for folder in [&dir, &dir.parent().unwrap().to_path_buf()] {
            let mode = std::fs::metadata(folder).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
        }
    }
    // Another clone has another folder.
    let other = project();
    assert_ne!(super::dir(other.path()), Some(dir));
}

#[test]
fn the_data_home_s_app_state_folder_is_the_one_its_name_says() {
    let data = Path::new("/data");
    let root = Path::new("/no/such/project");
    assert_eq!(
        in_data(data, root),
        data.join("app-state")
            .join(crate::plane::CloneKey::of(root).as_str())
    );
}

#[test]
fn the_move_carries_drafts_and_local_grades_and_discards_the_rest() {
    let tmp = project();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    let state = root.join(".purlis");
    write(&state.join("skill-drafts/abc/draft.json"), "{\"draft\":1}");
    write(&state.join("skill-drafts/abc/files/SKILL.md"), "drafted");
    write(
        &state.join("skill-grades/2026-10.jsonl"),
        "{\"row\":\"old\"}\n",
    );
    write(&state.join("skill-tries/t1/piece"), "/somewhere");
    write(&state.join("grades-under-way/g1.json"), "{}");
    write(&state.join("grades-under-way.lock"), "");
    write(&state.join("skill-grade-after-edit.json"), "{\"v\":1}");
    write(&state.join("skill-grade-after-edit.lock"), "");
    write(
        &state.join("skill-grade-chats/deploy/g1/c1.screen"),
        "screen",
    );
    write(
        &state.join("skill-levels/unaccepted.json"),
        "{\"levels\":[]}",
    );
    // What else is in the state folder is not the move's.
    write(&state.join("theme-drafts/keep.json"), "{}");
    // A grade this machine recorded already in the app's state is kept, the old rows added.
    let into = made(root).unwrap();
    write(
        &into.join("skill-grades/2026-10.jsonl"),
        "{\"row\":\"new\"}\n",
    );

    let moved = move_out_of_the_plane(root);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    for (name, _) in STORES {
        assert!(
            std::fs::symlink_metadata(state.join(name)).is_err(),
            "{name} is still in the plane"
        );
    }
    assert!(state.join("theme-drafts/keep.json").is_file());
    // Drafts and local grades: the person's own work, moved.
    assert_eq!(
        std::fs::read_to_string(into.join("skill-drafts/abc/files/SKILL.md")).unwrap(),
        "drafted"
    );
    assert_eq!(
        std::fs::read_to_string(into.join("skill-grades/2026-10.jsonl")).unwrap(),
        "{\"row\":\"new\"}\n{\"row\":\"old\"}\n"
    );
    // The rest: discarded, never carried forward.
    for gone in [
        "skill-tries",
        "grades-under-way",
        "grades-under-way.lock",
        "skill-grade-after-edit.json",
        "skill-grade-after-edit.lock",
        "skill-grade-chats",
    ] {
        assert!(
            std::fs::symlink_metadata(into.join(gone)).is_err(),
            "{gone} was carried"
        );
    }
    // The records of the levels: every workspace and persona folder is asked about again.
    let held = crate::skills::levels::made_since(root);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(held(&root.join(owner)).is_some(), "{owner} is not held");
    }
    assert_eq!(moved.done.len(), STORES.len());
}

#[test]
fn the_move_runs_once_and_never_reads_the_plane_again() {
    let tmp = project();
    let root = tmp.path();
    let state = root.join(".purlis");
    assert!(move_out_of_the_plane(root).done.is_empty());
    assert!(dir(root).unwrap().join(MOVED_MARK).is_file());
    // A copy an older purlis left after the move is neither moved nor read.
    write(&state.join("skill-drafts/abc/draft.json"), "{}");
    assert_eq!(move_out_of_the_plane(root), Moved::default());
    assert!(state.join("skill-drafts/abc/draft.json").is_file());
    assert!(!dir(root).unwrap().join("skill-drafts").exists());
}

#[cfg(unix)]
#[test]
fn a_store_that_is_a_link_is_removed_and_nothing_it_points_at_is_carried() {
    let tmp = project();
    let root = tmp.path();
    let elsewhere = tempfile::tempdir().unwrap();
    write(&elsewhere.path().join("abc/draft.json"), "{}");
    std::os::unix::fs::symlink(elsewhere.path(), root.join(".purlis/skill-drafts")).unwrap();
    let moved = move_out_of_the_plane(root);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    assert!(std::fs::symlink_metadata(root.join(".purlis/skill-drafts")).is_err());
    assert!(!dir(root).unwrap().join("skill-drafts").exists());
    assert!(
        elsewhere.path().join("abc/draft.json").is_file(),
        "never followed"
    );
}

#[test]
fn the_older_state_folder_s_copies_move_too() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    write(&root.join(".charter/skill-drafts/abc/draft.json"), "{}");
    write(&root.join(".charter/skill-grade-after-edit.json"), "{}");
    let moved = move_out_of_the_plane(root);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    assert!(
        dir(root)
            .unwrap()
            .join("skill-drafts/abc/draft.json")
            .is_file()
    );
    assert!(!root.join(".charter/skill-grade-after-edit.json").exists());
}
