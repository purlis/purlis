// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;

/// A report of chat `chat`'s, whose harness names no conversation and no process.
fn a_report(chat: u32, event: crate::state::Event) -> crate::hookwire::Report {
    crate::hookwire::Report {
        chat,
        event,
        conversation: crate::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: crate::state::Detail::default(),
    }
}

/// What the host's board keeps, read back as a window takes it.
fn as_a_window_takes(board: &KeptBoard) -> crate::state::Board {
    let mut taken = crate::state::Board::new();
    taken.keep(
        serde_json::from_value(board.whole().expect("a board within an answer"))
            .expect("a whole board"),
    );
    taken
}

#[test]
fn a_line_no_window_took_moves_the_hosts_board_and_one_a_window_took_does_not() {
    use crate::hookwire::relay::Heard;
    use crate::state::{Event, Move, State};
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
    use crate::state::{Move, State};
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
fn place_of(moved: &crate::state::Move) -> usize {
    use crate::state::Move;
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
fn one_of_every_move() -> Vec<(u32, crate::state::Move)> {
    use crate::doing::{Kind, Said};
    use crate::harness::hooked::Asker;
    use crate::hookwire::{Conversation, Speaker};
    use crate::state::{Event, FailedTask, HowFailed, Move, Need, Waits};
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
    let mut window = crate::state::Board::new();
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

#[test]
fn the_socket_sits_beside_the_record_the_app_already_writes() {
    // `.charter/app/` is where M1.7 put `reopen.json`. One place for what the app keeps.
    assert_eq!(
        socket_for(Some(Path::new("/Users/o/plane"))).socket,
        PathBuf::from("/Users/o/plane/.charter/app/hooks.sock")
    );
}

#[test]
fn a_run_outside_any_plane_still_has_somewhere_to_listen() {
    // The channel is the APP's, not the plane's. A chat started outside a plane is still
    // a chat, and answering `unknown` for it because there is no `charter.toml` above it
    // would be a strange rule — it is also how this was found: it made every chat in the
    // scenario tests unknown.
    let socket = socket_for(None).socket;

    assert!(socket.starts_with(private_dir()));
    assert!(socket.as_os_str().len() <= LONGEST_SOCKET_PATH);
}

#[test]
fn a_plane_too_deep_for_a_unix_socket_path_falls_back_instead_of_failing() {
    // macOS allows 104 bytes for the whole path. A plane checked out under a long CI
    // path, or a deeply nested workspace, would otherwise get no event channel at all —
    // and the failure would look like hooks being broken rather than a path being long.
    let deep = PathBuf::from("/Users/operator").join("a".repeat(120));

    let socket = socket_for(Some(&deep)).socket;

    assert!(
        socket.as_os_str().len() <= LONGEST_SOCKET_PATH,
        "{} is {} bytes",
        socket.display(),
        socket.as_os_str().len()
    );
    assert!(socket.starts_with(private_dir()));
}

#[test]
fn the_fallback_prefers_the_runtime_directory_where_the_platform_has_one() {
    // On Linux the temp directory is `/tmp`, which everyone can write to; the standard
    // per-user place for a socket is `$XDG_RUNTIME_DIR`. macOS names no such variable and
    // its own temp directory is already per-user.
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) => assert!(socket_for(None).socket.starts_with(runtime)),
        None => assert!(socket_for(None).socket.starts_with(std::env::temp_dir())),
    }
}

#[test]
fn two_deep_planes_do_not_share_one_socket() {
    let one = PathBuf::from("/Users/operator").join("a".repeat(120));
    let two = PathBuf::from("/Users/operator").join("b".repeat(120));

    assert_ne!(socket_for(Some(&one)).socket, socket_for(Some(&two)).socket);
}

#[test]
fn a_plane_and_no_plane_do_not_share_one_socket() {
    let deep = PathBuf::from("/Users/operator").join("a".repeat(120));

    assert_ne!(socket_for(Some(&deep)).socket, socket_for(None).socket);
}

/// The device the logs here are written for.
const DEVICE: &str = "01J9ZZDEVICE0000000000000";

/// An event log in `dir`, as a host opens it the first time it is needed.
fn a_log_in(dir: &Path) -> EventLog {
    use crate::eventlog::{ArgsKey, Log};
    let dir = dir.to_path_buf();
    EventLog::opening(move || {
        Ok(Recorder::new(
            Log::open(&dir, DEVICE)?,
            ArgsKey::open(&dir)?,
        ))
    })
}

/// Chat `number` of the project at `root` began run `run` in `log`, as a runner begins the run
/// of a chat it composes (ADR 0078, amended 2026-10-18).
fn a_run_begun(log: &EventLog, root: &Path, number: u32, run: &str) {
    use crate::eventlog::{Attributes, Began};
    log.write(Entry::Begin {
        plane: root.to_path_buf(),
        number,
        chat: "01K6W0RKER000000000000000B".into(),
        run: run.into(),
        cause: Began::Start,
        attributes: Attributes::default(),
        held: false,
        trust: None,
    })
    .expect("the run begins");
}

/// Each `hook.stop` in the log in `dir`, by the run it is of.
fn stops_in(dir: &Path) -> Vec<Option<String>> {
    crate::eventlog::read(dir)
        .expect("the log reads")
        .into_iter()
        .filter(|event| event.kind == "hook.stop")
        .map(|event| event.run)
        .collect()
}

#[test]
fn a_hook_line_the_window_sends_back_by_its_own_path_is_recorded_under_the_hosts_project() {
    // A pool's runner holds the project at `<data>/projects/<id>`, and the window names it by
    // its own path on the desktop (#3304): the line is this channel's project's all the same.
    let (data, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let log = a_log_in(data.path());
    let run = "01K6W0RKRUN00000000000000B";
    a_run_begun(&log, project.path(), 1, run);
    let record = hook_lines_recorded(log, project.path());

    record(Entry::Report {
        plane: PathBuf::from("/Users/o/plane"),
        report: a_report(1, crate::state::Event::Stop),
        followed: crate::eventlog::Followed::No,
        begun: None,
    })
    .expect("it is recorded");

    assert_eq!(stops_in(data.path()), vec![Some(run.to_owned())]);
}

#[test]
fn only_a_hook_line_is_recorded_through_what_the_window_sends_back() {
    // A line's reply records that line, and no other write the window could ask of the log.
    let (data, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let record = hook_lines_recorded(a_log_in(data.path()), project.path());

    let refused = record(Entry::Begun {
        plane: project.path().to_path_buf(),
        number: 1,
    });

    assert!(refused.is_err(), "{refused:?}");
    assert!(
        crate::eventlog::read(data.path())
            .expect("the log reads")
            .is_empty()
    );
}

#[test]
fn a_run_a_clear_begins_is_never_under_an_id_the_log_holds_or_no_id_at_all() {
    // The window names the run a `/clear` begins, so its board and the log agree (ADR 0066).
    // An id that is no id, or one the log already holds, is not taken: the run is begun under
    // one the writer mints, and the line is still recorded.
    for named in ["01K6W0RKRUN00000000000000B", "not-an-id"] {
        let (data, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let log = a_log_in(data.path());
        let run = "01K6W0RKRUN00000000000000B";
        a_run_begun(&log, project.path(), 1, run);
        let record = hook_lines_recorded(log, project.path());

        record(Entry::Report {
            plane: project.path().to_path_buf(),
            report: a_report(1, crate::state::Event::SessionStart),
            followed: crate::eventlog::Followed::Moved,
            begun: Some(named.to_owned()),
        })
        .expect("it is recorded");

        let started: Vec<String> = crate::eventlog::read(data.path())
            .expect("the log reads")
            .into_iter()
            .filter(|event| event.kind == "run.started")
            .filter_map(|event| event.run)
            .collect();
        assert_eq!(started.len(), 2, "{named}: {started:?}");
        assert_ne!(started[1], run, "{named}");
        assert_ne!(started[1], named, "{named}");
        assert!(ulid::Ulid::from_string(&started[1]).is_ok(), "{started:?}");
    }
}
