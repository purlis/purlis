// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::time::Duration;

use super::*;
use crate::eventlog::stops::{
    self, Answer, AnsweredBy, AskAnswered, AskGroup, Block, Hold, Source, Stop, Target,
};
use crate::eventlog::{ArgsKey, Log, Recorder, Retention, RunOf, VERSION};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const NOW_MS: u64 = 1_790_000_000_000;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const OURS: &str = "01K6CHAT000000000000000004";
const THEIRS: &str = "01K6CHAT000000000000000005";

/// Event `seq` of chat `chat`, written `days_ago` days before [`NOW_MS`].
fn event(seq: u64, days_ago: u64, chat: &str, kind: &str, body: serde_json::Value) -> Event {
    Event {
        v: VERSION,
        device_id: DEVICE.to_owned(),
        seq,
        ulid: ulid::Ulid::from_parts(NOW_MS - days_ago * DAY_MS, u128::from(seq)).to_string(),
        chat: Some(chat.to_owned()),
        run: None,
        parent_run: None,
        kind: kind.to_owned(),
        body,
    }
}

/// A sandbox block of a connection to `host`, as the log writes it.
fn block(host: &str) -> Block {
    Block {
        tool: "Bash".to_owned(),
        source: Source::Sandbox,
        qualifier: None,
        target: Some(Target::host(host).unwrap()),
        args: None,
    }
}

fn body(block: &Block) -> serde_json::Value {
    serde_json::to_value(block).unwrap()
}

/// R-221: the Blocked log is a view of the event log, kept 30 days. The query's window and the
/// log's pruning are the same 30 days; a block 29 days old is listed and one 31 days old is not;
/// only blocks are listed (a hold and an answered ask are not), only the caller's chats, newest
/// first. Nothing beside the log is read: [`listed`] takes the log's events and the caller's
/// filter alone, and [`listed_in`] answers what [`listed`] answers of the log in its folder.
///
/// Mutations it catches: the window as `>` 30 days or dropped (the 31-day block listed), the
/// filter dropped (the other chat's block listed), any stop kind listed, oldest first.
#[test]
fn the_blocked_log_is_a_30_day_view_of_the_event_log() {
    assert_eq!(KEPT, Duration::from_secs(30 * 24 * 60 * 60));
    assert_eq!(
        Retention::DEFAULT.keep,
        KEPT,
        "the log prunes at the Blocked log's window"
    );

    let hold = serde_json::to_value(Hold {
        tool: "Bash".to_owned(),
        target: None,
        args: None,
    })
    .unwrap();
    let answered = serde_json::to_value(AskAnswered {
        ask: AskGroup::Sandbox,
        answer: Answer::Allowed,
        by: AnsweredBy::Window,
    })
    .unwrap();
    let events = vec![
        event(1, 31, OURS, stops::BLOCK, body(&block("old.example"))),
        event(2, 29, OURS, stops::BLOCK, body(&block("kept.example"))),
        event(3, 1, THEIRS, stops::BLOCK, body(&block("theirs.example"))),
        event(4, 1, OURS, stops::HOLD, hold),
        event(5, 1, OURS, stops::ASK_ANSWERED, answered),
        event(6, 0, OURS, stops::BLOCK, body(&block("new.example"))),
    ];
    let shown = listed(&events, NOW_MS, &|chat| chat == OURS);
    assert_eq!(
        shown,
        vec![
            Listed {
                at_ms: NOW_MS,
                seq: 6,
                chat: OURS.to_owned(),
                block: block("new.example"),
            },
            Listed {
                at_ms: NOW_MS - 29 * DAY_MS,
                seq: 2,
                chat: OURS.to_owned(),
                block: block("kept.example"),
            },
        ]
    );

    // The log in a folder of its own, written by the recorder: what `listed_in` reads of it is
    // what `listed` makes of its events, and nothing else.
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut recorder = Recorder::new(
        Log::open(dir.path(), DEVICE).unwrap(),
        ArgsKey::open(dir.path()).unwrap(),
    );
    recorder.knows(
        plane,
        4,
        RunOf {
            chat: OURS,
            run: "01K6RUN0000000000000000004",
        },
    );
    recorder
        .stop(plane, Some(4), &Stop::Block(block("api.example.com")))
        .unwrap();
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let read = listed_in(dir.path(), now, &|chat| chat == OURS).unwrap();
    assert_eq!(
        read,
        listed(&crate::eventlog::read(dir.path()).unwrap(), now, &|chat| {
            chat == OURS
        })
    );
    assert_eq!(read.len(), 1);
    assert_eq!(
        read[0].block.target,
        Some(Target::host("api.example.com").unwrap())
    );
}
