// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A hook's line is taken by the host or spooled, and the hook knows which before it answers
//! (FD-30, #667).

use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn call(chat: u32, id: &str) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "posttooluse".to_owned(),
        tool: Some("Read".to_owned()),
        call: Some(id.to_owned()),
        args: None,
        decision: Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

fn hearing(tool: Tooled) -> Hearing {
    Hearing {
        serve: crate::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| crate::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| Answer::No { why: String::new() }),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool,
        permission: Box::new(|_| None),
    }
}

fn spooled_calls(socket: &std::path::Path) -> Vec<String> {
    let mut ids = Vec::new();
    spool::drain(&spool::dir_for(socket), &mut |item| {
        if let spool::Drained::Line {
            line: spool::Spooled::Tool(call),
            ..
        } = item
        {
            ids.push(call.call.unwrap_or_default());
        }
        Ok(())
    })
    .expect("the spool drains");
    ids
}

#[test]
fn a_line_the_host_takes_is_answered_only_once_its_hearer_has_recorded_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(1).expect("a token");
    let recorded = Arc::new(AtomicBool::new(false));
    let _reading = listener.hear(hearing({
        let recorded = Arc::clone(&recorded);
        Box::new(move |_| {
            std::thread::sleep(Duration::from_millis(60));
            recorded.store(true, Ordering::SeqCst);
            Ok(())
        })
    }));

    let delivered = deliver_tool(&path, Some(&token), &call(1, "a")).expect("delivered");

    assert_eq!(delivered, Delivered::Taken);
    assert!(
        recorded.load(Ordering::SeqCst),
        "the host said it took the line before it had recorded it"
    );
    assert_eq!(spooled_calls(&path), Vec::<String>::new());
}

#[test]
fn a_line_no_host_takes_is_spooled_under_the_next_number() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let token = {
        // A host issued the token and has gone, as an app that quit has.
        let listener = Listener::bind(dir.path(), &path).expect("a socket");
        listener.tokens().issue_to_this_process(2).expect("a token")
    };

    let first = deliver_tool(&path, Some(&token), &call(2, "a")).expect("spooled");
    let second = deliver_tool(&path, Some(&token), &call(2, "b")).expect("spooled");

    assert_eq!(
        (first, second),
        (Delivered::Spooled(1), Delivered::Spooled(2))
    );
    assert_eq!(spooled_calls(&path), ["a", "b"]);
}

#[test]
fn a_host_that_does_not_say_it_took_the_line_in_time_has_it_spooled() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(3).expect("a token");
    let _reading = listener.hear(hearing(Box::new(|_| {
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    })));

    let began = std::time::Instant::now();
    let delivered = deliver_tool(&path, Some(&token), &call(3, "slow")).expect("spooled");

    assert_eq!(delivered, Delivered::Spooled(1));
    assert!(
        began.elapsed() < Duration::from_millis(900),
        "{:?}",
        began.elapsed()
    );
}

#[test]
fn a_line_the_host_could_not_record_is_spooled() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(6).expect("a token");
    // The event log's disk is full.
    let _reading = listener.hear(hearing(Box::new(|_| {
        Err(std::io::Error::other("no space left on the device"))
    })));

    let delivered = deliver_tool(&path, Some(&token), &call(6, "full")).expect("spooled");

    assert_eq!(delivered, Delivered::Spooled(1));
    assert_eq!(spooled_calls(&path), ["full"]);
}

#[test]
fn a_line_no_host_takes_outside_the_sandboxs_denial_is_lost_and_says_so() {
    let dir = tempfile::tempdir().expect("a directory");
    // A fallback channel: a socket beside no project's `.charter/app/`.
    let path = dir
        .path()
        .join("charter-op-0123456789abcdef")
        .join("hooks.sock");
    let token = {
        let listener = Listener::bind(dir.path(), &path).expect("a socket");
        listener.tokens().issue_to_this_process(7).expect("a token")
    };

    let lost = deliver_tool(&path, Some(&token), &call(7, "a"));

    assert!(lost.is_err(), "{lost:?}");
    for spooled in ["7", "7.jsonl"] {
        assert!(!spool::dir_for(&path).join(spooled).exists(), "{spooled}");
    }
}

#[test]
fn a_line_with_no_token_and_no_host_is_lost_and_says_so() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");

    assert!(deliver_tool(&path, None, &call(4, "a")).is_err());
}

#[test]
fn a_line_its_spool_gave_up_waiting_on_may_be_lost_and_any_other_is_lost() {
    let away = io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused");
    let slow = io::Error::new(
        io::ErrorKind::TimedOut,
        "this chat's spool did not take the line within 1000 ms",
    );
    let full = io::Error::other("no space left on the device");

    let may_be = not_spooled(&away, slow);
    let is = not_spooled(&away, full);

    // The write goes on after the hook stops waiting, so the next open may still record it.
    assert_eq!(may_be.kind(), io::ErrorKind::TimedOut);
    assert!(
        may_be.to_string().ends_with("so it may be lost"),
        "{may_be}"
    );
    assert!(may_be.to_string().contains("within 1000 ms"), "{may_be}");
    assert!(is.to_string().ends_with("so it is lost"), "{is}");
}

/// The first acceptance line of #667: a host restart during a busy turn loses no event.
#[test]
fn a_host_restart_during_a_busy_turn_loses_no_event() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(5).expect("a token");
    let heard = Arc::new(Mutex::new(Vec::new()));
    let reading = listener.hear(hearing({
        let heard = Arc::clone(&heard);
        Box::new(move |call: ToolCall| {
            heard.lock().unwrap().push(call.call.unwrap_or_default());
            Ok(())
        })
    }));
    let reading = Mutex::new(Some(reading));

    // Eight hooks at a time, two hundred calls, and the host goes away a third of the way in.
    let next = AtomicUsize::new(0);
    let delivered = Mutex::new(Vec::new());
    let lost = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::SeqCst);
                    if n >= 200 {
                        return;
                    }
                    if n == 70 {
                        drop(reading.lock().unwrap().take());
                    }
                    let id = format!("call-{n}");
                    match deliver_tool(&path, Some(&token), &call(5, &id)) {
                        Ok(_) => delivered.lock().unwrap().push(id),
                        Err(why) => lost.lock().unwrap().push(format!("{id}: {why}")),
                    }
                }
            });
        }
    });

    // The next host drains what the hooks spooled while there was none.
    let mut recorded = heard.lock().unwrap().clone();
    recorded.extend(spooled_calls(&path));
    let delivered = delivered.into_inner().unwrap();
    // What each hook that lost its line said, so a red run names the wait that ran out.
    let lost = lost.into_inner().unwrap();
    assert_eq!(
        delivered.len(),
        200,
        "every hook either reached the host or spooled; the ones that did not said: {lost:#?}"
    );
    for id in &delivered {
        assert!(recorded.contains(id), "{id} was lost");
    }
}

/// Chat `chat`'s spool drained while the host runs, the ids of the tool calls it handed on.
fn drained_while_running(socket: &std::path::Path, chat: u32) -> Vec<String> {
    let mut ids = Vec::new();
    spool::drain_running(&spool::dir_for(socket), chat, &mut |item| {
        if let spool::Drained::Line {
            line: spool::Spooled::Tool(call),
            ..
        } = item
        {
            ids.push(call.call.unwrap_or_default());
        }
        Ok(())
    })
    .expect("the spool drains");
    ids
}

/// #952 L2: a host slower than its hook waits records the line live, and the hook spools it
/// too; the drain while the host runs knows the copy, and the line is recorded once.
#[test]
fn a_line_the_host_recorded_late_and_its_hook_spooled_is_recorded_once() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(11)
        .expect("a token");
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let _reading = listener.hear(hearing({
        let recorded = Arc::clone(&recorded);
        Box::new(move |call| {
            std::thread::sleep(Duration::from_millis(400));
            recorded
                .lock()
                .expect("the record")
                .push(call.call.unwrap_or_default());
            Ok(())
        })
    }));

    let delivered = deliver_tool(&path, Some(&token), &call(11, "late")).expect("spooled");
    assert_eq!(
        delivered,
        Delivered::Spooled(1),
        "the hook gave up on the host"
    );
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while recorded.lock().expect("the record").is_empty() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(*recorded.lock().expect("the record"), ["late"]);
    assert_eq!(
        drained_while_running(&path, 11),
        Vec::<String>::new(),
        "the spooled copy of a line recorded live is not handed on"
    );
}

/// A line the host heard late and could not record is the spooled copy's to record.
#[test]
fn a_line_the_host_heard_late_and_could_not_record_is_handed_on_from_the_spool() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(12)
        .expect("a token");
    let heard = Arc::new(AtomicBool::new(false));
    let _reading = listener.hear(hearing({
        let heard = Arc::clone(&heard);
        Box::new(move |_| {
            std::thread::sleep(Duration::from_millis(400));
            heard.store(true, Ordering::SeqCst);
            Err(std::io::Error::other("no space left on the device"))
        })
    }));

    let delivered = deliver_tool(&path, Some(&token), &call(12, "unrecorded")).expect("spooled");
    assert_eq!(delivered, Delivered::Spooled(1));
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while !heard.load(Ordering::SeqCst) && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(drained_while_running(&path, 12), ["unrecorded"]);
}

/// #2003, closing #2000's D-2000-8: with chats in the session host, the host's listener takes
/// the line and the window's hearing, handed it over the relay, is slower than the hook waits;
/// the hook spools it too. The host records what the window carried back on the thread that
/// heard the line, and drains the spool in the same process, so the line is recorded once.
#[test]
fn a_line_the_host_took_late_while_the_window_heard_it_and_its_hook_spooled_is_recorded_once() {
    use super::relay::{Heard, Heeding, Keeping, Relaying, Replied, Reply, carry};
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener
        .tokens()
        .issue_to_this_process(13)
        .expect("a token");
    // The window's own hearing: slow, and carrying back what it would record.
    let window = Arc::new(Heeding::new(hearing(Box::new(|call| {
        std::thread::sleep(Duration::from_millis(400));
        let _ = carry(crate::eventlog::Entry::Tool {
            plane: "/planes/one".into(),
            call,
        });
        Ok(())
    }))));
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let relaying = Relaying::keeping(
        {
            let host = Arc::clone(&host);
            Box::new(move |id, heard| {
                let heard: Heard =
                    serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
                let host = Arc::clone(&host);
                let reply: Reply = Arc::new(move |replied: Replied| {
                    let relaying = host.lock().unwrap().clone().unwrap();
                    relaying.replied(id, replied);
                });
                let window = Arc::clone(&window);
                std::thread::spawn(move || window.hear(1, id, heard, reply));
                true
            })
        },
        Keeping {
            record: {
                let recorded = Arc::clone(&recorded);
                Box::new(move |entry| {
                    if let crate::eventlog::Entry::Tool { call, .. } = entry {
                        recorded
                            .lock()
                            .expect("the record")
                            .push(call.call.unwrap_or_default());
                    }
                    Ok(())
                })
            },
            heard_from: Box::new(|_| {}),
            unheard: Box::new(|_| {}),
        },
    );
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    let tokens = listener.tokens();
    let _reading = listener.hear(relaying.hearing(tokens));

    let delivered = deliver_tool(&path, Some(&token), &call(13, "late")).expect("spooled");
    assert_eq!(
        delivered,
        Delivered::Spooled(1),
        "the hook gave up on the host"
    );
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while recorded.lock().expect("the record").is_empty() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(*recorded.lock().expect("the record"), ["late"]);
    assert_eq!(
        drained_while_running(&path, 13),
        Vec::<String>::new(),
        "the host's drain knows the copy of a line it recorded live"
    );
}
