// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A board holding chat `session`, which has stopped and is asking for you.
fn asking(session: u32) -> Hooks {
    // Spelled the way a window hands one back; only the registry mints one for real.
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::deaf(plane);
    hooks.board().opened(session, None, None);
    assert!(
        apply(
            &hooks.board,
            &hooks.plane,
            &stop(session),
            purlis_core::state::Waits::default()
        )
        .moved
        .is_some()
    );
    hooks
}

fn stop(session: u32) -> Report {
    Report {
        chat: session,
        event: purlis_core::state::Event::Stop,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    }
}

fn said(session: u32, event: purlis_core::state::Event) -> Report {
    Report {
        event,
        ..stop(session)
    }
}

/// The pid and the conversation of the Claude Code run [`adopted`] reports come from.
const OUR_PID: u32 = 4242;
const OUR_CONVERSATION: &str = "11111111-2222-4333-8444-555555555555";

/// A report of the Claude Code run a chat opened by [`claude_code_chat`] adopts.
fn adopted(session: u32, event: purlis_core::state::Event) -> Report {
    Report {
        pid: Some(OUR_PID),
        conversation: purlis_core::hookwire::Conversation::Named(OUR_CONVERSATION.to_owned()),
        ..said(session, event)
    }
}

/// The run a tool line names: the adopted one's, or another's (#1601).
fn speaker(pid: u32, conversation: &str) -> purlis_core::hookwire::Speaker {
    purlis_core::hookwire::Speaker {
        pid: Some(pid),
        conversation: purlis_core::hookwire::Conversation::Named(conversation.to_owned()),
    }
}

fn ours() -> purlis_core::hookwire::Speaker {
    speaker(OUR_PID, OUR_CONVERSATION)
}

/// Hooks holding Claude Code chat `session`, its run adopted, at work on a turn.
fn claude_code_chat(session: u32) -> Hooks {
    use purlis_core::state::{Event, Waits};
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::deaf(plane);
    hooks.board().opened(
        session,
        Some(purlis_core::harness::Harness::ClaudeCode),
        Some(OUR_CONVERSATION.to_owned()),
    );
    for event in [Event::SessionStart, Event::UserPromptSubmit] {
        apply(
            &hooks.board,
            &hooks.plane,
            &adopted(session, event),
            Waits::default(),
        );
    }
    hooks
}

/// Hooks holding chat `session` stopped, mid-turn, on its harness's prompt.
fn stopped_on_its_prompt(session: u32) -> (Hooks, Moved) {
    use purlis_core::state::{Event, Waits};
    let hooks = claude_code_chat(session);
    let asked = apply(
        &hooks.board,
        &hooks.plane,
        &adopted(session, Event::Notification),
        Waits::default(),
    )
    .moved
    .expect("asking is a move");
    (hooks, asked)
}

#[test]
fn the_window_is_told_a_chat_is_stopped_on_its_prompt_until_its_turn_ends() {
    // #1601: past purlis's own hold of a permission prompt (about a minute), the window
    // still draws the task as waiting on the person, from the board.
    use purlis_core::state::{Event, Waits};
    let (hooks, asked) = stopped_on_its_prompt(7);
    assert_eq!(
        (asked.state.as_str(), asked.asking),
        ("waiting", Some(true))
    );
    // A window that opens now is told the same.
    assert_eq!(now(&hooks.board, hooks.plane.clone(), 7).asking, Some(true));

    // The turn ends: still waiting, still in the queue, and no longer on its prompt. The
    // window is told, and nothing interrupts the person for it.
    let ended = apply(
        &hooks.board,
        &hooks.plane,
        &adopted(7, Event::Stop),
        Waits::default(),
    )
    .moved
    .expect("the end of the wait is a move");
    assert_eq!((ended.state.as_str(), ended.asking), ("waiting", None));
    assert!(!ended.interrupts());
}

#[test]
fn a_tool_of_its_own_that_came_back_takes_a_chat_past_its_prompt() {
    // #1601: the person answered the prompt in the task's own pane, which no hook says.
    use purlis_core::doing::{Kind, Said};
    let (hooks, _) = stopped_on_its_prompt(7);
    let ours = ours();
    let past = |said: Said| {
        got_past_its_prompt(
            &hooks.board,
            &hooks.plane,
            ToolLine {
                chat: 7,
                agent: None,
                speaker: &ours,
            },
            &said,
        )
    };

    // A tool at work when the chat asked, run beside the asked call, comes back whatever
    // the person does; a helper back is not the chat's own answer. Neither moves it.
    assert!(
        past(Said::Ended {
            kind: Some(Kind::Command)
        })
        .is_none()
    );
    assert!(
        past(Said::Ended {
            kind: Some(Kind::Helper)
        })
        .is_none()
    );
    // A tool about to run may be the call being asked about: no move of its own.
    assert!(
        past(Said::Began {
            kind: Kind::Command,
            name: None
        })
        .is_none()
    );
    assert!(hooks.board().waits_on_its_prompt(7));

    // One of its own that comes back after it: the turn went on past the prompt.
    let moved = past(Said::Ended {
        kind: Some(Kind::Command),
    })
    .expect("a move");
    assert_eq!(
        (moved.state.as_str(), moved.asking, moved.needs_you),
        ("running", None, false)
    );
    // Once: a chat at work is past nothing.
    assert!(
        past(Said::Ended {
            kind: Some(Kind::Command)
        })
        .is_none()
    );
    // And a chat the board does not have moves nowhere.
    assert!(
        got_past_its_prompt(
            &hooks.board,
            &hooks.plane,
            ToolLine {
                chat: 9,
                agent: None,
                speaker: &ours,
            },
            &Said::Ended { kind: None }
        )
        .is_none()
    );
}

#[test]
fn a_helper_s_tools_take_only_its_own_prompt_and_the_chat_s_take_none_of_it() {
    // #1644: a background helper asked while the chat works on beside it.
    use purlis_core::doing::{Kind, Said};
    use purlis_core::state::{Event, Waits};
    let hooks = claude_code_chat(7);
    let helper = Report {
        agent: Some("a1".to_owned()),
        ..adopted(7, Event::Notification)
    };
    apply(&hooks.board, &hooks.plane, &helper, Waits::default());
    assert!(hooks.board().waits_on_its_prompt(7));
    let began = Said::Began {
        kind: Kind::Command,
        name: None,
    };
    let back = Said::Ended {
        kind: Some(Kind::Command),
    };
    let ours = ours();
    let past = |agent: Option<&str>, said: &Said| {
        got_past_its_prompt(
            &hooks.board,
            &hooks.plane,
            ToolLine {
                chat: 7,
                agent,
                speaker: &ours,
            },
            said,
        )
    };

    // The chat's own tools say nothing of the helper's prompt.
    past(None, &began);
    assert!(past(None, &back).is_none());
    assert!(hooks.board().waits_on_its_prompt(7));

    // The helper's own, past its prompt, do.
    assert!(past(Some("a1"), &began).is_none());
    let moved = past(Some("a1"), &back).expect("a move");
    assert_eq!((moved.state.as_str(), moved.asking), ("running", None));
}

#[test]
fn a_tool_line_from_another_run_in_the_chat_moves_nothing() {
    // #1601: a harness nested in the chat, or a job it started, holds the chat's token and
    // sits in its process tree. Its tool lines say nothing of the chat's prompt, or of a
    // helper's; the adopted run's own still do.
    use purlis_core::doing::{Kind, Said};
    use purlis_core::state::{Event, Waits};
    let (hooks, _) = stopped_on_its_prompt(7);
    let helper = Report {
        agent: Some("a1".to_owned()),
        ..adopted(7, Event::Notification)
    };
    apply(&hooks.board, &hooks.plane, &helper, Waits::default());
    let ran = |agent: Option<&str>, speaker: &purlis_core::hookwire::Speaker| {
        let line = ToolLine {
            chat: 7,
            agent,
            speaker,
        };
        let began = Said::Began {
            kind: Kind::Command,
            name: None,
        };
        let back = Said::Ended {
            kind: Some(Kind::Command),
        };
        let a = got_past_its_prompt(&hooks.board, &hooks.plane, line, &began);
        let b = got_past_its_prompt(&hooks.board, &hooks.plane, line, &back);
        a.or(b)
    };
    let nested = speaker(OUR_PID + 1, "22222222-3333-4444-8555-666666666666");
    let no_one = purlis_core::hookwire::Speaker::default();
    for other in [&nested, &speaker(OUR_PID + 1, OUR_CONVERSATION), &no_one] {
        assert!(ran(None, other).is_none(), "{other:?}");
        assert!(ran(Some("a1"), other).is_none(), "{other:?}");
        assert!(hooks.board().waits_on_its_prompt(7), "{other:?}");
    }
    // The adopted run's own lines: the chat's prompt, then the helper's.
    let ours = ours();
    assert!(ran(None, &ours).is_none(), "the helper still asks");
    let moved = ran(Some("a1"), &ours).expect("a move");
    assert_eq!((moved.state.as_str(), moved.asking), ("running", None));
}

#[test]
fn a_report_taken_before_a_close_is_numbered_before_it() {
    // charter-app#248, the race #256 left open: a report's `Moved` is sent on the thread
    // that read it, after the board is let go, so it can reach the window AFTER a close
    // that came later. The number is what lets the window tell which is newer.
    let hooks = asking(7);
    let late = apply(
        &hooks.board,
        &hooks.plane,
        &stop(7),
        purlis_core::state::Waits::default(),
    )
    .moved;
    hooks.board().ignored(7);
    let report = apply(
        &hooks.board,
        &hooks.plane,
        &stop(7),
        purlis_core::state::Waits::default(),
    )
    .moved
    .expect("a new request");

    let close = hooks.closed(7);

    assert!(
        late.is_none(),
        "a stop on a chat already asking moved nothing"
    );
    assert_eq!(report.queue, vec![7]);
    assert!(close.queue.is_empty());
    assert!(
        report.sequence < close.sequence,
        "the close ({}) is not numbered after the report it follows ({})",
        close.sequence,
        report.sequence
    );
}

#[test]
fn every_snapshot_is_numbered_after_the_one_before_it() {
    let hooks = asking(7);

    let first = hooks.now(7);
    let second = hooks.now(7);

    assert!(first.sequence < second.sequence);
}

#[test]
fn ignoring_a_chat_tells_a_queue_without_it_and_the_chat_still_waiting() {
    let hooks = asking(7);
    let asked = hooks.now(7);

    let ignored = hooks.ignored(7);

    assert!(ignored.queue.is_empty());
    assert!(!ignored.needs_you);
    assert_eq!(ignored.state, "waiting");
    assert!(ignored.sequence > asked.sequence);
}

#[test]
fn a_harness_started_by_hand_in_a_shell_tab_is_told_to_the_window_with_its_plane() {
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let hooks = Hooks::listening_on(
        plane.clone(),
        &at,
        Arc::new(|_| panic!("a harness started by hand moves no chat")),
        Arc::new(move |told| tx.lock().unwrap().send(told).unwrap()),
    )
    .expect("listening");

    purlis_core::hookwire::tell(
        hooks.socket().expect("a socket"),
        Some(&hooks.token_for(3)),
        &StartedByHand {
            chat: 3,
            started_by_hand: "codex".to_owned(),
            cwd: Some(PathBuf::from("/work/alpha")),
        },
    )
    .expect("told");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok(ByHand {
            plane,
            session: 3,
            harness: "codex".to_owned(),
            cwd: Some("/work/alpha".to_owned()),
        })
    );
}

#[cfg(unix)]
#[test]
fn a_permission_prompt_is_told_to_the_window_and_the_window_s_answer_goes_back_on_its_hook() {
    // HP-6: Claude Code's `PermissionRequest` hook, on a project's own socket, becomes an
    // ask the window is told; the window answers it, and the hook hears the option.
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::listening_on(plane.clone(), &at, Arc::new(|_| {}), Arc::new(|_| {}))
        .expect("listening");
    let (tx, told) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    hooks.tell_asks_to(Arc::new(move |asking| {
        tx.lock().unwrap().send(asking).unwrap()
    }));

    let hook = {
        let socket = hooks.socket().expect("a socket").to_path_buf();
        let token = hooks.token_for(3);
        std::thread::spawn(move || {
            // In the host's own process, which no ancestry check could admit: the
            // exchange alone, as a hook makes it once it has admitted its host.
            purlis_core::hookwire::permission::ask_permission_of_an_admitted_host(
                std::os::unix::net::UnixStream::connect(&socket).expect("connects"),
                Some(&token),
                &purlis_core::hookwire::PermissionAsked {
                    chat: 3,
                    permission_request: purlis_core::harness::hooked::Source::ClaudeCode,
                    payload: serde_json::json!({"tool_name": "Bash",
                            "tool_input": {"command": "npm test"}}),
                    ran_for_ms: None,
                },
                std::time::Duration::from_secs(30),
            )
        })
    };
    let asking = told
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the window is told");
    assert_eq!(asking.plane, plane);
    let [shown] = asking.asks.as_slice() else {
        panic!("one ask: {asking:?}");
    };
    assert_eq!(shown.session, 3);
    assert!(shown.says.contains("npm test"), "{}", shown.says);
    let labels: Vec<&str> = shown.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels, ["Allow", "Deny"]);

    assert_eq!(
        hooks.answer(4, &shown.ask, "allow"),
        Err("no such ask is waiting".to_owned()),
        "another chat's answer never lands on it"
    );
    hooks
        .answer(3, &shown.ask, "allow")
        .expect("the window's answer applies");

    assert_eq!(
        hook.join().expect("the hook").expect("a reply"),
        Some("allow".to_owned())
    );
    assert!(hooks.answer(3, &shown.ask, "allow").is_err(), "once");
}

#[test]
fn every_hook_call_the_channel_hears_is_one_event_in_the_hosts_log() {
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    hooks.board().opened(3, Some(Harness::ClaudeCode), None);
    let socket = hooks.socket().expect("a socket");
    let token = hooks.token_for(3);

    purlis_core::hookwire::send(
        socket,
        Some(&token),
        &Report {
            chat: 3,
            event: purlis_core::state::Event::UserPromptSubmit,
            conversation: Default::default(),
            pid: None,
            agent: None,
            detail: Default::default(),
        },
    )
    .expect("sent");
    // Each line is its own connection; the second is sent once the first is in the log.
    let logged = |at_least: usize| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let kinds: Vec<String> = eventlog::read(&logs)
                .expect("the log reads")
                .into_iter()
                .map(|event| event.kind)
                .collect();
            if kinds.len() >= at_least || std::time::Instant::now() > until {
                break kinds;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    assert_eq!(logged(2).len(), 2, "the report is in the log");
    purlis_core::hookwire::deliver_tool(
        socket,
        Some(&token),
        &purlis_core::hookwire::ToolCall {
            chat: 3,
            tool_hook: "pretooluse".to_owned(),
            tool: Some("Bash".to_owned()),
            call: Some("toolu_1".to_owned()),
            args: Some(eventlog::args_hash(&serde_json::json!({"command": "ls"}))),
            decision: purlis_core::hookwire::Decision::None,
            rule: None,
            hook_ms: 1,
            agent: None,
            at_ms: 0,
            skill: None,
        },
    )
    .expect("told");

    let kinds = logged(3);
    assert_eq!(
        kinds,
        vec!["run.started", "hook.userpromptsubmit", "hook.pretooluse"],
        "one event per hook call, after the run it is under"
    );
}

#[test]
fn recorded_violation_lines_reach_the_window_as_the_notice_says_them() {
    use purlis_core::hookwire::SandboxBlocked;
    use purlis_core::sandboxblock::{Place, detect};
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    // What Claude Code handed `posttoolusefailure-blocked` for a smart close from a clone.
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "purlis session record --title close"},
        "error": "Exit code 1\n<sandbox_violations>\npurlis(48211) deny(1) \
                  file-write-create /plane/workspaces/alpha/sessions/close.md\n\
                  </sandbox_violations>",
    });
    let place = Place {
        root: Path::new("/plane"),
        chat: Path::new("/plane/workspaces/alpha/repo"),
        cwd: Path::new("/plane/workspaces/alpha/repo"),
        home: None,
    };
    let told: Vec<ChatBlocked> = detect(&payload, &place)
        .into_iter()
        .map(|block| {
            blocked(
                &plane,
                &SandboxBlocked {
                    chat: 3,
                    sandbox_blocked: block,
                    harness: Some("claude".to_owned()),
                    target: None,
                    read_back: false,
                    suggested: None,
                }
                .into(),
            )
        })
        .collect();
    assert_eq!(
        told,
        vec![ChatBlocked {
            plane: plane.clone(),
            session: 3,
            operation: "write".to_owned(),
            kind: "project-files".to_owned(),
            ours: true,
            harness: Some("claude".to_owned()),
            said: "a write to the project's own files".to_owned(),
            // purlis's own: a bug to report, never something to allow.
            offer: BlockOffer::None,
            target: None,
            route: None,
            levels: Vec::new(),
            held: false,
            ruled: None,
            vault: None,
            choose: None,
            quiet: None,
            suggested: None,
        }]
    );
    // A harness word purlis does not start is no harness.
    let other = blocked(
        &plane,
        &SandboxBlocked {
            chat: 3,
            sandbox_blocked: told_block(),
            harness: Some("/home/dev/bin/thing".to_owned()),
            target: None,
            read_back: false,
            suggested: None,
        }
        .into(),
    );
    assert_eq!(other.harness, None);
}

#[test]
fn a_heard_block_is_told_once_and_a_flood_is_held_back() {
    use purlis_core::hookwire::SandboxBlocked;
    let throttle = Mutex::new(purlis_core::sandboxblock::Throttle::default());
    let told: Arc<Mutex<Vec<HeardBlock>>> = Arc::new(Mutex::new(Vec::new()));
    let slot: Mutex<Option<HeardBlocks>> = Mutex::new(Some({
        let told = Arc::clone(&told);
        Arc::new(move |block| told.lock().unwrap().push(block))
    }));
    let now = std::time::Instant::now();
    let line = SandboxBlocked {
        chat: 3,
        sandbox_blocked: told_block(),
        harness: None,
        target: None,
        read_back: false,
        suggested: None,
    };
    for _ in 0..50 {
        heard_block(
            &throttle,
            &slot,
            &Mutex::new(None),
            line.clone().into(),
            now,
        );
    }
    assert_eq!(
        *told.lock().unwrap(),
        vec![line.clone().into()],
        "one, however often it is sent"
    );
}

/// #1681: the repeats the throttle held back are told for the record once their minute is
/// over, at the next block heard, and never shown.
#[test]
fn the_repeats_held_back_are_told_once_their_minute_is_over() {
    use purlis_core::hookwire::SandboxBlocked;
    use purlis_core::sandboxblock::{Repeated, THROTTLE_WINDOW};
    let throttle = Mutex::new(purlis_core::sandboxblock::Throttle::default());
    let told: Arc<Mutex<Vec<HeardBlock>>> = Arc::default();
    let slot: Mutex<Option<HeardBlocks>> = Mutex::new(Some({
        let told = Arc::clone(&told);
        Arc::new(move |block| told.lock().unwrap().push(block))
    }));
    let counted: Arc<Mutex<Vec<Repeated>>> = Arc::default();
    let repeated: Mutex<Option<Repeats>> = Mutex::new(Some({
        let counted = Arc::clone(&counted);
        Arc::new(move |over| counted.lock().unwrap().extend(over))
    }));
    let now = std::time::Instant::now();
    let line = SandboxBlocked {
        chat: 3,
        sandbox_blocked: told_block(),
        harness: None,
        target: None,
        read_back: false,
        suggested: None,
    };
    for _ in 0..4 {
        heard_block(&throttle, &slot, &repeated, line.clone().into(), now);
    }
    assert!(counted.lock().unwrap().is_empty(), "its minute runs");
    let other = SandboxBlocked {
        chat: 4,
        ..line.clone()
    };
    heard_block(
        &throttle,
        &slot,
        &repeated,
        other.into(),
        now + THROTTLE_WINDOW,
    );
    let counted = counted.lock().unwrap();
    assert_eq!(
        counted
            .iter()
            .map(|one| (one.chat, one.times))
            .collect::<Vec<_>>(),
        [(3, 3)]
    );
    assert_eq!(told.lock().unwrap().len(), 2, "a repeat is never shown");
}

/// qw146 follow-up: a chat's held-back repeats are told as it ends, and every one left when
/// the project is let go of, not left for the next block the project hears.
#[test]
fn held_back_repeats_are_told_as_a_chat_ends_and_as_the_project_is_let_go_of() {
    use purlis_core::sandboxblock::Repeated;
    let plane: PlaneId = serde_json::from_value(serde_json::json!("/tmp/repeats")).unwrap();
    let hooks = Hooks::deaf(plane);
    let counted: Arc<Mutex<Vec<Repeated>>> = Arc::default();
    hooks.when_repeated({
        let counted = Arc::clone(&counted);
        Arc::new(move |over| counted.lock().unwrap().extend(over))
    });
    let now = std::time::Instant::now();
    {
        let mut throttle = hooks.throttle.lock().unwrap();
        for chat in [7, 7, 7, 8, 8] {
            throttle.lets(chat, &told_block(), now);
        }
    }
    let said = |counted: &Mutex<Vec<Repeated>>| -> Vec<(u32, u64)> {
        counted
            .lock()
            .unwrap()
            .iter()
            .map(|one| (one.chat, one.times))
            .collect()
    };
    hooks.chat_ended(7);
    assert_eq!(said(&counted), [(7, 2)]);
    hooks.stop();
    assert_eq!(said(&counted), [(7, 2), (8, 1)]);
}

/// A host a brokered `secret exec`'s sandbox refused reaches the asking chat's Notice as its
/// own block does: Allow, naming the host and port the proxy refused whole, at every level
/// a host may be kept at; and it is held for the chat, so that Allow answers it (#1538).
/// It names the vault the run was given and the chat's harness (#1353), and the throttle
/// keys it on the host it names, never on the vault.
#[test]
fn a_host_a_brokered_run_was_refused_is_the_asking_chats_block_to_allow() {
    use purlis_core::secrets::brokered::RunBlocked;
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let told: Arc<Mutex<Vec<HeardBlock>>> = Arc::default();
    let throttle = Arc::new(Mutex::new(purlis_core::sandboxblock::Throttle::default()));
    let slot: Arc<Mutex<Option<HeardBlocks>>> = Arc::new(Mutex::new(Some({
        let told = Arc::clone(&told);
        Arc::new(move |block| told.lock().unwrap().push(block))
    })));
    let hear: HeardBlocks = {
        let (throttle, slot) = (Arc::clone(&throttle), Arc::clone(&slot));
        Arc::new(move |block| {
            heard_block(
                &throttle,
                &slot,
                &Mutex::new(None),
                block,
                std::time::Instant::now(),
            );
        })
    };
    let run = run_told(Arc::clone(&hear));
    // What the core tells for chat 7's run, from its proxy's record.
    let line = purlis_core::hookwire::SandboxBlocked {
        chat: 7,
        sandbox_blocked: told_block(),
        harness: Some("claude".to_owned()),
        target: Some("api.cluster.example-k8s.com:6443".to_owned()),
        read_back: false,
        suggested: None,
    };
    let with = |line: &purlis_core::hookwire::SandboxBlocked, vault: &str| RunBlocked {
        block: line.clone(),
        vault: vault.to_owned(),
    };
    run(with(&line, "team"));
    let first = HeardBlock {
        line: line.clone(),
        vault: Some("team".to_owned()),
    };
    assert_eq!(*told.lock().unwrap(), vec![first.clone()]);
    // Another host within the minute has its own Notice; the same one again does not, with
    // another vault or with none.
    let other = purlis_core::hookwire::SandboxBlocked {
        target: Some("registry.example.com:443".to_owned()),
        ..line.clone()
    };
    run(with(&other, "team"));
    run(with(&line, "prod"));
    hear(line.clone().into());
    assert_eq!(
        *told.lock().unwrap(),
        vec![
            first.clone(),
            HeardBlock {
                line: other,
                vault: Some("team".to_owned()),
            }
        ]
    );
    let said = blocked(&plane, &first);
    assert_eq!(said.session, 7);
    assert_eq!(said.offer, BlockOffer::Host);
    assert_eq!(
        said.target.as_deref(),
        Some("api.cluster.example-k8s.com:6443")
    );
    assert_eq!(said.vault.as_deref(), Some("team"));
    assert_eq!(said.harness.as_deref(), Some("claude"));
    assert!(
        said.levels.contains(&crate::sandboxing::GrantLevel::Chat),
        "{:?}",
        said.levels
    );
    assert!(
        said.levels.len() > 1,
        "Always allow is offered too: {:?}",
        said.levels
    );
    // A chat's own line names no vault, whatever it sends.
    assert_eq!(blocked(&plane, &line.clone().into()).vault, None);
}

/// #1353: a vault's name is drawn as an ask's line is: one line, every credential shape
/// masked, and nothing in it that draws as nothing.
#[test]
fn a_vault_s_name_on_a_notice_is_drawn_inertly() {
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let line = purlis_core::hookwire::SandboxBlocked {
        chat: 7,
        sandbox_blocked: told_block(),
        harness: None,
        target: Some("api.example.com:443".to_owned()),
        read_back: false,
        suggested: None,
    };
    let said = |vault: &str| {
        blocked(
            &plane,
            &HeardBlock {
                line: line.clone(),
                vault: Some(vault.to_owned()),
            },
        )
        .vault
        .expect("a vault")
    };
    assert_eq!(said("team"), "team");
    let turned = said("team\u{202e}dorp");
    assert!(!turned.contains('\u{202e}'), "{turned}");
    assert!(turned.contains("⟦U+202E⟧"), "{turned}");
    assert!(!said("a\nb").contains('\n'));
}

/// #2423: a connection a program made straight to an internet address, past purlis's proxy,
/// raises a quiet line that names the address, offers no Allow, and says the way round.
#[test]
fn an_address_dialled_past_the_proxy_is_a_quiet_line_that_names_it_and_offers_no_allow() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let said = blocked(
        &plane,
        &purlis_core::hookwire::SandboxBlocked {
            chat: 3,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Address,
                ours: false,
            },
            harness: Some("codex".to_owned()),
            target: Some("93.184.216.34:443".to_owned()),
            read_back: false,
            suggested: None,
        }
        .into(),
    );
    assert_eq!(said.offer, BlockOffer::Unsandboxed);
    assert_eq!(said.target.as_deref(), Some("93.184.216.34:443"));
    assert_eq!(said.choose, None);
    let route = said.route.expect("the way round");
    assert!(route.contains("through the sandbox's proxy"), "{route}");
    assert!(said.quiet.is_some(), "nothing waits on it");
    // Never an address that is no address: the line names nothing then.
    let said = blocked(
        &plane,
        &purlis_core::hookwire::SandboxBlocked {
            chat: 3,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Address,
                ours: false,
            },
            harness: Some("codex".to_owned()),
            target: Some("example.com:443".to_owned()),
            read_back: false,
            suggested: None,
        }
        .into(),
    );
    assert_eq!(said.target, None);
}

#[test]
fn a_block_s_notice_offers_allow_only_for_a_host_or_a_folder_and_the_way_out_otherwise() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let told = |operation, kind, ours, target: Option<&str>| {
        let said = blocked(
            &plane,
            &purlis_core::hookwire::SandboxBlocked {
                chat: 3,
                sandbox_blocked: Block {
                    operation,
                    kind,
                    ours,
                },
                harness: None,
                target: target.map(str::to_owned),
                read_back: false,
                suggested: None,
            }
            .into(),
        );
        (said.offer, said.target, said.route.is_some())
    };
    let host = |target| told(Operation::Connect, Kind::Host, false, target);
    // A host, named where the report named one, typed by the person where it did not.
    assert_eq!(
        host(Some("API.example.com:443")),
        (
            BlockOffer::Host,
            Some("api.example.com:443".to_owned()),
            false
        )
    );
    assert_eq!(host(None), (BlockOffer::Host, None, false));
    // Never proposed: a host the sandbox never lets a chat reach, and a wildcard a line
    // named (a real refusal names one host).
    for target in ["169.254.169.254", "*.amazonaws.com"] {
        assert_eq!(
            host(Some(target)),
            (BlockOffer::Host, None, false),
            "{target}"
        );
    }
    // A write in the project, on the folder it was refused in. A fixture made in a
    // harness's own temp root (an agent's `/private/tmp/claude-<uid>`) is refused with it:
    // that refusal is never lifted for a project that lives there.
    let other = root.join("workspaces/beta/out");
    let in_a_harness_temp_root = root
        .strip_prefix("/private/tmp")
        .ok()
        .and_then(|below| below.components().next())
        .is_some_and(|first| first.as_os_str().to_string_lossy().starts_with("claude-"));
    let (offer, target, why) = told(
        Operation::Write,
        Kind::ProjectFiles,
        false,
        Some(&other.join("x.lock").display().to_string()),
    );
    assert_eq!(target, Some(other.display().to_string()));
    if in_a_harness_temp_root {
        assert_eq!((offer, why), (BlockOffer::Unsandboxed, true));
    } else {
        assert_eq!((offer, why), (BlockOffer::Write, false));
    }
    // Off the allowlist (D-1342-10): no Allow, and the chat without the sandbox instead.
    let home = std::env::var("HOME").expect("a home");
    let (offer, _, why) = told(
        Operation::Write,
        Kind::Home,
        false,
        Some("/opt/tool/cache/x.lock"),
    );
    assert_eq!((offer, why), (BlockOffer::Unsandboxed, true));
    // What a later program runs at the person's own level is never granted since #2071, and
    // the way that works is said: the person makes the change themselves.
    for path in [
        format!("{home}/Library/LaunchAgents/x.plist"),
        format!("{home}/.config/git/config"),
    ] {
        let (offer, _, why) = told(Operation::Write, Kind::Home, false, Some(&path));
        assert_eq!((offer, why), (BlockOffer::Brokered, true), "{path}");
    }
    // A local socket: the same.
    assert_eq!(
        told(Operation::Connect, Kind::LocalSocket, false, None),
        (BlockOffer::Unsandboxed, None, true)
    );
    // A lookup a program made itself, past the proxy (#1631): no host grant would reach
    // it, so none is offered, and the Notice says why rather than nothing. The host it
    // looked up is shown (#1663), checked as a grant checks one, and never offered.
    assert_eq!(
        told(Operation::Lookup, Kind::Host, false, None),
        (BlockOffer::Unsandboxed, None, true)
    );
    assert_eq!(
        told(
            Operation::Lookup,
            Kind::Host,
            false,
            Some("DB.Prod.example.com")
        ),
        (
            BlockOffer::Unsandboxed,
            Some("db.prod.example.com".to_owned()),
            true
        )
    );
    for target in ["localhost", "*.example.com", "db", "169.254.169.254"] {
        assert_eq!(
            told(Operation::Lookup, Kind::Host, false, Some(target)),
            (BlockOffer::Unsandboxed, None, true),
            "{target}"
        );
    }
    // Never granted: the way that works instead.
    for (operation, kind) in [
        (Operation::Write, Kind::ProjectState),
        (Operation::Write, Kind::ProtectedFile),
        (Operation::Read, Kind::Home),
    ] {
        assert_eq!(
            told(operation, kind, false, Some("/x/y")),
            (BlockOffer::Brokered, None, true),
            "{operation:?} {kind:?}"
        );
    }
    // purlis's own operation is reported, never allowed.
    assert_eq!(
        told(Operation::Write, Kind::System, true, Some("/opt/x")),
        (BlockOffer::None, None, false)
    );
}

/// #1476 and #1408 l32: a refused write the app makes for a chat through a purlis command
/// offers no grant, and its Notice names that command: `purlis worktree commit` for a
/// branch folder's git record, the brokered write's own for a file purlis writes. Any other
/// write keeps its own offer.
#[test]
fn a_refused_write_purlis_makes_for_a_chat_names_its_command_and_offers_no_grant() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let write = |kind| Block {
        operation: Operation::Write,
        kind,
        ours: false,
    };
    let lock = root.join("workspaces/alpha/api/.git/worktrees/api-fix/index.lock");
    let (offer, target, route) = offered(
        &root,
        &write(Kind::ProtectedFile),
        Some(&lock.display().to_string()),
    );
    assert_eq!((offer, target), (BlockOffer::Brokered, None));
    assert!(
        route
            .as_deref()
            .is_some_and(|why| why.contains("`purlis worktree commit")),
        "{route:?}"
    );
    let memory = root.join("workspaces/alpha/memory/a-note.md");
    let (offer, target, route) = offered(
        &root,
        &write(Kind::ProjectFiles),
        Some(&memory.display().to_string()),
    );
    assert_eq!((offer, target), (BlockOffer::Brokered, None));
    assert!(
        route
            .as_deref()
            .is_some_and(|why| why.contains("`purlis workspace remember")),
        "{route:?}"
    );
    // A protected file with no path keeps the generic route, which names no command.
    let (offer, _, route) = offered(&root, &write(Kind::ProtectedFile), None);
    assert_eq!(offer, BlockOffer::Brokered);
    assert!(!route.unwrap_or_default().contains("worktree commit"));
}

/// #1476 and #1408: the path a refused write names is the chat's to forge, so the way out it
/// chooses may only take a grant away. Whatever the operation and kind, a path that names a
/// purlis command offers Brokered with nothing to allow, never a folder or a host, inside
/// the project or outside it.
#[test]
fn a_forged_path_only_ever_takes_a_grant_away() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let forged = [
        root.join("workspaces/alpha/api/.git/worktrees/fix/index.lock"),
        root.join("workspaces/alpha/.git/worktrees/fix/../../../memory/x.md"),
        PathBuf::from("/opt/elsewhere/.git/worktrees/fix/index"),
        root.join("sessions/one.md"),
        root.join("workspaces/alpha/memory/a.md"),
        root.join("workspaces/alpha/todos/b.md"),
        root.join("workspaces/alpha/workspace.md"),
        root.join("personas/steward/memory/c.md"),
    ];
    for path in &forged {
        let path = path.display().to_string();
        for operation in Operation::ALL {
            for kind in Kind::ALL {
                let block = Block {
                    operation,
                    kind,
                    ours: false,
                };
                let (offer, target, _) = offered(&root, &block, Some(&path));
                let consulted = matches!(operation, Operation::Write | Operation::File)
                    && matches!(kind, Kind::ProjectFiles | Kind::ProtectedFile);
                if consulted
                    && purlis_core::sandboxblock::way_out(Path::new(&path), &root).is_some()
                {
                    assert_eq!(
                        (offer, target.as_deref()),
                        (BlockOffer::Brokered, None),
                        "{operation:?} {kind:?} on {path}"
                    );
                }
                assert!(
                    !(consulted && offer == BlockOffer::Host),
                    "{operation:?} {kind:?} on {path} offers a host"
                );
                // A protected file never offers a folder, named path or not.
                if kind == Kind::ProtectedFile {
                    assert_ne!(offer, BlockOffer::Write, "{operation:?} on {path}");
                    assert_eq!(target, None, "{operation:?} on {path}");
                }
            }
        }
    }
}

/// **No block of the chat's own work dead-ends its Notice** (#1637): whatever the operation
/// and kind, and whether or not the report named what was refused, the Notice offers Allow,
/// the way that works, or Start without the sandbox, and says why where it is not Allow.
/// Only purlis's own block offers nothing to allow, and its Notice offers a Report.
#[test]
fn no_block_of_the_chats_own_work_dead_ends_its_notice() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    for operation in Operation::ALL {
        for kind in Kind::ALL {
            for target in [None, Some("relative/x"), Some("/opt/tool/x.lock")] {
                let block = Block {
                    operation,
                    kind,
                    ours: false,
                };
                let (offer, _, route) = offered(&root, &block, target);
                assert_ne!(
                    offer,
                    BlockOffer::None,
                    "{operation:?} {kind:?} on {target:?} offers nothing"
                );
                if !matches!(offer, BlockOffer::Host | BlockOffer::Write) {
                    assert!(
                        route.as_deref().is_some_and(|why| !why.trim().is_empty()),
                        "{operation:?} {kind:?} on {target:?}: {offer:?} says no why"
                    );
                }
            }
            let ours = Block {
                operation,
                kind,
                ours: true,
            };
            assert_eq!(offered(&root, &ours, None).0, BlockOffer::None);
        }
    }
    // The two the review named: neither a host grant nor a folder would reach them. A
    // service that names nothing has Start without the sandbox; the certificate check is
    // the project's setting (R1), on here, so its way on is a restart, named or not.
    for (kind, comes_to) in [
        (Kind::CertificateCheck, BlockOffer::Allowed),
        (Kind::SystemService, BlockOffer::Unsandboxed),
    ] {
        let block = Block {
            operation: Operation::Lookup,
            kind,
            ours: false,
        };
        assert_eq!(offered(&root, &block, None).0, comes_to, "{kind:?}");
    }
}

/// R1 (2026-10-13): a refused certificate check is never offered an Allow, and its Notice
/// never dead-ends. On for the project, the chat started before it was: Restart. Off by
/// the project's setting: said, with Settings and Start without the sandbox. Off by
/// policy: said with who set it, and nothing offered where policy forbids the opt-out too.
#[test]
fn a_refused_certificate_check_says_what_stands_and_never_offers_an_allow() {
    use purlis_core::sandbox::policy::{self, Locks};
    use purlis_core::sandbox::services::TRUSTD;
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let block = Block {
        operation: Operation::Lookup,
        kind: Kind::CertificateCheck,
        ours: false,
    };
    let project_with = |settings: &str| {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(
            project.path().join(purlis_core::plane::MANIFEST),
            format!("[sandbox]\nmode = \"on\"\n{settings}"),
        )
        .expect("toml");
        project
    };
    // The default project, and one that says `true`: on, so a restart is what is owed.
    for settings in ["", "certificate-checks = true\n"] {
        let project = project_with(settings);
        let (offer, target, said) = offered(project.path(), &block, Some(TRUSTD));
        assert_eq!(offer, BlockOffer::Allowed, "{settings}");
        assert_eq!(target, None);
        let said = said.expect("said");
        assert!(said.contains("on for this project"), "{said}");
        assert!(said.contains("once it restarts"), "{said}");
        // And policy has nothing to take away from it.
        let (offer, _, _, levels) = held(&Locks::none(), (offer, target, Some(said)));
        assert_eq!((offer, levels), (BlockOffer::Allowed, Vec::new()));
    }
    // A chat that started WITH the check and is refused it all the same: a restart would
    // start it with the same sandbox, so the Notice says the check is on already, that
    // something else refused it, and offers the way to tell, never Restart.
    let project = project_with("");
    for named in [Some(TRUSTD), None] {
        let (offer, target, said) = offered_to(project.path(), &block, named, true);
        assert_eq!(offer, BlockOffer::Unsandboxed, "{named:?}");
        assert_eq!(target, None);
        let said = said.expect("said");
        assert!(said.contains("already has certificate checks"), "{said}");
        assert!(!said.contains("once it restarts"), "{said}");
        let (offer, _, _, levels) = held(&Locks::none(), (offer, target, Some(said)));
        assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    }
    // What the chat started with changes nothing where the project or policy turned the
    // check off since: that is what stands for its next start, and what is said.
    let off = project_with("certificate-checks = false\n");
    assert_eq!(
        offered_to(off.path(), &block, Some(TRUSTD), true),
        offered_to(off.path(), &block, Some(TRUSTD), false)
    );
    // The project's own `false`.
    let off = project_with("certificate-checks = false\n");
    let (offer, target, said) = offered(off.path(), &block, Some(TRUSTD));
    assert_eq!(offer, BlockOffer::Unsandboxed);
    let said = said.expect("said");
    assert!(
        said.contains("This project turns certificate checks off"),
        "{said}"
    );
    assert!(said.contains("Settings › Sandbox"), "{said}");
    let (offer, _, _, levels) = held(&Locks::none(), (offer, target, Some(said)));
    assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    // Policy's lock, over a project that leaves it on: who locked it, and nothing to
    // allow. With the opt-out forbidden too, nothing is offered at all.
    let project = project_with("");
    for (text, comes_to) in [
        (
            r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
            BlockOffer::Unsandboxed,
        ),
        (
            r#"{"owner": "IT", "sandbox": {"socket-grants": false, "opt-out": false}}"#,
            BlockOffer::Policy,
        ),
    ] {
        let locks = Locks::parse(text, Path::new("/etc/purlis/policy.json"));
        policy::set_for_this_test(locks.clone());
        let (offer, target, said) = offered(project.path(), &block, Some(TRUSTD));
        policy::set_for_this_test(Locks::none());
        assert_eq!(offer, BlockOffer::Unsandboxed);
        let (offer, _, said, levels) = held(&locks, (offer, target, said));
        assert_eq!((offer, levels), (comes_to, Vec::new()), "{text}");
        let said = said.expect("said");
        assert!(
            said.contains("Policy turns certificate checks off"),
            "{said}"
        );
        assert!(said.contains("IT"), "{said}");
    }
}

/// ADR 0093: a refused system service is offered by its name, with what allowing it means.
/// Claude Code's sandbox takes no service by name, so there its Notice offers Start
/// without the sandbox, saying why.
#[test]
fn a_service_is_offered_by_name_and_claude_code_takes_none_by_name() {
    use purlis_core::harness::Harness;
    use purlis_core::sandbox::services::RISK_UNKNOWN;
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let lookup = |kind| Block {
        operation: Operation::Lookup,
        kind,
        ours: false,
    };
    let other = offered(
        &root,
        &lookup(Kind::SystemService),
        Some("com.example.helper"),
    );
    assert_eq!(other.0, BlockOffer::Service);
    assert!(
        other
            .2
            .as_deref()
            .is_some_and(|said| said.contains(RISK_UNKNOWN))
    );
    // Claude Code: none by name.
    let (offer, target, why) = for_its_harness(Some(Harness::ClaudeCode), other.clone());
    assert_eq!(offer, BlockOffer::Unsandboxed);
    assert_eq!(target.as_deref(), Some("com.example.helper"));
    assert!(why.is_some_and(|why| why.contains("certificate check")));
    // purlis's own wrap takes any service by name.
    assert_eq!(
        for_its_harness(Some(Harness::Codex), other).0,
        BlockOffer::Service
    );
    // A keychain's service is never granted: the way that works instead.
    assert_eq!(
        offered(
            &root,
            &lookup(Kind::SystemService),
            Some("com.apple.SecurityServer")
        )
        .0,
        BlockOffer::Brokered
    );
}

/// ADR 0093 (D-sock-3, D-sock-8): a socket or a service is offered once, for this chat and
/// for this project on this machine, never for everyone; and an administrator's
/// `socket-grants` lock takes every one away, saying who set it.
#[test]
fn a_socket_is_offered_once_for_this_chat_and_for_you_and_policy_can_forbid_it() {
    use crate::sandboxing::GrantLevel;
    use purlis_core::sandbox::policy::Locks;
    let socket = || {
        (
            BlockOffer::Socket,
            Some("/var/run/docker.sock".to_owned()),
            Some("Allowing it lets this chat control containers.".to_owned()),
        )
    };
    // R2: every project on this machine is among them, the first standing answer.
    assert_eq!(
        held(&Locks::none(), socket()).3,
        [
            GrantLevel::Once,
            GrantLevel::Machine,
            GrantLevel::Chat,
            GrantLevel::You
        ]
    );
    // A policy that fixes the scopes and leaves every project out removes it (C3)...
    let scoped = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(
        held(&scoped, socket()).3,
        [GrantLevel::Chat, GrantLevel::You]
    );
    // ...and one that names it by its word, `machine`, keeps it on offer.
    let worded = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["once", "machine"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(
        held(&worded, socket()).3,
        [GrantLevel::Once, GrantLevel::Machine]
    );
    // What it removed is said on the Notice, "Allow once" too: never only missing.
    let said = scopes_ruled_out(&scoped, BlockOffer::Socket).expect("said");
    assert!(
        said.contains("Allow once, for this chat's next run")
            && said.contains("Allow on this machine, for every project")
            && said.contains("set by IT"),
        "{said}"
    );
    assert_eq!(scopes_ruled_out(&Locks::none(), BlockOffer::Socket), None);
    assert_eq!(scopes_ruled_out(&scoped, BlockOffer::Write), None);
    assert_eq!(scopes_ruled_out(&scoped, BlockOffer::Unsandboxed), None);
    // A block that names no socket yet is offered at no scope policy removed either.
    let unnamed = || (BlockOffer::Socket, None, None);
    assert_eq!(
        held(&scoped, unnamed()).3,
        [GrantLevel::Chat, GrantLevel::You]
    );
    // A folder is offered at the scopes policy leaves open, and not at all where it
    // leaves none a folder is kept at.
    let write = || (BlockOffer::Write, Some("/p/out".to_owned()), None);
    let you = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["you", "machine"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(held(&you, write()).3, [GrantLevel::You]);
    let (offer, _, route, levels) = held(&worded, write());
    assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    assert!(route.is_some_and(|why| why.contains("Policy removed Allow")));
    let locked = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let (offer, _, route, levels) = held(&locked, socket());
    assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    assert!(route.is_some_and(|why| why.contains("set by IT")));
}

#[test]
fn a_block_s_notice_offers_nothing_policy_forbids_and_says_who_forbade_it() {
    use crate::sandboxing::GrantLevel;
    use purlis_core::sandbox::policy::Locks;
    let policy = |json: &str| Locks::parse(json, Path::new("/etc/purlis/policy.json"));
    let host = || (BlockOffer::Host, Some("pastebin.example".to_owned()), None);
    let write = || (BlockOffer::Write, Some("/p/out".to_owned()), None);
    let socket = || {
        (
            BlockOffer::Unsandboxed,
            None,
            Some("Not a grant.".to_owned()),
        )
    };
    // No policy: every level, and Start without the sandbox where nothing is granted.
    assert_eq!(
        held(&Locks::none(), host()).3,
        [GrantLevel::Chat, GrantLevel::You, GrantLevel::Project]
    );
    assert_eq!(held(&Locks::none(), socket()).0, BlockOffer::Unsandboxed);
    // Your own hosts forbidden: only everyone in the project.
    let mine = policy(r#"{"owner": "IT", "sandbox": {"personal-hosts": false}}"#);
    assert_eq!(held(&mine, host()).3, [GrantLevel::Project]);
    // A host policy does not allow: no Allow; Start without the sandbox, saying why.
    let listed = policy(r#"{"owner": "IT", "sandbox": {"hosts": ["*.corp.example"]}}"#);
    let (offer, _, route, levels) = held(&listed, host());
    assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    let route = route.expect("why");
    assert!(route.contains("not a host policy allows"), "{route}");
    assert!(route.contains("set by IT"), "{route}");
    // Write grants forbidden: no Allow for a folder.
    let writes = policy(r#"{"owner": "IT", "sandbox": {"write-grants": false}}"#);
    assert_eq!(held(&writes, write()).0, BlockOffer::Unsandboxed);
    // …and with the opt-out forbidden too, nothing is offered, and it still says why.
    let both = policy(
        r#"{"owner": "IT", "sandbox": {"write-grants": false, "opt-out": false,
                "hosts": []}}"#,
    );
    for offered in [host(), write(), socket()] {
        let (offer, _, route, levels) = held(&both, offered);
        assert_eq!((offer, levels), (BlockOffer::Policy, Vec::new()));
        let route = route.expect("why");
        assert!(route.contains("Locked by policy, set by IT"), "{route}");
    }
    // What is never granted keeps the way that works, policy or not.
    let brokered = (
        BlockOffer::Brokered,
        None,
        Some("Use the broker.".to_owned()),
    );
    assert_eq!(held(&both, brokered).0, BlockOffer::Brokered);
}

#[test]
fn a_host_s_notice_says_what_policy_ruled_out_and_who_set_it() {
    use crate::sandboxing::GrantLevel;
    use purlis_core::sandbox::policy::Locks;
    let policy = |json: &str| Locks::parse(json, Path::new("/etc/purlis/policy.json"));
    let host = || (BlockOffer::Host, Some("pastebin.example".to_owned()), None);
    // Nothing ruled out: nothing said.
    let none = held(&Locks::none(), host());
    assert_eq!(ruled_out(&Locks::none(), none.1.as_deref(), &none.3), None);
    // A scope removed: the Notice says which, and who removed it.
    let scopes = policy(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["you"]}}"#);
    let (_, target, _, levels) = held(&scopes, host());
    assert_eq!(levels, [GrantLevel::You]);
    let said = ruled_out(&scopes, target.as_deref(), &levels).expect("ruled");
    assert!(
        said.contains("Policy removed Allow for this chat"),
        "{said}"
    );
    assert!(said.contains("set by IT"), "{said}");
    // Asking while a connection waits turned off: said too.
    let off = policy(r#"{"owner": "IT", "sandbox": {"live-asks": false}}"#);
    let (_, target, _, levels) = held(&off, host());
    let said = ruled_out(&off, target.as_deref(), &levels).expect("ruled");
    assert!(said.contains("Policy turns off asking"), "{said}");
    // A host pinned never allowed: no Allow at all, and the Notice says why.
    let pinned = policy(r#"{"owner": "IT", "sandbox": {"never-hosts": ["pastebin.example"]}}"#);
    let (offer, _, route, levels) = held(&pinned, host());
    assert_eq!((offer, levels), (BlockOffer::Unsandboxed, Vec::new()));
    assert!(route.expect("why").contains("Policy never allows"));
}

#[test]
fn a_host_allowed_already_offers_no_allow_and_says_the_chat_takes_it_on_a_restart() {
    // No project file says so here: nothing is allowed, so Allow stays on offer.
    let root = tempfile::tempdir().expect("a project");
    assert_eq!(
        allowed_already(root.path(), BlockOffer::Host, Some("api.example.com:443")),
        None
    );
    // Only a host offered to allow is asked about.
    assert_eq!(
        allowed_already(root.path(), BlockOffer::Write, Some("/p/out")),
        None
    );
}

/// qw351: the person allowed a service or a socket for themselves on this machine, and a
/// chat that started before that is refused it still. Its block says the Allow stands and
/// offers Restart this chat: never the same Allow again, which would keep nothing new.
#[test]
fn a_service_or_a_socket_allowed_on_this_machine_already_offers_a_restart_and_no_allow() {
    use purlis_core::sandbox::local;
    const HELPER: &str = "com.example.helper";
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    assert_eq!(
        allowed_already(&root, BlockOffer::Service, Some(HELPER)),
        None,
        "nothing is allowed yet, so Allow stays on offer"
    );
    local::grant_service(&root, HELPER).expect("kept");
    let said = allowed_already(&root, BlockOffer::Service, Some(HELPER)).expect("said");
    assert!(said.contains("allowed already"), "{said}");
    assert!(said.contains("on this machine"), "{said}");
    assert!(said.contains("once it restarts"), "{said}");
    // By its whole name: another service is asked about as before.
    assert_eq!(
        allowed_already(&root, BlockOffer::Service, Some("com.example.other")),
        None
    );
    let socket = "/purlis-test-nowhere/docker.sock";
    assert_eq!(
        allowed_already(&root, BlockOffer::Socket, Some(socket)),
        None
    );
    local::grant_socket(&root, std::path::Path::new(socket)).expect("kept");
    assert!(allowed_already(&root, BlockOffer::Socket, Some(socket)).is_some());
    // #1927: kept at a scope policy now leaves out, it reaches no chat, so a restart is
    // not what the Notice offers: it is asked about at the scopes that are left.
    purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["once", "chat"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let held = (
        allowed_already(&root, BlockOffer::Socket, Some(socket)),
        allowed_already(&root, BlockOffer::Service, Some(HELPER)),
    );
    purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::none());
    assert_eq!(held, (None, None));
    // R2: allowed for every project on this machine, a chat of any project that started
    // before is told the same, in that scope's words.
    let everywhere = tempfile::tempdir().expect("a config home");
    local::everywhere_for_this_test(Some(everywhere.path().join("every-project.json")));
    let other = tempfile::tempdir().expect("another project");
    let agent = "/purlis-test-nowhere/agent.sock";
    assert_eq!(
        allowed_already(other.path(), BlockOffer::Socket, Some(agent)),
        None
    );
    local::grant_socket_everywhere(std::path::Path::new(agent), None).expect("kept");
    local::grant_service_everywhere("com.example.everywhere", None).expect("kept");
    let said = allowed_already(other.path(), BlockOffer::Socket, Some(agent)).expect("said");
    assert!(
        said.contains("on this machine, for every project"),
        "{said}"
    );
    assert!(
        allowed_already(
            other.path(),
            BlockOffer::Service,
            Some("com.example.everywhere")
        )
        .is_some()
    );
    local::everywhere_for_this_test(None);
    // Revoked in Settings, it is asked about again.
    local::revoke_service(&root, HELPER).expect("revoked");
    assert_eq!(
        allowed_already(&root, BlockOffer::Service, Some(HELPER)),
        None
    );
}

fn told_block() -> purlis_core::sandboxblock::Block {
    purlis_core::sandboxblock::Block {
        operation: purlis_core::sandboxblock::Operation::Connect,
        kind: purlis_core::sandboxblock::Kind::Host,
        ours: false,
    }
}

#[test]
fn a_touched_file_reaches_the_window_only_inside_the_chats_folder_and_rated() {
    use purlis_core::hookwire::Touching;
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let gate = Mutex::new(purlis_core::touching::Gate::default());
    let folder = Path::new("/w/branch");
    let now = std::time::Instant::now();
    let said = |path: &str| Touching {
        chat: 3,
        touching: path.to_owned(),
        wrote: false,
    };

    assert_eq!(
        touched(
            &plane,
            Some(folder),
            &gate,
            &said("/w/branch/src/a.rs"),
            now
        ),
        Some(ChatTouching {
            plane: plane.clone(),
            session: 3,
            path: "src/a.rs".to_owned(),
        })
    );
    for outside in [
        "/w/other/a.rs",
        "/w/branch/../other/a.rs",
        "../x",
        "/etc/hosts",
    ] {
        assert_eq!(
            touched(&plane, Some(folder), &gate, &said(outside), now),
            None,
            "{outside}"
        );
    }
    assert_eq!(
        touched(&plane, None, &gate, &said("/w/branch/a.rs"), now),
        None,
        "a chat with no folder known marks nothing"
    );
    let told = (0..100)
        .filter(|n| touched(&plane, Some(folder), &gate, &said(&format!("f{n}")), now).is_some())
        .count();
    assert!(told < 10, "a flood is cut at the chat's share: {told}");
}

#[test]
fn a_touched_path_reaches_the_window_and_no_file_the_host_writes() {
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    const CANARY: &str = "CANARY-touched-9e1b";
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    let (tx, heard) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    hooks.when_touching(Arc::new(move |touching| {
        tx.lock().unwrap().send(touching).unwrap();
    }));
    hooks.board().opened(3, Some(Harness::ClaudeCode), None);
    let socket = hooks.socket().expect("a socket");
    let token = hooks.token_for(3);
    let file = format!("/w/branch/{CANARY}.rs");
    let input = serde_json::json!({"file_path": file});

    purlis_core::hookwire::deliver_tool(
        socket,
        Some(&token),
        &purlis_core::hookwire::ToolCall {
            chat: 3,
            tool_hook: "pretooluse-read".to_owned(),
            tool: Some("Read".to_owned()),
            call: Some("toolu_1".to_owned()),
            args: Some(eventlog::args_hash(&input)),
            decision: purlis_core::hookwire::Decision::None,
            rule: None,
            hook_ms: 1,
            agent: None,
            at_ms: 0,
            skill: None,
        },
    )
    .expect("told");
    purlis_core::hookwire::touch(
        socket,
        Some(&token),
        &purlis_core::hookwire::Touching {
            chat: 3,
            touching: file.clone(),
            wrote: false,
        },
    )
    .expect("told");
    let got = heard
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the touch is handed on");
    assert_eq!(got.touching, file);
    drop(hooks);

    let kinds: Vec<String> = eventlog::read(&logs)
        .expect("the log reads")
        .into_iter()
        .map(|event| event.kind)
        .collect();
    assert!(
        kinds.contains(&"hook.pretooluse-read".to_owned()),
        "{kinds:?}"
    );
    let mut stack = vec![dir.path().to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = std::fs::read(&path).unwrap_or_default();
                assert!(
                    !String::from_utf8_lossy(&text).contains(CANARY),
                    "{} holds the touched path",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn what_a_chat_is_doing_reaches_the_window_from_its_hooks_and_no_file_the_host_writes() {
    use purlis_core::doing::{Kind, Said};
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    use purlis_core::hookwire::{Doing, send, tell_doing};
    use purlis_core::state::Event;
    const CANARY: &str = "CANARY-doing-77aa";
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    let (tx, told) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    hooks.doings().tell_to(Arc::new(move |doing| {
        let _ = tx.lock().unwrap().send(doing);
    }));
    hooks.board().opened(3, None, None);
    let socket = hooks.socket().expect("a socket").to_path_buf();
    let token = hooks.token_for(3);
    let report = |event| Report {
        chat: 3,
        event,
        conversation: purlis_core::hookwire::Conversation::Unknown,
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail::default(),
    };
    let says = |doing: Said, agent: Option<&str>| {
        tell_doing(
            &socket,
            Some(&token),
            &Doing {
                chat: 3,
                doing,
                agent: agent.map(str::to_owned),
                speaker: purlis_core::hookwire::Speaker::default(),
            },
        )
        .expect("told");
    };
    let next = || {
        told.recv_timeout(std::time::Duration::from_secs(5))
            .expect("the window is told")
    };
    let apart = || std::thread::sleep(purlis_core::doing::AT_MOST_EVERY);

    send(&socket, Some(&token), &report(Event::UserPromptSubmit)).expect("sent");
    let first = next();
    assert_eq!(first.session, 3);
    assert_eq!(
        first.doing.as_ref().map(|doing| doing.kind.as_str()),
        Some("thinking")
    );

    apart();
    says(
        Said::Began {
            kind: Kind::Editing,
            name: Some(format!("{CANARY}.rs")),
        },
        None,
    );
    assert_eq!(
        next().doing,
        Some(crate::doing::Doing {
            kind: "editing".to_owned(),
            name: Some(format!("{CANARY}.rs")),
            count: 0,
            over: false
        })
    );

    // A helper's tool is not the chat's own work, and a name the core does not pass is
    // not said: the kind alone is.
    apart();
    says(
        Said::Began {
            kind: Kind::Command,
            name: Some("helper".to_owned()),
        },
        Some("agent-1"),
    );
    says(
        Said::Began {
            kind: Kind::Command,
            name: Some(format!("{CANARY} needs you")),
        },
        None,
    );
    assert_eq!(
        next().doing,
        Some(crate::doing::Doing {
            kind: "command".to_owned(),
            name: None,
            count: 0,
            over: false
        })
    );
    assert_eq!(hooks.doing_now().len(), 1);

    send(&socket, Some(&token), &report(Event::Stop)).expect("sent");
    assert_eq!(next().doing, None);
    assert!(hooks.doing_now().is_empty());
    drop(hooks);

    // In memory only: nothing the host wrote holds a word of it.
    let mut stack = vec![dir.path().to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = std::fs::read(&path).unwrap_or_default();
                assert!(
                    !String::from_utf8_lossy(&text).contains(CANARY),
                    "{} holds what the chat was doing",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn a_refused_commit_puts_the_chat_in_the_queue_saying_why() {
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let hooks = Hooks::listening_on(
        plane,
        &at,
        Arc::new(move |moved| tx.lock().unwrap().send(moved).unwrap()),
        Arc::new(|_| panic!("a refused commit is not a harness started by hand")),
    )
    .expect("listening");
    hooks.board().opened(3, Some(Harness::ClaudeCode), None);
    let said = "commit refused in app: a.py:2  an email address  ad**";

    purlis_core::hookwire::deliver_refused(
        hooks.socket().expect("a socket"),
        Some(&hooks.token_for(3)),
        &purlis_core::hookwire::CommitRefused {
            chat: 3,
            commit_refused: said.to_owned(),
        },
    )
    .expect("told");

    let moved = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the window is told");
    assert_eq!(moved.session, 3);
    assert!(moved.needs_you);
    assert_eq!(moved.queue, vec![3]);
    assert_eq!(moved.refusals, vec![said.to_owned()]);
}

/// A chat refused an update of the project's root asks the person for it (qw348): the
/// guard's own verdict raises the item, in the app's words. Another refusal raises none.
#[test]
fn a_chat_refused_an_update_of_the_root_asks_the_person_for_one() {
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let hooks = Hooks::listening_on(
        plane,
        &at,
        Arc::new(move |moved| tx.lock().unwrap().send(moved).unwrap()),
        Arc::new(|_| panic!("a refused update is not a harness started by hand")),
    )
    .expect("listening");
    hooks.board().opened(3, Some(Harness::ClaudeCode), None);
    let refused_by = |rule: &str| purlis_core::hookwire::ToolCall {
        chat: 3,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: None,
        args: None,
        decision: purlis_core::hookwire::Decision::Deny,
        rule: Some(rule.to_owned()),
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    let deliver = |call: &purlis_core::hookwire::ToolCall| {
        purlis_core::hookwire::deliver_tool(
            hooks.socket().expect("a socket"),
            Some(&hooks.token_for(3)),
            call,
        )
        .expect("told");
    };

    // Another guard's refusal first: heard, and no item of this kind.
    deliver(&refused_by("project-root-branch"));
    deliver(&refused_by(purlis_core::rootupdate::REASON));

    let moved = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the window is told");
    assert_eq!(moved.session, 3);
    assert!(moved.needs_you);
    assert_eq!(moved.needs, Some(vec![Need::RootUpdateRefused]));
    assert_eq!(
        hooks.board().needs_of(3),
        vec![purlis_core::state::Need::RootUpdateRefused]
    );
}

#[test]
fn a_word_that_is_no_harness_charter_starts_puts_nothing_on_the_tab() {
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");

    let told = by_hand(
        &plane,
        StartedByHand {
            chat: 3,
            started_by_hand: "vim".to_owned(),
            cwd: None,
        },
    );

    assert_eq!(told, None);
}

use purlis_core::harness::Harness;
use purlis_core::hookwire::Conversation;

/// A board holding chat 7 running `harness`, under `conversation` where charter chose one.
fn running(harness: Harness, conversation: Option<&str>) -> Hooks {
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::deaf(plane);
    hooks
        .board()
        .opened(7, Some(harness), conversation.map(str::to_owned));
    hooks
}

/// The conversation chat 7 moved onto when `conversation` was reported from `pid`, if any.
fn followed_after(hooks: &Hooks, conversation: Conversation, pid: Option<u32>) -> Option<String> {
    let report = Report {
        agent: None,
        chat: 7,
        event: purlis_core::state::Event::UserPromptSubmit,
        conversation,
        pid,
        detail: purlis_core::state::Detail::default(),
    };
    apply(
        &hooks.board,
        &hooks.plane,
        &report,
        purlis_core::state::Waits::default(),
    )
    .followed
}

fn named(id: &str) -> Conversation {
    Conversation::Named(id.to_owned())
}

#[test]
fn a_codex_chat_follows_the_first_conversation_its_harness_names_and_no_other() {
    let hooks = running(Harness::Codex, None);

    assert_eq!(
        followed_after(&hooks, named("aaa"), None).as_deref(),
        Some("aaa")
    );
    assert_eq!(
        followed_after(&hooks, named("aaa"), None),
        None,
        "no rewrite"
    );
    assert_eq!(
        followed_after(&hooks, named("bbb"), None),
        None,
        "a later id from a pid-less harness is a nested run, never followed"
    );
}

#[test]
fn an_opencode_chat_follows_the_first_conversation_its_plugin_names() {
    let hooks = running(Harness::Opencode, None);

    assert_eq!(
        followed_after(&hooks, named("ses_abc"), None).as_deref(),
        Some("ses_abc")
    );
}

#[test]
fn a_report_that_is_not_the_chat_s_own_harness_moves_no_conversation() {
    // ADR 0024 C5: a harness nested in the chat's shell must not be able to rewrite what
    // the chat resumes at the next launch.
    let codex = running(Harness::Codex, None);
    assert_eq!(
        followed_after(&codex, Conversation::Contradicted, None),
        None
    );
    assert_eq!(
        followed_after(&codex, named("ccc"), Some(99)),
        None,
        "a claude inside a codex chat"
    );
    assert_eq!(
        followed_after(&codex, named("aaa"), None).as_deref(),
        Some("aaa")
    );
    assert_eq!(followed_after(&codex, Conversation::Foreign, None), None);

    let claude = running(Harness::ClaudeCode, Some("chosen"));
    assert_eq!(followed_after(&claude, named("chosen"), Some(10)), None);
    assert_eq!(
        followed_after(&claude, named("nested"), Some(11)),
        None,
        "a claude started inside a claude chat"
    );
    assert_eq!(
        followed_after(&claude, Conversation::Contradicted, Some(10)),
        None
    );
}

#[test]
fn a_claude_chat_follows_its_own_process_onto_the_conversation_a_clear_starts() {
    let hooks = running(Harness::ClaudeCode, Some("chosen"));
    assert_eq!(
        followed_after(&hooks, named("chosen"), Some(10)),
        None,
        "the id charter chose is already recorded; adopting it rewrites nothing"
    );

    assert_eq!(
        followed_after(&hooks, named("cleared"), Some(10)).as_deref(),
        Some("cleared")
    );
}

#[test]
fn a_line_spooled_while_no_host_listened_is_recorded_when_the_project_is_opened_again() {
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join(".charter/app/hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_value(serde_json::json!(dir.path())).expect("a plane id");
    let call = purlis_core::hookwire::ToolCall {
        chat: 7,
        tool_hook: "posttooluse".to_owned(),
        tool: Some("Read".to_owned()),
        call: Some("toolu_1".to_owned()),
        args: None,
        decision: purlis_core::hookwire::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    // The app issued chat 7 its token and quit; the chat's hook spooled its call.
    {
        let gone = Hooks::listening_on(plane.clone(), &at, Arc::new(|_| {}), Arc::new(|_| {}))
            .expect("listening");
        let token = gone.token_for(7);
        gone.stop();
        let delivered =
            purlis_core::hookwire::deliver_tool(&at.socket, Some(&token), &call).expect("spooled");
        assert_eq!(delivered, purlis_core::hookwire::Delivered::Spooled(1));
    }

    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    hooks.drain_spool();

    let kinds: Vec<String> = eventlog::read(&logs)
        .expect("the log reads")
        .into_iter()
        .map(|event| event.kind)
        .collect();
    assert_eq!(kinds, ["hook.posttooluse", "hook.spool.drained"]);
}

/// A chat's spool key lasts as long as the chat (V99i): what it spooled while the host was
/// busy is recorded as it closes, and a line under its key after that is rejected.
#[test]
fn a_closed_chats_spool_is_drained_into_the_log_and_its_key_does_not_outlive_it() {
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    use purlis_core::hookwire::spool;
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join(".charter/app/hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_value(serde_json::json!(dir.path())).expect("a plane id");
    let call = |chat: u32| purlis_core::hookwire::ToolCall {
        chat,
        tool_hook: "posttooluse".to_owned(),
        tool: Some("Read".to_owned()),
        call: Some("toolu_1".to_owned()),
        args: None,
        decision: purlis_core::hookwire::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    let spooling = spool::dir_for(&at.socket);
    let keys = || std::fs::read_to_string(spooling.join(spool::KEYS)).expect("the keys");
    let (closing, staying) = (hooks.token_for(7), hooks.token_for(8));
    // Each chat's hook spooled a call the host did not take in time.
    spool::append(&spooling, 7, &closing, &call(7)).expect("spooled");
    spool::append(&spooling, 8, &staying, &call(8)).expect("spooled");
    let both = keys();

    hooks.chat_ended(7);

    let kinds = || -> Vec<String> {
        eventlog::read(&logs)
            .expect("the log reads")
            .into_iter()
            .map(|event| event.kind)
            .collect()
    };
    assert_eq!(kinds(), ["hook.posttooluse", "hook.spool.drained"]);
    let id = |token: &purlis_core::hookwire::ChatToken| spool::SpoolKey::of(token).id();
    assert!(both.contains(&id(&closing)) && both.contains(&id(&staying)));
    assert!(!keys().contains(&id(&closing)), "{}", keys());
    assert!(keys().contains(&id(&staying)), "{}", keys());

    // A hook of the closed chat that is still about spools once more. Then the project is
    // opened again, with no reopen record: chat 8 is not brought back, so it has ended too.
    spool::append(&spooling, 7, &closing, &call(7)).expect("spooled");
    hooks.drain_spool();

    assert_eq!(
        kinds()[2..],
        [
            "hook.spool.rejected",
            "hook.posttooluse",
            "hook.spool.drained"
        ]
    );
    assert!(!keys().contains(&id(&staying)), "{}", keys());
}

/// #983: an open with no event log drains nothing, and still drops the spool key of a chat
/// it does not bring back, unless that chat's spool holds a line a later host can record.
/// So a host that never has a log does not pile up keys launch after launch.
#[test]
fn an_open_with_no_event_log_drops_an_ended_chats_key_unless_its_line_waits() {
    use purlis_core::hookwire::spool;
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join(".charter/app/hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_value(serde_json::json!(dir.path())).expect("a plane id");
    let spooling = spool::dir_for(&at.socket);
    let id = |token: &purlis_core::hookwire::ChatToken| spool::SpoolKey::of(token).id();
    let (idle, waiting) = {
        let gone = Hooks::listening_on(plane.clone(), &at, Arc::new(|_| {}), Arc::new(|_| {}))
            .expect("listening");
        let tokens = (gone.token_for(7), gone.token_for(8));
        gone.stop();
        tokens
    };
    let call = purlis_core::hookwire::ToolCall {
        chat: 8,
        tool_hook: "posttooluse".to_owned(),
        tool: Some("Read".to_owned()),
        call: Some("toolu_1".to_owned()),
        args: None,
        decision: purlis_core::hookwire::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    spool::append(&spooling, 8, &waiting, &call).expect("spooled");

    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    hooks.drain_spool();

    let keys = std::fs::read_to_string(spooling.join(spool::KEYS)).expect("the keys");
    assert!(!keys.contains(&id(&idle)), "{keys}");
    assert!(keys.contains(&id(&waiting)), "{keys}");
    assert!(spool::holds_lines(&spooling, 8), "nothing was drained");
}

/// #952: a line a running chat's hook spooled, because this host was slow to take it, is
/// recorded once the chat is heard from again: no restart, and no close of the chat.
#[test]
fn a_running_chats_spooled_line_is_recorded_once_the_chat_is_heard_from_again() {
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    use purlis_core::hookwire::spool;
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join(".charter/app/hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_value(serde_json::json!(dir.path())).expect("a plane id");
    let call = |id: &str| purlis_core::hookwire::ToolCall {
        chat: 7,
        tool_hook: "posttooluse".to_owned(),
        tool: Some("Read".to_owned()),
        call: Some(id.to_owned()),
        args: None,
        decision: purlis_core::hookwire::Decision::None,
        rule: None,
        hook_ms: 1,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    let token = hooks.token_for(7);
    // The chat's hook did not hear from this host in time and spooled its call.
    spool::append(
        &spool::dir_for(&at.socket),
        7,
        &token,
        &call("toolu_spooled"),
    )
    .expect("spooled");

    // The chat's next call reaches the host.
    let delivered =
        purlis_core::hookwire::deliver_tool(&at.socket, Some(&token), &call("toolu_live"))
            .expect("delivered");
    assert_eq!(delivered, purlis_core::hookwire::Delivered::Taken);

    let kinds = || -> Vec<String> {
        eventlog::read(&logs)
            .expect("the log reads")
            .into_iter()
            .map(|event| event.kind)
            .collect()
    };
    // #1871: the drain runs on the spool watcher's thread, and it records before it
    // removes: `hook.spool.drained` is in the log, then the keys advance, then the lines it
    // read are taken away (`spool::drain_running`, so a crash between never loses a line,
    // and the advanced keys keep the next drain from handing one on twice). So the drain
    // is over only once both are seen; the event alone left a window where the spool still
    // held the line it had just recorded.
    let spooling = spool::dir_for(&at.socket);
    let drained =
        || kinds().contains(&"hook.spool.drained".to_owned()) && !spool::holds_lines(&spooling, 7);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !drained() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        kinds(),
        [
            "run.started",
            "hook.posttooluse",
            "hook.posttooluse",
            "hook.spool.drained"
        ],
        "the run the live call is under, the live call, then the spooled one, drained while \
             the host runs"
    );
    assert!(!spool::holds_lines(&spooling, 7), "the spool is empty");
}

#[test]
fn the_run_a_clear_begins_is_told_with_the_conversation_it_moved_to() {
    // ADR 0066's `clear`: the event log begins the run, and the record has to hold it as
    // the chat's current run, so it is told beside the conversation.
    use purlis_core::eventlog::{self, ArgsKey, Log, Recorder};
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let logs = dir.path().join("events");
    hooks.record_into(Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    )));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    hooks.when_it_follows(Arc::new(move |chat, id, run| {
        let _ = tx
            .lock()
            .unwrap()
            .send((chat, id.to_owned(), run.map(str::to_owned)));
    }));
    hooks
        .board()
        .opened(7, Some(Harness::ClaudeCode), Some("chosen".to_owned()));
    let socket = hooks.socket().expect("a socket");
    let token = hooks.token_for(7);
    let say = |conversation: &str| {
        purlis_core::hookwire::send(
            socket,
            Some(&token),
            &Report {
                chat: 7,
                event: purlis_core::state::Event::UserPromptSubmit,
                conversation: named(conversation),
                pid: Some(10),
                agent: None,
                detail: Default::default(),
            },
        )
        .expect("sent");
    };

    say("chosen");
    say("cleared");

    let (chat, id, run) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the move is told");
    assert_eq!((chat, id.as_str()), (7, "cleared"));
    let cleared: Vec<_> = eventlog::read(&logs)
        .expect("the log reads")
        .into_iter()
        .filter(|event| event.kind == "run.started" && event.body["cause"] == "clear")
        .map(|event| event.run)
        .collect();
    assert_eq!(cleared, vec![run], "the run the log began for the clear");
}

#[test]
fn a_clear_begins_a_run_the_record_is_told_even_with_no_event_log() {
    // #856 review F5: the run is the host's, so `reopen.json` moves with it either way.
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks =
        Hooks::listening_on(plane, &at, Arc::new(|_| {}), Arc::new(|_| {})).expect("listening");
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    hooks.when_it_follows(Arc::new(move |_, id, run| {
        let _ = tx
            .lock()
            .unwrap()
            .send((id.to_owned(), run.map(str::to_owned)));
    }));
    hooks
        .board()
        .opened(7, Some(Harness::ClaudeCode), Some("chosen".to_owned()));
    let token = hooks.token_for(7);
    for conversation in ["chosen", "cleared"] {
        purlis_core::hookwire::send(
            hooks.socket().expect("a socket"),
            Some(&token),
            &Report {
                chat: 7,
                event: purlis_core::state::Event::UserPromptSubmit,
                conversation: named(conversation),
                pid: Some(10),
                agent: None,
                detail: Default::default(),
            },
        )
        .expect("sent");
    }

    let (id, run) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the move is told");
    assert_eq!(id, "cleared");
    assert!(
        run.as_deref()
            .and_then(purlis_core::reopen::a_ulid)
            .is_some(),
        "{run:?}"
    );
}

/// qw384: a refused connection to a port on this Mac is offered as that one port, with what
/// allowing it means, at this chat or every chat of this project on this machine; a port the
/// refusal did not name is typed; Claude Code's sandbox takes no one port, so there its Notice
/// offers Start without the sandbox, saying why.
#[test]
fn a_port_on_this_mac_is_offered_as_that_port_and_claude_code_is_carried_it_by_name() {
    use purlis_core::harness::Harness;
    use purlis_core::sandbox::localport::RISK;
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let refused = Block {
        operation: Operation::Connect,
        kind: Kind::LocalService,
        ours: false,
    };
    let named = offered(&root, &refused, Some("50795"));
    assert_eq!(named.0, BlockOffer::Port);
    assert_eq!(named.1.as_deref(), Some("50795"));
    assert!(named.2.as_deref().is_some_and(|said| said.contains(RISK)));
    let locks = purlis_core::sandbox::policy::Locks::of(&root);
    let (offer, target, _, levels) = held(&locks, named.clone());
    assert_eq!(offer, BlockOffer::Port);
    assert_eq!(target.as_deref(), Some("50795"));
    assert_eq!(
        levels,
        vec![
            crate::sandboxing::GrantLevel::Chat,
            crate::sandboxing::GrantLevel::You
        ]
    );
    // Named by nothing: the person types it.
    assert_eq!(offered(&root, &refused, None).0, BlockOffer::Port);
    assert_eq!(offered(&root, &refused, None).1, None);
    // purlis's own wrap takes one port.
    assert_eq!(
        for_its_harness(Some(Harness::Opencode), named.clone()).0,
        BlockOffer::Port
    );
    // qw384b: a Claude Code chat is offered the same Allow, its proxy carrying the port by the
    // name its commands send a proxy, and the Notice says so beside the risk.
    let (offer, target, why) = for_its_harness(Some(Harness::ClaudeCode), named);
    assert_eq!(offer, BlockOffer::Port);
    assert_eq!(target.as_deref(), Some("50795"));
    // The route whole: purlis's own two sentences, the risk and how it is reached. Compared,
    // never printed: it is drawn on the Notice, not logged.
    let said = format!("Allowing it {RISK}. {CLAUDE_CODE_PORT_SAID}");
    assert!(why.as_deref() == Some(said.as_str()));
    assert!(CLAUDE_CODE_PORT_SAID.contains("this-mac.internal"));
    assert!(CLAUDE_CODE_PORT_SAID.contains("kubectl"));
}

/// #2019: a server refused its port is offered Allow listening on that one port, for this chat
/// or this project on this machine, saying what it means and that only the loopback is
/// allowed; never purlis's own port; and a Claude Code chat, whose sandbox cannot take one
/// port, is offered Start without the sandbox, saying why.
#[test]
fn a_port_to_listen_on_is_offered_as_that_port_and_claude_code_is_told_why_not() {
    use purlis_core::harness::Harness;
    use purlis_core::sandbox::localport::LISTEN_RISK;
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let refused = Block {
        operation: Operation::Listen,
        kind: Kind::LocalService,
        ours: false,
    };
    let named = offered(&root, &refused, Some("5173"));
    assert_eq!(named.0, BlockOffer::Listen);
    assert_eq!(named.1.as_deref(), Some("5173"));
    // Compared, never printed: it is drawn on the Notice, not logged.
    let said = named.2.clone().expect("what allowing it means");
    assert!(said.contains(LISTEN_RISK));
    assert!(said.contains("127.0.0.1"));
    let locks = purlis_core::sandbox::policy::Locks::of(&root);
    let (offer, target, _, levels) = held(&locks, named.clone());
    assert_eq!(offer, BlockOffer::Listen);
    assert_eq!(target.as_deref(), Some("5173"));
    assert_eq!(
        levels,
        vec![
            crate::sandboxing::GrantLevel::Chat,
            crate::sandboxing::GrantLevel::You
        ]
    );
    // Named by nothing: the person types it.
    assert_eq!(offered(&root, &refused, None).0, BlockOffer::Listen);
    assert_eq!(offered(&root, &refused, None).1, None);
    // purlis's own wrap takes one port; Claude Code's sandbox cannot.
    assert_eq!(
        for_its_harness(Some(Harness::Codex), named.clone()).0,
        BlockOffer::Listen
    );
    let (offer, target, why) = for_its_harness(Some(Harness::ClaudeCode), named);
    assert_eq!(offer, BlockOffer::Unsandboxed);
    assert_eq!(target.as_deref(), Some("5173"));
    assert!(why.as_deref() == Some(CLAUDE_CODE_LISTEN_SAID));
    // What turning Claude Code's own setting on would open, said rather than offered.
    assert!(CLAUDE_CODE_LISTEN_SAID.contains("every port of this Mac"));
    assert!(CLAUDE_CODE_LISTEN_SAID.contains("every service on this Mac"));
    // A port purlis itself listens on is never one to listen on.
    let own = purlis_core::sandbox::localport::bind_loopback().expect("a port of purlis's");
    let own_port = own.local_addr().expect("bound").port();
    let refused_own = offered(&root, &refused, Some(&own_port.to_string()));
    assert_eq!(refused_own.0, BlockOffer::Brokered);
    purlis_core::sandbox::localport::release(own_port);
}

/// #2019: a port allowed to listen on for this project is said to be allowed already, and a
/// policy that holds ports holds it too.
#[test]
fn a_port_to_listen_on_allowed_already_or_held_by_policy_is_said_so() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    assert_eq!(
        allowed_already(&root, BlockOffer::Listen, Some("5173")),
        None
    );
    purlis_core::sandbox::local::grant_listen(&root, 5173).expect("kept");
    assert!(
        allowed_already(&root, BlockOffer::Listen, Some("5173"))
            .is_some_and(|said| said.contains("allowed already"))
    );
    // Allowed to listen on is not allowed to connect to as a service of its own.
    assert_eq!(allowed_already(&root, BlockOffer::Port, Some("5173")), None);
    let locked = purlis_core::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"socket-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    let (offer, _, _, levels) = held(&locked, (BlockOffer::Listen, Some("5173".to_owned()), None));
    assert_ne!(offer, BlockOffer::Listen);
    assert!(levels.is_empty());
}

/// qw384 review: a port only printed words name is pre-filled for the person to check, on a
/// Notice that asks for one; never where the refusal named the port, never on another Notice,
/// and never a port no grant could name.
#[test]
fn a_printed_port_is_only_ever_a_suggestion_on_a_notice_that_asks_for_one() {
    use purlis_core::harness::Harness;
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let line = |harness: Harness, target: Option<&str>, suggested: Option<&str>| HeardBlock {
        line: purlis_core::hookwire::SandboxBlocked {
            chat: 7,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::LocalService,
                ours: false,
            },
            harness: Some(harness.name().to_owned()),
            target: target.map(str::to_owned),
            read_back: false,
            suggested: suggested.map(str::to_owned),
        },
        vault: None,
    };
    let said = blocked(&plane, &line(Harness::Codex, None, Some("50795")));
    assert_eq!(said.offer, BlockOffer::Port);
    assert_eq!(said.target, None);
    assert_eq!(said.suggested.as_deref(), Some("50795"));
    // Named already: nothing to suggest.
    let named = blocked(&plane, &line(Harness::Codex, Some("50795"), Some("6443")));
    assert_eq!(named.suggested, None);
    // No port a grant could name.
    let never = blocked(&plane, &line(Harness::Opencode, None, Some("0")));
    assert_eq!(never.suggested, None);
    // A Claude Code chat's Notice asks for the port too since qw384b: suggested alike.
    let claude = blocked(&plane, &line(Harness::ClaudeCode, None, Some("50795")));
    assert_eq!(claude.offer, BlockOffer::Port);
    assert_eq!(claude.suggested.as_deref(), Some("50795"));
}

/// #2510: a service the runner a chat runs on asks about is offered by its port, once or for
/// this chat alone, and its Notice names the runner, never this Mac's service.
#[test]
fn a_runners_service_is_offered_once_or_for_this_chat_and_named_by_its_runner() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let plane: PlaneId = serde_json::from_value(serde_json::json!(project.path())).unwrap();
    let block = Block {
        operation: Operation::Connect,
        kind: Kind::RunnerService,
        ours: false,
    };
    let told = blocked_on(&plane, 3, &block, None, Some("5432"), false);
    assert_eq!(told.offer, BlockOffer::Port);
    assert_eq!(told.target.as_deref(), Some("5432"));
    assert_eq!(
        told.levels,
        [
            crate::sandboxing::GrantLevel::Once,
            crate::sandboxing::GrantLevel::Chat
        ]
    );
    assert_eq!(
        told.said,
        "a connection to a service on the runner this chat runs on"
    );

    let named = on_its_runner(told, Some("runner-7"));
    let route = named.route.expect("what allowing it means");
    assert!(route.contains("port 5432 of the runner runner-7"));
    assert!(route.contains("nothing on this Mac"));
}

/// Lines the host handed on before the window took its board are heard before any it hands on
/// while they are heard (#2571): a secret exec that waited reads the input that comes for it
/// then, to its end, rather than having it dropped as for no exec.
#[test]
fn a_secret_exec_that_waited_for_the_board_reads_input_handed_on_while_it_is_heard() {
    use purlis_core::hookwire::relay::{Heard, Replied, Reply};
    use std::io::{BufRead, Write};
    use std::sync::TryLockError;
    use std::time::{Duration, Instant};
    const LINK: u64 = 1;
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::relayed(plane, &at, Arc::new(|_| {}), Arc::new(|_| {}));
    hooks.exec_secrets_with(Arc::new(|_ask, reader, mut writer, _told| {
        for line in reader.lines() {
            let _ = writeln!(writer, "ran {}", line.unwrap_or_default());
        }
    }));
    let relayed = hooks.relayed_from_the_host().expect("relayed");
    relayed.linked(LINK);
    let nothing: Reply = Arc::new(|_| {});
    let (replies, replied) = std::sync::mpsc::channel();
    let reply: Reply = Arc::new(move |r| {
        let _ = replies.send(r);
    });
    relayed.take(
        LINK,
        1,
        Heard::AskerGone { connection: 9 },
        Arc::clone(&nothing),
    );
    relayed.take(
        LINK,
        2,
        Heard::SecretExec(purlis_core::secrets::brokered::Ask {
            chat: 3,
            secret_exec: Default::default(),
            ..Default::default()
        }),
        reply,
    );

    // The lines that waited are heard with the line in order held up, the first of them on it.
    let held = relayed.in_order.lock().unwrap();
    let keeping = std::thread::spawn({
        let relayed = Arc::clone(&relayed);
        move || relayed.keep_with(Arc::new(|_, _| {}), None)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match relayed.waiting.try_lock() {
            Err(TryLockError::WouldBlock) => break,
            Ok(waiting) => assert!(
                waiting.is_some(),
                "the lines that waited are no longer held while they are heard: a line handed \
                 on now would be heard first"
            ),
            Err(TryLockError::Poisoned(_)) => panic!("poisoned"),
        }
        assert!(
            Instant::now() < deadline,
            "the lines that waited were never heard"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let handing_on = std::thread::spawn({
        let relayed = Arc::clone(&relayed);
        let nothing = Arc::clone(&nothing);
        move || {
            relayed.take(
                LINK,
                3,
                Heard::Input {
                    of: 2,
                    line: "typed".into(),
                },
                Arc::clone(&nothing),
            );
            relayed.take(LINK, 4, Heard::InputEnded { of: 2 }, nothing);
        }
    });
    drop(held);
    keeping.join().expect("kept");
    handing_on.join().expect("handed on");

    let within = Duration::from_secs(10);
    assert_eq!(
        replied.recv_timeout(within),
        Ok(Replied::Wrote {
            text: "ran typed\n".into()
        })
    );
    assert_eq!(replied.recv_timeout(within), Ok(Replied::Ended));
}

/// #2428: a lookup a program made itself, whether a hook named its host or the wrap refused the
/// system's name lookup at its socket, says what reaching that lookup would mean, and offers
/// neither the host nor the socket.
#[test]
fn a_lookup_past_the_proxy_says_what_reaching_the_system_s_name_lookup_would_mean() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    for target in [None, Some("api.example.com")] {
        let said = blocked(
            &plane,
            &purlis_core::hookwire::SandboxBlocked {
                chat: 3,
                sandbox_blocked: Block {
                    operation: Operation::Lookup,
                    kind: Kind::Host,
                    ours: false,
                },
                harness: Some("codex".to_owned()),
                target: target.map(str::to_owned),
                read_back: false,
                suggested: None,
            }
            .into(),
        );
        assert_eq!(said.offer, BlockOffer::Unsandboxed, "{target:?}");
        assert_eq!(said.choose, None, "{target:?}");
        let route = said.route.expect("why");
        assert!(route.contains("system's name lookup"), "{route}");
        assert!(route.contains("look up any name past the proxy"), "{route}");
    }
}

/// #2424: on Linux, the wrap's supervisor sends the app the block a hook would send. Taken
/// through `blocked_on` for a Codex chat, each kind it tells (a write of another workspace's
/// file, a local socket, a service on this machine) gets the Notice macOS's kernel line of the
/// same refusal gets: the same offer, target and levels.
#[test]
fn a_linux_supervisor_s_block_gets_the_notice_macos_s_kernel_line_of_it_gets() {
    use purlis_core::harness::Harness;
    use purlis_core::sandboxblock::denial::Denial;
    use purlis_core::sandboxblock::supervised::{Refusal, block_of};
    use purlis_core::sandboxblock::{Kind, Place};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let chat = root.join("workspaces/a/repo");
    std::fs::create_dir_all(&chat).expect("the chat's folder");
    std::fs::create_dir_all(root.join("workspaces/b")).expect("another workspace");
    let home = root.join("home");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let place = Place {
        root: &root,
        chat: &chat,
        cwd: &chat,
        home: Some(&home),
    };
    let elsewhere = root.join("workspaces/b/notes.md").display().to_string();
    let notice = |operation: &'static str, target: &str| {
        let refusal = Refusal {
            operation,
            target: target.to_owned(),
            process: Some("sh".to_owned()),
        };
        let (block, named) = block_of(&refusal, &place).expect("a block the supervisor tells");
        let line = format!("Sandbox: sh(4120) deny(1) {operation} {target}");
        let (on_macos, named_on_macos) =
            Denial::of_line(&line).expect("a kernel line").block(&place);
        assert_eq!((block, &named), (on_macos, &named_on_macos), "{line}");
        let from_the_supervisor = blocked_on(
            &plane,
            7,
            &block,
            Some(Harness::Codex),
            named.as_deref(),
            false,
        );
        let from_the_kernel = blocked_on(
            &plane,
            7,
            &on_macos,
            Some(Harness::Codex),
            named_on_macos.as_deref(),
            false,
        );
        assert_eq!(from_the_supervisor, from_the_kernel, "{line}");
        from_the_supervisor
    };

    // A write of another workspace's file: Allow on its folder, for this chat or always. A
    // fixture made in a harness's own temp root is refused with it, as the test above says.
    let write = notice("file-write-create", &elsewhere);
    assert_eq!(write.kind, Kind::ProjectFiles.word());
    assert_eq!(write.harness.as_deref(), Some("codex"));
    let in_a_harness_temp_root = root
        .strip_prefix("/private/tmp")
        .ok()
        .and_then(|below| below.components().next())
        .is_some_and(|first| first.as_os_str().to_string_lossy().starts_with("claude-"));
    if !in_a_harness_temp_root {
        assert_eq!(write.offer, BlockOffer::Write);
        assert_eq!(
            write.target,
            Some(root.join("workspaces/b").display().to_string())
        );
        assert!(
            write.levels.contains(&crate::sandboxing::GrantLevel::Chat) && write.levels.len() > 1,
            "{:?}",
            write.levels
        );
    }

    // A local socket: by its whole path; allowed where a socket's grant compiles (D-sock-7).
    let socket = notice("network-outbound", "/var/run/purlis-test-none.sock");
    assert_eq!(socket.kind, Kind::LocalSocket.word());
    if !cfg!(target_os = "macos") {
        assert_eq!(socket.offer, BlockOffer::Unsandboxed);
    }

    // A service on this machine: by its one port, which the person may allow.
    let service = notice("network-outbound", "127.0.0.1:5432");
    assert_eq!(service.kind, Kind::LocalService.word());
    assert_eq!(service.offer, BlockOffer::Port);
    assert_eq!(service.target.as_deref(), Some("5432"));
    assert!(!service.levels.is_empty(), "{:?}", service.levels);
}

/// #2657: a block's Notice for the system's name lookup offers no Allow, only the chat
/// without the sandbox: for each of its sockets, and on macOS saying why in the grant's own
/// words, and for the Mac's own lookup service.
#[test]
fn a_notice_for_the_systems_name_lookup_offers_no_allow() {
    use purlis_core::sandbox::services::{
        NAME_LOOKUP_REFUSED, NAME_LOOKUP_SERVICES, NAME_LOOKUP_SOCKETS,
    };
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("an id");
    let told = |operation, kind, target: &str| {
        blocked(
            &plane,
            &purlis_core::hookwire::SandboxBlocked {
                chat: 3,
                sandbox_blocked: Block {
                    operation,
                    kind,
                    ours: false,
                },
                harness: None,
                target: Some(target.to_owned()),
                read_back: false,
                suggested: None,
            }
            .into(),
        )
    };
    for lookup in NAME_LOOKUP_SOCKETS {
        let said = told(Operation::Connect, Kind::LocalSocket, lookup);
        assert_eq!(said.offer, BlockOffer::Unsandboxed, "{lookup}");
        if cfg!(target_os = "macos") {
            assert_eq!(said.route.as_deref(), Some(NAME_LOOKUP_REFUSED), "{lookup}");
        }
    }
    for lookup in NAME_LOOKUP_SERVICES {
        let said = told(Operation::Lookup, Kind::SystemService, lookup);
        assert_eq!(said.offer, BlockOffer::Unsandboxed, "{lookup}");
        assert_eq!(said.route.as_deref(), Some(NAME_LOOKUP_REFUSED), "{lookup}");
    }
}

/// A project routed by workspace (#2046): this machine's host's lines and a pool's runner's,
/// each heard through its own, chat 3 here and chat 7 on pool `builds`. Each secret exec runs
/// and says what it read.
fn routed_hearing() -> (Hooks, Arc<Relayed>, Arc<Relayed>) {
    use std::io::{BufRead, Write};
    let dir = tempfile::tempdir().expect("a directory");
    let at = Where {
        within: dir.path().to_path_buf(),
        socket: dir.path().join("app").join("hooks.sock"),
    };
    let plane: PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Hooks::relayed(plane, &at, Arc::new(|_| {}), Arc::new(|_| {}));
    hooks.exec_secrets_with(Arc::new(|ask, reader, mut writer, _told| {
        let _ = writeln!(writer, "chat {}", ask.chat);
        for line in reader.lines() {
            let _ = writeln!(writer, "ran {}", line.unwrap_or_default());
        }
    }));
    let routes: Routes = Arc::new(|chat| (chat == 7).then(|| "builds".to_owned()));
    let here = hooks.relayed_from_the_host().expect("relayed");
    here.routed(Arc::clone(&routes));
    here.keep_with(Arc::new(|_, _| {}), None);
    let builds = here.for_a_pool("builds", routes);
    (hooks, here, builds)
}

/// A secret exec of `chat` taken through `relayed` over `link`, as line `id`, its input ended:
/// what it replied.
fn exec_through(
    relayed: &Relayed,
    link: u64,
    id: u64,
    chat: u32,
) -> Vec<purlis_core::hookwire::relay::Replied> {
    use purlis_core::hookwire::relay::{Heard, Replied, Reply};
    let (replies, replied) = std::sync::mpsc::channel();
    let reply: Reply = Arc::new(move |r| {
        let _ = replies.send(r);
    });
    relayed.take(
        link,
        id,
        Heard::SecretExec(purlis_core::secrets::brokered::Ask {
            chat,
            secret_exec: Default::default(),
            ..Default::default()
        }),
        reply,
    );
    relayed.take(link, id + 1, Heard::InputEnded { of: id }, Arc::new(|_| {}));
    let mut all = Vec::new();
    while let Ok(one) = replied.recv_timeout(std::time::Duration::from_secs(10)) {
        let ended = one == Replied::Ended;
        all.push(one);
        if ended {
            break;
        }
    }
    all
}

#[test]
fn a_desktop_chats_secret_exec_still_runs_once_a_runner_chat_has_started() {
    use purlis_core::hookwire::relay::Replied;
    let (_hooks, here, builds) = routed_hearing();
    here.linked(1);
    // A pool's link holds the project after this machine's, numbering its own lines.
    builds.linked(2);

    assert_eq!(
        exec_through(&here, 1, 10, 3),
        vec![
            Replied::Wrote {
                text: "chat 3\n".into()
            },
            Replied::Ended
        ]
    );
    assert_eq!(
        exec_through(&builds, 2, 10, 7),
        vec![
            Replied::Wrote {
                text: "chat 7\n".into()
            },
            Replied::Ended
        ]
    );
    // The pool's link ending leaves this machine's as it was.
    builds.unlinked(2);
    assert_eq!(exec_through(&here, 1, 20, 3).len(), 2);
}

#[test]
fn a_line_is_heard_only_from_the_host_its_chat_runs_on() {
    use purlis_core::hookwire::relay::Replied;
    let (_hooks, here, builds) = routed_hearing();
    here.linked(1);
    builds.linked(2);

    // The pool's link names this machine's chat 3, and an unknown chat; this machine's names
    // the pool's chat 7. None of them runs.
    assert_eq!(exec_through(&builds, 2, 10, 3), vec![Replied::Ended]);
    assert_eq!(exec_through(&builds, 2, 20, 99), vec![Replied::Ended]);
    assert_eq!(exec_through(&here, 1, 10, 7), vec![Replied::Ended]);
}
