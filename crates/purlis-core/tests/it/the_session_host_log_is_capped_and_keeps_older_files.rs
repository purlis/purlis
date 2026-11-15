// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `session-host.log` is capped by size and keeps a few older files (#6687): what launchd
//! appends to for a session host that is a login item, and may run for days (FD-17).

use std::io::Write as _;
use std::path::Path;

use purlis_core::hostlog::{self, Capped};

fn write(path: &Path, bytes: usize, fill: u8) {
    std::fs::write(path, vec![fill; bytes]).unwrap();
}

fn len(path: &Path) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

#[test]
fn a_log_past_its_cap_is_moved_aside_whole_and_the_log_starts_again_empty() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    write(&log, 2_000, b'a');

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Rotated);

    assert_eq!(len(&log), 0, "the log starts again");
    assert_eq!(
        std::fs::read(hostlog::older(&log, 1)).unwrap(),
        vec![b'a'; 2_000]
    );
}

#[test]
fn a_log_within_its_cap_is_left_as_it_is() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    write(&log, 1_000, b'a');

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Within);

    assert_eq!(len(&log), 1_000);
    assert!(!hostlog::older(&log, 1).exists());
}

#[test]
fn no_log_yet_is_nothing_to_cap() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Within);
    assert!(!log.exists());
}

#[test]
fn each_rotation_shifts_the_older_files_and_only_the_newest_few_are_kept() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    for fill in *b"abcde" {
        write(&log, 2_000, fill);
        assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Rotated);
    }

    let kept: Vec<u8> = (1..=3)
        .map(|n| std::fs::read(hostlog::older(&log, n)).unwrap()[0])
        .collect();
    assert_eq!(kept, b"edc", "newest first");
    assert!(!hostlog::older(&log, 4).exists(), "a fourth is never kept");
}

/// launchd opens `StandardErrorPath` to append, so the host's standard error goes on into the
/// same file after it is emptied, from its start: never into the file moved aside, and never
/// past a hole the size of what was there.
#[test]
fn a_writer_holding_the_log_open_to_append_writes_on_into_the_emptied_log() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    let mut held = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .unwrap();
    held.write_all(&[b'a'; 2_000]).unwrap();

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Rotated);
    held.write_all(b"after\n").unwrap();

    assert_eq!(std::fs::read(&log).unwrap(), b"after\n");
    assert_eq!(len(&hostlog::older(&log, 1)), 2_000);
}

#[cfg(unix)]
#[test]
fn a_link_in_the_logs_place_is_refused_and_what_it_points_to_is_left_alone() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    write(&elsewhere, 2_000, b'a');
    let log = dir.path().join(hostlog::NAME);
    std::os::unix::fs::symlink(&elsewhere, &log).unwrap();

    assert!(hostlog::cap_at(&log, 1_000, 3).is_err());
    assert_eq!(len(&elsewhere), 2_000);
    assert!(!hostlog::older(&log, 1).exists());
}

/// A disk that is full fails the copy (ENOSPC), and the host tries again each minute: a copy
/// that fails leaves every older file where it was and the log whole, so trying again loses
/// nothing. A log the host may not read stands in for the full disk.
#[cfg(unix)]
#[test]
fn a_copy_that_fails_leaves_the_older_files_and_the_log_as_they_were() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    for (n, fill) in (1..=3).zip(*b"bcd") {
        write(&hostlog::older(&log, n), 100, fill);
    }
    write(&log, 2_000, b'a');
    let mode = |m| std::fs::set_permissions(&log, std::fs::Permissions::from_mode(m)).unwrap();
    mode(0o200);

    for _ in 0..3 {
        assert!(hostlog::cap_at(&log, 1_000, 3).is_err());
    }

    mode(0o600);
    let kept: Vec<Vec<u8>> = (1..=3)
        .map(|n| std::fs::read(hostlog::older(&log, n)).unwrap_or_default())
        .collect();
    assert_eq!(kept, [vec![b'b'; 100], vec![b'c'; 100], vec![b'd'; 100]]);
    assert_eq!(
        std::fs::read(&log).unwrap(),
        vec![b'a'; 2_000],
        "the log is not emptied"
    );
}

/// The acceptance (#6687): a host running with its log past the cap rotates it, keeping the
/// older files, without a restart.
#[test]
fn a_running_host_rotates_its_log_once_it_passes_the_cap_and_keeps_older_files() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    let mut held = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .unwrap();
    hostlog::keep_capped_at(log.clone(), 1_000, 3, std::time::Duration::from_millis(10));

    for fill in *b"abcd" {
        held.write_all(&[fill; 2_000]).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while len(&log) != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "the log was not rotated while the host ran"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    let kept: Vec<u8> = (1..=3)
        .map(|n| std::fs::read(hostlog::older(&log, n)).unwrap()[0])
        .collect();
    assert_eq!(kept, b"dcb", "newest first, three kept");
    assert!(!hostlog::older(&log, 4).exists());
}
