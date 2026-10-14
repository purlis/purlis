// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_group_is_read_from_the_processes_listed_and_one_that_cannot_be_listed_is_unknown() {
    let proc = tempfile::tempdir().expect("a directory");
    let record = |pid: &str, stat: &str| {
        std::fs::create_dir(proc.path().join(pid)).expect("an entry");
        std::fs::write(proc.path().join(pid).join("stat"), stat).expect("its record");
    };
    record("77", "77 (git) Z 1 77 77 0 -1 4194304");
    record("90", "90 (sh) S 1 12 12 0 -1 4194304");
    // An entry that is no process, and a process whose record is already gone.
    std::fs::create_dir(proc.path().join("self")).expect("an entry");
    std::fs::create_dir(proc.path().join("91")).expect("an entry");
    assert_eq!(
        group_runs_as_listed(proc.path(), 77),
        Some(false),
        "only its exited leader"
    );
    record("78", "78 (git merge) R 77 77 77 0 -1 4194304");
    assert_eq!(group_runs_as_listed(proc.path(), 77), Some(true));
    assert_eq!(group_runs_as_listed(proc.path(), 12), Some(true));
    assert_eq!(
        group_runs_as_listed(&proc.path().join("not-there"), 77),
        None,
        "nothing to read is not an answer"
    );
}

#[test]
fn a_wait_looks_at_once_then_less_and_less_often_and_ends_on_time() {
    let began = std::time::Instant::now();
    let mut looks = Vec::new();
    let done = wait_until(began + std::time::Duration::from_millis(400), || {
        looks.push(began.elapsed());
        false
    });
    assert!(!done);
    assert!(began.elapsed() >= std::time::Duration::from_millis(400));
    // 0, 10, 30, 70, 150, 310, and the last at the end: never the forty of a fixed ten.
    assert!((4..=9).contains(&looks.len()), "{looks:?}");
    assert!(looks[0] < std::time::Duration::from_millis(10), "{looks:?}");

    let mut asked = 0;
    assert!(wait_until(
        std::time::Instant::now() + std::time::Duration::from_secs(30),
        || {
            asked += 1;
            asked == 3
        }
    ));
    assert_eq!(asked, 3, "it stops asking once it is done");
}

#[test]
fn a_process_record_is_read_past_a_name_that_holds_spaces_and_parentheses() {
    let running = "812 (tmux: server (1)) S 1 77 77 0 -1 4194304";
    assert!(in_group_and_running(running, "77"));
    assert!(!in_group_and_running(running, "7"));
    assert!(
        !in_group_and_running(running, "1"),
        "the parent is not the group"
    );
    let exited = "77 (git) Z 1 77 77 0 -1 4194304";
    assert!(
        !in_group_and_running(exited, "77"),
        "an exited leader is not running"
    );
    assert!(!in_group_and_running("", "77"));
    assert!(!in_group_and_running("77 (git)", "77"));
}
