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
            by: crate::eventlog::stops::AnsweredBy::Window,
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

/// Every group of ask (R-229), in the order the log names them.
const GROUPS: [AskGroup; 7] = [
    AskGroup::Sandbox,
    AskGroup::HarnessPrompt,
    AskGroup::Consent,
    AskGroup::Hold,
    AskGroup::LoopPause,
    AskGroup::Budget,
    AskGroup::HookGiveUp,
];

/// R-229 (core): **raising an ask and answering it each write an event under the chat, and the
/// Chat log's rows have both, with the answer and who answered**. One ask of each group is
/// raised and answered, sent as the window sends a stop; the harness's permission prompt is
/// raised in and answered through the asks registry itself, so its answer is the one the
/// registry applied, from the human scope it came through. Allowed and refused, from the window
/// and from a shell, each read back as written; nothing of what an ask said is in the log.
#[test]
fn raising_and_answering_an_ask_writes_its_event_with_the_answer_and_who() {
    use crate::eventlog::Entry;
    use crate::eventlog::stops::{AnsweredBy, AskRaised};
    use crate::harness::asks::{Admitted, Answerer, Asks};
    use crate::harness::model::{Ask, Channel, Choice, ChoiceKind, ChoiceScope};

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let plane = Path::new(PLANE);
    let mut written = Log::open(log, DEVICE).unwrap();
    run_started(&mut written, ONE, RUN_ONE, None);
    drop(written);
    let mut host = recorder(log);
    let send = |host: &mut Recorder, stop: Stop| {
        // As the window sends it, over the wire.
        let entry = serde_json::to_value(Entry::Stop {
            plane: plane.to_path_buf(),
            number: Some(4),
            stop,
        })
        .unwrap();
        host.apply(serde_json::from_value(entry).unwrap()).unwrap();
    };

    // The registry's own ask: a permission prompt, answered Deny from the window.
    let asks = Asks::new();
    let now = std::time::Instant::now();
    let prompt = "Allow `rm -rf build` in the project?";
    let raised = asks
        .raise(
            "4",
            Ask {
                summary: crate::harness::model::Summary::of(prompt),
                options: vec![
                    Choice {
                        id: "allow".into(),
                        label: "Allow".into(),
                        kind: ChoiceKind::Allow,
                        scope: ChoiceScope::Once,
                    },
                    Choice {
                        id: "deny".into(),
                        label: "Deny".into(),
                        kind: ChoiceKind::Reject,
                        scope: ChoiceScope::Once,
                    },
                ],
                channel: Channel::Hook,
                ..Ask::default()
            },
            now,
        )
        .raised;
    let window = Answerer::admitted(Admitted::LocalUi).unwrap();
    let applied = asks.answer("4", &raised.id, "deny", window, now).unwrap();

    let mut expected = Vec::new();
    for (at, group) in GROUPS.into_iter().enumerate() {
        send(&mut host, Stop::AskRaised(AskRaised { ask: group }));
        let answered = match group {
            AskGroup::HarnessPrompt => AskAnswered::applied(group, &applied),
            _ => AskAnswered {
                ask: group,
                answer: if at % 2 == 0 {
                    Answer::Allowed
                } else {
                    Answer::Refused
                },
                by: if at % 3 == 0 {
                    AnsweredBy::Inbox
                } else {
                    AnsweredBy::Window
                },
            },
        };
        send(&mut host, Stop::AskAnswered(answered));
        expected.push((group, answered));
    }
    drop(host);
    assert_eq!(
        expected[1].1,
        AskAnswered {
            ask: AskGroup::HarnessPrompt,
            answer: Answer::Refused,
            by: AnsweredBy::Window,
        },
        "the registry's applied answer: Deny, from the window"
    );

    let events = crate::eventlog::read(log).unwrap();
    let asked: Vec<(AskGroup, AskAnswered)> = rows(&events, ONE)
        .into_iter()
        .filter(|row| !matches!(row, Row::Started { .. }))
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|pair| match pair {
            [
                Row::AskRaised { ask, .. },
                Row::AskAnswered { answered, .. },
            ] => (*ask, *answered),
            other => panic!("each ask raised, then its answer: {other:?}"),
        })
        .collect();
    assert_eq!(asked, expected);
    let story = timeline(&events, ONE);
    let kinds: Vec<&str> = story
        .iter()
        .map(|event| event.kind.as_str())
        .filter(|kind| kind.starts_with("ask."))
        .collect();
    assert_eq!(kinds.len(), 2 * GROUPS.len(), "{kinds:?}");
    let answered = story
        .iter()
        .find(|event| event.kind == stops::ASK_ANSWERED)
        .unwrap();
    assert_eq!(
        answered.body,
        serde_json::json!({"ask": "sandbox", "answer": "allowed", "by": "inbox"}),
        "closed words alone"
    );
    let everything = std::fs::read_to_string(log.join(FILE)).unwrap();
    assert!(
        !everything.contains("rm -rf"),
        "an ask's words are never written"
    );
}

/// A tool event of chat `chat`, as the recorder writes one: `hook.<word>` with its decision,
/// the call's id and, where given, its rule.
fn tool_event(
    log: &mut Log,
    chat: &str,
    word: &str,
    decision: &str,
    call: &str,
    rule: Option<&str>,
) {
    let mut body =
        serde_json::json!({"decision": decision, "hook_ms": 1, "tool": "Bash", "call": call});
    if let Some(rule) = rule {
        body["rule"] = rule.into();
    }
    log.append(
        Some(chat),
        Some(RUN_ONE),
        None,
        &format!("hook.{word}"),
        body,
    )
    .unwrap();
}

/// R-234 (core), Q261: **the Chat log's summary counts the chat's allowed calls, and of them
/// those allowed by the person's rules**, from its tool events' decision and rule: of 212 calls,
/// 100 purlis's hook allowed, 9 of them by an entry of the Allowed list, give 100 and 9. A call
/// its hook had no say on (`none`) is not counted as allowed, nor one refused or asked about; a
/// call's post hook is the same call, never a second; another chat's calls are its own.
#[test]
fn the_timeline_query_counts_allowed_calls_and_those_by_your_rules() {
    assert_eq!(
        super::super::phase("pretooluse"),
        Some(super::super::Phase::Pre)
    );
    assert_eq!(
        super::super::phase("posttooluse"),
        Some(super::super::Phase::Post)
    );
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    run_started(&mut log, ONE, RUN_ONE, None);
    for at in 0..212 {
        let call = format!("toolu_{at:04}");
        // 9 allowed by a rule of the person's; the rest let through with nothing said, or
        // allowed by the hook.
        let (decision, rule) = match at {
            0..9 => ("allow", Some("allowed-list")),
            9..100 => ("allow", None),
            _ => ("none", None),
        };
        tool_event(&mut log, ONE, "pretooluse", decision, &call, rule);
        tool_event(&mut log, ONE, "posttooluse", "none", &call, None);
    }
    tool_event(
        &mut log,
        ONE,
        "pretooluse",
        "deny",
        "toolu_deny",
        Some("A4"),
    );
    tool_event(&mut log, ONE, "pretooluse", "ask", "toolu_ask", None);
    tool_event(
        &mut log,
        TWO,
        "pretooluse",
        "allow",
        "toolu_two",
        Some("allowed-list"),
    );
    drop(log);
    let events = crate::eventlog::read(dir.path()).unwrap();

    let read = summary(&events, ONE);
    assert_eq!(
        (read.allowed, read.by_your_rules),
        (100, 9),
        "`allow` alone is allowed: `none` is a call the hook had no say on"
    );
    let two = summary(&events, TWO);
    assert_eq!((two.allowed, two.by_your_rules), (1, 1));
}

/// A chat's own `run.started`, by `harness` with the sandbox backend `sandbox`, at sandbox
/// level `level`.
fn started_as(log: &mut Log, chat: &str, harness: &str, sandbox: &str, level: &str) {
    let body = serde_json::json!({
        "cause": "start",
        "device": DEVICE,
        "persona": "steward",
        "harness": harness,
        "sandbox": sandbox,
        "protection": {
            "layers": [{"layer": "sandbox", "level": level, "tier": "person"}],
            "charter_commit": "none",
            "answers_in_force": [],
        },
    });
    log.append(Some(chat), Some(RUN_ONE), None, "run.started", body)
        .unwrap();
}

/// R-235 (core): **the Chat log's summary says whether the chat is unverified and whether it
/// ran as the person**, from its start event's harness and level: a Codex chat, whose hooks run
/// inside what its sandbox holds, reads unverified; a chat started at No sandbox (`off`) reads
/// ran as you; a Claude Code chat at the floor, its hooks outside its Seatbelt, reads neither.
#[test]
fn the_timeline_query_says_unverified_and_ran_as_you() {
    const THREE: &str = "01K6CHAT000000000000000006";
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    started_as(&mut log, ONE, "codex", "seatbelt", "floor");
    started_as(&mut log, TWO, "claude", "none", "off");
    started_as(&mut log, THREE, "claude", "seatbelt", "floor");
    drop(log);
    let events = crate::eventlog::read(dir.path()).unwrap();

    let flags = |chat| {
        let read = summary(&events, chat);
        (read.unverified, read.ran_as_you)
    };
    assert_eq!(flags(ONE), (true, false), "a Codex chat is unverified");
    assert!(flags(TWO).1, "a chat started at off ran as you");
    assert_eq!(
        flags(THREE),
        (false, false),
        "Claude Code at the floor: neither"
    );
}

/// R-228, R-231, R-232 (core): **a protection change and who made it, a hold and a loop pause
/// are each a row of the Chat log**, read from their events: the sandbox lifted by the person
/// and put back on by purlis, a held merge, and a pause for blocks on the same target. A grant
/// (`trust.sandbox.grant`) is no protection change: grants are phase 3's rows (R-133).
#[test]
fn a_protection_change_a_hold_and_a_loop_pause_are_rows() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path();
    let plane = Path::new(PLANE);
    let mut written = Log::open(log, DEVICE).unwrap();
    run_started(&mut written, ONE, RUN_ONE, None);
    written
        .append(
            Some(ONE),
            Some(RUN_ONE),
            None,
            "trust.sandbox.off",
            serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui"}),
        )
        .unwrap();
    written
        .append(
            Some(ONE),
            Some(RUN_ONE),
            None,
            "trust.sandbox.grant",
            serde_json::json!({"actor_kind": "human", "actor": "operator"}),
        )
        .unwrap();
    drop(written);
    let mut host = recorder(log);
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
    let hold = Hold {
        tool: "Bash".to_owned(),
        target: Some(Target::guard_arm(crate::mergehold::REASON).unwrap()),
        args: None,
    };
    host.stop(plane, Some(4), &Stop::Hold(hold.clone()))
        .unwrap();
    let pause = LoopPause {
        rule: LoopRule::Blocked,
        times: 3,
    };
    host.stop(plane, Some(4), &Stop::LoopPause(pause)).unwrap();
    drop(host);
    let events = crate::eventlog::read(log).unwrap();

    let drawn: Vec<Row> = rows(&events, ONE)
        .into_iter()
        .map(|row| match row {
            Row::ProtectionChanged {
                change,
                actor_kind,
                actor,
                ..
            } => Row::ProtectionChanged {
                at_ms: None,
                change,
                actor_kind,
                actor,
            },
            Row::Held {
                hold, sub_agent, ..
            } => Row::Held {
                at_ms: None,
                hold,
                sub_agent,
            },
            Row::LoopPaused { pause, .. } => Row::LoopPaused { at_ms: None, pause },
            other => other,
        })
        .filter(|row| !matches!(row, Row::Started { .. }))
        .collect();
    assert_eq!(
        drawn,
        [
            Row::ProtectionChanged {
                at_ms: None,
                change: "off".to_owned(),
                actor_kind: "human".to_owned(),
                actor: "operator".to_owned(),
            },
            Row::ProtectionChanged {
                at_ms: None,
                change: "on".to_owned(),
                actor_kind: "host".to_owned(),
                actor: "charter".to_owned(),
            },
            Row::Held {
                at_ms: None,
                hold: Hold { args: None, ..hold },
                sub_agent: false,
            },
            Row::LoopPaused { at_ms: None, pause },
        ]
    );
}

/// R-235 (core), Q261: **the honesty lines are taken over the whole chat**, never its latest
/// start alone: a chat that ran once unverified, or once with No sandbox, says so for good, a
/// later start under better protection notwithstanding; and a chat whose sandbox was lifted for
/// a run (`trust.sandbox.off`) ran as the person, whatever its start said.
#[test]
fn the_honesty_lines_are_taken_over_every_start_and_any_sandbox_lifted() {
    const THREE: &str = "01K6CHAT000000000000000006";
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    // A Codex start, whose hooks run within its reach, then a Claude Code one at the floor.
    started_as(&mut log, ONE, "codex", "seatbelt", "floor");
    started_as(&mut log, ONE, "claude", "seatbelt", "floor");
    // A start at No sandbox, then one at the floor.
    started_as(&mut log, TWO, "claude", "seatbelt", "off");
    started_as(&mut log, TWO, "claude", "seatbelt", "floor");
    // A start at the floor, then its sandbox lifted for a run.
    started_as(&mut log, THREE, "claude", "seatbelt", "floor");
    log.append(
        Some(THREE),
        Some(RUN_ONE),
        None,
        "trust.sandbox.off",
        serde_json::json!({"actor_kind": "human", "actor": "operator", "scope": "local-ui"}),
    )
    .unwrap();
    drop(log);
    let events = crate::eventlog::read(dir.path()).unwrap();

    let flags = |chat| {
        let read = summary(&events, chat);
        (read.unverified, read.ran_as_you)
    };
    assert_eq!(flags(ONE), (true, false), "unverified once is unverified");
    assert_eq!(flags(TWO), (false, true), "No sandbox once ran as you");
    assert_eq!(
        flags(THREE),
        (false, true),
        "a sandbox lifted for a run ran as you"
    );
}

/// R-235 (core), Q261, Q235: **a chat with no start in the log reads unverified**: verified is
/// an allowlist, and nothing says how a chat with no start ran.
#[test]
fn a_chat_with_no_start_in_the_log_reads_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    tool_event(&mut log, ONE, "pretooluse", "allow", "toolu_1", None);
    drop(log);
    let events = crate::eventlog::read(dir.path()).unwrap();

    let read = summary(&events, ONE);
    assert!(read.unverified, "no start: nothing vouches for its reports");
    assert!(!read.ran_as_you, "nothing says it ran with No sandbox");
}

/// R-231 (core), Q261: **a sub-agent's stop row is marked as the sub-agent's**: a hold written
/// under a run with a parent is still a row of the chat's, and says it is its sub-agent's; one
/// under the chat's own run says nothing of the kind.
#[test]
fn a_sub_agents_stop_row_is_marked_as_the_sub_agents() {
    const CHILD: &str = "01K6RUN0000000000000000009";
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::open(dir.path(), DEVICE).unwrap();
    run_started(&mut log, ONE, RUN_ONE, None);
    let hold = serde_json::json!({"tool": "Bash", "target": {"guard_arm": "merge-hold"}});
    log.append(Some(ONE), Some(RUN_ONE), None, stops::HOLD, hold.clone())
        .unwrap();
    log.append(Some(ONE), Some(CHILD), Some(RUN_ONE), stops::HOLD, hold)
        .unwrap();
    drop(log);
    let events = crate::eventlog::read(dir.path()).unwrap();

    let marks: Vec<bool> = rows(&events, ONE)
        .into_iter()
        .filter_map(|row| match row {
            Row::Held { sub_agent, .. } => Some(sub_agent),
            _ => None,
        })
        .collect();
    assert_eq!(
        marks,
        [false, true],
        "the chat's own hold, then its sub-agent's"
    );
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
