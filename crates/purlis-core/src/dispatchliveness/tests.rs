// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! How a task's liveness is said (#3900): whether its program runs, when it last ran a command
//! or sent a note, and how it stands across the app's restart. Every expected sentence is
//! written out.

use std::time::{Duration, SystemTime};

use super::*;

fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn a_task_started_here_says_its_program_runs_and_what_it_did_last() {
    let mut heard = Heard::default();
    heard.did(Did::RanACommand, at(1_000));
    let live = heard.liveness(true, Restart::StartedHere, at(1_180));
    assert_eq!(
        live.said(),
        "its program is running, last ran a command 3m ago"
    );
}

#[test]
fn a_note_is_heard_as_a_note_and_the_later_of_the_two_counts() {
    let mut heard = Heard::default();
    heard.did(Did::RanACommand, at(1_000));
    heard.did(Did::SentANote, at(1_100));
    let live = heard.liveness(true, Restart::StartedHere, at(1_145));
    assert_eq!(
        live.said(),
        "its program is running, last sent a note 45s ago"
    );
}

#[test]
fn a_task_nothing_was_heard_from_says_so() {
    let live = Heard::default().liveness(true, Restart::StartedHere, at(10));
    assert_eq!(
        live.said(),
        "its program is running, no command or note heard from it since this app started"
    );
}

#[test]
fn a_task_kept_running_across_the_restart_says_it_survived() {
    let mut heard = Heard::default();
    heard.did(Did::RanACommand, at(0));
    let live = heard.liveness(true, Restart::KeptRunning, at(7_500));
    assert_eq!(
        live.said(),
        "its program is running, last ran a command 2h 5m ago, it kept running across \
         purlis's restart"
    );
}

#[test]
fn a_task_started_again_after_the_restart_says_so() {
    let live = Heard::default().liveness(true, Restart::StartedAgain, at(0));
    assert_eq!(
        live.said(),
        "its program is running, no command or note heard from it since this app started, \
         purlis's restart ended it and started it again on its conversation"
    );
}

#[test]
fn a_task_whose_program_ended_says_it_is_not_running() {
    let live = Heard::default().liveness(false, Restart::StartedHere, at(0));
    assert_eq!(
        live.said(),
        "its program is not running, no command or note heard from it since this app started"
    );
}

#[test]
fn an_orphan_is_marked_orphaned_by_restart_explicitly() {
    let live = Liveness::orphaned();
    assert!(!live.running);
    assert_eq!(
        live.said(),
        "orphaned by restart: purlis can no longer reach it, and no program of it runs under \
         this app"
    );
    assert!(live.is_orphaned());
}

#[test]
fn a_heard_time_after_now_reads_as_just_now() {
    // A clock that stepped back is no age in the future.
    let mut heard = Heard::default();
    heard.did(Did::SentANote, at(500));
    let live = heard.liveness(true, Restart::StartedHere, at(400));
    assert_eq!(live.heard_secs, Some(0));
}

#[test]
fn it_travels_on_the_wire_and_an_older_app_s_row_has_none() {
    let mut heard = Heard::default();
    heard.did(Did::RanACommand, at(0));
    let live = heard.liveness(true, Restart::KeptRunning, at(60));
    let json = serde_json::to_value(&live).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "running": true,
            "heard_secs": 60,
            "did": "ran_a_command",
            "restart": "kept_running"
        })
    );
    let back: Liveness = serde_json::from_value(json).unwrap();
    assert_eq!(back, live);
    // A restart word a newer app knows that this one does not reads as no restart.
    let newer: Liveness =
        serde_json::from_value(serde_json::json!({"running": false, "restart": "moved"})).unwrap();
    assert_eq!(newer.restart, Restart::StartedHere);
}
