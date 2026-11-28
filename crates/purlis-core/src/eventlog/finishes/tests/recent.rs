// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! FS-C2 (#7902): the log's files indexed per file and per chat ([`Recent`]).

use std::path::Path;
use std::time::Duration;

use proptest::prelude::*;

use super::*;

/// **The fold as it read events before FS-C2**, kept as it was so the fold over records is held
/// to it: `finishes_among` of `eventlog/finishes.rs` at 8dcd5faf68.
fn before_c2(events: &[Event], chat: &str) -> Vec<Finish> {
    let mut out = Vec::new();
    let mut turn = Turn::default();
    let mut harness = crate::eventlog::UNKNOWN.to_owned();
    for event in events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
    {
        if event.kind == "run.started" {
            harness = event
                .body
                .get("harness")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(crate::eventlog::UNKNOWN)
                .to_owned();
        }
        let own = event.parent_run.is_none();
        let ended = |how, blocked, unresolved| Finish {
            seq: event.seq,
            ulid: event.ulid.clone(),
            at_ms: crate::eventlog::written_at_ms(&event.ulid),
            how,
            blocked,
            unresolved,
        };
        match event.kind.as_str() {
            "hook.userpromptsubmit" if own => turn = Turn::default(),
            "hook.stop" if own => {
                out.push(ended(How::Finished, turn.count(), turn.named()));
                turn = Turn::default();
            }
            "run.ended" if own => {
                if event.body.get("state").and_then(serde_json::Value::as_str) == Some("failed") {
                    out.push(ended(How::Failed, 0, Vec::new()));
                }
                turn = Turn::default();
            }
            crate::eventlog::stops::BLOCK => {
                if let Ok(block) = serde_json::from_value::<Block>(event.body.clone()) {
                    turn.blocked(block, &harness);
                }
            }
            crate::eventlog::stops::ASK_ANSWERED => {
                if let Ok(AskAnswered {
                    ask,
                    answer: Answer::Allowed,
                    ..
                }) = serde_json::from_value(event.body.clone())
                {
                    turn.allowed(ask);
                }
            }
            kind if came_back(kind) => {
                let word = |field: &str| event.body.get(field).and_then(serde_json::Value::as_str);
                if let (Some(tool), Some(args)) = (word("tool"), word("args")) {
                    turn.came_back(&crate::shown::readable(tool, TARGET_CHARS), args);
                }
            }
            _ => {}
        }
    }
    out
}

/// One thing written to the log, or done to it, between asks.
#[derive(Debug, Clone)]
enum Op {
    /// An event of `chat`'s (0 or 1), its own run's or, with `sub`, a sub-agent's.
    Write { chat: usize, sub: bool, what: What },
    /// The live file cut back to the end of its line before last.
    Shrink,
    /// The same, and the log opened again, so it reads where its live file ends now and grows it
    /// again without sealing it.
    ShrinkAndReopen,
    /// The records of chat 1 let go of, as a project's close does.
    Forget,
    /// Both chats asked, and the answer held to the whole log's.
    Ask,
}

#[derive(Debug, Clone, Copy)]
enum What {
    Started,
    Prompt,
    Stop,
    Block(bool, Source),
    CameBack(bool, bool),
    Answered(bool),
    Ended(bool),
}

fn what() -> impl Strategy<Value = What> {
    prop_oneof![
        1 => Just(What::Started),
        3 => Just(What::Prompt),
        3 => Just(What::Stop),
        4 => (any::<bool>(), prop_oneof![Just(Source::Guard), Just(Source::Sandbox)])
            .prop_map(|(push, source)| What::Block(push, source)),
        3 => (any::<bool>(), any::<bool>()).prop_map(|(push, failed)| What::CameBack(push, failed)),
        1 => any::<bool>().prop_map(What::Answered),
        1 => any::<bool>().prop_map(What::Ended),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        12 => (0..2usize, proptest::bool::weighted(0.25), what())
            .prop_map(|(chat, sub, what)| Op::Write { chat, sub, what }),
        1 => Just(Op::Shrink),
        1 => Just(Op::ShrinkAndReopen),
        1 => Just(Op::Forget),
        3 => Just(Op::Ask),
    ]
}

const CHATS: [&str; 2] = [CHAT, OTHER];

fn written(log: &mut Turns<'_>, sub: bool, what: What) {
    let parent = sub.then_some(RUN);
    let args = |push: bool| if push { PUSH } else { LIST };
    match what {
        What::Started => log.write(parent, "run.started", serde_json::json!({ "harness": if sub { "codex" } else { "claude" } })),
        What::Prompt => log.write(parent, "hook.userpromptsubmit", serde_json::json!({})),
        What::Stop => log.write(parent, "hook.stop", serde_json::json!({})),
        What::Block(push, source) => {
            let body = serde_json::to_value(Stop::Block(Block {
                tool: "Bash".to_owned(),
                source,
                qualifier: None,
                target: None,
                args: Some(args(push).to_owned()),
            }))
            .unwrap();
            log.write(parent, crate::eventlog::stops::BLOCK, body)
        }
        What::CameBack(push, failed) => log.write(
            parent,
            if failed {
                "hook.posttoolusefailure-blocked"
            } else {
                "hook.posttooluse-blocked"
            },
            serde_json::json!({ "decision": "allow", "hook_ms": 1, "tool": "Bash", "args": args(push) }),
        ),
        What::Answered(allowed) => {
            let body = serde_json::to_value(Stop::AskAnswered(AskAnswered {
                ask: AskGroup::Sandbox,
                answer: if allowed { Answer::Allowed } else { Answer::Refused },
                by: AnsweredBy::Window,
            }))
            .unwrap();
            log.write(parent, crate::eventlog::stops::ASK_ANSWERED, body)
        }
        What::Ended(failed) => log.write(
            parent,
            "run.ended",
            serde_json::json!({ "state": if failed { "failed" } else { "ended" } }),
        ),
    };
}

/// The live file cut back to the end of its line before last, where it has two lines.
fn shrunk(dir: &Path) {
    let path = live(dir);
    let Ok(text) = std::fs::read(&path) else {
        return;
    };
    let ends: Vec<usize> = text
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .map(|(at, _)| at)
        .collect();
    if ends.len() < 2 {
        return;
    }
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(ends[ends.len() - 2] as u64 + 1)
        .unwrap();
}

/// Runs `ops` on a log of small segments (so seals come mid-turn) and holds every ask of a
/// [`Recent`] keeping at most `cap` to [`finishes_in`], and the fold to [`before_c2`].
fn held_to_the_whole_log(ops: &[Op], cap: usize) -> Result<(), TestCaseError> {
    let dir = tempfile::tempdir().unwrap();
    let recent = Recent::with_cap(cap);
    // One writer, as the log allows: each event names its chat.
    let mut log = small(dir.path(), CHAT);
    for op in ops.iter().chain([&Op::Ask]) {
        match op {
            Op::Write { chat, sub, what } => {
                log.chat = CHATS[*chat];
                written(&mut log, *sub, *what);
            }
            Op::Shrink => shrunk(dir.path()),
            Op::ShrinkAndReopen => {
                let chat = log.chat;
                drop(log);
                shrunk(dir.path());
                log = small(dir.path(), chat);
            }
            Op::Forget => recent.forget(&[OTHER]),
            Op::Ask => {
                let said = recent.finishes(dir.path(), &CHATS, 0).unwrap();
                let events = crate::eventlog::read(dir.path()).unwrap();
                for chat in CHATS {
                    let whole = finishes(&events, chat);
                    prop_assert_eq!(&whole, &before_c2(&events, chat), "the fold, of {}", chat);
                    prop_assert_eq!(&said[chat], &whole, "{} after {:?}", chat, ops);
                }
                prop_assert!(recent.kept_bytes() <= cap, "{} kept", recent.kept_bytes());
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Whatever two chats and their sub-agents write between asks, sealed mid-turn, grown and cut
    /// back, an ask answers what the whole log says.
    #[test]
    fn recent_answers_what_the_whole_log_says_whatever_is_written_between_asks(
        ops in proptest::collection::vec(op(), 0..40),
    ) {
        held_to_the_whole_log(&ops, Recent::CAP)?;
    }

    /// The same, kept under a cap far below what the log holds.
    #[test]
    fn recent_kept_under_a_small_cap_answers_what_the_whole_log_says(
        ops in proptest::collection::vec(op(), 0..40),
    ) {
        held_to_the_whole_log(&ops, 600)?;
    }
}

#[test]
fn a_warm_ask_parses_no_line() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 4);
    sealed_turns(dir.path(), OTHER, 3);
    let recent = Recent::default();
    recent.finishes(dir.path(), &[CHAT, OTHER], 0).unwrap();
    let (parsed, lines) = (recent.parsed(), recent.lines());
    assert!(lines > 0, "the premise: the cold ask parsed the log");
    for chats in [&[CHAT, OTHER][..], &[CHAT], &[OTHER]] {
        recent.finishes(dir.path(), chats, 0).unwrap();
    }
    assert_eq!(
        (recent.parsed(), recent.lines()),
        (parsed, lines),
        "warm asks parsed {} lines",
        recent.lines() - lines
    );
}

/// A log whose chat `deep` began its run and turn in a sealed file outside the window, and whose
/// chat `shallow` writes only in the live file: an ask of `deep` reads older files, one of
/// `shallow` reads only the window.
fn deep_and_shallow(dir: &Path, deep: &str, shallow: &str) {
    small(dir, deep)
        .started("claude")
        .prompt()
        .block(PUSH, None);
    sealed_turns(dir, "01K6CHAT000000000000000006", 6);
    age_sealed(dir);
    small(dir, deep).stop();
    small(dir, shallow).started("claude").prompt().stop();
}

#[test]
fn asks_of_windows_of_different_depths_in_turn_parse_nothing_again() {
    // The epic's `retain` thrash: an ask kept only the files it read, so two projects whose
    // chats need different depths of the log made each other's asks parse again.
    let dir = tempfile::tempdir().unwrap();
    deep_and_shallow(dir.path(), CHAT, OTHER);
    let recent = Recent::default();
    let deep = recent.finishes(dir.path(), &[CHAT], a_day_ago()).unwrap();
    assert_eq!(deep[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert_eq!(
        deep[CHAT][0].blocked, 1,
        "the premise: the deep ask read its turn's start"
    );
    recent.finishes(dir.path(), &[OTHER], a_day_ago()).unwrap();
    let lines = recent.lines();
    for _ in 0..3 {
        recent.finishes(dir.path(), &[CHAT], a_day_ago()).unwrap();
        recent.finishes(dir.path(), &[OTHER], a_day_ago()).unwrap();
    }
    assert_eq!(recent.lines(), lines, "the asks in turn parsed again");
}

/// Runs `ask` on a thread of its own and answers what it answered, or fails when it has not
/// answered in a minute: no ask here needs a fraction of that, so only one waiting on a lock it
/// will never get gets there.
fn answers<T: Send + 'static>(ask: impl FnOnce() -> T + Send + 'static) -> T {
    let (told, answered) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = told.send(ask());
    });
    answered
        .recv_timeout(Duration::from_secs(60))
        .expect("the ask answered while the other file parsed")
}

#[test]
fn an_ask_of_one_projects_chat_answers_while_another_projects_older_file_parses() {
    // The epic: a finishedTurns ask stuck in the core for more than 260 s, behind one lock held
    // across every parse.
    let dir = tempfile::tempdir().unwrap();
    deep_and_shallow(dir.path(), OTHER, CHAT);
    let oldest = sealed_files(dir.path())[0].clone();
    let recent = std::sync::Arc::new(Recent::default());
    let (began, release) = recent.hold_parse_of(&oldest);
    let deep = {
        let (recent, dir) = (recent.clone(), dir.path().to_path_buf());
        std::thread::spawn(move || recent.finishes(&dir, &[OTHER], a_day_ago()).unwrap())
    };
    began
        .recv_timeout(Duration::from_secs(60))
        .expect("plane B's ask parses its oldest file");
    let shallow = answers({
        let (recent, dir) = (recent.clone(), dir.path().to_path_buf());
        move || recent.finishes(&dir, &[CHAT], a_day_ago()).unwrap()
    });
    assert_eq!(shallow[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert!(!deep.is_finished(), "the premise: B's parse is still held");
    release.send(()).unwrap();
    let deep = deep.join().unwrap();
    assert_eq!(deep[OTHER], finishes_in(dir.path(), OTHER).unwrap());
}

#[test]
fn asks_of_one_file_share_its_one_parse() {
    let dir = tempfile::tempdir().unwrap();
    let mut written = Turns::in_dir(dir.path(), CHAT);
    written.started("claude").prompt().block(PUSH, None).stop();
    let path = live(dir.path());
    let recent = std::sync::Arc::new(Recent::default());
    let (began, release) = recent.hold_parse_of(&path);
    let ask = || {
        let (recent, dir) = (recent.clone(), dir.path().to_path_buf());
        std::thread::spawn(move || recent.finishes(&dir, &[CHAT], 0).unwrap())
    };
    let first = ask();
    began
        .recv_timeout(Duration::from_secs(60))
        .expect("the first ask parses");
    let second = ask();
    // Until the second ask has come to the file's lock: nothing else lets it be late.
    while recent.waits_for(&path) < 2 && !second.is_finished() {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    let (first, second) = (first.join().unwrap(), second.join().unwrap());
    assert_eq!(first, second);
    assert_eq!(first[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert_eq!(recent.parsed(), 1, "two asks of one file parsed it twice");
}

#[test]
fn a_seal_during_an_ask_leaves_it_an_answer_of_the_log_as_it_was_at_some_moment() {
    let dir = tempfile::tempdir().unwrap();
    let mut written = small(dir.path(), CHAT);
    written.started("claude").prompt().block(PUSH, None);
    let before = crate::eventlog::read(dir.path()).unwrap().len();
    let recent = std::sync::Arc::new(Recent::default());
    let (began, release) = recent.hold_parse_of(&live(dir.path()));
    let asked = {
        let (recent, dir) = (recent.clone(), dir.path().to_path_buf());
        std::thread::spawn(move || recent.finishes(&dir, &[CHAT], 0).unwrap())
    };
    began
        .recv_timeout(Duration::from_secs(60))
        .expect("the ask reads the live file");
    let sealed = sealed_files(dir.path()).len();
    written
        .came_back(PUSH, false)
        .stop()
        .prompt()
        .block(LIST, None)
        .stop();
    assert!(
        sealed_files(dir.path()).len() > sealed,
        "the premise: the log sealed meanwhile"
    );
    release.send(()).unwrap();
    let said = asked.join().unwrap();
    let events = crate::eventlog::read(dir.path()).unwrap();
    assert!(
        (before..=events.len()).any(|end| finishes(&events[..end], CHAT) == said[CHAT]),
        "{:?} is no moment of the log's",
        said[CHAT]
    );
    let after = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(after[CHAT], finishes_in(dir.path(), CHAT).unwrap());
}

#[test]
fn a_closed_projects_chats_are_let_go_of_and_read_again_if_asked() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 3);
    sealed_turns(dir.path(), OTHER, 3);
    let recent = Recent::default();
    recent.finishes(dir.path(), &[CHAT, OTHER], 0).unwrap();
    let kept = recent.kept_bytes();
    recent.forget(&[OTHER]);
    assert!(recent.kept_bytes() < kept, "nothing was let go of");
    let lines = recent.lines();
    recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(recent.lines(), lines, "the chat still open is read again");
    let said = recent.finishes(dir.path(), &[OTHER], 0).unwrap();
    assert_eq!(said[OTHER], finishes_in(dir.path(), OTHER).unwrap());
    assert_eq!(said[OTHER].len(), 3);
}

#[test]
fn past_its_cap_the_files_asked_least_recently_are_let_go_of() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 8);
    let roomy = Recent::default();
    roomy.finishes(dir.path(), &[CHAT], 0).unwrap();
    let all = roomy.kept_bytes();
    let cap = all / 3;
    let recent = Recent::with_cap(cap);
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert!(
        recent.kept_bytes() <= cap,
        "{} kept past a cap of {cap}",
        recent.kept_bytes()
    );
    let oldest = sealed_files(dir.path())[0].clone();
    assert!(
        recent.kept().contains(&oldest),
        "the file asked last is kept"
    );
}

#[test]
fn a_line_torn_after_its_event_then_cut_and_written_over_is_never_in_an_answer() {
    // FS-C2 review (#8030): a last line with no end that parsed was kept, and the offset moved
    // past it; the log cut it back and wrote a longer line over it, and the next ask read from
    // the middle of that line, kept the event cut and missed the one written.
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let mut written = Turns::in_dir(dir.path(), CHAT);
    written.started("claude").prompt();
    drop(written);
    let path = live(dir.path());
    let last = crate::eventlog::read(dir.path()).unwrap().pop().unwrap();
    let torn = Event {
        seq: last.seq + 1,
        ulid: ulid::Ulid::generate().to_string(),
        kind: "hook.stop".to_owned(),
        body: serde_json::json!({}),
        ..last
    };
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    serde_json::to_writer(&mut file, &torn).unwrap();
    file.flush().unwrap();
    drop(file);
    let recent = Recent::default();
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert_eq!(said[CHAT].len(), 1, "the premise: the torn stop was read");
    let before = std::fs::metadata(&path).unwrap().len();
    // The log cuts the torn line back as it opens, and writes over it.
    Turns::in_dir(dir.path(), CHAT).block(&"x".repeat(400), None);
    assert!(
        std::fs::metadata(&path).unwrap().len() > before,
        "the premise: the line written over is longer"
    );
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert!(
        said[CHAT].is_empty(),
        "the stop cut back is still in the answer"
    );
}

#[test]
fn a_parse_in_flight_under_the_cap_is_shared_not_done_again() {
    // FS-C2 review (#8030): an entry was made as asked least recently of all, and the cap let go
    // of it first while its parse was held, freeing nothing; the next ask of its file made
    // another and parsed it again.
    let parsing = tempfile::tempdir().unwrap();
    Turns::in_dir(parsing.path(), CHAT)
        .started("claude")
        .prompt()
        .block(PUSH, None)
        .stop();
    let others = tempfile::tempdir().unwrap();
    sealed_turns(others.path(), OTHER, 4);
    let path = live(parsing.path());
    let recent = std::sync::Arc::new(Recent::with_cap(1));
    let (began, release) = recent.hold_parse_of(&path);
    let ask = || {
        let (recent, dir) = (recent.clone(), parsing.path().to_path_buf());
        std::thread::spawn(move || recent.finishes(&dir, &[CHAT], 0).unwrap())
    };
    let first = ask();
    began
        .recv_timeout(Duration::from_secs(60))
        .expect("the first ask parses");
    // Another log's files, past the cap, while the parse is held.
    recent.finishes(others.path(), &[OTHER], 0).unwrap();
    let second = ask();
    while recent.waits_for(&path) < 2 && !second.is_finished() {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    let (first, second) = (first.join().unwrap(), second.join().unwrap());
    assert_eq!(first, second);
    assert_eq!(first[CHAT], finishes_in(parsing.path(), CHAT).unwrap());
    assert_eq!(
        recent.parsed_of(&path),
        1,
        "the file in flight was parsed again"
    );
}

/// **A bench, not a gate** (#7902): fifty chats over about 100 MB of log, as the epic's slow log
/// had it. Run by hand:
/// `cargo test -p purlis-core --release --lib -- --ignored --nocapture fifty_chats`.
#[test]
#[ignore = "a bench, not a gate: run by hand with --ignored --nocapture"]
fn bench_fifty_chats_over_a_hundred_megabytes() {
    use std::io::Write;
    const CHAT_COUNT: usize = 50;
    const SEGMENT: usize = 16 << 20;
    const TOTAL: usize = 100 << 20;
    let dir = tempfile::tempdir().unwrap();
    let chats: Vec<String> = (0..CHAT_COUNT)
        .map(|n| format!("01K6CHAT0000000000000{n:05}"))
        .collect();
    let mut seq = 0u64;
    let mut written = 0usize;
    let mut segment: Vec<u8> = Vec::new();
    let mut first = 1u64;
    let mut last = 0u64;
    let mut line = |chat: &str, kind: &str, body: serde_json::Value, out: &mut Vec<u8>| {
        seq += 1;
        let event = Event {
            v: 1,
            device_id: DEVICE.to_owned(),
            seq,
            ulid: ulid::Ulid::from_parts(1_790_000_000_000 + seq, u128::from(seq)).to_string(),
            chat: Some(chat.to_owned()),
            run: Some(RUN.to_owned()),
            parent_run: None,
            kind: kind.to_owned(),
            body,
        };
        serde_json::to_writer(&mut *out, &event).unwrap();
        out.push(b'\n');
        seq
    };
    let padding = "x".repeat(300);
    let mut turn = 0usize;
    while written < TOTAL {
        let chat = &chats[turn % CHAT_COUNT];
        line(
            chat,
            "hook.userpromptsubmit",
            serde_json::json!({ "prompt": padding }),
            &mut segment,
        );
        line(
            chat,
            crate::eventlog::stops::BLOCK,
            serde_json::to_value(Stop::Block(Block {
                tool: "Bash".to_owned(),
                source: Source::Guard,
                qualifier: None,
                target: None,
                args: Some(PUSH.to_owned()),
            }))
            .unwrap(),
            &mut segment,
        );
        for _ in 0..6 {
            line(
                chat,
                "hook.posttooluse-blocked",
                serde_json::json!({ "tool": "Read", "args": LIST, "output": padding }),
                &mut segment,
            );
        }
        last = line(chat, "hook.stop", serde_json::json!({}), &mut segment);
        turn += 1;
        if segment.len() >= SEGMENT {
            written += segment.len();
            std::fs::write(dir.path().join(format!("events.{first}.jsonl")), &segment).unwrap();
            segment.clear();
            first = last + 1;
        }
    }
    written += segment.len();
    std::fs::File::create(live(dir.path()))
        .unwrap()
        .write_all(&segment)
        .unwrap();
    let asked: Vec<&str> = chats.iter().map(String::as_str).collect();
    let timed = |what: &str, run: &mut dyn FnMut()| {
        let start = std::time::Instant::now();
        run();
        println!("{what}: {:?}", start.elapsed());
    };
    println!("{} MB in {last} events, {turn} turns", written >> 20);
    timed(
        "before C2, one chat's finishes_in (each ask parsed this)",
        &mut || {
            finishes_in(dir.path(), &chats[0]).unwrap();
        },
    );
    let recent = Recent::default();
    timed("cold ask of 50 chats", &mut || {
        recent.finishes(dir.path(), &asked, 0).unwrap();
    });
    println!("kept: {} MB of records", recent.kept_bytes() >> 20);
    let lines = recent.lines();
    timed("warm ask of 50 chats", &mut || {
        recent.finishes(dir.path(), &asked, 0).unwrap();
    });
    timed("warm ask of one chat", &mut || {
        recent.finishes(dir.path(), &asked[..1], 0).unwrap();
    });
    assert_eq!(recent.lines(), lines, "warm asks parse nothing");
    let mut grown = std::fs::OpenOptions::new()
        .append(true)
        .open(live(dir.path()))
        .unwrap();
    let mut more = Vec::new();
    line(
        &chats[0],
        "hook.userpromptsubmit",
        serde_json::json!({}),
        &mut more,
    );
    line(&chats[0], "hook.stop", serde_json::json!({}), &mut more);
    grown.write_all(&more).unwrap();
    timed("an ask of 50 chats after a turn ended", &mut || {
        recent.finishes(dir.path(), &asked, 0).unwrap();
    });
    println!("lines parsed for it: {}", recent.lines() - lines);
}
