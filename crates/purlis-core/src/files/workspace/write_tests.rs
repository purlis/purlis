// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5551: a write holds the folders it checked through to the write, so a link a chat swaps in
//! between the two sends nothing elsewhere; and import's and rename's edge cases.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::super::NotWritten;
use super::super::tests::{files, project};
use super::{AFTER_THE_CHECK, Trash};

/// A Trash of the test's own, which only notes what it is handed.
#[derive(Default)]
struct Bin {
    took: RefCell<Vec<PathBuf>>,
}

impl Trash for Bin {
    fn put(&self, path: &Path) -> Result<(), String> {
        self.took.borrow_mut().push(path.to_path_buf());
        Ok(())
    }
}

/// The workspace `alpha` of [`project`], with a `plan.md` in repo `api` too.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let (dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::write(ws.join("repos/api/plan.md"), "the repo's plan\n").unwrap();
    (dir, plane)
}

/// What repo `api` holds, by name.
fn in_the_repo(ws: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(ws.join("repos/api"))
        .unwrap()
        .map(|one| one.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Between the check and the write, `notes/` is moved to `notes.held/` and a link to repo `api`
/// put in its place: what a chat in the workspace's folder can do.
#[cfg(unix)]
fn notes_swapped_for_a_link_after_the_check(ws: &Path) {
    let ws = ws.to_path_buf();
    AFTER_THE_CHECK.with(|fault| {
        *fault.borrow_mut() = Some(Box::new(move || {
            std::fs::rename(ws.join("notes"), ws.join("notes.held")).unwrap();
            std::os::unix::fs::symlink(ws.join("repos/api"), ws.join("notes")).unwrap();
        }));
    });
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_after_the_check_gets_nothing_new_through_it() {
    crate::unsteered!();
    for write in ["new file", "new folder", "import"] {
        let (_dir, plane) = workspace();
        let ws = plane.join("workspaces/alpha");
        let files = files(&plane);
        let away = tempfile::tempdir().unwrap();
        let picked = away.path().join("new.md");
        std::fs::write(&picked, "picked\n").unwrap();
        let before = in_the_repo(&ws);

        notes_swapped_for_a_link_after_the_check(&ws);
        let answer = match write {
            "new file" => files.new_file("notes", "new.md", &[]).map(|_| ()),
            "new folder" => files.new_folder("notes", "new.md", &[]).map(|_| ()),
            _ => files.import("notes", &[picked], &[]).map(|_| ()),
        };

        assert!(answer.is_ok(), "{write}: {answer:?}");
        assert_eq!(in_the_repo(&ws), before, "{write} went through the link");
        assert!(
            ws.join("notes.held/new.md").exists(),
            "{write} is made in the very folder that was checked"
        );
    }
}

#[cfg(unix)]
#[test]
fn an_entry_whose_folder_is_swapped_for_a_link_after_the_check_is_renamed_where_it_was() {
    crate::unsteered!();
    let (_dir, plane) = workspace();
    let ws = plane.join("workspaces/alpha");
    let files = files(&plane);

    notes_swapped_for_a_link_after_the_check(&ws);
    files.rename("notes/plan.md", "renamed.md", &[]).unwrap();

    assert!(ws.join("repos/api/plan.md").is_file(), "the repo's is kept");
    assert!(!ws.join("repos/api/renamed.md").exists());
    assert_eq!(
        std::fs::read_to_string(ws.join("notes.held/renamed.md")).unwrap(),
        "the plan\n"
    );
}

#[cfg(unix)]
#[test]
fn a_delete_whose_folder_is_swapped_for_a_link_after_the_check_is_refused() {
    crate::unsteered!();
    let (_dir, plane) = workspace();
    let ws = plane.join("workspaces/alpha");
    let files = files(&plane);
    let bin = Bin::default();

    notes_swapped_for_a_link_after_the_check(&ws);
    let refused = files.trash("notes/plan.md", &[], &bin).unwrap_err();

    assert!(matches!(refused, NotWritten::Moved(_)), "{refused}");
    assert!(bin.took.borrow().is_empty(), "{:?}", bin.took.borrow());
    assert!(ws.join("repos/api/plan.md").is_file());
    assert!(ws.join("notes.held/plan.md").is_file());
}

#[cfg(unix)]
#[test]
fn a_rename_onto_another_hard_link_of_the_same_file_is_refused() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    std::fs::hard_link(ws.join("notes/plan.md"), ws.join("notes/also.md")).unwrap();

    let refused = files(&plane)
        .rename("notes/plan.md", "also.md", &[])
        .unwrap_err();

    assert!(matches!(refused, NotWritten::Taken(_)), "{refused}");
    assert!(ws.join("notes/plan.md").is_file());
    assert!(ws.join("notes/also.md").is_file());
}

#[cfg(unix)]
#[test]
fn an_import_of_a_fifo_is_refused_without_waiting_for_a_writer() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let away = tempfile::tempdir().unwrap();
    let fifo = away.path().join("pipe");
    let made = crate::forklock::status(std::process::Command::new("mkfifo").arg(&fifo)).unwrap();
    assert!(made.success());

    let (said, answer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let refused = files(&plane).import("notes", &[fifo], &[]).unwrap_err();
        let _ = said.send(refused.to_string());
    });
    let refused = answer
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the import waited on the FIFO");

    assert!(refused.contains("is not one"), "{refused}");
}

#[test]
fn an_import_that_stops_partway_says_which_files_were_copied() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    let away = tempfile::tempdir().unwrap();
    let first = away.path().join("first.md");
    let last = away.path().join("last.md");
    std::fs::write(&first, "first\n").unwrap();
    std::fs::write(&last, "last\n").unwrap();
    let folder = away.path().join("folder");
    std::fs::create_dir(&folder).unwrap();

    let refused = files(&plane)
        .import("notes", &[first, folder, last], &[])
        .unwrap_err();

    match &refused {
        NotWritten::PartlyImported { copied, why } => {
            assert_eq!(copied, &["notes/first.md"]);
            assert!(matches!(**why, NotWritten::NotAFileToImport(_)), "{why}");
        }
        other => panic!("{other}"),
    }
    assert!(
        refused.to_string().contains("'notes/first.md'"),
        "{refused}"
    );
    assert!(ws.join("notes/first.md").is_file());
    assert!(!ws.join("notes/last.md").exists());
}

#[cfg(unix)]
#[test]
fn a_case_only_rename_is_not_stopped_by_a_file_left_from_another() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");
    let left = ws.join(format!("notes/.purlis-renaming-{}", std::process::id()));
    std::fs::write(&left, "left from another rename\n").unwrap();

    let renamed = files(&plane)
        .rename("notes/plan.md", "Plan.md", &[])
        .unwrap();

    assert_eq!(renamed, "notes/Plan.md");
    assert_eq!(
        std::fs::read_to_string(&left).unwrap(),
        "left from another rename\n"
    );
    let names: Vec<String> = std::fs::read_dir(ws.join("notes"))
        .unwrap()
        .map(|one| one.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(names.contains(&"Plan.md".to_string()), "{names:?}");
    assert!(!names.contains(&"plan.md".to_string()), "{names:?}");
}

#[cfg(unix)]
#[test]
fn a_case_only_renames_spare_name_is_the_persons_and_new_each_time() {
    let one = super::spare_name();
    let other = super::spare_name();

    assert_ne!(one, other);
    for spare in [&one, &other] {
        assert!(!super::managed(spare, &[]), "{spare} is purlis's");
        assert!(super::checked_name(spare).is_ok(), "{spare}");
    }
}

#[cfg(unix)]
#[test]
fn a_rename_on_a_disk_with_no_no_replace_rename_is_refused() {
    crate::unsteered!();
    let (_dir, plane) = project();
    let ws = plane.join("workspaces/alpha");

    crate::held::without_no_replace_rename(|| {
        let refused = files(&plane)
            .rename("notes/plan.md", "renamed.md", &[])
            .unwrap_err();
        assert!(matches!(refused, NotWritten::Failed { .. }), "{refused}");
    });

    assert!(ws.join("notes/plan.md").is_file());
    assert!(!ws.join("notes/renamed.md").exists());
}

/// A folder of the test's own with `mode`, and the test's own uid.
#[cfg(unix)]
fn folder_with(mode: u32) -> (tempfile::TempDir, u32) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    (dir, rustix::process::getuid().as_raw())
}

#[cfg(unix)]
#[test]
fn a_volume_whose_own_trash_can_be_made_takes_a_delete_on_its_own_disk() {
    let (top, uid) = folder_with(0o755);
    assert!(super::volume_trash_usable(top.path(), uid));
}

#[cfg(unix)]
#[test]
fn a_volume_with_a_writable_trash_of_the_users_own_takes_a_delete() {
    let (top, uid) = folder_with(0o755);
    std::fs::create_dir(top.path().join(format!(".Trash-{uid}"))).unwrap();
    assert!(super::volume_trash_usable(top.path(), uid));
}

#[cfg(unix)]
#[test]
fn a_volume_whose_top_cannot_be_written_and_has_no_trash_does_not() {
    if rustix::process::geteuid().is_root() {
        return; // root writes anywhere, so there is no such volume to make
    }
    let (top, uid) = folder_with(0o755);
    let path = top.path().to_path_buf();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o555)).unwrap();
    let usable = super::volume_trash_usable(&path, uid);
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert!(!usable);
}

#[cfg(unix)]
#[test]
fn a_volume_whose_shared_trash_has_the_users_folder_takes_a_delete_when_the_top_is_read_only() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (top, uid) = folder_with(0o755);
    let shared = top.path().join(".Trash");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o1777)).unwrap();
    std::fs::create_dir(shared.join(uid.to_string())).unwrap();
    std::fs::set_permissions(top.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let usable = super::volume_trash_usable(top.path(), uid);
    std::fs::set_permissions(top.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(usable);
}

#[cfg(unix)]
#[test]
fn a_volume_whose_trash_of_the_users_own_cannot_be_written_does_not() {
    use std::os::unix::fs::PermissionsExt;
    if rustix::process::geteuid().is_root() {
        return;
    }
    let (top, uid) = folder_with(0o755);
    let own = top.path().join(format!(".Trash-{uid}"));
    std::fs::create_dir(&own).unwrap();
    std::fs::set_permissions(&own, std::fs::Permissions::from_mode(0o555)).unwrap();

    let usable = super::volume_trash_usable(top.path(), uid);
    std::fs::set_permissions(&own, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!usable);
}

#[test]
fn the_top_of_a_path_is_the_longest_mount_point_it_is_in_as_the_trash_finds_it() {
    let mounts = "/dev/sda1 / ext4 rw 0 0\n\
                  /dev/sda2 /home ext4 rw 0 0\n\
                  /dev/sdb1 /mnt/my\\040disk vfat rw 0 0\n";
    let top = |path: &str| super::volume_top(Path::new(path), mounts);

    assert_eq!(
        top("/mnt/my disk/ws/notes"),
        Some(PathBuf::from("/mnt/my disk"))
    );
    assert_eq!(top("/home/me/ws"), Some(PathBuf::from("/home")));
    assert_eq!(top("/homer/ws"), Some(PathBuf::from("/")));
    assert_eq!(top("/mnt/my diskette/ws"), Some(PathBuf::from("/")));
    assert_eq!(super::volume_top(Path::new("/home/me"), ""), None);
}
