// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A git directory, with a reflog and `packed-refs` that are plain files.
fn git_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let git = std::fs::canonicalize(dir.path()).unwrap().join(".git");
    std::fs::create_dir_all(git.join("logs/refs/remotes/origin")).unwrap();
    std::fs::write(git.join("logs/HEAD"), "").unwrap();
    std::fs::write(git.join("logs/refs/remotes/origin/main"), "").unwrap();
    std::fs::write(git.join("packed-refs"), "").unwrap();
    (dir, git)
}

#[test]
fn plain_ref_records_are_written() {
    let (_dir, git) = git_dir();
    assert_eq!(ref_records_linked(&git), None);
    assert_eq!(ref_records_refused(&git), Ok(()));
}

#[test]
fn a_reflog_packed_refs_or_logs_folder_that_links_elsewhere_is_refused() {
    for planted in ["logs/refs/remotes/origin/main", "packed-refs", "logs/refs"] {
        let (dir, git) = git_dir();
        let path = git.join(planted);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).unwrap();
        } else {
            std::fs::remove_file(&path).unwrap();
        }
        std::os::unix::fs::symlink(dir.path().join("target"), &path).unwrap();
        assert_eq!(ref_records_linked(&git), Some(path.clone()), "{planted}");
        let refused = ref_records_refused(&git).unwrap_err();
        assert!(refused.contains(&path.display().to_string()), "{refused}");
    }
}

#[test]
fn a_reflog_with_a_second_name_is_refused() {
    let (dir, git) = git_dir();
    let reflog = git.join("logs/refs/remotes/origin/main");
    std::fs::remove_file(&reflog).unwrap();
    let other = dir.path().join("other");
    std::fs::write(&other, "fixed\n").unwrap();
    std::fs::hard_link(&other, &reflog).unwrap();
    assert_eq!(ref_records_linked(&git), Some(reflog));
}

#[test]
fn a_branch_folders_shared_records_are_looked_at_too() {
    let (dir, common) = git_dir();
    let own = common.join("worktrees/p1");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("commondir"), "../..\n").unwrap();
    assert_eq!(ref_records_linked(&own), None);
    let packed = common.join("packed-refs");
    std::fs::remove_file(&packed).unwrap();
    std::os::unix::fs::symlink(dir.path().join("x"), &packed).unwrap();
    assert_eq!(
        ref_records_linked(&own),
        Some(own.join("../..").join("packed-refs"))
    );
}

#[test]
fn an_absolute_commondir_is_looked_at_where_git_reads_it() {
    let (dir, common) = git_dir();
    let own = dir.path().join("elsewhere/p1");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::write(own.join("commondir"), format!("{}\n", common.display())).unwrap();
    assert_eq!(ref_records_linked(&own), None);
    let packed = common.join("packed-refs");
    std::fs::remove_file(&packed).unwrap();
    std::os::unix::fs::symlink(dir.path().join("x"), &packed).unwrap();
    assert_eq!(ref_records_linked(&own), Some(packed));
}

#[test]
fn a_commondir_that_names_no_folder_or_is_a_link_is_refused() {
    let (dir, common) = git_dir();
    let own = common.join("worktrees/p1");
    std::fs::create_dir_all(&own).unwrap();
    let file = own.join("commondir");
    std::fs::write(&file, "../../missing\n").unwrap();
    assert_eq!(ref_records_linked(&own), Some(file.clone()));
    std::fs::remove_file(&file).unwrap();
    let said = dir.path().join("said");
    std::fs::write(&said, "../..\n").unwrap();
    std::os::unix::fs::symlink(&said, &file).unwrap();
    assert_eq!(ref_records_linked(&own), Some(file));
}

#[test]
fn a_logs_folder_that_cannot_be_listed_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, git) = git_dir();
    let logs = git.join("logs/refs");
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Run as a user the mode binds: one it does not (root) lists the folder anyway.
    let listable = std::fs::read_dir(&logs).is_ok();
    let found = ref_records_linked(&git);
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !listable {
        assert_eq!(found, Some(logs));
    }
}

#[test]
fn plain_git_files_are_written() {
    let (_dir, git) = git_dir();
    std::fs::write(git.join("COMMIT_EDITMSG"), "one\n").unwrap();
    std::fs::create_dir_all(git.join("refs/heads")).unwrap();
    std::fs::write(git.join("refs/heads/main"), "0\n").unwrap();
    assert_eq!(git_files_linked(&git), None);
    assert_eq!(git_files_refused(&git), Ok(()));
}

#[test]
fn a_message_pseudo_ref_or_state_folder_that_links_elsewhere_is_refused() {
    for planted in [
        "COMMIT_EDITMSG",
        "MERGE_MSG",
        "ORIG_HEAD",
        "FETCH_HEAD",
        "rebase-merge",
        "refs",
        "packed-refs",
    ] {
        let (dir, git) = git_dir();
        let path = git.join(planted);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).unwrap();
        } else if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
        std::os::unix::fs::symlink(dir.path().join("target"), &path).unwrap();
        assert_eq!(git_files_linked(&git), Some(path.clone()), "{planted}");
        let refused = git_files_refused(&git).unwrap_err();
        assert!(refused.contains(&path.display().to_string()), "{refused}");
    }
}

#[test]
fn a_commit_message_file_with_a_second_name_is_refused() {
    let (dir, git) = git_dir();
    let other = dir.path().join("other");
    std::fs::write(&other, "fixed\n").unwrap();
    std::fs::hard_link(&other, git.join("COMMIT_EDITMSG")).unwrap();
    assert_eq!(git_files_linked(&git), Some(git.join("COMMIT_EDITMSG")));
}

#[test]
fn a_link_inside_a_state_folder_is_refused() {
    let (dir, git) = git_dir();
    std::fs::create_dir_all(git.join("rebase-merge")).unwrap();
    let message = git.join("rebase-merge/message");
    std::os::unix::fs::symlink(dir.path().join("target"), &message).unwrap();
    assert_eq!(git_files_linked(&git), Some(message));
}
