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

/// One row of a process table: `(pid, parent, group)`.
fn row(pid: u32, parent: u32, group: u32) -> Listed {
    Listed { pid, parent, group }
}

#[test]
fn what_a_program_started_in_a_group_of_its_own_is_a_stray_and_its_own_group_is_not() {
    // The harness, 100, leads its group. Its helper stays in that group; a shell it started
    // detached (Claude Code's background Bash) leads a session of its own, 200, and that shell's
    // `sleep` is in 200's group; a grandchild that left 200's too leads 300. A stranger, 400,
    // and its child, are nobody's.
    let table = [
        row(100, 1, 100),
        row(101, 100, 100),
        row(200, 100, 200),
        row(201, 200, 200),
        row(300, 201, 300),
        row(400, 1, 400),
        row(401, 400, 400),
    ];

    let strays = strays_in(&table, 100);

    assert_eq!(
        strays,
        [
            Stray {
                pid: 200,
                leads: true
            },
            Stray {
                pid: 201,
                leads: false
            },
            Stray {
                pid: 300,
                leads: true
            },
        ]
    );
}

#[test]
fn a_program_that_started_nothing_outside_its_group_has_no_strays() {
    let table = [row(100, 1, 100), row(101, 100, 100), row(102, 101, 100)];

    assert!(strays_in(&table, 100).is_empty());
    assert!(strays_in(&table, 999).is_empty(), "nor one not listed");
}

#[test]
fn a_table_that_says_a_process_is_its_own_ancestor_is_walked_once() {
    // A table read while processes come and go can say anything: a pid reused as its own
    // ancestor's parent must not loop.
    let table = [row(100, 1, 100), row(200, 100, 200), row(100, 200, 100)];

    assert_eq!(
        strays_in(&table, 100),
        [Stray {
            pid: 200,
            leads: true
        }]
    );
}

#[test]
fn a_ps_listing_and_a_proc_stat_line_read_as_rows() {
    assert_eq!(
        listed_by_ps("  100     1   100\n  200   100   200\nnot a row\n"),
        [row(100, 1, 100), row(200, 100, 200)]
    );
    assert_eq!(
        listed_in_stat(201, "201 (sleep (a) b) S 200 200 1 0 -1 4194304"),
        Some(row(201, 200, 200))
    );
    assert_eq!(
        listed_in_stat(77, "77 (git) Z 1 77 77 0"),
        None,
        "an exited one"
    );
}
