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
pub(super) struct Window {
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

/// The link to the host the lines come over, and the one a host started again comes over.
pub(super) const LINK: u64 = 1;
const AGAIN: u64 = 2;

/// The chat whose `secret exec` the window's hearing ends without reading its input.
const ENDS_UNREAD: u32 = 4;

/// A chat whose `secret exec` writes back each line it reads.
const ECHOES: u32 = 3;

/// A chat whose `secret exec`, in a hearing that has one, waits for a test to let it end.
const WAITS_FOR_THE_GATE: u32 = 5;
/// The chat whose secret exec's run panics.
const GOES_WRONG: u32 = 6;

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
pub(super) struct Holding {
    options: Mutex<Vec<mpsc::Sender<Choice>>>,
    left: Mutex<Vec<Left>>,
}

/// The window's own hearing: it records what it hears, takes a report unless it is chat 13's,
/// answers an ask with a ticket naming its connection, holds every permission ask for a
/// minute, and runs a `secret exec` that writes back each input line it reads, unless it is
/// [`ENDS_UNREAD`]'s, which ends without reading any.
pub(super) fn the_windows(window: &Arc<Window>, holding: &Arc<Holding>) -> Hearing {
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
            Box::new(move |connection, ask| {
                window.said(format!("ask on {connection}"));
                match ask {
                    // A login is performed, and its page said (#2050).
                    Ask::Browse(browse) => Answer::Browsed {
                        browser: "b1".into(),
                        browsed: Box::new(purlis_session_protocol::browsing::Browsed {
                            settled: true,
                            note: Some(if browse.waited {
                                format!("asked afresh about {}", browse.call_id)
                            } else {
                                format!("did {}", browse.call_id)
                            }),
                            ..Default::default()
                        }),
                    },
                    _ => Answer::Ticket {
                        ticket: format!("t{connection}"),
                    },
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
        secret_exec: Box::new(|ask, reader, mut writer| {
            if ask.chat == ENDS_UNREAD {
                return;
            }
            assert_ne!(ask.chat, GOES_WRONG, "the command's run went wrong");
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
    heeding.linked(LINK);
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
            // One not taken is answered already.
            if heeding.admit(LINK, id, &heard, &reply) {
                if Heeding::waits(&heard) {
                    std::thread::spawn(move || heeding.hear(LINK, id, heard, reply));
                } else {
                    heeding.hear(LINK, id, heard, reply);
                }
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

pub(super) fn a_window() -> (Arc<Window>, Arc<Holding>) {
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
    let (ended, ran) = mpsc::channel();

    std::thread::spawn({
        let written = Arc::clone(&written);
        move || {
            (host.secret_exec)(
                brokered::Ask {
                    chat: 3,
                    secret_exec: Default::default(),
                    ..Default::default()
                },
                Box::new(Cursor::new(b"one\ntwo\n".to_vec())),
                Box::new(Shared(written)),
            );
            let _ = ended.send(());
        }
    });

    assert_eq!(
        ran.recv_timeout(Duration::from_secs(10)),
        Ok(()),
        "the secret exec did not end; it wrote {:?}",
        String::from_utf8_lossy(&written.lock().unwrap())
    );
    assert_eq!(
        String::from_utf8(written.lock().unwrap().clone()).unwrap(),
        "ran one\nran two\n"
    );
}

/// Hears `heard`, a `secret exec` handed on as `id`, on a thread of its own, as the window's
/// link does, and answers what it replied up to its end: a panic naming what it replied where it
/// does not end within seconds.
fn an_exec_heard(heeding: &Arc<Heeding>, id: u64, chat: u32) -> Vec<Replied> {
    an_exec_heard_on(heeding, LINK, id, chat)
}

/// [`an_exec_heard`], over link `link`.
fn an_exec_heard_on(heeding: &Arc<Heeding>, link: u64, id: u64, chat: u32) -> Vec<Replied> {
    let (replies, replied) = mpsc::channel();
    let heeding = Arc::clone(heeding);
    std::thread::spawn(move || {
        let reply: Reply = Arc::new(move |r| {
            let _ = replies.send(r);
        });
        let exec = Heard::SecretExec(brokered::Ask {
            chat,
            secret_exec: Default::default(),
            ..Default::default()
        });
        heeding.hear(link, id, exec, reply);
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut heard = Vec::new();
    while heard.last() != Some(&Replied::Ended) {
        match replied.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(r) => heard.push(r),
            Err(_) => panic!("the secret exec never ended; it replied {heard:?}"),
        }
    }
    heard
}

fn input(of: u64, line: &str) -> Heard {
    Heard::Input {
        of,
        line: line.into(),
    }
}

fn an_exec(chat: u32) -> Heard {
    Heard::SecretExec(brokered::Ask {
        chat,
        secret_exec: Default::default(),
        ..Default::default()
    })
}

/// Takes the `secret exec` handed on as `id`, of chat `chat`, over link `link`, as the window's
/// link does in the order the lines came, before it is heard: whether it was taken.
fn admitted(heeding: &Heeding, link: u64, id: u64, chat: u32) -> bool {
    heeding.admit(link, id, &an_exec(chat), &nothing())
}

/// The window's link hears a `secret exec` on a thread of its own and its input in order on
/// another, so the input, and its end, can come before the exec has begun (#2271): the exec
/// still reads every line, then the end, and ends.
#[test]
fn a_secret_execs_input_heard_before_it_began_is_still_read_to_its_end() {
    let heeding = a_heeding();
    assert!(admitted(&heeding, LINK, 7, ECHOES));

    heeding.hear(LINK, 8, input(7, "one"), nothing());
    heeding.hear(LINK, 9, Heard::InputEnded { of: 7 }, nothing());

    assert_eq!(an_exec_heard(&heeding, 7, ECHOES), ran(&["one"]));
}

fn ran(lines: &[&str]) -> Vec<Replied> {
    lines
        .iter()
        .map(|line| Replied::Wrote {
            text: format!("ran {line}\n"),
        })
        .chain([Replied::Ended])
        .collect()
}

fn a_heeding() -> Arc<Heeding> {
    let (window, holding) = a_window();
    let heeding = Arc::new(Heeding::new(the_windows(&window, &holding)));
    heeding.linked(LINK);
    heeding
}

fn nothing() -> Reply {
    Arc::new(|_| {})
}

/// Input that comes for a `secret exec` after it ended, and before the host says that input
/// ended, has no reader and is not kept: an exec under the same number later reads only its own
/// (#2271).
#[test]
fn a_secret_execs_input_heard_after_it_ended_is_not_kept() {
    let heeding = a_heeding();

    assert_eq!(an_exec_heard(&heeding, 1, ENDS_UNREAD), [Replied::Ended]);
    heeding.hear(LINK, 2, input(1, "late"), nothing());
    heeding.hear(LINK, 3, Heard::InputEnded { of: 1 }, nothing());
    assert!(admitted(&heeding, LINK, 1, ECHOES));
    heeding.hear(LINK, 4, input(1, "own"), nothing());
    heeding.hear(LINK, 5, Heard::InputEnded { of: 1 }, nothing());

    assert_eq!(an_exec_heard(&heeding, 1, ECHOES), ran(&["own"]));
}

/// A link to the host that ends ends every `secret exec`'s input on it, and keeps none of what
/// came for one not begun (#2271).
#[test]
fn when_the_link_ends_no_secret_execs_input_is_kept() {
    let heeding = a_heeding();
    heeding.linked(LINK);
    assert!(admitted(&heeding, LINK, 5, ECHOES));
    heeding.hear(LINK, 2, input(5, "typed"), nothing());

    assert!(admitted(&heeding, LINK, 6, ECHOES));
    let (replies, replied) = mpsc::channel();
    std::thread::spawn({
        let heeding = Arc::clone(&heeding);
        move || {
            let reply: Reply = Arc::new(move |r| {
                let _ = replies.send(r);
            });
            let exec = Heard::SecretExec(brokered::Ask {
                chat: ECHOES,
                secret_exec: Default::default(),
                ..Default::default()
            });
            heeding.hear(LINK, 6, exec, reply);
        }
    });
    heeding.hear(LINK, 3, input(6, "first"), nothing());
    // It runs: it wrote back what it read.
    assert_eq!(
        replied.recv_timeout(Duration::from_secs(10)),
        Ok(Replied::Wrote {
            text: "ran first\n".into()
        })
    );

    heeding.unlinked(LINK);

    assert_eq!(
        replied.recv_timeout(Duration::from_secs(10)),
        Ok(Replied::Ended),
        "the running exec's input did not end with the link"
    );
    heeding.linked(AGAIN);
    assert!(admitted(&heeding, AGAIN, 5, ECHOES));
    heeding.hear(AGAIN, 4, Heard::InputEnded { of: 5 }, nothing());
    assert_eq!(
        an_exec_heard_on(&heeding, AGAIN, 5, ECHOES),
        [Replied::Ended]
    );
}

/// #3408: a host the lines are relayed from that is lost to its relay let go of every line it
/// handed on, so every `secret exec` from it ends its input here: one that runs, and one taken
/// but not yet begun, which begins already ended. The link stays, and the next exec on it reads
/// its own input.
#[test]
fn a_host_lost_to_its_relay_ends_every_secret_execs_input_and_the_link_goes_on() {
    let heeding = a_heeding();
    assert!(admitted(&heeding, LINK, 5, ECHOES));
    heeding.hear(LINK, 2, input(5, "typed"), nothing());
    assert!(admitted(&heeding, LINK, 6, ECHOES));
    let (replies, replied) = mpsc::channel();
    std::thread::spawn({
        let heeding = Arc::clone(&heeding);
        move || {
            let reply: Reply = Arc::new(move |r| {
                let _ = replies.send(r);
            });
            heeding.hear(LINK, 6, an_exec(ECHOES), reply);
        }
    });
    heeding.hear(LINK, 3, input(6, "first"), nothing());
    assert_eq!(
        replied.recv_timeout(Duration::from_secs(10)),
        Ok(Replied::Wrote {
            text: "ran first\n".into()
        })
    );

    heeding.lost_its_host();

    assert_eq!(
        replied.recv_timeout(Duration::from_secs(10)),
        Ok(Replied::Ended),
        "the running exec's input did not end"
    );
    assert_eq!(
        an_exec_heard(&heeding, 5, ECHOES),
        ran(&["typed"]),
        "the exec not begun begins with its input ended"
    );
    assert!(admitted(&heeding, LINK, 9, ECHOES), "the link goes on");
    heeding.hear(LINK, 10, input(9, "own"), nothing());
    heeding.hear(LINK, 11, Heard::InputEnded { of: 9 }, nothing());
    assert_eq!(an_exec_heard(&heeding, 9, ECHOES), ran(&["own"]));
}

/// A host started again numbers its lines from the start: an exec under a number used before
/// the window linked to it again reads only its own input, never what came before (#2271).
#[test]
fn after_the_window_links_again_a_reused_number_reads_only_its_own_input() {
    let heeding = a_heeding();
    heeding.linked(LINK);
    assert_eq!(an_exec_heard(&heeding, 1, ENDS_UNREAD), [Replied::Ended]);
    assert!(admitted(&heeding, LINK, 2, ECHOES));
    heeding.hear(LINK, 2, input(2, "before"), nothing());

    heeding.linked(AGAIN);
    // The end of the link before, told late, is not this link's.
    heeding.unlinked(LINK);
    assert!(admitted(&heeding, AGAIN, 1, ECHOES));
    assert!(admitted(&heeding, AGAIN, 2, ECHOES));
    heeding.hear(AGAIN, 3, input(1, "own"), nothing());
    heeding.hear(AGAIN, 4, Heard::InputEnded { of: 1 }, nothing());
    heeding.hear(AGAIN, 5, Heard::InputEnded { of: 2 }, nothing());

    assert_eq!(an_exec_heard_on(&heeding, AGAIN, 1, ECHOES), ran(&["own"]));
    assert_eq!(
        an_exec_heard_on(&heeding, AGAIN, 2, ECHOES),
        [Replied::Ended]
    );
}

/// An exec from before the window linked again that ends only after an exec under its number
/// began on the new link leaves the new one's input as it was (#2271).
#[test]
fn an_exec_from_the_link_before_ending_late_does_not_end_one_under_its_number_now() {
    let (window, holding) = a_window();
    let mut hearing = the_windows(&window, &holding);
    let echo = std::mem::replace(&mut hearing.secret_exec, Box::new(|_, _, _| {}));
    let (began, has_begun) = mpsc::channel::<()>();
    let (open, gate) = mpsc::channel::<()>();
    let (began, gate) = (Mutex::new(began), Mutex::new(gate));
    hearing.secret_exec = Box::new(move |ask, reader, writer| {
        if ask.chat == WAITS_FOR_THE_GATE {
            let _ = began.lock().unwrap().send(());
            let _ = gate.lock().unwrap().recv();
        } else {
            echo(ask, reader, writer);
        }
    });
    let heeding = Arc::new(Heeding::new(hearing));
    heeding.linked(LINK);
    let before = std::thread::spawn({
        let heeding = Arc::clone(&heeding);
        move || an_exec_heard(&heeding, 1, WAITS_FOR_THE_GATE)
    });
    has_begun
        .recv_timeout(Duration::from_secs(10))
        .expect("the exec before began");

    heeding.linked(AGAIN);
    assert!(admitted(&heeding, AGAIN, 1, ECHOES));
    let (replies, replied) = mpsc::channel();
    std::thread::spawn({
        let heeding = Arc::clone(&heeding);
        move || {
            let reply: Reply = Arc::new(move |r| {
                let _ = replies.send(r);
            });
            let exec = Heard::SecretExec(brokered::Ask {
                chat: ECHOES,
                secret_exec: Default::default(),
                ..Default::default()
            });
            heeding.hear(AGAIN, 1, exec, reply);
        }
    });
    heeding.hear(AGAIN, 2, input(1, "x"), nothing());
    let within = Duration::from_secs(10);
    assert_eq!(
        replied.recv_timeout(within),
        Ok(Replied::Wrote {
            text: "ran x\n".into()
        })
    );
    open.send(()).unwrap();
    assert_eq!(before.join().expect("ended"), [Replied::Ended]);
    heeding.hear(AGAIN, 3, input(1, "y"), nothing());
    heeding.hear(AGAIN, 4, Heard::InputEnded { of: 1 }, nothing());

    assert_eq!(
        replied.recv_timeout(within),
        Ok(Replied::Wrote {
            text: "ran y\n".into()
        })
    );
    assert_eq!(replied.recv_timeout(within), Ok(Replied::Ended));
}

/// A `secret exec` the window could not begin is told to the host as ended, and what came for
/// it is not kept (#2271).
#[test]
fn a_secret_exec_that_could_not_begin_ends_and_keeps_no_input() {
    let heeding = a_heeding();
    let (replies, replied) = mpsc::channel();
    let reply: Reply = Arc::new(move |r| {
        let _ = replies.send(r);
    });
    assert!(admitted(&heeding, LINK, 3, ECHOES));
    heeding.hear(LINK, 2, input(3, "typed"), nothing());

    heeding.not_begun(LINK, 3, &reply);
    heeding.hear(LINK, 4, input(3, "more"), nothing());
    heeding.hear(LINK, 5, Heard::InputEnded { of: 3 }, nothing());

    assert_eq!(replied.try_iter().collect::<Vec<_>>(), [Replied::Ended]);
    assert!(admitted(&heeding, LINK, 3, ECHOES));
    heeding.hear(LINK, 6, input(3, "own"), nothing());
    heeding.hear(LINK, 7, Heard::InputEnded { of: 3 }, nothing());
    assert_eq!(an_exec_heard(&heeding, 3, ECHOES), ran(&["own"]));
}

/// A line of a `secret exec`, or its end, that comes over a link other than the current one is
/// of a host that went or started again: it neither feeds nor ends an exec of its number now,
/// and an exec from it does not run (#2271).
#[test]
fn a_line_from_a_link_that_is_not_the_current_one_is_dropped() {
    let heeding = a_heeding();
    heeding.linked(AGAIN);

    heeding.hear(LINK, 2, input(1, "old"), nothing());
    heeding.hear(LINK, 3, Heard::InputEnded { of: 1 }, nothing());
    assert!(!admitted(&heeding, LINK, 9, ECHOES));
    assert!(admitted(&heeding, AGAIN, 1, ECHOES));
    heeding.hear(AGAIN, 4, input(1, "own"), nothing());
    heeding.hear(AGAIN, 5, Heard::InputEnded { of: 1 }, nothing());

    assert_eq!(
        an_exec_heard_on(&heeding, LINK, 9, ECHOES),
        [Replied::Ended]
    );
    assert_eq!(an_exec_heard_on(&heeding, AGAIN, 1, ECHOES), ran(&["own"]));
}

fn bytes(of: char, newline_and_all: usize) -> String {
    std::iter::repeat_n(of, newline_and_all - 1).collect()
}

/// What waits for a `secret exec` not begun is bounded: a line that would take it past
/// [`AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST`] is dropped and the exec's input ends there; one that
/// brings it to exactly that is kept (#2271).
#[test]
fn a_secret_exec_not_begun_is_kept_at_most_so_much_input() {
    let heeding = a_heeding();
    let all = bytes('a', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST);
    let one_more = bytes('b', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST + 1);
    assert!(admitted(&heeding, LINK, 7, ECHOES));
    assert!(admitted(&heeding, LINK, 8, ECHOES));

    heeding.hear(LINK, 2, input(7, &all), nothing());
    heeding.hear(LINK, 3, input(7, "after"), nothing());
    heeding.hear(LINK, 4, input(8, &one_more), nothing());
    heeding.hear(LINK, 5, Heard::InputEnded { of: 8 }, nothing());

    assert_eq!(an_exec_heard(&heeding, 7, ECHOES), ran(&[&all]));
    assert_eq!(an_exec_heard(&heeding, 8, ECHOES), [Replied::Ended]);
}

/// An exec whose input ended at the cap is left a mark: what the host still sends for it is
/// dropped, not kept again for a later exec of its number (#2271).
#[test]
fn input_after_the_cap_is_not_kept() {
    let heeding = a_heeding();
    let half = bytes('a', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST / 2);
    assert!(admitted(&heeding, LINK, 7, ECHOES));
    heeding.hear(LINK, 2, input(7, &half), nothing());
    heeding.hear(LINK, 3, input(7, &half), nothing());
    heeding.hear(LINK, 4, input(7, "past"), nothing());
    assert_eq!(an_exec_heard(&heeding, 7, ECHOES), ran(&[&half, &half]));

    heeding.hear(LINK, 5, input(7, "late"), nothing());
    heeding.hear(LINK, 6, Heard::InputEnded { of: 7 }, nothing());
    assert!(admitted(&heeding, LINK, 7, ECHOES));
    heeding.hear(LINK, 7, input(7, "own"), nothing());
    heeding.hear(LINK, 8, Heard::InputEnded { of: 7 }, nothing());

    assert_eq!(an_exec_heard(&heeding, 7, ECHOES), ran(&["own"]));
}

/// An exec that could not begin after its input passed the cap is left a mark too (#2271).
#[test]
fn input_after_the_cap_for_an_exec_that_could_not_begin_is_not_kept() {
    let heeding = a_heeding();
    let past = bytes('b', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST + 1);
    assert!(admitted(&heeding, LINK, 3, ECHOES));
    heeding.hear(LINK, 2, input(3, &past), nothing());

    heeding.not_begun(LINK, 3, &nothing());
    heeding.hear(LINK, 3, input(3, "late"), nothing());
    heeding.hear(LINK, 4, Heard::InputEnded { of: 3 }, nothing());
    assert!(admitted(&heeding, LINK, 3, ECHOES));
    heeding.hear(LINK, 5, input(3, "own"), nothing());
    heeding.hear(LINK, 6, Heard::InputEnded { of: 3 }, nothing());

    assert_eq!(an_exec_heard(&heeding, 3, ECHOES), ran(&["own"]));
}

/// What waits for every `secret exec` not begun on a link together is bounded by
/// [`EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST`]: up to exactly that is kept, and a line past it is
/// dropped and its exec's input ends there (#2271).
#[test]
fn every_secret_exec_not_begun_on_a_link_is_kept_at_most_so_much_input_together() {
    let heeding = a_heeding();
    let each = bytes('a', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST);
    let execs = (EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST / AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST) as u64;
    for exec in 1..=execs + 1 {
        assert!(admitted(&heeding, LINK, exec, ECHOES));
    }
    for exec in 1..=execs {
        heeding.hear(LINK, 100 + exec, input(exec, &each), nothing());
    }
    heeding.hear(LINK, 200, input(execs + 1, "x"), nothing());
    for exec in 1..=execs + 1 {
        heeding.hear(LINK, 300 + exec, Heard::InputEnded { of: exec }, nothing());
    }

    assert_eq!(an_exec_heard(&heeding, execs + 1, ECHOES), [Replied::Ended]);
    for exec in 1..=execs {
        assert_eq!(an_exec_heard(&heeding, exec, ECHOES), ran(&[&each]));
    }
}

/// How many `secret exec`s the current link tracks in `heeding` now.
fn tracked(heeding: &Heeding) -> usize {
    lock(&heeding.inputs).by_exec.len()
}

/// A chat other than the ones the window's hearing knows, for exec `exec` of many: no more
/// than [`EXECS_A_CHAT_HAS_TRACKED_AT_MOST`] numbers in a row share one.
fn a_chat_of_many(exec: u64) -> u32 {
    1000 + (exec / EXECS_A_CHAT_HAS_TRACKED_AT_MOST as u64) as u32
}

/// How many `secret exec`s a link tracks is bounded (#2571): the exec that brings it to exactly
/// [`EXECS_A_LINK_TRACKS_AT_MOST`] is taken, and still reads its input to its end; one past it
/// is not taken, until one ends.
#[test]
fn a_link_tracks_at_most_so_many_secret_execs() {
    let heeding = a_heeding();
    let last = EXECS_A_LINK_TRACKS_AT_MOST as u64;
    for exec in 1..last {
        assert!(admitted(&heeding, LINK, exec, a_chat_of_many(exec)));
    }

    assert!(admitted(&heeding, LINK, last, ECHOES));
    assert!(!admitted(&heeding, LINK, last + 1, ECHOES));
    assert_eq!(tracked(&heeding), EXECS_A_LINK_TRACKS_AT_MOST);

    heeding.hear(LINK, 1, input(last, "kept"), nothing());
    heeding.hear(LINK, 2, Heard::InputEnded { of: last }, nothing());
    assert_eq!(an_exec_heard(&heeding, last, ECHOES), ran(&["kept"]));
    assert!(admitted(&heeding, LINK, last + 1, ECHOES));
}

/// One chat's `secret exec`s cannot crowd out another's (#2571): a chat has at most
/// [`EXECS_A_CHAT_HAS_TRACKED_AT_MOST`] tracked on a link, another chat's are still taken, and
/// the chat has room again once one of its own ends.
#[test]
fn one_chats_secret_execs_cannot_crowd_out_anothers() {
    let heeding = a_heeding();
    let most = EXECS_A_CHAT_HAS_TRACKED_AT_MOST as u64;
    for exec in 1..=most {
        assert!(admitted(&heeding, LINK, exec, ECHOES));
    }

    assert!(!admitted(&heeding, LINK, most + 1, ECHOES));
    assert!(admitted(&heeding, LINK, most + 2, ENDS_UNREAD));

    heeding.hear(LINK, 1, Heard::InputEnded { of: 1 }, nothing());
    assert_eq!(an_exec_heard(&heeding, 1, ECHOES), [Replied::Ended]);
    assert!(admitted(&heeding, LINK, most + 3, ECHOES));
}

/// An exec taken while its link tracks as many as it may is still told its input ended, and
/// ends: lines for numbers not taken neither crowd it out nor are kept (#2571).
#[test]
fn an_exec_taken_at_the_cap_is_still_told_its_input_ended() {
    let heeding = a_heeding();
    let last = EXECS_A_LINK_TRACKS_AT_MOST as u64;
    for exec in 1..last {
        assert!(admitted(&heeding, LINK, exec, a_chat_of_many(exec)));
    }
    assert!(admitted(&heeding, LINK, last, ECHOES));

    for stray in last + 1..last + 1000 {
        heeding.hear(LINK, stray, input(stray, "stray"), nothing());
        heeding.hear(LINK, stray, Heard::InputEnded { of: stray }, nothing());
    }
    heeding.hear(LINK, 2, Heard::InputEnded { of: last }, nothing());

    assert_eq!(tracked(&heeding), EXECS_A_LINK_TRACKS_AT_MOST);
    assert_eq!(an_exec_heard(&heeding, last, ECHOES), [Replied::Ended]);
}

/// A `secret exec` heard with no room for it on its link is not run and is told ended at once,
/// never left waiting for input that is not kept, and nothing more is tracked (#2571).
#[test]
fn a_secret_exec_with_no_room_ends_at_once_and_keeps_nothing() {
    let heeding = a_heeding();
    for exec in 1..=EXECS_A_LINK_TRACKS_AT_MOST as u64 {
        assert!(admitted(&heeding, LINK, exec, a_chat_of_many(exec)));
    }

    let past = EXECS_A_LINK_TRACKS_AT_MOST as u64 + 1;
    assert_eq!(an_exec_heard(&heeding, past, ECHOES), [Replied::Ended]);
    assert_eq!(tracked(&heeding), EXECS_A_LINK_TRACKS_AT_MOST);
}

/// A link that sends execs, their input and its end, execs that could not begin, and lines for
/// numbers it never took keeps a bounded number of entries (#2571).
#[test]
fn a_link_sending_many_numbers_keeps_a_bounded_number_of_entries() {
    let heeding = a_heeding();
    let many = 10 * EXECS_A_LINK_TRACKS_AT_MOST as u64;
    for exec in 1..=many {
        admitted(&heeding, LINK, exec, 1000 + (exec % 16) as u32);
        heeding.hear(LINK, exec, input(exec, "x"), nothing());
        heeding.hear(LINK, exec, Heard::InputEnded { of: exec }, nothing());
        heeding.hear(LINK, exec, input(many + exec, "y"), nothing());
        heeding.hear(LINK, exec, Heard::InputEnded { of: many + exec }, nothing());
        heeding.not_begun(LINK, 2 * many + exec, &nothing());
    }

    assert_eq!(tracked(&heeding), EXECS_A_LINK_TRACKS_AT_MOST);
}

/// The bytes kept for execs not begun are counted as they come and go: an exec that begins, or
/// could not, gives its bytes back, and the end of an exec's input does not count them twice
/// (#2571).
#[test]
fn the_bytes_of_an_exec_that_began_or_could_not_are_room_again() {
    let heeding = a_heeding();
    let each = bytes('a', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST);
    let execs = (EXECS_NOT_BEGUN_ARE_KEPT_AT_MOST / AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST) as u64;
    for exec in 1..=execs + 2 {
        assert!(admitted(&heeding, LINK, exec, ECHOES));
    }
    for exec in 1..=execs {
        heeding.hear(LINK, 100 + exec, input(exec, &each), nothing());
        heeding.hear(LINK, 200 + exec, Heard::InputEnded { of: exec }, nothing());
    }

    assert_eq!(an_exec_heard(&heeding, 1, ECHOES), ran(&[&each]));
    heeding.not_begun(LINK, 2, &nothing());
    for exec in [execs + 1, execs + 2] {
        heeding.hear(LINK, 300 + exec, input(exec, &each), nothing());
        heeding.hear(LINK, 400 + exec, Heard::InputEnded { of: exec }, nothing());
    }

    assert_eq!(an_exec_heard(&heeding, execs + 1, ECHOES), ran(&[&each]));
    assert_eq!(an_exec_heard(&heeding, execs + 2, ECHOES), ran(&[&each]));
}

/// What is logged as a link passes a cap: one warning for each cap on each link, never one per
/// line or per exec, and never what the input said (#2571).
#[test]
fn passing_a_cap_is_warned_of_once_per_link_without_the_input() {
    let logged = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer({
            let logged = Arc::clone(&logged);
            move || Shared(Arc::clone(&logged))
        })
        .finish();
    let heeding = a_heeding();
    let secret = "do-not-log-me";
    let past = format!("{secret}{}", bytes('s', AN_EXEC_NOT_BEGUN_IS_KEPT_AT_MOST));
    let many = 3 * EXECS_A_LINK_TRACKS_AT_MOST as u64;

    let warnings = |link: u64| {
        heeding.linked(link);
        for exec in 1..=many {
            admitted(&heeding, link, exec, 1000 + (exec % 16) as u32);
            heeding.hear(link, 2 * exec, input(exec, &past), nothing());
            heeding.hear(link, 2 * exec + 1, input(exec, secret), nothing());
        }
        let said = String::from_utf8(std::mem::take(&mut *logged.lock().unwrap())).unwrap();
        assert!(!said.contains(secret), "the input was logged: {said}");
        said.lines().filter(|line| line.contains("WARN")).count()
    };
    tracing::subscriber::with_default(subscriber, || {
        assert_eq!(warnings(LINK), 2, "one for the bytes, one for the count");
        assert_eq!(warnings(LINK), 0, "the same link again");
        assert_eq!(warnings(AGAIN), 2, "a new link");
    });
}

/// A `secret exec` whose run panics is still no longer tracked, and its host is still told it
/// ended (#2571).
#[test]
fn a_secret_exec_whose_run_panics_ends_and_is_no_longer_tracked() {
    let heeding = a_heeding();
    assert!(admitted(&heeding, LINK, 1, GOES_WRONG));

    assert_eq!(an_exec_heard(&heeding, 1, GOES_WRONG), [Replied::Ended]);
    assert_eq!(tracked(&heeding), 0);
}

/// The asker of a `secret exec` not run for want of room is told why, then that it ended; one
/// from a link that is not the current one is only told it ended (#2571).
#[test]
fn a_secret_exec_not_run_for_want_of_room_tells_its_asker_why() {
    let heeding = a_heeding();
    for exec in 1..=EXECS_A_CHAT_HAS_TRACKED_AT_MOST as u64 {
        assert!(admitted(&heeding, LINK, exec, ECHOES));
    }
    let (replies, replied) = mpsc::channel();
    let reply: Reply = Arc::new(move |r| {
        let _ = replies.send(r);
    });

    assert!(!heeding.admit(LINK, 100, &an_exec(ECHOES), &reply));
    let told: Vec<Replied> = replied.try_iter().collect();
    let [Replied::Wrote { text }, Replied::Ended] = told.as_slice() else {
        panic!("told {told:?}");
    };
    assert!(text.contains(TOO_MANY_SECRET_EXECS), "{text}");

    assert!(!heeding.admit(AGAIN, 101, &an_exec(ENDS_UNREAD), &reply));
    assert_eq!(replied.try_iter().collect::<Vec<_>>(), [Replied::Ended]);
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

/// A report handed on to a window that went before it replied (#2006): its hook spools it, and
/// the host is told it went unheard, so the board it keeps still hears it.
#[test]
fn a_report_whose_window_went_before_replying_is_told_to_the_host_as_unheard() {
    let kept = Arc::new(Kept::default());
    let relaying = Relaying::keeping(Box::new(|_, _| true), kept.keeping());
    let host = relaying.hearing(Arc::new(ChatTokens::default()));
    let reporting = std::thread::spawn(move || (host.each)(report(3)));
    std::thread::sleep(Duration::from_millis(200));

    relaying.let_go();

    assert!(reporting.join().expect("answered").is_err(), "it spools");
    assert_eq!(*kept.unheard.lock().unwrap(), [Heard::Report(report(3))]);
}

/// A window that is only slow still has the line (#2006): it moves its own board and tells the
/// host, so the host is not told the line went unheard.
#[test]
fn a_report_a_window_did_not_take_in_time_is_not_told_as_unheard() {
    let kept = Arc::new(Kept::default());
    let relaying = Relaying::keeping(Box::new(|_, _| true), kept.keeping());
    let host = relaying.hearing(Arc::new(ChatTokens::default()));

    let taken = (host.each)(report(3));

    assert!(taken.is_err());
    assert!(kept.unheard.lock().unwrap().is_empty());
}

#[test]
fn the_window_is_told_an_asker_hung_up_until_its_ask_is_answered() {
    let (window, holding) = a_window();
    let heeding = Heeding::new(the_windows(&window, &holding));
    let nothing: Reply = Arc::new(|_| {});

    heeding.hear(
        LINK,
        9,
        Heard::AskerGone { connection: 42 },
        Arc::clone(&nothing),
    );
    let gone_while_asked = heeding.asker_gone(42);
    heeding.hear(
        LINK,
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
pub(super) struct Shared(pub(super) Arc<Mutex<Vec<u8>>>);

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

/// What the host's keeping was given: each entry it recorded, each chat heard from, and each
/// line handed on that no window replied to because the window went.
#[derive(Default)]
struct Kept {
    recorded: Mutex<Vec<Entry>>,
    heard_from: Mutex<Vec<u32>>,
    unheard: Mutex<Vec<Heard>>,
}

impl Kept {
    fn keeping(self: &Arc<Self>) -> Keeping {
        Keeping {
            record: {
                let kept = Arc::clone(self);
                Box::new(move |entry| {
                    kept.recorded.lock().unwrap().push(entry);
                    Ok(())
                })
            },
            heard_from: {
                let kept = Arc::clone(self);
                Box::new(move |chat| kept.heard_from.lock().unwrap().push(chat))
            },
            unheard: {
                let kept = Arc::clone(self);
                Box::new(move |heard| kept.unheard.lock().unwrap().push(heard.clone()))
            },
        }
    }
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
                None => Ok(()),
                Some(_) => Err(io::Error::other("nothing carried it")),
            }
        })
    };
    hearing
}

/// A host's hearing over JSON to `window`'s, keeping the event log in `kept`.
fn relayed_keeping(window: Hearing, kept: &Arc<Kept>) -> Hearing {
    relayed_keeping_as(window, kept, false, None).0
}

/// [`relayed_keeping`], of a host told it holds the project at `root` where one is named, and
/// that said it records a line there where `pathless` (1.40, #3400): the window's replies then
/// name no path, as the window sends them to such a host. And each reply as it went over, in
/// JSON.
fn relayed_keeping_as(
    window: Hearing,
    kept: &Arc<Kept>,
    pathless: bool,
    root: Option<&str>,
) -> (Hearing, Arc<Mutex<Vec<serde_json::Value>>>) {
    let sent: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
    let without_path = pathless;
    let heeding = Arc::new(Heeding::new(window));
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let relaying = Relaying::keeping(
        {
            let (host, sent) = (Arc::clone(&host), Arc::clone(&sent));
            Box::new(move |id, heard| {
                let heard: Heard =
                    serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
                let (host, sent) = (Arc::clone(&host), Arc::clone(&sent));
                let reply: Reply = Arc::new(move |replied: Replied| {
                    let replied = if without_path {
                        replied.without_path()
                    } else {
                        replied
                    };
                    let json = serde_json::to_value(&replied).unwrap();
                    sent.lock().unwrap().push(json.clone());
                    let replied: Replied = serde_json::from_value(json).unwrap();
                    let relaying = host.lock().unwrap().clone().unwrap();
                    relaying.replied(id, replied);
                });
                heeding.hear(LINK, id, heard, reply);
                true
            })
        },
        kept.keeping(),
    );
    if let Some(root) = root {
        relaying.of_project_at(std::path::Path::new(root));
    }
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    (relaying.hearing(Arc::new(ChatTokens::default())), sent)
}

#[test]
fn a_host_that_said_it_records_a_line_where_it_holds_the_project_is_sent_no_path() {
    // A pool's runner holds the project at `<data>/projects/<id>` (#3400): the window's record
    // of each line names no project, and the host records it under the one it holds.
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let held = "/data/projects/01J9PRJCT00000000000000000";
    let (host, sent) = relayed_keeping_as(the_windows_carrying(&window), &kept, true, Some(held));

    let taken = (host.each)(report(3));
    let tool = {
        let window = Arc::new(Window::default());
        let kept = Arc::new(Kept::default());
        let (host, sent) = relayed_keeping_as(
            the_windows_sending_back(&window, a_tool_entry(tool_call(5, "c1"))),
            &kept,
            true,
            Some(held),
        );
        ((host.tool)(tool_call(5, "c1")), sent, kept)
    };

    assert!(taken.is_ok(), "{taken:?}");
    assert_eq!(
        *kept.recorded.lock().unwrap(),
        [Entry::Report {
            plane: held.into(),
            report: report(3),
            followed: crate::eventlog::Followed::No,
            begun: None,
        }]
    );
    assert!(tool.0.is_ok(), "{:?}", tool.0);
    assert_eq!(
        *tool.2.recorded.lock().unwrap(),
        [Entry::Tool {
            plane: held.into(),
            call: tool_call(5, "c1"),
        }]
    );
    for said in sent
        .lock()
        .unwrap()
        .iter()
        .chain(tool.1.lock().unwrap().iter())
    {
        let said = said.to_string();
        assert!(said.starts_with(r#"{"line":"#), "{said}");
        assert!(
            !said.contains("plane") && !said.contains("/planes/one"),
            "{said}"
        );
    }
}

#[test]
fn a_host_that_has_not_said_it_is_sent_the_path_as_before() {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let (host, sent) = relayed_keeping_as(the_windows_carrying(&window), &kept, false, None);

    let taken = (host.each)(report(3));

    assert!(taken.is_ok(), "{taken:?}");
    assert_eq!(*kept.recorded.lock().unwrap(), [a_report_entry(3)]);
    let said = sent.lock().unwrap()[0].to_string();
    assert!(said.contains(r#""plane":"/planes/one""#), "{said}");
}

#[test]
fn a_line_sent_back_with_no_path_is_still_recorded_only_as_the_record_of_its_own_line() {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let (host, _) = relayed_keeping_as(
        the_windows_sending_back(&window, a_report_entry(4)),
        &kept,
        true,
        Some("/data/projects/one"),
    );

    let refused = (host.each)(report(3));

    assert!(refused.is_err());
    assert!(kept.recorded.lock().unwrap().is_empty());
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
fn a_line_the_hosts_drain_recorded_first_moves_the_window_and_is_not_recorded_again() {
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

    assert_eq!(carry(entry.clone()), Some(entry.clone()));
    let ((), carried) = carrying(|| {
        assert_eq!(carry(entry.clone()), None);
        assert_eq!(
            carry(entry.clone()),
            Some(entry.clone()),
            "one line carries one entry"
        );
    });
    assert_eq!(carried, Some(entry.clone()));
    assert_eq!(carry(entry.clone()), Some(entry), "not past the line");
}

/// The window's hearing, sending back `entry` whatever line it heard.
fn the_windows_sending_back(window: &Arc<Window>, entry: Entry) -> Hearing {
    let (_, holding) = a_window();
    let mut hearing = the_windows(window, &holding);
    hearing.each = {
        let (window, entry) = (Arc::clone(window), entry.clone());
        Box::new(move |report| {
            window.said(format!("report {}", report.chat));
            let _ = carry(entry.clone());
            Ok(())
        })
    };
    hearing.tool = {
        let (window, entry) = (Arc::clone(window), entry.clone());
        Box::new(move |call| {
            window.said(format!("tool {}", call.chat));
            let _ = carry(entry.clone());
            Ok(())
        })
    };
    hearing.refused = {
        let window = Arc::clone(window);
        Box::new(move |refused| {
            window.said(format!("refused {}", refused.chat));
            let _ = carry(entry.clone());
            Ok(())
        })
    };
    hearing
}

/// Tool call `id` of chat `chat`.
fn tool_call(chat: u32, id: &str) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some(id.to_owned()),
        args: None,
        decision: super::super::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

fn a_tool_entry(call: ToolCall) -> Entry {
    Entry::Tool {
        plane: "/planes/one".into(),
        call,
    }
}

fn a_refused_entry(chat: u32) -> Entry {
    Entry::Refused {
        plane: "/planes/one".into(),
        chat,
    }
}

fn commit_refused(chat: u32) -> CommitRefused {
    CommitRefused {
        chat,
        commit_refused: "repo: a finding".to_owned(),
    }
}

/// What the host made of `line` when the window's hearing sent `entry` back with it: whether
/// the hook was told it is taken, and what was written.
fn sent_back_with(line: &dyn Fn(&Hearing) -> io::Result<()>, entry: &Entry) -> (bool, Vec<Entry>) {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let host = relayed_keeping(the_windows_sending_back(&window, entry.clone()), &kept);
    let taken = line(&host).is_ok();
    let written = kept.recorded.lock().unwrap().clone();
    (taken, written)
}

#[test]
fn the_host_records_only_an_entry_that_is_the_record_of_the_line_it_was_sent_back_with() {
    let not_this_line = [
        a_report_entry(4),
        Entry::Report {
            plane: "/planes/one".into(),
            report: Report {
                event: Event::SessionStart,
                ..report(3)
            },
            followed: crate::eventlog::Followed::No,
            begun: None,
        },
        Entry::Refused {
            plane: "/planes/one".into(),
            chat: 3,
        },
        Entry::Audited {
            plane: "/planes/one".into(),
            number: Some(3),
            kind: "trust.dispatch.grant".into(),
            body: serde_json::json!({}),
        },
        Entry::KillSwitchRead,
    ];
    for entry in not_this_line {
        let window = Arc::new(Window::default());
        let kept = Arc::new(Kept::default());
        let host = relayed_keeping(the_windows_sending_back(&window, entry.clone()), &kept);

        let taken = (host.each)(report(3));

        assert!(taken.is_err(), "{entry:?}: its hook spools the line");
        assert!(
            kept.recorded.lock().unwrap().is_empty(),
            "{entry:?} is not written"
        );
    }
}

#[test]
fn the_host_records_of_a_tool_line_or_a_refused_commit_only_that_line_s_own_record() {
    let tool_line = |host: &Hearing| (host.tool)(tool_call(3, "toolu_01"));
    let refused_line = |host: &Hearing| (host.refused)(commit_refused(3));

    // Each line's own record is written, and its hook told it is taken.
    let own = a_tool_entry(tool_call(3, "toolu_01"));
    assert_eq!(sent_back_with(&tool_line, &own), (true, vec![own]));
    let own = a_refused_entry(3);
    assert_eq!(sent_back_with(&refused_line, &own), (true, vec![own]));

    let not_the_tool_line = [
        a_tool_entry(tool_call(3, "toolu_02")),
        a_tool_entry(tool_call(4, "toolu_01")),
        a_report_entry(3),
        a_refused_entry(3),
    ];
    for entry in not_the_tool_line {
        assert_eq!(
            sent_back_with(&tool_line, &entry),
            (false, Vec::new()),
            "{entry:?}: not written, and its hook spools the line"
        );
    }
    let not_the_refused_line = [
        a_refused_entry(4),
        a_report_entry(3),
        a_tool_entry(tool_call(3, "toolu_01")),
    ];
    for entry in not_the_refused_line {
        assert_eq!(
            sent_back_with(&refused_line, &entry),
            (false, Vec::new()),
            "{entry:?}: not written, and its hook spools the line"
        );
    }
}

/// A host's hearing whose window is there only while `here` says so, as a runner's desktop comes
/// and goes (#2050).
fn now_and_then(
    window: &Arc<Window>,
    holding: &Arc<Holding>,
    here: &Arc<std::sync::atomic::AtomicBool>,
) -> Hearing {
    now_and_then_with(window, holding, here).0
}

/// [`now_and_then`], with its relaying and the tokens of the chats that are there: 3, 4 and 100
/// to 120.
fn now_and_then_with(
    window: &Arc<Window>,
    holding: &Arc<Holding>,
    here: &Arc<std::sync::atomic::AtomicBool>,
) -> (Hearing, Arc<Relaying>, Arc<ChatTokens>) {
    let heeding = Arc::new(Heeding::new(the_windows(window, holding)));
    heeding.linked(LINK);
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let relaying = Relaying::new({
        let host = Arc::clone(&host);
        let here = Arc::clone(here);
        Box::new(move |id, heard| {
            if !here.load(std::sync::atomic::Ordering::SeqCst) {
                return false;
            }
            let heard: Heard =
                serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
            let host = Arc::clone(&host);
            let reply: Reply = Arc::new(move |replied: Replied| {
                let relaying = host.lock().unwrap().clone().unwrap();
                relaying.replied(id, replied);
            });
            let heeding = Arc::clone(&heeding);
            if heeding.admit(LINK, id, &heard, &reply) {
                std::thread::spawn(move || heeding.hear(LINK, id, heard, reply));
            }
            true
        })
    });
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    let tokens = Arc::new(ChatTokens::default());
    for chat in [3, 4].into_iter().chain(100..=120) {
        tokens.issue(chat).unwrap();
    }
    (relaying.hearing(Arc::clone(&tokens)), relaying, tokens)
}

fn a_browse(chat: u32, call_id: &str, asked: Browsing) -> Ask {
    Ask::Browse(Box::new(BrowseAsk {
        chat,
        browser: None,
        call_id: call_id.into(),
        waited: false,
        asked,
    }))
}

fn a_login(chat: u32, call_id: &str) -> Ask {
    a_browse(
        chat,
        call_id,
        Browsing::LogIn {
            login: "shop-login".into(),
        },
    )
}

/// What `answer` says to its chat, whatever it is.
fn said_of(answer: &Answer) -> String {
    match answer {
        Answer::No { why } => why.clone(),
        Answer::Browsed { browsed, .. } => browsed.note.clone().unwrap_or_default(),
        other => format!("{other:?}"),
    }
}

#[test]
fn a_login_with_the_desktop_away_waits_and_says_so_in_words_that_carry_on() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let host = now_and_then(&window, &holding, &here);

    let answer = (host.answer)(42, a_login(3, "c-1"));

    let said = said_of(&answer);
    assert!(matches!(answer, Answer::No { .. }), "{answer:?}");
    assert!(said.contains("shop-login"), "{said}");
    assert!(
        said.contains("within 15 minutes they are asked about it"),
        "{said}"
    );
    assert!(
        said.contains("nothing is filled unless they allow it"),
        "{said}"
    );
    assert!(said.contains("Go on"), "{said}");
    assert!(window.heard().is_empty(), "nobody heard it");
}

#[test]
fn a_login_that_waited_is_handed_on_once_the_window_hears_again_and_its_chat_is_told() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let host = now_and_then(&window, &holding, &here);
    let _ = (host.answer)(42, a_login(3, "c-1"));

    here.store(true, std::sync::atomic::Ordering::SeqCst);
    (host.each)(report(3)).expect("the window is back and takes the report");
    let start = std::time::Instant::now();
    while !window.heard().contains(&"ask on 42".to_owned()) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            window.heard()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // Its answer is in before the chat's next call is.
    std::thread::sleep(Duration::from_millis(100));
    let next = (host.answer)(43, a_browse(3, "c-2", Browsing::New));

    let said = said_of(&next);
    assert!(said.contains("while the person was away"), "{said}");
    assert!(said.contains("shop-login"), "{said}");
    assert!(
        said.contains("asked afresh about c-1"),
        "handed on as one that waited, and what it came to: {said}"
    );
    assert!(
        said.ends_with("did c-2"),
        "then the call's own answer: {said}"
    );
}

#[test]
fn a_chats_next_call_is_told_of_a_login_that_waited_once_and_another_chat_never() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let host = now_and_then(&window, &holding, &here);
    let _ = (host.answer)(42, a_login(3, "c-1"));
    here.store(true, std::sync::atomic::Ordering::SeqCst);
    (host.each)(report(3)).unwrap();
    let start = std::time::Instant::now();
    while !window.heard().contains(&"ask on 42".to_owned()) {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));

    let other = said_of(&(host.answer)(44, a_browse(4, "c-3", Browsing::New)));
    let first = said_of(&(host.answer)(43, a_browse(3, "c-2", Browsing::New)));
    let again = said_of(&(host.answer)(45, a_browse(3, "c-4", Browsing::New)));

    assert_eq!(other, "did c-3", "another chat is told nothing of it");
    assert!(first.contains("shop-login"), "{first}");
    assert_eq!(again, "did c-4", "told once");
}

#[test]
fn one_chat_cannot_crowd_out_anothers_logins_that_wait() {
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (window, holding) = a_window();
    let host = now_and_then(&window, &holding, &here);
    let login =
        |chat: u32, name: &str| a_browse(chat, "c-1", Browsing::LogIn { login: name.into() });

    for n in 0..MOST_WAITING_FOR_A_CHAT {
        let _ = (host.answer)(40, login(3, &format!("login-{n}")));
    }
    let past = said_of(&(host.answer)(40, login(3, "one-more")));
    let another = said_of(&(host.answer)(41, login(4, "login-0")));

    assert!(past.contains("too many"), "{past}");
    assert!(another.contains("they are asked about it"), "{another}");
}

#[test]
fn the_same_login_asked_again_while_away_waits_once_and_only_so_many_wait() {
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (window, holding) = a_window();
    let host = now_and_then(&window, &holding, &here);

    for n in 0..3 {
        let _ = (host.answer)(40 + n, a_login(3, &format!("c-{n}")));
    }
    let mut last = Answer::No { why: String::new() };
    for chat in 0..(MOST_WAITING as u32 + 1) {
        last = (host.answer)(60, a_login(100 + chat, "c-x"));
    }
    here.store(true, std::sync::atomic::Ordering::SeqCst);
    (host.each)(report(3)).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    let asked = window
        .heard()
        .iter()
        .filter(|heard| heard.starts_with("ask on 4"))
        .count();
    assert_eq!(
        asked,
        1,
        "chat 3's login was handed on once: {:?}",
        window.heard()
    );
    assert!(
        said_of(&last).contains("too many"),
        "one past the bound is refused: {}",
        said_of(&last)
    );
}

#[test]
fn a_hook_lines_own_mark_that_it_waited_is_cleared() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let host = now_and_then(&window, &holding, &here);
    let Ask::Browse(mut browse) = a_login(3, "c-9") else {
        unreachable!()
    };
    browse.waited = true;

    let said = said_of(&(host.answer)(42, Ask::Browse(browse)));

    assert_eq!(said, "did c-9", "answered as any live call");
}

#[test]
fn a_login_that_waited_too_long_is_let_go_of_and_its_chat_told() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (host, relaying, _tokens) = now_and_then_with(&window, &holding, &here);
    let Ask::Browse(browse) = a_login(3, "c-1") else {
        unreachable!()
    };
    let long_ago = Instant::now() - A_HANDED_LINE_WAITS_AT_MOST - Duration::from_secs(1);
    assert!(relaying.waits(42, *browse, long_ago));

    here.store(true, std::sync::atomic::Ordering::SeqCst);
    (host.each)(report(3)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let next = said_of(&(host.answer)(43, a_browse(3, "c-2", Browsing::New)));

    assert!(
        !window.heard().contains(&"ask on 42".to_owned()),
        "never handed on: {:?}",
        window.heard()
    );
    assert!(next.contains("not back within 15 minutes"), "{next}");
}

#[test]
fn a_login_whose_chat_has_gone_is_let_go_of() {
    let (window, holding) = a_window();
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (host, _relaying, tokens) = now_and_then_with(&window, &holding, &here);
    let _ = (host.answer)(42, a_login(3, "c-1"));
    tokens.forget(3);

    here.store(true, std::sync::atomic::Ordering::SeqCst);
    (host.each)(report(4)).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    assert!(
        !window.heard().contains(&"ask on 42".to_owned()),
        "{:?}",
        window.heard()
    );
}

#[test]
fn a_login_that_waited_is_handed_on_once_and_a_window_gone_mid_answer_is_told_not_retried() {
    let here = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let host_slot: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    // A window that takes each line and never answers; then goes.
    let relaying = Relaying::new({
        let (here, handed) = (Arc::clone(&here), Arc::clone(&handed));
        Box::new(move |_, heard| {
            if !here.load(std::sync::atomic::Ordering::SeqCst) {
                return false;
            }
            if matches!(heard, Heard::Ask { .. }) {
                handed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            true
        })
    });
    *host_slot.lock().unwrap() = Some(Arc::clone(&relaying));
    let tokens = Arc::new(ChatTokens::default());
    tokens.issue(3).unwrap();
    let host = relaying.hearing(Arc::clone(&tokens));
    let _ = (host.answer)(42, a_login(3, "c-1"));

    here.store(true, std::sync::atomic::Ordering::SeqCst);
    relaying.hand_on_the_logins_that_waited();
    let start = Instant::now();
    while handed.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    relaying.let_go();
    std::thread::sleep(Duration::from_millis(200));
    relaying.hand_on_the_logins_that_waited();
    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(
        handed.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "handed on once"
    );
    let told = lock(&relaying.told).get(&3).cloned().unwrap_or_default();
    assert!(
        told.iter()
            .any(|said| said.contains("went again before it answered")),
        "{told:?}"
    );
}

#[test]
fn a_line_sent_back_with_no_path_to_a_channel_that_knows_no_project_is_not_recorded() {
    let window = Arc::new(Window::default());
    let kept = Arc::new(Kept::default());
    let (host, _) = relayed_keeping_as(the_windows_carrying(&window), &kept, true, None);

    let refused = (host.each)(report(3));

    assert!(refused.is_err());
    assert!(kept.recorded.lock().unwrap().is_empty());
}
