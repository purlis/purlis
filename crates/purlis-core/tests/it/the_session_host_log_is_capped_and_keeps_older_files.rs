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
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    write(&log, 2_000, b'a');

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Rotated);

    assert_eq!(len(&log), 0, "the log starts again");
    assert_eq!(std::fs::read(hostlog::older(&log, 1)).unwrap(), vec![b'a'; 2_000]);
}

#[test]
fn a_log_within_its_cap_is_left_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    write(&log, 1_000, b'a');

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Within);

    assert_eq!(len(&log), 1_000);
    assert!(!hostlog::older(&log, 1).exists());
}

#[test]
fn no_log_yet_is_nothing_to_cap() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);

    assert_eq!(hostlog::cap_at(&log, 1_000, 3).unwrap(), Capped::Within);
    assert!(!log.exists());
}

#[test]
fn each_rotation_shifts_the_older_files_and_only_the_newest_few_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join(hostlog::NAME);
    for fill in [b'a', b'b', b'c', b'd', b'e'] {
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
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    write(&elsewhere, 2_000, b'a');
    let log = dir.path().join(hostlog::NAME);
    std::os::unix::fs::symlink(&elsewhere, &log).unwrap();

    assert!(hostlog::cap_at(&log, 1_000, 3).is_err());
    assert_eq!(len(&elsewhere), 2_000);
    assert!(!hostlog::older(&log, 1).exists());
}

#[test]
fn the_host_keeps_a_few_older_files_of_a_bounded_size() {
    assert_eq!(hostlog::KEPT, 3);
    assert_eq!(hostlog::CAP, 1024 * 1024);
}
