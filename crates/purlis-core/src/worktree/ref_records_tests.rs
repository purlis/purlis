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
