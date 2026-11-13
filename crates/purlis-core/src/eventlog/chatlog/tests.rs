// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::dispatchrecord::{self, Asker, ChatRef, Mode, Opening, Place, Worker};
use crate::eventlog::stops::{
    Answer, AskAnswered, AskGroup, Block, Hold, LoopPause, LoopRule, Qualifier, Source, Stop,
    Target,
};
use crate::eventlog::{ArgsKey, FILE, Log, Recorder, RunOf};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";
const ONE: &str = "01K6CHAT000000000000000004";
const TWO: &str = "01K6CHAT000000000000000005";

fn recorder(dir: &Path) -> Recorder {
    let mut host = Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap());
    let plane = Path::new(PLANE);
    host.knows(
        plane,
        4,
        RunOf {
            chat: ONE,
            run: "01K6RUN0000000000000000004",
        },
    );
    host.knows(
        plane,
        5,
        RunOf {
            chat: TWO,
            run: "01K6RUN0000000000000000005",
        },
    );
    host
}

fn block(qualifier: Option<Qualifier>) -> Stop {
    Stop::Block(Block {
        tool: "Bash".to_owned(),
        source: Source::Guard,
        qualifier,
        target: Some(Target::guard_arm("A4").unwrap()),
        args: Some(crate::eventlog::args_hash(
            &serde_json::json!({"command": "git push --force"}),
        )),
    })
}

/// Every file under `dir` with its bytes, but the live segment's.
fn files_but_the_log(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path != dir.join(FILE) {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|event| event.kind.as_str()).collect()
}

fn the_activity(root: &Path) -> crate::activity::Timeline {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-08T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    crate::activity::timeline(root, &chat_ref(3, "steward 3"), now)
}

fn chat_ref(number: u32, name: &str) -> ChatRef {
    ChatRef {
        chat: number,
        id: Some(format!("01K6CHAT{number:018}")),
        name: name.to_owned(),
        persona: None,
    }
}

/// R-223: a chat's timeline and the fleet counts are queries of the event log, through its
/// read API: removing an event from a fixture log removes it from both, adding one of a kind
/// the log did not hold adds it to both, with no write but its own line; and the shipped
/// Activity view, read from the dispatch records, is the same, with none of a chat's words in
/// the event log.
#[test]
fn a_chats_timeline_and_the_fleet_counts_are_queries_of_the_event_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let plane = Path::new(PLANE);

    // The Activity view's fixture: dispatch records, with the chats' own words.
    let project = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(project.path()).unwrap();
    let brief = "rebase the branch onto main and say what moved";
    let note = "halfway: the rebase stopped on src/lib.rs";
    let record = dispatchrecord::open(
        &root,
        Opening {
            mode: Mode::Task,
            asker: Asker {
                chat: chat_ref(3, "steward 3"),
                ..Asker::default()
            },
            persona: None,
            worker: Worker {
                chat: chat_ref(4, "implementer 4"),
                ..Worker::default()
            },
            task: Some("rebase".to_owned()),
            place: Place::default(),
            brief: brief.to_owned(),
            report_owed: true,
        },
        chrono::DateTime::parse_from_rfc3339("2026-10-08T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .unwrap();
    dispatchrecord::said(
        &root,
        &record.id,
        crate::dispatchtalk::Kind::Note,
        note,
        chrono::DateTime::parse_from_rfc3339("2026-10-08T10:05:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .unwrap();
    let activity_before = the_activity(&root);
    assert_eq!(activity_before.lines.len(), 2, "{activity_before:?}");

    // The fixture log: chat one's two blocks (one unverified) and a hold, with a commit its
    // hook refused, and chat two's loop pause and block.
    let mut host = recorder(log);
    host.stop(plane, Some(4), &block(None)).unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::Unverified)))
        .unwrap();
    host.stop(
        plane,
        Some(4),
        &Stop::Hold(Hold {
            tool: "Bash".to_owned(),
            target: Some(Target::service("github.com").unwrap()),
            args: None,
        }),
    )
    .unwrap();
    host.refused(plane, 4).unwrap();
    host.stop(
        plane,
        Some(5),
        &Stop::LoopPause(LoopPause {
            rule: LoopRule::Failing,
            times: 6,
        }),
    )
    .unwrap();
    host.stop(plane, Some(5), &block(None)).unwrap();
    drop(host);

    let one = timeline_in(log, ONE).unwrap();
    assert_eq!(
        kinds(&one),
        ["stop.block", "stop.block", "stop.hold"],
        "chat one's story, oldest first, and nothing of chat two's"
    );
    assert!(one.iter().all(|event| event.chat.as_deref() == Some(ONE)));
    assert_eq!(
        kinds(&timeline_in(log, TWO).unwrap()),
        ["stop.loop_pause", "stop.block"]
    );
    let counted = counts_in(log).unwrap();
    let tally_one = Tally {
        blocks: 1,
        unverified_blocks: 1,
        holds: 1,
        ..Tally::default()
    };
    let tally_two = Tally {
        blocks: 1,
        loop_pauses: 1,
        ..Tally::default()
    };
    assert_eq!(counted.per_chat.get(ONE), Some(&tally_one));
    assert_eq!(counted.per_chat.get(TWO), Some(&tally_two));
    assert_eq!(
        counted.all,
        Tally {
            blocks: 2,
            unverified_blocks: 1,
            holds: 1,
            loop_pauses: 1,
            asks_answered: 0,
        }
    );

    // Removing an event from the fixture log removes it from both: chat one's hold.
    let text = std::fs::read_to_string(log.join(FILE)).unwrap();
    let without: String = text
        .lines()
        .filter(|line| !line.contains("\"stop.hold\""))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(without.lines().count() + 1, text.lines().count());
    std::fs::write(log.join(FILE), &without).unwrap();
    assert_eq!(
        kinds(&timeline_in(log, ONE).unwrap()),
        ["stop.block", "stop.block"]
    );
    let counted = counts_in(log).unwrap();
    assert_eq!(counted.per_chat.get(ONE).unwrap().holds, 0);
    assert_eq!(counted.all.holds, 0);

    // Adding one of a kind the log did not hold adds it to both, and nothing else is written:
    // the recorder's own line is the one change, and the queries write nothing at all.
    let mut host = recorder(log);
    let others = files_but_the_log(log);
    let lines = std::fs::read_to_string(log.join(FILE))
        .unwrap()
        .lines()
        .count();
    host.stop(
        plane,
        Some(4),
        &Stop::AskAnswered(AskAnswered {
            ask: AskGroup::LoopPause,
            answer: Answer::Allowed,
        }),
    )
    .unwrap();
    let after_write = std::fs::read(log.join(FILE)).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&after_write).lines().count(),
        lines + 1
    );
    assert_eq!(
        kinds(&timeline_in(log, ONE).unwrap()),
        ["stop.block", "stop.block", "ask.answered"]
    );
    let counted = counts_in(log).unwrap();
    assert_eq!(counted.per_chat.get(ONE).unwrap().asks_answered, 1);
    assert_eq!(counted.all.asks_answered, 1);
    assert_eq!(counted.per_chat.get(TWO), Some(&tally_two));
    assert_eq!(files_but_the_log(log), others, "no second write path");
    assert_eq!(std::fs::read(log.join(FILE)).unwrap(), after_write);

    // The Activity view is unchanged, and the event log holds none of the chats' words.
    assert_eq!(the_activity(&root), activity_before);
    let everything = String::from_utf8_lossy(&after_write).into_owned();
    for words in [brief, note, "git push --force"] {
        assert!(!everything.contains(words), "{words:?} is in the event log");
    }
}
