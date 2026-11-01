// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::*;

/// A stand-in for git: `script` run by `sh`, its output piped, leading a group as git does.
fn stand_in(script: &str) -> Child {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    git::lead(&mut command);
    crate::forklock::spawn(&mut command).unwrap()
}

fn at(stage: Stage, percent: u8) -> Progress {
    Progress {
        stage,
        percent: Some(percent),
    }
}

#[test]
fn each_stage_has_its_place_on_one_scale() {
    let mut climb = Climb::default();
    let seen: Vec<_> = [
        at(Stage::Connecting, 100),
        at(Stage::Downloading, 0),
        at(Stage::Downloading, 40),
        at(Stage::Downloading, 100),
        at(Stage::Unpacking, 50),
        at(Stage::WritingFiles, 50),
        at(Stage::WritingFiles, 100),
    ]
    .into_iter()
    .filter_map(|now| climb.heard(now))
    .collect();
    assert_eq!(
        seen,
        [
            (Stage::Connecting, 5),
            (Stage::Downloading, 5),
            (Stage::Downloading, 35),
            (Stage::Downloading, 80),
            (Stage::Unpacking, 85),
            (Stage::WritingFiles, 92),
            (Stage::WritingFiles, 95),
        ]
    );
}

#[test]
fn a_stage_that_starts_again_at_nought_never_moves_the_percent_back() {
    // What git prints: three sub-stages of connecting, each 0 to 100, and a second try's start.
    let lines = [
        "remote: Enumerating objects: 100% (3/3), done.",
        "remote: Counting objects:   0% (0/3)",
        "remote: Counting objects: 100% (3/3), done.",
        "remote: Compressing objects:  50% (1/2)",
        "Receiving objects:  60% (6/10)",
        "remote: Counting objects:  10% (1/10)",
        "Receiving objects:  10% (1/10)",
        "Receiving objects:  80% (8/10)",
    ];
    let mut climb = Climb::default();
    let mut shown = vec![0];
    for line in lines {
        if let Some((_, percent)) = read_progress(line).and_then(|now| climb.heard(now)) {
            shown.push(percent);
        }
    }
    assert!(shown.is_sorted(), "{shown:?}");
    assert_eq!(climb.now(), (Stage::Downloading, 65));
}

#[test]
fn a_line_that_says_no_share_shows_the_stage_it_reached_and_nothing_more() {
    let mut climb = Climb::default();
    let said = climb.heard(Progress {
        stage: Stage::Downloading,
        percent: None,
    });
    assert_eq!(said, Some((Stage::Downloading, 5)));
}

#[test]
fn what_a_clone_cannot_be_cured_of_is_not_tried_again() {
    let words = |line: &str| vec![line.to_owned()];
    for transient in [
        "fatal: early EOF",
        "error: RPC failed; curl 92 HTTP/2 stream 5 was not closed cleanly: CANCEL (err 8)",
        "fatal: the remote end hung up unexpectedly",
    ] {
        assert!(is_transient(&words(transient)), "{transient}");
    }
    for lasting in [
        "fatal: Authentication failed for 'https://forge.example/acme/widget.git/'",
        "fatal: repository 'https://forge.example/acme/widget.git/' not found",
        "fatal: unable to access 'x': Could not resolve host: forge.example",
        "error: RPC failed; curl 28 Operation too slow. Less than 1 bytes/sec transferred",
        "fatal: destination path 'widget' already exists",
    ] {
        assert!(!is_transient(&words(lasting)), "{lasting}");
    }
}

#[test]
fn curl_ending_a_dead_transfer_reads_as_a_stall() {
    assert_eq!(
        read_words(&[
            "error: RPC failed; curl 28 Operation too slow. Less than 1 bytes/sec".to_owned()
        ]),
        Reading::Stalled
    );
}

#[test]
fn a_clone_that_keeps_making_progress_outlasts_any_stall_limit() {
    // Well past the limit in all, with no silence as long as it. A clone that outlasts git's 120
    // seconds is this, at a scale a test can wait for.
    let child = stand_in(
        "i=0; while [ $i -lt 8 ]; do echo \"Receiving objects:  $i% ($i/100)\" >&2; \
         sleep 0.1; i=$((i+1)); done",
    );
    let mut heard = Vec::new();
    let started = Instant::now();
    let followed = follow(
        child,
        &AtomicBool::new(false),
        Some(Duration::from_millis(500)),
        &mut |now| heard.push(now.percent),
    )
    .unwrap();
    assert!(
        matches!(followed, Followed::Ended { ok: true, .. }),
        "{followed:?}"
    );
    assert_eq!(heard.len(), 8, "{heard:?}");
    assert!(started.elapsed() >= Duration::from_millis(700));
}

#[test]
fn a_clone_that_says_nothing_is_stopped_at_the_stall_limit_with_what_it_said() {
    let child = stand_in("echo 'warning: a word first' >&2; sleep 30");
    let started = Instant::now();
    let followed = follow(
        child,
        &AtomicBool::new(false),
        Some(Duration::from_millis(300)),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(
        followed,
        Followed::Stalled {
            said: vec!["warning: a word first".to_owned()]
        }
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn with_no_stall_limit_only_the_person_ends_it() {
    let child = stand_in("sleep 30");
    let off = std::sync::Arc::new(AtomicBool::new(false));
    let press = {
        let off = off.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            off.store(true, Ordering::SeqCst);
        })
    };
    assert_eq!(
        follow(child, &off, None, &mut |_| {}).unwrap(),
        Followed::CalledOff
    );
    press.join().unwrap();
}

#[test]
fn a_git_that_fails_keeps_its_last_words_and_not_its_progress() {
    let child = stand_in(
        "echo 'Receiving objects:  10% (1/10)' >&2; echo 'fatal: early EOF' >&2; exit 128",
    );
    assert_eq!(
        follow(child, &AtomicBool::new(false), None, &mut |_| {}).unwrap(),
        Followed::Ended {
            ok: false,
            said: vec!["fatal: early EOF".to_owned()]
        }
    );
}

#[test]
fn git_lfs_fetching_large_files_is_the_last_stage_and_never_moves_the_percent_back() {
    // #4102: what `git lfs pull` draws, after git's own checkout reached its end.
    let lines = [
        "Updating files: 100% (3/3), done.",
        "Downloading LFS objects:   0% (0/2), 120 KB | 1 MB/s",
        "Downloading LFS objects:  50% (1/2), 4.0 MB | 1 MB/s",
        "Downloading LFS objects: 100% (2/2), 8.0 MB | 1 MB/s, done.",
    ];
    let mut climb = Climb::default();
    let mut shown = vec![0];
    for line in lines {
        if let Some((_, percent)) = read_progress(line).and_then(|now| climb.heard(now)) {
            shown.push(percent);
        }
    }
    assert!(shown.is_sorted(), "{shown:?}");
    assert_eq!(shown, [0, 95, 95, 97, 100]);
    assert_eq!(climb.now(), (Stage::LargeFiles, 100));
    assert_eq!(Stage::LargeFiles.words(), "Downloading large files");
}

#[test]
fn what_a_program_says_on_stdout_is_life_and_progress_as_on_stderr() {
    // `git lfs pull` draws its meter on stdout: a healthy download reads as silent otherwise.
    let child = {
        let mut command = std::process::Command::new("/bin/sh");
        command
            .args([
                "-c",
                "for i in 1 2 3 4 5 6; do \
                 printf 'Downloading LFS objects:   0%% (0/1), %d KB | 1 MB/s\\r' $i; \
                 sleep 0.2; done",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        git::lead(&mut command);
        crate::forklock::spawn(&mut command).unwrap()
    };
    let mut heard = Vec::new();
    let followed = follow(
        child,
        &AtomicBool::new(false),
        Some(Duration::from_millis(700)),
        &mut |now| heard.push(now.stage),
    )
    .unwrap();
    assert!(
        matches!(followed, Followed::Ended { ok: true, .. }),
        "{followed:?}"
    );
    assert_eq!(heard, [Stage::LargeFiles]);
}
