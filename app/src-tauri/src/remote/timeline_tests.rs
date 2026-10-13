//! What a phone's level-2 timeline is made of (iOS design §5.1): the items each event and
//! ask becomes, and the stream a link reads them on, over the app's own planes.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::harness::asks::test_support;
use purlis_session_protocol::timeline::{Role, Timelines, What};

use super::board_tests::{KEPT, a_plane_recording, board_over, held_of, linked, start};
use super::timeline::{ask_item, item_of};
use crate::host::pretend::Pretend;

const DEVICE: &str = "01J9DEVCE00000000000000000";
const RUN: &str = "01J9ZQ7000000000000000000C";

fn event(kind: &str, body: serde_json::Value) -> purlis_core::eventlog::Event {
    purlis_core::eventlog::Event {
        v: 1,
        device_id: DEVICE.into(),
        seq: 7,
        ulid: "01J9ZQ6000000000000000000B".into(),
        chat: Some(KEPT.into()),
        run: Some(RUN.into()),
        parent_run: None,
        kind: kind.into(),
        body,
    }
}

#[test]
fn a_prompt_submitted_begins_a_turn_and_a_stop_ends_it() {
    let began = item_of(&event("hook.userpromptsubmit", serde_json::json!({})), KEPT).unwrap();
    assert_eq!(began.level, 2);
    assert!(matches!(
        began.what,
        What::Turn {
            began: true,
            ref turn,
            ..
        } if turn == RUN
    ));
    let ended = item_of(&event("hook.stop", serde_json::json!({})), KEPT).unwrap();
    assert!(matches!(
        ended.what,
        What::Turn {
            began: false,
            stop_reason: Some(ref reason),
            ..
        } if reason == "end_turn"
    ));
}

#[test]
fn a_tool_event_is_a_tool_item_with_name_verdict_and_timing_and_never_its_arguments() {
    let item = item_of(
        &event(
            "hook.pretooluse",
            serde_json::json!({
                "tool": "Bash", "call": "c1", "args": "deadbeef",
                "decision": "allow", "hook_ms": 3, "tool_ms": 1200
            }),
        ),
        KEPT,
    )
    .unwrap();
    match &item.what {
        What::Tool {
            title,
            status,
            duration_ms,
            ..
        } => {
            assert_eq!(title, "Bash");
            assert_eq!(status, "allow");
            assert_eq!(*duration_ms, Some(1200));
        }
        other => panic!("{other:?}"),
    }
    assert!(!serde_json::to_string(&item).unwrap().contains("deadbeef"));
}

#[test]
fn another_chats_event_and_an_unknown_kind_are_not_items() {
    assert!(
        item_of(
            &event("hook.stop", serde_json::json!({})),
            "01J9OTHER00000000000000000"
        )
        .is_none()
    );
    assert!(item_of(&event("trust.sandbox.grant", serde_json::json!({})), KEPT).is_none());
}

#[test]
fn an_ask_item_carries_the_sources_choices_and_marks_the_lasting_one() {
    let raised = test_support::raised_with_an_always_choice(KEPT);
    let item = ask_item(&raised);
    match item.what {
        What::Ask {
            choices,
            elicits_secret,
            summary,
            ..
        } => {
            assert!(!elicits_secret);
            assert_eq!(summary, "Allow cargo test?");
            assert!(choices.iter().any(|c| c.lasts));
            assert!(choices.iter().any(|c| !c.lasts));
        }
        other => panic!("{other:?}"),
    }
    let _ = Role::User;
}

fn stop(session: u32) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat: session,
        event: purlis_core::state::Event::Stop,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

/// The next items on `reader`, up to and with the first for which `until` holds.
async fn read_until(
    reader: &mut purlis_session_protocol::timeline::Reader,
    until: impl Fn(&What) -> bool,
) -> Vec<What> {
    let mut got = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let item = reader.next().await.unwrap().unwrap();
            let done = until(&item.what);
            got.push(item.what);
            if done {
                break;
            }
        }
    })
    .await
    .expect("the items arrive within the poll");
    got
}

#[tokio::test]
async fn a_timeline_opens_with_the_logs_turns_and_the_open_asks_then_follows_who_closed_each() {
    use purlis_core::eventlog::{ArgsKey, Followed, Log, Recorder, RunOf};
    let pretend = Pretend::default();
    let logs = tempfile::tempdir().unwrap();
    let events: crate::hooks::Events = Arc::new(Mutex::new(Recorder::new(
        Log::open(logs.path(), DEVICE).unwrap(),
        ArgsKey::open(logs.path()).unwrap(),
    )));
    let planes = a_plane_recording(&pretend, Some(Arc::clone(&events)));
    let session = start(&planes, "/nowhere/claude", "one", Some(KEPT), true);
    let held = held_of(&planes);
    {
        let mut recorder = events.lock().unwrap();
        recorder.knows(
            held.root(),
            session,
            RunOf {
                chat: KEPT,
                run: RUN,
            },
        );
        recorder
            .report(held.root(), &stop(session), Followed::No)
            .unwrap();
    }
    let first = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_allow_ask(),
        Instant::now(),
    );
    let first_id = first.raised.id.to_string();

    let (client, mut streams) = linked(board_over(planes.clone())).await;
    let attached = client.attach_timeline(KEPT).await.unwrap();
    assert_eq!(attached.level, 2, "a hooks harness");
    let mut reader = Timelines::default()
        .accept(streams.accept().await.unwrap())
        .await
        .unwrap();
    let snapshot = read_until(&mut reader, |what| *what == What::CaughtUp).await;
    // The run the app's start began, the turn the hook ended, the open ask, caught up.
    assert!(
        matches!(&snapshot[0], What::Turn { began: true, .. }),
        "{snapshot:?}"
    );
    assert!(
        matches!(&snapshot[1], What::Turn { began: false, turn, .. } if turn == RUN),
        "{snapshot:?}"
    );
    assert!(
        matches!(&snapshot[2], What::Ask { ask, .. } if *ask == first_id),
        "{snapshot:?}"
    );
    assert_eq!(snapshot.len(), 4, "{snapshot:?}");

    // The desktop answers: the phone hears the ask close, by the desktop.
    held.hooks().answer(session, &first_id, "allow").unwrap();
    let heard = read_until(&mut reader, |what| matches!(what, What::AskClosed { .. })).await;
    assert_eq!(
        heard.last(),
        Some(&What::AskClosed {
            ask: first_id.clone(),
            by: "desktop".into()
        })
    );

    // A new ask reaches the phone live; its own answer closes it, by the device.
    let second = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_allow_ask(),
        Instant::now(),
    );
    let second_id = second.raised.id.to_string();
    let heard = read_until(&mut reader, |what| matches!(what, What::Ask { .. })).await;
    assert!(matches!(heard.last(), Some(What::Ask { ask, .. }) if *ask == second_id));
    client.answer(KEPT, &second_id, "allow").await.unwrap();
    let heard = read_until(&mut reader, |what| matches!(what, What::AskClosed { .. })).await;
    assert_eq!(
        heard.last(),
        Some(&What::AskClosed {
            ask: second_id,
            by: "device".into()
        })
    );
    client
        .detach_timeline(KEPT, attached.timeline)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_timeline_without_an_event_log_still_carries_the_asks() {
    let pretend = Pretend::default();
    let planes = a_plane_recording(&pretend, None);
    let session = start(&planes, "/nowhere/claude", "one", Some(KEPT), true);
    let held = held_of(&planes);
    let raised = held.hooks().asks().raise_ask(
        &session.to_string(),
        test_support::an_ask_that_elicits_a_secret(),
        Instant::now(),
    );
    let (client, mut streams) = linked(board_over(planes.clone())).await;
    client.attach_timeline(KEPT).await.unwrap();
    let mut reader = Timelines::default()
        .accept(streams.accept().await.unwrap())
        .await
        .unwrap();
    let snapshot = read_until(&mut reader, |what| *what == What::CaughtUp).await;
    assert!(matches!(
        &snapshot[0],
        What::Ask { ask, elicits_secret: true, .. } if *ask == raised.raised.id.to_string()
    ));
}
