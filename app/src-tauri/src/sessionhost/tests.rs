// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;

#[test]
fn the_window_starts_its_own_binary_as_its_session_host_holding_its_input() {
    let command = command(Path::new(
        "/Applications/purlis.app/Contents/MacOS/charter-app",
    ));
    assert_eq!(
        command.get_program(),
        "/Applications/purlis.app/Contents/MacOS/charter-app"
    );
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["purlis-serve-for-the-window"]);
}

#[test]
fn a_host_that_found_another_serving_leaves_the_window_to_reattach_to_that_one() {
    assert_eq!(not_ready(Some(3)), NotReady::AnotherServes);
    for code in [None, Some(0), Some(1)] {
        assert_eq!(
            not_ready(code),
            NotReady::Gone("the session host ended before it was ready"),
            "{code:?}"
        );
    }
}

/// A report of chat `chat`'s, whose harness names no conversation and no process.
fn a_report(chat: u32, event: purlis_core::state::Event) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

/// What the host's board keeps, read back as a window takes it.
fn as_a_window_takes(board: &KeptBoard) -> purlis_core::state::Board {
    let mut taken = purlis_core::state::Board::new();
    taken.keep(
        serde_json::from_value(board.whole().expect("a board within an answer"))
            .expect("a whole board"),
    );
    taken
}

#[test]
fn a_line_no_window_took_moves_the_hosts_board_and_one_a_window_took_does_not() {
    use purlis_core::hookwire::relay::Heard;
    use purlis_core::state::{Event, Move, State};
    let board = KeptBoard::default();
    board.moved(
        1,
        serde_json::to_value(Move::Opened {
            reports_its_process: false,
            conversation: None,
        })
        .unwrap(),
    );
    let window_there = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let hand_on = board.handing_on({
        let window_there = std::sync::Arc::clone(&window_there);
        std::sync::Arc::new(move |_, _| window_there.load(std::sync::atomic::Ordering::SeqCst))
    });

    assert!(hand_on(
        1,
        &Heard::Report(a_report(1, Event::UserPromptSubmit))
    ));
    assert_eq!(
        as_a_window_takes(&board).state(1),
        State::Unknown,
        "the window's to decide"
    );

    window_there.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(!hand_on(2, &Heard::Report(a_report(1, Event::Stop))));

    let kept = as_a_window_takes(&board);
    assert_eq!(kept.state(1), State::Waiting);
    assert_eq!(kept.needs_you(), [1]);
}

#[test]
fn a_program_that_ends_with_no_window_is_told_to_the_hosts_board_with_its_code() {
    use purlis_core::state::{Move, State};
    let board = KeptBoard::default();
    for chat in [1, 2] {
        board.moved(
            chat,
            serde_json::to_value(Move::Opened {
                reports_its_process: false,
                conversation: None,
            })
            .unwrap(),
        );
    }

    board.ended(1, &session::Exit::Code(0));
    board.ended(2, &session::Exit::Signal("SIGKILL".into()));

    let kept = as_a_window_takes(&board);
    assert_eq!(kept.state(1), State::Done);
    assert_eq!(kept.state(2), State::Failed);
}

#[test]
fn a_move_the_host_cannot_read_moves_nothing() {
    let board = KeptBoard::default();

    assert!(!board.moved(1, serde_json::json!({"move": "something else"})));

    assert!(as_a_window_takes(&board).chats().next().is_none());
}

/// One of every move a board makes, each named by its place here. The match names every
/// variant with no catch-all, so a move added to the core does not compile until it is placed,
/// and the test below fails until it is in [`one_of_every_move`].
fn place_of(moved: &purlis_core::state::Move) -> usize {
    use purlis_core::state::Move;
    match moved {
        Move::Opened { .. } => 0,
        Move::Reported { .. } => 1,
        Move::Rested => 2,
        Move::TaskFailed { .. } => 3,
        Move::FailuresSeen => 4,
        Move::FailureCleared { .. } => 5,
        Move::ChildHeard { .. } => 6,
        Move::Ignored => 7,
        Move::Answered => 8,
        Move::AnsweredBy { .. } => 9,
        Move::ToolSaidBy { .. } => 10,
        Move::ChildToolSaidBy { .. } => 11,
        Move::Exited { .. } => 12,
        Move::ReportedBack { .. } => 13,
        Move::StoppedBelow { .. } => 14,
        Move::Needs { .. } => 15,
        Move::ReportedToItsAsker => 16,
        Move::CommitRefused { .. } => 17,
        Move::Closed => 18,
    }
}

/// How many places [`place_of`] names.
const EVERY_MOVE: usize = 19;

/// One of every move, in an order in which each moves something a reader can see: chat 1 takes
/// them all, and chat 2 is opened only to be closed.
fn one_of_every_move() -> Vec<(u32, purlis_core::state::Move)> {
    use purlis_core::doing::{Kind, Said};
    use purlis_core::harness::hooked::Asker;
    use purlis_core::hookwire::{Conversation, Speaker};
    use purlis_core::state::{Event, FailedTask, HowFailed, Move, Need, Waits};
    let speaker = Speaker {
        pid: None,
        conversation: Conversation::Unknown,
    };
    let began = Said::Began {
        kind: Kind::Command,
        name: Some("cargo".into()),
    };
    vec![
        (
            1,
            Move::Opened {
                reports_its_process: false,
                conversation: Some("0199a2f4-1c2b-7f00-8000-000000000001".into()),
            },
        ),
        (
            2,
            Move::Opened {
                reports_its_process: true,
                conversation: None,
            },
        ),
        (
            1,
            Move::Reported {
                report: a_report(1, Event::UserPromptSubmit),
                waits: Waits::default(),
            },
        ),
        (
            1,
            Move::ToolSaidBy {
                speaker: speaker.clone(),
                said: began.clone(),
            },
        ),
        (
            1,
            Move::ChildHeard {
                agent: "helper-1".into(),
            },
        ),
        (
            1,
            Move::ChildToolSaidBy {
                agent: "helper-1".into(),
                speaker,
                said: began,
            },
        ),
        (
            1,
            Move::Reported {
                report: a_report(1, Event::Notification),
                waits: Waits::default(),
            },
        ),
        (1, Move::AnsweredBy { asker: Asker::Chat }),
        (
            1,
            Move::Reported {
                report: a_report(1, Event::Notification),
                waits: Waits::default(),
            },
        ),
        (1, Move::Answered),
        (
            1,
            Move::Reported {
                report: a_report(1, Event::Stop),
                waits: Waits {
                    tasks: 1,
                    ..Waits::default()
                },
            },
        ),
        (
            1,
            Move::ReportedBack {
                from: "the task".into(),
            },
        ),
        (
            1,
            Move::StoppedBelow {
                from: "another task".into(),
            },
        ),
        (
            1,
            Move::TaskFailed {
                failed: FailedTask::new("task-1", "the task", HowFailed::Failed, "it broke"),
            },
        ),
        (1, Move::FailuresSeen),
        (
            1,
            Move::FailureCleared {
                id: "task-1".into(),
            },
        ),
        (
            1,
            Move::CommitRefused {
                what: "a key in config.toml".into(),
            },
        ),
        (
            1,
            Move::Needs {
                need: Need::RootUpdateRefused,
            },
        ),
        (1, Move::ReportedToItsAsker),
        (1, Move::Ignored),
        (1, Move::Rested),
        (1, Move::Exited { code: Some(0) }),
        (2, Move::Closed),
    ]
}

/// A whole board as JSON, without when each chat last moved: that is each process's own count.
fn without_counts(mut whole: serde_json::Value) -> serde_json::Value {
    if let Some(chats) = whole
        .get_mut("chats")
        .and_then(serde_json::Value::as_object_mut)
    {
        for chat in chats.values_mut() {
            if let Some(chat) = chat.as_object_mut() {
                chat.remove("moved_at");
            }
        }
    }
    whole
}

#[test]
fn every_move_the_window_tells_is_read_and_made_by_the_hosts_board() {
    let moves = one_of_every_move();
    let placed: std::collections::BTreeSet<usize> =
        moves.iter().map(|(_, moved)| place_of(moved)).collect();
    assert_eq!(placed.len(), EVERY_MOVE, "one of every move is told");

    let host = KeptBoard::default();
    let mut window = purlis_core::state::Board::new();
    for (chat, moved) in moves {
        let said = serde_json::to_value(&moved).expect("a move is said as JSON");
        assert!(host.moved(chat, said), "the host reads {moved:?}");
        window.make(chat, moved);
    }

    assert_eq!(
        without_counts(host.whole().expect("a board within an answer")),
        without_counts(serde_json::to_value(window.kept()).expect("said")),
    );
}

// ----- the supervisor and its breaker (#2007) ---------------------------------------------

/// A host that ends at once, as one that crashes on its way up does, writing a line to its
/// log and a line to `started` each time it is started.
fn a_host_that_crashes_as_it_starts(
    started: &Path,
) -> impl Fn() -> Result<Command, String> + Send + Sync + use<> {
    let started = started.to_path_buf();
    move || {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("echo once >> \"$1\"; echo 'purlisd: it went wrong' >&2; exit 1")
            .arg("sh")
            .arg(&started)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(command)
    }
}

/// A place of a host's own, in `home`.
fn a_place(home: &Path) -> purlis_host::Place {
    let run = home.join("run");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    purlis_host::Place::at(run.join("purlisd.sock"), home.join("credentials"))
}

fn starts_in(started: &Path) -> usize {
    std::fs::read_to_string(started)
        .map(|said| said.lines().count())
        .unwrap_or(0)
}

fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_host_that_keeps_ending_is_started_again_until_three_exits_trip_the_breaker() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();

    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );

    until("the breaker trips", || failing.now().is_some());
    assert_eq!(
        starts_in(&started),
        breaker::EXITS,
        "started once more after each exit"
    );
    let said = failing.now().unwrap();
    assert_eq!((said.exits, said.within_minutes), (3, 5));
    assert!(
        said.said
            .iter()
            .any(|line| line == "purlisd: it went wrong"),
        "{:?}",
        said.said
    );
    let refused = host
        .connection()
        .wait(Duration::ZERO)
        .err()
        .unwrap_or_default();
    assert!(refused.contains("stopped starting it again"), "{refused}");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        starts_in(&started),
        breaker::EXITS,
        "started after the breaker tripped"
    );
    host.end();
}

#[test]
fn try_again_starts_a_failing_host_once_more_with_the_count_started_over() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();
    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );
    until("the breaker trips", || failing.now().is_some());

    failing.try_again().expect("tried again");

    until("it trips again", || {
        starts_in(&started) == 2 * breaker::EXITS && failing.now().is_some()
    });
    host.end();
}

#[test]
fn a_host_the_window_ends_is_never_started_again() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    host.end();

    host.start_at(
        a_place(home.path()),
        a_host_that_crashes_as_it_starts(&started),
    );
    std::thread::sleep(Duration::from_millis(500));

    assert_eq!(starts_in(&started), 0);
}

/// A host that finds another serving leaves the window to reattach to that one (#2006), never
/// to be started again while its link holds. One that cannot be reached counts as an
/// unexpected end, so a host that is never reached trips the breaker rather than looping.
#[test]
fn a_host_that_found_another_serving_is_reattached_to_and_one_never_reached_trips_the_breaker() {
    let home = tempfile::tempdir().unwrap();
    let started = home.path().join("started");
    let host = SessionHost::default();
    let failing = host.failing();
    let another_serves = {
        let started = started.clone();
        move || {
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg("echo once >> \"$1\"; exit 3")
                .arg("sh")
                .arg(&started)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Ok(command)
        }
    };

    host.start_at(a_place(home.path()), another_serves);
    until("the breaker trips", || failing.now().is_some());

    assert_eq!(starts_in(&started), breaker::EXITS);
    host.end();
}

#[test]
fn a_host_started_as_the_window_ends_it_is_ended_and_not_left_running() {
    // The window's end lands after the supervisor looked and before the host it is starting is
    // held: the end finds nothing to end, so the start must end it.
    let home = tempfile::tempdir().unwrap();
    let pids = home.path().join("pids");
    let host = SessionHost::default();
    let ender = host.ender();
    let starts = {
        let pids = pids.clone();
        move || {
            ender();
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg("echo $$ >> \"$1\"; echo ready; exec cat")
                .arg("sh")
                .arg(&pids)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            Ok(command)
        }
    };

    host.start_at(a_place(home.path()), starts);

    until("the host is started", || starts_in(&pids) == 1);
    let pid: u32 = std::fs::read_to_string(&pids)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    until("the host it started is ended", || {
        !purlis_core::process::alive(pid)
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        starts_in(&pids),
        1,
        "started again after the window ended it"
    );
}
