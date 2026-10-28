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
    // A fenced build's, in the test's own project, beside the stores and never one of them.
    assert_eq!(
        dir,
        crate::names::state(root)
            .join(FENCED_DATA)
            .join(DIR)
            .join(key.as_str())
    );
    assert_eq!(made(root).unwrap(), dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        for folder in [&dir, &dir.parent().unwrap().to_path_buf()] {
            let mode = std::fs::metadata(folder).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
        }
    }
    // Bound to its project.
    assert_eq!(
        record_text(&dir.join(OWNER)),
        record_text(&plane_id_file(root))
    );
    // Another clone has another folder.
    let other = project();
    assert_ne!(super::dir(other.path()), Some(dir));
}

#[test]
fn a_new_project_at_the_same_path_starts_with_none_of_the_old_ones_state() {
    let data = tempfile::tempdir().unwrap();
    let tmp = project();
    let root = tmp.path().to_path_buf();
    let was = made_in(data.path(), &root).unwrap();
    write(&was.join("skill-drafts/abc/draft.json"), "{}");
    write(&was.join("skill-grade-after-edit.json"), "{\"v\":1}");
    write(&was.join("skill-levels/unaccepted.json"), "{\"levels\":[]}");
    // The project is removed, and another made at its path.
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::create_dir_all(root.join(".purlis")).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let now = made_in(data.path(), &root).unwrap();
    assert_eq!(now, was, "the same clone key");
    for name in [
        "skill-drafts",
        "skill-grade-after-edit.json",
        "skill-levels",
    ] {
        assert!(!now.join(name).exists(), "{name} was inherited");
    }
    let aside = now.with_file_name(format!(
        "{}.foreign-1",
        now.file_name().unwrap().to_string_lossy()
    ));
    assert!(
        aside.join("skill-drafts/abc/draft.json").is_file(),
        "set aside, unread"
    );
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

#[cfg(unix)]
#[test]
fn a_folder_already_there_is_made_private_and_a_link_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let tmp = project();
    let root = tmp.path();
    let dir = dir(root).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for folder in [&dir, &dir.parent().unwrap().to_path_buf()] {
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    made(root).unwrap();
    for folder in [&dir, &dir.parent().unwrap().to_path_buf()] {
        let mode = std::fs::metadata(folder).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
    }
    // The project's folder as a link to another is set aside, and nothing is written through it.
    let other = tempfile::tempdir().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(other.path(), &dir).unwrap();
    assert_eq!(made(root).unwrap(), dir);
    assert!(std::fs::symlink_metadata(&dir).unwrap().is_dir());
    assert!(std::fs::read_dir(other.path()).unwrap().next().is_none());
    // One that names this project, but is a link, is refused.
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(
        other.path().join(OWNER),
        record_text(&plane_id_file(root)).unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(other.path(), &dir).unwrap();
    assert!(made(root).is_err());
    assert!(!other.path().join(ROOT_FILE).exists());
    std::fs::remove_file(&dir).unwrap();
}

#[test]
fn carrying_again_after_a_removal_that_failed_adds_nothing_twice() {
    let from = tempfile::tempdir().unwrap();
    let to = tempfile::tempdir().unwrap();
    let into = to.path().join("skill-grades");
    write(
        &from.path().join("2026-10.jsonl"),
        "{\"row\":1}\n{\"row\":2}\n",
    );
    write(&into.join("2026-10.jsonl"), "{\"row\":0}\n");
    assert!(carry(from.path(), &into, true).is_empty());
    // The source was not removed: the next open carries it again.
    assert!(carry(from.path(), &into, true).is_empty());
    assert_eq!(
        std::fs::read_to_string(into.join("2026-10.jsonl")).unwrap(),
        "{\"row\":0}\n{\"row\":1}\n{\"row\":2}\n"
    );
    // No file is left half written beside its place.
    let names: Vec<String> = std::fs::read_dir(&into)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["2026-10.jsonl"]);
}

#[test]
fn a_store_not_carried_whole_stays_in_the_plane_and_is_said() {
    let tmp = project();
    let root = tmp.path();
    let into = made(root).unwrap();
    write(&into.join("skill-drafts/abc/draft.json"), "{\"theirs\":1}");
    let state = root.join(".purlis");
    write(&state.join("skill-drafts/abc/draft.json"), "{\"mine\":1}");
    write(&state.join("skill-drafts/def/draft.json"), "{}");
    let moved = move_out_of_the_plane(root);
    assert_eq!(moved.left.len(), 1, "{:?}", moved.left);
    assert!(state.join("skill-drafts/abc/draft.json").is_file(), "kept");
    assert_eq!(
        std::fs::read_to_string(into.join("skill-drafts/abc/draft.json")).unwrap(),
        "{\"theirs\":1}"
    );
    // Not marked as moved: the next open tries again.
    assert!(!into.join(MOVED_MARK).exists());
}

/// A project at `old` with app state of its own in `data`, as the clone it was: drafts, a grade
/// row, kept chats, an opt-in and levels records.
fn moved_from(data: &Path, old: &Path) -> crate::reopen::CloneSeat {
    let was = made_in(data, old).unwrap();
    write(&was.join("skill-drafts/abc/draft.json"), "{}");
    write(&was.join("skill-grades/2026-10.jsonl"), "{\"row\":1}\n");
    write(&was.join("skill-grade-chats/deploy/g/c.screen"), "screen");
    write(&was.join("skill-grade-after-edit.json"), "{\"v\":1}");
    write(&was.join("skill-levels/unaccepted.json"), "{\"levels\":[]}");
    crate::reopen::CloneSeat::of(old, None)
}

/// Where the project at `old` moved to: its state folder's `app/` with it, and a workspace and a
/// persona.
fn moved_to(old: &Path) -> tempfile::TempDir {
    let tmp = project();
    std::fs::create_dir_all(tmp.path().join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(tmp.path().join("personas/steward")).unwrap();
    let id = plane_id_file(old);
    let to = plane_id_file(tmp.path());
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::copy(id, to).unwrap();
    tmp
}

#[test]
fn a_moved_project_keeps_its_drafts_and_grades_and_is_asked_about_its_folders_again() {
    let data = tempfile::tempdir().unwrap();
    let old = project();
    let was = moved_from(data.path(), old.path());
    let new = moved_to(old.path());
    let root = new.path();
    let moved = follow_move_in(data.path(), root, &was);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    let here = in_data(data.path(), root);
    assert!(here.join("skill-drafts/abc/draft.json").is_file());
    assert_eq!(
        std::fs::read_to_string(here.join("skill-grades/2026-10.jsonl")).unwrap(),
        "{\"row\":1}\n"
    );
    assert!(here.join("skill-grade-chats/deploy/g/c.screen").is_file());
    // Never the opt-in, nor the levels records: every folder waits for the person's Accept.
    assert!(!here.join("skill-grade-after-edit.json").exists());
    let held = crate::skills::levels::made_since_in_for(root, &here);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(held(&root.join(owner)).is_some(), "{owner} is not held");
    }
    assert!(
        !in_data(data.path(), old.path()).exists(),
        "the old folder is removed"
    );
}

#[test]
fn a_move_whose_old_folder_names_another_project_carries_nothing() {
    let data = tempfile::tempdir().unwrap();
    let old = project();
    let mut was = moved_from(data.path(), old.path());
    // The record names a root the old key's folder does not say it is.
    let elsewhere = tempfile::tempdir().unwrap();
    was.root = elsewhere.path().canonicalize().unwrap();
    let new = moved_to(old.path());
    assert_eq!(
        follow_move_in(data.path(), new.path(), &was),
        Moved::default()
    );
    // And a project that is not the old one's, though the record names its root.
    let was = crate::reopen::CloneSeat::of(old.path(), None);
    let stranger = project();
    assert_eq!(
        follow_move_in(data.path(), stranger.path(), &was),
        Moved::default()
    );
    for root in [new.path(), stranger.path()] {
        let here = in_data(data.path(), root);
        for name in [
            "skill-drafts",
            "skill-grades",
            "skill-grade-chats",
            "skill-grade-after-edit.json",
            "skill-levels",
        ] {
            assert!(!here.join(name).exists(), "{name} was carried");
        }
    }
    assert!(
        in_data(data.path(), old.path()).exists(),
        "left where it was"
    );
}

#[test]
fn only_the_state_folder_purlis_reads_is_moved_and_the_other_is_left_unread() {
    let tmp = project();
    let root = tmp.path();
    std::fs::create_dir_all(root.join(".charter")).unwrap();
    let picked = crate::names::state(root);
    let other = [root.join(".purlis"), root.join(".charter")]
        .into_iter()
        .find(|state| *state != picked)
        .unwrap();
    write(&picked.join("skill-drafts/abc/draft.json"), "{}");
    write(&other.join("skill-drafts/def/draft.json"), "{}");
    let moved = move_out_of_the_plane(root);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    let here = dir(root).unwrap();
    assert!(here.join("skill-drafts/abc/draft.json").is_file());
    assert!(
        !here.join("skill-drafts/def").exists(),
        "the other is never read"
    );
    assert!(
        other.join("skill-drafts/def/draft.json").is_file(),
        "and left as it is"
    );
}
