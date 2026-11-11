// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::{Value, json};

use super::*;
use crate::state::State;

/// A chat at rest in `alpha`, the person's own, with an id.
fn chat(id: &str, name: &str) -> Chat {
    Chat {
        id: id.to_owned(),
        name: name.to_owned(),
        persona: Some("steward".into()),
        harness: Some("claude".into()),
        workspace: Some("alpha".into()),
        parent: None,
        facts: Facts {
            board: Some(State::Waiting),
            ..Facts::default()
        },
        started: Some(1_762_862_400),
        last_activity: Some(1_762_866_000),
        session_record: None,
    }
}

fn task_of(parent: &str, mut child: Chat) -> Chat {
    child.parent = Some(Parent {
        id: parent.to_owned(),
        relation: Relation::Dispatched,
    });
    child.facts.task = Some(TaskFacts {
        report: Report::Owed,
        outcome: None,
    });
    child
}

fn live() -> Asked {
    Asked {
        history: false,
        limit: HISTORY_DEFAULT,
    }
}

fn chats_of(data: &Value) -> Vec<Value> {
    data["chats"].as_array().expect("chats").clone()
}

fn ids(data: &Value) -> Vec<String> {
    chats_of(data)
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect()
}

/// An ended dispatch's record, as `app/dispatches/<id>.json` holds it.
fn ended_record(worker: &str, asker: &str, workspace: &str, outcome: &str) -> Record {
    serde_json::from_value(json!({
        "v": 1,
        "id": "01KA0000000000000000000000",
        "mode": "task",
        "asker": { "chat": 3, "id": asker, "name": "the asker", "persona": "steward",
                   "workspace": workspace },
        "persona": "builder",
        "worker": { "chat": 7, "id": worker, "name": "a worker", "persona": "builder",
                    "harness": "claude",
                    "session_record": format!("workspaces/{workspace}/sessions/20261111-120000-done.md") },
        "task": "build the thing",
        "place": { "workspace": workspace },
        "brief": "THE BRIEF, which no page reads",
        "report_owed": true,
        "started": "2026-11-11T10:00:00+00:00",
        "ended": "2026-11-11T11:00:00+00:00",
        "report": { "outcome": outcome, "text": "THE REPORT, which no page reads" },
    }))
    .expect("a record")
}

#[test]
fn the_default_answer_is_the_live_chats_of_the_asking_workspace_and_no_other() {
    let mut elsewhere = chat("01KA00000000000000000000B0", "elsewhere");
    elsewhere.workspace = Some("beta".into());
    let mut at_root = chat("01KA00000000000000000000C0", "at the root");
    at_root.workspace = None;
    let open = [
        chat("01KA00000000000000000000A0", "here"),
        elsewhere,
        at_root,
    ];
    let data = answer(&open, &[], "alpha", live());
    assert_eq!(ids(&data), ["01KA00000000000000000000A0"]);
    assert_eq!(data["history"], false);
}

#[test]
fn a_chat_says_its_name_persona_harness_state_cause_times_and_parent() {
    let open = [
        chat("01KA00000000000000000000A0", "asker"),
        task_of(
            "01KA00000000000000000000A0",
            chat("01KA00000000000000000000A1", "task"),
        ),
    ];
    let data = answer(&open, &[], "alpha", live());
    let task = &chats_of(&data)[1];
    assert_eq!(task["name"], "task");
    assert_eq!(task["persona"], "steward");
    assert_eq!(task["harness"], "claude");
    assert_eq!(task["state"], "waiting");
    assert_eq!(task["state_said"], "Waiting on a chat");
    assert_eq!(task["cause"], "turn-ended-owing-report");
    assert_eq!(task["started"], "2025-11-11T12:00:00Z");
    assert_eq!(task["last_activity"], "2025-11-11T13:00:00Z");
    assert_eq!(task["open"], true);
    assert_eq!(
        task["parent"],
        json!({ "id": "01KA00000000000000000000A0", "relation": "dispatched" })
    );
    assert_eq!(chats_of(&data)[0]["parent"], Value::Null);
}

#[test]
fn the_eight_chat_states_are_read_in_the_window_s_order() {
    let base = Facts::default();
    let with = |change: &dyn Fn(&mut Facts)| {
        let mut facts = base.clone();
        change(&mut facts);
        mark(&facts)
    };
    // Waiting on you comes before anything else that is true.
    assert_eq!(
        with(&|f| {
            f.needs_you = true;
            f.board = Some(State::Running);
            f.stuck = true;
        }),
        (Mark::Needs, "needs-you-queue")
    );
    assert_eq!(
        with(&|f| {
            f.stuck = true;
            f.board = Some(State::Running);
        }),
        (Mark::Stuck, "stuck-watch")
    );
    assert_eq!(
        with(&|f| f.board = Some(State::Running)),
        (Mark::Working, "turn-running")
    );
    assert_eq!(
        with(&|f| f.board = Some(State::Failed)),
        (Mark::Failed, "exited-failed")
    );
    assert_eq!(
        with(&|f| f.board = Some(State::Done)),
        (Mark::Ended, "exited")
    );
    assert_eq!(
        with(&|f| f.board = Some(State::Waiting)),
        (Mark::Idle, "turn-ended")
    );
    assert_eq!(
        with(&|f| {
            f.board = Some(State::Waiting);
            f.left_work = Some(false);
        }),
        (Mark::Ended, "turn-ended-nothing-left")
    );
    assert_eq!(
        with(&|f| {
            f.board = Some(State::Waiting);
            f.tasks_at_work = 2;
        }),
        (Mark::Waiting, "tasks-at-work")
    );
    assert_eq!(
        with(&|f| {
            f.board = Some(State::Waiting);
            f.own_work = true;
        }),
        (Mark::Working, "own-work")
    );
    assert_eq!(with(&|f| f.board = None), (Mark::CantTell, "shell"));
    assert_eq!(
        with(&|f| f.board = Some(State::Unknown)),
        (Mark::CantTell, "no-hook-heard")
    );
}

#[test]
fn a_task_s_record_is_read_before_its_program_s_state() {
    let task = |report, outcome: Option<&str>, board| {
        mark(&Facts {
            board: Some(board),
            task: Some(TaskFacts {
                report,
                outcome: outcome.map(str::to_owned),
            }),
            ..Facts::default()
        })
    };
    assert_eq!(
        task(Report::Sent, Some("done"), State::Waiting),
        (Mark::Ended, "reported")
    );
    assert_eq!(
        task(Report::Sent, Some("blocked"), State::Waiting),
        (Mark::Failed, "reported-failed")
    );
    assert_eq!(
        task(Report::Sent, Some("done"), State::Running),
        (Mark::Working, "turn-running")
    );
    assert_eq!(
        task(Report::Failed, Some("closed_by_person"), State::Done),
        (Mark::Ended, "ended-by-person")
    );
    assert_eq!(
        task(Report::Failed, None, State::Done),
        (Mark::Failed, "ended-without-report")
    );
    assert_eq!(
        task(Report::Owed, None, State::Done),
        (Mark::Failed, "ended-without-report")
    );
}

#[test]
fn every_mark_says_the_window_s_name_for_it() {
    let said: Vec<(&str, &str)> = Mark::ALL.iter().map(|m| (m.key(), m.said())).collect();
    assert_eq!(
        said,
        [
            ("needs", "Waiting on you"),
            ("stuck", "Stuck?"),
            ("failed", "Failed"),
            ("working", "Working"),
            ("waiting", "Waiting on a chat"),
            ("idle", "Idle"),
            ("ended", "Done"),
            ("cant-tell", "Can't tell"),
        ]
    );
}

#[test]
fn history_adds_the_workspace_s_ended_dispatches_with_their_asker_and_never_their_words() {
    let records = [
        ended_record(
            "01KA00000000000000000000D1",
            "01KA00000000000000000000D0",
            "alpha",
            "failed",
        ),
        ended_record(
            "01KA00000000000000000000E1",
            "01KA00000000000000000000E0",
            "beta",
            "done",
        ),
    ];
    let open = [chat("01KA00000000000000000000A0", "here")];
    assert_eq!(
        ids(&answer(&open, &records, "alpha", live())),
        ["01KA00000000000000000000A0"],
        "history is asked for, never given by default"
    );
    let data = answer(
        &open,
        &records,
        "alpha",
        Asked {
            history: true,
            limit: HISTORY_DEFAULT,
        },
    );
    assert_eq!(data["history"], true);
    assert_eq!(
        ids(&data),
        [
            "01KA00000000000000000000A0",
            "01KA00000000000000000000D1",
            "01KA00000000000000000000D0",
        ]
    );
    let worker = &chats_of(&data)[1];
    assert_eq!(worker["name"], "build the thing");
    assert_eq!(worker["persona"], "builder");
    assert_eq!(worker["state"], "failed");
    assert_eq!(worker["cause"], "reported-failed");
    assert_eq!(worker["open"], false);
    assert_eq!(worker["started"], "2026-11-11T10:00:00Z");
    assert_eq!(worker["last_activity"], "2026-11-11T11:00:00Z");
    assert_eq!(worker["session_record"], "20261111-120000-done.md");
    assert_eq!(
        worker["parent"],
        json!({ "id": "01KA00000000000000000000D0", "relation": "dispatched" })
    );
    let asker = &chats_of(&data)[2];
    assert_eq!(asker["name"], "the asker");
    assert_eq!(asker["state"], "ended");
    assert_eq!(asker["cause"], "closed");
    let text = data.to_string();
    assert!(!text.contains("THE BRIEF") && !text.contains("THE REPORT"));
}

#[test]
fn history_never_lists_an_open_chat_twice_and_keeps_to_its_limit() {
    let records: Vec<Record> = (0..5)
        .map(|n| {
            ended_record(
                &format!("01KA00000000000000000000F{n}"),
                "01KA00000000000000000000A0",
                "alpha",
                "done",
            )
        })
        .collect();
    let open = [
        chat("01KA00000000000000000000A0", "asker, still open"),
        chat("01KA00000000000000000000F0", "a worker kept open"),
    ];
    let data = answer(
        &open,
        &records,
        "alpha",
        Asked {
            history: true,
            limit: 2,
        },
    );
    assert_eq!(
        ids(&data),
        [
            "01KA00000000000000000000A0",
            "01KA00000000000000000000F0",
            "01KA00000000000000000000F1",
            "01KA00000000000000000000F2",
        ]
    );
}

#[test]
fn the_parameters_are_history_and_a_limit_of_one_to_two_hundred() {
    let parsed = |args: Value| asked(args.as_object().unwrap());
    assert_eq!(parsed(json!({})), Ok(live()));
    assert_eq!(
        parsed(json!({ "history": true, "limit": 200 })),
        Ok(Asked {
            history: true,
            limit: 200
        })
    );
    assert!(parsed(json!({ "history": true, "limit": 0 })).is_err());
    assert!(parsed(json!({ "history": true, "limit": 201 })).is_err());
    assert!(parsed(json!({ "history": "yes" })).is_err());
}

#[test]
fn a_chat_with_no_id_is_named_by_its_number_and_its_task_finds_it() {
    let mut asker = chat("", "no id yet");
    asker.id = number_id(4);
    let child = task_of(&number_id(4), chat("01KA00000000000000000000A1", "task"));
    let data = answer(&[asker, child], &[], "alpha", live());
    assert_eq!(ids(&data)[0], "chat-4");
    assert_eq!(chats_of(&data)[1]["parent"]["id"], "chat-4");
}
