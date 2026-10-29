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
    assert_eq!(dir(root), None, "none until it is made");
    let made = made(root).unwrap();
    let dir = dir(root).expect("a fenced build always has one");
    assert_eq!(made, dir);
    let key = crate::plane::CloneKey::of(root);
    // A fenced build's, in the test's own project, beside the stores and never one of them.
    assert_eq!(
        dir,
        crate::names::state(root)
            .join(FENCED_DATA)
            .join(DIR)
            .join(key.as_str())
    );
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
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    let now = made_in(data.path(), &root).unwrap();
    // The folder set aside held levels records: every folder waits for Accept again.
    assert!(
        crate::skills::levels::made_since_in_for(&root, &now)(&root.join("workspaces/ops"))
            .is_some()
    );
    assert_eq!(now, was, "the same clone key");
    for name in ["skill-drafts", "skill-grade-after-edit.json"] {
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
    // The kept grade chats are carried, as a move carries them.
    assert!(into.join("skill-grade-chats/deploy/g1/c1.screen").is_file());
    // The rest: discarded, never carried forward.
    for gone in [
        "skill-tries",
        "grades-under-way",
        "grades-under-way.lock",
        "skill-grade-after-edit.json",
        "skill-grade-after-edit.lock",
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
    let data = data_home(root).unwrap();
    let dir = made_in(&data, root).unwrap();
    for folder in [&dir, &dir.parent().unwrap().to_path_buf()] {
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    made_in(&data, root).unwrap();
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
    // Moved: the old root is gone.
    std::fs::remove_dir_all(old.path()).unwrap();
    let root = new.path();
    let moved = follow_arrival_in(data.path(), root, &was, Arrived::Moved);
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
        follow_arrival_in(data.path(), new.path(), &was, Arrived::Moved),
        Moved::default()
    );
    // And a project that is not the old one's, though the record names its root.
    let was = crate::reopen::CloneSeat::of(old.path(), None);
    let stranger = project();
    assert_eq!(
        follow_arrival_in(data.path(), stranger.path(), &was, Arrived::Moved),
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

#[test]
fn a_copy_read_as_a_move_takes_nothing_from_the_original_and_gets_an_id_of_its_own() {
    let data = tempfile::tempdir().unwrap();
    let original = project();
    let was = moved_from(data.path(), original.path());
    // `cp -R`: the copy carries the original's state folder, its id among it.
    let copy = moved_to(original.path());
    let root = copy.path();
    made_in(data.path(), root).unwrap();
    let moved = follow_arrival_in(data.path(), root, &was, Arrived::Moved);
    assert!(moved.done.is_empty(), "{moved:?}");
    let theirs = in_data(data.path(), original.path());
    assert!(
        theirs.join("skill-drafts/abc/draft.json").is_file(),
        "the original keeps it"
    );
    assert!(theirs.join("skill-grade-after-edit.json").is_file());
    let here = in_data(data.path(), root);
    assert!(!here.join("skill-drafts").exists(), "the copy gets nothing");
    let (mine, its) = (
        record_text(&plane_id_file(root)),
        record_text(&plane_id_file(original.path())),
    );
    assert_ne!(mine, its, "their ids differ");
    assert_eq!(record_text(&here.join(OWNER)), mine);
    assert_eq!(
        record_text(&theirs.join(OWNER)),
        its,
        "the original is still its own"
    );
    // Its folders wait for the person's Accept, as a moved project's do.
    let held = crate::skills::levels::made_since_in_for(root, &here);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(held(&root.join(owner)).is_some(), "{owner} is not held");
    }
    // A copy V43 reads as one is given an id of its own too.
    let again = moved_to(original.path());
    made_in(data.path(), again.path()).unwrap();
    follow_arrival_in(data.path(), again.path(), &was, Arrived::Copied);
    assert_ne!(record_text(&plane_id_file(again.path())), its);
}

#[test]
fn a_move_cut_short_is_taken_up_again_until_it_is_whole() {
    let data = tempfile::tempdir().unwrap();
    let old = project();
    let was = moved_from(data.path(), old.path());
    let new = moved_to(old.path());
    std::fs::remove_dir_all(old.path()).unwrap();
    let root = new.path();
    // A draft of the same name with other bytes is in the way: the carry is not whole.
    let here = made_in(data.path(), root).unwrap();
    write(&here.join("skill-drafts/abc/draft.json"), "{\"other\":1}");
    let moved = follow_arrival_in(data.path(), root, &was, Arrived::Moved);
    assert!(!moved.left.is_empty());
    let old_folder = data.path().join(DIR).join(was.key.as_str());
    assert!(old_folder.exists(), "kept until whole");
    assert!(here.join(format!("{MOVING}{}", was.key)).is_file());
    // Once the way is clear, the carry is taken up again from its mark.
    std::fs::remove_dir_all(here.join("skill-drafts")).unwrap();
    let key = was.key.to_string();
    let taken = carried_from(
        data.path(),
        root,
        &key,
        &std::path::PathBuf::from(record_text(&here.join(format!("{MOVING}{key}"))).unwrap()),
    );
    assert!(taken.left.is_empty(), "{:?}", taken.left);
    assert!(here.join(format!("{MOVED}{key}")).is_file());
    assert!(!old_folder.exists());
    assert_eq!(
        std::fs::read_to_string(here.join("skill-grades/2026-10.jsonl")).unwrap(),
        "{\"row\":1}\n",
        "no row added twice"
    );
}

#[test]
fn two_at_once_make_one_folder_bound_to_one_id() {
    let data = tempfile::tempdir().unwrap();
    let tmp = project();
    let root = tmp.path().to_path_buf();
    let made: Vec<Result<PathBuf, String>> = std::thread::scope(|scope| {
        let calls: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| made_in(data.path(), &root)))
            .collect();
        calls.into_iter().map(|call| call.join().unwrap()).collect()
    });
    let here = in_data(data.path(), &root);
    for one in &made {
        assert_eq!(one.as_ref().unwrap(), &here);
    }
    assert_eq!(
        record_text(&here.join(OWNER)),
        record_text(&plane_id_file(&root))
    );
    let aside: Vec<_> = std::fs::read_dir(data.path().join(DIR))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".foreign-"))
        .collect();
    assert!(aside.is_empty(), "nothing set aside");
}

#[test]
fn levels_records_in_the_spelling_not_moved_are_asked_about_again_before_the_mark() {
    let tmp = project();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    std::fs::create_dir_all(root.join(".charter")).unwrap();
    let picked = crate::names::state(root);
    let other = [root.join(".purlis"), root.join(".charter")]
        .into_iter()
        .find(|state| *state != picked)
        .unwrap();
    write(
        &other.join("skill-levels/unaccepted.json"),
        "{\"levels\":[]}",
    );
    let moved = move_out_of_the_plane(root);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    assert!(dir(root).unwrap().join(MOVED_MARK).is_file());
    assert!(crate::skills::levels::made_since(root)(&root.join("workspaces/ops")).is_some());
    assert!(
        other.join("skill-levels/unaccepted.json").is_file(),
        "left as it is, unread"
    );
}

#[test]
fn a_reader_before_the_project_s_first_open_sees_nothing_of_another_project_s_state() {
    // The command line reads before the app has opened the project: a folder at its path that
    // another project left is never read, and the first write sets it aside.
    let tmp = project();
    let root = tmp.path();
    let theirs = in_data(&data_home(root).unwrap(), root);
    write(&theirs.join(OWNER), "0123456789abcdef0123456789abcdef");
    write(&theirs.join("skill-drafts/abc/draft.json"), "{}");
    assert_eq!(dir(root), None);
    assert!(crate::skills::verbs::drafts::list(root).is_empty());
    let mine = made(root).unwrap();
    assert_eq!(dir(root), Some(mine.clone()));
    assert!(!mine.join("skill-drafts").exists());
}

#[test]
fn a_move_from_a_root_that_is_only_out_of_reach_carries_and_keeps_the_old_folder() {
    let data = tempfile::tempdir().unwrap();
    let old = project();
    let mut was = moved_from(data.path(), old.path());
    // The old root is on a volume that is not mounted: neither it nor its folder above is there.
    let unreachable = std::path::PathBuf::from("/no-such-volume-3718/proj");
    let old_folder = data.path().join(DIR).join(was.key.as_str());
    std::fs::write(old_folder.join(ROOT_FILE), unreachable.to_str().unwrap()).unwrap();
    was.root = unreachable;
    let new = moved_to(old.path());
    std::fs::remove_dir_all(old.path()).unwrap();
    let root = new.path();
    let moved = follow_arrival_in(data.path(), root, &was, Arrived::Moved);
    assert!(moved.left.is_empty(), "{:?}", moved.left);
    let here = in_data(data.path(), root);
    assert!(
        here.join("skill-drafts/abc/draft.json").is_file(),
        "carried"
    );
    assert!(
        old_folder.join("skill-drafts/abc/draft.json").is_file(),
        "never removed"
    );
    assert!(here.join(format!("{MOVED}{}", was.key)).is_file());
}

#[test]
fn another_data_home_holds_every_folder_until_they_are_accepted_again() {
    // The project's state is kept under a data home it was not kept in before.
    let tmp = project();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    let first = tempfile::tempdir().unwrap();
    made_in(first.path(), root).unwrap();
    let other = tempfile::tempdir().unwrap();
    let here = made_in(other.path(), root).unwrap();
    assert!(!levels_confirmed(&here, root));
    assert_eq!(
        crate::skills::levels::made_since_in_for(root, &here)(&root.join("workspaces/ops"))
            .as_deref(),
        Some(crate::skills::levels::UNCONFIRMED)
    );
}

#[cfg(unix)]
#[test]
fn a_records_folder_that_is_a_link_is_never_confirmed() {
    let tmp = project();
    let root = tmp.path();
    let here = made(root).unwrap();
    mark_levels_confirmed(&here, root).unwrap();
    assert!(levels_confirmed(&here, root));
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::copy(
        here.join(crate::skills::levels::DIR).join(CONFIRMED),
        elsewhere.path().join(CONFIRMED),
    )
    .unwrap();
    std::fs::remove_dir_all(here.join(crate::skills::levels::DIR)).unwrap();
    forgotten(&here);
    std::os::unix::fs::symlink(elsewhere.path(), here.join(crate::skills::levels::DIR)).unwrap();
    assert!(!levels_confirmed(&here, root));
}

#[test]
fn a_project_with_a_state_folder_is_held_on_its_first_open_whatever_it_holds() {
    // A project chats ran in, whose levels records were taken away: its record of open chats is
    // there, and no levels records.
    let tmp = project();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    crate::reopen::write(root, &crate::reopen::Record::default()).unwrap();
    assert!(
        !crate::names::state(root)
            .join(crate::skills::levels::DIR)
            .exists()
    );
    move_out_of_the_plane(root);
    assert!(!crate::skills::levels::confirmed(root));
    assert_eq!(
        crate::skills::levels::made_since(root)(&root.join("workspaces/ops")).as_deref(),
        Some(crate::skills::levels::UNCONFIRMED)
    );
}

#[test]
fn a_project_replaced_at_the_same_path_is_never_answered_from_the_cache() {
    let data = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let mine = made_in(data.path(), root).unwrap();
    assert!(bound(&mine, root));
    assert!(levels_confirmed(&mine, root), "a project new to purlis");
    // Another project now stands at the path, with an id of its own.
    let other = "fedcba9876543210fedcba9876543210";
    crate::plane::write_private(root, &plane_id_file(root), other.as_bytes()).unwrap();
    assert!(!bound(&mine, root));
    assert!(!levels_confirmed(&mine, root));
}
