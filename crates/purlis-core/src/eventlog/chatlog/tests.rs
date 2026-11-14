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
const RUN_ONE: &str = "01K6RUN0000000000000000004";
const RUN_TWO: &str = "01K6RUN0000000000000000005";

fn recorder(dir: &Path) -> Recorder {
    let mut host = Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap());
    let plane = Path::new(PLANE);
    host.knows(
        plane,
        4,
        RunOf {
            chat: ONE,
            run: RUN_ONE,
        },
    );
    host.knows(
        plane,
        5,
        RunOf {
            chat: TWO,
            run: RUN_TWO,
        },
    );
    host
}

/// A `run.started` line in the shape T1-06 reads leniently (Q238): every attribute, and
/// `protection` as `protection` says, or with no `protection` field at all.
fn run_started(log: &mut Log, chat: &str, run: &str, protection: Option<&str>) {
    let mut body = serde_json::json!({
        "cause": "start",
        "device": DEVICE,
        "persona": "steward",
        "harness": "claude",
        "profile": "unknown",
        "model_source": "unknown",
        "provider": "unknown",
        "model": "unknown",
        "sandbox": "seatbelt",
        "conversation": "unknown",
        "started_by": "local-ui",
    });
    if let Some(protection) = protection {
        body["protection"] = protection.into();
    }
    log.append(Some(chat), Some(run), None, "run.started", body)
        .unwrap();
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

    // The fixture log: each chat's run starting, in T1-06's lenient shape (chat one's with no
    // `protection` field, chat two's with `"unknown"`); chat one's three blocks (one
    // unverified, one can't tell) and a hold, with a commit its hook refused, and its sandbox
    // put back on; chat two's loop pause and block, and its run ending.
    let mut written = Log::open(log, DEVICE).unwrap();
    run_started(&mut written, ONE, RUN_ONE, None);
    run_started(&mut written, TWO, RUN_TWO, Some("unknown"));
    drop(written);
    let mut host = recorder(log);
    host.stop(plane, Some(4), &block(None)).unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::Unverified)))
        .unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::CantTell)))
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
    host.trust(
        RunOf {
            chat: ONE,
            run: RUN_ONE,
        },
        &crate::sandbox::Change::On,
        None,
        Some("steward"),
    )
    .unwrap();
    drop(host);
    let mut written = Log::open(log, DEVICE).unwrap();
    written
        .append(
            Some(TWO),
            Some(RUN_TWO),
            None,
            "run.ended",
            serde_json::json!({"state": "completed", "cause": "exited"}),
        )
        .unwrap();
    drop(written);

    let one = timeline_in(log, ONE).unwrap();
    assert_eq!(
        kinds(&one),
        [
            "run.started",
            "stop.block",
            "stop.block",
            "stop.block",
            "stop.hold",
            "trust.sandbox.on"
        ],
        "chat one's story, oldest first, and nothing of chat two's"
    );
    assert!(one.iter().all(|event| event.chat.as_deref() == Some(ONE)));
    assert_eq!(
        kinds(&timeline_in(log, TWO).unwrap()),
        ["run.started", "stop.loop_pause", "stop.block", "run.ended"]
    );
    let counted = counts_in(log).unwrap();
    let tally_one = Tally {
        blocks: 1,
        unverified_blocks: 1,
        cant_tell_blocks: 1,
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
            cant_tell_blocks: 1,
            unreadable_blocks: 0,
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
        [
            "run.started",
            "stop.block",
            "stop.block",
            "stop.block",
            "trust.sandbox.on"
        ]
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
        [
            "run.started",
            "stop.block",
            "stop.block",
            "stop.block",
            "trust.sandbox.on",
            "ask.answered"
        ]
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

/// A `stop.block` whose body does not read back as a block is in the chat's timeline and is
/// counted in a bucket of its own: neither a plain block nor one reported unverified or can't
/// tell, since nothing says which it was.
#[test]
fn a_block_whose_body_does_not_read_back_is_counted_as_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let mut written = Log::open(log, DEVICE).unwrap();
    written
        .append(
            Some(ONE),
            Some(RUN_ONE),
            None,
            stops::BLOCK,
            serde_json::json!({"tool": 7, "source": "no such source"}),
        )
        .unwrap();
    drop(written);

    assert_eq!(kinds(&timeline_in(log, ONE).unwrap()), ["stop.block"]);
    let counted = counts_in(log).unwrap();
    let unreadable = Tally {
        unreadable_blocks: 1,
        ..Tally::default()
    };
    assert_eq!(counted.per_chat.get(ONE), Some(&unreadable));
    assert_eq!(counted.all, unreadable);
}

/// R-233 (core): **a chat's end and its result reach its timeline as one event of the log**.
/// Each fixture chat starts, and its program ends through the host's own end path: chat one's
/// exits 0 and is done, chat two's exits 3 and has failed. Its timeline ends with one
/// `run.ended` of the chat's own run, saying the result.
#[test]
fn a_chats_end_and_result_are_in_its_timeline_query() {
    use crate::hookchannel::{how_its_run_ended, its_run_ends};
    use crate::session::Exit;

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let plane = Path::new(PLANE);
    let mut written = Log::open(log, DEVICE).unwrap();
    for (chat, run) in [(ONE, RUN_ONE), (TWO, RUN_TWO)] {
        run_started(&mut written, chat, run, None);
    }
    drop(written);
    let host = std::sync::Mutex::new(recorder(log));
    for (number, exit) in [(4, Exit::Code(0)), (5, Exit::Code(3))] {
        let (state, cause) = how_its_run_ended(None, &exit);
        its_run_ends(&host, plane, number, state, cause);
    }
    drop(host);

    for (chat, run, state, code) in [(ONE, RUN_ONE, "completed", 0), (TWO, RUN_TWO, "failed", 3)] {
        let timeline = timeline_in(log, chat).unwrap();
        let ends: Vec<&Event> = timeline
            .iter()
            .filter(|event| event.kind == "run.ended" && event.parent_run.is_none())
            .collect();
        assert_eq!(ends.len(), 1, "one end of the chat's own run: {timeline:?}");
        let end = timeline.last().unwrap();
        assert_eq!(
            end.kind, "run.ended",
            "the end is the timeline's last event"
        );
        assert_eq!(end.run.as_deref(), Some(run));
        assert_eq!(end.parent_run, None);
        assert_eq!(
            end.body,
            serde_json::json!({"state": state, "cause": "exited", "code": code}),
        );
    }
}

const CHILD: &str = "01K6RUN0000000000000000009";
const CLEARED: &str = "01K6RUN000000000000000000A";

/// R-227 (row) and R-233, and how a child run shows (T1-03's review): **the Chat log's start
/// row is built from the start event, and its end row from the end event**. The start row
/// carries the four fields the event recorded, read leniently: an event with no `protection`,
/// or `"unknown"`, gives a row with none. A sub-agent's run starting and ending, and a
/// `/clear`'s run, which copy what the chat started under, are no start or end row: the
/// Chat log is the chat's story, and its sub-agents' stops are still its rows (T1-28).
#[test]
fn the_start_and_end_rows_are_built_from_the_start_and_end_events() {
    use crate::sandbox::protection::{Layer, Tier};

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let mut written = Log::open(log, DEVICE).unwrap();
    let protection = serde_json::json!({
        "layers": [{"layer": "sandbox", "level": "floor", "tier": "project"}],
        "charter_commit": "4f2a9c1",
        "charter_dirty": true,
        "answers_in_force": [],
    });
    let mut started = serde_json::json!({"cause": "start", "persona": "steward"});
    started["protection"] = protection;
    written
        .append(Some(ONE), Some(RUN_ONE), None, "run.started", started)
        .unwrap();
    written
        .append(
            Some(ONE),
            Some(CHILD),
            Some(RUN_ONE),
            "run.started",
            serde_json::json!({"cause": "child"}),
        )
        .unwrap();
    written
        .append(
            Some(ONE),
            Some(CHILD),
            Some(RUN_ONE),
            "run.ended",
            serde_json::json!({"state": "completed", "cause": "child-ended"}),
        )
        .unwrap();
    written
        .append(
            Some(ONE),
            Some(CLEARED),
            None,
            "run.started",
            serde_json::json!({"cause": "clear"}),
        )
        .unwrap();
    written
        .append(
            Some(ONE),
            Some(CLEARED),
            None,
            "run.ended",
            serde_json::json!({"state": "failed", "cause": "exited", "code": 3}),
        )
        .unwrap();
    run_started(&mut written, TWO, RUN_TWO, Some("unknown"));
    drop(written);
    let events = crate::eventlog::read(log).unwrap();

    let drawn = rows(&events, ONE);
    assert_eq!(
        drawn.len(),
        2,
        "a start and an end, and no child's: {drawn:?}"
    );
    let Row::Started {
        cause, protection, ..
    } = &drawn[0]
    else {
        panic!("the first row is the start: {drawn:?}");
    };
    assert_eq!(cause, "start");
    assert_eq!(
        protection.as_ref(),
        Some(&InForce {
            layers: vec![Layer {
                layer: "sandbox".to_owned(),
                level: "floor".to_owned(),
                tier: Tier::Project,
            }],
            charter_commit: "4f2a9c1".to_owned(),
            charter_dirty: true,
            answers_in_force: Vec::new(),
        })
    );
    let Row::Ended {
        state, cause, code, ..
    } = &drawn[1]
    else {
        panic!("the last row is the end: {drawn:?}");
    };
    assert_eq!(
        (state.as_str(), cause.as_str(), *code),
        ("failed", "exited", Some(3))
    );

    assert!(matches!(
        rows(&events, TWO).as_slice(),
        [Row::Started {
            protection: None,
            ..
        }]
    ));
}

/// R-174 (core): **the blocked marker's count is the verified blocks after the last look**,
/// from a running tally of the log fed through its subscription ([`super::super::subscribe`]),
/// never a read of the whole log on each refresh. Chat one is blocked plainly and unverified
/// before the look and after it; chat two's block is its own. Each look is the latest block's
/// id the marker was drawn from, and a look at the latest block sets the count to 0.
#[test]
fn the_marker_count_is_the_verified_blocks_after_the_last_look() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let plane = Path::new(PLANE);
    let mut host = recorder(log);
    let mut tally = Blocks::default();
    let mut feed = super::super::subscribe(log, 0);
    let mut fed = |tally: &mut Blocks| {
        for delivery in feed.poll().unwrap() {
            tally.take(delivery);
        }
    };

    // Before any look: every block of the chat counts, the unverified one apart.
    host.stop(plane, Some(4), &block(None)).unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::Unverified)))
        .unwrap();
    host.stop(plane, Some(5), &block(None)).unwrap();
    fed(&mut tally);
    let before = tally.since(ONE, None);
    assert_eq!((before.blocks, before.unverified), (1, 1), "{before:?}");
    let looked = before.latest.expect("the latest block's seq");

    // A look at the latest block: nothing is new.
    let now = tally.since(ONE, Some(looked));
    assert_eq!((now.blocks, now.unverified), (0, 0), "{now:?}");
    assert_eq!(now.latest, Some(looked));

    // Later blocks count, verified apart from unverified; a can't-tell one and a hold do not.
    host.stop(plane, Some(4), &block(None)).unwrap();
    host.stop(plane, Some(4), &block(None)).unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::Unverified)))
        .unwrap();
    host.stop(plane, Some(4), &block(Some(Qualifier::CantTell)))
        .unwrap();
    host.stop(
        plane,
        Some(4),
        &Stop::Hold(Hold {
            tool: "Bash".to_owned(),
            target: None,
            args: None,
        }),
    )
    .unwrap();
    fed(&mut tally);
    let after = tally.since(ONE, Some(looked));
    assert_eq!((after.blocks, after.unverified), (2, 1), "{after:?}");
    // It is the fleet counts' own bucket (T1-03): the sidebar's count is `Tally.blocks` alone.
    assert_eq!(after.tally.blocks, after.blocks);
    assert_eq!(after.tally.cant_tell_blocks, 1);
    assert_eq!(after.tally.holds, 0, "a hold is no block");

    // Another chat's blocks are its own.
    let two = tally.since(TWO, None);
    assert_eq!((two.blocks, two.unverified), (1, 0), "{two:?}");

    // A new look sets it to 0.
    let again = after.latest.expect("the latest block's seq");
    let cleared = tally.since(ONE, Some(again));
    assert_eq!((cleared.blocks, cleared.unverified), (0, 0), "{cleared:?}");

    // A chat with no block has nothing to show, and no point to look at.
    assert_eq!(
        tally.since("01K6CHAT000000000000000009", None),
        Marker::default()
    );
}

/// **Q272 (T1-33): a block row says whether it has since been resolved**, so its row offers
/// Tell the chat… only while it stands: a later call on the same target that came back
/// resolves it, a failed retry or another call's success does not, and a sandbox block is also
/// resolved by an Allow of a sandbox ask, as a turn's finish reads it (Q263).
#[test]
fn a_block_row_is_resolved_once_a_later_call_on_its_target_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    let block_of = |source: Source, args: &str| {
        serde_json::to_value(Stop::Block(Block {
            tool: "Bash".to_owned(),
            source,
            qualifier: None,
            target: None,
            args: Some(args.to_owned()),
        }))
        .unwrap()
    };
    let came_back = |args: &str| {
        serde_json::json!({ "decision": "allow", "hook_ms": 1, "tool": "Bash", "args": args })
    };
    let mut write = |kind: &str, body: serde_json::Value| {
        log.append(Some(ONE), Some(RUN_ONE), None, kind, body)
            .unwrap();
    };
    write(stops::BLOCK, block_of(Source::Guard, "digest-of-push"));
    write(stops::BLOCK, block_of(Source::Guard, "digest-of-ls"));
    write(stops::BLOCK, block_of(Source::Guard, "digest-of-cat"));
    write(stops::BLOCK, block_of(Source::Sandbox, "digest-of-curl"));
    // Its call came back: resolved.
    write(
        "hook.posttooluse-blocked",
        came_back("digest-of-push"),
    );
    // A failed retry is no call that came back.
    write(
        "hook.posttoolusefailure-blocked",
        came_back("digest-of-ls"),
    );
    // Another call's success resolves nothing of these.
    write(
        "hook.posttooluse-blocked",
        came_back("digest-of-other"),
    );
    // A sandbox ask allowed resolves the sandbox block, and no guard's.
    write(
        stops::ASK_ANSWERED,
        serde_json::to_value(Stop::AskAnswered(AskAnswered {
            ask: AskGroup::Sandbox,
            answer: Answer::Allowed,
        }))
        .unwrap(),
    );
    let events = super::super::read(dir.path()).unwrap();
    let resolved: Vec<bool> = rows(&events, ONE)
        .into_iter()
        .filter_map(|row| match row {
            Row::Blocked { resolved, .. } => Some(resolved),
            _ => None,
        })
        .collect();
    assert_eq!(resolved, vec![true, false, false, true]);
}
