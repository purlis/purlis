// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The relay, end to end in one process: the host's [`Hearing`] hands each line on, the
//! window's [`Heeding`] hears it with the window's own [`Hearing`], and the reply comes back.
//! What travels between them goes as JSON, as it does over the session protocol.

use std::io::{BufRead, Cursor, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use super::*;
use crate::harness::hooked::Source;
use crate::harness::model::{ChoiceKind, ChoiceScope};
use crate::state::Event;

/// What the window's hearing was handed, in order.
#[derive(Default)]
struct Window {
    heard: Mutex<Vec<String>>,
}

impl Window {
    fn said(&self, what: impl Into<String>) {
        self.heard.lock().unwrap().push(what.into());
    }

    fn heard(&self) -> Vec<String> {
        self.heard.lock().unwrap().clone()
    }
}

fn report(chat: u32) -> Report {
    Report {
        chat,
        event: Event::Stop,
        conversation: Default::default(),
        pid: None,
        agent: None,
        detail: Default::default(),
    }
}

fn asked(chat: u32) -> PermissionAsked {
    PermissionAsked {
        chat,
        permission_request: Source::ClaudeCode,
        payload: serde_json::json!({"tool_name": "Bash"}),
        ran_for_ms: None,
    }
}

fn allow() -> Choice {
    Choice {
        id: "allow".into(),
        label: "Allow".into(),
        kind: ChoiceKind::Allow,
        scope: ChoiceScope::Once,
    }
}

/// What the window's permission hearer holds: the sender of each ask's option, and how each
/// ask it held was let go of.
#[derive(Default)]
struct Holding {
    options: Mutex<Vec<mpsc::Sender<Choice>>>,
    left: Mutex<Vec<Left>>,
}

/// The window's own hearing: it records what it hears, takes a report unless it is chat 13's,
/// answers an ask with a ticket naming its connection, holds every permission ask for a
/// minute, and runs a `secret exec` that writes back each input line it reads.
fn the_windows(window: &Arc<Window>, holding: &Arc<Holding>) -> Hearing {
    Hearing {
        each: {
            let window = Arc::clone(window);
            Box::new(move |report| {
                window.said(format!("report {}", report.chat));
                if report.chat == 13 {
                    Err(io::Error::other("the event log is full"))
                } else {
                    Ok(())
                }
            })
        },
        answer: {
            let window = Arc::clone(window);
            Box::new(move |connection, _ask| {
                window.said(format!("ask on {connection}"));
                Answer::Ticket {
                    ticket: format!("t{connection}"),
                }
            })
        },
        noticed: {
            let window = Arc::clone(window);
            Box::new(move |notice| window.said(format!("by hand {}", notice.chat)))
        },
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(|_| Ok(())),
        permission: {
            let holding = Arc::clone(holding);
            Box::new(move |asked| {
                if asked.chat == 13 {
                    return None;
                }
                let (option, answered) = mpsc::channel();
                holding.options.lock().unwrap().push(option);
                let holding = Arc::clone(&holding);
                Some(Waiting {
                    answered,
                    until: Duration::from_secs(60),
                    left: Box::new(move |left| holding.left.lock().unwrap().push(left)),
                })
            })
        },
        touching: Box::new(|_| {}),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        secret_exec: Box::new(|_, reader, mut writer| {
            for line in reader.lines() {
                let line = line.unwrap();
                writeln!(writer, "ran {line}").unwrap();
            }
        }),
    }
}

/// A host's hearing whose lines reach `window`'s hearing over JSON, each line heard as the
/// window's link hears it: one that waits on a thread of its own, every other in order.
fn relayed(window: &Arc<Window>, holding: &Arc<Holding>) -> Hearing {
    let heeding = Arc::new(Heeding::new(the_windows(window, holding)));
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let relaying = Relaying::new({
        let host = Arc::clone(&host);
        Box::new(move |id, heard| {
            let heard: Heard =
                serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
            let host = Arc::clone(&host);
            let reply: Reply = Arc::new(move |replied: Replied| {
                let replied: Replied =
                    serde_json::from_str(&serde_json::to_string(&replied).unwrap()).unwrap();
                let relaying = host.lock().unwrap().clone().unwrap();
                relaying.replied(id, replied);
            });
            let heeding = Arc::clone(&heeding);
            if Heeding::waits(&heard) {
                std::thread::spawn(move || heeding.hear(id, heard, reply));
            } else {
                heeding.hear(id, heard, reply);
            }
            true
        })
    });
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    relaying.hearing(Arc::new(ChatTokens::default()))
}

/// A host's hearing with no window to hand anything on to.
fn unheard() -> Hearing {
    Relaying::new(Box::new(|_, _| false)).hearing(Arc::new(ChatTokens::default()))
}

fn a_window() -> (Arc<Window>, Arc<Holding>) {
    (Arc::default(), Arc::default())
}

#[test]
fn a_report_the_window_takes_is_taken_and_one_it_refuses_is_not() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);

    let taken = (host.each)(report(3));
    let refused = (host.each)(report(13));

    assert!(taken.is_ok(), "{taken:?}");
    assert_eq!(
        refused.expect_err("not taken").to_string(),
        "the event log is full"
    );
    assert_eq!(window.heard(), ["report 3", "report 13"]);
}

#[test]
fn an_ask_is_answered_by_the_window_for_the_connection_it_came_on() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);

    let answer = (host.answer)(42, Ask::Ticket { chat: 3 });

    assert_eq!(
        answer,
        Answer::Ticket {
            ticket: "t42".into()
        }
    );
    assert_eq!(window.heard(), ["ask on 42"]);
}

#[test]
fn a_line_that_is_only_told_reaches_the_window() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);

    (host.noticed)(StartedByHand {
        chat: 5,
        started_by_hand: "claude".into(),
        cwd: None,
    });

    assert_eq!(window.heard(), ["by hand 5"]);
}

#[test]
fn a_permission_ask_held_in_the_window_is_answered_with_the_option_chosen_there() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);

    let waiting = (host.permission)(asked(3)).expect("held");
    holding.options.lock().unwrap()[0]
        .send(allow())
        .expect("the window still holds it");

    assert_eq!(
        waiting.answered.recv_timeout(Duration::from_secs(5)),
        Ok(allow())
    );
    assert_eq!(waiting.until, Duration::from_secs(60));
}

#[test]
fn a_permission_ask_the_window_does_not_hold_is_not_held() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);

    assert!((host.permission)(asked(13)).is_none());
}

#[test]
fn a_held_permission_ask_whose_hook_went_away_is_let_go_of_in_the_window() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);
    let waiting = (host.permission)(asked(3)).expect("held");

    (waiting.left)(Left::Closed);

    let deadline = Instant::now() + Duration::from_secs(5);
    while holding.left.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the window was never told");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*holding.left.lock().unwrap(), [Left::Closed]);
}

#[test]
fn a_secret_exec_runs_in_the_window_on_what_its_asker_types() {
    let (window, holding) = a_window();
    let host = relayed(&window, &holding);
    let written = Arc::new(Mutex::new(Vec::new()));

    (host.secret_exec)(
        brokered::Ask {
            chat: 3,
            secret_exec: Default::default(),
        },
        Box::new(Cursor::new(b"one\ntwo\n".to_vec())),
        Box::new(Shared(Arc::clone(&written))),
    );

    assert_eq!(
        String::from_utf8(written.lock().unwrap().clone()).unwrap(),
        "ran one\nran two\n"
    );
}

#[test]
fn with_no_window_a_line_is_not_taken_an_ask_is_refused_and_a_permission_ask_is_not_held() {
    let host = unheard();

    let taken = (host.each)(report(3));
    let answer = (host.answer)(42, Ask::Ticket { chat: 3 });
    let held = (host.permission)(asked(3));

    assert_eq!(taken.expect_err("not taken").to_string(), NOBODY_HEARS);
    assert_eq!(
        answer,
        Answer::No {
            why: NOBODY_HEARS.into()
        }
    );
    assert!(held.is_none());
}

#[test]
fn a_line_still_waiting_when_the_window_is_let_go_of_is_answered_as_unheard() {
    let relaying = Relaying::new(Box::new(|_, _| true));
    let host = relaying.hearing(Arc::new(ChatTokens::default()));
    let asking = std::thread::spawn(move || (host.answer)(1, Ask::Ticket { chat: 3 }));
    std::thread::sleep(Duration::from_millis(200));

    relaying.let_go();

    assert_eq!(
        asking.join().expect("answered"),
        Answer::No {
            why: NOBODY_HEARS.into()
        }
    );
}

#[test]
fn the_window_is_told_an_asker_hung_up_until_its_ask_is_answered() {
    let (window, holding) = a_window();
    let heeding = Heeding::new(the_windows(&window, &holding));
    let nothing: Reply = Arc::new(|_| {});

    heeding.hear(9, Heard::AskerGone { connection: 42 }, Arc::clone(&nothing));
    let gone_while_asked = heeding.asker_gone(42);
    heeding.hear(
        10,
        Heard::Ask {
            connection: 42,
            ask: Ask::Ticket { chat: 3 },
        },
        nothing,
    );

    assert!(gone_while_asked);
    assert!(!heeding.asker_gone(42));
}

/// A writer a test reads back.
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------
// the event log is written in the host (#2003)
// ---------------------------------------------------------------------------------------

/// What the host's keeping was given: each entry it recorded, and each chat heard from.
#[derive(Default)]
struct Kept {
    recorded: Mutex<Vec<Entry>>,
    heard_from: Mutex<Vec<u32>>,
}

fn a_report_entry(chat: u32) -> Entry {
    Entry::Report {
        plane: "/planes/one".into(),
        report: report(chat),
        followed: crate::eventlog::Followed::No,
        begun: None,
    }
}

/// The window's own hearing, as the window's hooks hear in a project whose chats run in the
/// host: what it would record of a report it carries back ([`carry`]), and what it hands back
/// written nothing of its own.
fn the_windows_carrying(window: &Arc<Window>) -> Hearing {
    let (_, holding) = a_window();
    let mut hearing = the_windows(window, &holding);
    hearing.each = {
        let window = Arc::clone(window);
        Box::new(move |report| {
            window.said(format!("report {}", report.chat));
            match carry(a_report_entry(report.chat)) {
                Ok(()) => Ok(()),
                Err(_) => Err(io::Error::other("nothing carried it")),
            }
        })
    };
    hearing
}

/// A host's hearing over JSON to `window`'s, keeping the event log in `kept`.
fn relayed_keeping(window: Hearing, kept: &Arc<Kept>) -> Hearing {
    let heeding = Arc::new(Heeding::new(window));
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let relaying = Relaying::keeping(
        {
            let host = Arc::clone(&host);
            Box::new(move |id, heard| {
                let heard: Heard =
                    serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
                let host = Arc::clone(&host);
                let reply: Reply = Arc::new(move |replied: Replied| {
                    let replied: Replied =
                        serde_json::from_str(&serde_json::to_string(&replied).unwrap()).unwrap();
                    let relaying = host.lock().unwrap().clone().unwrap();
                    relaying.replied(id, replied);
                });
                heeding.hear(id, heard, reply);
                true
            })
        },
        Keeping {
            record: {
                let kept = Arc::clone(kept);
                Box::new(move |entry| {
                    kept.recorded.lock().unwrap().push(entry);
                    Ok(())
                })
            },
            heard_from: {
                let kept = Arc::clone(kept);
                Box::new(move |chat| kept.heard_from.lock().unwrap().push(chat))
            },
        },
    );
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    relaying.hearing(Arc::new(ChatTokens::default()))
}

#[test]
fn what_the_windows_hearing_would_record_is_recorded_in_the_host_and_its_hook_told_it_is_taken() {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let host = relayed_keeping(the_windows_carrying(&window), &kept);

    let taken = (host.each)(report(3));

    assert!(taken.is_ok(), "{taken:?}");
    assert_eq!(window.heard(), ["report 3"], "the window still hears it");
    assert_eq!(*kept.recorded.lock().unwrap(), [a_report_entry(3)]);
    assert_eq!(*kept.heard_from.lock().unwrap(), [3]);
}

#[test]
fn a_line_whose_spooled_copy_the_hosts_drain_recorded_first_moves_the_window_and_is_not_recorded_again()
 {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let host = relayed_keeping(the_windows_carrying(&window), &kept);

    // As the host's listener holds it for a line the drain recorded first (#952).
    let taken = {
        let _already = super::super::RecordedAlready::held();
        (host.each)(report(3))
    };

    assert!(taken.is_ok(), "{taken:?}");
    assert_eq!(window.heard(), ["report 3"], "what it moves, it moves");
    assert!(
        kept.recorded.lock().unwrap().is_empty(),
        "recorded once, from its spooled copy"
    );
}

#[test]
fn a_line_the_window_could_not_take_is_recorded_nowhere_and_its_hook_spools_it() {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let (_, holding) = a_window();
    let host = relayed_keeping(the_windows(&window, &holding), &kept);

    let refused = (host.each)(report(13));

    assert_eq!(
        refused.expect_err("not taken").to_string(),
        "the event log is full"
    );
    assert!(kept.recorded.lock().unwrap().is_empty());
}

#[test]
fn an_entry_carried_with_no_line_being_heard_is_handed_back_to_be_written_as_anything_else_is() {
    let entry = a_report_entry(3);

    assert_eq!(carry(entry.clone()), Err(entry.clone()));
    let ((), carried) = carrying(|| {
        assert_eq!(carry(entry.clone()), Ok(()));
        assert_eq!(
            carry(entry.clone()),
            Err(entry.clone()),
            "one line carries one entry"
        );
    });
    assert_eq!(carried, Some(entry.clone()));
    assert_eq!(carry(entry.clone()), Err(entry), "not past the line");
}
