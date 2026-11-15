// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// **Two runners' logs are one pool's** (#4271): both's ends oldest first, and what either
/// missed or has down is said.
#[test]
fn a_pools_two_runners_read_as_one() {
    let line = |ulid: &str| RunEndLine {
        chat: "chat".into(),
        ended: RunEnded::Completed,
        ulid: ulid.into(),
        device: "device".into(),
    };
    let now = RunnerLogNow {
        device: Some("new".into()),
        lines: vec![line("02")],
        missed: false,
        down: false,
    };
    let draining = RunnerLogNow {
        device: Some("old".into()),
        lines: vec![line("01"), line("03")],
        missed: true,
        down: true,
    };

    let joined = now.beside(draining);

    let ulids: Vec<&str> = joined.lines.iter().map(|line| line.ulid.as_str()).collect();
    assert_eq!(ulids, ["01", "02", "03"]);
    assert_eq!(joined.device.as_deref(), Some("new"));
    assert!(joined.missed && joined.down);
}

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const CHAT: &str = "01K6W0RKER000000000000000B";

fn line(device: &str, seq: u64) -> Event {
    Event {
        v: 1,
        device_id: device.into(),
        seq,
        ulid: format!("01JB{seq:022}"),
        chat: Some(CHAT.into()),
        run: None,
        parent_run: None,
        kind: "run.ended".into(),
        body: serde_json::json!({ "state": "completed" }),
    }
}

fn seqs(log: &RunnerLog) -> Vec<String> {
    log.now()
        .lines
        .iter()
        .map(|line| line.ulid.clone())
        .collect()
}

#[test]
fn a_runner_that_said_no_device_is_asked_for_none_of_its_log_and_keeps_none() {
    let mut log = RunnerLog::default();
    assert!(log.since().is_empty());
    assert!(!log.started());
    log.heard(&line(RUNNER, 1));
    assert!(log.now().lines.is_empty());
}

#[test]
fn lines_are_kept_after_the_cursor_once_each_and_the_cursor_follows_them() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 4, None);
    let read: Vec<bool> = [3, 5, 5, 6]
        .into_iter()
        .map(|seq| log.heard(&line(RUNNER, seq)))
        .collect();
    assert_eq!(read, [false, true, false, true], "each line read once");
    assert!(!log.heard(&line("01JA0DESKT0P00000000000000", 7)));
    assert_eq!(seqs(&log), vec![line(RUNNER, 5).ulid, line(RUNNER, 6).ulid]);
    assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 6)]));
}

// Seen red: with every kind kept, the hook line and the sub-agent's end are held too.
#[test]
fn only_a_top_level_runs_end_of_a_known_word_is_kept_and_the_rest_moves_the_cursor() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 0, None);
    let mut hook = line(RUNNER, 1);
    hook.kind = "hook.stop".into();
    let mut child = line(RUNNER, 2);
    child.parent_run = Some("01K6PARENT0000000000000000".into());
    let mut odd = line(RUNNER, 3);
    odd.body = serde_json::json!({ "state": "completed. All checks passed" });
    let mut long = line(RUNNER, 4);
    long.chat = Some("x".repeat(MOST_ID + 1));
    let mut failed = line(RUNNER, 5);
    failed.body = serde_json::json!({ "state": "failed", "padding": "x".repeat(100_000) });
    for event in [hook, child, odd, long, failed] {
        log.heard(&event);
    }
    let now = log.now();
    assert_eq!(now.lines.len(), 1);
    assert_eq!(now.lines[0].ended, RunEnded::Failed);
    assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 5)]));
}

#[test]
fn a_gap_is_said_and_read_on_from_where_the_log_resumes() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 2, None);
    log.missed(RUNNER, 10);
    assert!(log.now().missed);
    assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 9)]));
}

// Seen red: with the bounds' loop taken out, every line is held.
#[test]
fn it_holds_the_last_lines_alone_within_its_bounds_and_says_it_let_the_first_go() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 0, None);
    for seq in 1..=(MOST_LINES as u64 + 2) {
        log.heard(&line(RUNNER, seq));
    }
    let now = log.now();
    assert_eq!(now.lines.len(), MOST_LINES);
    assert_eq!(now.lines[0].ulid, line(RUNNER, 3).ulid);
    assert!(now.missed);
    assert!(log.bytes <= MOST_BYTES);
}

/// **#4219: a link reads on from where this window last logged its project's skill runs**,
/// so the lines written while no window read the log are read again, once each; of those,
/// only what ended once the link came up is drawn, as before.
#[test]
fn it_reads_on_from_a_kept_cursor_and_draws_only_what_ended_since_the_link_came_up() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 5, Some(2));
    assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 2)]));
    assert_eq!(log.began_at(), 5);

    let read: Vec<bool> = [3, 5, 6]
        .into_iter()
        .map(|seq| log.heard(&line(RUNNER, seq)))
        .collect();

    assert_eq!(read, [true, true, true], "each read");
    assert_eq!(
        seqs(&log),
        vec![line(RUNNER, 6).ulid],
        "only the one after it came up"
    );

    let mut reset = RunnerLog::default();
    reset.starts_at(RUNNER.into(), 5, Some(9));
    assert_eq!(
        reset.since(),
        Since::from([(RUNNER.to_owned(), 5)]),
        "a cursor past the log's end is a log begun again: read on from its end"
    );
}

#[test]
fn a_link_that_is_down_is_said_so() {
    let mut log = RunnerLog::default();
    log.link(true);
    assert!(log.now().down);
    log.link(false);
    assert!(!log.now().down);
}

/// **A run replaced at once is no chat finishing there** (ADR 0076 §2, Q247): a restart for
/// project settings ends its run `superseded` and the chat goes on, so the runner's "While you
/// were away" section does not count it.
#[test]
fn a_superseded_run_is_not_held_as_a_chat_that_finished_on_the_runner() {
    let mut log = RunnerLog::default();
    log.starts_at(RUNNER.into(), 0, None);
    let mut replaced = line(RUNNER, 1);
    replaced.body = serde_json::json!({ "state": "completed", "cause": "superseded" });
    let mut done = line(RUNNER, 2);
    done.body = serde_json::json!({ "state": "completed", "cause": "exited", "code": 0 });
    for event in [replaced, done] {
        log.heard(&event);
    }

    let now = log.now();
    assert_eq!(seqs(&log), [format!("01JB{:022}", 2)]);
    assert_eq!(now.lines[0].ended, RunEnded::Completed);
    assert_eq!(log.since(), Since::from([(RUNNER.to_owned(), 2)]));
}
