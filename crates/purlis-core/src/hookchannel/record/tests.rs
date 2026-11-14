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
            arm: None,
            hook_ms: 1,
            agent: Some("agent-1".into()),
            at_ms: 0,
            skill: None,
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

/// A skill change a terminal asked the host to audit (SK-11a, #6030).
fn an_install_at_a_terminal() -> purlis_session_protocol::session::SkillChangeAudit {
    purlis_session_protocol::session::SkillChangeAudit {
        act: purlis_session_protocol::session::SkillAct::Install,
        level: "project".into(),
        skills: vec!["notify".into()],
        source: "https://github.com/acme/skills".into(),
        pin: "0123456789abcdef0123456789abcdef01234567".into(),
    }
}

/// A project at `dir` whose shared settings hold `table`.
fn a_project_holding(dir: &Path, table: &str) {
    std::fs::write(dir.join("charter.toml"), format!("schema = 1\n\n{table}")).unwrap();
}

#[test]
fn a_terminals_skill_change_on_an_org_project_is_one_audited_event_of_the_person_at_the_terminal() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_project_holding(project.path(), "[skill_policy]\nmin_age_days = 0\n");
    let log = a_log_in(logs.path());

    log.audit_skill_change(project.path(), &an_install_at_a_terminal())
        .expect("audited");

    let events = crate::eventlog::read(logs.path()).expect("the log reads");
    let audited: Vec<_> = events
        .iter()
        .filter(|event| event.kind.starts_with("skill."))
        .map(|event| (event.kind.as_str(), event.chat.clone(), event.body.clone()))
        .collect();
    assert_eq!(
        audited,
        [(
            "skill.install",
            None,
            serde_json::json!({
                "actor_kind": "human",
                "actor": "operator",
                "scope": "terminal",
                "level": "project",
                "skills": ["notify"],
                "source": "https://github.com/acme/skills",
                "pin": "0123456789abcdef0123456789abcdef01234567",
            })
        )]
    );
}

#[test]
fn a_terminals_skill_change_on_a_project_without_controls_is_refused_and_nothing_is_written() {
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    a_project_holding(project.path(), "");
    let log = a_log_in(logs.path());

    let refused = log
        .audit_skill_change(project.path(), &an_install_at_a_terminal())
        .expect_err("refused");

    assert!(refused.contains("[skill_policy]"), "{refused}");
    let events = crate::eventlog::read(logs.path()).unwrap_or_default();
    assert!(
        !events.iter().any(|event| event.kind.starts_with("skill.")),
        "nothing audited: {events:?}"
    );
}

/// The chats' own runs' ends in the log in `dir` (no parent run): each one's state and cause.
fn own_runs_ended(dir: &Path) -> Vec<(String, String)> {
    crate::eventlog::read(dir)
        .expect("the log reads")
        .iter()
        .filter(|event| event.kind == "run.ended" && event.parent_run.is_none())
        .map(|event| {
            (
                event.body["state"].as_str().unwrap_or_default().to_owned(),
                event.body["cause"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_run_the_window_replaced_ends_superseded_in_the_hosts_log_though_the_host_closed_it() {
    // ADR 0076 §2, Q247: a run a restart for project settings replaces has ended superseded,
    // never "stopped as you closed it", where the host writes the log as where the window does.
    let (logs, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(logs.path());
    a_chat_with_a_sub_agent(&log, project.path(), 4);
    log.record(
        serde_json::to_value(Entry::Superseded {
            plane: project.path().to_path_buf(),
            number: 4,
        })
        .unwrap(),
    )
    .expect("the host records it");

    log.ended(&an_end(
        project.path(),
        4,
        Exit::Code(0),
        Some(purlis_host::Stopped::Closed),
        true,
    ));

    let superseded = [("completed".to_owned(), "superseded".to_owned())];
    assert_eq!(sub_agents_ended(logs.path()), superseded);
    assert_eq!(
        own_runs_ended(logs.path()),
        superseded,
        "ended once, superseded"
    );
}
