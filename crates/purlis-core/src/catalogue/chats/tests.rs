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
        workspace: Some("alpha".into()),
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

/// `ended_record`, minted as `id`, from `started` to `ended` (hours of 2026-11-11).
fn ended_between(id: &str, (worker, asker): (&str, &str), (started, ended): (u32, u32)) -> Record {
    let mut record = ended_record(worker, asker, "alpha", "done");
    record.id = id.to_owned();
    record.started = format!("2026-11-11T{started:02}:00:00+00:00");
    record.ended = Some(format!("2026-11-11T{ended:02}:00:00+00:00"));
    record
}

fn with_history(limit: u32) -> Asked {
    Asked {
        history: true,
        limit,
    }
}

#[test]
fn a_nested_dispatch_lists_each_closed_chat_once_as_the_worker_it_was() {
    // G asked P, and P asked S. P was closed before S ended, so S's record is the newest
    // ended and names P as its asker before P's own record is read.
    let (g, p, s) = (
        "01KA00000000000000000000G0",
        "01KA00000000000000000000P0",
        "01KA00000000000000000000S0",
    );
    let records = [
        ended_between("01KA0000000000000000000002", (s, p), (10, 12)),
        ended_between("01KA0000000000000000000001", (p, g), (9, 11)),
    ];
    let data = answer(&[], &records, "alpha", with_history(HISTORY_DEFAULT));
    assert_eq!(ids(&data), [s, p, g], "each chat once");
    let p_row = &chats_of(&data)[1];
    assert_eq!(p_row["harness"], "claude", "P is the worker it was");
    assert_eq!(p_row["cause"], "reported");
    assert_eq!(p_row["parent"]["id"], g);
    assert_eq!(p_row["started"], "2026-11-11T09:00:00Z");
    assert_eq!(chats_of(&data)[0]["parent"]["id"], p);
    assert_eq!(chats_of(&data)[2]["cause"], "closed");
}

#[test]
fn a_closed_chat_that_asked_twice_is_listed_once() {
    let a = "01KA00000000000000000000A0";
    let records = [
        ended_between(
            "01KA0000000000000000000002",
            ("01KA00000000000000000000W2", a),
            (10, 11),
        ),
        ended_between(
            "01KA0000000000000000000001",
            ("01KA00000000000000000000W1", a),
            (9, 10),
        ),
    ];
    let data = answer(&[], &records, "alpha", with_history(HISTORY_DEFAULT));
    assert_eq!(
        ids(&data),
        [
            "01KA00000000000000000000W2",
            a,
            "01KA00000000000000000000W1"
        ]
    );
}

#[test]
fn history_lists_the_dispatch_that_ended_last_first() {
    let a = "01KA00000000000000000000A0";
    // The newer record (by its start) ended first.
    let records = [
        ended_between(
            "01KA0000000000000000000002",
            ("01KA00000000000000000000W2", a),
            (10, 11),
        ),
        ended_between(
            "01KA0000000000000000000001",
            ("01KA00000000000000000000W1", a),
            (9, 12),
        ),
    ];
    let data = answer(&[], &records, "alpha", with_history(1));
    assert_eq!(ids(&data), ["01KA00000000000000000000W1", a]);
}

#[test]
fn a_parent_in_another_workspace_or_at_the_root_is_named_by_its_relation_only() {
    let mut handed = chat("01KA00000000000000000000A1", "handed here from beta");
    handed.parent = Some(Parent {
        id: "01KA00000000000000000000B0".to_owned(),
        relation: Relation::HandedOffFrom,
        workspace: Some("beta".to_owned()),
    });
    let mut from_root = task_of(
        "01KA00000000000000000000R0",
        chat("01KA00000000000000000000A2", "dispatched from the root"),
    );
    from_root.parent.as_mut().unwrap().workspace = None;
    let here = task_of(
        "01KA00000000000000000000A1",
        chat("01KA00000000000000000000A3", "a task of a chat here"),
    );
    let mut record = ended_record(
        "01KA00000000000000000000D1",
        "01KA00000000000000000000E0",
        "alpha",
        "done",
    );
    record.asker.workspace = Some("beta".to_owned());
    let data = answer(
        &[handed, from_root, here],
        &[record],
        "alpha",
        with_history(HISTORY_DEFAULT),
    );
    let rows = chats_of(&data);
    assert_eq!(
        rows[0]["parent"],
        json!({ "id": null, "relation": "handed-off-from" })
    );
    assert_eq!(
        rows[1]["parent"],
        json!({ "id": null, "relation": "dispatched" })
    );
    assert_eq!(rows[2]["parent"]["id"], "01KA00000000000000000000A1");
    assert_eq!(
        rows[3]["parent"],
        json!({ "id": null, "relation": "dispatched" }),
        "a history worker whose asker is elsewhere"
    );
    assert_eq!(rows.len(), 4, "and that asker is not listed");
    assert!(!data.to_string().contains("01KA00000000000000000000B0"));
    assert!(!data.to_string().contains("01KA00000000000000000000E0"));
}

#[test]
fn recorded_chats_with_no_id_from_two_runs_are_two_chats() {
    let unnamed = |id: &str| {
        let mut record = ended_record("", "", "alpha", "done");
        record.id = id.to_owned();
        record.worker.chat.id = None;
        record.asker.chat.id = None;
        record
    };
    let records = [
        unnamed("01KA0000000000000000000002"),
        unnamed("01KA0000000000000000000001"),
    ];
    let data = answer(&[], &records, "alpha", with_history(HISTORY_DEFAULT));
    let listed = ids(&data);
    assert_eq!(listed.len(), 4, "{listed:?}");
    let unique: std::collections::HashSet<&String> = listed.iter().collect();
    assert_eq!(unique.len(), 4, "{listed:?}");
    assert!(
        listed
            .iter()
            .all(|id| id.starts_with("01KA00000000000000000000"))
    );
    assert_eq!(chats_of(&data)[0]["parent"]["id"], listed[1].as_str());
}

#[test]
fn the_longest_history_fits_in_an_answer() {
    // Every name a record keeps at its most, in the character JSON escapes longest.
    let longest = "\u{1}".repeat(crate::dispatchrecord::MOST_NAME_BYTES);
    let records: Vec<Record> = (0..HISTORY_MOST)
        .map(|n| {
            let mut record = ended_record(
                &format!("01KA0000000000000000000W{n:02X}"),
                &format!("01KA0000000000000000000A{n:02X}"),
                "alpha",
                "done",
            );
            record.task = Some(longest.clone());
            record.worker.chat.persona = Some(longest.clone());
            record.worker.harness = Some(longest.clone());
            record.asker.chat.name = longest.clone();
            record.asker.chat.persona = Some(longest.clone());
            record
        })
        .collect();
    let data = answer(&[], &records, "alpha", with_history(HISTORY_MOST));
    assert_eq!(chats_of(&data).len(), 2 * HISTORY_MOST as usize);
    let bytes = serde_json::to_vec(&data).unwrap().len();
    assert!(
        bytes < crate::catalogue::answer::MOST_BYTES,
        "{bytes} bytes"
    );
}
