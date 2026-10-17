// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use purlis_host::Record as _;
use purlis_session_protocol::session::Exit;

use crate::dispatchrecord;
use crate::eventlog::{ArgsKey, Attributes, Began, Entry, Log, Recorder};

use super::*;

/// The device every log here is written for.
const DEVICE: &str = "01J9ZZDEVICE0000000000000";

/// An event log in `dir`, as a host opens it the first time it is needed.
fn a_log_in(dir: &Path) -> EventLog {
    let dir = dir.to_path_buf();
    EventLog::opening(move || {
        Ok(Recorder::new(
            Log::open(&dir, DEVICE)?,
            ArgsKey::open(&dir)?,
        ))
    })
}

const CHAT: &str = "01K6W0RKER000000000000000B";
const RUN: &str = "01K6W0RKRUN00000000000000B";

/// Chat `number` of the project at `root` began run [`RUN`], and a sub-agent of it ran a tool,
/// as the window tells the host's log.
fn a_chat_with_a_sub_agent(log: &EventLog, root: &Path, number: u32) {
    let record = |entry: Entry| {
        log.record(serde_json::to_value(entry).unwrap())
            .expect("the host records it");
    };
    record(Entry::Begin {
        plane: root.to_path_buf(),
        number,
        chat: CHAT.into(),
        run: RUN.into(),
        cause: Began::Start,
        attributes: Attributes::default(),
        held: false,
        trust: None,
    });
    record(Entry::Tool {
        plane: root.to_path_buf(),
        call: crate::hookwire::ToolCall {
            chat: number,
            tool_hook: "pretooluse".into(),
            tool: Some("Bash".into()),
            call: Some("toolu_01".into()),
            args: None,
            decision: crate::hookwire::Decision::None,
            rule: None,
            hook_ms: 1,
            agent: Some("agent-1".into()),
            at_ms: 0,
        },
    });
}

/// The sub-agents' ends in the log in `dir`: each child run's state and cause.
fn sub_agents_ended(dir: &Path) -> Vec<(String, String)> {
    crate::eventlog::read(dir)
        .expect("the log reads")
        .iter()
        .filter(|event| event.kind == "run.ended" && event.parent_run.as_deref() == Some(RUN))
        .map(|event| {
            (
                event.body["state"].as_str().unwrap_or_default().to_owned(),
                event.body["cause"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// A dispatch the chat [`CHAT`], number `number`, works, owing its report to a chat that asked.
fn a_dispatch_worked_by(root: &Path, number: u32) -> String {
    dispatchrecord::open(
        root,
        dispatchrecord::Opening {
            mode: dispatchrecord::Mode::Task,
            asker: dispatchrecord::Asker {
                chat: dispatchrecord::ChatRef {
                    chat: 99,
                    id: Some("01K6ASKER0000000000000000A".into()),
                    name: "steward 99".into(),
                    persona: Some("steward".into()),
                },
                workspace: None,
                by_person: false,
                session_record: None,
            },
            persona: Some("devops".into()),
            worker: dispatchrecord::Worker {
                chat: dispatchrecord::ChatRef {
                    chat: number,
                    id: Some(CHAT.into()),
                    name: "look at the queue".into(),
                    persona: Some("devops".into()),
                },
                harness: None,
                profile: None,
                session_record: None,
            },
            task: Some("look at the queue".into()),
            place: dispatchrecord::Place {
                workspace: None,
                folder: None,
                worktree: None,
            },
            brief: "Look at the queue.".into(),
            report_owed: true,
        },
        chrono::Utc::now(),
    )
    .expect("a running dispatch")
    .id
}

fn an_end<'a>(
    root: &'a Path,
    number: u32,
    exit: Exit,
    stopped: Option<purlis_host::Stopped>,
    watched: bool,
) -> purlis_host::Ended<'a> {
    purlis_host::Ended {
        project_path: root,
        number,
        exit,
        stopped,
        watched,
    }
}

#[test]
fn a_chat_that_ends_by_itself_ends_its_sub_agents_runs_in_the_hosts_log_for_its_exit() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);

    log.ended(&an_end(project.path(), 4, Exit::Code(3), None, true));

    assert_eq!(
        sub_agents_ended(logs.path()),
        [("failed".to_owned(), "exited".to_owned())]
    );
}

#[test]
fn a_chat_the_host_stopped_ends_its_sub_agents_runs_for_that_stop() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);

    log.ended(&an_end(
        project.path(),
        4,
        Exit::Code(0),
        Some(purlis_host::Stopped::Killed),
        false,
    ));

    assert_eq!(
        sub_agents_ended(logs.path()),
        [("stopped".to_owned(), "killed".to_owned())]
    );
}

#[test]
fn a_dispatch_whose_chat_ended_with_no_window_there_ends_without_its_report() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);
    let dispatched = a_dispatch_worked_by(project.path(), 4);

    log.ended(&an_end(project.path(), 4, Exit::Code(0), None, false));

    let ended = dispatchrecord::read(project.path(), &dispatched).expect("the record");
    assert!(!ended.running());
    assert_eq!(ended.ended_by, Some(dispatchrecord::EndedBy::Unreported));
    assert_eq!(
        ended.report.map(|report| report.text),
        Some(dispatchrecord::ENDED_WITHOUT_A_REPORT.to_owned())
    );
    assert!(ended.undelivered.is_some(), "owed to the chat that asked");
}

#[test]
fn a_dispatch_whose_chat_ended_with_a_window_there_is_left_to_the_window() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);
    let watched = a_dispatch_worked_by(project.path(), 4);

    log.ended(&an_end(project.path(), 4, Exit::Code(0), None, true));

    assert!(
        dispatchrecord::read(project.path(), &watched)
            .expect("the record")
            .running(),
        "the window decides it"
    );
}

#[test]
fn a_dispatch_whose_chat_the_host_stopped_is_not_ended_as_unreported() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);
    let stopped = a_dispatch_worked_by(project.path(), 4);

    log.ended(&an_end(
        project.path(),
        4,
        Exit::Code(0),
        Some(purlis_host::Stopped::Operator),
        false,
    ));

    assert!(
        dispatchrecord::read(project.path(), &stopped)
            .expect("the record")
            .running()
    );
}

#[test]
fn a_chat_its_last_host_left_running_ends_its_sub_agents_runs_as_a_crash() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);
    let left: purlis_host::Started = serde_json::from_value(serde_json::json!({
        "program": "p-4",
        "project": "01J9PRJCT00000000000000000",
        "project_path": project.path(),
        "number": 4,
    }))
    .expect("a start the last host kept");

    log.left_by_its_last_host(&left);

    assert_eq!(
        sub_agents_ended(logs.path()),
        [("failed".to_owned(), "host-crash".to_owned())]
    );
}

#[test]
fn a_write_the_host_does_not_read_is_refused_in_words() {
    let logs = tempfile::tempdir().unwrap();
    let refused = a_log_in(logs.path())
        .record(serde_json::json!({"no": "such entry"}))
        .expect_err("refused");
    assert!(refused.contains("does not read"), "{refused}");
}

#[test]
fn a_run_s_end_is_the_host_s_own_stop_first_and_then_its_exit_status() {
    use crate::session::Exit as Ended;
    use crate::state::run::{Cause, Exit, RunState, StopBy};
    assert_eq!(
        how_its_run_ended(Some(StopBy::Killed), &Ended::Code(0)),
        (RunState::Stopped, Cause::Stop(StopBy::Killed))
    );
    assert_eq!(
        how_its_run_ended(None, &Ended::Code(0)),
        (RunState::Completed, Cause::Exited(Exit::Code(0)))
    );
    assert_eq!(
        how_its_run_ended(None, &Ended::Code(3)),
        (RunState::Failed, Cause::Exited(Exit::Code(3)))
    );
    assert_eq!(
        how_its_run_ended(None, &Ended::Signal("SIGKILL".to_owned())),
        (RunState::Failed, Cause::Exited(Exit::Signal))
    );
}
