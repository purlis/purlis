// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// #1986: a chat whose program ended before its next pause is restarted by nothing: what
/// it was owed goes, unless a restart of it is under way, which carries what it owes.
#[test]
fn a_chat_whose_program_ended_is_owed_no_restart() {
    let chats = Chats::new();
    lock(&chats.owed).insert(3, vec!["allowed".to_owned()]);
    lock(&chats.owed).insert(4, vec!["allowed".to_owned()]);
    chats.its_program_ended(3);
    assert_eq!(chats.owed_restarts(), [4]);

    chats.claim(4).expect("claimed");
    chats.its_program_ended(4);
    assert_eq!(
        chats.owed_restarts(),
        [4],
        "the restart under way takes what it owes"
    );
    chats.unclaim(4);
}

/// #3808: the GitHub App tokens minted for a chat go when it ends, whether its program ended
/// or the chat was closed, and every chat's when they all end; another chat's stay.
#[test]
fn a_chats_github_app_tokens_go_when_it_ends() {
    use purlis_core::forge::apptoken::{keep_for_a_test, kept_by_a_test};
    let size = Size {
        columns: 80,
        rows: 24,
    };
    // A project of its own: another test ending every chat of the shared one would take these.
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(project.path().join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(Sessions::reporting_to(None)),
        project.path().to_path_buf(),
    );
    let root = chats.project.clone();
    let started = chats
        .start(
            &Chat {
                program: "/bin/sleep".to_owned(),
                args: vec!["5".to_owned()],
                name: "gh".to_owned(),
                ..Default::default()
            },
            size,
        )
        .expect("started");
    keep_for_a_test(&root, started, "closed");
    keep_for_a_test(&root, 9001, "program-ended");
    keep_for_a_test(&root, 9002, "staying");
    chats.close(started).expect("closed");
    assert_eq!(kept_by_a_test(&root, started), 0, "gone as it closed");
    chats.its_program_ended(9001);
    assert_eq!(kept_by_a_test(&root, 9001), 0, "gone as its program ended");
    assert_eq!(kept_by_a_test(&root, 9002), 1, "another chat's stays");
    chats.end_all();
    assert_eq!(kept_by_a_test(&root, 9002), 0, "gone as every chat ended");
}

/// #1353: past the hosts it names, a wrapped chat's proxy tells the count under the chat's
/// number, as it grows, once the chat has one and only to a listener there is.
#[test]
fn the_hosts_a_wrapped_chats_proxy_refused_past_the_ones_it_names_are_told_as_a_count() {
    use purlis_core::sandbox::egress::{REFUSALS_KEPT, Refusals};
    let hear: Arc<Mutex<Option<super::MoreRefused>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let refusals = Refusals::default().telling_more(super::more_refused_by_the_proxy(
        Arc::clone(&hear),
        Arc::clone(&whose),
    ));
    for n in 0..REFUSALS_KEPT {
        refusals.heard(&format!("kept{n}.example"), 443);
    }
    // Nobody listens yet: nothing is told, and nothing breaks.
    refusals.heard("more0.example", 443);
    let told: Arc<Mutex<Vec<(u32, u32)>>> = Arc::default();
    *lock(&hear) = Some({
        let told = Arc::clone(&told);
        Arc::new(move |chat, more| lock(&told).push((chat, more)))
    });
    // No number yet: not told under 0.
    refusals.heard("more1.example", 443);
    whose.store(7, std::sync::atomic::Ordering::SeqCst);
    refusals.heard("more2.example", 443);
    assert_eq!(*lock(&told), [(7, 3)]);
}

/// #1663: each host purlis's own proxy refused a chat it wraps is that chat's block, named
/// whole by the proxy, once the chat has its number, and only to a listener there is.
#[test]
fn a_host_the_proxy_refused_a_wrapped_chat_is_its_block_by_the_proxys_word() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let refusals =
        super::refused_by_the_proxy(Arc::clone(&hear), Arc::clone(&whose), Some(Harness::Codex));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    // Nobody listening yet, then no number yet: nothing is told.
    refusals.heard("early.example.com", 443);
    *lock(&hear) = Some(Arc::new(move |block| lock(&tx).send(block).unwrap()));
    refusals.heard("before.example.com", 443);
    whose.store(12, std::sync::atomic::Ordering::SeqCst);
    refusals.heard("api.example.com", 8443);
    let told = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("told");
    assert_eq!(
        told,
        purlis_core::hookwire::SandboxBlocked {
            chat: 12,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::Host,
                ours: false,
            },
            harness: Some("codex".to_owned()),
            target: Some("api.example.com:8443".to_owned()),
            read_back: false,
            suggested: None,
        }
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "once"
    );
}

/// #1683: a host the proxy refused before the chat had its number, or before anyone
/// listened, is not dropped: it is told under the chat's number once both are there, once.
#[test]
fn a_host_the_proxy_refused_before_the_chat_had_its_number_is_told_once_it_has() {
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let refusals =
        super::refused_by_the_proxy(Arc::clone(&hear), Arc::clone(&whose), Some(Harness::Codex));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let none_yet = |rx: &std::sync::mpsc::Receiver<purlis_core::hookwire::SandboxBlocked>| {
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err()
    };
    // Nobody listening, then no number: kept, not told.
    refusals.heard("early.example.com", 443);
    refusals.tell_untold();
    *lock(&hear) = Some(Arc::new(move |block| lock(&tx).send(block).unwrap()));
    refusals.heard("before.example.com", 443);
    refusals.tell_untold();
    assert!(none_yet(&rx), "no number yet");
    // The chat is given its number: each is told under it, oldest first, once.
    whose.store(12, std::sync::atomic::Ordering::SeqCst);
    refusals.clone().tell_untold();
    let mut told: Vec<(u32, Option<String>)> = (0..2)
        .map(|_| {
            let block = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("told");
            (block.chat, block.target)
        })
        .collect();
    told.sort();
    assert_eq!(
        told,
        [
            (12, Some("before.example.com:443".to_owned())),
            (12, Some("early.example.com:443".to_owned())),
        ]
    );
    refusals.tell_untold();
    refusals.heard("early.example.com", 443);
    assert!(none_yet(&rx), "once");
}

/// #1709: where no Notice can be raised, because the project's hooks are not listening yet
/// or the chat has no number yet, a connection nothing lists is refused at once, never held
/// its minute for an ask nobody sees.
#[test]
fn with_no_notice_to_raise_a_held_connection_is_refused_at_once() {
    use purlis_core::sandbox::asks::Answer;
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let board = super::asked_by_the_proxy(
        super::Asking {
            hear: Arc::clone(&hear),
            reached: Arc::new(Mutex::new(None)),
            whose: Arc::clone(&whose),
            who: super::ReachedAs::default(),
            harness: Some(Harness::ClaudeCode),
            blocks: Arc::default(),
            moved: Arc::new(Mutex::new(None)),
            stops: Arc::new(Mutex::new(None)),
        },
        std::env::temp_dir().join("purlis-no-such-project-1709"),
    )
    .board();
    let quick = |host: &str| {
        let started = std::time::Instant::now();
        let answer = board.hold(host, 443, &[443]);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "not held"
        );
        answer
    };
    // Neither listening nor numbered, then listening with no number, then numbered with
    // nobody listening.
    assert_eq!(quick("one.example.com"), Answer::NobodyToAsk);
    *lock(&hear) = Some(Arc::new(|_| {}));
    assert_eq!(quick("two.example.com"), Answer::NobodyToAsk);
    *lock(&hear) = None;
    whose.store(9, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(quick("three.example.com"), Answer::NobodyToAsk);
    assert_eq!(board.holding(), 0);
}

/// #1709: each live ask is held for its chat as the proxy asks, so the asks registry lists
/// it and an Allow or Keep blocked through the registry is checked against it, whether or
/// not its Notice got past the throttle; and the window is told its asks moved.
#[test]
fn a_live_ask_is_held_for_its_chat_and_the_window_told_though_no_notice_went_up() {
    use crate::sandboxing::GrantWhat;
    use crate::taskblocks::HeldBlock;
    use purlis_core::sandbox::asks::Answer;
    // Something listens, but the throttle held this Notice back: nothing reaches it.
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> =
        Arc::new(Mutex::new(Some(Arc::new(|_| {}))));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(9));
    let blocks: Arc<crate::taskblocks::Blocks> = Arc::default();
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let moved: Arc<Mutex<Option<super::AsksMoved>>> =
        Arc::new(Mutex::new(Some(Arc::new(move || {
            let _ = lock(&tx).send(());
        }))));
    let board = super::asked_by_the_proxy(
        super::Asking {
            hear,
            reached: Arc::new(Mutex::new(None)),
            whose,
            who: super::ReachedAs::default(),
            harness: Some(Harness::ClaudeCode),
            blocks: Arc::clone(&blocks),
            moved,
            stops: Arc::new(Mutex::new(None)),
        },
        std::env::temp_dir().join("purlis-no-such-project-1709-2"),
    )
    .board();
    let held = {
        let board = Arc::clone(&board);
        std::thread::spawn(move || board.hold("API.example.com", 443, &[443]))
    };
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("the window is told its asks moved");
    let host = HeldBlock {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: "api.example.com".to_owned(),
    };
    assert!(blocks.holds(9, &host), "{:?}", blocks.every());
    assert_eq!(blocks.every().len(), 1, "held once, as a grant names it");
    board.keep_blocked(&purlis_core::sandbox::grant::host("api.example.com").expect("a host"));
    assert_eq!(held.join().expect("answered"), Answer::Refused);
}

/// #1709: a burst of live asks, more hosts than one Notice lists, keeps every one held for
/// its chat while its connection waits, so none of them is pushed out of the registry
/// unanswerable.
#[test]
fn a_burst_of_live_asks_keeps_each_held_while_its_connection_waits() {
    use purlis_core::sandbox::asks::Answer;
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> =
        Arc::new(Mutex::new(Some(Arc::new(|_| {}))));
    let blocks: Arc<crate::taskblocks::Blocks> = Arc::default();
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let moved: Arc<Mutex<Option<super::AsksMoved>>> =
        Arc::new(Mutex::new(Some(Arc::new(move || {
            let _ = lock(&tx).send(());
        }))));
    let board = super::asked_by_the_proxy(
        super::Asking {
            hear,
            reached: Arc::new(Mutex::new(None)),
            whose: Arc::new(std::sync::atomic::AtomicU32::new(9)),
            who: super::ReachedAs::default(),
            harness: Some(Harness::ClaudeCode),
            blocks: Arc::clone(&blocks),
            moved,
            stops: Arc::new(Mutex::new(None)),
        },
        std::env::temp_dir().join("purlis-no-such-project-1709-3"),
    )
    .board();
    let many = purlis_core::sandboxblock::AT_MOST_PER_RESULT + 2;
    let hosts: Vec<String> = (0..many).map(|at| format!("h{at}.example.com")).collect();
    let held: Vec<_> = hosts
        .iter()
        .map(|host| {
            let (board, host) = (Arc::clone(&board), host.clone());
            std::thread::spawn(move || board.hold(&host, 443, &[443]))
        })
        .collect();
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("the window is told its asks moved");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while blocks.every().len() < many && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    let listed: Vec<String> = blocks.every().into_iter().map(|(_, b)| b.target).collect();
    for host in &hosts {
        assert!(listed.contains(host), "{host} in {listed:?}");
    }
    for host in &hosts {
        board.keep_blocked(&purlis_core::sandbox::grant::host(host).expect("a host"));
    }
    for one in held {
        assert_eq!(one.join().expect("answered"), Answer::Refused);
    }
}

/// R-290 (Q154): a held connection's refusal has one home at every moment. While the chat
/// waits on it, it is an ask in the registry and no block; once its hold times out, the ask is
/// withdrawn and one Sandbox block is written, naming the host as a grant names it and the tool
/// the chat's call in flight was, by the harness's name. A timeout told again writes no second
/// block. The chat's next call is the other way it stops waiting ([`moved_on`], Q240), and that
/// too reaches a block only through the timeout told here.
#[test]
fn a_sandbox_refusal_leaves_asks_and_becomes_one_block_when_the_chat_stops_waiting() {
    use crate::sandboxing::GrantWhat;
    use crate::taskblocks::HeldBlock;
    use purlis_core::eventlog::stops::{Block, Source, Stop, Target};
    use purlis_core::sandbox::asks::Heard;
    let chats = Chats::new();
    let written: Arc<Mutex<Vec<(u32, Stop)>>> = Arc::default();
    chats.write_stops_with({
        let written = Arc::clone(&written);
        Arc::new(move |chat, stop, _| {
            lock(&written).push((chat, stop));
            Ok(())
        })
    });
    let tell = super::asked_by_the_proxy(
        super::Asking {
            hear: Arc::new(Mutex::new(Some(Arc::new(|_| {})))),
            reached: Arc::new(Mutex::new(None)),
            whose: Arc::new(std::sync::atomic::AtomicU32::new(9)),
            who: super::ReachedAs::default(),
            harness: Some(Harness::ClaudeCode),
            blocks: Arc::clone(&chats.blocks),
            moved: Arc::new(Mutex::new(None)),
            stops: Arc::clone(&chats.stops),
        },
        std::env::temp_dir().join("purlis-no-such-project-290"),
    )
    .tell;
    let call = |tool: &str| purlis_core::hookwire::ToolCall {
        chat: 9,
        tool_hook: "pretooluse".to_owned(),
        tool: Some(tool.to_owned()),
        call: None,
        args: None,
        decision: purlis_core::hookwire::Decision::default(),
        rule: None,
        arm: None,
        hook_ms: 0,
        agent: None,
        at_ms: 0,
        skill: None,
    };
    let asked = |host: &str| {
        chats.blocks().every_since().iter().any(|(chat, block, _)| {
            *chat == 9
                && *block
                    == HeldBlock {
                        operation: "connect".to_owned(),
                        kind: "host".to_owned(),
                        what: GrantWhat::Host,
                        target: host.to_owned(),
                    }
        })
    };
    let blocks_of = |target: &str| -> Vec<Block> {
        lock(&written)
            .iter()
            .filter_map(|(chat, stop)| match stop {
                Stop::Block(block)
                    if *chat == 9
                        && block.target == Some(Target::host(target).expect("a host")) =>
                {
                    Some(block.clone())
                }
                _ => None,
            })
            .collect()
    };

    // The timeout: the chat's call in flight is a Bash command whose connection is held.
    chats.called(&call("Bash"));
    tell(Heard::Asked(vec!["slow.example.com:443".to_owned()]));
    assert!(asked("slow.example.com"), "an ask while the chat waits");
    assert!(blocks_of("slow.example.com").is_empty(), "and no block");
    tell(Heard::TimedOut("slow.example.com:443".to_owned()));
    assert!(!asked("slow.example.com"), "the ask is withdrawn");
    let timed_out = blocks_of("slow.example.com");
    assert_eq!(timed_out.len(), 1, "one block: {timed_out:?}");
    assert_eq!(timed_out[0].source, Source::Sandbox);
    assert_eq!(timed_out[0].tool, "Bash");

    // Its connection is told timed out again: the refusal is a block already, never a second.
    tell(Heard::TimedOut("slow.example.com:443".to_owned()));
    assert_eq!(blocks_of("slow.example.com").len(), 1, "never a second");
}

/// #1664: a local address the proxy refused is the chat's Block of a local socket, naming no
/// host, since no Allow would let it through.
#[test]
fn a_local_address_the_proxy_refused_is_a_block_naming_no_host() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(7));
    let refusals = super::refused_by_the_proxy(
        Arc::clone(&hear),
        Arc::clone(&whose),
        Some(Harness::Opencode),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    *lock(&hear) = Some(Arc::new(move |block| lock(&tx).send(block).unwrap()));
    refusals.heard_local("169.254.169.254", 80);
    let told = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("told");
    assert_eq!(
        told,
        purlis_core::hookwire::SandboxBlocked {
            chat: 7,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::LocalSocket,
                ours: false,
            },
            harness: Some("opencode".to_owned()),
            target: None,
            read_back: false,
            suggested: None,
        }
    );
    assert!(
        refusals.refused().is_empty(),
        "never kept as a host to allow"
    );
}

/// qw384: this machine's loopback at a port, refused by the proxy, is the chat's Block of a
/// service on this Mac, naming the port its Notice offers to allow.
#[test]
fn a_loopback_port_the_proxy_refused_is_a_block_of_a_service_on_this_mac() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let hear: Arc<Mutex<Option<crate::hooks::Blocks>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(7));
    let refusals =
        super::refused_by_the_proxy(Arc::clone(&hear), Arc::clone(&whose), Some(Harness::Codex));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    *lock(&hear) = Some(Arc::new(move |block| lock(&tx).send(block).unwrap()));
    refusals.heard_local("127.0.0.1", 50795);
    let told = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("told");
    assert_eq!(
        told,
        purlis_core::hookwire::SandboxBlocked {
            chat: 7,
            sandbox_blocked: Block {
                operation: Operation::Connect,
                kind: Kind::LocalService,
                ours: false,
            },
            harness: Some(Harness::Codex.name().to_owned()),
            target: Some("50795".to_owned()),
            read_back: false,
            suggested: None,
        }
    );
}

/// #1664: what a chat's proxy carried is told under the chat whose proxy it is, once the
/// chat has its number, with who it is as known when it started.
#[test]
fn connections_the_proxy_carried_are_told_under_its_chat() {
    type Told = (
        u32,
        Option<String>,
        Option<String>,
        Option<String>,
        &'static str,
        u64,
    );
    let hear: Arc<Mutex<Option<super::ProxyReached>>> = Arc::new(Mutex::new(None));
    let whose = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let reached = super::reached_by_the_proxy(
        Arc::clone(&hear),
        Arc::clone(&whose),
        super::ReachedAs {
            id: Some("01JCHAT".to_owned()),
            persona: Some("steward".to_owned()),
        },
    );
    let (tx, rx) = std::sync::mpsc::channel::<Told>();
    let tx = Mutex::new(tx);
    reached(Some("early.example.com:443"), "open", 1);
    *lock(&hear) = Some(Arc::new(move |chat, who, target, by, times| {
        lock(&tx)
            .send((
                chat,
                who.id.clone(),
                who.persona.clone(),
                target.map(str::to_owned),
                by,
                times,
            ))
            .unwrap();
    }));
    reached(Some("before.example.com:443"), "open", 1);
    whose.store(5, std::sync::atomic::Ordering::SeqCst);
    reached(Some("api.example.com:443"), "you", 3);
    let told = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("told");
    assert_eq!(
        told,
        (
            5,
            Some("01JCHAT".to_owned()),
            Some("steward".to_owned()),
            Some("api.example.com:443".to_owned()),
            "you",
            3
        )
    );
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "nothing before the chat had its number"
    );
}

#[test]
fn a_chats_hooks_are_told_its_folder_and_only_a_sandboxed_chat_is_called_sandboxed() {
    let of = |env: &[(String, String)], name: &str| {
        env.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    };
    let mut env = vec![
        ("PURLIS_SANDBOXED".to_owned(), "1".to_owned()),
        ("PATH".to_owned(), "/bin".to_owned()),
    ];
    what_its_hooks_read(
        &mut env,
        Some(Path::new("/plane/workspaces/a")),
        false,
        None,
    );
    assert_eq!(
        of(&env, "PURLIS_SANDBOXED"),
        None,
        "a profile cannot claim it"
    );
    assert_eq!(
        of(&env, "PURLIS_CHAT_DIR").as_deref(),
        Some("/plane/workspaces/a")
    );
    what_its_hooks_read(&mut env, Some(Path::new("/plane/workspaces/a")), true, None);
    assert_eq!(of(&env, "PURLIS_SANDBOXED").as_deref(), Some("1"));
    assert_eq!(
        env.iter().filter(|(n, _)| n == "PURLIS_CHAT_DIR").count(),
        1
    );
    assert!(purlis_core::sandbox::chat_is_sandboxed_in(&|name| of(
        &env, name
    )));
}

#[test]
fn a_chats_harness_is_told_its_id_by_the_app_and_never_by_a_profile() {
    // #1457: the id its status line keeps the session's cost under.
    let of = |env: &[(String, String)], name: &str| {
        env.iter()
            .filter(|(n, _)| purlis_core::envvar::same(n, name))
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
    };
    let mut env = vec![
        ("PURLIS_CHAT_ULID".to_owned(), "someone-else".to_owned()),
        ("CHARTER_CHAT_ULID".to_owned(), "someone-else".to_owned()),
        ("CHARTER_SANDBOXED".to_owned(), "1".to_owned()),
    ];
    what_its_hooks_read(&mut env, None, false, Some("01J9ZQ3V7K8M2N4P6R8T0V2X4Z"));
    assert_eq!(
        of(&env, purlis_core::hookwire::CHAT_ID_ENV),
        vec!["01J9ZQ3V7K8M2N4P6R8T0V2X4Z".to_owned()]
    );
    assert!(
        of(&env, purlis_core::hookwire::SANDBOXED_ENV).is_empty(),
        "a profile cannot claim it under its old name either"
    );
    what_its_hooks_read(&mut env, None, false, None);
    assert!(of(&env, purlis_core::hookwire::CHAT_ID_ENV).is_empty());
}

/// ADR 0093 (D-sock-3): a socket allowed once reaches the run that takes it, the restart
/// that starts while the session it was allowed in is still open, and goes when that
/// session ends, so no later start has it. Allowed for the chat after, it lasts.
#[test]
fn a_socket_allowed_once_goes_when_the_session_it_was_allowed_in_ends() {
    use purlis_core::sandbox::grant::What;
    let chats = Chats::new();
    let size = Size {
        columns: 80,
        rows: 24,
    };
    let chat = Chat {
        program: "/bin/sleep".to_owned(),
        args: vec!["5".to_owned()],
        name: "docker".to_owned(),
        ..Default::default()
    };
    let first = chats.start(&chat, size).expect("started");
    let docker = What::Socket("/var/run/docker.sock".into());
    chats
        .hold_grant_once(first, docker.clone(), 1)
        .expect("held once");
    let again = lock(&chats.open)[&first].chat.clone();
    let id = again.identity.id.clone().expect("an id");
    // The restart starts while the first session is open: it takes the grant.
    assert_eq!(
        chats.grants_of(&again).sockets,
        vec![std::path::PathBuf::from("/var/run/docker.sock")]
    );
    let second = chats.start(&again, size).expect("restarted");
    assert!(
        chats.chat_grants().iter().any(|(_, one)| one.once),
        "listed as once"
    );
    chats.close(first).expect("closed");
    assert!(
        !chats.holds(&id, &docker),
        "gone with the session it was allowed in"
    );
    assert!(chats.grants_of(&again).sockets.is_empty());

    // Allowed once, then for the chat: it lasts past the session.
    chats
        .hold_grant_once(second, docker.clone(), 2)
        .expect("once");
    chats
        .hold_grant(second, docker.clone(), 3)
        .expect("for the chat");
    let third = chats.start(&again, size).expect("restarted");
    chats.close(second).expect("closed");
    assert!(chats.holds(&id, &docker));
    chats.close(third).expect("closed");
}

#[test]
fn a_chats_grants_are_its_own_queue_its_restart_and_end_when_it_closes() {
    // #1342 and #1348 (D-1348-1).
    use purlis_core::sandbox::grant::What;
    let chats = Chats::new();
    let size = Size {
        columns: 80,
        rows: 24,
    };
    let chat = Chat {
        program: "/bin/sleep".to_owned(),
        args: vec!["5".to_owned()],
        name: "granted".to_owned(),
        ..Default::default()
    };
    let session = chats.start(&chat, size).expect("started");
    let other = chats.start(&chat, size).expect("started");
    let host = What::Host(purlis_core::sandbox::hosts::Host::parse("a.example").unwrap());
    chats
        .grant(session, What::Write("/tmp/x".into()), 1, "first".to_owned())
        .expect("granted");
    chats
        .grant(session, host.clone(), 2, "second".to_owned())
        .expect("granted");

    assert_eq!(chats.chat_grants().len(), 2);
    assert_eq!(chats.owed_restarts(), [session]);
    // Queued, not replaced: the restart tells both.
    assert_eq!(lock(&chats.owed)[&session], ["first", "second"]);
    // Never another chat's.
    let id = |n: u32| {
        lock(&chats.open)[&n]
            .chat
            .identity
            .id
            .clone()
            .expect("an id")
    };
    assert!(chats.holds(&id(session), &host));
    assert!(!chats.holds(&id(other), &host));
    // A chat with no conversation yet is not restarted, and keeps what it is owed.
    let refused = chats.restart(session, size).unwrap_err();
    assert!(refused.contains("no conversation to resume"), "{refused}");
    assert_eq!(chats.owed_restarts(), [session]);
    assert_eq!(
        lock(&chats.owed)[&session],
        ["first", "second"],
        "put back whole"
    );
    assert!(
        lock(&chats.restarting).is_empty(),
        "the claim is given back"
    );

    // One restart at a time: a second, from either way in, is refused while one runs, and
    // a grant made meanwhile queues rather than being lost.
    chats.claim(session).expect("claimed");
    let twice = chats.restart(session, size).unwrap_err();
    assert!(twice.contains("already starting"), "{twice}");
    let twice = chats.restart_without_sandbox(session, size).unwrap_err();
    assert!(twice.contains("already starting"), "{twice}");
    chats.owe_restart(session, "third".to_owned());
    assert_eq!(lock(&chats.owed)[&session], ["first", "second", "third"]);
    chats.unclaim(session);

    let gone = id(session);
    chats.close(session).ok();
    assert!(chats.chat_grants().is_empty());
    assert!(
        !chats.holds(&gone, &host),
        "a closed chat's grants end with it"
    );
    assert!(chats.owed_restarts().is_empty());
    // A closed chat is owed nothing (a grant for you made from its Notice reaches its next
    // start instead).
    chats.owe_restart(session, "late".to_owned());
    assert!(chats.owed_restarts().is_empty());
    chats.close(other).ok();
}

/// Needs a terminal (#1538, V100-57). What a start compiles into a chat's sandbox: a task
/// started from a session that holds a grant is compiled without it, and the session's own
/// next start with it.
#[test]
fn a_task_started_from_a_session_holding_a_grant_compiles_its_sandbox_without_it() {
    use purlis_core::reopen::{HandedFrom, Mode, Owed};
    use purlis_core::sandbox::grant::{Grants, What};
    let compiled: std::sync::Arc<Mutex<Vec<(String, Grants)>>> = std::sync::Arc::default();
    let seen = std::sync::Arc::clone(&compiled);
    let chats = Chats::new().deciding_by(Box::new(move |chat, grants, _| {
        lock(&seen).push((chat.name.clone(), grants.clone()));
        Ok((None, None, None))
    }));
    let size = Size {
        columns: 80,
        rows: 24,
    };
    let plain = Chat {
        program: "/bin/sleep".to_owned(),
        args: vec!["5".to_owned()],
        name: "steward 1".to_owned(),
        ..Default::default()
    };
    let session = chats.start(&plain, size).expect("started");
    let host = What::Host(purlis_core::sandbox::hosts::Host::parse("a.example").unwrap());
    chats
        .grant(session, host.clone(), 1, "allowed".to_owned())
        .expect("granted to the session");

    let task = Chat {
        name: "talk".to_owned(),
        from: Some(HandedFrom {
            id: None,
            chat: session,
            name: "steward 1".to_owned(),
            workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
            report: Owed::Due,
            mode: Mode::Task,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
        }),
        ..plain.clone()
    };
    let talk = chats.start(&task, size).expect("started");
    let of = |name: &str| {
        lock(&compiled)
            .iter()
            .filter(|(chat, _)| chat == name)
            .map(|(_, grants)| grants.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        of("talk"),
        [Grants::default()],
        "the task starts with none of it"
    );
    // The session's own next start is compiled with it: the same seam, on its record.
    let again = lock(&chats.open)[&session].chat.clone();
    let mut held = Grants::default();
    held.add(&host);
    assert_eq!(chats.grants_of(&again), held);
    assert_eq!(
        chats.grants_of(&lock(&chats.open)[&talk].chat),
        Grants::default()
    );
    for chat in [talk, session] {
        chats.close(chat).ok();
    }
}

#[test]
fn a_chat_is_announced_before_its_program_starts() {
    // A harness fires `SessionStart` at its own exec, so anything that learned the chat's
    // number afterwards would miss it — and for a chat that is then idle, waiting for a
    // first prompt, no second event ever comes. That is every chat of a relaunch.
    //
    // The order is the whole point: the announcement must land before the program can
    // have run at all.
    use std::sync::mpsc;

    let chats = Chats::new();
    let (tx, rx) = mpsc::channel();
    chats.when_one_starts(Box::new(move |session, harness, conversation, _| {
        let _ = tx.send((session, harness, conversation));
    }));

    let session = chats
        .start(
            &Chat {
                // A program that prints and stops at once: by the time `start` returns it
                // may already be gone, so an announcement made afterwards could be too
                // late even in this test.
                program: "/bin/echo".to_owned(),
                args: vec!["hello".to_owned()],
                cwd: None,
                name: "ide.7".to_owned(),
                resume: None,
                active: false,
                profile: None,
                persona: None,
                show_footer: false,
                pinned: false,
                number: None,
                label: None,
                from: None,
                renamed_from: None,
                ..Default::default()
            },
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("the chat starts");

    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)),
        Ok((session, None, None)),
        "the board was not told about the chat"
    );
}

#[test]
fn a_chat_is_announced_before_its_program_could_have_run_a_single_byte() {
    // **The ORDER is the fix, and a surviving mutant proved the first test does not check
    // it**: moving the announcement after the spawn still delivers it, so the defect
    // could come back silently. Checking what the app had bookkept was no better — that
    // happens after the spawn either way.
    //
    // So the announcement WAITS, briefly, for something only a running program could
    // make. A program that has not been started cannot make it however long we wait; one
    // that has makes it in milliseconds. The wait is what turns an ordering into
    // something a test can see.
    use std::sync::{Arc, Mutex};

    let dir = tempfile::tempdir().expect("a directory");
    let mark = dir.path().join("the-program-ran");
    let seen = Arc::new(Mutex::new(None));

    let chats = Chats::new();
    chats.when_one_starts({
        let mark = mark.clone();
        let seen = Arc::clone(&seen);
        Box::new(move |_, _, _, _| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(750);
            while std::time::Instant::now() < deadline && !mark.exists() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            *seen.lock().expect("not poisoned") = Some(mark.exists());
        })
    });

    chats
        .start(
            &Chat {
                program: "/bin/sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    format!("touch {}; exec sleep 30", mark.display()),
                ],
                cwd: None,
                name: "ide.7".to_owned(),
                resume: None,
                active: false,
                profile: None,
                persona: None,
                show_footer: false,
                pinned: false,
                number: None,
                label: None,
                from: None,
                renamed_from: None,
                ..Default::default()
            },
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("the chat starts");

    assert_eq!(
        *seen.lock().expect("not poisoned"),
        Some(false),
        "the program had already run when the board was told about its chat"
    );
}

#[test]
fn a_chat_that_was_announced_and_then_did_not_start_is_taken_back() {
    // The announcement must come before the program, so it can be about a chat that never
    // happens. Without taking it back, the board holds one entry per failed start for the
    // life of the app.
    use std::sync::{Arc, Mutex};

    let chats = Chats::new();
    let announced = Arc::new(Mutex::new(Vec::new()));
    let taken_back = Arc::new(Mutex::new(Vec::new()));
    chats.when_one_starts({
        let announced = Arc::clone(&announced);
        Box::new(move |session, _, _, _| announced.lock().expect("not poisoned").push(session))
    });
    chats.when_one_does_not_start({
        let taken_back = Arc::clone(&taken_back);
        Box::new(move |session| taken_back.lock().expect("not poisoned").push(session))
    });

    let refused = chats.start(
        &Chat {
            program: "/no/such/program/anywhere".to_owned(),
            args: Vec::new(),
            cwd: None,
            name: "ide.7".to_owned(),
            resume: None,
            active: false,
            profile: None,
            persona: None,
            show_footer: false,
            pinned: false,
            number: None,
            label: None,
            from: None,
            renamed_from: None,
            ..Default::default()
        },
        Size {
            columns: 80,
            rows: 24,
        },
    );

    assert!(refused.is_err(), "a program that is not there started");
    let announced = announced.lock().expect("not poisoned").clone();
    assert_eq!(announced.len(), 1, "it was never announced");
    assert_eq!(*taken_back.lock().expect("not poisoned"), announced);
}

#[test]
fn a_chat_is_announced_with_the_harness_and_conversation_it_was_started_under() {
    // What the board needs in order to judge a report: which rulebook, and which
    // conversation charter chose. A `claude` nested in the chat's shell reports a
    // different one, and that is the whole of what keeps it out (ADR 0024, C5).
    use std::sync::mpsc;

    let chats = Chats::new();
    let (tx, rx) = mpsc::channel();
    chats.when_one_starts(Box::new(move |session, harness, conversation, graded| {
        let _ = tx.send((session, harness, conversation, graded));
    }));

    // A program named `claude` is what `Harness::of_command` reads, and it is the file
    // name that decides — so this is a Claude Code chat as far as the app is concerned. It
    // answers `--version` as Claude Code does, which the floor's start asks (#2071).
    let dir = tempfile::tempdir().expect("a directory");
    let claude = stand_in::claude_code(dir.path(), "claude", "#!/bin/sh\necho \"$@\"\n");

    chats
        .start(
            &Chat {
                program: claude.display().to_string(),
                args: Vec::new(),
                cwd: None,
                name: "ide.7".to_owned(),
                resume: None,
                active: false,
                profile: None,
                persona: None,
                show_footer: false,
                pinned: false,
                number: None,
                label: None,
                from: None,
                renamed_from: None,
                ..Default::default()
            },
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("the chat starts");

    let (_, harness, conversation, graded) = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the board was told");
    assert_eq!(harness, Some(Harness::ClaudeCode));
    // Charter chooses Claude Code's id at the start, so the board has it before the
    // harness has said anything.
    assert!(
        conversation.is_some(),
        "the chosen conversation was not passed on"
    );
    // Not a grade's, so the board queues it as ever (#3583): a grade's chat, which starts only
    // in a folder a grade laid out, is announced as one, and the board's own tests hold that
    // such a chat is never queued.
    assert!(!graded, "a chat of the person's is announced as no grade's");
}
use purlis_core::harness::SessionId;
use purlis_core::reopen::Fresh;
use purlis_core::reopen::layout::{Direction, EVEN, Layout, Node, Pane, PaneView, Tab};

use super::*;
use crate::host::pretend::Pretend;

/// #2071: a start is made as the app's record says: a chat another chat asked for names
/// it, at its first start and every later one; the person's own is theirs.
#[test]
fn a_start_is_made_as_the_record_says_and_a_dispatched_chat_names_its_asker() {
    use purlis_core::reopen::{HandedFrom, Mode as Moved, Owed as Report};
    use purlis_core::sandbox::{Asking, Made};
    let from = |mode, above: Option<Vec<Option<String>>>, by_person| HandedFrom {
        id: None,
        chat: 1,
        name: "steward 1".to_owned(),
        workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
        report: Report::Due,
        mode,
        depth: 1,
        root: None,
        above,
        by_person,
        long: false,
    };
    let chat = |from| Chat {
        from,
        ..Chat::default()
    };
    let steward = Some(vec![Some("steward".to_owned())]);
    assert_eq!(
        made_of(
            &chat(Some(from(Moved::Task, steward.clone(), false))),
            Why::Relaunch
        ),
        Made::Asked {
            how: Asking::Dispatch,
            above: Some(vec![Some("steward".to_owned())]),
        }
    );
    assert_eq!(
        made_of(
            &chat(Some(from(Moved::Handoff, steward.clone(), false))),
            Why::New
        ),
        Made::Asked {
            how: Asking::Handoff,
            above: Some(vec![Some("steward".to_owned())]),
        }
    );
    // A record that cannot say who asked names nobody.
    assert_eq!(
        made_of(&chat(Some(from(Moved::Task, None, false))), Why::New),
        Made::Asked {
            how: Asking::Dispatch,
            above: None,
        }
    );
    // Asked for by the person from a tab, or no other chat at all: the person's.
    assert_eq!(
        made_of(&chat(Some(from(Moved::Task, steward, true))), Why::New),
        Made::Window
    );
    assert_eq!(made_of(&chat(None), Why::New), Made::Window);
    assert_eq!(made_of(&chat(None), Why::Relaunch), Made::Relaunch);
}

const SIZE: Size = Size {
    columns: 80,
    rows: 24,
};
const ID: &str = "11111111-2222-4333-8444-555555555555";
/// A conversation the chat is not in: a nested harness's.
const OTHER: &str = "66666666-7777-4888-9999-000000000000";

/// A program called `name` that is there, beside every test's project and never in one, and
/// answers `--version` as Claude Code does: a harness chat starts at the floor (#2071), which
/// checks the program it starts even where a pretend host runs nothing.
fn standing(name: &str) -> String {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| tempfile::tempdir().expect("a folder for the stand-ins"));
    stand_in::claude_code(dir.path(), name, "#!/bin/sh\nexec sleep 600\n")
        .display()
        .to_string()
}

/// A directory holding a program called `claude` that prints the arguments it was given
/// and then waits, so a test can see what the app actually put on its command line. It
/// answers `--version` as Claude Code does, which a sandboxed start asks (#2071).
fn a_claude(dir: &std::path::Path) -> String {
    // Through `stand_in::program`, which holds both halves of this. The rename is what
    // charter-app#39 needed: the stand-in ends in `sleep 600`, so an earlier chat still
    // has it open for execution, and writing a running program is ETXTBSY — measured at
    // 2 failures in 5 runs, and because this binary runs first, `cargo test` stopped and
    // every later test binary was SKIPPED. The write from a child is what charter-app#81
    // needed, in the other direction: a chat runs this the moment it is written.
    stand_in::claude_code(
            dir,
            "claude",
            "#!/bin/sh\nprintf 'argv:'\nfor word in \"$@\"; do printf ' %s' \"$word\"; done\nprintf '\\n'\nexec sleep 600\n",
        )
        .display()
        .to_string()
}

/// A project that leaves the sandbox off: what chats are of when their project is not what
/// a test is about. It has a manifest, because a project whose manifest is missing starts
/// no harness chat (D-1410e). Made once per test process, in a folder of its own.
pub(crate) fn no_project() -> PathBuf {
    static ONE: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    ONE.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a project");
        std::fs::write(dir.path().join(purlis_core::plane::MANIFEST), "").expect("a manifest");
        dir
    })
    .path()
    .to_path_buf()
}

/// **Whether a Claude Code `chat` on no profile, in the project at `project`, starts as in a
/// project that leaves the sandbox off**, in a test of something else: on Linux, where every
/// sandboxed Claude Code chat runs inside purlis's wrap, on a sandbox-only profile (ADR 0067 as
/// amended 2026-11-10), which a chat on no profile does not have, and only where neither the
/// project nor your level sets a sandbox, so a test that sets one still decides it as the app
/// does.
pub(crate) fn claude_left_unsandboxed_here(project: &std::path::Path, chat: &Chat) -> bool {
    use purlis_core::sandbox::{Plane, personal, policy::Locks};
    if !crate::sandboxing::claude_is_wrapped_here_for_tests()
        || chat.harness() != Some(Harness::ClaudeCode)
    {
        return false;
    }
    let plane = Plane::read(project);
    !plane.missing()
        && !plane.unreadable()
        && plane.in_force(&Locks::of(project)).is_none()
        && personal::in_force(project)
            .for_persona(chat.persona.as_deref())
            .is_none()
}

/// [`crate::sandboxing::no_claude_sandbox_here_for_tests`] for the project at `root`, whose
/// chats on a profile `chats` start: where that starts them without the sandbox, `chats` take
/// the record of it, as the app's event log does (ADR 0067 §7).
fn no_claude_sandbox_for(chats: &Chats, root: &std::path::Path) {
    crate::sandboxing::no_claude_sandbox_here_for_tests(root);
    if crate::sandboxing::claude_is_wrapped_here_for_tests() {
        chats.when_the_sandbox_is_decided(Box::new(|_| Ok(())));
    }
}

fn chat(program: &str, name: &str, resume: Option<&str>) -> Chat {
    Chat {
        program: program.to_owned(),
        args: vec![],
        cwd: None,
        name: name.to_owned(),
        resume: resume.map(|id| SessionId::new(id).expect("a valid id in a test")),
        active: false,
        profile: None,
        persona: None,
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    }
}

/// How long a chat's program may take to run its first line, which only a broken test
/// waits out. A freshly written stand-in is slow to start on a loaded machine: measured at
/// 1.3–6.5 s from the spawn at load 50–70, and past ten seconds at load 100 (#1138). The
/// spawn itself returned in under 0.2 s, and the script needed only milliseconds once it ran.
const TO_START: std::time::Duration = std::time::Duration::from_secs(60);

/// How long a stand-in that is running may take to say what a test waits for. It counts
/// from the stand-in's own first sign of life, never from the spawn, so how slowly the
/// machine starts a program does not count against it.
const ONCE_RUNNING: std::time::Duration = std::time::Duration::from_secs(10);

/// Everything a session has printed, once `text` is among it — as a READER would see
/// it, not as the terminal encoded it.
///
/// Two things had to be got right here, and the second cost a CI round. **Wait for what
/// you are about to assert**: the stand-in `claude` prints `argv:` as a write of its own
/// and its arguments as later ones, so waiting for `argv:` returned before a single
/// argument had arrived. And **match against the text, not the encoding**: what a view
/// emits is a terminal's output — erase-to-end-of-line, carriage returns, a line wrapped
/// at the pane's width — so a long argv is `--resume` then an escape then the rest, and
/// no substring of the command line is present as contiguous bytes. Both spellings can
/// report a failure that has not happened and miss one that has.
fn until_printed(chats: &Chats, session: u32, text: &str) -> String {
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    let seen = Arc::new(Mutex::new(String::new()));
    let collect = {
        let seen = Arc::clone(&seen);
        move |more: Vec<u8>| lock(&seen).push_str(&String::from_utf8_lossy(&more))
    };
    chats
        .sessions()
        .watch(session, Box::new(collect))
        .expect("the view opens");
    // The terminal's own drawing arrives before the program has run at all, so the
    // program's first word on the screen is the sign it is running.
    let watched = Instant::now();
    let mut running_since = None;
    loop {
        let so_far = lock(&seen).clone();
        let plain = as_a_reader_sees(&so_far);
        if plain.contains(text) {
            return plain;
        }
        if running_since.is_none() && !plain.trim().is_empty() {
            running_since = Some(Instant::now());
        }
        match running_since {
            None => assert!(
                watched.elapsed() < TO_START,
                "the program printed nothing in {TO_START:?} (raw: {so_far:?})"
            ),
            Some(since) => assert!(
                since.elapsed() < ONCE_RUNNING,
                "{text:?} never arrived, only {plain:?} (raw: {so_far:?})"
            ),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Terminal output as the words on the screen: escape sequences dropped, and the breaks
/// a terminal inserts — a wrap, a carriage return — read as the single space that was
/// between the words before it laid them out.
///
/// **Every escape, not only CSI** (charter-app#169). This used to drop an `ESC` and then
/// step over the sequence only when the next byte was `[`; every other escape lost its
/// `ESC` and kept the rest as TEXT. So `ESC 7` — DECSC, save cursor, two bytes — put a
/// literal `7` into what this function claims a reader sees, and a redraw landing between
/// the stand-in's two writes of its argv turned
/// `--resume <id> --name ide.7` into `--resume <id> 7 --name ide.7` and failed a chats
/// test that had nothing wrong with it. Seen once on CI, green on a rerun and 8/8
/// locally on the same commit, which is what an assertion about a race looks like.
///
/// A helper that reports a failure that did not happen is worse than no helper, so this
/// consumes the escapes a terminal actually emits rather than the one byte that was
/// caught: [`eat_escape`] has the shapes and why each is here.
fn as_a_reader_sees(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            eat_escape(&mut chars);
            continue;
        }
        out.push(if c == '\r' || c == '\n' { ' ' } else { c });
    }
    // A wrap becomes one space, and so does a run of them, so a command line reads the
    // way it was written however the pane laid it out.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Step over the rest of one escape sequence, the `ESC` itself already taken.
///
/// The four shapes ECMA-48 gives an escape, because a test helper that knows only one of
/// them is a helper that invents characters (charter-app#169):
///
/// - **CSI** (`ESC [`) — parameter and intermediate bytes, then one final byte. The final
///   byte is `0x40..=0x7e` rather than "a letter or `~`": `ESC [ 2 q` (the cursor shape
///   charter's own engine emits, and the sequence standing next to the `ESC 7` in the
///   failing run) ends on `q`, but `ESC [ 0 c` and the `}`-final forms do not, and a
///   final byte this stopped short of would spill parameters into the text.
/// - **string sequences** — OSC (`ESC ]`, a window title), DCS, SOS, PM, APC — run to a
///   string terminator: `ESC \`, or BEL, which every terminal accepts for OSC and which
///   is what `xterm.js` and this engine emit.
/// - **nF** — an intermediate byte (`0x20..=0x2f`) then a final one: `ESC ( B` puts
///   US-ASCII into G0, which a harness clearing the screen emits, and `ESC # 8` is DECALN.
/// - **everything else is the whole sequence**: `ESC 7`/`ESC 8` (save and restore cursor),
///   `ESC =`/`ESC >` (keypad mode), `ESC M` (reverse index), `ESC c` (full reset). These
///   are the ones that were leaving a character behind.
fn eat_escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.next() {
        Some('[') => {
            for c in chars.by_ref() {
                if matches!(c, '\u{40}'..='\u{7e}') {
                    break;
                }
            }
        }
        Some(']' | 'P' | 'X' | '^' | '_') => {
            let mut closed = None;
            for c in chars.by_ref() {
                if c == '\u{7}' || c == '\u{1b}' {
                    closed = Some(c);
                    break;
                }
            }
            // `ESC \` is the terminator; the `ESC` is consumed above and the `\` here.
            if closed == Some('\u{1b}') {
                chars.next_if_eq(&'\\');
            }
        }
        Some('\u{20}'..='\u{2f}') => {
            for c in chars.by_ref() {
                if !matches!(c, '\u{20}'..='\u{2f}') {
                    break;
                }
            }
        }
        _ => {}
    }
}

/// charter-app#169. `ESC 7` is a saved cursor, and a reader sees nothing of it.
#[test]
fn a_redraw_between_two_writes_adds_no_character_a_reader_could_see() {
    // The bytes CI captured (run 35758178617, job 106849600023, PR #168's head), with the
    // id shortened: a redraw landed between the stand-in's `argv:` and its arguments.
    let raw = "argv: --resume 1111\u{1b}[K\r\n\u{1b}[1;1H\u{1b}7\u{1b}[2 q\u{1b}[1;52H \
                   --name ide.7\r\n";

    assert_eq!(as_a_reader_sees(raw), "argv: --resume 1111 --name ide.7");
}

/// Each shape [`eat_escape`] knows, consumed whole — `a` and `b` stay adjacent.
#[test]
fn every_escape_a_terminal_emits_is_consumed_whole() {
    for (raw, what) in [
        ("a\u{1b}7b", "ESC 7, save cursor"),
        ("a\u{1b}8b", "ESC 8, restore cursor"),
        ("a\u{1b}=b", "ESC =, application keypad"),
        ("a\u{1b}>b", "ESC >, normal keypad"),
        ("a\u{1b}Mb", "ESC M, reverse index"),
        ("a\u{1b}cb", "ESC c, full reset"),
        ("a\u{1b}(Bb", "ESC ( B, US-ASCII into G0"),
        ("a\u{1b}#8b", "ESC # 8, DECALN"),
        ("a\u{1b}]0;a window title\u{7}b", "OSC closed by BEL"),
        ("a\u{1b}]0;a window title\u{1b}\\b", "OSC closed by ST"),
        ("a\u{1b}[1;1Hb", "CSI, cursor home"),
        ("a\u{1b}[?25lb", "CSI with a private parameter"),
        ("a\u{1b}[2 qb", "CSI with an intermediate byte"),
        ("a\u{1b}[0mb", "CSI, reset"),
    ] {
        assert_eq!(as_a_reader_sees(raw), "ab", "{what} left something behind");
    }
}

/// Chats that write every record they make into `wrote`, newest last.
fn recorded() -> (Chats, std::sync::Arc<Mutex<Vec<Record>>>) {
    let wrote = std::sync::Arc::new(Mutex::new(Vec::new()));
    let keep = std::sync::Arc::clone(&wrote);
    let chats = Chats::recorded_by(Box::new(move |record| lock(&keep).push(record.clone())));
    (chats, wrote)
}

fn a_view(key: &str) -> purlis_core::reopen::View {
    purlis_core::reopen::View {
        from: None,
        view: "persona".into(),
        key: key.into(),
        title: key.into(),
        workspace: None,
        at: 0,
        active: false,
        pinned: false,
        split: None,
    }
}

#[test]
fn the_view_tabs_the_window_holds_are_written_into_the_record_beside_the_chats() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.hold_views(vec![a_view("steward")]);

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(
        last.chats.len(),
        1,
        "the chats went missing from the record"
    );
    assert_eq!(last.views, vec![a_view("steward")]);
}

fn a_focus() -> Focus {
    Focus {
        workspace: "alpha".into(),
        repo: "svc".into(),
        piece: Some("fix-login".into()),
    }
}

#[test]
fn the_branch_the_window_focused_is_written_into_the_record_once() {
    // FM-5: the cockpit's branch is remembered with the window's views.
    let (chats, wrote) = recorded();

    chats.hold_focus(Some(a_focus()));
    let so_far = lock(&wrote).len();
    chats.hold_focus(Some(a_focus()));

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.focus, Some(a_focus()));
    assert_eq!(
        lock(&wrote).len(),
        so_far,
        "the same focus was written twice"
    );
    chats.hold_focus(None);
    assert_eq!(lock(&wrote).last().cloned().unwrap().focus, None);
}

#[test]
fn a_record_s_focus_is_held_for_the_window_when_it_is_put_back() {
    let (chats, wrote) = recorded();

    chats.put_back(
        &Record {
            focus: Some(a_focus()),
            ..Default::default()
        },
        SIZE,
    );

    assert_eq!(chats.focus(), Some(a_focus()));
    assert!(lock(&wrote).is_empty(), "putting a record back wrote it");
}

#[test]
fn saying_the_same_view_tabs_again_writes_nothing() {
    // The window says what its view tabs are after every change to its tabs, and most of
    // those changes are to chats.
    let (chats, wrote) = recorded();
    chats.hold_views(vec![a_view("steward")]);
    let so_far = lock(&wrote).len();

    chats.hold_views(vec![a_view("steward")]);

    assert_eq!(lock(&wrote).len(), so_far);
}

#[test]
fn a_record_s_view_tabs_are_held_for_the_window_and_not_written_back_while_it_is_put_back() {
    let (chats, wrote) = recorded();

    chats.put_back(
        &Record {
            views: vec![a_view("steward")],
            ..Default::default()
        },
        SIZE,
    );

    assert_eq!(chats.views(), vec![a_view("steward")]);
    assert!(lock(&wrote).is_empty(), "putting a record back wrote it");
}

/// The names the record lists its chats under, in the order it lists them.
fn names_in(record: &Record) -> Vec<String> {
    record.chats.iter().map(|chat| chat.name.clone()).collect()
}

#[test]
fn the_record_lists_the_chats_in_the_order_the_window_arranged_them() {
    // SI-6: the operator drags a tab, and the strip's order is what comes back at the next
    // launch — not the order the chats happened to be numbered in.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let a = chats.start(&chat(&claude, "a", None), SIZE).unwrap();
    let b = chats.start(&chat(&claude, "b", None), SIZE).unwrap();
    let c = chats.start(&chat(&claude, "c", None), SIZE).unwrap();

    chats.hold_order(vec![c, a, b]);

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(names_in(&last), ["c", "a", "b"]);
    let open: Vec<String> = chats.open_now().into_iter().map(|one| one.name).collect();
    assert_eq!(
        open,
        ["c", "a", "b"],
        "a reloaded window would draw another order"
    );
}

#[test]
fn saying_the_same_chat_order_again_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let a = chats.start(&chat(&claude, "a", None), SIZE).unwrap();
    let b = chats.start(&chat(&claude, "b", None), SIZE).unwrap();
    chats.hold_order(vec![b, a]);
    let so_far = lock(&wrote).len();

    chats.hold_order(vec![b, a]);

    assert_eq!(lock(&wrote).len(), so_far);
}

#[test]
fn a_chat_no_relaunch_puts_back_is_never_written_into_the_record() {
    // #4976: Try it's chat lives for the seconds it takes to report its start. A quit in those
    // seconds must not bring back a chat nobody asked to keep.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    chats.start(&chat(&claude, "kept", None), SIZE).unwrap();
    let trying = Chat {
        unrecorded: true,
        ..chat(&claude, "try", None)
    };
    chats.start(&trying, SIZE).unwrap();

    assert_eq!(names_in(&chats.record()), ["kept"]);
    let written = lock(&wrote);
    assert!(
        written
            .iter()
            .all(|record| !names_in(record).contains(&"try".to_owned())),
        "a record holding it was written"
    );
}

#[test]
fn saying_the_order_the_record_already_has_writes_nothing() {
    // The window says the order after every change to its tabs, opening a chat included,
    // and a chat it has just opened is already last in the record.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let a = chats.start(&chat(&claude, "a", None), SIZE).unwrap();
    let b = chats.start(&chat(&claude, "b", None), SIZE).unwrap();
    let so_far = lock(&wrote).len();

    chats.hold_order(vec![a, b]);

    assert_eq!(lock(&wrote).len(), so_far);
}

#[test]
fn a_chat_the_window_has_not_placed_yet_comes_after_the_ones_it_has() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let a = chats.start(&chat(&claude, "a", None), SIZE).unwrap();
    let b = chats.start(&chat(&claude, "b", None), SIZE).unwrap();
    chats.hold_order(vec![b, a]);

    chats.start(&chat(&claude, "c", None), SIZE).unwrap();

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(names_in(&last), ["b", "a", "c"]);
}

// --- the sandbox (ADR 0067): a chat on no profile goes through the same decision ------- //

#[test]
fn a_sandboxed_opencode_chat_runs_inside_charters_wrap_and_its_proxy_lives_as_long_as_it() {
    // opencode has no sandbox of its own, so the whole harness runs under the profile
    // charter writes, reaching the network through charter's proxy alone (ADR 0067 §2).
    let plane = a_sandboxed_plane();
    let plugin = crate::Shipped::default()
        .plugin
        .expect("the stand-in plugin");
    let shim = purlis_core::opencode::shim_in(&plugin);
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("the bundle");
    std::fs::write(&shim, "export default {}\n").expect("the shim");
    let socket = plane.path().join(".charter/app/hooks.sock");
    let host = Pretend::default();
    host.reporting_on(socket.clone());
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: Some(plugin),
        shims: None,
        git_hooks: None,
    });
    // A profile that names its own proxy and temp directory: the wrap's win, or the chat's
    // traffic and temp files would go where the profile says.
    let ready = purlis_core::start::Ready {
        env: vec![
            ("ZZ_KEPT".to_owned(), "yes".to_owned()),
            ("HTTPS_PROXY".to_owned(), "http://zz.example:1".to_owned()),
            ("NO_PROXY".to_owned(), "*".to_owned()),
            ("TMPDIR".to_owned(), "/zz".to_owned()),
        ],
        ..ready_under(
            Harness::Opencode,
            a_sandbox_for(Harness::Opencode, plane.path()),
        )
    };
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");

    let opening = host.openings().pop().expect("opened");
    assert_eq!(opening.program.as_deref(), Some("/usr/bin/sandbox-exec"));
    assert_eq!(opening.args[0], "-p");
    assert!(
        opening.args[1].contains("(remote unix-socket (path-literal"),
        "{}",
        opening.args[1]
    );
    assert_eq!(opening.args[2..], ["/bin/sh", "-c", "sleep 30"]);
    let named = |wanted: &str| -> Vec<String> {
        opening
            .env
            .iter()
            .filter(|(key, _)| key == wanted)
            .map(|(_, value)| value.clone())
            .collect()
    };
    assert_eq!(named("NO_PROXY"), [""]);
    assert_eq!(named("ZZ_KEPT"), ["yes"]);
    // The word the chat's own processes read for "this chat was given a sandbox" (#1345).
    assert_eq!(named(purlis_core::hookwire::SANDBOXED_ENV), ["1"]);
    let tmp = named("TMPDIR");
    assert!(tmp.len() == 1 && tmp[0] != "/zz", "{tmp:?}");
    let proxy = match named("HTTPS_PROXY").as_slice() {
        [one] => one.clone(),
        many => panic!("pointed at {many:?}"),
    };
    let port: u16 = proxy
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .expect("a port");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());

    chats.close(session).expect("closed");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "the proxy outlived its chat"
    );
}

// --- qw384b: a chat's own kubeconfig for a cluster on this Mac ------------------------ //

#[test]
fn a_chat_pointed_at_a_cluster_on_this_mac_is_owed_a_restart_and_only_it_holds_the_copy() {
    let chats = Chats::new();
    let size = Size {
        columns: 80,
        rows: 24,
    };
    let chat = Chat {
        program: "/bin/sleep".to_owned(),
        args: vec!["5".to_owned()],
        name: "kube".to_owned(),
        ..Default::default()
    };
    let session = chats.start(&chat, size).expect("started");
    let other = chats.start(&chat, size).expect("started");
    chats
        .point_kubectl(
            session,
            "{\"kind\": \"Config\"}".to_owned(),
            "told".to_owned(),
        )
        .expect("pointed");
    assert_eq!(chats.owed_restarts(), [session]);
    assert_eq!(lock(&chats.owed)[&session], ["told"]);
    let id = |n: u32| {
        lock(&chats.open)[&n]
            .chat
            .identity
            .id
            .clone()
            .expect("an id")
    };
    assert_eq!(
        chats.kubeconfig_of(&id(session)).as_deref(),
        Some("{\"kind\": \"Config\"}")
    );
    assert_eq!(chats.kubeconfig_of(&id(other)), None);
    // A chat that is not open is pointed at nothing.
    assert!(
        chats
            .point_kubectl(9999, "{}".to_owned(), "x".to_owned())
            .is_err()
    );
    chats.close(session).expect("closed");
    chats.close(other).expect("closed");
}

/// Chats on a pretend host armed to hand a sandbox to an opencode chat in `plane`, as the
/// app ships it.
fn armed_for_opencode(plane: &std::path::Path) -> (Chats, Pretend) {
    let plugin = crate::Shipped::default()
        .plugin
        .expect("the stand-in plugin");
    let shim = purlis_core::opencode::shim_in(&plugin);
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("the bundle");
    std::fs::write(&shim, "export default {}\n").expect("the shim");
    let host = Pretend::default();
    host.reporting_on(plane.join(".charter/app/hooks.sock"));
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: Some(plugin),
        shims: None,
        git_hooks: None,
    });
    (chats, host)
}

#[test]
fn a_sandboxed_chat_starts_on_a_record_of_its_folder_and_what_appears_after_is_told() {
    use purlis_core::sandbox::arrived;
    let plane = a_sandboxed_plane();
    let (chats, _host) = armed_for_opencode(plane.path());
    let ready = ready_under(
        Harness::Opencode,
        a_sandbox_for(Harness::Opencode, plane.path()),
    );
    std::fs::write(plane.path().join(".envrc"), "there at the start\n").expect("a name");
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };
    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    let id = chats.id_of(session).expect("an id");

    // At once, as a chat's program may, while the start may still be recording (#5555).
    std::fs::write(plane.path().join(".mcp.json"), "{}\n").expect("a name");
    crate::arrived::recorded(&id);
    let looked = arrived::look(plane.path(), &id, None)
        .expect("a look")
        .expect("its record");
    let [told] = looked.new.as_slice() else {
        panic!("{looked:?}");
    };
    assert!(told.ends_with(".mcp.json"), "{told}");
}

#[test]
fn a_sandboxed_chat_pointed_at_a_cluster_starts_with_kubeconfig_naming_its_own_copy() {
    let plane = a_sandboxed_plane();
    let (chats, host) = armed_for_opencode(plane.path());
    let ready = purlis_core::start::Ready {
        // A kubeconfig of the person's own, from the app's environment: the chat's copy
        // wins.
        env: vec![(
            "KUBECONFIG".to_owned(),
            "/Users/someone/.kube/config".to_owned(),
        )],
        ..ready_under(
            Harness::Opencode,
            a_sandbox_for(Harness::Opencode, plane.path()),
        )
    };
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };
    let first = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    let started = lock(&chats.open)[&first].chat.clone();
    let config = "{\"kind\": \"Config\", \"current-context\": \"kind\"}";
    chats
        .point_kubectl(first, config.to_owned(), "told".to_owned())
        .expect("pointed");
    chats.close(first).expect("closed");

    let session = chats
        .start_ready(&started, &ready, SIZE)
        .expect("starts again");

    let opening = host.openings().pop().expect("opened");
    let named: Vec<&String> = opening
        .env
        .iter()
        .filter(|(key, _)| key == "KUBECONFIG")
        .map(|(_, value)| value)
        .collect();
    let [path] = named.as_slice() else {
        panic!("KUBECONFIG {named:?}")
    };
    let path = std::path::PathBuf::from(path);
    assert_ne!(path, std::path::Path::new("/Users/someone/.kube/config"));
    assert!(
        path.ends_with(purlis_core::sandbox::kubeconfig::FILE),
        "{path:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("written"), config);
    // Another chat, never pointed, is started with the person's own, untouched.
    let plain = chats
        .start_ready(&chat, &ready, SIZE)
        .expect("another starts");
    let opening = host.openings().pop().expect("opened");
    assert!(
        opening
            .env
            .iter()
            .any(|(key, value)| key == "KUBECONFIG" && value == "/Users/someone/.kube/config"),
        "{:?}",
        opening.env
    );
    chats.close(plain).expect("closed");
    chats.close(session).expect("closed");
    assert!(!path.exists(), "the copy outlived its chat");
}

#[test]
fn a_port_removed_while_a_chat_runs_is_withdrawn_from_its_proxy_unless_it_still_holds_it() {
    use purlis_core::sandbox::grant::What;
    let plane = a_sandboxed_plane();
    let (chats, _host) = armed_for_opencode(plane.path());
    let ready = ready_under(
        Harness::Opencode,
        a_sandbox_for(Harness::Opencode, plane.path()),
    );
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };
    let held = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    let other = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    chats.hold_grant(held, What::Port(50795), 1).expect("held");
    // Removed for every chat of this project on this machine: the chat that holds it for
    // itself keeps it; the other's proxy carries it no longer.
    assert_eq!(chats.withdraw_port(None, 50795), Ok(vec![other]));
    // Removed for the one chat: withdrawn from it.
    let id = lock(&chats.open)[&held]
        .chat
        .identity
        .id
        .clone()
        .expect("an id");
    assert_eq!(chats.withdraw_port(Some(&id), 50795), Ok(vec![held]));
    chats.close(held).expect("closed");
    chats.close(other).expect("closed");
}

/// #1970: a port added on Settings' Network page for one open chat is held for that chat
/// alone, audited, and owes it the restart its Notice's Allow would.
#[test]
fn a_port_added_in_settings_for_one_chat_is_held_for_it_alone() {
    use purlis_core::sandbox::grant::What;
    let plane = a_sandboxed_plane();
    let (chats, _host) = armed_for_opencode(plane.path());
    let ready = ready_under(
        Harness::Opencode,
        a_sandbox_for(Harness::Opencode, plane.path()),
    );
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };
    let held = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    let other = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    let shell = chats
        .start_ready(&chat, &a_shell_ready(), SIZE)
        .expect("a shell starts");
    let audited = std::sync::Mutex::new(Vec::new());
    let audit = |chat: Option<u32>, made: &purlis_core::sandbox::grant::Audited<'_>| {
        audited
            .lock()
            .unwrap()
            .push((chat, made.what.to_owned(), made.target.to_owned()));
        Ok(())
    };
    let machine = purlis_core::sandbox::Machine::this();
    let form = |chat, port: &str| {
        crate::sandboxing::allow_port(
            plane.path(),
            &machine,
            &chats,
            Some(chat),
            (port, crate::sandboxing::GrantLevel::Chat),
            &audit,
            1,
        )
    };
    // What a chat's own Notice refuses for it, the form refuses in the same words.
    for never in ["0", "1-65535", "*", "localhost:50795", "47823"] {
        let notice = crate::sandboxing::judged_and_kept(
            plane.path(),
            &machine,
            &chats,
            Some(held),
            (
                crate::sandboxing::GrantWhat::Port,
                never,
                crate::sandboxing::GrantLevel::Chat,
            ),
            &audit,
            1,
        )
        .expect_err("the Notice refuses it")
        .to_string();
        assert_eq!(
            form(held, never).expect_err("refused").to_string(),
            notice,
            "{never}"
        );
    }
    // A shell is no chat to allow a port for, and nothing is audited for it.
    assert_eq!(
        form(shell, "50795").expect_err("refused").to_string(),
        format!(
            "purlis did not allow port 50795: chat {shell} is not a chat open in this project."
        )
    );
    assert!(audited.lock().unwrap().is_empty());
    let allowed = form(held, "50795").expect("allowed");
    assert!(
        allowed.said.starts_with("Allowed for this chat."),
        "{}",
        allowed.said
    );
    assert!(chats.holds_for(held, &What::Port(50795)));
    assert!(!chats.holds_for(other, &What::Port(50795)));
    assert!(chats.owed_restarts().contains(&held));
    assert!(!chats.owed_restarts().contains(&other));
    assert_eq!(
        *audited.lock().unwrap(),
        [(Some(held), "port".to_owned(), "50795".to_owned())]
    );
    assert!(purlis_core::sandbox::local::granted_ports(plane.path()).is_empty());
    chats.close(held).expect("closed");
    chats.close(other).expect("closed");
    chats.close(shell).expect("closed");
}

#[test]
fn a_chat_started_without_a_sandbox_is_never_told_it_has_one() {
    // #1345: what a chat says about "this chat's sandbox" is read from this word, so a
    // chat the app started unsandboxed must not carry it, even in a sandboxed project.
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let ready = purlis_core::start::Ready {
        sandbox: None,
        env: vec![(
            purlis_core::hookwire::SANDBOXED_ENV.to_owned(),
            "1".to_owned(),
        )],
        ..ready_under(Harness::ClaudeCode, a_claude_sandbox(plane.path()))
    };
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");

    let opening = host.openings().pop().expect("opened");
    assert!(
        !opening
            .env
            .iter()
            .any(|(key, _)| key == purlis_core::hookwire::SANDBOXED_ENV),
        "{:?}",
        opening.env
    );
    chats.close(session).expect("closed");
}

/// A plane that turned the sandbox on.
fn a_sandboxed_plane() -> tempfile::TempDir {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(
        plane.path().join(purlis_core::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("charter.toml");
    plane
}

/// Chats of the project at `plane`, on the app's own sessions.
fn chats_of(plane: &std::path::Path) -> Chats {
    Chats::on_host(
        Box::new(|_| {}),
        Box::new(Sessions::reporting_to(None)),
        plane.to_path_buf(),
    )
}

/// A chat on no profile whose program is `program`, standing in `plane`.
fn a_chat_in(plane: &std::path::Path, program: &str) -> Chat {
    Chat {
        cwd: Some(plane.to_path_buf()),
        ..chat(program, "off-profile", None)
    }
}

#[test]
fn a_harness_opened_on_no_profile_in_a_sandboxed_plane_is_refused_not_run_unconfined() {
    // `open_session` with a harness as its program: no profile, and still a harness.
    let plane = a_sandboxed_plane();
    let chats = chats_of(plane.path());

    let refused = chats
        .start(&a_chat_in(plane.path(), &standing("opencode")), SIZE)
        .expect_err("not started");

    // Refused for one reason or another on every system — no compiler here, no wrap there,
    // no shipped binary to arm it with — and never started unconfined.
    assert!(
        refused.starts_with("this project runs every chat sandboxed"),
        "{refused}"
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[test]
fn a_recorded_harness_chat_on_no_profile_is_not_put_back_unconfined() {
    let plane = a_sandboxed_plane();
    // A program that would run, so only the sandbox decision can keep it from starting.
    let opencode = stand_in::program(plane.path(), "opencode", "#!/bin/sh\nexec sleep 600\n");
    let chats = chats_of(plane.path());

    let open = chats.put_back(
        &Record {
            chats: vec![a_chat_in(plane.path(), &opencode.display().to_string())],
            ..Default::default()
        },
        SIZE,
    );

    assert!(open.is_empty(), "it was put back");
}

#[cfg(target_os = "macos")]
#[test]
fn a_harness_on_no_profile_whose_program_a_chat_could_write_is_refused() {
    // Ruling V87g: the system temp folders are writable to a chat, and so is the plane.
    let plane = a_sandboxed_plane();
    let elsewhere = tempfile::tempdir().expect("a temp folder");
    let opencode = stand_in::program(elsewhere.path(), "opencode", "#!/bin/sh\nexec sleep 600\n");
    let chats = chats_of(plane.path());

    let refused = chats
        .start(
            &a_chat_in(plane.path(), &opencode.display().to_string()),
            SIZE,
        )
        .expect_err("not started");

    assert!(
        refused.contains("the program lives where this chat can write"),
        "{refused}"
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[cfg(unix)]
#[test]
fn a_harness_on_no_profile_under_a_charter_toml_that_is_not_a_file_is_refused() {
    // The plane's walk takes only a regular file for a plane: a FIFO, or a link, there
    // would otherwise read as no plane at all and start the chat unsandboxed.
    for kind in ["fifo", "dangling"] {
        let plane = tempfile::tempdir().expect("a plane");
        let marker = plane.path().join(purlis_core::plane::MANIFEST);
        if kind == "fifo" {
            let made =
                purlis_core::forklock::status(std::process::Command::new("mkfifo").arg(&marker))
                    .expect("mkfifo runs");
            assert!(made.success());
        } else {
            std::os::unix::fs::symlink("gone", &marker).expect("a link");
        }
        let below = plane.path().join("workspaces/w");
        std::fs::create_dir_all(&below).expect("a workspace");
        let chats = chats_of(plane.path());
        let refused = chats
            .start(
                &Chat {
                    cwd: Some(below),
                    ..chat(&standing("opencode"), "off-profile", None)
                },
                SIZE,
            )
            .expect_err("not started");
        assert!(
            refused.contains("cannot be read as TOML"),
            "{kind}: {refused}"
        );
        assert!(chats.in_order().is_empty(), "{kind}: a chat was opened");
    }
}

#[test]
fn a_shell_in_a_sandboxed_plane_is_still_the_operators_own() {
    let plane = a_sandboxed_plane();
    let chats = chats_of(plane.path());

    let session = chats
        .start(&a_chat_in(plane.path(), "/bin/sh"), SIZE)
        .expect("a shell starts");

    let _ = chats.close(session);
}

/// The sandbox the core compiles for a `harness` chat in `plane`, on a machine that has
/// every backend program — so the answer does not depend on the machine the test runs on.
fn a_sandbox_for(harness: Harness, plane: &std::path::Path) -> purlis_core::sandbox::Applied {
    purlis_core::sandbox::for_start(harness, plane, &a_machine(), &|_| true)
        .expect("compiles")
        .expect("sandboxed")
}

/// A machine that has every backend program, whatever machine the test runs on.
fn a_machine() -> purlis_core::sandbox::Machine {
    purlis_core::sandbox::Machine {
        env: purlis_core::secrets::Env::of(&[]),
        home: None,
        // Where every harness has a sandbox charter compiles.
        os: purlis_core::sandbox::Os::MacOs,
    }
}

// ----- Restart chat, and the chats a sandbox change leaves behind (#1428) -----

/// Chats of the sandboxed project at `plane` on `host`, armed to carry a sandbox, whose
/// chats on no profile are decided as the core decides them, on [`a_machine`] and without
/// the check of the real program: a stand-in harness can then start sandboxed, and start
/// again, which is what these tests are about.
pub(crate) fn restartable_chats_of(plane: &std::path::Path, host: Pretend) -> Chats {
    restartable_chats_on(plane, host, a_machine())
}

/// [`restartable_chats_of`], with chats decided on `machine`.
fn restartable_chats_on(
    plane: &std::path::Path,
    host: Pretend,
    machine: purlis_core::sandbox::Machine,
) -> Chats {
    let root = plane.to_path_buf();
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host), root.clone()).deciding_by(
        Box::new(move |chat, grants, opt_out| {
            use purlis_core::sandbox::Decided;
            // A shell is the person's own, as in the start this stands in for.
            let Some(harness) = chat.harness() else {
                return Ok((None, None, None));
            };
            let decided = purlis_core::sandbox::decide_granted(
                harness,
                &root,
                &machine,
                &|_| true,
                &purlis_core::sandbox::Person {
                    opt_out,
                    level_as: None,
                    made: &purlis_core::sandbox::Made::Window,
                },
                None,
                grants,
            )
            .map_err(|not| not.to_string())?;
            Ok(match decided {
                Some(Decided::Sandboxed(applied)) => {
                    (Some((applied, chat.program.clone())), None, None)
                }
                Some(Decided::Unsandboxed(lifted)) => (None, Some(lifted), None),
                Some(Decided::Floorless(why)) => (None, None, Some(why)),
                None => (None, None, None),
            })
        }),
    );
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    chats
}

/// A Claude Code chat on no profile, with a conversation to resume, standing in `plane`.
pub(crate) fn a_resumable_chat_in(plane: &std::path::Path) -> Chat {
    a_chat_in_with(plane, Some(ID))
}

fn a_chat_in_with(plane: &std::path::Path, resume: Option<&str>) -> Chat {
    Chat {
        cwd: Some(plane.to_path_buf()),
        ..chat(&standing("claude"), "restarted", resume)
    }
}

/// The project at `plane` sets its sandbox to `settings`, the lines under `[sandbox]`.
fn the_sandbox_becomes(plane: &std::path::Path, settings: &str) {
    std::fs::write(
        plane.join(purlis_core::plane::MANIFEST),
        format!("[sandbox]\nmode = \"on\"\n{settings}"),
    )
    .expect("charter.toml");
}

fn older(chats: &Chats) -> Option<OlderSandbox> {
    chats.on_older_sandbox_on(&a_machine(), &|_| true)
}

fn reaches(confines: &purlis_core::sandbox::Confines, host: &str) -> bool {
    confines.hosts.iter().any(|one| one.contains(host))
}

/// R1 (2026-10-13): **a chat starts with the certificate check**, with nothing allowed by
/// anyone, unless the project's file turns it off; and a chat that started while it was
/// off keeps the sandbox it started with, is listed on the Notice after a sandbox setting
/// changed, and has the check once it restarts.
#[test]
fn a_chat_starts_with_the_certificate_check_and_one_that_started_without_is_told_to_restart() {
    let trusts = |chats: &Chats, session: u32| {
        chats
            .confines_of(session)
            .expect("it started sandboxed")
            .widened
            .trust
    };
    // The default project: on from the first start.
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert!(trusts(&chats, session), "on by default");
    assert_eq!(older(&chats), None);
    chats.close(session).expect("ends");

    // A project that turned it off: a chat starts without it. The project turns it back
    // on (as an update does for a chat that started under an older build): the chat is on
    // an older sandbox, and its restart has the check.
    let plane = a_sandboxed_plane();
    the_sandbox_becomes(plane.path(), "certificate-checks = false\n");
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert!(!trusts(&chats, session), "the project turned it off");
    assert_eq!(older(&chats), None);
    the_sandbox_becomes(plane.path(), "");
    assert_eq!(
        older(&chats).expect("told").sessions(),
        [session],
        "it runs without what the project has now"
    );
    chats.ask_restart(session).expect("asked");
    let started = chats.restart(session, SIZE).expect("restarts");
    chats.close(session).expect("the old run ends");
    assert!(trusts(&chats, started));
    assert_eq!(
        older(&chats),
        None,
        "restarted, it is on the sandbox of now"
    );
    chats.close(started).expect("ends");
}

/// #1750: **a chat started before a cluster pool was added is told to restart**, and the
/// Notice says the pool is why. Its sandbox, compiled as it started, holds none of the
/// programs the pool runs; once it restarts it holds them, and nothing is behind.
#[cfg(unix)]
#[test]
fn a_chat_started_before_a_cluster_pool_was_added_is_told_to_restart_for_it() {
    let config = tempfile::tempdir().expect("a config home");
    let plugin = config.path().join("kube-plugin");
    std::fs::write(&plugin, "#!/bin/sh\n").expect("a plugin");
    let machine = purlis_core::sandbox::Machine {
        env: purlis_core::secrets::Env::of(&[(
            "XDG_CONFIG_HOME",
            &config.path().display().to_string(),
        )]),
        ..a_machine()
    };
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_on(plane.path(), Pretend::default(), machine.clone());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let older_on = |chats: &Chats| chats.on_older_sandbox_on(&machine, &|_| true);
    assert_eq!(older_on(&chats), None, "no pool yet");

    // The pool is added while the chat runs.
    let folder =
        purlis_same_user::PrivateDirectory::open(&purlis_core::machine::dir(config.path()))
            .expect("the config folder");
    let file = serde_json::json!({ "version": 1, "runners": [], "pools": [{
        "name": "team", "kind": "cluster", "kubeconfig": "/home/op/.kube/config",
        "context": "team", "pool": "default", "namespace": "ada",
        "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
        "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
        "holds": [plugin.display().to_string()],
    }]});
    folder
        .replace_file(
            purlis_core::machine::RUNNERS_FILE,
            file.to_string().as_bytes(),
        )
        .expect("the pool kept");
    let behind = older_on(&chats).expect("told");
    assert_eq!(
        behind.sessions(),
        [session],
        "it runs without the pool's holds"
    );
    assert_eq!(
        behind.chats[0].pool.as_deref(),
        Some(POOL_ADDED),
        "the pool is why"
    );

    chats.ask_restart(session).expect("asked");
    let started = chats.restart(session, SIZE).expect("restarts");
    chats.close(session).expect("the old run ends");
    assert_eq!(
        older_on(&chats),
        None,
        "restarted, it holds the pool's programs"
    );
    chats.close(started).expect("ends");
}

/// The project at `plane` holds `settings` in its `.claude/settings.json`.
fn project_settings_become(plane: &std::path::Path, settings: &str) {
    std::fs::create_dir_all(plane.join(".claude")).expect(".claude");
    std::fs::write(plane.join(".claude/settings.json"), settings).expect("settings");
}

/// Whether the last chat `host` started was limited to the person's settings and purlis's.
fn limited_to_the_person_s(host: &Pretend) -> bool {
    let opened = host.openings();
    let args = &opened.last().expect("a start").args;
    args.windows(2)
        .any(|pair| pair[0] == "--setting-sources" && pair[1] == "user")
}

/// Chats of a sandboxed project whose own Claude Code settings hold only what a chat keeps,
/// each told chat owed its restart as the project wires it.
fn chats_on_kept_project_settings(
    watch: crate::projectsettings::Watch,
) -> (tempfile::TempDir, Arc<Chats>, Pretend, Arc<Mutex<Vec<u32>>>) {
    let plane = a_sandboxed_plane();
    project_settings_become(
        plane.path(),
        r#"{"permissions": {"deny": ["Bash(rm -rf *)"]}}"#,
    );
    let host = Pretend::default();
    let mut chats = restartable_chats_of(plane.path(), host.clone());
    chats.watching_project_settings_with(watch);
    let chats = Arc::new(chats);
    let told = Arc::new(Mutex::new(Vec::new()));
    chats.tell_project_settings_to({
        let (weak, told) = (Arc::downgrade(&chats), Arc::clone(&told));
        Arc::new(move |session| {
            if let Some(chats) = weak.upgrade() {
                chats.project_settings_changed(session);
            }
            lock(&told).push(session);
        })
    });
    (plane, chats, host, told)
}

#[test]
fn a_chat_whose_project_settings_stop_passing_is_restarted_on_the_person_s_settings() {
    // ADR 0067 as amended 2026-10-16.
    let (plane, chats, host, told) =
        chats_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert!(!limited_to_the_person_s(&host), "its project's files load");
    assert!(chats.owed_restarts().is_empty());

    project_settings_become(plane.path(), r#"{"permissions": {"allow": ["Bash"]}}"#);
    chats.screen_project_settings_again();
    // Told by this screen, or by the watch's own on the change it heard.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while lock(&told).is_empty() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(*lock(&told), [session], "the window is told");
    assert_eq!(chats.owed_restarts(), [session], "owed its restart");

    let started = chats.restart(session, SIZE).expect("restarts");
    chats.close(session).expect("the old run ends");
    assert!(
        limited_to_the_person_s(&host),
        "on the person's settings now"
    );
    let notes = chats.start_notes(started);
    assert!(
        notes.iter().any(|note| note.contains("permissions.allow")),
        "its tab says why: {notes:?}"
    );
    // Started on the person's settings, it is watched no more: nothing is owed again.
    project_settings_become(plane.path(), r#"{"hooks": {"Stop": []}}"#);
    chats.screen_project_settings_again();
    assert_eq!(*lock(&told), [session]);
    chats.close(started).expect("ends");
}

#[test]
fn a_change_caught_as_the_chat_is_first_screened_is_contained_too() {
    // Between its arming and its first screen: the start loaded the files, and they changed.
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, host, told) = chats_on_kept_project_settings(watch.clone());
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    // The start answers the run that goes on: started again on the person's settings, the
    // first run ended, nobody told and nothing owed.
    assert!(limited_to_the_person_s(&host));
    assert_eq!(host.openings().len(), 2, "started once more");
    assert_eq!(chats.open_now().len(), 1);
    assert_eq!(chats.open_now()[0].session, session);
    assert!(lock(&told).is_empty());
    assert!(chats.owed_restarts().is_empty());
    chats.close(session).expect("ends");
}

/// Chats of a sandboxed project whose own Claude Code settings hold only what a chat keeps,
/// saying each run that goes on and each count ([`saying`]), on `watch`.
fn saying_on_kept_project_settings(
    watch: crate::projectsettings::Watch,
) -> (tempfile::TempDir, Chats, Pretend, Said) {
    let plane = a_sandboxed_plane();
    project_settings_become(
        plane.path(),
        r#"{"permissions": {"deny": ["Bash(rm -rf *)"]}}"#,
    );
    let host = Pretend::default();
    let mut chats = restartable_chats_of(plane.path(), host.clone());
    chats.watching_project_settings_with(watch);
    let said = saying(&mut chats);
    (plane, chats, host, said)
}

#[test]
fn a_chat_whose_project_settings_pass_its_start_counts_and_records_its_run_once() {
    let (plane, chats, _host, said) =
        saying_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert_eq!(
        *lock(&said),
        [
            format!("{session} run start"),
            "counted Sandboxed claude -".to_owned(),
        ]
    );
    chats.close(session).expect("ends");
}

#[test]
fn a_chat_started_again_at_once_counts_and_records_one_run_not_two() {
    // #2250: the first run, put aside at its start, is no run of the chat.
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, host, said) = saying_on_kept_project_settings(watch.clone());
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert_eq!(host.openings().len(), 2, "started once more");
    assert_eq!(
        *lock(&said),
        [
            format!("{session} run start"),
            "counted Sandboxed claude -".to_owned(),
        ],
        "one run, under the number that goes on, counted once"
    );
    chats.close(session).expect("ends");
}

/// A chat whose last run was unsandboxed, in `plane`: its start brings the sandbox back on.
fn a_resumable_chat_back_in_its_sandbox(plane: &std::path::Path) -> Chat {
    Chat {
        unsandboxed: true,
        ..a_resumable_chat_in(plane)
    }
}

#[test]
fn a_chat_back_in_its_sandbox_whose_project_settings_pass_records_the_change_once() {
    let (plane, chats, _host, said) =
        saying_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_back_in_its_sandbox(plane.path()), SIZE)
        .expect("starts");
    assert_eq!(
        *lock(&said),
        [
            "trust.sandbox.on claude -".to_owned(),
            format!("{session} run start"),
            "counted Sandboxed claude -".to_owned(),
        ]
    );
    chats.close(session).expect("ends");
}

#[test]
fn a_chat_back_in_its_sandbox_started_again_at_once_records_the_change_once() {
    // #2252: the start put aside writes no trust event; the one in its place writes it.
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, host, said) = saying_on_kept_project_settings(watch.clone());
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_resumable_chat_back_in_its_sandbox(plane.path()), SIZE)
        .expect("starts");
    assert_eq!(host.openings().len(), 2, "started once more");
    assert_eq!(
        *lock(&said),
        [
            "trust.sandbox.on claude -".to_owned(),
            format!("{session} run start"),
            "counted Sandboxed claude -".to_owned(),
        ],
        "one trust event, one run, one count"
    );
    chats.close(session).expect("ends");
}

#[test]
fn a_start_put_aside_at_once_is_told_to_the_log_as_taken_back() {
    // #2252: the log then records its program's later lines with no run.
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, _host, _said) = saying_on_kept_project_settings(watch.clone());
    let settled: std::sync::Arc<Mutex<Vec<(u32, HeldStart)>>> = std::sync::Arc::default();
    let keep = std::sync::Arc::clone(&settled);
    chats.when_a_held_start_is_settled(Box::new(move |session, how| {
        lock(&keep).push((session, how));
    }));
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let settled = lock(&settled).clone();
    assert!(
        matches!(settled.as_slice(), [(first, HeldStart::TakenBack)] if *first != session),
        "the start put aside, and only it, is taken back: {settled:?}"
    );
    chats.close(session).expect("ends");
}

/// The word after `flag` in `args`, where there is one.
fn after(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

#[test]
fn a_new_conversation_started_once_more_at_its_start_is_under_a_new_id() {
    // Claude Code refuses `--session-id` for an id whose transcript exists, and the first
    // run may have begun one: the run that goes on is a new conversation of its own, and the
    // chat's record names it.
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, host, _told) = chats_on_kept_project_settings(watch.clone());
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_chat_in_with(plane.path(), None), SIZE)
        .expect("starts");
    let opened = host.openings();
    assert_eq!(opened.len(), 2, "started once more");
    let first = after(&opened[0].args, "--session-id").expect("a new conversation");
    let second = after(&opened[1].args, "--session-id").expect("a new conversation");
    assert_ne!(first, second, "under a new id");
    assert_eq!(after(&opened[1].args, "--resume"), None);
    let recorded = chats.recorded_chat(session).expect("its record");
    assert_eq!(
        recorded.resume.map(|id| id.as_str().to_owned()),
        Some(second),
        "its record names the conversation that goes on"
    );
    chats.close(session).expect("ends");
}

#[test]
fn a_resumed_conversation_started_once_more_at_its_start_is_resumed_again() {
    let watch = crate::projectsettings::Watch::new();
    let (plane, chats, host, _told) = chats_on_kept_project_settings(watch.clone());
    let root = plane.path().to_path_buf();
    watch.changing_before_the_next_chat(move || {
        project_settings_become(&root, r#"{"permissions": {"allow": ["Bash"]}}"#);
    });
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let opened = host.openings();
    assert_eq!(opened.len(), 2, "started once more");
    assert_eq!(after(&opened[0].args, "--resume").as_deref(), Some(ID));
    assert_eq!(after(&opened[1].args, "--resume").as_deref(), Some(ID));
    assert_eq!(after(&opened[1].args, "--session-id"), None);
    chats.close(session).expect("ends");
}

#[test]
fn a_restart_for_project_settings_ends_the_old_run_before_the_new_one_starts() {
    let (plane, chats, host, _told) =
        chats_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let started = chats
        .restart_for_project_settings(session)
        .expect("started again");
    let running = host.running_when_opened();
    assert!(
        !running.last().expect("the new start").contains(&session),
        "two runs never share its conversation: {running:?}"
    );
    assert_ne!(started, session);
    let _ = chats.close(session);
    chats.close(started).expect("ends");
}

#[test]
fn a_restart_for_project_settings_says_the_old_run_is_replaced_before_it_ends_it() {
    // ADR 0076 §2, Q247: the event log is told the run is superseded while it still runs, so
    // its end is written superseded wherever the log is written, the session host's included,
    // and the close that follows ends nothing more.
    let (plane, chats, host, _told) =
        chats_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let replaced = Arc::new(Mutex::new(Vec::new()));
    chats.when_a_run_is_replaced(Box::new({
        let (replaced, host) = (Arc::clone(&replaced), host.clone());
        move |number| lock(&replaced).push((number, host.openings().len()))
    }));

    let started = chats
        .restart_for_project_settings(session)
        .expect("started again");

    assert_eq!(
        *lock(&replaced),
        [(session, 1)],
        "once, before the new run started"
    );
    let _ = chats.close(session);
    chats.close(started).expect("ends");
}

#[test]
fn a_session_is_renamed_only_where_a_new_conversation_is_started() {
    let mut words: Vec<String> = ["--session-id", "a", "--name", "a"]
        .map(str::to_owned)
        .to_vec();
    assert!(renamed_session(&mut words, "a", "b"));
    assert_eq!(words, ["--session-id", "b", "--name", "a"]);
    let mut resumed: Vec<String> = ["--resume", "a"].map(str::to_owned).to_vec();
    assert!(!renamed_session(&mut resumed, "a", "b"));
    assert_eq!(resumed, ["--resume", "a"]);
}

#[test]
fn a_chat_whose_project_settings_still_pass_is_left_as_it_is() {
    let (plane, chats, host, told) =
        chats_on_kept_project_settings(crate::projectsettings::Watch::new());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    project_settings_become(
        plane.path(),
        r#"{"permissions": {"ask": ["Bash(git push *)"]}, "model": "sonnet"}"#,
    );
    chats.screen_project_settings_again();
    assert!(lock(&told).is_empty());
    assert!(chats.owed_restarts().is_empty());
    assert!(!limited_to_the_person_s(&host));
    chats.close(session).expect("ends");
}

#[test]
fn a_chat_whose_project_settings_cannot_be_watched_starts_on_the_person_s_settings() {
    let (plane, chats, host, _) =
        chats_on_kept_project_settings(crate::projectsettings::Watch::unable());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert!(limited_to_the_person_s(&host), "fail closed");
    let notes = chats.start_notes(session);
    assert!(
        notes
            .iter()
            .any(|note| note == purlis_core::harness::claude::project::UNWATCHED),
        "{notes:?}"
    );
    chats.close(session).expect("ends");
}

#[test]
fn a_restart_keeps_the_conversation_and_compiles_the_sandbox_the_project_has_now() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    let chats = restartable_chats_of(plane.path(), host.clone());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let id = chats.record().chats[0].identity.id.clone();
    // Its record written, as a slow walk finishes it after the start (#5555): the record's folder
    // under `.charter` is part of what a start compiles.
    crate::arrived::recorded(id.as_deref().expect("an id"));
    let before = chats.confines_of(session).expect("it started sandboxed");
    assert!(!reaches(&before, "a.example"), "{before:?}");

    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    assert_eq!(
        chats.confines_of(session),
        Some(before.clone()),
        "a running chat keeps the sandbox it started with"
    );
    // Nothing a chat sends restarts it: a restart is owed, or asked for by the person.
    let unasked = chats.restart(session, SIZE).unwrap_err();
    assert!(unasked.contains("owed no restart"), "{unasked}");

    chats.ask_restart(session).expect("asked");
    assert_eq!(chats.owed_restarts(), [session]);
    let started = chats.restart(session, SIZE).expect("restarts");

    let after = chats.confines_of(started).expect("sandboxed still");
    assert!(reaches(&after, "a.example"), "{after:?}");
    assert_eq!(
        &after,
        a_claude_sandbox(plane.path()).confines(),
        "what a start compiles from the project's settings now"
    );
    assert_ne!(before, after);
    let opening = host.openings().pop().expect("opened");
    assert!(
        opening
            .args
            .windows(2)
            .any(|pair| pair[0] == "--resume" && pair[1] == ID),
        "{:?}",
        opening.args
    );
    chats.close(session).expect("the old run ends");
    let record = chats.record();
    assert_eq!(record.chats.len(), 1);
    assert_eq!(record.chats[0].identity.id, id, "the same chat");
    assert!(chats.owed_restarts().is_empty());
    assert!(
        chats.start_notes(started).is_empty(),
        "a sandboxed chat's restart has nothing to say"
    );
}

/// **Resume on the band of a chat the person ended** (#5019, spec #4750 story 62): End… takes
/// the chat off the board, and its band's Resume starts it again on its conversation, the same
/// chat under its id in a new run, as Restart does for one still open.
#[test]
fn a_chat_the_person_ended_resumes_on_its_conversation_once() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    let chats = restartable_chats_of(plane.path(), host.clone());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let id = chats.record().chats[0].identity.id.clone();
    chats.close(session).expect("End… ends it");
    assert!(chats.record().chats.is_empty(), "it is off the board");

    let started = chats.resume_ended(session, SIZE).expect("resumes");

    assert_ne!(started, session, "a new run, under a new number");
    let opening = host.openings().pop().expect("opened");
    assert!(
        opening
            .args
            .windows(2)
            .any(|pair| pair[0] == "--resume" && pair[1] == ID),
        "{:?}",
        opening.args
    );
    let record = chats.record();
    assert_eq!(record.chats.len(), 1);
    assert_eq!(record.chats[0].identity.id, id, "the same chat");
    // A second press finds nothing left to resume: it is running again.
    let twice = chats.resume_ended(session, SIZE).unwrap_err();
    assert!(twice.contains("nothing to resume"), "{twice}");
    assert_eq!(chats.record().chats.len(), 1, "nothing more started");
}

/// Resume on an ended band is for a chat the person ended and that has a conversation (#5019):
/// one still open is restarted instead, one purlis never ended has nothing to resume, and one
/// with no conversation says so. Each is refused, and nothing starts.
#[test]
fn a_chat_still_open_never_ended_or_with_no_conversation_is_not_resumed_and_says_why() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let open = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");

    let running = chats.resume_ended(open, SIZE).unwrap_err();
    assert!(running.contains("still open"), "{running}");
    let never = chats.resume_ended(open + 100, SIZE).unwrap_err();
    assert!(never.contains("nothing to resume"), "{never}");

    let fresh = chats
        .start(&a_chat_in_with(plane.path(), None), SIZE)
        .expect("starts");
    // A Claude Code chat is given its conversation's id as it starts: as the restart test of a
    // chat with no conversation does, the stand-in is made one that never recorded any.
    lock(&chats.open).get_mut(&fresh).expect("open").chat.resume = None;
    chats.close(fresh).expect("ends");
    let none = chats.resume_ended(fresh, SIZE).unwrap_err();
    assert!(none.contains("no conversation to resume"), "{none}");
    assert_eq!(chats.record().chats.len(), 1, "nothing started");
}

#[test]
fn a_changed_sandbox_leaves_running_chats_on_the_older_one_and_a_rewrite_alone_does_not() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    // Nothing runs: nothing is left behind, whatever changes.
    the_sandbox_becomes(plane.path(), "hosts = [\"early.example\"]\n");
    assert_eq!(older(&chats), None);
    the_sandbox_becomes(plane.path(), "");
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    // A shell is the person's own, and never counted.
    let shell = chats
        .start(&a_chat_in(plane.path(), "/bin/sh"), SIZE)
        .expect("starts");
    assert_eq!(older(&chats), None, "it started on what is there");

    // The file is written, and compiles to the same sandbox.
    the_sandbox_becomes(plane.path(), "# a note\n");
    assert_eq!(older(&chats), None);

    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    let first = older(&chats).expect("the chat is on the older sandbox");
    assert_eq!(first.sessions(), [session]);
    assert_eq!(older(&chats), Some(first.clone()), "one change, told once");

    the_sandbox_becomes(plane.path(), "hosts = [\"b.example\"]\n");
    let second = older(&chats).expect("on the older sandbox still");
    assert_eq!(second.sessions(), [session]);
    assert_ne!(
        second.chats[0].change, first.chats[0].change,
        "another change"
    );
    // The same hosts in another order are the same sandbox, under the same key.
    the_sandbox_becomes(plane.path(), "hosts = [\"b.example\", \"c.example\"]\n");
    let one_way = older(&chats).expect("behind");
    the_sandbox_becomes(plane.path(), "hosts = [\"c.example\", \"b.example\"]\n");
    assert_eq!(older(&chats), Some(one_way));

    chats.ask_restart(session).expect("asked");
    let started = chats.restart(session, SIZE).expect("restarts");
    chats.close(session).expect("the old run ends");
    assert_eq!(older(&chats), None, "it restarted on what is there");
    let _ = chats.close(started);
    let _ = chats.close(shell);
}

/// #2157: **chats started differently are compiled each for itself**: a chat the person
/// allowed a host for is compared under its own grants, never under its neighbour's
/// compile, and only the chat whose sandbox fell behind is listed.
/// #2013: **what a chat is now is what its browser is given**: a port withdrawn while the
/// chat runs is withdrawn from a browser it opens after, as from its own proxy.
#[test]
fn a_port_withdrawn_while_a_chat_runs_is_withdrawn_from_a_browser_it_opens_after() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let decided = purlis_core::sandbox::decide_granted(
        Harness::ClaudeCode,
        plane.path(),
        &a_machine(),
        &|_| true,
        &purlis_core::sandbox::Person {
            opt_out: None,
            level_as: None,
            made: &purlis_core::sandbox::Made::Relaunch,
        },
        None,
        &purlis_core::sandbox::grant::Grants::default(),
    )
    .expect("compiles");
    let Some(purlis_core::sandbox::Decided::Sandboxed(applied)) = decided else {
        panic!("not sandboxed");
    };
    lock(&chats.open).insert(
        1,
        Running {
            chat: a_resumable_chat_in(plane.path()),
            how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
            harness: Some(Harness::ClaudeCode),
            declared: None,
            workspace: None,
            confinement: None,
            confines: Some(applied.confines().clone()),
            started_with: StartedWith::default(),
            composed_by_its_host: false,
            ran: Some(purlis_session_protocol::session::RunSandbox::By(
                "seatbelt".to_owned(),
            )),
            grants_as: None,
            skills: None,
            plane_skills: true,
            skills_read: Default::default(),
            last_prompt: None,
            runs_as: None,
            runners_word: None,
            reports_verified: false,
            clones_at_start: None,
            prompt_key: None,
        },
    );

    let narrowed = Arc::new(Mutex::new(Vec::new()));
    chats.tell_narrowed_to({
        let narrowed = Arc::clone(&narrowed);
        Arc::new(move |session| narrowed.lock().unwrap().push(session))
    });

    chats.withdraw_port(None, 50795).expect("withdrawn");

    let given = purlis_core::browsertools::browser_reach(
        &chats.confines_now(1).expect("sandboxed").decides(),
        false,
    );
    assert_eq!(given.withdrawn, [50795]);
    // Its running browsers are told to end, as they are when an Allow it had is removed.
    assert_eq!(*narrowed.lock().unwrap(), [1]);
    chats.forget_live(
        None,
        &purlis_core::sandbox::grant::host("docs.example.com").expect("a host"),
        purlis_core::sandbox::grant::Level::You,
    );
    assert_eq!(*narrowed.lock().unwrap(), [1, 1]);
}

#[test]
fn chats_started_with_different_grants_are_compiled_apart_and_listed_apart() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let granted = purlis_core::sandbox::grant::Grants {
        hosts: vec![purlis_core::sandbox::hosts::Host::parse("granted.example").expect("a host")],
        ..Default::default()
    };
    let compiled_with = |grants: &purlis_core::sandbox::grant::Grants| {
        let decided = purlis_core::sandbox::decide_granted(
            Harness::ClaudeCode,
            plane.path(),
            &a_machine(),
            &|_| true,
            &purlis_core::sandbox::Person {
                opt_out: None,
                level_as: None,
                made: &purlis_core::sandbox::Made::Relaunch,
            },
            None,
            grants,
        )
        .expect("compiles");
        match decided {
            Some(purlis_core::sandbox::Decided::Sandboxed(applied)) => applied.confines().clone(),
            other => panic!("not sandboxed: {other:?}"),
        }
    };
    // Each chat as its start recorded it, with the grants it started with.
    let open_as = |session: u32, grants: purlis_core::sandbox::grant::Grants| {
        lock(&chats.open).insert(
            session,
            Running {
                chat: a_resumable_chat_in(plane.path()),
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: None,
                confines: Some(compiled_with(&grants)),
                started_with: StartedWith { held: None, grants },
                ran: Some(purlis_session_protocol::session::RunSandbox::By(
                    "seatbelt".to_owned(),
                )),
                composed_by_its_host: false,
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    };
    open_as(1, Default::default());
    open_as(2, granted.clone());
    assert!(reaches(
        &chats.confines_of(2).expect("sandboxed"),
        "granted.example"
    ));
    let asked = std::sync::atomic::AtomicUsize::new(0);
    let counted = |_: &str| {
        asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        true
    };
    assert_eq!(
        chats.on_older_sandbox_on(&a_machine(), &counted),
        None,
        "each compared under its own grants"
    );
    assert_eq!(asked.into_inner(), 2, "two starts, compiled once each");

    // Only the granted chat's recorded sandbox falls behind.
    lock(&chats.open)
        .get_mut(&2)
        .and_then(|running| running.confines.as_mut())
        .expect("sandboxed")
        .hosts
        .push("gone.example".to_owned());
    assert_eq!(older(&chats).expect("one behind").sessions(), [2]);
}

/// #2157: **asking which chats run on an older sandbox compiles each distinct start once**,
/// however many chats share it. The window asks whenever a chat opens or closes, so a
/// compile per chat made opening the fiftieth chat cost fifty compiles, and fifty chats
/// over a thousand. A compile asks whether the sandbox's backend is installed, which is
/// what is counted here.
#[test]
fn asking_about_older_sandboxes_compiles_once_for_chats_that_started_alike() {
    let compiles_for = |many: usize| {
        let plane = a_sandboxed_plane();
        let chats = restartable_chats_of(plane.path(), Pretend::default());
        let sessions: Vec<u32> = (0..many)
            .map(|_| {
                chats
                    .start(&a_resumable_chat_in(plane.path()), SIZE)
                    .expect("starts")
            })
            .collect();
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let older = chats.on_older_sandbox_on(&a_machine(), &|_| {
            asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            true
        });
        assert_eq!(older, None, "every chat started on what is there");
        for session in sessions {
            let _ = chats.close(session);
        }
        asked.into_inner()
    };
    let one = compiles_for(1);
    assert!(one > 0, "a chat's sandbox is compiled to be compared");
    assert_eq!(
        compiles_for(10),
        one,
        "ten chats started alike cost what one does"
    );
}

#[test]
fn a_chat_that_ran_without_the_sandbox_restarts_in_it_and_its_tab_says_so() {
    // ADR 0067 §7: a person's opt-out is for one start, and a restart is another.
    let plane = a_sandboxed_plane();
    let mut chats = restartable_chats_of(plane.path(), Pretend::default());
    let said = saying(&mut chats);
    let opt_out = purlis_core::sandbox::OptOut { reason: None };
    let session = chats
        .start_as_opted(
            &a_resumable_chat_in(plane.path()),
            SIZE,
            false,
            Why::New,
            Some(&opt_out),
        )
        .expect("starts");
    assert!(chats.started_unsandboxed(session));
    assert_eq!(chats.confines_of(session), None);
    // By the person's choice, so no setting leaves it "on an older sandbox".
    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    assert_eq!(older(&chats), None);

    chats.ask_restart(session).expect("asked");
    let started = chats.restart(session, SIZE).expect("restarts");

    assert!(
        !chats.started_unsandboxed(started),
        "the opt-out was not inherited"
    );
    assert!(chats.confines_of(started).is_some());
    assert_eq!(chats.start_notes(started), [SANDBOXED_AGAIN]);
    assert!(
        lock(&said)
            .iter()
            .any(|line| line.starts_with("trust.sandbox.on")),
        "{:?}",
        lock(&said)
    );
}

/// #1431: a policy that requires the sandbox, landing while a chat runs without it, lists
/// that chat for the Notice with Restart, and says the policy and who set it is the cause.
/// A chat behind for the project's own settings says no policy, though one is in force.
#[test]
fn a_policy_that_requires_the_sandbox_lists_a_chat_running_without_it_and_says_so() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let plane = a_sandboxed_plane();
    let mut chats = restartable_chats_of(plane.path(), Pretend::default());
    let _said = saying(&mut chats);
    let opt_out = purlis_core::sandbox::OptOut { reason: None };
    let unconfined = chats
        .start_as_opted(
            &a_resumable_chat_in(plane.path()),
            SIZE,
            false,
            Why::New,
            Some(&opt_out),
        )
        .expect("starts");
    let confined = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert_eq!(older(&chats), None, "no policy, and nothing changed");

    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let required = older(&chats);
    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    let both = older(&chats);
    set_for_this_test(Locks::none());

    let required = required.expect("the chat without the sandbox is behind the policy");
    assert_eq!(required.sessions(), [unconfined]);
    assert_eq!(
        required.chats[0].policy.as_deref(),
        Some(
            "The sandbox is now required by this machine's policy, set by IT in \
                 /etc/purlis/policy.json."
        )
    );
    let both = both.expect("both behind");
    assert_eq!(both.sessions(), [unconfined, confined]);
    assert!(both.chats[0].policy.is_some());
    assert_eq!(
        both.chats[1].policy, None,
        "the project's settings moved it"
    );
    let _ = chats.close(unconfined);
    let _ = chats.close(confined);
}

#[test]
fn a_restart_the_person_asked_for_that_is_refused_is_not_owed_again() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_chat_in_with(plane.path(), None), SIZE)
        .expect("starts");
    // The stand-in never reports a conversation, and none was recorded for it.
    lock(&chats.open)
        .get_mut(&session)
        .expect("open")
        .chat
        .resume = None;

    chats.ask_restart(session).expect("asked");
    let refused = chats.restart(session, SIZE).unwrap_err();

    // Why, and a way out that is true of a harness that never records a conversation
    // too: nothing here says "yet", and nothing was allowed, so nothing "reaches" it.
    assert_eq!(
        refused,
        format!(
            "purlis did not restart chat {session}: it has no conversation to resume. If it \
                 has only just started, send it a message and restart it again. Start fresh on its \
                 menu starts it again without a conversation."
        )
    );
    assert!(
        chats.owed_restarts().is_empty(),
        "the person was told why, and asks again"
    );
    assert!(lock(&chats.restarting).is_empty());
    let closed = chats.ask_restart(session + 100).unwrap_err();
    assert!(closed.contains("is not open"), "{closed}");
}

/// `restarted`, on a thread of its own: its answer, or a panic if it has not answered in
/// ten seconds. It once waited for good on a lock its own thread held (S1).
fn restarted_in_time(
    chats: &std::sync::Arc<Chats>,
    session: u32,
    started: u32,
) -> Result<u32, String> {
    let (answer, answered) = std::sync::mpsc::channel();
    let on_a_thread = std::sync::Arc::clone(chats);
    std::thread::spawn(move || {
        let _ = answer.send(on_a_thread.restarted(session, started, true));
    });
    answered
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("a restart with something queued behind it answers")
}

/// A chat mid-restart, as [`Chats::restart`] leaves it between starting its new run and
/// [`Chats::restarted`]: claimed, what it was owed taken, its new run open.
fn mid_restart(plane: &std::path::Path) -> (std::sync::Arc<Chats>, u32, u32) {
    let chats = std::sync::Arc::new(restartable_chats_of(plane, Pretend::default()));
    let session = chats
        .start(&a_resumable_chat_in(plane), SIZE)
        .expect("starts");
    chats.ask_restart(session).expect("asked");
    chats.claim(session).expect("claimed");
    assert_eq!(lock(&chats.owed).remove(&session), Some(Vec::new()));
    let again = chats.again_on_its_conversation(session).expect("open");
    let started = chats.start(&again, SIZE).expect("its new run starts");
    (chats, session, started)
}

#[test]
fn a_grant_queued_while_a_chat_restarts_is_carried_to_its_new_run() {
    // D-1342-13 said so and it hung instead: on main too, where Allow on a block's Notice
    // while the chat restarted locked every chat's close, grant and owed list for good.
    let plane = a_sandboxed_plane();
    let (chats, session, started) = mid_restart(plane.path());
    let host = purlis_core::sandbox::grant::What::Host(
        purlis_core::sandbox::hosts::Host::parse("a.example").unwrap(),
    );
    chats
        .grant(session, host, 1, "allowed meanwhile".to_owned())
        .expect("granted");

    assert_eq!(restarted_in_time(&chats, session, started), Ok(started));

    assert_eq!(chats.owed_restarts(), [started], "owed to the new run");
    assert_eq!(lock(&chats.owed)[&started], ["allowed meanwhile"]);
    assert!(lock(&chats.restarting).is_empty());
    // And nothing is left locked behind it.
    chats.close(session).expect("the old run ends");
    chats.close(started).expect("closed");
}

#[test]
fn an_ask_while_a_chat_restarts_asks_for_nothing_more_and_is_not_carried() {
    let plane = a_sandboxed_plane();
    let (chats, session, started) = mid_restart(plane.path());

    // The restart under way is the one asked for.
    chats.ask_restart(session).expect("answered");
    assert!(chats.owed_restarts().is_empty(), "nothing queued behind it");
    // And an entry with nothing to tell, however it got there, is not carried over: the
    // new run would be restarted a second time, which nobody asked for.
    lock(&chats.owed).insert(session, Vec::new());

    assert_eq!(restarted_in_time(&chats, session, started), Ok(started));

    assert!(chats.owed_restarts().is_empty());
    chats.close(session).expect("the old run ends");
    // The new run can be asked again once it is the chat.
    chats.ask_restart(started).expect("asked");
    assert_eq!(chats.owed_restarts(), [started]);
    chats.close(started).expect("closed");
}

#[test]
fn a_chat_owed_a_restart_or_restarting_is_not_among_the_chats_a_setting_left_behind() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");

    // A grant for this chat alone is not the project's sandbox changing, owed or not.
    let host = purlis_core::sandbox::grant::What::Host(
        purlis_core::sandbox::hosts::Host::parse("mine.example").unwrap(),
    );
    chats
        .grant(session, host, 1, "allowed".to_owned())
        .expect("granted");
    assert_eq!(chats.owed_restarts(), [session]);
    assert_eq!(older(&chats), None);
    lock(&chats.owed).clear();
    assert_eq!(older(&chats), None, "compiled with what it started with");

    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    assert_eq!(older(&chats).expect("behind").sessions(), [session]);
    // Owed a restart already: it gets the sandbox the project has now without being asked.
    chats.ask_restart(session).expect("asked");
    assert_eq!(older(&chats), None);
    lock(&chats.owed).clear();
    // Restarting: the same.
    chats.claim(session).expect("claimed");
    assert_eq!(older(&chats), None);
    chats.unclaim(session);
    assert_eq!(older(&chats).expect("behind again").sessions(), [session]);
    chats.close(session).expect("closed");
}

#[test]
fn only_what_settings_decide_leaves_a_chat_behind_and_each_chat_has_its_own_key() {
    // D-1428-10.
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let other = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let recorded = |session: u32, change: &dyn Fn(&mut purlis_core::sandbox::Confines)| {
        change(
            lock(&chats.open)
                .get_mut(&session)
                .expect("open")
                .confines
                .as_mut()
                .expect("sandboxed"),
        );
    };

    // What a start finds by walking the project's tree is denied as it is found: a start
    // now that denies other paths (a clone gained a hooks folder, say) is no change.
    recorded(session, &|confines| confines.denied.clear());
    assert_eq!(older(&chats), None);

    // A folder every chat could write when this one started, revoked since in Settings:
    // the chat goes on writing there until it restarts.
    recorded(session, &|confines| {
        confines.writable.push("/opt/tools/cache".into());
    });
    let behind = older(&chats).expect("it keeps the folder until it restarts");
    assert_eq!(behind.sessions(), [session]);
    let key = behind.chats[0].change.clone();
    assert_eq!(key.len(), 16, "{key}");
    assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()), "{key}");
    // The same on every build of purlis: SHA-256 over one canonical text, not a `Debug`
    // text under the standard hasher. A start with no sandbox has a key of its own.
    assert_eq!(Settled::key(None), "fa38af3a7ad96915");

    // Another chat falls behind for a reason of its own, and each keeps its own key: one
    // restarting does not change what stands for the other.
    recorded(other, &|confines| {
        confines.hosts.push("gone.example".to_owned())
    });
    let both = older(&chats).expect("both behind");
    assert_eq!(both.sessions(), [session, other]);
    assert_eq!(both.chats[0].change, key);
    chats.close(other).expect("closed");
    assert_eq!(older(&chats).expect("one behind").chats[0].change, key);
    chats.close(session).expect("closed");
}

#[test]
fn a_chat_on_a_profile_is_compared_under_the_persona_grants_it_started_with() {
    // S10: every chat from the picker is on a profile, and takes a persona's grants.
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let devops_reaches = |hosts: &str| {
        the_sandbox_becomes(
            plane.path(),
            &format!("\n[sandbox.personas.devops]\nhosts = [{hosts}]\n"),
        );
    };
    devops_reaches("\"ops.example\"");
    // The person here allowed devops's hosts as they were first committed (D-1362-7).
    let allow_devops = || {
        purlis_core::sandbox::local::allow_persona_hosts(
            plane.path(),
            "devops",
            &purlis_core::sandbox::persona::digest(
                &[purlis_core::sandbox::hosts::Host::parse("ops.example").expect("a host")],
                false,
            ),
        )
        .expect("kept");
    };
    allow_devops();
    let compiled_as = |persona: Option<&str>| {
        let decided = purlis_core::sandbox::decide_granted(
            Harness::ClaudeCode,
            plane.path(),
            &a_machine(),
            &|_| true,
            &purlis_core::sandbox::Person {
                opt_out: None,
                level_as: persona,
                made: &purlis_core::sandbox::Made::Window,
            },
            persona,
            &purlis_core::sandbox::grant::Grants::default(),
        )
        .expect("compiles");
        match decided {
            Some(purlis_core::sandbox::Decided::Sandboxed(applied)) => applied.confines().clone(),
            other => panic!("not sandboxed: {other:?}"),
        }
    };
    // A chat on a profile, as its start left it: no stand-in can go through the real
    // start's program check, so it is recorded here as that start records it.
    let open_as = |session: u32, held: Option<purlis_core::reopen::HeldGrants>| {
        let chat = Chat {
            profile: Some("claude".to_owned()),
            persona: Some("devops".to_owned()),
            held: held.clone(),
            ..a_resumable_chat_in(plane.path())
        };
        let persona = purlis_core::start::runs_with(&chat, plane.path());
        lock(&chats.open).insert(
            session,
            Running {
                chat,
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: None,
                confines: Some(compiled_as(persona.as_deref())),
                started_with: StartedWith {
                    held,
                    grants: purlis_core::sandbox::grant::Grants::default(),
                },
                ran: Some(purlis_session_protocol::session::RunSandbox::By(
                    "seatbelt".to_owned(),
                )),
                composed_by_its_host: false,
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    };
    open_as(7, None);
    assert!(reaches(
        &chats.confines_of(7).expect("sandboxed"),
        "ops.example"
    ));
    assert_eq!(
        older(&chats),
        None,
        "its persona's hosts are compiled in now, as its start compiled them"
    );

    devops_reaches("\"ops.example\", \"more.example\"");
    assert_eq!(older(&chats).expect("behind").sessions(), [7]);
    // Back to the list that was allowed: the Allow was seen to change, so it grants nothing
    // until it is allowed anew (D-1362-13), and the chat is still behind until then.
    devops_reaches("\"ops.example\"");
    assert_eq!(older(&chats).expect("behind").sessions(), [7]);
    allow_devops();
    assert_eq!(older(&chats), None);

    // A handed-off chat holds the asking chat's persona grants (here, no persona's)
    // until the person allows its own on its tab. That Allow has its own Notice and its
    // own restart, and is not the project's sandbox changing.
    open_as(8, Some(purlis_core::reopen::HeldGrants { persona: None }));
    assert!(!reaches(
        &chats.confines_of(8).expect("sandboxed"),
        "ops.example"
    ));
    assert_eq!(older(&chats), None);
    assert!(chats.allow_own_grants(8));
    assert_eq!(older(&chats), None);
    // A setting that changes what it was started holding still leaves it behind.
    the_sandbox_becomes(plane.path(), "hosts = [\"a.example\"]\n");
    assert_eq!(older(&chats).expect("both behind").sessions(), [7, 8]);
    lock(&chats.open).clear();
}

fn a_claude_sandbox(plane: &std::path::Path) -> purlis_core::sandbox::Applied {
    a_sandbox_for(Harness::ClaudeCode, plane)
}

fn ready_under(
    harness: Harness,
    sandbox: purlis_core::sandbox::Applied,
) -> purlis_core::start::Ready {
    purlis_core::start::Ready {
        program: "/bin/sh".to_owned(),
        command: vec!["-c".to_owned(), "sleep 30".to_owned()],
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
        harness: Some(harness),
        declared: None,
        session: None,
        how: purlis_core::reopen::Reopened::Fresh(Fresh::NoConversationRecorded),
        plugins: purlis_core::harness_plugin::Chosen::new(),
        sandbox: Some(sandbox),
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: None,
        first_message: None,
    }
}

#[test]
fn a_sandbox_the_app_is_not_armed_to_hand_over_refuses_the_chat() {
    // No plugin shipped: Claude Code is armed with nothing, so the sandbox would be
    // dropped. The chat is refused instead.
    let plane = a_sandboxed_plane();
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: None,
        shims: None,
        git_hooks: None,
    });
    let ready = ready_under(Harness::ClaudeCode, a_claude_sandbox(plane.path()));

    let refused = chats
        .start_ready(&chat("/bin/sh", "c", None), &ready, SIZE)
        .expect_err("not started");

    assert!(refused.contains("purlis's plugin"), "{refused}");
}

#[test]
fn a_sandbox_compiled_for_one_harness_never_reaches_another() {
    let plane = a_sandboxed_plane();
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    let ready = ready_under(Harness::Codex, a_claude_sandbox(plane.path()));

    let refused = chats
        .start_ready(&chat("/bin/sh", "c", None), &ready, SIZE)
        .expect_err("not started");

    assert!(refused.contains("cannot hand the sandbox"), "{refused}");
    assert!(refused.starts_with(purlis_core::sandbox::LEAD), "{refused}");
}

/// #1431: where an administrator's policy requires the sandbox, the refusal says the policy
/// is why, not the project, and who set it.
#[test]
fn a_refusal_where_policy_requires_the_sandbox_leads_with_the_policy() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let plane = a_sandboxed_plane();
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    let ready = ready_under(Harness::Codex, a_claude_sandbox(plane.path()));
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let refused = chats.start_ready(&chat("/bin/sh", "c", None), &ready, SIZE);
    set_for_this_test(Locks::none());

    let refused = refused.expect_err("not started");
    assert!(
        refused.starts_with(purlis_core::sandbox::POLICY_LEAD),
        "{refused}"
    );
    assert!(refused.contains("cannot hand the sandbox"), "{refused}");
    assert!(
        refused.ends_with("Locked by policy, set by IT in /etc/purlis/policy.json."),
        "{refused}"
    );
}

#[test]
fn a_codex_chat_on_no_profile_in_a_sandboxed_plane_is_never_started_unsandboxed() {
    // #1123: charter wraps Codex where it can (macOS); a program that is not there, or a
    // system charter cannot wrap it on, refuses the chat rather than starting it without.
    let plane = a_sandboxed_plane();
    let chats = chats_of(plane.path());

    let refused = chats
        .start(&a_chat_in(plane.path(), &standing("codex")), SIZE)
        .expect_err("not started");

    assert!(!refused.contains("#1123"), "{refused}");
    assert!(refused.contains("started"), "{refused}");
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

/// Chats of the project at `plane`, on a host that runs nothing: a start that is not
/// refused opens, whatever program it names.
fn pretend_chats_of(plane: &std::path::Path) -> Chats {
    Chats::on_host(
        Box::new(|_| {}),
        Box::new(Pretend::default()),
        plane.to_path_buf(),
    )
}

#[cfg(unix)]
#[test]
fn a_harness_on_no_profile_whose_folder_was_swapped_for_a_link_is_refused() {
    // #1410: `workspaces` replaced by a link to a folder outside the project. Walking up
    // from the chat's folder then finds no project, and the chat used to start unsandboxed.
    let plane = a_sandboxed_plane();
    let elsewhere = tempfile::tempdir().expect("a folder outside the project");
    std::fs::create_dir_all(elsewhere.path().join("w")).expect("a folder");
    std::os::unix::fs::symlink(elsewhere.path(), plane.path().join("workspaces")).expect("a link");
    let chats = pretend_chats_of(plane.path());

    let refused = chats
        .start(
            &a_chat_in(&plane.path().join("workspaces/w"), &standing("opencode")),
            SIZE,
        )
        .expect_err("not started");

    assert_eq!(
        refused,
        purlis_core::sandbox::FolderRefusal::Linked
            .said(&purlis_core::sandbox::policy::Locks::none())
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[test]
fn a_harness_on_no_profile_in_a_folder_with_no_project_is_refused() {
    // #1410: no project above the chat's folder is no reason to drop the open project's
    // sandbox.
    let plane = a_sandboxed_plane();
    let elsewhere = tempfile::tempdir().expect("a folder in no project");
    let chats = pretend_chats_of(plane.path());

    let refused = chats
        .start(&a_chat_in(elsewhere.path(), &standing("opencode")), SIZE)
        .expect_err("not started");
    assert_eq!(
        refused,
        purlis_core::sandbox::FolderRefusal::Outside
            .said(&purlis_core::sandbox::policy::Locks::none())
    );

    let refused = chats
        .start(&chat(&standing("opencode"), "no folder", None), SIZE)
        .expect_err("not started");
    assert_eq!(
        refused,
        purlis_core::sandbox::FolderRefusal::Missing
            .said(&purlis_core::sandbox::policy::Locks::none())
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[test]
fn a_manifest_planted_below_the_project_does_not_change_a_chats_project() {
    // #1410: a manifest written in a folder below the root (outside `workspaces/`, which
    // is not re-rooted) would be found as the project of a chat started there.
    let plane = a_sandboxed_plane();
    let planted = plane.path().join("notes");
    std::fs::create_dir_all(&planted).expect("a folder");
    std::fs::write(planted.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    let chats = pretend_chats_of(plane.path());

    let refused = chats
        .start(&a_chat_in(&planted, &standing("opencode")), SIZE)
        .expect_err("not started");

    assert!(
        refused.starts_with("this project runs every chat sandboxed"),
        "{refused}"
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[test]
fn a_harness_on_no_profile_is_told_the_open_project_and_a_shell_is_not() {
    // Its hooks and guards act for the project the chat was started in, as a chat on a
    // profile's do, and never for one they would find above its folder.
    let project = no_project();
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), project.clone());
    let elsewhere = tempfile::tempdir().expect("a folder");
    let root_of = |opening: &crate::host::Opening| {
        opening
            .env
            .iter()
            .filter(|(key, _)| key == "PURLIS_ROOT")
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>()
    };

    let agent = chats
        .start(&a_chat_in(elsewhere.path(), &standing("claude")), SIZE)
        .expect("starts");
    let opening = host.openings().pop().expect("opened");
    assert_eq!(root_of(&opening), [project.display().to_string()]);

    let shell = chats
        .start(&a_chat_in(elsewhere.path(), "/bin/sh"), SIZE)
        .expect("starts");
    let opening = host.openings().pop().expect("opened");
    assert_eq!(root_of(&opening), Vec::<String>::new());
    let _ = chats.close(agent);
    let _ = chats.close(shell);
}

#[test]
fn a_harness_on_no_profile_in_a_project_whose_manifest_has_gone_is_refused() {
    // D-1410e: the open project cannot say whether it runs chats sandboxed.
    let gone = tempfile::tempdir().expect("a project whose manifest has gone");
    let chats = pretend_chats_of(gone.path());

    let refused = chats
        .start(&a_chat_in(gone.path(), &standing("claude")), SIZE)
        .expect_err("not started");

    assert_eq!(
        refused,
        purlis_core::sandbox::NotStarted::PlaneMissing.to_string()
    );
    assert!(chats.in_order().is_empty(), "a chat was opened");
}

#[test]
fn a_new_chat_and_a_chat_put_back_say_why_where_what_to_keep_from_them_cannot_be_told() {
    // ADR 0067 as amended: every start goes through the one decision, so a chat the app
    // puts back as it opens the project, and a retry of it, are told what a new chat is.
    let plane = a_sandboxed_plane();
    std::fs::write(plane.path().join("vaults.json"), "{ not json").expect("a registry");
    let chats = pretend_chats_of(plane.path());
    let says_why = |refused: &str| {
        refused.contains("vaults.json could not be read, so purlis cannot tell what to keep")
            && refused.contains("nothing was started")
            && refused.contains("Doctor says what is wrong")
    };

    let refused = chats
        .start(&a_chat_in(plane.path(), &standing("claude")), SIZE)
        .expect_err("not started");
    assert!(says_why(&refused), "{refused}");
    assert!(chats.in_order().is_empty(), "a chat was opened");

    let record = Record {
        chats: vec![a_resumable_chat_in(plane.path())],
        ..one_recorded(&standing("claude"))
    };
    chats.put_back_here(&record, SIZE);
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert!(says_why(&waiting[0].why), "{}", waiting[0].why);
    let again = chats
        .retry(&waiting[0].id, SIZE)
        .expect_err("still not started");
    assert!(says_why(&again), "{again}");
    assert!(chats.open_now().is_empty(), "a chat was opened");
}

#[cfg(unix)]
#[test]
fn a_system_with_no_backend_checks_no_folder_for_a_sandbox_it_never_applies() {
    // Windows starts every chat unsandboxed, and says why: a folder check there would
    // refuse for a sandbox that was never going to be applied.
    let plane = a_sandboxed_plane();
    let elsewhere = tempfile::tempdir().expect("a folder outside the project");
    std::os::unix::fs::symlink(elsewhere.path(), plane.path().join("workspaces")).expect("a link");
    let on = |os| purlis_core::sandbox::Machine {
        env: purlis_core::secrets::Env::of(&[]),
        home: None,
        os,
    };
    let linked = plane.path().join("workspaces");

    let windows = project_sandbox(
        Harness::ClaudeCode,
        plane.path(),
        Some(&linked),
        &on(purlis_core::sandbox::Os::Windows),
        &|_| true,
        &purlis_core::sandbox::Person {
            opt_out: None,
            level_as: None,
            made: &purlis_core::sandbox::Made::Window,
        },
        &Default::default(),
    );
    assert!(
        matches!(
            windows,
            Ok(Some(purlis_core::sandbox::Decided::Unsandboxed(
                purlis_core::sandbox::Lifted {
                    by: purlis_core::sandbox::By::NoBackend(_),
                    ..
                }
            )))
        ),
        "{windows:?}"
    );
    let mac = project_sandbox(
        Harness::ClaudeCode,
        plane.path(),
        Some(&linked),
        &on(purlis_core::sandbox::Os::MacOs),
        &|_| true,
        &purlis_core::sandbox::Person {
            opt_out: None,
            level_as: None,
            made: &purlis_core::sandbox::Made::Window,
        },
        &Default::default(),
    );
    assert_eq!(
        mac.map(|_| ()),
        Err(purlis_core::sandbox::FolderRefusal::Linked
            .said(&purlis_core::sandbox::policy::Locks::none()))
    );
}

#[cfg(unix)]
#[test]
fn a_persons_opt_out_still_starts_without_the_sandbox_in_a_linked_folder() {
    // "Start without the sandbox" is a person's choice for one chat, never a fallback: the
    // folder check guards a sandbox, and this chat was asked for without one.
    let plane = a_sandboxed_plane();
    let elsewhere = tempfile::tempdir().expect("a folder outside the project");
    std::os::unix::fs::symlink(elsewhere.path(), plane.path().join("workspaces")).expect("a link");
    let mut chats = pretend_chats_of(plane.path());
    // An opt-out is recorded before it runs (ADR 0067 §7).
    let said = saying(&mut chats);
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(a_person_lifted_it()),
        ..a_shell_ready()
    };
    let chat = Chat {
        cwd: Some(plane.path().join("workspaces")),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");

    assert!(
        lock(&said)
            .iter()
            .any(|line| line.starts_with("trust.sandbox.off")),
        "{:?}",
        lock(&said)
    );
    assert!(chats.record().chats[0].unsandboxed);
    let _ = chats.close(session);
}

#[test]
fn a_record_is_put_back_in_its_own_order_and_not_by_chat_number() {
    // The record lists the chats in the order the strip drew them, and a chat keeps its
    // number across a launch (charter-app#90) — so number order is not strip order.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            chats: vec![
                Chat {
                    number: Some(5),
                    ..chat(&claude, "dragged first", None)
                },
                Chat {
                    number: Some(2),
                    ..chat(&claude, "opened first", None)
                },
            ],
            ..Default::default()
        },
        SIZE,
    );

    let names: Vec<&str> = open.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["dragged first", "opened first"]);
    assert_eq!(names_in(&chats.record()), ["dragged first", "opened first"]);
}

#[test]
fn opening_a_chat_writes_the_record_without_waiting_for_a_quit() {
    // An app that is killed, or crashes, runs no exit handler. Everything open would be
    // lost if the record were only written on the way out.
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();

    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats.len(), 1);
    assert_eq!(last.chats[0].name, "ide.7");
}

#[test]
fn closing_a_chat_writes_the_record_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let going = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.close(going).expect("it closes");

    assert_eq!(lock(&wrote).last().expect("a record").chats, vec![]);
}

#[test]
fn bringing_another_chat_to_the_front_writes_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let front = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();

    chats.bring_to_front(Some(front));

    let last = lock(&wrote).last().cloned().expect("a record");
    let active: Vec<&str> = last
        .chats
        .iter()
        .filter(|c| c.active)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(active, vec!["ide.8"]);
}

#[test]
fn bringing_the_same_chat_to_the_front_again_writes_nothing() {
    // Every click on the tab already in front would otherwise be a write.
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let only = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    chats.bring_to_front(Some(only));
    let so_far = lock(&wrote).len();

    chats.bring_to_front(Some(only));

    assert_eq!(lock(&wrote).len(), so_far);
}

#[test]
fn putting_a_record_back_writes_it_once_when_every_chat_is_back() {
    // Fifty chats coming back must not be fifty writes at the one moment cold start is
    // measured. One write, though, there has to be: each chat came back in a run begun at
    // this launch, and a chat recorded before ids with one minted for it, which an app
    // that crashed before its next write would mint again (#856 review F1).
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();

    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: (0..5)
                .map(|n| chat(&claude, &format!("ide.{n}"), None))
                .collect(),
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let written = lock(&wrote).clone();
    assert_eq!(written.len(), 1, "{written:#?}");
    assert_eq!(written[0].chats.len(), 5, "every chat, in the one write");
    assert!(
        written[0]
            .chats
            .iter()
            .all(|one| one.identity.id.is_some() && one.identity.run.is_some()),
        "{written:#?}"
    );
}

#[test]
fn a_chat_that_was_started_is_one_the_quit_would_record() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();

    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .expect("the chat starts");

    let record = chats.record();
    assert_eq!(record.chats.len(), 1);
    assert_eq!(record.chats[0].name, "ide.7");
    assert_eq!(chats.open_now()[0].session, session);
    assert_eq!(chats.open_now()[0].harness, Some(Harness::ClaudeCode));
}

/// #967: a chat on a harness the project declares is opened with its declaration's `env`
/// namespace for the session to compose (#1013), and its pane's Shift+Enter is the
/// declaration's `[terminal] newline`; one that names none keeps the terminal's own Enter.
#[test]
fn a_declared_harnesss_chat_is_opened_with_its_namespace_and_types_its_newline() {
    let parse = |text: &str| {
        purlis_core::harness_declaration::parse(
            text,
            purlis_core::harness_declaration::Origin::Project,
            "harnesses/shelly.toml",
        )
        .expect("a declaration")
    };
    let shelly = parse(
        "name = \"shelly\"\nprogram = \"shelly\"\nenv = [\"SHELLY_*\"]\n[terminal]\n\
             newline = \"\\u001b\\r\"\n",
    );
    let plain = parse("name = \"shelly\"\nprogram = \"shelly\"\n");
    let dir = tempfile::tempdir().unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        dir.path().to_path_buf(),
    );
    let on = |declared: &purlis_core::harness_declaration::Declaration, name: &str| {
        chats
            .start_ready(
                &chat("/bin/sh", name, None),
                &purlis_core::start::Ready {
                    declared: Some(declared.clone()),
                    ..a_shell_ready()
                },
                SIZE,
            )
            .expect("the chat starts")
    };

    let declared = on(&shelly, "shelly 1");
    let without = on(&plain, "shelly 2");
    let shell = chats
        .start_ready(&chat("/bin/sh", "a shell", None), &a_shell_ready(), SIZE)
        .unwrap();

    let openings = host.openings();
    assert_eq!(openings[0].declared_env, ["SHELLY_*"]);
    assert_eq!(
        openings[0].harness, None,
        "a declared harness has no adapter"
    );
    assert!(openings[1].declared_env.is_empty());
    assert!(openings[2].declared_env.is_empty());
    assert_eq!(chats.newline(declared).as_deref(), Some("\x1b\r"));
    assert_eq!(chats.newline(without), None);
    assert_eq!(chats.newline(shell), None);
}

/// #968, #1134: a chat started on this machine's replacement of a built-in's facts keeps
/// its harness, so its arming is the built-in's, and takes the replacement's newline;
/// what is open carries the declaration its header's card is made from.
#[test]
fn a_replaced_built_ins_chat_keeps_its_harness_and_takes_the_replacements_facts() {
    let replaced = purlis_core::harness_declaration::read_with(
        std::path::Path::new("/nowhere/at/all"),
        Ok(Some(
            "[harnesses.claude]\nprogram = \"claude\"\n\
                 [harnesses.claude.terminal]\nnewline = \"\\u001b[13;2u\"\n"
                .to_owned(),
        )),
    );
    let replacement = replaced
        .replacement(Harness::ClaudeCode)
        .expect("a replacement")
        .clone();
    let dir = tempfile::tempdir().unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        dir.path().to_path_buf(),
    );

    let session = chats
        .start_ready(
            &chat("/bin/sh", "claude 1", None),
            &purlis_core::start::Ready {
                harness: Some(Harness::ClaudeCode),
                declared: Some(replacement.clone()),
                ..a_shell_ready()
            },
            SIZE,
        )
        .expect("the chat starts");

    assert_eq!(chats.harness(session), Some(Harness::ClaudeCode));
    assert_eq!(host.openings()[0].harness, Some(Harness::ClaudeCode));
    assert_eq!(host.openings()[0].declared_env, ["CLAUDE_*"]);
    assert_eq!(chats.newline(session).as_deref(), Some("\x1b[13;2u"));
    assert_eq!(chats.open_now()[0].declared, Some(replacement));
}

#[test]
fn a_chats_harness_is_answered_by_its_session_number_and_a_shells_is_none() {
    // What a pane asks as it opens its view (SI-4): Shift+Enter is the harness's newline,
    // and a shell keeps the terminal's own Enter.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let claude = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .expect("the chat starts");
    let shell = chats
        .start(&chat("/bin/sh", "a shell", None), SIZE)
        .expect("the shell starts");

    assert_eq!(chats.harness(claude), Some(Harness::ClaudeCode));
    assert_eq!(
        chats.newline(claude).as_deref(),
        Some(Harness::ClaudeCode.newline())
    );
    assert_eq!(chats.harness(shell), None);
    assert_eq!(
        chats.harness(claude + shell + 1),
        None,
        "a chat that is not open"
    );
}

#[test]
fn the_record_holds_the_conversation_id_the_app_chose_for_a_new_claude_chat() {
    // The whole point of the record: a chat started fresh today is resumable tomorrow.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    let recorded = chats.record().chats[0].resume.clone();

    assert!(
        recorded.is_some(),
        "the chat was recorded with no conversation"
    );
}

#[test]
fn the_id_in_the_record_is_the_one_the_harness_was_actually_given() {
    // Recording an id the harness never saw would give a resume that always failed.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    // The id charter chose is known before the harness has finished printing it, so the
    // wait is for that exact id rather than for the line it will appear on.
    let recorded = chats.record().chats[0]
        .resume
        .clone()
        .expect("the chat has a conversation");
    let printed = until_printed(&chats, session, recorded.as_str());

    assert!(
        printed.contains(recorded.as_str()),
        "the record says {recorded}, but claude was given {printed:?}"
    );
}

#[test]
fn a_chat_that_closed_is_not_in_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let going = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    chats
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();

    chats.close(going).expect("it closes");

    let names: Vec<String> = chats
        .record()
        .chats
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names, vec!["ide.8"]);
}

#[test]
fn the_chat_in_front_is_the_one_the_record_marks_active() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let front = chats
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();

    chats.bring_to_front(Some(front));

    let active: Vec<String> = chats
        .record()
        .chats
        .iter()
        .filter(|c| c.active)
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(active, vec!["ide.8"]);
}

#[test]
fn a_chat_that_closed_costs_nothing_to_remember() {
    // An app left running all day closes chats all day. Each one that stayed remembered
    // would be a little more memory that never comes back — invisible without this.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let going = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.close(going).expect("it closes");

    assert_eq!(chats.remembered(), 0);
}

#[test]
fn the_chat_that_was_in_front_comes_back_in_front() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();
    let was_in_front = Chat {
        active: true,
        profile: None,
        persona: None,
        ..chat(&claude, "ide.8", None)
    };

    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat(&claude, "ide.7", None), was_in_front],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let active: Vec<String> = chats
        .record()
        .chats
        .iter()
        .filter(|c| c.active)
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(active, vec!["ide.8"]);
}

#[test]
fn a_record_is_put_back_as_one_session_for_each_chat_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![
                chat(&claude, "ide.7", Some(ID)),
                chat(&claude, "ide.8", None),
            ],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(open.len(), 2);
    assert_eq!(chats.sessions().running().len(), 2);
    assert_eq!(open[0].name, "ide.7");
    assert_eq!(open[1].name, "ide.8");
}

#[test]
fn a_chat_put_back_with_a_conversation_is_resumed_by_it() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat(&a_claude(dir.path()), "ide.7", Some(ID))],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(open[0].how, Reopened::Resumed(SessionId::new(ID).unwrap()));
    let want = format!("--resume {ID} --name ide.7");
    let printed = until_printed(&chats, open[0].session, &want);
    assert!(
        printed.contains(&want),
        "claude was not asked to resume: {printed:?}"
    );
}

#[test]
fn the_chat_that_was_in_front_is_the_one_the_window_is_told_to_show() {
    // The record holds which chat was in front; without this the window would put every
    // chat back and then show whichever one it happened to draw last.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![
                chat(&claude, "ide.7", None),
                Chat {
                    active: true,
                    profile: None,
                    persona: None,
                    ..chat(&claude, "ide.8", None)
                },
            ],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let in_front: Vec<&str> = open
        .iter()
        .filter(|one| one.in_front)
        .map(|one| one.name.as_str())
        .collect();
    assert_eq!(in_front, vec!["ide.8"]);
}

#[test]
fn a_chat_put_back_with_no_conversation_says_so_and_starts_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat(&a_claude(dir.path()), "ide.7", None)],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(open[0].how, Reopened::Fresh(Fresh::NoConversationRecorded));
}

#[test]
fn the_record_names_the_process_each_chat_runs_as_now_and_not_the_one_it_was_put_back_with() {
    // V82 (#1018): `commit-msg` stamps a commit only below this process, so a pid carried
    // over from the last launch would name a process that is gone.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![Chat {
                pid: Some(4_000_000),
                ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
            }],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let running = chats.sessions.process_id(open[0].session);
    assert!(running.is_some());
    assert_eq!(chats.record().chats[0].pid, running);
}

#[test]
fn the_last_record_is_written_after_every_other_and_names_no_pid() {
    // R2-1: a write already on its way when the quit begins must not land after the quit's
    // own and put live pids back on disk.
    let dir = tempfile::tempdir().unwrap();
    let written: std::sync::Arc<Mutex<Vec<Record>>> = std::sync::Arc::default();
    let chats = Chats::recorded_by(Box::new({
        let written = std::sync::Arc::clone(&written);
        move |record| lock(&written).push(record.clone())
    }));
    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .expect("the chat starts");
    lock(&written).clear();

    std::thread::scope(|scope| {
        chats.write_last(|record| {
            // A program's end heard, on its own thread, while the quit's record is being
            // written: it must not land after it.
            scope.spawn(|| chats.a_program_ended());
            std::thread::sleep(std::time::Duration::from_millis(50));
            lock(&written).push(record.clone());
        });
    });
    chats.a_program_ended();
    chats.end_all();

    let written = lock(&written);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(
        written[0].chats.len(),
        1,
        "the chat is kept for the next launch"
    );
    assert_eq!(written[0].chats[0].pid, None);
}

#[test]
fn a_claude_chat_reopened_after_its_workspace_was_renamed_starts_fresh_and_says_so_once() {
    // charter#367, D10: Claude Code keeps the conversation under the old folder, so the
    // rename dropped it from the record. The reopen starts a new conversation instead of a
    // `--resume` that would fail, says why, and records the chat as an ordinary one.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("plane");
    std::fs::create_dir_all(root.join("workspaces/beta")).unwrap();
    let mut record = Record {
        views: Vec::new(),
        chats: vec![Chat {
            cwd: Some(root.join("workspaces/alpha")),
            ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
        }],
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        layout: None,
        focus: None,
    };
    // What `charter workspace rename alpha beta` does to the record.
    assert!(purlis_core::wscmd::rename::Move::in_plane(&root, "alpha", "beta").record(&mut record));
    let chats = Chats::new();

    let open = chats.put_back_here(&record, SIZE);

    assert_eq!(open[0].how, Reopened::Fresh(Fresh::WorkspaceRenamed));
    let printed = until_printed(&chats, open[0].session, "--session-id");
    assert!(!printed.contains("--resume"), "{printed:?}");
    let recorded = &chats.record().chats[0];
    assert_eq!(
        recorded.renamed_from, None,
        "it would say so again next time"
    );
    assert!(
        recorded.resume.is_some(),
        "the new conversation is not recorded"
    );
    assert_ne!(recorded.resume, Some(SessionId::new(ID).unwrap()));
}

#[test]
fn a_chat_that_could_not_be_started_stays_in_the_record_for_the_next_launch() {
    // Otherwise a workspace directory that is moved, or a harness that is being
    // reinstalled, silently deletes the chat: it fails to start once, the record is
    // written without it, and by the launch after that there is no trace it existed.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();

    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![
                chat("/definitely/not/a/program", "ide.7", Some(ID)),
                chat(&claude, "ide.8", None),
            ],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let names: Vec<String> = chats
        .record()
        .chats
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names, vec!["ide.7", "ide.8"]);
    let written: Vec<Vec<String>> = lock(&wrote)
        .iter()
        .map(|record| record.chats.iter().map(|c| c.name.clone()).collect())
        .collect();
    assert_eq!(
        written,
        vec![vec!["ide.7".to_owned(), "ide.8".to_owned()]],
        "the one write a put-back makes keeps the chat that did not start"
    );
}

#[test]
fn a_record_cannot_ask_a_launch_to_start_an_unbounded_number_of_programs() {
    // At a bound of four, not the app's two hundred: each start opens a terminal, and two
    // test binaries at two hundred each ran macOS out of them (511), failing every other
    // test that opened a chat (#1139). The bound itself is the next test's.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new().starting_at_most(4);

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: (0..4 + 3)
                .map(|n| chat(&claude, &format!("ide.{n}"), None))
                .collect(),
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(open.len(), 4);
    // And the ones it would not start are kept, not thrown away, saying why.
    let kept = chats.would_not_start();
    assert_eq!(kept.len(), 3);
    assert_eq!(kept[0].why, "more than 4 chats were recorded");
    chats.end_all();
}

#[test]
fn the_app_starts_at_most_two_hundred_chats_from_a_record() {
    // The product's scale is fifty; the backstop sits far above it.
    assert_eq!(Chats::new().most_at_once, 200);
}

#[test]
fn a_chat_that_could_not_be_started_is_named_with_the_reason() {
    // The operator is told, rather than finding a tab quietly missing. Nothing here
    // starts, so there is no stand-in harness to put anywhere.
    let (chats, _) = recorded();

    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat("/definitely/not/a/program", "ide.7", Some(ID))],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let trouble = chats.would_not_start();
    assert_eq!(trouble.len(), 1);
    assert_eq!(trouble[0].name, "ide.7");
    assert!(!trouble[0].why.is_empty(), "no reason was kept");
}

#[test]
fn a_chat_that_could_not_be_started_is_still_recorded_after_a_later_change() {
    // The record is written again as soon as anything changes; the chat that could not
    // start has to survive that write too, not just the launch.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat("/definitely/not/a/program", "ide.7", Some(ID))],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    chats.start(&chat(&claude, "ide.9", None), SIZE).unwrap();

    let last = lock(&wrote).last().cloned().expect("a record was written");
    let names: Vec<String> = last.chats.iter().map(|c| c.name.clone()).collect();
    assert_eq!(names, vec!["ide.7", "ide.9"]);
}

/// A record holding one chat, `ide.7`, that runs `program` and resumes [`ID`].
fn one_recorded(program: &str) -> Record {
    Record {
        views: Vec::new(),
        chats: vec![chat(program, "ide.7", Some(ID))],
        dealt: 0,
        relaunch_after_update: false,
        clone_seat: None,
        layout: None,
        focus: None,
    }
}

/// The id of the one chat a record put back that is waiting to start.
fn waiting_id(chats: &Chats) -> String {
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 1, "one chat waiting");
    assert!(!waiting[0].id.is_empty(), "a waiting chat has an id");
    waiting[0].id.clone()
}

/// How many times each record written since `from` holds the chat with id `id`.
fn times_recorded(wrote: &Mutex<Vec<Record>>, from: usize, id: &str) -> Vec<usize> {
    lock(wrote)[from..]
        .iter()
        .map(|record| {
            record
                .chats
                .iter()
                .filter(|one| one.identity.id.as_deref() == Some(id))
                .count()
        })
        .collect()
}

#[test]
fn a_chat_that_did_not_start_is_started_by_retry_once_what_it_needs_is_back() {
    // NO-3: Retry now, after the harness was reinstalled. The chat is the recorded one,
    // under its id — and every record written on the way holds it exactly once: never
    // twice (waiting and running), never not at all.
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("claude").display().to_string();
    let (chats, wrote) = recorded();
    chats.put_back_here(&one_recorded(&program), SIZE);
    let id = waiting_id(&chats);
    let from = lock(&wrote).len();

    assert_eq!(a_claude(dir.path()), program);
    let session = chats.retry(&id, SIZE).expect("it starts now");

    assert!(chats.would_not_start().is_empty());
    let open: Vec<(u32, String)> = chats
        .open_now()
        .into_iter()
        .map(|one| (one.session, one.name))
        .collect();
    assert_eq!(open, vec![(session, "ide.7".to_owned())]);
    let times = times_recorded(&wrote, from, &id);
    assert!(!times.is_empty(), "the retry wrote the record");
    assert!(
        times.iter().all(|&n| n == 1),
        "recorded once each time: {times:?}"
    );
    chats.end_all();
}

#[test]
fn a_retry_that_fails_again_keeps_the_chat_and_says_why() {
    let (chats, wrote) = recorded();
    chats.put_back_here(&one_recorded("/definitely/not/a/program"), SIZE);
    let id = waiting_id(&chats);

    let refused = chats
        .retry(&id, SIZE)
        .expect_err("the program is still not there");

    assert_eq!(
        chats.would_not_start(),
        vec![NotStarted {
            id: id.clone(),
            name: "ide.7".to_owned(),
            why: refused,
            // A shell is on no profile, so it has nothing to approve.
            approval: None,
        }]
    );
    let last = lock(&wrote).len() - 1;
    assert_eq!(
        times_recorded(&wrote, last, &id),
        vec![1],
        "still recorded, once"
    );
}

/// A plane at `dir/plane` declaring the profile `work` (kind `claude`) in its local file,
/// running a stand-in that waits, with `extra` after its program in its command. Nothing is
/// approved. Answers the plane's root. The stand-in is beside the plane, not in it: the
/// floor starts no program the chat could have written (#2071).
fn a_plane_with_work(dir: &std::path::Path, extra: &str) -> std::path::PathBuf {
    let root = dir.join("plane");
    std::fs::create_dir_all(&root).expect("the plane");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    let program = a_claude(dir);
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!("[harness.work]\nkind = \"claude\"\ncommand = [{program:?}{extra}]\n"),
    )
    .expect("the profile");
    root
}

/// A record holding one chat, `ide.7`, on the profile `work`, standing in `root`.
fn one_on_work(root: &std::path::Path) -> Record {
    Record {
        chats: vec![Chat {
            profile: Some("work".to_owned()),
            cwd: Some(root.to_path_buf()),
            ..chat("claude", "ide.7", None)
        }],
        ..one_recorded("claude")
    }
}

/// The profile `work` as the launch reads it at `root`.
fn work_at(root: &std::path::Path) -> purlis_core::profiles::Profile {
    purlis_core::profiles::for_launch(root)
        .0
        .get("work")
        .expect("work is declared")
        .clone()
}

#[test]
fn a_chat_waiting_on_a_profile_nobody_approved_says_which_line_needs_approving() {
    // #1246 (D-1246-5): the waiting chat's record says WHICH profile and WHICH exact line
    // need approval, so the window offers Review and approve… without reading the reason.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);

    let open = chats.put_back(&one_on_work(&root), SIZE);

    assert!(open.is_empty(), "an unapproved profile started");
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 1);
    assert_eq!(
        waiting[0].approval,
        Some(NeedsApproval {
            profile: "work".to_owned(),
            kind: "claude".to_owned(),
            source: purlis_core::profiles::Source::Local.as_str().to_owned(),
            approval: "new".to_owned(),
            // The very line the picker shows and `approve_profile` checks a click against.
            shown: purlis_core::profiletrust::shown(&root, &work_at(&root)),
        })
    );
    chats.end_all();
}

#[test]
fn a_chat_waiting_on_a_profile_whose_command_changed_says_it_changed() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");
    purlis_core::profiletrust::record_launched(
        &root,
        "work",
        &purlis_core::profiletrust::fingerprint(&work_at(&root)),
    )
    .expect("approved once");
    a_plane_with_work(dir.path(), ", \"--then-something-else\"");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);

    chats.put_back(&one_on_work(&root), SIZE);

    let waiting = chats.would_not_start();
    let asked = waiting[0].approval.clone().expect("it asks again");
    assert_eq!(asked.approval, "changed");
    assert!(asked.shown.contains("--then-something-else"), "{asked:?}");
    chats.end_all();
}

#[test]
fn a_waiting_chats_approval_shows_a_command_longer_than_the_display_limit_whole() {
    // #1014: Review and approve… asks with this line, so its last word reaches the
    // question however long the command is.
    let dir = tempfile::tempdir().expect("a directory");
    let filler = "x".repeat(purlis_core::shown::DISPLAY_LIMIT);
    let root = a_plane_with_work(dir.path(), &format!(", \"{filler}\", \"the-last-word\""));
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);

    chats.put_back(&one_on_work(&root), SIZE);

    let waiting = chats.would_not_start();
    let asked = waiting[0].approval.clone().expect("it asks");
    assert!(
        asked
            .shown
            .ends_with(&format!("{filler} the-last-word (kind claude)")),
        "{asked:?}"
    );
    assert!(!asked.shown.contains("..."), "clipped: {asked:?}");
    chats.end_all();
}

#[test]
fn a_waiting_chat_whose_profile_is_approved_then_starts_on_retry_and_asks_nothing() {
    // The approval is recorded by `approve` against the line shown, and Retry now runs the
    // whole start again: nothing about the approval starts a chat on its own.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);
    no_claude_sandbox_for(&chats, &root);
    chats.put_back(&one_on_work(&root), SIZE);
    let waiting = chats.would_not_start();
    let asked = waiting[0].approval.clone().expect("it asks");

    purlis_core::profiletrust::approve(&root, &work_at(&root), &asked.shown)
        .expect("the line shown is the line on disk");
    assert!(chats.open_now().is_empty(), "approving started the chat");
    chats.retry(&waiting[0].id, SIZE).expect("it starts now");

    assert!(chats.would_not_start().is_empty());
    chats.end_all();
}

/// [`one_on_work`], with its one chat dispatched by another as persona `devops`.
fn one_dispatched_on_work(root: &std::path::Path) -> Record {
    let record = one_on_work(root);
    Record {
        chats: vec![Chat {
            persona: Some("devops".to_owned()),
            from: Some(purlis_core::reopen::HandedFrom {
                id: None,
                chat: 1,
                name: "steward 1".to_owned(),
                workspace: purlis_core::active::Place::Workspace("ide".to_owned()),
                report: purlis_core::reopen::Owed::Due,
                mode: purlis_core::reopen::Mode::Task,
                depth: 1,
                root: None,
                above: None,
                by_person: false,
                long: false,
            }),
            ..record.chats[0].clone()
        }],
        ..record
    }
}

#[test]
fn a_dispatched_chat_is_not_put_back_on_a_profile_that_now_asks_nobody() {
    // #1509: the profile's command is looked up again at every start. Where it has come
    // to switch the prompts off since the dispatch, nobody chose that for this chat.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), ", \"--dangerously-skip-permissions\"");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);

    let open = chats.put_back(&one_dispatched_on_work(&root), SIZE);

    assert!(open.is_empty(), "a dispatched chat started asking nobody");
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 1);
    assert!(
        waiting[0].why.starts_with(
            "This chat was dispatched by another chat, and its profile 'work' now starts \
                 its harness with the permission prompts off (--dangerously-skip-permissions)"
        ),
        "{}",
        waiting[0].why
    );
    // It says what the person can do.
    assert!(
        waiting[0].why.contains("Settings › Harness"),
        "{}",
        waiting[0].why
    );

    // The person's own chat on that profile is not held to this: it waits on the
    // approval any new command does, and nothing else.
    let (own, _) = recorded();
    let own = own.in_project(&root);
    own.put_back(&one_on_work(&root), SIZE);
    let waiting = own.would_not_start();
    assert_eq!(waiting.len(), 1);
    assert!(!waiting[0].why.contains("dispatched"), "{}", waiting[0].why);
    assert!(waiting[0].approval.is_some());
}

#[test]
fn a_dispatched_chat_is_not_put_back_on_a_profile_the_project_stopped_listing() {
    // #1509: the project's list is read again too. A chat dispatched on `work` before the
    // project listed only another profile for its persona does not come back on `work`.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[dispatch.profiles]\ndevops = [\"other\"]\n",
    )
    .expect("the manifest");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);

    let open = chats.put_back(&one_dispatched_on_work(&root), SIZE);

    assert!(
        open.is_empty(),
        "a dispatched chat started on an unlisted profile"
    );
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 1);
    assert_eq!(
        waiting[0].why,
        "This chat was dispatched as persona 'devops' on profile 'work', and the project \
             now lists only 'other' for that persona, so it was not started again. List 'work' \
             for devops under [dispatch.profiles] in the project's file and start it again, or \
             close it and dispatch the work again."
    );

    // Listed again, the same record is no longer refused for the list: it gets as far as
    // the approval a profile nobody approved waits on.
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[dispatch.profiles]\ndevops = [\"other\", \"work\"]\n",
    )
    .expect("the manifest");
    let (listed, _) = recorded();
    let listed = listed.in_project(&root);
    listed.put_back(&one_dispatched_on_work(&root), SIZE);
    let waiting = listed.would_not_start();
    assert_eq!(waiting.len(), 1);
    assert!(!waiting[0].why.contains("dispatched"), "{}", waiting[0].why);
}

#[test]
fn a_retry_refused_for_approval_after_another_refusal_says_so_now() {
    // The approval is read again at every refused start, so a chat that first failed for
    // another reason offers it once a retry is refused for it.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");
    let (chats, _) = recorded();
    let chats = chats.in_project(&root);
    let record = one_on_work(&root);
    chats.put_back(
        &Record {
            chats: vec![Chat {
                profile: Some("not-declared-yet".to_owned()),
                ..record.chats[0].clone()
            }],
            ..record
        },
        SIZE,
    );
    let waiting = chats.would_not_start();
    assert_eq!(waiting[0].approval, None, "no such profile to approve");
    let local = root.join(purlis_core::profiles::LOCAL_FILE);

    std::fs::write(
        &local,
        std::fs::read_to_string(&local)
            .unwrap()
            .replace("[harness.work]", "[harness.not-declared-yet]"),
    )
    .unwrap();

    chats.retry(&waiting[0].id, SIZE).expect_err("not approved");

    let asked = chats.would_not_start()[0].approval.clone();
    assert_eq!(
        asked.map(|a| a.profile),
        Some("not-declared-yet".to_owned())
    );
    chats.end_all();
}

#[test]
fn forget_drops_a_chat_that_did_not_start_from_the_record() {
    let (chats, wrote) = recorded();
    chats.put_back_here(&one_recorded("/definitely/not/a/program"), SIZE);

    chats
        .forget(&waiting_id(&chats))
        .expect("it is there to forget");

    assert!(chats.would_not_start().is_empty());
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert!(last.chats.is_empty(), "the record no longer holds it");
}

#[test]
fn of_two_waiting_chats_with_one_name_forget_drops_only_the_one_named_by_its_id() {
    // A split's chat takes its tab's name, and tab numbers start again at every launch:
    // two waiting chats can be called the same. Forget names one by its id (NO-3 review).
    let (chats, wrote) = recorded();
    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![
                chat("/definitely/not/a/program", "3", None),
                chat("/definitely/not/a/program/either", "3", None),
            ],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );
    let waiting = chats.would_not_start();
    assert_eq!(waiting.len(), 2);
    assert_ne!(waiting[0].id, waiting[1].id, "each has its own id");

    chats.forget(&waiting[1].id).expect("the second is there");

    assert_eq!(chats.would_not_start(), vec![waiting[0].clone()]);
    let last = lock(&wrote).last().cloned().expect("a record was written");
    let ids: Vec<Option<String>> = last.chats.iter().map(|c| c.identity.id.clone()).collect();
    assert_eq!(
        ids,
        vec![Some(waiting[0].id.clone())],
        "the first is still recorded"
    );
}

#[test]
fn a_record_holding_one_id_twice_puts_back_two_chats_with_their_own_ids_and_keeps_both() {
    // A record hand-edited, corrupt, or left by an older bug can hold two chats under one
    // id. The record writes one chat per id (the newest open), which is right only for a
    // retry or a fresh start under way — so the put-back gives the repeat an id of its own,
    // as it does a chat that had none, and neither drops out at the next write.
    // A chat's id as a record holds one: a ULID (ADR 0066; `reopen` reads any other as none).
    // A Claude Code chat's settings on macOS go through a file named by it (SK-9), which takes
    // nothing else.
    const SHARED: &str = "01J9CHAT000000000000000000";
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let shared = |name: &str| {
        let one = chat(&claude, name, None);
        Chat {
            identity: purlis_core::reopen::Identity {
                id: Some(SHARED.to_owned()),
                ..one.identity.clone()
            },
            ..one
        }
    };
    let (chats, wrote) = recorded();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![shared("ide.7"), shared("ide.8")],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(
        open.len(),
        2,
        "both start: {:?}",
        chats
            .would_not_start()
            .iter()
            .map(|one| (one.name.clone(), one.why.clone()))
            .collect::<Vec<_>>()
    );
    let last = lock(&wrote).last().cloned().expect("a record was written");
    let ids: Vec<Option<String>> = last.chats.iter().map(|c| c.identity.id.clone()).collect();
    assert_eq!(ids.len(), 2, "both are recorded: {ids:?}");
    assert_eq!(ids[0].as_deref(), Some(SHARED), "the first keeps the id");
    assert!(
        ids[1].is_some() && ids[1] != ids[0],
        "the repeat has its own: {ids:?}"
    );
    chats.end_all();
}

#[test]
fn a_chat_that_is_not_waiting_to_start_cannot_be_retried_or_forgotten() {
    let (chats, _) = recorded();
    assert!(chats.forget(ID).is_err());
    assert!(chats.retry(ID, SIZE).is_err());
}

#[test]
fn start_fresh_is_the_same_chat_in_a_new_run_recorded_once_while_both_run() {
    // NO-3: the plane-updated mark's Start fresh. The chat keeps its id and its name. The
    // old one stays open until the new one has started, and ending it is the caller's
    // (`Held::start_chat_fresh`) — in between, the record writes the newer, once.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let open = chats.put_back_here(&one_recorded(&claude), SIZE);
    let was = open[0].session;
    let id = chats.record().chats[0].identity.id.clone().unwrap();

    let session = chats.start_fresh(was, SIZE).expect("it starts again");
    assert_ne!(session, was);
    let both = chats.record();
    assert_eq!(both.chats.len(), 1, "one chat, while two programs run");
    assert_eq!(both.chats[0].number, Some(session), "as the newer");
    chats.close(was).unwrap();

    let open: Vec<(u32, String)> = chats
        .open_now()
        .into_iter()
        .map(|one| (one.session, one.name))
        .collect();
    assert_eq!(open, vec![(session, "ide.7".to_owned())]);
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats.len(), 1);
    assert_eq!(last.chats[0].identity.id.as_deref(), Some(id.as_str()));
    assert_ne!(
        last.chats[0].resume,
        Some(SessionId::new(ID).unwrap()),
        "a new conversation, not the one it was resuming"
    );
    chats.end_all();
}

#[test]
fn a_reopened_task_keeps_its_mark_through_every_rebuild_of_its_chat() {
    // #1543: a finished task the person reopened is held to its profile at every later
    // start (ADR 0090 item 16), by the mark its record keeps. Every road that builds the
    // chat again keeps it: a restart on its conversation, Start fresh, a start in the
    // place of one that lost its conversation, and the record a relaunch reads.
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    let mark = purlis_core::reopen::ReopenedTask { by_person: true };
    // A harness with a conversation to resume, which a restart starts again on.
    let reopened = Chat {
        reopened: Some(mark),
        ..a_resumable_chat_in(&no_project())
    };
    let marked =
        |chats: &Chats, session: u32| chats.recorded_chat(session).and_then(|chat| chat.reopened);
    let recorded = |chats: &Chats| {
        chats
            .record()
            .chats
            .iter()
            .map(|chat| chat.reopened)
            .collect::<Vec<_>>()
    };
    let first = chats.start(&reopened, SIZE).expect("it starts");
    assert_eq!(marked(&chats, first), Some(mark));

    chats.ask_restart(first).expect("asked");
    let restarted = chats.restart(first, SIZE).expect("it restarts");
    chats.close(first).expect("the old run ends");
    assert_eq!(marked(&chats, restarted), Some(mark), "a restart");

    let fresh = chats.start_fresh(restarted, SIZE).expect("it starts fresh");
    chats.close(restarted).expect("the old run ends");
    assert_eq!(marked(&chats, fresh), Some(mark), "Start fresh");

    // The window closes the tab before it asks for the start in its place, and names a
    // chat with no mark: the mark is the record's, not the window's.
    chats.close(fresh).expect("its tab closes");
    let again = chats
        .start_ready_instead_of(
            fresh,
            &chat("/bin/sh", "ide.7", None),
            &a_shell_ready(),
            SIZE,
        )
        .expect("it starts in its own place");
    assert_eq!(marked(&chats, again), Some(mark), "a start in its place");
    assert_eq!(recorded(&chats), [Some(mark)], "what a relaunch reads");
    let _ = chats.close(again);
}

#[test]
fn a_refused_fresh_start_keeps_the_old_chat_open_and_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, _) = recorded();
    let open = chats.put_back_here(&one_recorded(&claude), SIZE);
    let was = open[0].session;
    // The program goes while the chat runs: a start of it now cannot happen.
    std::fs::remove_file(&claude).unwrap();

    chats
        .start_fresh(was, SIZE)
        .expect_err("its program is gone");

    let open: Vec<u32> = chats
        .open_now()
        .into_iter()
        .map(|one| one.session)
        .collect();
    assert_eq!(open, vec![was], "still open");
    let names: Vec<String> = chats
        .record()
        .chats
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(names, vec!["ide.7"], "still recorded");
    chats.end_all();
}

#[test]
fn a_chat_started_fresh_keeps_its_place_in_the_record_s_order() {
    // #1246: the window keeps the tab where it was, so the record must too. The new
    // session is not one the window has placed yet, and without this it went last.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (chats, wrote) = recorded();
    let a = chats.start(&chat(&claude, "a", None), SIZE).unwrap();
    let b = chats.start(&chat(&claude, "b", None), SIZE).unwrap();
    chats.hold_order(vec![a, b]);

    let again = chats.start_fresh(a, SIZE).expect("it starts again");
    chats.close(a).unwrap();

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(names_in(&last), ["a", "b"], "the next launch's strip");
    let open: Vec<u32> = chats
        .open_now()
        .into_iter()
        .map(|one| one.session)
        .collect();
    assert_eq!(open, vec![again, b], "a reloaded window's strip");
    chats.end_all();
}

#[test]
fn a_chat_that_is_not_open_cannot_be_started_fresh() {
    let (chats, _) = recorded();
    assert!(chats.start_fresh(42, SIZE).is_err());
}

#[test]
fn a_chat_whose_program_has_gone_is_left_out_and_the_others_still_come_back() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();

    let open = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![
                chat("/definitely/not/a/program", "ide.7", Some(ID)),
                chat(&claude, "ide.8", None),
            ],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(open.len(), 1);
    assert_eq!(open[0].name, "ide.8");
}

// --- a chat keeps its number across a relaunch (charter-app#90) --------------------- //

/// A plane with nothing selected anywhere, so the only rung that can answer about a
/// workspace is the per-session pointer the tests below write.
///
/// The same shape `sessions.rs` uses for charter-app#63, and for the same reason: these
/// two defects are one defect at two levels, and a reproduction that let another rung
/// answer would prove nothing about which chat a pointer belongs to.
fn bare_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join("charter.toml"), "").expect("a manifest");
    std::fs::create_dir_all(root.join(".charter/sessions")).expect("a state directory");
    (dir, root)
}

/// Who a `charter` running inside chat `session` says it is.
///
/// No pane id and no tty, which is the app's own case: a chat gets a pty of its own and
/// none of `$TERM_SESSION_ID`/`$TMUX_PANE`/`$STY`/`$SSH_TTY`, so the per-session pointer
/// is the only one there is and nothing catches a wrong key by accident.
fn who_it_is(session: u32) -> purlis_core::active::Ids {
    let held = HashMap::from([(
        purlis_core::active::SESSION_ID_ENV.to_owned(),
        session.to_string(),
    )]);
    purlis_core::active::Ids::of(&|name| held.get(name).cloned())
}

/// `charter ws use <name>` from inside chat `session`, through the writer the command
/// itself uses.
fn picks(root: &std::path::Path, session: u32, name: &str) {
    use purlis_core::wscmd::select::{Scope, set_active};
    assert_eq!(
        set_active(root, name, &who_it_is(session), false),
        Scope::Session,
        "the selection did not land on the chat's own pointer"
    );
}

/// The workspace a `charter` inside chat `session` resolves, and the rung that answered.
fn workspace_of(root: &std::path::Path, session: u32) -> purlis_core::active::ActiveWorkspace {
    purlis_core::active::workspace(&purlis_core::active::Asking {
        root,
        // Not inside any tree, so the cwd rung cannot answer and the pointers decide.
        cwd: root,
        flag: None,
        ids: &who_it_is(session),
        env: None,
    })
}

#[test]
fn a_chat_that_comes_back_reads_the_workspace_it_picked_and_not_the_one_below_it() {
    // **The defect as the operator meets it** (charter-app#90). Two chats, each with a
    // workspace of its own. Close the first, quit, launch again: the second chat comes
    // back — and before this fix it came back as chat 1, reading the workspace the
    // CLOSED chat had picked and holding the lock that chat took. Nothing on screen says
    // so; the sidebar shows the chat the operator left, filed under a stranger's
    // workspace.
    //
    // It is the issue's headline shape turned around, and the turn matters: the report
    // was about a NEW chat inheriting a closed one's pointer, but a chat does not have to
    // be new. Numbers were dealt again at every launch in the order the record held, so
    // closing ANY chat shifted every later one down by one, and each of them landed on
    // the pointer of the chat that used to sit above it.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (_plane, root) = bare_plane();
    let chats = Chats::new().in_project(&root);
    let first = chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let second = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();
    picks(&root, first, "finance");
    picks(&root, second, "ops");
    chats.close(first).expect("it closes");
    let record = chats.record();
    chats.end_all();

    let relaunched = Chats::new().in_project(&root);
    let back = relaunched.put_back(&record, SIZE);

    assert_eq!(
        back.len(),
        1,
        "the chat that was left open did not come back"
    );
    let found = workspace_of(&root, back[0].session);
    assert_eq!(
        found.name, "ops",
        "chat {} came back under number {} and read the workspace of the chat that closed",
        back[0].name, back[0].session
    );
    assert_eq!(
        found.rung,
        purlis_core::active::WorkspaceRung::SessionPointer,
        "it landed on 'ops' by some other rung, which proves nothing about the key"
    );
    relaunched.end_all();
}

#[test]
fn a_new_chat_is_not_given_the_number_of_one_that_closed_before_the_quit() {
    // The half the issue reports in as many words: close a chat, and the number it was
    // using is free again at the next launch, while its `.charter/sessions/<n>.workspace`
    // and `<n>.lock` are still on disk — `wscmd::select`'s prune only drops them at 30
    // days. So the next chat the operator starts is filed in a workspace a chat they
    // closed had chosen, and is locked to it.
    //
    // Keeping each chat's number is not enough for this one. The closed chat is not in
    // the record at all, so nothing the chats say could hold the number; it is the
    // record's own counter that does (`Record::dealt`).
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let (_plane, root) = bare_plane();
    let chats = Chats::new().in_project(&root);
    let staying = chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let going = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();
    picks(&root, staying, "finance");
    picks(&root, going, "ops");
    chats.close(going).expect("it closes");
    let record = chats.record();
    chats.end_all();

    let relaunched = Chats::new().in_project(&root);
    relaunched.put_back(&record, SIZE);
    let fresh = relaunched
        .start(&chat(&claude, "ide.9", None), SIZE)
        .unwrap();

    let found = workspace_of(&root, fresh);
    assert_eq!(
        found.name, "default",
        "a chat the operator just started was handed number {fresh}, which a closed chat \
             had already selected a workspace under"
    );
    assert_eq!(found.rung, purlis_core::active::WorkspaceRung::BuiltIn);
    relaunched.end_all();
}

#[test]
fn the_record_keeps_the_number_each_chat_is_running_under() {
    // What the two tests above rest on, written out so a record that stops carrying it
    // fails here rather than only in a reproduction that takes a plane to see.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();
    let first = chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let second = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();

    let record = chats.record();

    let numbers: Vec<Option<u32>> = record.chats.iter().map(|one| one.number).collect();
    assert_eq!(numbers, vec![Some(first), Some(second)]);
    assert_eq!(record.dealt, second, "the counter is not what was dealt");
    chats.end_all();
}

#[test]
fn the_counter_a_quit_records_counts_the_chats_that_closed_too() {
    // `dealt` is a high water mark and not a count of what is open. A record that wrote
    // the number of chats it holds would hand the next launch a number it had already
    // spent, which is the whole defect.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();
    chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let going = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();

    chats.close(going).expect("it closes");

    let record = chats.record();
    assert_eq!(record.chats.len(), 1);
    assert_eq!(record.dealt, going);
    chats.end_all();
}

#[test]
fn the_next_number_the_picker_names_is_the_one_the_next_start_takes() {
    // Start a chat's Name placeholder says `steward 10` by it (#3376): one past the highest
    // dealt, so a chat that closed has spent its number too.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();
    assert_eq!(chats.next_number(), 1);
    chats.start(&chat(&claude, "ide.7", None), SIZE).unwrap();
    let going = chats.start(&chat(&claude, "ide.8", None), SIZE).unwrap();
    chats.close(going).expect("it closes");

    let named = chats.next_number();
    let next = chats.start(&chat(&claude, "ide.9", None), SIZE).unwrap();

    assert_eq!(named, going + 1);
    assert_eq!(
        named, next,
        "the placeholder named a number the start did not take"
    );
    chats.end_all();
}

#[test]
fn a_record_written_before_chats_kept_their_numbers_is_put_back_as_it_always_was() {
    // Every operator has one of these at the first launch after this change, and it says
    // nothing about which chat was which. Dealing them in order is what the app did
    // before and the only honest answer; from that launch on they carry numbers.
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    let chats = Chats::new();

    let back = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat(&claude, "ide.7", None), chat(&claude, "ide.8", None)],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let numbers: Vec<u32> = back.iter().map(|one| one.session).collect();
    assert_eq!(numbers, vec![1, 2]);
    chats.end_all();
}

#[test]
fn a_chat_put_back_is_one_the_next_quit_records_again() {
    // A relaunch that lost the record would resume once and never again.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![chat(&a_claude(dir.path()), "ide.7", Some(ID))],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    let again = chats.record();

    assert_eq!(again.chats.len(), 1);
    assert_eq!(again.chats[0].resume, Some(SessionId::new(ID).unwrap()));
}

// ----- the pin is a tab's (#4762, ADR 0039 amended 2026-11-02) -----

#[test]
fn a_chat_pin_is_read_once_from_a_record_with_no_tabs_and_then_the_tab_holds_it() {
    // The upgrade: a record written before tabs were recorded is read into a layout as it is
    // put back (#4758), and a chat's pin becomes its tab's there. From then on the chat's own
    // field is never written again.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let old = Record {
        chats: vec![
            Chat {
                number: Some(3),
                active: true,
                ..chat(&a_claude(dir.path()), "ide.3", None)
            },
            Chat {
                number: Some(4),
                pinned: true,
                ..chat(&a_claude(dir.path()), "ide.4", None)
            },
        ],
        ..Record::default()
    };
    chats.put_back(&old, SIZE);
    let upgraded = chats.record().layout.expect("the upgrade's layout");
    assert!(
        upgraded
            .tabs
            .iter()
            .any(|tab| tab.pinned && tab.tree == Node::chat(4)),
        "the pin is on chat 4's tab from the upgrade: {upgraded:?}"
    );

    let mut pinned = front_tab(Node::chat(4));
    pinned.pinned = true;
    chats.lay_out(on_ops(vec![pinned])).unwrap();

    let record = chats.record();
    assert!(
        record.chats.iter().all(|one| !one.pinned),
        "the chat's pin retired"
    );
    assert!(record.layout.expect("a layout").tabs[0].pinned);
    chats.end_all();
}

// ----- an announced chat's "new" mark (#4759) -----

#[test]
fn an_announced_chat_stays_new_until_a_layout_puts_it_on_screen_and_then_the_record_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let record = Record {
        chats: vec![
            Chat {
                number: Some(3),
                active: true,
                ..chat(&a_claude(dir.path()), "ide.3", None)
            },
            Chat {
                number: Some(4),
                unseen: true,
                ..chat(&a_claude(dir.path()), "ide.4", None)
            },
        ],
        ..Record::default()
    };
    chats.put_back(&record, SIZE);

    // On no screen: it stays new, and nothing is said.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(3))]))
        .unwrap();
    assert!(!chats.mark_seen(), "nothing new is on screen");
    assert!(chats.record().chats[1].unseen, "still new");

    // The person picks it: on screen, so looked at, once.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(4))]))
        .unwrap();
    assert!(chats.mark_seen(), "its mark went");
    assert!(!chats.record().chats[1].unseen, "and the record keeps that");
    assert!(!chats.mark_seen(), "said once");
    chats.end_all();
}

// ----- every tab as a tree of panes (#4751) -----

/// A tab on `ops`'s strip in the project window, in front, holding `tree`, its first pane
/// focused.
fn front_tab(tree: Node) -> Tab {
    Tab {
        workspace: Some("ops".to_owned()),
        pinned: false,
        window: 0,
        front: true,
        focused: tree.to_a_pane(&[]),
        tree,
    }
}

/// The project window drawing `ops`'s strip, holding `tabs`.
fn on_ops(tabs: Vec<Tab>) -> Layout {
    Layout {
        strip: Some("ops".to_owned()),
        tabs,
    }
}

/// Two chats split side by side, the first holding a third of the tab.
fn split_of(one: u32, other: u32) -> Layout {
    on_ops(vec![front_tab(Node::split(
        Direction::Row,
        333,
        Node::chat(one),
        Node::chat(other),
    ))])
}

/// **A one-job pane is never written to the record** (#4761, spec #4750): a terminal editor or
/// a forge login is held out of every record from its start, and its split out of the layout
/// held, so the pane it opened beside is what a relaunch draws, and what is on screen still
/// counts that pane's chat (a task is not ended under the person, nor a notice sent about it).
#[test]
fn a_one_job_pane_is_never_written_to_the_record() {
    let (chats, wrote) = recorded();
    let shell = chats
        .start(&chat("/bin/sh", "shell 1", None), SIZE)
        .unwrap();
    let job = chats
        .start_one_job(&chat("/bin/sh", "vim notes.md", None), SIZE)
        .unwrap();

    chats.lay_out(split_of(shell, job)).expect("both are open");

    assert!(chats.is_one_job(job));
    assert!(!chats.is_one_job(shell));
    let listed = chats.open_now();
    assert!(listed.iter().any(|one| one.session == job && one.one_job));
    assert!(
        listed
            .iter()
            .any(|one| one.session == shell && !one.one_job)
    );
    let records = lock(&wrote).clone();
    assert!(
        !records.is_empty(),
        "the starts and the layout were written"
    );
    for record in &records {
        assert!(
            record.chats.iter().all(|one| one.name != "vim notes.md"),
            "no record names the one-job program: {:?}",
            record.chats
        );
        if let Some(layout) = &record.layout {
            assert!(!layout.chats().contains(&job), "{layout:?}");
        }
    }
    let last = records.last().expect("written");
    assert_eq!(
        last.layout,
        Some(on_ops(vec![front_tab(Node::chat(shell))])),
        "the pane it opened beside takes its split's place"
    );
    assert_eq!(chats.looked_at(), vec![shell]);
    let _ = chats.close(job);
    assert!(!chats.is_one_job(job));
    let _ = chats.close(shell);
}

/// **A shell keeps running when its pane closes, or another chat replaces it** (#4761, ADR 0062
/// as amended 2026-11-02): a layout without its pane ends nothing, and the record still holds
/// it, so a dev server in it survives a closed pane and a relaunch.
#[test]
fn a_shell_keeps_running_when_its_pane_closes() {
    let (chats, wrote) = recorded();
    let shell = chats
        .start(&chat("/bin/sh", "shell 1", None), SIZE)
        .unwrap();
    let other = chats
        .start(&chat("/bin/sh", "shell 2", None), SIZE)
        .unwrap();
    let third = chats
        .start(&chat("/bin/sh", "shell 3", None), SIZE)
        .unwrap();
    chats.lay_out(split_of(shell, other)).unwrap();

    // Its pane closed: the split goes, the other pane takes its place.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(other))]))
        .unwrap();
    let running = |session: u32| chats.open_now().iter().any(|one| one.session == session);
    assert!(running(shell), "closing its pane ended nothing");
    assert!(
        lock(&wrote)
            .last()
            .is_some_and(|record| record.chats.iter().any(|one| one.name == "shell 1")),
        "the record still holds it"
    );

    // Shown again, then replaced in its pane by another chat.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(shell))]))
        .unwrap();
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(third))]))
        .unwrap();
    assert!(running(shell), "replacing it ended nothing");
    assert!(!chats.looks_at(shell));
    for session in [shell, other, third] {
        let _ = chats.close(session);
    }
}

#[test]
fn two_chats_split_together_with_their_sizes_come_back_the_same_after_a_relaunch() {
    let dir = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let before = Chats::new();
    let one = before
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let other = before
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();
    before.lay_out(split_of(one, other)).expect("both are open");
    purlis_core::reopen::write(plane.path(), &before.record()).unwrap();
    before.end_all();

    let after = Chats::new();
    let record = purlis_core::reopen::read_or_refusal(plane.path()).unwrap();
    after.put_back(&record, SIZE);

    assert_eq!(after.layout(), Some(split_of(one, other)));
    assert_eq!(after.looked_at(), vec![one, other]);
    after.end_all();
}

#[test]
fn view_tabs_and_a_file_tab_s_divider_are_still_written_beside_a_layout() {
    // #4751 keeps `views[]` beside the tree: which view tab was in front and where a file
    // tab's divider was are read from it, and the switch (#4758) retires it.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let devops = View {
        from: None,
        view: "persona".to_owned(),
        key: "devops".to_owned(),
        title: "devops".to_owned(),
        workspace: Some("ops".to_owned()),
        at: 1,
        active: true,
        pinned: false,
        split: None,
    };
    let files = View {
        view: "piece-files".to_owned(),
        key: "ops/svc/fix-login".to_owned(),
        title: "fix-login".to_owned(),
        at: 2,
        active: false,
        split: Some(31),
        ..devops.clone()
    };
    chats.hold_views(vec![devops.clone(), files.clone()]);

    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(one))]))
        .unwrap();

    let record = chats.record();
    assert!(record.layout.is_some());
    assert_eq!(record.views, vec![devops, files]);
    let _ = chats.close(one);
}

#[test]
fn a_view_beside_a_chat_comes_back_after_a_relaunch() {
    let dir = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let before = Chats::new();
    let one = before
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let view = Node::Pane(Pane::View(PaneView {
        from: None,
        view: "persona".to_owned(),
        key: "steward".to_owned(),
        title: "steward".to_owned(),
        split: None,
    }));
    let laid = on_ops(vec![front_tab(Node::split(
        Direction::Row,
        EVEN,
        Node::chat(one),
        view,
    ))]);
    before.lay_out(laid.clone()).unwrap();
    purlis_core::reopen::write(plane.path(), &before.record()).unwrap();
    before.end_all();

    let after = Chats::new();
    let record = purlis_core::reopen::read_or_refusal(plane.path()).unwrap();
    assert!(record.holds_anything());
    after.put_back(&record, SIZE);

    assert_eq!(after.layout(), Some(laid));
    assert_eq!(after.looked_at(), vec![one], "a view counts as nothing");
    after.end_all();
}

#[test]
fn a_layout_that_names_a_chat_twice_is_refused_says_why_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    // In a tab window too: a chat is shown once across every window of the project.
    let mut elsewhere = Tab::of_chat(Some("ops"), one);
    elsewhere.window = 2;
    elsewhere.front = true;
    let twice = on_ops(vec![front_tab(Node::chat(one)), elsewhere]);

    let refused = chats.lay_out(twice).unwrap_err();

    assert!(
        refused.starts_with("purlis did not keep that layout")
            && refused.contains(&format!("chat {one} in two panes")),
        "{refused}"
    );
    assert_eq!(chats.layout(), None);
    assert_eq!(chats.record().layout, None);
    let _ = chats.close(one);
}

#[test]
fn a_layout_that_names_a_chat_not_open_is_refused_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(one))]))
        .unwrap();

    let refused = chats.lay_out(split_of(one, one + 100)).unwrap_err();

    assert!(
        refused.contains(&format!("chat {}, which is not open", one + 100)),
        "{refused}"
    );
    assert_eq!(chats.looked_at(), vec![one], "the layout held stands");
    let _ = chats.close(one);
}

#[test]
fn what_is_on_screen_is_every_chat_in_the_front_tab_of_the_drawn_strip_and_each_tab_window() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let mut numbers = Vec::new();
    for name in ["ide.1", "ide.2", "ide.3", "ide.4", "ide.5"] {
        numbers.push(
            chats
                .start(&chat(&a_claude(dir.path()), name, None), SIZE)
                .unwrap(),
        );
    }
    let [split_a, split_b, behind, other_strip, torn_off] = numbers[..] else {
        unreachable!("five chats")
    };
    let mut behind_tab = Tab::of_chat(Some("ops"), behind);
    behind_tab.front = false;
    let mut other = Tab::of_chat(None, other_strip);
    other.front = true;
    let mut window = Tab::of_chat(Some("ops"), torn_off);
    window.window = 2;
    window.front = true;
    let with_empty = front_tab(Node::split(
        Direction::Column,
        EVEN,
        Node::split(
            Direction::Row,
            EVEN,
            Node::chat(split_a),
            Node::chat(split_b),
        ),
        Node::Pane(Pane::Empty),
    ));

    chats
        .lay_out(on_ops(vec![with_empty, behind_tab, other, window]))
        .unwrap();

    assert_eq!(chats.looked_at(), vec![split_a, split_b, torn_off]);
    assert_eq!(
        chats.front(),
        Some(split_a),
        "the first chat of the front tab"
    );
    chats.end_all();
}

#[test]
fn a_pane_shows_a_chat_that_is_not_its_tab_s_own_beside_an_empty_pane_and_the_core_keeps_it() {
    // #4755: a pane shows any open chat. Here a task sits on another strip's tab, its asker in
    // no pane at all, beside an empty pane; the core holds it, says it is on screen, and a
    // relaunch reads it back as it was.
    use purlis_core::reopen::{HandedFrom, Mode, Owed};
    let dir = tempfile::tempdir().unwrap();
    let plane = tempfile::tempdir().unwrap();
    let before = Chats::new();
    let asker = before
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let task = before
        .start(
            &Chat {
                from: Some(HandedFrom {
                    id: None,
                    chat: asker,
                    name: "ide.7".to_owned(),
                    workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
                    report: Owed::Due,
                    mode: Mode::Task,
                    depth: 1,
                    root: None,
                    above: None,
                    by_person: false,
                    long: false,
                }),
                ..chat(&a_claude(dir.path()), "talk", None)
            },
            SIZE,
        )
        .unwrap();
    let laid = on_ops(vec![front_tab(Node::split(
        Direction::Row,
        EVEN,
        Node::Pane(Pane::Empty),
        Node::chat(task),
    ))]);

    before
        .lay_out(laid.clone())
        .expect("any open chat may be in a pane");

    assert_eq!(before.looked_at(), vec![task]);
    purlis_core::reopen::write(plane.path(), &before.record()).unwrap();
    before.end_all();
    let after = Chats::new();
    let record = purlis_core::reopen::read_or_refusal(plane.path()).unwrap();
    after.put_back(&record, SIZE);
    assert_eq!(after.layout(), Some(laid));
    after.end_all();
}

#[test]
fn a_pane_follows_its_chat_when_that_chat_is_started_again_under_a_new_number() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let other = chats
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();
    chats.lay_out(split_of(one, other)).unwrap();

    chats.followed(other, 52);

    assert_eq!(chats.layout(), Some(split_of(one, 52)));
    assert_eq!(chats.record().layout, Some(split_of(one, 52)));
    chats.end_all();
}

#[test]
fn a_focused_path_that_stops_at_a_split_is_held_as_the_pane_a_relaunch_reads() {
    // The record reads a focused path on to a pane, so the core holds it so too: what it holds
    // is what the next launch reads back.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let other = chats
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();
    let mut laid = split_of(one, other);
    laid.tabs[0].focused = Vec::new();

    chats.lay_out(laid).unwrap();

    assert_eq!(
        chats.layout().map(|held| held.tabs[0].focused.clone()),
        Some(vec![purlis_core::reopen::layout::Side::A])
    );
    chats.end_all();
}

#[test]
fn laying_out_what_is_already_laid_out_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let written = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = std::sync::Arc::clone(&written);
    let chats = Chats::recorded_by(Box::new(move |_| {
        counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    let one = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(one))]))
        .unwrap();
    let before = written.load(std::sync::atomic::Ordering::SeqCst);

    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(one))]))
        .unwrap();

    assert_eq!(written.load(std::sync::atomic::Ordering::SeqCst), before);
    let _ = chats.close(one);
}

#[test]
fn a_record_with_no_tabs_list_is_upgraded_to_a_layout_at_launch_and_its_old_fields_retire() {
    // A record written before tabs were recorded (#4758): the upgrade keeps the tab in front,
    // with the chat beside it as the other half of a split, and the old fields that said so
    // retire at once, since the layout now says it.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let old = Record {
        chats: vec![
            Chat {
                number: Some(3),
                active: true,
                ..chat(&a_claude(dir.path()), "ide.3", None)
            },
            Chat {
                number: Some(4),
                beside: Some(3),
                ..chat(&a_claude(dir.path()), "ide.4", None)
            },
        ],
        ..Record::default()
    };
    chats.put_back(&old, SIZE);
    let layout = chats.layout().expect("the upgrade lays it out");
    assert_eq!(layout.tabs.len(), 1);
    assert!(layout.tabs[0].front);
    assert_eq!(
        layout.tabs[0].tree,
        Node::Split(Box::new(purlis_core::reopen::layout::Split {
            direction: purlis_core::reopen::layout::Direction::Row,
            share: 500,
            a: Node::chat(3),
            b: Node::chat(4),
        }))
    );
    assert_eq!(chats.looked_at(), vec![3, 4]);
    let record = chats.record();
    assert!(
        record
            .chats
            .iter()
            .all(|one| one.beside.is_none() && one.shows.is_none())
    );
    assert!(record.layout.is_some());

    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(4))]))
        .unwrap();

    assert_eq!(chats.looked_at(), vec![4]);
    chats.end_all();
}

// ----- a task chat is listed with no tab until the person opens it (#1447) -----

/// A chat another chat started in `mode`, as a dispatch records it.
fn started_by(dir: &std::path::Path, name: &str, mode: purlis_core::reopen::Mode) -> Chat {
    Chat {
        from: Some(purlis_core::reopen::HandedFrom {
            id: None,
            chat: 1,
            name: "steward 1".to_owned(),
            workspace: purlis_core::active::Place::Workspace("ide".to_owned()),
            report: purlis_core::reopen::Owed::Due,
            mode,
            depth: 1,
            root: None,
            above: None,
            by_person: false,
            long: false,
        }),
        ..chat(&a_claude(dir), name, None)
    }
}

#[test]
fn a_task_chat_is_open_with_no_tab_and_a_handoff_with_one() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let task = chats
        .start(
            &started_by(dir.path(), "ide.7", purlis_core::reopen::Mode::Task),
            SIZE,
        )
        .unwrap();
    let handoff = chats
        .start(
            &started_by(dir.path(), "ide.8", purlis_core::reopen::Mode::Handoff),
            SIZE,
        )
        .unwrap();

    let tab = |session: u32| {
        chats
            .open_now()
            .into_iter()
            .find(|open| open.session == session)
            .map(|open| open.tab)
    };
    assert_eq!(tab(task), Some(false), "listed, and on no strip");
    assert_eq!(tab(handoff), Some(true));
    let _ = chats.close(task);
    let _ = chats.close(handoff);
}

#[test]
fn the_tab_the_person_opened_on_a_task_chat_is_written_into_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(
            &started_by(dir.path(), "ide.7", purlis_core::reopen::Mode::Task),
            SIZE,
        )
        .unwrap();

    chats.open_tab(session).expect("the chat is open");

    assert!(chats.record().chats[0].has_tab());
    assert!(chats.open_now()[0].tab);
    let _ = chats.close(session);
}

#[test]
fn a_task_chat_sent_back_out_of_its_tab_has_none_in_the_record_and_is_still_open() {
    // #1489: the minimise. Nothing ends: the chat is open, and listed with no tab again.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(
            &started_by(dir.path(), "ide.7", purlis_core::reopen::Mode::Task),
            SIZE,
        )
        .unwrap();
    chats.open_tab(session).unwrap();

    chats.close_tab(session).expect("the chat is open");

    assert!(!chats.record().chats[0].has_tab());
    assert_eq!(chats.open_now().len(), 1, "sending it back ends nothing");
    assert!(!chats.open_now()[0].tab);
    let _ = chats.close(session);
}

#[test]
fn closing_the_tab_of_a_chat_that_is_not_a_task_ends_nothing_and_is_not_refused() {
    // #4756: a close never ends a chat. The core used to refuse this one, saying closing its
    // tab ends it; now it is open after, and its record is as it was.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "steward", None), SIZE)
        .unwrap();
    let before = chats.record();

    chats
        .close_tab(session)
        .expect("a tab close is never refused");

    assert_eq!(chats.open_now().len(), 1, "closing its tab ends nothing");
    assert_eq!(chats.record(), before);
    let _ = chats.close(session);
}

/// A chat dispatched from `asker` in the lineage `root`, as a dispatch records it.
fn dispatched_from(dir: &std::path::Path, name: &str, asker: u32, root: &str) -> Chat {
    let mut chat = started_by(dir, name, purlis_core::reopen::Mode::Task);
    let from = chat.from.as_mut().expect("asked");
    from.chat = asker;
    from.root = Some(root.to_owned());
    chat
}

fn root_of(chats: &Chats, session: u32) -> Option<String> {
    chats
        .recorded_chat(session)
        .and_then(|chat| chat.from)
        .and_then(|from| from.root)
}

#[test]
fn a_lineage_whose_first_chat_comes_back_under_a_new_id_stays_one_with_its_middle_closed() {
    // #1456: the first chat started again under an id of its own, and the chat between it
    // and a task below already closed. The walk by who asked whom cannot reach the task, so
    // the root it names is what keeps the lineage one.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let first = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let root = chats
        .recorded_chat(first)
        .and_then(|chat| chat.identity.id)
        .expect("an id");
    let middle = chats
        .start(&dispatched_from(dir.path(), "ide.8", first, &root), SIZE)
        .unwrap();
    let below = chats
        .start(&dispatched_from(dir.path(), "ide.9", middle, &root), SIZE)
        .unwrap();
    let _ = chats.close(middle);
    // In its place, under an id it was just given.
    let again = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let new_root = chats
        .recorded_chat(again)
        .and_then(|chat| chat.identity.id)
        .expect("an id");
    assert_ne!(new_root, root);

    chats.followed(first, again);

    assert_eq!(root_of(&chats, below).as_deref(), Some(new_root.as_str()));
    for session in [first, below, again] {
        let _ = chats.close(session);
    }
}

#[test]
fn a_chat_in_the_middle_of_a_lineage_started_again_changes_no_root() {
    // Only the chat the person started is a lineage's root: a task below it that comes back
    // under an id of its own is still in its asker's lineage, and so is what it asked for.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let first = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let root = chats
        .recorded_chat(first)
        .and_then(|chat| chat.identity.id)
        .expect("an id");
    let middle = chats
        .start(&dispatched_from(dir.path(), "ide.8", first, &root), SIZE)
        .unwrap();
    let below = chats
        .start(&dispatched_from(dir.path(), "ide.9", middle, &root), SIZE)
        .unwrap();
    let again = chats
        .start(&dispatched_from(dir.path(), "ide.8", first, &root), SIZE)
        .unwrap();

    chats.followed(middle, again);

    assert_eq!(root_of(&chats, below).as_deref(), Some(root.as_str()));
    assert_eq!(root_of(&chats, again).as_deref(), Some(root.as_str()));
    for session in [first, middle, below, again] {
        let _ = chats.close(session);
    }
}

#[test]
fn opening_the_tab_of_a_chat_that_has_one_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let written = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = std::sync::Arc::clone(&written);
    let chats = Chats::recorded_by(Box::new(move |_| {
        counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let before = written.load(std::sync::atomic::Ordering::SeqCst);

    chats.open_tab(session).unwrap();

    assert_eq!(written.load(std::sync::atomic::Ordering::SeqCst), before);
    assert!(chats.open_tab(session + 40).is_err(), "no chat, no tab");
    let _ = chats.close(session);
}

// ----- a chat the session host still runs is reattached (#2006) -----

/// What a launch's record holds of chat 3, put back on `host`, which still runs it.
fn left_running_in(plane: &std::path::Path, chat: Chat) -> Record {
    Record {
        chats: vec![Chat {
            number: Some(3),
            identity: purlis_core::reopen::Identity {
                id: Some(CHAT_ID.to_owned()),
                device: Some(DEVICE.to_owned()),
                run: Some(OLD_RUN.to_owned()),
                resumed_from: None,
            },
            cwd: Some(plane.to_path_buf()),
            ..chat
        }],
        ..Default::default()
    }
}

/// D-2006-7: a chat put back that the session host still runs is reattached, and nothing a
/// start begins happens: no program starts (so no first message is typed and nothing is
/// handed to it), no run begins and the run it was is kept, no trust event or opt-out count
/// is written, and no start is held.
#[test]
fn a_chat_the_host_still_runs_is_reattached_and_nothing_a_start_begins_happens() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running(3, None);
    let mut chats = restartable_chats_of(plane.path(), host.clone());
    let begun = beginning(&mut chats);
    let decided = Arc::new(Mutex::new(0_usize));
    chats.when_the_sandbox_is_decided({
        let decided = Arc::clone(&decided);
        Box::new(move |_| {
            *lock(&decided) += 1;
            Ok(())
        })
    });
    let held = Arc::new(Mutex::new(0_usize));
    chats.when_a_held_start_is_settled({
        let held = Arc::clone(&held);
        Box::new(move |_, _| *lock(&held) += 1)
    });

    let open = chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );

    assert_eq!(open.iter().map(|one| one.session).collect::<Vec<_>>(), [3]);
    assert_eq!(host.reattached(), [3]);
    assert!(host.openings().is_empty(), "nothing was started");
    assert!(lock(&begun).is_empty(), "no run began");
    assert_eq!(*lock(&decided), 0, "no trust event, no opt-out count");
    assert_eq!(*lock(&held), 0, "no start held");
    assert_eq!(
        chats.record().chats[0].identity.run.as_deref(),
        Some(OLD_RUN),
        "the run it was"
    );
    assert!(chats.start_notes(3).is_empty());
}

/// D-2006-11: a chat the host still runs is reattached, never refused, though its start
/// now would be: its profile is gone. It is not closed.
#[test]
fn a_chat_the_host_still_runs_is_reattached_though_its_start_would_now_be_refused() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());

    let open = chats.put_back(
        &left_running_in(
            plane.path(),
            Chat {
                profile: Some("gone".to_owned()),
                ..a_resumable_chat_in(plane.path())
            },
        ),
        SIZE,
    );
    chats.close_what_was_not_reattached();

    assert_eq!(open.iter().map(|one| one.session).collect::<Vec<_>>(), [3]);
    assert_eq!(host.reattached(), [3]);
    assert!(host.closed_left().is_empty(), "it is not closed");
    assert!(chats.would_not_start().is_empty());
}

/// D-2006-11: the sandbox narrowed while no window was there. The chat the host still runs
/// in the plan it was started with is reattached, not closed, and the narrowing reaches it
/// as it reaches any running chat a sandbox change left behind: it is offered a restart on
/// the sandbox of now (#1428).
#[test]
fn a_chat_whose_sandbox_narrowed_while_no_window_was_there_is_reattached_and_offered_a_restart() {
    use purlis_core::sandbox::egress::{PLAIN_PORTS, TUNNEL_PORTS};
    use purlis_core::sandbox::plan::{Beside, Plan};
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    // It ran reaching anything; the project's sandbox reaches less now.
    host.leaves_running(
        3,
        Some(Plan {
            reach: purlis_core::sandbox::reach::Reach::anything().written(),
            tunnel_ports: TUNNEL_PORTS.to_vec(),
            plain_ports: PLAIN_PORTS.to_vec(),
            any_port: true,
            beside: Beside::Wrapped,
            given: Vec::new(),
        }),
    );
    let chats = restartable_chats_of(plane.path(), host.clone());

    chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );
    chats.close_what_was_not_reattached();

    assert_eq!(host.reattached(), [3]);
    assert!(host.closed_left().is_empty(), "it is not closed");
    assert_eq!(
        older(&chats).expect("it is offered a restart").sessions(),
        [3]
    );
}

/// #3443: a grade's chat a crash left running in the session host is never put back: its grade
/// ended with the run it was in (SK-16). Nothing reattaches it, so the launch's end of what was
/// not reattached (#2006) ends it, and no chat of it is opened or started.
#[test]
fn a_graded_chat_a_crash_left_in_the_session_host_is_ended_at_relaunch() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());

    let open = chats.put_back(
        &left_running_in(
            plane.path(),
            Chat {
                graded: true,
                ..a_resumable_chat_in(plane.path())
            },
        ),
        SIZE,
    );
    chats.close_what_was_not_reattached();

    assert!(open.is_empty(), "no chat of it is opened");
    assert!(host.reattached().is_empty(), "nothing reattached it");
    assert!(host.openings().is_empty(), "nothing was started");
    assert_eq!(host.closed_left(), [3], "the host ended it");
}

/// Whether chat 3, reattached from `host` on the project at `plane`, is judged to run with no
/// sandbox at all, as a browser login judges it (#2560).
fn reattached_runs_unconfined(plane: &std::path::Path, host: Pretend) -> (Chats, bool) {
    let chats = restartable_chats_of(plane, host.clone());
    chats.put_back(&left_running_in(plane, a_resumable_chat_in(plane)), SIZE);
    assert_eq!(host.reattached(), [3]);
    let (_, unconfined) = chats.login_asker(plane, 3).expect("open");
    (chats, unconfined)
}

/// #2636: a chat its host ran sandboxed, reattached after the project's sandbox was turned
/// off while no window was there, is judged by how its run started: sandboxed.
#[test]
fn a_sandboxed_run_reattached_after_the_sandbox_was_turned_off_is_judged_sandboxed() {
    use purlis_session_protocol::session::RunSandbox;
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(
        plane.path().join(purlis_core::plane::MANIFEST),
        "[sandbox]\nmode = \"off\"\n",
    )
    .expect("charter.toml");
    let host = Pretend::default();
    host.leaves_running_in(3, None, Some(RunSandbox::By("seatbelt".to_owned())));

    let (_, unconfined) = reattached_runs_unconfined(plane.path(), host);

    assert!(!unconfined, "it runs in the sandbox it started in");
}

/// #2636: a chat its host ran with no sandbox, reattached after the project's sandbox was
/// turned on while no window was there, is judged by how its run started: unconfined, with no
/// sandbox of now standing in for it.
#[test]
fn an_unsandboxed_run_reattached_after_the_sandbox_was_turned_on_is_judged_unconfined() {
    use purlis_session_protocol::session::RunSandbox;
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running_in(3, None, Some(RunSandbox::Off));

    let (chats, unconfined) = reattached_runs_unconfined(plane.path(), host);

    assert!(unconfined, "it runs with no sandbox, as it started");
    assert!(
        chats.confines_of(3).is_none(),
        "no sandbox of now stands in"
    );
}

/// A chat reattached says what its host says of its run, whatever its record said before.
#[test]
fn a_reattached_run_records_its_hosts_word_on_whether_it_has_a_sandbox() {
    use purlis_session_protocol::session::RunSandbox;
    for (recorded, ran, says) in [
        (false, RunSandbox::Off, true),
        (true, RunSandbox::By("seatbelt".to_owned()), false),
    ] {
        let plane = a_sandboxed_plane();
        let host = Pretend::default();
        host.leaves_running_in(3, None, Some(ran.clone()));
        let chats = restartable_chats_of(plane.path(), host.clone());
        let chat = Chat {
            no_sandbox: recorded,
            ..a_resumable_chat_in(plane.path())
        };
        chats.put_back(&left_running_in(plane.path(), chat), SIZE);
        assert_eq!(host.reattached(), [3]);
        assert_eq!(chats.record().chats[0].no_sandbox, says, "{ran:?}");
    }
}

/// #2636: a chat whose host keeps no record of how its run started (a host before it kept
/// one) is judged sandboxed, and offered a restart.
#[test]
fn a_reattached_run_with_no_record_of_its_sandbox_is_judged_sandboxed_and_offered_a_restart() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running_in(3, None, None);

    let (chats, unconfined) = reattached_runs_unconfined(plane.path(), host);

    assert!(!unconfined, "taken as sandboxed");
    assert!(chats.confines_of(3).is_none());
    assert_eq!(
        older(&chats).expect("it is offered a restart").sessions(),
        [3]
    );
}

// ----- who a chat is beyond this clone, and its runs (ADR 0066, #834) -----

const CHAT_ID: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7F";
const DEVICE: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7G";
const ELSEWHERE: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7J";
const OLD_RUN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7H";

type Begun = std::sync::Arc<Mutex<Vec<(u32, String, String, Began)>>>;

/// Chats on device [`DEVICE`] that keep every run they begin in the answer, oldest first.
fn beginning(chats: &mut Chats) -> Begun {
    chats.on_device(Some(DEVICE.to_owned()));
    let begun: Begun = std::sync::Arc::default();
    let keep = std::sync::Arc::clone(&begun);
    chats.when_a_run_begins(Box::new(move |session, of, cause, _, _| {
        lock(&keep).push((session, of.chat.to_owned(), of.run.to_owned(), cause));
    }));
    begun
}

fn a_ulid(id: Option<&String>) -> bool {
    id.and_then(|id| purlis_core::reopen::a_ulid(id)).is_some()
}

/// What the core's start answers for a shell on no profile.
pub(crate) fn a_shell_ready() -> purlis_core::start::Ready {
    purlis_core::start::Ready {
        program: "/bin/sh".to_owned(),
        command: Vec::new(),
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
        harness: None,
        declared: None,
        session: None,
        how: Reopened::Fresh(Fresh::NoConversationRecorded),
        plugins: purlis_core::harness_plugin::Chosen::new(),
        sandbox: None,
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: None,
        first_message: None,
    }
}

#[test]
fn a_new_chat_is_given_an_id_on_this_device_and_a_first_run_the_record_holds() {
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);

    let session = chats.start(&chat("/bin/sh", "shell", None), SIZE).unwrap();

    let identity = chats.record().chats[0].identity.clone();
    assert!(a_ulid(identity.id.as_ref()), "{identity:?}");
    assert_eq!(identity.device.as_deref(), Some(DEVICE));
    assert!(a_ulid(identity.run.as_ref()), "{identity:?}");
    assert_eq!(
        *lock(&begun),
        vec![(
            session,
            identity.id.clone().unwrap(),
            identity.run.clone().unwrap(),
            Began::Start
        )]
    );
    let _ = chats.close(session);
}

/// #868: the run a start begins is the persona the chat runs as, its harness, the profile
/// it started on, the sandbox's backend where it is under one, and the conversation it was
/// given; what the start does not know, it says it does not.
#[test]
fn a_run_a_start_begins_is_what_the_start_decided_and_says_what_it_does_not_know() {
    use purlis_core::eventlog::{NONE, UNKNOWN};
    let mut on_a_profile = chat("claude", "steward 1", None);
    on_a_profile.profile = Some("work".to_owned());
    let said = |attributes: Attributes| {
        let word = |v: Option<String>| v.unwrap_or_else(|| UNKNOWN.to_owned());
        [
            word(attributes.persona),
            word(attributes.harness),
            word(attributes.profile),
            word(attributes.model_source),
            word(attributes.provider),
            word(attributes.model),
            word(attributes.sandbox),
            word(attributes.conversation),
        ]
    };
    let backend = purlis_core::sandbox::Os::this()
        .backend_word()
        .unwrap_or(UNKNOWN);

    assert_eq!(
        said(run_attributes(
            &on_a_profile,
            Some(Harness::ClaudeCode),
            Some("steward"),
            true,
            Some("0f6c2a1e-aaaa-4bbb-8ccc-123456789abc"),
        )),
        [
            "steward",
            "claude",
            "work",
            UNKNOWN,
            UNKNOWN,
            UNKNOWN,
            backend,
            "0f6c2a1e-aaaa-4bbb-8ccc-123456789abc"
        ]
    );
    // A harness chat on no persona, unsandboxed, whose record names no profile, and that
    // has no conversation yet.
    assert_eq!(
        said(run_attributes(
            &chat("codex", "codex 2", None),
            Some(Harness::Codex),
            None,
            false,
            None,
        )),
        [
            NONE, "codex", UNKNOWN, UNKNOWN, UNKNOWN, UNKNOWN, NONE, UNKNOWN
        ]
    );
    // A shell runs no harness, and so on no profile.
    assert_eq!(
        said(run_attributes(
            &chat("/bin/sh", "shell", None),
            None,
            None,
            false,
            None,
        ))[..3],
        [NONE, NONE, NONE]
    );
}

#[test]
fn a_chat_put_back_after_a_relaunch_keeps_its_id_and_begins_a_reopen_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);
    let was = purlis_core::reopen::Identity {
        id: Some(CHAT_ID.to_owned()),
        device: Some(ELSEWHERE.to_owned()),
        run: Some(OLD_RUN.to_owned()),
        resumed_from: None,
    };

    let open = chats.put_back_here(
        &Record {
            chats: vec![Chat {
                identity: was.clone(),
                ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
            }],
            ..Record::default()
        },
        SIZE,
    );

    let now = chats.record().chats[0].identity.clone();
    assert_eq!(now.id.as_deref(), Some(CHAT_ID), "the same chat");
    assert_eq!(
        now.device.as_deref(),
        Some(ELSEWHERE),
        "the origin device is the one that minted it"
    );
    assert_ne!(now.run.as_deref(), Some(OLD_RUN), "a run of its own");
    assert_eq!(
        *lock(&begun),
        vec![(
            open[0].session,
            CHAT_ID.to_owned(),
            now.run.clone().unwrap(),
            Began::Reopen
        )]
    );
}

#[test]
fn a_chat_recorded_before_ids_is_given_one_at_the_launch_that_reads_it_in_a_reopen_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);

    chats.put_back_here(
        &Record {
            chats: vec![chat(&a_claude(dir.path()), "ide.7", Some(ID))],
            ..Record::default()
        },
        SIZE,
    );

    let now = chats.record().chats[0].identity.clone();
    assert!(a_ulid(now.id.as_ref()));
    assert_eq!(now.device.as_deref(), Some(DEVICE));
    assert_eq!(lock(&begun)[0].3, Began::Reopen, "ADR 0066's migration");
}

#[test]
fn a_chat_a_rename_left_without_its_conversation_comes_back_in_a_fresh_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);

    chats.put_back_here(
        &Record {
            chats: vec![Chat {
                renamed_from: Some("alpha".to_owned()),
                identity: purlis_core::reopen::Identity {
                    id: Some(CHAT_ID.to_owned()),
                    ..Default::default()
                },
                ..chat(&a_claude(dir.path()), "ide.7", None)
            }],
            ..Record::default()
        },
        SIZE,
    );

    assert_eq!(lock(&begun)[0].1, CHAT_ID);
    assert_eq!(lock(&begun)[0].3, Began::Fresh);
}

#[test]
fn a_chat_started_again_in_place_of_one_that_lost_its_conversation_is_that_chat_in_a_fresh_run() {
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);
    let first = chats.start(&chat("/bin/sh", "ide.7", None), SIZE).unwrap();
    let was = chats.record().chats[0].identity.clone();
    // The window closes the tab before it asks for the fresh start, so the chat it names
    // may already be gone.
    chats.close(first).unwrap();

    let again = chats
        .start_ready_instead_of(
            first,
            &chat("/bin/sh", "ide.7", None),
            &a_shell_ready(),
            SIZE,
        )
        .unwrap();

    let now = chats.record().chats[0].identity.clone();
    assert_eq!(now.id, was.id, "the same chat");
    assert_ne!(now.run, was.run);
    let last = lock(&begun).last().cloned().unwrap();
    assert_eq!((last.0, last.3), (again, Began::Fresh));
    let _ = chats.close(again);
}

#[test]
fn a_dispatched_chat_started_again_in_its_own_place_keeps_its_lineage_and_its_task() {
    // A task chat whose conversation could not be brought back is started fresh in its
    // place by the window, which knows neither who dispatched it nor what it is called.
    // It is the same chat: it still owes its report, at its depth, in its lineage.
    let chats = Chats::new();
    let root = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
    let lineage = purlis_core::reopen::HandedFrom {
        id: None,
        chat: 1,
        name: "steward 1".to_owned(),
        workspace: purlis_core::active::Place::Workspace("ide".to_owned()),
        report: purlis_core::reopen::Owed::Due,
        mode: purlis_core::reopen::Mode::Task,
        depth: 2,
        root: Some(root.to_owned()),
        above: None,
        by_person: false,
        long: false,
    };
    let task = Chat {
        from: Some(lineage.clone()),
        label: Some("check the queue".to_owned()),
        ..chat("/bin/sh", "ide.7", None)
    };
    let first = chats.start(&task, SIZE).unwrap();
    // Closed first, or not: either way the start in its place is that chat.
    for close_first in [true, false] {
        let old = if close_first {
            chats.close(first).unwrap();
            first
        } else {
            chats.start(&task, SIZE).unwrap()
        };
        let again = chats
            .start_ready_instead_of(old, &chat("/bin/sh", "ide.7", None), &a_shell_ready(), SIZE)
            .unwrap();
        assert_eq!(
            chats.handed_from(again),
            Some(lineage.clone()),
            "{close_first}"
        );
        assert_eq!(
            chats
                .recorded_chat(again)
                .and_then(|chat| chat.label)
                .as_deref(),
            Some("check the queue")
        );
        let _ = chats.close(again);
    }
}

/// A Claude Code chat's start, on a stand-in program: a harness that takes a first message.
fn a_claude_ready() -> purlis_core::start::Ready {
    purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        ..a_shell_ready()
    }
}

#[test]
fn a_dispatched_chat_whose_conversation_was_lost_is_handed_its_brief_on_its_fresh_start() {
    // #1609: the start in its place has no conversation, and the brief was the whole of
    // what it was asked. It is read from the dispatch's record, last on its line.
    use purlis_core::dispatchrecord::{self, Asker, ChatRef, Mode, Opening, Place, Worker};
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), root.clone());
    let worker = chat("/bin/sh", "devops 2", None);
    let first = chats.start_ready(&worker, &a_claude_ready(), SIZE).unwrap();
    let id = chats.record().chats[0].identity.id.clone().expect("an id");
    const BRIEF: &str = "# Check prod\n\nSay which pods restart.\n";
    let record = dispatchrecord::open(
        &root,
        Opening {
            mode: Mode::Handoff,
            asker: Asker {
                chat: ChatRef {
                    chat: 1,
                    id: Some("01K6ASKER0000000000000000A".to_owned()),
                    name: "steward 1".to_owned(),
                    persona: Some("steward".to_owned()),
                },
                workspace: None,
                by_person: false,
                session_record: None,
            },
            persona: Some("devops".to_owned()),
            worker: Worker {
                chat: ChatRef {
                    chat: first,
                    id: Some(id.clone()),
                    name: "devops 2".to_owned(),
                    persona: Some("devops".to_owned()),
                },
                harness: Some("claude".to_owned()),
                profile: None,
                session_record: None,
            },
            task: None,
            place: Place {
                workspace: None,
                folder: None,
                worktree: None,
            },
            brief: BRIEF.to_owned(),
            report_owed: false,
        },
        chrono::Utc::now(),
    )
    .unwrap();
    chats.brief_was_sent(&record, BRIEF);

    let again = chats
        .start_ready_instead_of(first, &worker, &a_claude_ready(), SIZE)
        .unwrap();

    let told = host
        .openings()
        .pop()
        .and_then(|opening| opening.args.last().cloned())
        .unwrap_or_default();
    assert!(
        told.starts_with(&crate::rebrief::STAMP.replace("{asker}", "steward 1")),
        "{told:?}"
    );
    assert!(told.ends_with(&format!("\n\n{BRIEF}")), "{told:?}");
    // And a chat no dispatch started, in the same place, is told nothing.
    let other = chats.start_ready(&chat("/bin/sh", "mine", None), &a_claude_ready(), SIZE);
    let other = other.unwrap();
    let fresh = chats
        .start_ready_instead_of(other, &worker, &a_claude_ready(), SIZE)
        .unwrap();
    let args = host
        .openings()
        .pop()
        .map(|one| one.args)
        .unwrap_or_default();
    assert!(
        !args
            .iter()
            .any(|arg| arg.contains("started this chat again")),
        "{args:?}"
    );
    let _ = chats.close(again);
    let _ = chats.close(fresh);
}

#[test]
fn a_chat_started_again_before_the_window_closed_the_old_one_is_recorded_once() {
    // #856 review F3: the window's close and its fresh start are not ordered, so the start
    // takes the old chat out of what is recorded itself.
    let (chats, wrote) = recorded();
    let first = chats.start(&chat("/bin/sh", "ide.7", None), SIZE).unwrap();
    let was = chats.record().chats[0].identity.id.clone();

    let again = chats
        .start_ready_instead_of(
            first,
            &chat("/bin/sh", "ide.7", None),
            &a_shell_ready(),
            SIZE,
        )
        .unwrap();

    for record in lock(&wrote).iter() {
        let with_it = record
            .chats
            .iter()
            .filter(|one| one.identity.id == was)
            .count();
        assert!(with_it <= 1, "two chats under one id: {record:#?}");
    }
    assert_eq!(chats.record().chats.len(), 1);
    let _ = chats.close(first);
    let _ = chats.close(again);
}

#[test]
fn a_chat_recorded_before_ids_begins_a_reopen_run_even_with_no_conversation_to_resume() {
    // ADR 0066's migration: "Its first run after the upgrade has `cause: reopen`."
    let dir = tempfile::tempdir().unwrap();
    let mut chats = Chats::new();
    let begun = beginning(&mut chats);

    let open = chats.put_back_here(
        &Record {
            chats: vec![chat(&a_claude(dir.path()), "ide.7", None)],
            ..Record::default()
        },
        SIZE,
    );

    assert_eq!(open[0].how, Reopened::Fresh(Fresh::NoConversationRecorded));
    assert_eq!(lock(&begun)[0].3, Began::Reopen);
}

#[test]
fn a_run_the_host_moved_the_chat_onto_is_the_one_the_record_holds() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.follow_conversation(session, ID, Some(OLD_RUN));

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].identity.run.as_deref(), Some(OLD_RUN));
    let _ = chats.close(session);
}

// ----- the conversation a chat is in now (Q10) -----

#[test]
fn a_conversation_the_chat_s_harness_moved_to_is_the_one_the_record_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.follow_conversation(session, ID, None);

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].resume, Some(SessionId::new(ID).unwrap()));
    let _ = chats.close(session);
}

#[test]
fn following_the_conversation_the_record_already_has_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    let before = lock(&wrote).len();

    chats.follow_conversation(session, ID, None);

    assert_eq!(lock(&wrote).len(), before);
    let _ = chats.close(session);
}

#[test]
fn a_shell_tab_is_resumed_by_no_conversation_whatever_ran_in_it() {
    // A shell tab resumes nothing (`Fresh::NoResumeForThisProgram`), so an id in its record
    // would be a write that means nothing.
    let (chats, wrote) = recorded();
    let session = chats.start(&chat("/bin/sh", "shell", None), SIZE).unwrap();
    let before = lock(&wrote).len();

    chats.follow_conversation(session, ID, None);

    assert_eq!(lock(&wrote).len(), before);
    assert_eq!(chats.record().chats[0].resume, None);
    let _ = chats.close(session);
}

/// **A shell that ran `exit` has left, with its tab still open** (#4202): it is still a shell
/// tab at its root, but it runs nothing a sign-in could finish in. A program whose restart is
/// under way has not ended: the new run takes its place.
#[test]
fn a_shell_whose_program_ended_runs_no_shell_though_its_tab_is_open() {
    let (chats, _wrote) = recorded();
    let root = std::env::temp_dir();
    let session = chats
        .start(
            &Chat {
                cwd: Some(root.clone()),
                ..chat("/bin/sh", "shell", None)
            },
            SIZE,
        )
        .unwrap();
    assert!(chats.runs_a_shell_at(session, &root));

    chats.claim(session).expect("claimed");
    chats.its_program_ended(session);
    assert!(
        chats.runs_a_shell_at(session, &root),
        "a restart under way has not ended it"
    );
    lock(&chats.restarting).remove(&session);

    chats.its_program_ended(session);

    assert!(chats.is_shell_at(session, &root), "its tab is still open");
    assert!(!chats.runs_a_shell_at(session, &root));
    let _ = chats.close(session);
    assert!(!chats.runs_a_shell_at(session, &root));
}

#[test]
fn a_conversation_for_a_chat_that_is_not_open_invents_nothing() {
    let (chats, wrote) = recorded();

    chats.follow_conversation(7, ID, None);

    assert!(lock(&wrote).is_empty());
    assert_eq!(chats.record(), Record::default());
}

/// Chats of a plane that start with no sandbox: what is said of them is their conversations.
fn unsandboxed_chats_of(plane: &std::path::Path) -> Chats {
    let mut chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(Pretend::default()),
        plane.to_path_buf(),
    )
    .deciding_by(Box::new(|_, _, _| Ok((None, None, None))));
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    chats
}

/// A chat on Codex in `plane`, whose harness chooses its own conversation and names it by a hook.
fn a_codex_chat_in(plane: &std::path::Path) -> Chat {
    Chat {
        cwd: Some(plane.to_path_buf()),
        ..chat(&standing("codex"), "restarted", None)
    }
}

/// Each chat of `chats` names conversation `ID` as it starts, before it is listed as open.
fn each_names_its_conversation_as_it_starts(chats: &std::sync::Arc<Chats>) {
    let hears = std::sync::Arc::downgrade(chats);
    chats.when_one_starts(Box::new(move |session, _, _, _| {
        hears
            .upgrade()
            .expect("the chats")
            .follow_conversation(session, ID, Some(OLD_RUN));
    }));
}

/// **A harness can name its conversation before its start has returned** (#5170): its report is
/// heard while the start still reads the plane's skills, before the chat is listed as open. The
/// board adopts it and never tells it again, so a report dropped here left the chat with nothing
/// to resume for good, and "send it a message and restart it again" was not true of it.
#[test]
fn a_conversation_named_while_the_chat_is_starting_is_the_one_a_restart_resumes() {
    let plane = tempfile::tempdir().expect("a plane");
    let chats = std::sync::Arc::new(unsandboxed_chats_of(plane.path()));
    each_names_its_conversation_as_it_starts(&chats);

    let session = chats
        .start(&a_codex_chat_in(plane.path()), SIZE)
        .expect("starts");

    let record = chats.record();
    assert_eq!(record.chats[0].resume, Some(SessionId::new(ID).unwrap()));
    assert_eq!(record.chats[0].identity.run.as_deref(), Some(OLD_RUN));
    chats.ask_restart(session).expect("asked");
    let started = chats.restart(session, SIZE).expect("it has a conversation");
    let _ = chats.close(started);
    let _ = chats.close(session);
}

#[test]
fn a_conversation_named_while_a_shell_tab_is_starting_resumes_nothing() {
    let plane = tempfile::tempdir().expect("a plane");
    let chats = std::sync::Arc::new(unsandboxed_chats_of(plane.path()));
    each_names_its_conversation_as_it_starts(&chats);
    let shell = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "shell", None)
    };

    let session = chats.start(&shell, SIZE).expect("starts");

    assert_eq!(chats.record().chats[0].resume, None);
    let _ = chats.close(session);
}

/// A closed chat's last report comes late, and its number may be the next chat's.
#[test]
fn a_conversation_named_for_a_chat_that_has_closed_is_not_kept_for_a_later_start() {
    let plane = tempfile::tempdir().expect("a plane");
    let chats = unsandboxed_chats_of(plane.path());
    let first = chats
        .start(&a_codex_chat_in(plane.path()), SIZE)
        .expect("starts");
    chats.close(first).unwrap();

    chats.follow_conversation(first, ID, None);

    let next = Chat {
        number: Some(first),
        ..a_codex_chat_in(plane.path())
    };
    let again = chats.start(&next, SIZE).expect("starts");
    assert_eq!(chats.record().chats[0].resume, None);
    let _ = chats.close(again);
}

// ----- the model a chat's harness reported (#1021) -----

fn session_start(chat: u32, conversation: &str, model: &str) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event: purlis_core::state::Event::SessionStart,
        conversation: purlis_core::hookwire::Conversation::Named(conversation.to_owned()),
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail {
            model: purlis_core::state::Model::new(model),
            ..purlis_core::state::Detail::default()
        },
    }
}

#[test]
fn only_the_chats_own_harness_ending_its_turn_says_what_it_left_running_5747() {
    use purlis_core::hookwire::Conversation;
    let a_shell = purlis_core::ownwork::Said {
        shells: 1,
        lapses_in: Some(3600),
        ..purlis_core::ownwork::Said::default()
    };
    let own = purlis_core::hookwire::Report {
        chat: 7,
        event: purlis_core::state::Event::Stop,
        conversation: Conversation::Named(ID.to_owned()),
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail {
            own_work: a_shell,
            ..purlis_core::state::Detail::default()
        },
    };
    assert_eq!(its_own_word(&own, Some(ID)), Some(Heard::Ended(a_shell)));
    // What its own `Stop` said, a prompt or a notice notwithstanding, and nothing at all too.
    let nothing = purlis_core::hookwire::Report {
        detail: purlis_core::state::Detail::default(),
        ..own.clone()
    };
    assert_eq!(
        its_own_word(&nothing, Some(ID)),
        Some(Heard::Ended(purlis_core::ownwork::Said::default()))
    );
    // A harness nested in the chat, under another conversation, says nothing of the chat's.
    assert_eq!(its_own_word(&own, Some(OTHER)), None, "a nested harness's");
    assert_eq!(its_own_word(&own, None), None);
    let helper = purlis_core::hookwire::Report {
        agent: Some("helper".to_owned()),
        ..own.clone()
    };
    assert_eq!(its_own_word(&helper, Some(ID)), None, "a helper's");
    // A turn's start and its end on an error say whether a turn is under way (#5750); a fresh
    // session start says nothing, and `/clear` clears it all.
    let event = |event| purlis_core::hookwire::Report {
        event,
        ..own.clone()
    };
    assert_eq!(
        its_own_word(
            &event(purlis_core::state::Event::UserPromptSubmit),
            Some(ID)
        ),
        Some(Heard::Began)
    );
    assert_eq!(
        its_own_word(&event(purlis_core::state::Event::StopFailure), Some(ID)),
        Some(Heard::Failed)
    );
    assert_eq!(
        its_own_word(&event(purlis_core::state::Event::SessionStart), Some(ID)),
        None,
        "a session's start"
    );
    let cleared = purlis_core::hookwire::Report {
        event: purlis_core::state::Event::SessionStart,
        detail: purlis_core::state::Detail {
            started: purlis_core::state::Started::Cleared,
            ..purlis_core::state::Detail::default()
        },
        ..own
    };
    assert_eq!(its_own_word(&cleared, Some(ID)), Some(Heard::Cleared));
}

fn a_piece(id: &str) -> purlis_core::ownwork::pieces::Piece {
    purlis_core::ownwork::pieces::Piece::new(
        purlis_core::ownwork::pieces::Kind::Shell,
        id,
        "npm test",
    )
    .expect("a piece")
}

fn said_of_its_own(
    chat: u32,
    turn_output: purlis_core::hookwire::Output,
) -> purlis_core::hookwire::TurnOutput {
    purlis_core::hookwire::TurnOutput {
        chat,
        turn_output,
        speaker: purlis_core::hookwire::Speaker::default(),
    }
}

#[test]
fn a_piece_of_own_work_is_recorded_as_its_tool_starts_it_and_the_turn_s_end_corrects_it_5750() {
    use purlis_core::hookwire::Output;
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();

    // Written as the tool starts it, with no turn's end heard: a kill writes nothing more.
    chats.heard_own_output(&said_of_its_own(
        session,
        Output::OwnWorkStarted {
            piece: a_piece("b1"),
        },
    ));
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].own_pieces, vec![a_piece("b1")]);

    // A piece of no shape is never kept.
    let before = lock(&wrote).len();
    chats.heard_own_output(&said_of_its_own(
        session,
        Output::OwnWorkStarted {
            piece: purlis_core::ownwork::pieces::Piece {
                id: "../x".to_owned(),
                ..a_piece("b2")
            },
        },
    ));
    assert_eq!(lock(&wrote).len(), before, "nothing changed");

    // The turn's end is the authority: what it does not name has ended.
    chats.heard_own_output(&said_of_its_own(
        session,
        Output::OwnWorkInFlight {
            pieces: vec![a_piece("b3")],
        },
    ));
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].own_pieces, vec![a_piece("b3")]);

    // And the quit's own write keeps it.
    let mut at_quit = None;
    chats.write_last(|record| at_quit = Some(record.clone()));
    assert_eq!(
        at_quit.expect("written").chats[0].own_pieces,
        vec![a_piece("b3")]
    );
    let _ = chats.close(session);
}

#[test]
fn a_shell_or_monitor_its_tool_starts_has_its_host_note_the_chat_s_strays_and_nothing_else_does_5806()
 {
    use purlis_core::hookwire::Output;
    use purlis_core::ownwork::pieces::{Kind, Piece};
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let session = chats
        .start(&chat("/bin/sh", "c", None), SIZE)
        .expect("starts");
    let started = |kind, id: &str| {
        chats.heard_own_output(&said_of_its_own(
            session,
            Output::OwnWorkStarted {
                piece: Piece::new(kind, id, "npm test").expect("a piece"),
            },
        ));
    };

    started(Kind::Shell, "b1");
    started(Kind::Monitor, "m1");
    // A helper, a workflow or a wake-up is the harness's own, in its own process: nothing
    // of it leaves the harness's group.
    started(Kind::Subagent, "a1");
    started(Kind::WakeUp, "w1");

    let noted: Vec<String> = host
        .terminal()
        .into_iter()
        .filter(|said| said.starts_with("noted strays"))
        .collect();
    assert_eq!(
        noted,
        vec![
            format!("noted strays {session}"),
            format!("noted strays {session}")
        ]
    );
    chats.close(session).expect("closed");
}

#[test]
fn a_turn_under_way_is_recorded_from_its_prompt_to_its_end_and_clear_forgets_everything_5750() {
    use purlis_core::hookwire::{Conversation, Output, Report};
    use purlis_core::state::{Detail, Event, Started};
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    let its_own = |event, detail| Report {
        chat: session,
        event,
        conversation: Conversation::Named(ID.to_owned()),
        pid: None,
        agent: None,
        detail,
    };
    let last = || lock(&wrote).last().cloned().expect("a record was written");

    chats.heard_own_work(
        &its_own(Event::UserPromptSubmit, Detail::default()),
        Some(ID),
    );
    assert!(last().chats[0].mid_turn, "a turn is under way");
    chats.heard_own_work(&its_own(Event::Stop, Detail::default()), Some(ID));
    assert!(!last().chats[0].mid_turn, "the turn ended");

    // An interrupt fires no `Stop`: the chat sitting idle at its prompt ends the mark.
    chats.heard_own_work(
        &its_own(Event::UserPromptSubmit, Detail::default()),
        Some(ID),
    );
    chats.heard_own_work(
        &its_own(
            Event::Notification,
            Detail {
                idle: true,
                ..Detail::default()
            },
        ),
        Some(ID),
    );
    assert!(!last().chats[0].mid_turn, "it sits idle");

    // A wake-up whose time came fired, and began a turn: it is no longer on the record.
    chats.heard_own_output(&said_of_its_own(
        session,
        Output::OwnWorkStarted {
            piece: purlis_core::ownwork::pieces::Piece {
                due_at: Some(1),
                ..purlis_core::ownwork::pieces::Piece::new(
                    purlis_core::ownwork::pieces::Kind::WakeUp,
                    "w1",
                    "",
                )
                .expect("a piece")
            },
        },
    ));
    assert_eq!(last().chats[0].own_pieces.len(), 1);
    chats.heard_own_work(
        &its_own(Event::UserPromptSubmit, Detail::default()),
        Some(ID),
    );
    assert!(last().chats[0].own_pieces.is_empty(), "it fired");
    chats.heard_own_work(&its_own(Event::Stop, Detail::default()), Some(ID));

    // `/clear` begins a conversation with nothing of the old one's on the record.
    chats.heard_own_work(
        &its_own(Event::UserPromptSubmit, Detail::default()),
        Some(ID),
    );
    chats.heard_own_output(&said_of_its_own(
        session,
        Output::OwnWorkStarted {
            piece: a_piece("b1"),
        },
    ));
    chats.holds_own_work(
        session,
        Some(purlis_core::ownwork::OwnWork {
            shells: 1,
            ..purlis_core::ownwork::OwnWork::default()
        }),
    );
    chats.heard_own_work(
        &its_own(
            Event::SessionStart,
            Detail {
                started: Started::Cleared,
                ..Detail::default()
            },
        ),
        Some(ID),
    );
    let cleared = last();
    assert_eq!(
        (
            cleared.chats[0].own_work,
            cleared.chats[0].own_pieces.len(),
            cleared.chats[0].mid_turn
        ),
        (None, 0, false)
    );
    let _ = chats.close(session);
}

#[test]
fn only_the_chats_own_harness_starting_names_its_model() {
    use purlis_core::hookwire::Conversation;
    let own = session_start(7, ID, "claude-opus-4-1");
    assert_eq!(its_own_model(&own, Some(ID)), Some("claude-opus-4-1"));
    // A conversation the board does not hold for the chat: a nested harness's.
    assert_eq!(its_own_model(&own, Some(OTHER)), None);
    assert_eq!(its_own_model(&own, None), None);
    for conversation in [
        Conversation::Contradicted,
        Conversation::Foreign,
        Conversation::Unknown,
    ] {
        let odd = purlis_core::hookwire::Report {
            conversation,
            ..own.clone()
        };
        assert_eq!(its_own_model(&odd, Some(ID)), None);
    }
    let sub = purlis_core::hookwire::Report {
        agent: Some("a1".to_owned()),
        ..own.clone()
    };
    assert_eq!(its_own_model(&sub, Some(ID)), None, "a sub-agent's");
    let stop = purlis_core::hookwire::Report {
        event: purlis_core::state::Event::Stop,
        ..own.clone()
    };
    assert_eq!(its_own_model(&stop, Some(ID)), None, "only a SessionStart");
}

#[test]
fn the_model_a_chats_harness_reported_is_recorded_while_its_program_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    assert_eq!(chats.record().chats[0].model, None, "nothing said yet");

    chats.heard_model(&session_start(session, ID, "claude-opus-4-1"), Some(ID));
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].model.as_deref(), Some("claude-opus-4-1"));

    // The same model again writes nothing; a nested harness's changes nothing.
    let before = lock(&wrote).len();
    chats.heard_model(&session_start(session, ID, "claude-opus-4-1"), Some(ID));
    chats.heard_model(&session_start(session, OTHER, "elsewhere"), Some(ID));
    assert_eq!(lock(&wrote).len(), before);

    // The last write of all names no running program, and so no model.
    let mut last = None;
    chats.write_last(|record| last = Some(record.clone()));
    assert_eq!(last.expect("written").chats[0].model, None);
    let _ = chats.close(session);
}

#[test]
fn what_a_chat_left_running_of_its_own_is_recorded_as_it_changes_and_kept_at_the_quit_5747() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    assert_eq!(chats.record().chats[0].own_work, None, "nothing said yet");
    let a_shell = purlis_core::ownwork::OwnWork {
        shells: 1,
        until: Some(1_700_003_600),
        ..purlis_core::ownwork::OwnWork::default()
    };

    // Written as the turn's end is heard, not only at a quit: a crash writes nothing.
    chats.holds_own_work(session, Some(a_shell));
    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].own_work, Some(a_shell));

    // The same again writes nothing.
    let before = lock(&wrote).len();
    chats.holds_own_work(session, Some(a_shell));
    assert_eq!(lock(&wrote).len(), before);

    // The quit's own write keeps it, for the next launch to tell the chat.
    let mut at_quit = None;
    chats.write_last(|record| at_quit = Some(record.clone()));
    assert_eq!(at_quit.expect("written").chats[0].own_work, Some(a_shell));
    let _ = chats.close(session);
}

#[test]
fn a_turn_s_end_that_says_nothing_runs_clears_what_the_record_held_5747() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    chats.holds_own_work(
        session,
        Some(purlis_core::ownwork::OwnWork {
            monitors: 1,
            ..purlis_core::ownwork::OwnWork::default()
        }),
    );

    chats.holds_own_work(session, None);

    let last = lock(&wrote).last().cloned().expect("a record was written");
    assert_eq!(last.chats[0].own_work, None);
    let _ = chats.close(session);
}

#[test]
fn a_model_reported_for_an_earlier_program_never_names_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", Some(ID)), SIZE)
        .unwrap();
    chats.heard_model(&session_start(session, ID, "claude-opus-4-1"), Some(ID));
    let _ = chats.close(session);

    let again = chats
        .start(
            &Chat {
                number: Some(session),
                // As a record written while the first one ran would carry it.
                model: Some("claude-opus-4-1".to_owned()),
                ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
            },
            SIZE,
        )
        .unwrap();
    assert_eq!(chats.record().chats[0].model, None);
    let _ = chats.close(again);
}

// ----- the name the operator gave a chat (charter-app#254) -----

#[test]
fn a_name_given_to_a_chat_is_written_into_the_record_and_said_on_it() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();

    let held = chats
        .rename(session, "  billing bug ")
        .expect("the chat is open");

    assert_eq!(held.as_deref(), Some("billing bug"));
    assert_eq!(
        chats.record().chats[0].label.as_deref(),
        Some("billing bug")
    );
    assert_eq!(chats.open_now()[0].label.as_deref(), Some("billing bug"));
    let _ = chats.close(session);
}

#[test]
fn a_rename_is_charters_label_and_leaves_the_harness_name_alone() {
    // The harness was started with `--name 3` and is resumed under it: a running harness
    // is never disturbed by a rename, and a split still starts its chat as `3`.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();

    chats.rename(session, "billing bug").unwrap();

    assert_eq!(chats.record().chats[0].name, "3");
    assert_eq!(chats.open_now()[0].name, "3");
    let _ = chats.close(session);
}

#[test]
fn a_blank_name_takes_the_given_one_off() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();
    chats.rename(session, "billing bug").unwrap();

    let held = chats.rename(session, "   ").unwrap();

    assert_eq!(held, None);
    assert_eq!(chats.record().chats[0].label, None);
    let _ = chats.close(session);
}

#[test]
fn a_name_charter_refuses_changes_nothing_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();
    chats.rename(session, "billing bug").unwrap();

    let refused = chats.rename(session, "pay\u{202e}lanigiro").unwrap_err();

    assert!(refused.contains("invisible"), "{refused}");
    assert_eq!(
        chats.record().chats[0].label.as_deref(),
        Some("billing bug")
    );
    let _ = chats.close(session);
}

#[test]
fn a_chat_charter_does_not_have_open_cannot_be_renamed() {
    let chats = Chats::new();

    assert!(chats.rename(7, "billing bug").is_err());
    assert_eq!(chats.record(), Record::default());
}

#[test]
fn renaming_to_the_name_it_already_has_writes_nothing() {
    // `pin`'s reason: every write is a fingerprint the machine store has to vouch for.
    let dir = tempfile::tempdir().unwrap();
    let written = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = std::sync::Arc::clone(&written);
    let chats = Chats::recorded_by(Box::new(move |_| {
        counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();
    chats.rename(session, "billing bug").unwrap();
    let after_one = written.load(std::sync::atomic::Ordering::SeqCst);

    chats.rename(session, " billing bug").unwrap();

    assert_eq!(written.load(std::sync::atomic::Ordering::SeqCst), after_one);
    let _ = chats.close(session);
}

#[test]
fn a_chat_put_back_keeps_the_name_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let back = chats.put_back_here(
        &Record {
            views: Vec::new(),
            chats: vec![Chat {
                label: Some("billing bug".into()),
                ..chat(&a_claude(dir.path()), "3", None)
            }],
            dealt: 0,
            relaunch_after_update: false,
            clone_seat: None,
            layout: None,
            focus: None,
        },
        SIZE,
    );

    assert_eq!(back[0].label.as_deref(), Some("billing bug"));
    assert_eq!(
        chats.record().chats[0].label.as_deref(),
        Some("billing bug")
    );
    chats.end_all();
}

#[test]
fn ending_every_chat_leaves_nothing_to_record() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();

    chats.end_all();

    let record = chats.record();
    assert_eq!(record.chats, vec![]);
    assert_eq!(chats.sessions().running(), Vec::<u32>::new());
    // The counter is NOT reset with them. A chat that ends does not give its number
    // back: `.charter/sessions/1.workspace` outlives it by 30 days, and a later chat
    // dealt 1 again would read it (charter-app#90).
    assert_eq!(record.dealt, 1);
}
#[test]
fn the_record_keeps_the_profile_persona_and_footer_a_chat_was_started_on() {
    // `record()` rebuilds each chat with `..chat.clone()`, so these ride along — which
    // means nothing says so when they stop. A struct literal that names one field and
    // spreads the rest is exactly where a later edit drops one silently, and an edit
    // that added `profile: None` beside the spread would do it: the record would still
    // be written, still be read, and every chat would come back as a shell.
    //
    // The footer choice (ADR 0029) rides the same spread and fails the same way:
    // it would be dropped at the quit and the chat would come back blanked.
    let chats = Chats::new();
    let chat = Chat {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "sleep 30".to_owned()],
        cwd: None,
        name: "ide.7".to_owned(),
        resume: None,
        active: false,
        profile: Some("claude-work".to_owned()),
        persona: Some("steward".to_owned()),
        show_footer: true,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    };

    let session = chats
        .start(
            &chat,
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .unwrap();
    let record = chats.record();

    assert_eq!(record.chats.len(), 1);
    assert_eq!(record.chats[0].profile.as_deref(), Some("claude-work"));
    assert_eq!(record.chats[0].persona.as_deref(), Some("steward"));
    assert!(record.chats[0].show_footer);
    let _ = chats.close(session);
}
#[test]
fn the_sidebar_is_told_the_harness_a_chat_was_started_as_not_one_read_off_its_program() {
    // A profile's command is commonly a WRAPPER (ADR 0022), and `Harness::of_command`
    // answers `None` for one — the same answer it gives a shell, deliberately. The board
    // is told the profile's declared kind at the start; the sidebar used to ask the
    // program's name instead and say "no harness" for the very same chat. A scenario
    // test caught the disagreement; this is what keeps them one answer.
    let chats = Chats::new();
    let chat = Chat {
        program: "/bin/sh".to_owned(),
        args: Vec::new(),
        cwd: None,
        name: "ide.7".to_owned(),
        resume: None,
        active: false,
        profile: Some("claude-work".to_owned()),
        persona: None,
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    };
    assert_eq!(
        chat.harness(),
        None,
        "the premise: the program is not a harness"
    );
    let ready = purlis_core::start::Ready {
        program: "/bin/sh".to_owned(),
        command: vec!["-c".to_owned(), "sleep 30".to_owned()],
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
        harness: Some(Harness::ClaudeCode),
        declared: None,
        session: None,
        how: purlis_core::reopen::Reopened::Fresh(
            purlis_core::reopen::Fresh::NoConversationRecorded,
        ),
        plugins: purlis_core::harness_plugin::Chosen::new(),
        sandbox: None,
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: None,
        first_message: None,
    };

    let session = chats
        .start_ready(
            &chat,
            &ready,
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .unwrap();

    let open = chats.open_now();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].harness, Some(Harness::ClaudeCode));
    assert_eq!(open[0].profile.as_deref(), Some("claude-work"));
    let _ = chats.close(session);
}
#[test]
fn a_cloud_profiles_chat_is_handed_its_values_at_start_and_purlis_records_them_nowhere() {
    // #2036, as ruled on 2026-10-15 (#2151): a cloud's credentials reach its chat's
    // program at start, read from a vault its persona may use, and nothing purlis records
    // or shows holds them. (Which start reaches here is `profilesignin`'s to decide.)
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = purlis_core::secrets::Ctx::new(&root, purlis_core::secrets::Env::of(&[]));
    let file = ctx.vaults_dir().join("aws.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, r#"{"id": "the-aws-id-for-this-test"}"#).unwrap();
    std::fs::create_dir_all(ctx.local_registry().parent().unwrap()).unwrap();
    std::fs::write(
        ctx.local_registry(),
        serde_json::json!({"vaults": {"aws": {
            "provider": "plain-file",
            "persona": "dev",
            "config": {"file": file.to_string_lossy()},
        }}})
        .to_string(),
    )
    .unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), root.clone());
    let ready = purlis_core::start::Ready {
        env: vec![(
            purlis_core::active::PERSONA_ENV.to_owned(),
            "dev".to_owned(),
        )],
        sign_in: Some(purlis_core::profilesignin::AtStart {
            kind: purlis_core::harness_declaration::SignInKind::Cloud,
            billed_to: "your cloud account · team".to_owned(),
            credentials: vec![(
                "AWS_ACCESS_KEY_ID".to_owned(),
                purlis_core::profilesignin::Reference::parse("vault:aws/id").unwrap(),
            )],
        }),
        ..a_claude_ready()
    };

    chats
        .start_ready(&chat("/bin/sh", "dev 1", None), &ready, SIZE)
        .expect("it starts");

    let opening = host.openings().pop().expect("opened");
    assert_eq!(opening.handed.names(), ["AWS_ACCESS_KEY_ID"]);
    assert_eq!(
        purlis_core::profilesignin::handed_onto(Vec::new(), &opening.handed),
        [(
            std::ffi::OsString::from("AWS_ACCESS_KEY_ID"),
            std::ffi::OsString::from("the-aws-id-for-this-test")
        )],
        "the program is handed the value"
    );
    assert!(
        !format!("{opening:?}").contains("the-aws-id-for-this-test"),
        "nothing that shows the opening holds it"
    );
    assert!(
        !format!("{:?}", chats.record()).contains("the-aws-id-for-this-test"),
        "nor the chat's record"
    );
}

#[test]
fn a_chat_whose_profile_names_a_key_in_a_vault_its_persona_may_not_read_starts_nothing() {
    // #2036: a profile never widens who reads a vault. The read is made before anything
    // is opened, so a refused one leaves no session, no tab and no value anywhere.
    let chats = Chats::new();
    let chat = Chat {
        program: "/bin/sh".to_owned(),
        name: "ide.7".to_owned(),
        profile: Some("work".to_owned()),
        ..Default::default()
    };
    let ready = purlis_core::start::Ready {
        program: "/bin/sh".to_owned(),
        command: vec!["-c".to_owned(), "sleep 30".to_owned()],
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
        harness: Some(Harness::ClaudeCode),
        declared: None,
        session: None,
        how: purlis_core::reopen::Reopened::Fresh(
            purlis_core::reopen::Fresh::NoConversationRecorded,
        ),
        plugins: purlis_core::harness_plugin::Chosen::new(),
        sandbox: None,
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: Some(purlis_core::profilesignin::AtStart {
            kind: purlis_core::harness_declaration::SignInKind::ApiKey,
            billed_to: "your key · work".to_owned(),
            credentials: vec![(
                "ANTHROPIC_API_KEY".to_owned(),
                purlis_core::profilesignin::Reference::parse("vault:providers/anthropic").unwrap(),
            )],
        }),
        first_message: None,
    };

    let refused = chats
        .start_ready(
            &chat,
            &ready,
            Size {
                columns: 80,
                rows: 24,
            },
        )
        .unwrap_err();

    assert!(refused.contains("providers"), "{refused}");
    assert!(chats.open_now().is_empty(), "nothing was started");
}
#[test]
fn the_board_is_told_the_harness_the_profile_declared_before_the_program_starts() {
    // The board judges every report against the harness it was told at the start, and a
    // chat on a wrapper profile would otherwise be told `None` — the narrowest rule
    // there is — while the sidebar said Claude Code. One announcement, one answer.
    //
    // Before the program starts, because a harness fires `SessionStart` at its own exec
    // and a board that learned the chat's number afterwards would miss it.
    let told = std::sync::Arc::new(Mutex::new(
        Vec::<(u32, Option<Harness>, Option<String>)>::new(),
    ));
    let chats = Chats::new();
    {
        let told = std::sync::Arc::clone(&told);
        chats.when_one_starts(Box::new(move |session, harness, conversation, _| {
            lock(&told).push((session, harness, conversation));
        }));
    }
    let chat = Chat {
        program: "/bin/sh".to_owned(),
        args: Vec::new(),
        cwd: None,
        name: "ide.7".to_owned(),
        resume: None,
        active: false,
        profile: Some("claude-work".to_owned()),
        persona: Some("steward".to_owned()),
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    };
    assert_eq!(
        chat.harness(),
        None,
        "the premise: the program is not a harness"
    );
    let ready = purlis_core::start::Ready {
        program: "/bin/sh".to_owned(),
        command: vec!["-c".to_owned(), "sleep 30".to_owned()],
        args: Vec::new(),
        env: Vec::new(),
        cwd: None,
        harness: Some(Harness::ClaudeCode),
        declared: None,
        session: purlis_core::harness::SessionId::new(ID).ok(),
        how: purlis_core::reopen::Reopened::Fresh(
            purlis_core::reopen::Fresh::NoConversationRecorded,
        ),
        plugins: purlis_core::harness_plugin::Chosen::new(),
        sandbox: None,
        unsandboxed: None,
        floorless: None,
        notices: Vec::new(),
        agents_md: Vec::new(),
        sign_in: None,
        first_message: None,
    };

    let session = chats.start_ready(&chat, &ready, SIZE).unwrap();

    let told = lock(&told).clone();
    assert_eq!(
        told,
        vec![(session, Some(Harness::ClaudeCode), Some(ID.to_owned()))],
        "the board was told something other than the profile's declared kind"
    );
    let _ = chats.close(session);
}

/// The words a profile chat's program was started with, one to an element, once the
/// profile's command is `command` (its first word replaced by a stand-in that writes its
/// arguments down) and the app arms it with a plugin.
fn argv_of_a_profile_chat(command: &[&str]) -> (Vec<String>, String) {
    argv_of_a_chat_in(command, "", |_| String::new())
}

/// The same, in a plane whose `charter.toml` is `shared`, with `profile(root)` written
/// under the profile's table in `charter.local.toml`.
fn argv_of_a_chat_in(
    command: &[&str],
    shared: &str,
    profile: impl Fn(&std::path::Path) -> String,
) -> (Vec<String>, String) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    std::fs::create_dir_all(&root).expect("the plane");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), shared).expect("charter.toml");
    let argv = root.join("argv");
    let running = root.join("running");
    // Beside the plane, not in it, and answering as Claude Code: the floor starts no program
    // the chat could have written, and asks the one it starts what it is (#2071).
    let program = stand_in::claude_code(
        dir.path(),
        command[0],
        &format!(
            "#!/bin/sh\n: > {running:?}\n\
                 for a in \"$@\"; do printf '%s\\n' \"$a\"; done > {argv:?}.part\n\
                 mv {argv:?}.part {argv:?}\n"
        ),
    );
    let mut words = vec![format!("{:?}", program.display().to_string())];
    words.extend(command[1..].iter().map(|w| format!("{w:?}")));
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [{}]\n{}",
            words.join(", "),
            profile(&root)
        ),
    )
    .expect("the profile");
    let set = purlis_core::profiles::current(&root);
    purlis_core::profiletrust::record_launched(
        &root,
        "work",
        &purlis_core::profiletrust::fingerprint(set.get("work").expect("it reads")),
    )
    .expect("approved");

    let plugin = crate::Shipped::default()
        .plugin
        .expect("the stand-in plugin");
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: Some(plugin.clone()),
        shims: None,
        git_hooks: None,
    });
    // On Linux a sandboxed Claude Code chat runs inside purlis's wrap, on a sandbox-only profile
    // (ADR 0067 as amended 2026-11-10), which `work` is not: these tests are about its words.
    no_claude_sandbox_for(&chats, &root);
    let ready = purlis_core::start::ready(
        &purlis_core::start::Start {
            profile: Some("work".to_owned()),
            persona: None,
            name: "ide.7".to_owned(),
            cwd: Some(root.clone()),
            resume: None,
            show_footer: false,
            resuming: None,
            without_sandbox: None,
            held: None,
            grants: Default::default(),
            made: purlis_core::sandbox::Made::Window,
        },
        &root,
    )
    .expect("the chat starts");
    let chat = Chat {
        program: ready.program.clone(),
        args: Vec::new(),
        cwd: ready.cwd.clone(),
        name: "ide.7".to_owned(),
        resume: ready.session.clone(),
        active: false,
        profile: Some("work".to_owned()),
        persona: None,
        show_footer: false,
        pinned: false,
        number: None,
        label: None,
        from: None,
        renamed_from: None,
        ..Default::default()
    };
    let session = chats.start_ready(&chat, &ready, SIZE).expect("it runs");
    // The deadline that matters counts from the stand-in running (#1138).
    let spawned = std::time::Instant::now();
    while !running.exists() && spawned.elapsed() < TO_START {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let ran = std::time::Instant::now();
    while running.exists() && !argv.exists() && ran.elapsed() < ONCE_RUNNING {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = chats.close(session);
    assert!(
        running.exists(),
        "the stand-in did not start in {TO_START:?}"
    );
    let said = std::fs::read_to_string(&argv).expect("the stand-in ran");
    (
        said.lines().map(str::to_owned).collect(),
        plugin.display().to_string(),
    )
}

#[test]
fn a_wrapper_profile_keeps_its_own_words_first_and_the_apps_come_after_them() {
    // M8.3: a profile's command is commonly a WRAPPER whose first argument is its own
    // subcommand (`ccs work`). The app's flags used to go straight after argv[0], which
    // started `ccs --plugin-dir … --settings … work` and broke the wrapper.
    let (argv, plugin) = argv_of_a_profile_chat(&["ccs", "work"]);
    assert_eq!(argv.first().map(String::as_str), Some("work"), "{argv:?}");
    assert_eq!(argv[1..3], ["--plugin-dir".to_owned(), plugin], "{argv:?}");
    assert_eq!(argv[3], "--mcp-config", "{argv:?}");
    assert_eq!(argv[5], "--settings", "{argv:?}");
    // The app's own session words come after the flags, where they always were.
    assert_eq!(argv[7], "--session-id", "{argv:?}");
    assert_eq!(argv[9..], ["--name", "ide.7"], "{argv:?}");
}

#[test]
fn a_claude_code_chat_runs_with_purlis_s_pins_alone_whatever_the_old_keys_say() {
    // #3467: the words the program actually received. The project's old key turns one of the
    // account's installed plugins off, and it is ignored: every plugin but purlis's is left to
    // Claude Code, and the pins ride as they always have.
    let (argv, _) = argv_of_a_chat_in(
        &["claude"],
        "[harness_plugins.claude]\n\"figma@official\" = false\n",
        |root| {
            let config = root.join("claude-config");
            std::fs::create_dir_all(config.join("plugins")).expect("the config dir");
            std::fs::write(
                config.join("plugins/installed_plugins.json"),
                r#"{"version": 2, "plugins": {"figma@official": [{"scope": "user"}],
                        "serena@official": [{"scope": "user"}]}}"#,
            )
            .expect("the install record");
            format!(
                "env = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
                config.display().to_string()
            )
        },
    );
    let at = argv
        .iter()
        .position(|word| word == "--settings")
        .expect("--settings");
    // On the line where it fits one word, else in the file the line names (SK-9): a Claude Code
    // chat's on macOS passes that length.
    let settings: serde_json::Value = if argv[at + 1].starts_with('{') {
        serde_json::from_str(&argv[at + 1]).expect("JSON")
    } else {
        purlis_core::hoststart::settings_from_its_file(std::path::Path::new(&argv[at + 1]))
            .expect("the settings file purlis wrote")
    };
    assert_eq!(
        settings["enabledPlugins"],
        serde_json::json!({
            "purlis@inline": true,
            "charter@charter": false,
            "charter@inline": false,
            "charter-app@inline": false,
            "charter@charter-app": false,
        }),
        "{argv:?}"
    );
}

#[test]
fn a_plain_profile_is_started_exactly_as_before() {
    let (argv, plugin) = argv_of_a_profile_chat(&["claude"]);
    assert_eq!(argv[..2], ["--plugin-dir".to_owned(), plugin], "{argv:?}");
    assert_eq!(argv[2], "--mcp-config", "{argv:?}");
    assert_eq!(argv[4], "--settings", "{argv:?}");
    assert_eq!(argv[6], "--session-id", "{argv:?}");
    assert_eq!(argv[8..], ["--name", "ide.7"], "{argv:?}");
}

// --- charter's git hooks in a chat (SQ-16) ----------------------------------------------- //

fn armed_with_git_hooks() -> Chats {
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        git_hooks: Some(purlis_core::githooks::GitHooks::at("/app/data/git-hooks")),
        ..crate::Shipped::default()
    });
    chats
}

#[test]
fn a_harness_chat_commits_through_charters_git_hooks() {
    assert_eq!(
        armed_with_git_hooks().git_hooks_for(Some(Harness::ClaudeCode)),
        Some(purlis_core::githooks::GitHooks::at("/app/data/git-hooks"))
    );
}

#[test]
fn a_shell_tab_and_an_app_without_git_hooks_arm_nothing() {
    assert_eq!(armed_with_git_hooks().git_hooks_for(None), None);
    assert_eq!(Chats::new().git_hooks_for(Some(Harness::Codex)), None);
}

// --- a shell tab's shims (SI-5, ADR 0062) ---------------------------------------------- //

fn armed_with_shims() -> Chats {
    let mut chats = Chats::new();
    chats.arming_with(crate::Shipped {
        binary: None,
        plugin: None,
        shims: Some(purlis_core::shellguard::Shims::at("/app/data/shims")),
        git_hooks: None,
    });
    chats
}

fn path_of(env: &[(String, String)]) -> Option<&str> {
    env.iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.as_str())
}

#[test]
fn a_shell_tab_finds_charters_shims_first_on_its_path() {
    let chats = armed_with_shims();

    let (args, env) = chats.shell_start(&chat("/bin/sh", "shell", None), "/bin/sh", vec![]);

    assert!(args.is_empty(), "{args:?}");
    let path = path_of(&env).expect("a PATH");
    assert!(path.starts_with("/app/data/shims/bin:"), "{path}");
}

#[test]
fn a_zsh_shell_tab_is_pointed_at_charters_start_files_and_keeps_its_own_arguments() {
    let chats = armed_with_shims();
    let mut shell = chat("/bin/zsh", "shell", None);
    shell.args = vec!["-l".to_owned()];

    let (args, env) = chats.shell_start(&shell, "/bin/zsh", shell.args.clone());

    assert_eq!(args, ["-l"]);
    assert!(
        env.contains(&("ZDOTDIR".to_owned(), "/app/data/shims/zsh".to_owned())),
        "{env:?}"
    );
}

#[test]
fn a_harness_chat_never_gets_the_shims() {
    let chats = armed_with_shims();

    let (args, env) = chats.shell_start(&chat("claude", "1", None), "claude", vec![]);

    assert!(args.is_empty());
    assert!(env.is_empty(), "{env:?}");
}

#[test]
fn a_chat_on_a_profile_never_gets_the_shims() {
    let chats = armed_with_shims();
    let mut on_a_profile = chat("/usr/local/bin/wrapper", "1", None);
    on_a_profile.profile = Some("work".to_owned());

    let (_, env) = chats.shell_start(&on_a_profile, "/usr/local/bin/wrapper", vec![]);

    assert!(env.is_empty(), "{env:?}");
}

#[test]
fn a_shell_tab_in_an_app_with_no_shims_is_a_plain_shell() {
    let chats = Chats::new();

    let (args, env) = chats.shell_start(&chat("/bin/zsh", "shell", None), "/bin/zsh", vec![]);

    assert!(args.is_empty());
    assert!(env.is_empty(), "{env:?}");
}

// --- the session host is a seam (FD-3) ------------------------------------------- //

#[test]
fn a_chat_runs_on_whichever_session_host_the_chats_were_given() {
    let host = Pretend::default();
    host.already_dealt(6);
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());

    let session = chats
        .start(
            &chat("/nowhere/a-program-nothing-runs", "hosted", None),
            SIZE,
        )
        .expect("the host opens it");

    assert_eq!(session, 7, "the number is the host's to deal");
    assert_eq!(
        host.asked(),
        vec![(7, "/nowhere/a-program-nothing-runs".to_owned())]
    );
    assert_eq!(
        chats
            .open_now()
            .iter()
            .map(|one| one.session)
            .collect::<Vec<_>>(),
        vec![7]
    );
    assert_eq!(chats.record().dealt, 7);

    chats.close(session).expect("the host ends it");

    assert_eq!(host.running(), Vec::<u32>::new());
    assert!(chats.open_now().is_empty());
}

/// What the core's start answers for a Claude Code chat on the profile `work` as `steward`,
/// in a new conversation, told `first` where it is told something.
fn a_harness_ready(first: Option<&str>) -> purlis_core::start::Ready {
    let conversation = "0f6c2a1e-aaaa-4bbb-8ccc-123456789abc";
    let ready = purlis_core::start::Ready {
        program: "claude".to_owned(),
        args: vec!["--session-id".to_owned(), conversation.to_owned()],
        env: vec![(
            purlis_core::active::PERSONA_ENV.to_owned(),
            "steward".to_owned(),
        )],
        harness: Some(Harness::ClaudeCode),
        session: Some(SessionId::new(conversation).expect("an id")),
        how: Reopened::Fresh(Fresh::NoConversationRecorded),
        ..a_shell_ready()
    };
    told_first(ready, first)
}

fn on_the_profile_work() -> Chat {
    let mut on_a_profile = chat("claude", "steward 1", None);
    on_a_profile.profile = Some("work".to_owned());
    on_a_profile.persona = Some("steward".to_owned());
    on_a_profile
}

/// **A harness chat on a runner is started by its names** (#2446): the runner composes it,
/// with the sandbox it compiles for itself, and the window's own launch of it is never sent.
/// Its proxy's asks are the window's to answer, as a local chat's are. A shell on the runner
/// is still started from the window's launch.
#[test]
fn a_harness_chat_on_a_runner_is_started_by_its_names_and_a_shell_by_the_window_s_launch() {
    let host = Pretend::default();
    host.on_a_runner();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());

    let session = chats
        .start_ready(
            &on_the_profile_work(),
            &a_harness_ready(Some("carry on with the plan")),
            SIZE,
        )
        .expect("it starts on the runner");

    let named = host.named();
    assert_eq!(named.len(), 1, "{named:?}");
    let (number, named, asks) = &named[0];
    assert_eq!(*number, session);
    assert_eq!(named.profile.as_deref(), Some("work"));
    assert_eq!(named.persona.as_deref(), Some("steward"));
    assert_eq!(
        named.first_message.as_deref(),
        Some("carry on with the plan")
    );
    assert_eq!(named.resume, None, "a new chat carries on no conversation");
    assert!(!named.without_sandbox);
    assert!(*asks, "its proxy asks the window");
    let identity = chats.record().chats[0].identity.clone();
    assert_eq!(named.chat, identity.id);
    assert_eq!(named.run, identity.run);
    assert!(host.asked().is_empty(), "the window's launch was not sent");

    let shell = chats
        .start(&chat("/bin/sh", "shell", None), SIZE)
        .expect("a shell starts");
    assert_eq!(host.asked(), vec![(shell, "/bin/sh".to_owned())]);
    assert_eq!(host.named().len(), 1);
}

/// What a chat's start asked of its profile on a pool, and what it answers (#2049).
#[derive(Default)]
struct ProfilesOnAPool {
    asked: Mutex<Vec<(PathBuf, String, String)>>,
    refusing: Mutex<Option<String>>,
    /// Why it refuses from its second asking on: the row settled since the first.
    refusing_after_one: Mutex<Option<String>>,
}

impl crate::poolprofiles::ProfilesOnPools for ProfilesOnAPool {
    fn ready(&self, root: &Path, pool: &str, profile: &str) -> Result<(), String> {
        lock(&self.asked).push((root.to_path_buf(), pool.to_owned(), profile.to_owned()));
        if lock(&self.asked).len() > 1
            && let Some(why) = lock(&self.refusing_after_one).clone()
        {
            return Err(why);
        }
        match lock(&self.refusing).clone() {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }
}

/// What a runner's host answers a start with where it finds the profile signed out (#4233).
const THE_RUNNERS_REFUSAL: &str = "the profile 'work' is not signed in on this computer. Nothing \
     was started. It waits in the Inbox as Sign in work, or run purlis profile sign-in work in \
     your own terminal; then start the chat again.";

/// **A start that reaches a runner while its profile is signed out there says the pool's words**
/// (#4233): let through while the runner fetched, it is refused by the runner, and the row, asked
/// again, says it and raises the pool's wait.
#[test]
fn a_runners_sign_in_refusal_is_said_in_the_pools_words() {
    let host = Pretend::default();
    host.on_pool("builds");
    host.refuses(Some(THE_RUNNERS_REFUSAL));
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    chats.readying_profiles_on_pools(on_pools.clone());
    let refused = |on_pools: &ProfilesOnAPool, chats: &mut Chats| {
        let said = chats.start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE);
        assert_eq!(lock(&on_pools.asked).len() % 2, 0, "asked before and after");
        said.unwrap_err()
    };

    // The row has not settled even now: the pool's words all the same.
    let said = refused(&on_pools, &mut chats);
    assert!(said.contains("is not signed in on pool builds"), "{said}");
    assert!(said.contains("Sign in on pool builds"), "{said}");
    assert!(!said.contains("own terminal"), "{said}");

    // Its row says so by now, on the second asking.
    lock(&on_pools.asked).clear();
    *lock(&on_pools.refusing_after_one) = Some("the pool's own wait".into());
    assert_eq!(refused(&on_pools, &mut chats), "the pool's own wait");
}

/// Any other refusal by a runner is as it said it (#4233).
#[test]
fn another_refusal_by_a_runner_is_as_it_said_it() {
    let host = Pretend::default();
    host.on_pool("builds");
    host.refuses(Some("the runner is out of disk"));
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    chats.readying_profiles_on_pools(on_pools.clone());

    let said = chats.start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE);

    assert_eq!(said, Err("the runner is out of disk".to_owned()));
    assert_eq!(lock(&on_pools.asked).len(), 1);
}

/// **A chat on a pool is started there only once its profile is ready on the pool's runner**
/// (#2049): signed in, it starts with nobody asked, as a moved chat does; refused, nothing is
/// sent to the runner and the start says why.
#[test]
fn a_runner_chat_starts_once_its_profile_is_ready_on_the_pool_and_nothing_starts_otherwise() {
    let host = Pretend::default();
    host.on_pool("builds");
    let on_pools = Arc::new(ProfilesOnAPool::default());
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    chats.readying_profiles_on_pools(on_pools.clone());

    chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    assert_eq!(
        *lock(&on_pools.asked),
        vec![(no_project(), "builds".to_owned(), "work".to_owned())]
    );
    assert_eq!(host.named().len(), 1);

    *lock(&on_pools.refusing) = Some("the profile 'work' is not signed in on pool builds".into());
    let refused = chats.start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE);
    assert_eq!(
        refused,
        Err("the profile 'work' is not signed in on pool builds".to_owned())
    );
    assert_eq!(host.named().len(), 1, "nothing more was sent to the runner");

    // A shell on the runner starts on no profile, and asks nothing of one.
    chats
        .start(&chat("/bin/sh", "shell", None), SIZE)
        .expect("a shell starts");
    assert_eq!(lock(&on_pools.asked).len(), 2);
}

/// #2692: a harness chat a runner composes by its names is judged by the runner host's record of
/// how it applied the sandbox, as the start answered it: one recorded off has its persona's
/// browser logins, as an unsandboxed local chat does; one recorded in the runner's sandbox, or
/// with no record, is held by the runner's sandbox, which this window keeps no compile of.
#[test]
fn a_composed_runner_chat_is_judged_by_the_runner_host_s_record_of_its_run() {
    use purlis_session_protocol::session::RunSandbox;
    let host = Pretend::default();
    host.on_a_runner();
    let project = tempfile::tempdir().expect("a project");
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        project.path().to_path_buf(),
    );
    let start_recorded_as = |ran: Option<RunSandbox>| {
        host.records_composed_runs_as(ran);
        chats
            .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
            .expect("it starts on the runner")
    };
    let off = start_recorded_as(Some(RunSandbox::Off));
    let sandboxed = start_recorded_as(Some(RunSandbox::By("seatbelt".to_owned())));
    let unread = start_recorded_as(None);

    let unconfined = |session| {
        chats
            .login_asker(project.path(), session)
            .map(|(_, free)| free)
    };
    assert_eq!(unconfined(off), Some(true), "the runner recorded it off");
    assert!(chats.started_unsandboxed(off));
    assert!(!chats.sandboxed_by_its_host(off));

    assert_eq!(
        unconfined(sandboxed),
        Some(false),
        "in the runner's sandbox"
    );
    assert!(chats.sandboxed_by_its_host(sandboxed));
    assert!(!chats.started_unsandboxed(sandboxed));

    assert_eq!(unconfined(unread), Some(false), "no record: sandboxed");
    assert!(chats.sandboxed_by_its_host(unread));

    // No compile of the runner's sandbox is held here, so no path that lets a sandboxed chat
    // reach more takes any of them as sandboxed.
    for session in [off, sandboxed, unread] {
        assert!(!chats.runs_sandboxed(session), "chat {session}");
    }
}

/// #2692: a runner's word that a run is off is taken only where this window's own reading of
/// the chat's start is off too. For a start this window compiled a sandbox for, it is no record:
/// the chat is held by the runner's sandbox, and nothing that lets a chat with no sandbox reach
/// more takes it as one.
#[test]
fn a_runners_off_for_a_start_this_window_compiled_a_sandbox_for_is_no_record() {
    use purlis_session_protocol::session::RunSandbox;
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.on_a_runner();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    host.records_composed_runs_as(Some(RunSandbox::Off));
    let session = chats
        .start_ready(
            &on_the_profile_work(),
            &purlis_core::start::Ready {
                sandbox: Some(a_claude_sandbox(plane.path())),
                ..a_harness_ready(None)
            },
            SIZE,
        )
        .expect("it starts on the runner");

    assert!(!chats.started_unsandboxed(session));
    assert!(chats.sandboxed_by_its_host(session));
    assert_eq!(
        chats
            .login_asker(plane.path(), session)
            .map(|(_, free)| free),
        Some(false)
    );
}

/// #2692: each path reads the record of how a run started in the direction that fails closed
/// for it. Where a sandboxed chat is allowed more (dispatching to another persona, a handoff's
/// crossing, a dispatch while you are away, the network record), only a run recorded in a
/// sandbox whose compile this window holds counts as sandboxed. Where a chat with no sandbox is
/// allowed more (a browser login, a browser's listing, a git action's reach, the vault Notice's
/// silence), only a run recorded off counts as unsandboxed.
#[test]
fn each_path_reads_the_run_record_in_the_direction_that_fails_closed_for_it() {
    use purlis_session_protocol::session::RunSandbox;
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let compiled = Some(purlis_core::sandbox::Confines::default());
    let by = || Some(RunSandbox::By("seatbelt".to_owned()));
    let cases = [
        // (compiled, ran, runs sandboxed, started unsandboxed)
        (compiled.clone(), by(), true, false),
        (compiled.clone(), None, false, false),
        (compiled.clone(), Some(RunSandbox::Off), false, true),
        // Reattached under a sandbox that changed while no window was there (D-2006-11).
        (None, by(), false, false),
        (None, None, false, false),
        (None, Some(RunSandbox::Off), false, true),
    ];
    for (session, (confines, ran, sandboxed, unsandboxed)) in (1u32..).zip(cases) {
        lock(&chats.open).insert(
            session,
            Running {
                chat: a_resumable_chat_in(plane.path()),
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: None,
                confines: confines.clone(),
                started_with: StartedWith::default(),
                ran: ran.clone(),
                composed_by_its_host: false,
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
        let case = format!("compiled {}, recorded {ran:?}", confines.is_some());
        assert_eq!(chats.runs_sandboxed(session), sandboxed, "{case}");
        assert_eq!(chats.started_unsandboxed(session), unsandboxed, "{case}");
        assert_eq!(
            chats
                .login_asker(plane.path(), session)
                .map(|(_, free)| free),
            Some(unsandboxed),
            "{case}"
        );
    }
    assert!(
        !chats.runs_sandboxed(99),
        "a chat this app does not have open"
    );
    assert!(!chats.started_unsandboxed(99));
}

/// A chat on a runner put back on its conversation carries that conversation on there, and
/// one the runner refuses is a start that did not start (#2446).
#[test]
fn a_runner_chat_put_back_carries_its_conversation_on_and_a_refusal_starts_nothing() {
    let host = Pretend::default();
    host.on_a_runner();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let conversation = "0f6c2a1e-aaaa-4bbb-8ccc-123456789abc";
    let resumed = purlis_core::start::Ready {
        args: vec!["--resume".to_owned(), conversation.to_owned()],
        how: Reopened::Resumed(SessionId::new(conversation).expect("an id")),
        ..a_harness_ready(None)
    };

    chats
        .start_ready(&on_the_profile_work(), &resumed, SIZE)
        .expect("it starts on the runner");

    assert_eq!(host.named()[0].1.resume.as_deref(), Some(conversation));
    assert_eq!(host.named()[0].1.first_message, None);

    host.refuses(Some("this runner refused it"));
    let refused = chats.start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE);
    assert_eq!(refused, Err("this runner refused it".to_owned()));
    assert_eq!(chats.open_now().len(), 1);
}

// ----- the sandbox's audit and count (ADR 0067 §7, ruling V78) -----

/// What each start told `when_the_sandbox_is_decided`, and when each run began, in one
/// list in the order they were said.
type Said = std::sync::Arc<Mutex<Vec<String>>>;

fn saying(chats: &mut Chats) -> Said {
    saying_or(chats, None)
}

/// [`saying`], with every trust event refused for `refused` when one is given: what the
/// app's callback answers when the event log cannot take it.
fn saying_or(chats: &mut Chats, refused: Option<&'static str>) -> Said {
    let said: Said = std::sync::Arc::default();
    let runs = std::sync::Arc::clone(&said);
    // A run held is said once its start goes on, as the event log writes it.
    // So is the trust event held with it, just before it.
    type Holding = HashMap<u32, (Began, Option<purlis_core::eventlog::HeldTrust>)>;
    let held: std::sync::Arc<Mutex<Holding>> = std::sync::Arc::default();
    let holding = std::sync::Arc::clone(&held);
    chats.when_a_run_begins(Box::new(move |session, _, cause, _, when| match when {
        RunSaid::Now => lock(&runs).push(format!("{session} run {}", cause.word())),
        RunSaid::Held(trust) => {
            lock(&holding).insert(session, (cause, trust));
        }
    }));
    let runs = std::sync::Arc::clone(&said);
    chats.when_a_held_start_is_settled(Box::new(move |session, how| {
        if let Some((cause, trust)) = lock(&held).remove(&session)
            && how == HeldStart::GoesOn
        {
            let mut runs = lock(&runs);
            if let Some(trust) = trust {
                let said = |key: &str| trust.body()[key].as_str().unwrap_or("-").to_owned();
                runs.push(format!(
                    "{} {} {}",
                    trust.kind(),
                    said("harness"),
                    said("persona")
                ));
            }
            runs.push(format!("{session} run {}", cause.word()));
        }
    }));
    let decided = std::sync::Arc::clone(&said);
    chats.when_the_sandbox_is_decided(Box::new(move |it| {
        let what = match (it.change, it.counted) {
            (Some(change), _) => {
                if let Some(why) = refused {
                    return Err(why.to_owned());
                }
                assert!(it.run.is_some(), "a trust event names the run it is under");
                change.kind().to_owned()
            }
            (None, Some(counted)) => format!("counted {counted:?}"),
            (None, None) => "nothing".to_owned(),
        };
        lock(&decided).push(format!(
            "{what} {} {}",
            it.harness.map_or("-", Harness::name),
            it.persona.unwrap_or("-")
        ));
        Ok(())
    }));
    said
}

fn a_person_lifted_it() -> purlis_core::sandbox::Lifted {
    purlis_core::sandbox::Lifted {
        by: purlis_core::sandbox::By::Person,
        reason: Some("needs the network".to_owned()),
    }
}

#[test]
fn a_run_with_no_sandbox_and_no_opt_out_says_so_in_the_record_and_an_opt_out_does_not() {
    let on_a_host = || Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    let chat = Chat {
        persona: Some("steward".to_owned()),
        ..chat("/bin/sh", "c", None)
    };
    let chats = on_a_host();
    let session = chats
        .start_ready(&chat, &a_shell_ready(), SIZE)
        .expect("starts");
    assert!(chats.record().chats[0].no_sandbox);
    assert!(!chats.record().chats[0].unsandboxed, "it is no opt-out");
    let _ = chats.close(session);

    let mut chats = on_a_host();
    let _said = saying(&mut chats);
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(a_person_lifted_it()),
        ..a_shell_ready()
    };
    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    assert!(!chats.record().chats[0].no_sandbox);
    assert!(chats.record().chats[0].unsandboxed);
    let _ = chats.close(session);
}

/// A runner's chat runs in the sandbox the runner compiles for it, though this window has none.
#[test]
fn a_harness_chat_on_a_runner_is_not_recorded_as_a_run_with_no_sandbox() {
    let host = Pretend::default();
    host.on_a_runner();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let session = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    assert!(!chats.record().chats[0].no_sandbox);
    assert!(!chats.record().chats[0].unsandboxed);
    let _ = chats.close(session);
}

#[test]
fn a_chat_started_without_the_sandbox_is_audited_off_as_its_run_begins_and_counted_once_up() {
    let mut chats = Chats::new();
    let said = saying(&mut chats);
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(a_person_lifted_it()),
        ..a_shell_ready()
    };
    let chat = Chat {
        persona: Some("steward".to_owned()),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");

    assert_eq!(
        *lock(&said),
        [
            "trust.sandbox.off claude steward".to_owned(),
            format!("{session} run start"),
            "counted OptedOut claude steward".to_owned(),
        ],
        "the record is written before the chat's program runs"
    );
    assert!(
        chats.record().chats[0].unsandboxed,
        "the record says what the run ran under"
    );
    let _ = chats.close(session);
}

#[test]
fn a_chat_whose_last_run_was_unsandboxed_is_audited_back_on_when_it_starts_sandboxed() {
    let plane = a_sandboxed_plane();
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    let said = saying(&mut chats);
    let ready = ready_under(Harness::ClaudeCode, a_claude_sandbox(plane.path()));
    let chat = Chat {
        unsandboxed: true,
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats
        .start_ready_as(&chat, &ready, SIZE, Why::Relaunch)
        .expect("starts");

    assert_eq!(
        *lock(&said),
        [
            "trust.sandbox.on claude -".to_owned(),
            format!("{session} run fresh"),
        ],
        "a relaunch is never counted as a new chat"
    );
    assert!(!chats.record().chats[0].unsandboxed);
    assert!(
        !chats.record().chats[0].no_sandbox,
        "it runs in the sandbox it compiled"
    );
}

#[test]
fn a_new_sandboxed_chat_is_counted_and_has_nothing_to_audit() {
    let plane = a_sandboxed_plane();
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    chats.arming_with(crate::Shipped {
        binary: crate::Shipped::default().binary,
        plugin: crate::Shipped::default().plugin,
        shims: None,
        git_hooks: None,
    });
    let said = saying(&mut chats);
    let ready = ready_under(Harness::ClaudeCode, a_claude_sandbox(plane.path()));

    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };

    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");

    assert_eq!(
        *lock(&said),
        [
            format!("{session} run start"),
            "counted Sandboxed claude -".to_owned(),
        ]
    );
}

#[test]
fn a_shell_says_nothing_about_a_sandbox() {
    let mut chats = Chats::new();
    let said = saying(&mut chats);

    let session = chats.start(&chat("/bin/sh", "shell", None), SIZE).unwrap();

    assert_eq!(*lock(&said), [format!("{session} run start")]);
    let _ = chats.close(session);
}

/// ADR 0067 §7: an opt-out is never unaudited. A person's is refused, and nothing runs,
/// when its trust event cannot be written.
#[test]
fn a_persons_opt_out_that_cannot_be_recorded_is_not_started() {
    let host = Pretend::default();
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let said = saying_or(&mut chats, Some("the disk is full"));
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(a_person_lifted_it()),
        ..a_shell_ready()
    };

    let refused = chats
        .start_ready(&chat("/bin/sh", "c", None), &ready, SIZE)
        .expect_err("not started");

    assert!(refused.contains("could not record"), "{refused}");
    assert!(refused.contains("the disk is full"), "{refused}");
    assert!(host.asked().is_empty(), "something ran");
    assert!(lock(&said).is_empty(), "{:?}", lock(&said));
    assert!(chats.record().chats.is_empty());
}

#[test]
fn a_persons_opt_out_with_no_event_log_at_all_is_not_started() {
    let host = Pretend::default();
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(a_person_lifted_it()),
        ..a_shell_ready()
    };

    let refused = chats
        .start_ready(&chat("/bin/sh", "c", None), &ready, SIZE)
        .expect_err("not started");

    assert!(refused.contains("event log is not open"), "{refused}");
    assert!(host.asked().is_empty(), "something ran");
}

/// A system with no backend starts every chat without the sandbox, so a missing record
/// cannot refuse them all: the chat starts, and its tab says the record is missing.
#[test]
fn a_windows_start_that_cannot_be_recorded_still_starts_and_its_tab_says_so() {
    let host = Pretend::default();
    let mut chats = Chats::on_host(Box::new(|_| {}), Box::new(host.clone()), no_project());
    let _said = saying_or(&mut chats, Some("the disk is full"));
    let ready = purlis_core::start::Ready {
        harness: Some(Harness::ClaudeCode),
        unsandboxed: Some(purlis_core::sandbox::Lifted {
            by: purlis_core::sandbox::By::NoBackend(purlis_core::sandbox::Os::Windows),
            reason: None,
        }),
        ..a_shell_ready()
    };

    let session = chats
        .start_ready(&chat("/bin/sh", "c", None), &ready, SIZE)
        .expect("starts");

    let notes = chats.start_notes(session);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("could not record"), "{notes:?}");
    assert!(notes[0].contains("the disk is full"), "{notes:?}");
    assert!(chats.start_notes(session).is_empty(), "said once");
}

/// #1699: an older Claude Code's sentence is said once per version in this process, counted
/// from the tab taking it, so a chat that never reached its tab does not use it up.
#[test]
fn an_older_claude_code_s_sentence_counts_as_said_once_its_tab_takes_it() {
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    // A version no other test names: the mark is the process's.
    let version = (2, 0, 1699);
    lock(&chats.late_notes).insert(4, vec!["older".to_owned()]);
    lock(&chats.older_in_notes).insert(4, version);
    assert!(!purlis_core::sandbox::older_was_shown(version));
    assert!(chats.start_notes(5).is_empty());
    assert!(
        !purlis_core::sandbox::older_was_shown(version),
        "another tab"
    );
    assert_eq!(chats.start_notes(4), ["older"]);
    assert!(purlis_core::sandbox::older_was_shown(version));
}

#[test]
fn only_a_shell_tab_at_the_project_root_is_a_place_to_type_a_sign_in() {
    let root = tempfile::tempdir().expect("a project");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let chats = Chats::on_host(Box::new(|_| {}), Box::new(Pretend::default()), no_project());
    let shell_at = |cwd: &std::path::Path| {
        chats
            .start(
                &Chat {
                    cwd: Some(cwd.to_path_buf()),
                    ..chat("/bin/sh", "shell", None)
                },
                SIZE,
            )
            .expect("starts")
    };
    let here = shell_at(root.path());
    let there = shell_at(elsewhere.path());
    let agent = chats
        .start_ready(
            &Chat {
                cwd: Some(root.path().to_path_buf()),
                ..chat("/bin/sh", "c", None)
            },
            &purlis_core::start::Ready {
                harness: Some(Harness::ClaudeCode),
                ..a_shell_ready()
            },
            SIZE,
        )
        .expect("starts");

    assert!(chats.is_shell_at(here, root.path()));
    assert!(!chats.is_shell_at(there, root.path()), "another directory");
    assert!(!chats.is_shell_at(agent, root.path()), "an agent's pane");
    assert!(!chats.is_shell_at(9999, root.path()), "no such session");
}

/// #2510: a runner's proxy's ask about a port of its loopback is raised and held as that
/// runner's service, by its port, never as a service on this Mac nor as a host; the Ask it
/// derives offers once and this chat alone, and Allow for you is refused.
#[test]
fn a_runners_ask_about_its_loopback_is_raised_and_held_as_the_runners_service() {
    use purlis_core::sandboxblock::Kind;
    for target in ["127.0.0.1:5432", "[::1]:5432", "localhost:5432"] {
        assert_eq!(
            asked_as(target.to_owned()),
            (Kind::RunnerService, "5432".to_owned()),
            "{target}"
        );
        let held = live_block(target).expect("held");
        assert_eq!(held.kind, "runner-service");
        assert_eq!(held.what, crate::sandboxing::GrantWhat::Port);
        assert_eq!(held.target, "5432");
        let ask = crate::asking::sandbox_block(
            4,
            &held,
            &purlis_core::sandbox::policy::Locks::none(),
            &Default::default(),
        )
        .expect("an ask");
        assert!(
            ask.says
                .starts_with("The runner this chat runs on refused its service at port 5432."),
            "{}",
            ask.says
        );
        assert_eq!(
            ask.options
                .iter()
                .map(|one| one.id.as_str())
                .collect::<Vec<_>>(),
            ["once", "chat", "keep"]
        );
    }
    assert_eq!(
        asked_as("api.example.com:443".to_owned()),
        (Kind::Host, "api.example.com:443".to_owned())
    );
}

/// A chat is taken for one started without the sandbox only where no sandbox was compiled
/// for it and none holds it (#2560); a sandboxed one never is, nor one the app does not
/// have open.
#[test]
fn only_a_chat_with_no_sandbox_at_all_runs_unconfined() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let decided = purlis_core::sandbox::decide_granted(
        Harness::ClaudeCode,
        plane.path(),
        &a_machine(),
        &|_| true,
        &purlis_core::sandbox::Person {
            opt_out: None,
            level_as: None,
            made: &purlis_core::sandbox::Made::Relaunch,
        },
        None,
        &purlis_core::sandbox::grant::Grants::default(),
    )
    .expect("compiles");
    let Some(purlis_core::sandbox::Decided::Sandboxed(applied)) = decided else {
        panic!("not sandboxed");
    };
    let open_with = |session: u32, confines: Option<purlis_core::sandbox::Confines>| {
        lock(&chats.open).insert(
            session,
            Running {
                chat: a_resumable_chat_in(plane.path()),
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: None,
                ran: Some(match confines {
                    Some(_) => {
                        purlis_session_protocol::session::RunSandbox::By("seatbelt".to_owned())
                    }
                    None => purlis_session_protocol::session::RunSandbox::Off,
                }),
                confines,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    };
    open_with(1, Some(applied.confines().clone()));
    open_with(2, None);
    let unconfined = |session| {
        chats
            .login_asker(plane.path(), session)
            .map(|(_, free)| free)
    };
    assert_eq!(unconfined(1), Some(false), "a sandboxed chat");
    assert_eq!(
        unconfined(2),
        Some(true),
        "a chat started without the sandbox"
    );
    assert_eq!(unconfined(3), None, "a chat this app does not have open");
}

/// A login is read for the persona a chat was started as, as every brokered read of a vault is
/// (#2560): never the project's default for a chat on no persona, nor the persona a handoff
/// left it holding.
#[test]
fn a_login_is_asked_for_by_the_persona_its_chat_was_started_as() {
    let plane = a_sandboxed_plane();
    std::fs::write(
        plane.path().join(purlis_core::plane::MANIFEST),
        "[persona]\ndefault = \"devops\"\n\n[sandbox]\nmode = \"on\"\n",
    )
    .expect("charter.toml");
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let open_as = |session: u32, persona: Option<&str>, held: Option<&str>| {
        let mut chat = a_resumable_chat_in(plane.path());
        chat.persona = persona.map(str::to_owned);
        chat.held = held.map(|held| purlis_core::reopen::HeldGrants {
            persona: Some(held.to_owned()),
        });
        lock(&chats.open).insert(
            session,
            Running {
                chat,
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: None,
                confines: None,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                ran: Some(purlis_session_protocol::session::RunSandbox::Off),
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    };
    open_as(1, None, None);
    open_as(2, Some("qa"), Some("devops"));
    let (on_none, unconfined) = chats.login_asker(plane.path(), 1).expect("open");
    assert_eq!(on_none.persona, None, "no default stands in");
    assert!(unconfined);
    let (handed, _) = chats.login_asker(plane.path(), 2).expect("open");
    assert_eq!(handed.persona.as_deref(), Some("qa"), "its own persona");
    assert!(chats.login_asker(plane.path(), 3).is_none());
}

/// #2691: one function names a chat's persona for a browser login, its browser's reach and its
/// Asks, read from the app's record of its start. After a handoff that left a chat on a profile
/// holding another's grants, a login's vault is its own persona's, as `secret exec` judges it,
/// and its browser is the held persona's, as its sandbox was compiled. A chat on a profile and
/// no persona uses no persona's vault, and its browser is the project's default persona's at its
/// start. A chat on no profile runs with no persona's grants. Each is fixed at the start.
#[test]
fn one_function_names_a_chats_persona_for_every_path_after_a_handoff_too() {
    let plane = tempfile::tempdir().expect("a project");
    for name in ["devops", "qa"] {
        let persona = plane.path().join("personas").join(name);
        std::fs::create_dir_all(&persona).expect("a persona");
        std::fs::write(
            persona.join("persona.md"),
            format!("---\nname: {name}\n---\n# {name}\n"),
        )
        .expect("its file");
    }
    let default_is = |persona: &str| {
        std::fs::write(
            plane.path().join(purlis_core::plane::MANIFEST),
            format!("[persona]\ndefault = \"{persona}\"\n"),
        )
        .expect("charter.toml");
    };
    default_is("devops");
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let start = |persona: Option<&str>, held: Option<&str>, profile: Option<&str>| {
        chats
            .start_ready(
                &Chat {
                    persona: persona.map(str::to_owned),
                    held: held.map(|held| purlis_core::reopen::HeldGrants {
                        persona: Some(held.to_owned()),
                    }),
                    profile: profile.map(str::to_owned),
                    cwd: Some(plane.path().to_path_buf()),
                    ..chat("claude", "a chat", None)
                },
                &a_harness_ready(None),
                SIZE,
            )
            .expect("it starts")
    };
    let handed = start(Some("qa"), Some("devops"), Some("work"));
    let on_none = start(None, None, Some("work"));
    let no_profile = start(Some("qa"), Some("devops"), None);
    let named = |started_as: Option<&str>, grants: Option<&str>| {
        Some(ChatPersona {
            started_as: started_as.map(str::to_owned),
            grants: grants.map(str::to_owned),
        })
    };
    assert_eq!(chats.persona_of(handed), named(Some("qa"), Some("devops")));
    assert_eq!(chats.persona_of(on_none), named(None, Some("devops")));
    assert_eq!(
        chats.persona_of(no_profile),
        named(Some("qa"), None),
        "a chat on no profile is compiled with no persona's grants"
    );
    // A login reads the same record for its vault: the persona it was started as.
    let login_as = |session| {
        chats
            .login_asker(plane.path(), session)
            .and_then(|(asker, _)| asker.persona)
    };
    assert_eq!(login_as(handed).as_deref(), Some("qa"));
    assert_eq!(login_as(on_none), None, "no default stands in for a vault");
    // Its fills and logins read the grants it was narrowed to, never a default or a profile's
    // fallback (the decision of 2026-11-06).
    assert_eq!(chats.secret_grants_of(handed).as_deref(), Some("devops"));
    assert_eq!(
        chats.secret_grants_of(no_profile).as_deref(),
        Some("devops")
    );
    assert_eq!(chats.secret_grants_of(on_none), None);
    let own = start(Some("qa"), None, Some("work"));
    assert_eq!(chats.secret_grants_of(own).as_deref(), Some("qa"));

    // Fixed at the start: its own grants allowed, or a new default, change it at the next.
    assert!(chats.allow_own_grants(handed));
    default_is("qa");
    assert_eq!(chats.persona_of(handed), named(Some("qa"), Some("devops")));
    assert_eq!(chats.persona_of(on_none), named(None, Some("devops")));
    assert_eq!(
        chats.persona_of(99),
        None,
        "a chat this app does not have open"
    );
}

/// A board of live asks that writes down what it was told (#2629), standing in for a chat's
/// proxy that asks live.
#[derive(Debug, Default)]
struct Told {
    allowed: Mutex<Vec<(String, purlis_core::sandbox::reach::By)>>,
    forgot: Mutex<Vec<(String, purlis_core::sandbox::reach::By)>>,
}

impl purlis_core::sandbox::Board for Told {
    fn asks_about(&self, _: &purlis_core::sandbox::hosts::Host) -> bool {
        false
    }

    fn allow(
        &self,
        host: &purlis_core::sandbox::hosts::Host,
        by: purlis_core::sandbox::reach::By,
    ) -> Vec<String> {
        lock(&self.allowed).push((host.to_string(), by));
        Vec::new()
    }

    /// As a chat's board does: an Allow it covers, or that covers it, ends, and it says so.
    fn keep_blocked(&self, host: &purlis_core::sandbox::hosts::Host) -> bool {
        let mut allowed = lock(&self.allowed);
        let before = allowed.len();
        allowed.retain(|(entry, _)| {
            purlis_core::sandbox::hosts::Host::parse(entry)
                .is_ok_and(|allowed| !host.covers(&allowed) && !allowed.covers(host))
        });
        allowed.len() != before
    }

    fn forget(
        &self,
        host: &purlis_core::sandbox::hosts::Host,
        by: purlis_core::sandbox::reach::By,
    ) {
        lock(&self.forgot).push((host.to_string(), by));
    }

    fn allowed_live(&self) -> Vec<(String, purlis_core::sandbox::reach::By)> {
        lock(&self.allowed).clone()
    }

    fn allow_left_out(&self, _: u16) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    fn keep_left_out_blocked(&self, _: u16) {}

    fn hold_open(&self, _: bool) {}

    fn give_up(&self, _: &str) -> bool {
        false
    }
}

/// What holds a [`Told`] board, as the session host holds a chat's proxy.
#[derive(Debug)]
struct HeldWith(Arc<Told>);

impl purlis_core::sandbox::HeldElsewhere for HeldWith {
    fn id(&self) -> &str {
        "told"
    }

    fn withdraw_port(&self, _: u16) -> Result<(), String> {
        Ok(())
    }

    fn board(&self) -> Option<Arc<dyn purlis_core::sandbox::Board>> {
        Some(Arc::clone(&self.0) as Arc<dyn purlis_core::sandbox::Board>)
    }
}

/// Chats 1 and 2 open in `plane`, each with its id and a proxy that asks live on a [`Told`]
/// board.
fn two_chats_asking_live(plane: &std::path::Path) -> (Chats, [Arc<Told>; 2]) {
    let chats = restartable_chats_of(plane, Pretend::default());
    let boards = [Arc::new(Told::default()), Arc::new(Told::default())];
    for (number, board) in (1..).zip(&boards) {
        let mut chat = a_resumable_chat_in(plane);
        chat.identity.id = Some(format!("chat-{number}"));
        let confinement = purlis_core::sandbox::Confinement::held_on_another_machine(
            purlis_core::sandbox::Placed {
                http: 1,
                socks: 2,
                tmp: None,
                routed: None,
                ssh: None,
                left_out: Vec::new(),
                host_name: None,
            },
            Box::new(HeldWith(Arc::clone(board))),
        );
        lock(&chats.open).insert(
            number,
            Running {
                chat,
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: Some(confinement),
                confines: None,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                ran: Some(purlis_session_protocol::session::RunSandbox::By(
                    "seatbelt".to_owned(),
                )),
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    }
    (chats, boards)
}

/// Chat `number`, a harness chat named `name` working in `cwd`, open in `chats` with
/// `confinement`.
fn open_a_harness_chat(
    chats: &Chats,
    number: u32,
    name: &str,
    cwd: &std::path::Path,
    confinement: Option<purlis_core::sandbox::Confinement>,
) {
    let mut chat = a_resumable_chat_in(cwd);
    chat.identity.id = Some(format!("chat-{number}"));
    chat.name = name.to_owned();
    lock(&chats.open).insert(
        number,
        Running {
            chat,
            how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
            harness: Some(Harness::ClaudeCode),
            declared: None,
            workspace: None,
            confinement,
            confines: None,
            started_with: StartedWith::default(),
            composed_by_its_host: false,
            ran: None,
            grants_as: None,
            skills: None,
            plane_skills: true,
            skills_read: Default::default(),
            last_prompt: None,
            runs_as: None,
            runners_word: None,
            reports_verified: false,
            clones_at_start: None,
            prompt_key: None,
        },
    );
}

/// #5427: **a branch worked on by a chat on a runner or a pool's runner says so as why it has no
/// turn**, where its runner records none, though its harness reports its turns: one too old to
/// (before session protocol 1.78). A chat here with that harness gives no reason, nor one whose
/// runner records its turns, and one in another folder is not asked about.
#[test]
fn a_chat_on_a_runner_or_a_pool_is_why_its_branch_has_no_turn() {
    use super::NoTurnsFrom;
    let plane = a_sandboxed_plane();
    let root = plane.path();
    let app = root.join("workspaces/ws/app");
    let piece = root.join("workspaces/ws/.worktrees/app/fix");
    for folder in [&app, &piece] {
        std::fs::create_dir_all(folder).expect("the chat's folder");
    }
    let host = Pretend::default();
    let chats = restartable_chats_of(root, host.clone());
    let why = |piece: Option<&str>| chats.no_turns_from_in(root, "ws", "app", piece);

    open_a_harness_chat(&chats, 1, "Here", &app, None);
    assert_eq!(why(None), None, "a harness here records its turns");

    let on_a_runner = purlis_core::sandbox::Confinement::held_on_another_machine(
        purlis_core::sandbox::Placed {
            http: 1,
            socks: 2,
            tmp: None,
            routed: None,
            ssh: None,
            left_out: Vec::new(),
            host_name: Some("build-box".to_owned()),
        },
        Box::new(HeldWith(Arc::new(Told::default()))),
    );
    open_a_harness_chat(&chats, 2, "Remote", &app, Some(on_a_runner));
    assert_eq!(
        why(None),
        Some(NoTurnsFrom::OnARunner {
            chat: "Remote".to_owned(),
            runner: "build-box".to_owned(),
        })
    );

    open_a_harness_chat(&chats, 3, "Pooled", &piece, None);
    assert_eq!(why(Some("fix")), None, "not on a pool yet");
    host.runs_on_pool(3, "builds");
    assert_eq!(
        why(Some("fix")),
        Some(NoTurnsFrom::OnAPool {
            chat: "Pooled".to_owned(),
            pool: "builds".to_owned(),
        })
    );
    // A runner of 1.78 records its turns there, and they come back with its code.
    host.records_turns_of_chat(3);
    assert_eq!(why(Some("fix")), None, "its runner records its turns");
}

fn hosts_of(told: &Mutex<Vec<(String, purlis_core::sandbox::reach::By)>>) -> Vec<String> {
    lock(told).iter().map(|(host, _)| host.clone()).collect()
}

/// #2629: **a one-chat Allow reaches only its chat**, applied live: "Allow once" and "Allow
/// for this chat" each reach the chat they were given for and no other open chat, and with no
/// chat named they change none. An Allow for every chat of the project reaches each.
#[test]
fn a_one_chat_allow_applied_live_reaches_only_its_chat() {
    use purlis_core::sandbox::grant::Level;
    let host = purlis_core::sandbox::grant::host("live.example.com").expect("a host");
    for level in [Level::Once, Level::Chat] {
        let plane = a_sandboxed_plane();
        let (chats, [one, two]) = two_chats_asking_live(plane.path());
        let live = chats.allow_live(Some(1), &host, level);
        assert_eq!(live.reached, [1], "{level:?}");
        assert_eq!(hosts_of(&one.allowed), ["live.example.com"], "{level:?}");
        assert!(hosts_of(&two.allowed).is_empty(), "{level:?}: not chat 2");

        let live = chats.allow_live(None, &host, level);
        assert!(live.reached.is_empty(), "{level:?}: no chat named");
        assert_eq!(hosts_of(&one.allowed).len(), 1, "{level:?}");
        assert!(hosts_of(&two.allowed).is_empty(), "{level:?}");
    }
    let plane = a_sandboxed_plane();
    let (chats, [one, two]) = two_chats_asking_live(plane.path());
    let mut reached = chats.allow_live(None, &host, Level::You).reached;
    reached.sort_unstable();
    assert_eq!(reached, [1, 2]);
    assert_eq!(hosts_of(&one.allowed), ["live.example.com"]);
    assert_eq!(hosts_of(&two.allowed), ["live.example.com"]);
}

/// #2629: **a one-chat Allow taken back reaches only its chat**: forgetting "Allow once" or
/// "Allow for this chat" live reaches the chat it was given for, and with no chat named
/// changes none, while one for every chat of the project is forgotten by each.
#[test]
fn a_one_chat_allow_forgotten_live_reaches_only_its_chat() {
    use purlis_core::sandbox::grant::Level;
    let host = purlis_core::sandbox::grant::host("live.example.com").expect("a host");
    for level in [Level::Once, Level::Chat] {
        let plane = a_sandboxed_plane();
        let (chats, [one, two]) = two_chats_asking_live(plane.path());
        let narrowed = Arc::new(Mutex::new(Vec::new()));
        chats.tell_narrowed_to({
            let narrowed = Arc::clone(&narrowed);
            Arc::new(move |session| lock(&narrowed).push(session))
        });
        chats.forget_live(Some("chat-1"), &host, level);
        assert_eq!(hosts_of(&one.forgot), ["live.example.com"], "{level:?}");
        assert!(hosts_of(&two.forgot).is_empty(), "{level:?}: not chat 2");
        assert_eq!(*lock(&narrowed), [1], "{level:?}");

        chats.forget_live(None, &host, level);
        assert_eq!(hosts_of(&one.forgot).len(), 1, "{level:?}: no chat named");
        assert!(hosts_of(&two.forgot).is_empty(), "{level:?}");
        assert_eq!(*lock(&narrowed), [1], "{level:?}");
    }
    let plane = a_sandboxed_plane();
    let (chats, [one, two]) = two_chats_asking_live(plane.path());
    chats.forget_live(None, &host, Level::You);
    assert_eq!(hosts_of(&one.forgot), ["live.example.com"]);
    assert_eq!(hosts_of(&two.forgot), ["live.example.com"]);
}

/// #3407: **Keep blocked ends the chat's own Allow of the host**: the Allow kept for the chat
/// goes, so its Granted list and its next start agree with its proxy, and its browsers end as
/// they do when an Allow is removed. Another chat's is left as it is.
#[test]
fn keep_blocked_ends_the_chats_own_allow_of_the_host_and_its_browsers() {
    use purlis_core::sandbox::grant::{Level, What};
    let host = purlis_core::sandbox::grant::host("live.example.com").expect("a host");
    let plane = a_sandboxed_plane();
    let (chats, [one, two]) = two_chats_asking_live(plane.path());
    chats.allow_live(Some(1), &host, Level::Chat);
    chats.allow_live(Some(2), &host, Level::Chat);
    chats
        .hold_grant(1, What::Host(host.clone()), 1)
        .expect("held");
    chats
        .hold_grant(2, What::Host(host.clone()), 1)
        .expect("held");
    let narrowed = Arc::new(Mutex::new(Vec::new()));
    chats.tell_narrowed_to({
        let narrowed = Arc::clone(&narrowed);
        Arc::new(move |session| lock(&narrowed).push(session))
    });

    let audited = Mutex::new(Vec::new());
    chats.keep_blocked_live(1, &host, &|chat, audit| {
        lock(&audited).push((
            chat,
            audit.granted,
            audit.what.to_owned(),
            audit.target.to_owned(),
        ));
        Ok(())
    });

    assert_eq!(
        *lock(&audited),
        [(
            Some(1),
            false,
            "host".to_owned(),
            "live.example.com".to_owned()
        )],
        "taken back as a Revoke is audited"
    );
    assert!(!chats.holds_for(1, &What::Host(host.clone())), "its own");
    assert!(
        chats.holds_for(2, &What::Host(host.clone())),
        "not chat 2's"
    );
    assert!(hosts_of(&one.allowed).is_empty(), "its proxy's");
    assert_eq!(hosts_of(&two.allowed), ["live.example.com"]);
    assert_eq!(*lock(&narrowed), [1]);
}

/// #3407: Keep blocked of a host the chat was never allowed ends none of its browsers.
#[test]
fn keep_blocked_of_a_host_never_allowed_ends_no_browser() {
    let plane = a_sandboxed_plane();
    let (chats, _boards) = two_chats_asking_live(plane.path());
    let narrowed = Arc::new(Mutex::new(Vec::new()));
    chats.tell_narrowed_to({
        let narrowed = Arc::clone(&narrowed);
        Arc::new(move |session| lock(&narrowed).push(session))
    });

    chats.keep_blocked_live(
        1,
        &purlis_core::sandbox::grant::host("other.example.com").expect("a host"),
        &|_, _| panic!("nothing to audit"),
    );

    assert!(lock(&narrowed).is_empty());
}

/// #3407: Keep blocked of a host the chat was allowed live alone, with nothing kept for it,
/// ends its browsers too.
#[test]
fn keep_blocked_of_a_host_allowed_live_alone_ends_the_chats_browsers() {
    use purlis_core::sandbox::grant::Level;
    let host = purlis_core::sandbox::grant::host("live.example.com").expect("a host");
    let plane = a_sandboxed_plane();
    let (chats, [one, _two]) = two_chats_asking_live(plane.path());
    chats.allow_live(None, &host, Level::Project);
    let narrowed = Arc::new(Mutex::new(Vec::new()));
    chats.tell_narrowed_to({
        let narrowed = Arc::clone(&narrowed);
        Arc::new(move |session| lock(&narrowed).push(session))
    });

    chats.keep_blocked_live(1, &host, &|_, _| panic!("nothing kept to audit"));

    assert!(hosts_of(&one.allowed).is_empty());
    assert_eq!(*lock(&narrowed), [1]);
}

/// #3407: an Allow for the chat reserved before a Keep blocked that takes the chat's Allow
/// back is not kept after it, as after a Revoke.
#[test]
fn an_allow_reserved_before_keep_blocked_takes_it_back_is_not_kept() {
    use purlis_core::sandbox::grant::What;
    let host = purlis_core::sandbox::grant::host("live.example.com").expect("a host");
    let plane = a_sandboxed_plane();
    let (chats, _boards) = two_chats_asking_live(plane.path());
    chats
        .hold_grant(1, What::Host(host.clone()), 1)
        .expect("held");
    let reserved = chats
        .reserve_grant(1, What::Host(host.clone()), 2, false)
        .expect("reserved");

    chats.keep_blocked_live(1, &host, &|_, _| Ok(()));

    assert!(chats.commit_grant(reserved).is_err());
    assert!(!chats.holds_for(1, &What::Host(host.clone())));
}

/// #2629: **a host is never allowed once**, from a chat's Notice: the Allow is refused before
/// anything is kept, and no open chat's proxy is told of it.
#[test]
fn allowing_a_host_once_from_a_notice_is_refused_and_reaches_no_chat() {
    let plane = a_sandboxed_plane();
    let (chats, [one, two]) = two_chats_asking_live(plane.path());
    let audited = Mutex::new(0);
    let refused = crate::sandboxing::judged_and_kept(
        plane.path(),
        &purlis_core::sandbox::Machine::this(),
        &chats,
        Some(1),
        (
            crate::sandboxing::GrantWhat::Host,
            "live.example.com",
            crate::sandboxing::GrantLevel::Once,
        ),
        &|_, _| {
            *lock(&audited) += 1;
            Ok(())
        },
        1,
    )
    .expect_err("refused")
    .to_string();
    assert_eq!(
        refused,
        "purlis allows a host or a folder for this chat, or for longer, not once."
    );
    assert_eq!(*lock(&audited), 0);
    assert!(hosts_of(&one.allowed).is_empty());
    assert!(hosts_of(&two.allowed).is_empty());
}

/// #2770: **a running chat whose skills changed is told so**, where its harness does not take the
/// change in by itself: a Claude Code chat for an edit to a skill's text, which a Codex chat reads
/// from where it lies, and both for a skill added. A chat on no harness is never told. One whose
/// start the plane's skills did not reach (the Skill studio's flag off, #3445) was stamped
/// empty, so with the flag on now it is offered the restart that hands it them.
#[test]
fn a_chat_whose_skills_changed_since_it_started_is_listed_for_a_restart() {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("a manifest");
    let skill = |name: &str, body: &str| {
        let folder = plane.path().join("skills").join(name);
        std::fs::create_dir_all(&folder).expect("a skill folder");
        std::fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n\n{body}\n"),
        )
        .expect("a skill");
    };
    skill("deploy", "one");
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    for (session, harness, plane_skills) in [
        (1, Some(Harness::ClaudeCode), true),
        (2, Some(Harness::Codex), true),
        (3, None, true),
        (4, Some(Harness::ClaudeCode), false),
    ] {
        lock(&chats.open).insert(
            session,
            Running {
                chat: a_resumable_chat_in(plane.path()),
                how: Reopened::Fresh(Fresh::NoConversationRecorded),
                harness,
                declared: None,
                workspace: None,
                confinement: None,
                confines: None,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                ran: None,
                grants_as: None,
                skills: harness.map(|harness| {
                    purlis_core::skills::stamp_now(&purlis_core::skills::Running {
                        plane: plane.path(),
                        workspace: None,
                        persona: None,
                        harness,
                        built_in: None,
                        plane_skills,
                        start: None,
                    })
                }),
                plane_skills,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    }
    let sessions = |chats: &Chats| -> Vec<u32> {
        chats
            .skills_changed_while_on()
            .into_iter()
            .map(|(session, _)| session)
            .collect()
    };
    assert_eq!(sessions(&chats), [4], "the flag is on now");
    // Use skill… is told what its start read, never a constant.
    assert!(chats.skills_of(1).expect("open").plane_skills);
    assert!(!chats.skills_of(4).expect("open").plane_skills);

    // A Codex chat reads the SKILL.md purlis wrote for it at its start (SK-8), so an edit to a
    // body waits for a restart there too.
    skill("deploy", "two");
    assert_eq!(sessions(&chats), [1, 2, 4]);

    skill("triage", "one");
    assert_eq!(sessions(&chats), [1, 2, 4]);
    // And its Skills view is told what changed, by name (#3694).
    let changed = chats.skills_changed_while_on();
    let (_, first) = changed
        .iter()
        .find(|(session, _)| *session == 1)
        .expect("1");
    assert_eq!(first.added, ["triage"]);
    assert_eq!(first.edited, ["deploy"]);
    assert!(first.gone.is_empty());
}

/// #4738: **a tab mount with no instruction change reads no skill folders**. Once the plane is
/// watched, each open chat's stamp is read once and kept: a skill edited on disk that the watch
/// has not told of is not seen, because nothing read the folder again. The watch's word that the
/// instructions changed refreshes every open chat's stamp.
#[test]
fn a_chat_skill_stamp_is_read_again_only_once_the_instructions_change() {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), "schema = 1\n").expect("a manifest");
    let skill = |name: &str, body: &str| {
        let folder = plane.path().join("skills").join(name);
        std::fs::create_dir_all(&folder).expect("a skill folder");
        std::fs::write(
            folder.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Does {name}.\n---\n\n{body}\n"),
        )
        .expect("a skill");
    };
    skill("deploy", "one");
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    chats.skills_now().watched(true);
    for session in [1, 2] {
        let harness = Harness::ClaudeCode;
        lock(&chats.open).insert(
            session,
            Running {
                chat: a_resumable_chat_in(plane.path()),
                how: Reopened::Fresh(Fresh::NoConversationRecorded),
                harness: Some(harness),
                declared: None,
                workspace: None,
                confinement: None,
                confines: None,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                ran: None,
                grants_as: None,
                skills: Some(purlis_core::skills::stamp_now(
                    &purlis_core::skills::Running {
                        plane: plane.path(),
                        workspace: None,
                        persona: None,
                        harness,
                        built_in: None,
                        plane_skills: true,
                        start: None,
                    },
                )),
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
    }
    let sessions = |chats: &Chats| -> Vec<u32> {
        chats
            .skills_changed_while_on()
            .into_iter()
            .map(|(session, _)| session)
            .collect()
    };
    assert!(sessions(&chats).is_empty(), "nothing has changed yet");

    // Written behind the watch's back: a mount asks again, and reads no skill folder.
    skill("deploy", "two");
    assert!(
        sessions(&chats).is_empty(),
        "a skill folder was read again with no instruction change"
    );

    // The watch says the instructions changed: every open chat's stamp is read again.
    chats.skills_now().forget();
    assert_eq!(sessions(&chats), [1, 2]);
}

/// #2019: a chat serving on a port is not told its sandbox is older once its own door listens
/// there: a compile then finds the port in use, which changes no grant.
#[test]
fn a_chat_serving_on_its_port_is_not_behind_once_its_door_listens() {
    use purlis_core::sandbox::{Confines, Widened};
    let started = Confines {
        widened: Widened {
            ports: vec![8080, 5173],
            listens: vec![5173],
            listen_only: vec![5173],
            ..Widened::default()
        },
        ..Confines::default()
    };
    let now = Confines {
        widened: Widened {
            ports: vec![8080],
            unserved: vec![5173],
            ..Widened::default()
        },
        ..Confines::default()
    };
    assert_eq!(Settled::of(&started, &[]), Settled::of(&now, &[]));
    // A grant taken back still is.
    let taken = Confines {
        widened: Widened {
            ports: vec![8080],
            ..Widened::default()
        },
        ..Confines::default()
    };
    assert_ne!(Settled::of(&started, &[]), Settled::of(&taken, &[]));
}

/// #2019: a chat serving on a port is owed a restart once the grant to serve there is taken
/// back, even where a grant to connect to the same port still stands; a chat that does not
/// serve there is owed none. First runs on CI: the chat's door listens on the port.
#[test]
fn a_chat_serving_on_a_port_is_owed_a_restart_once_its_grant_is_taken_back() {
    let plane = a_sandboxed_plane();
    let port = std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    purlis_core::sandbox::local::grant_listen(plane.path(), port).expect("kept");
    purlis_core::sandbox::local::grant_port(plane.path(), port).expect("kept");
    let (chats, _host) = armed_for_opencode(plane.path());
    let ready = ready_under(
        Harness::Opencode,
        a_sandbox_for(Harness::Opencode, plane.path()),
    );
    let chat = Chat {
        cwd: Some(plane.path().to_path_buf()),
        ..chat("/bin/sh", "c", None)
    };
    let session = chats.start_ready(&chat, &ready, SIZE).expect("starts");
    assert_eq!(
        chats.owe_restart_serving(None, port.wrapping_add(1), false),
        Vec::<u32>::new()
    );
    assert_eq!(chats.served_ports(session), vec![port]);
    assert_eq!(chats.owe_restart_serving(None, port, false), vec![session]);
    assert!(chats.owed_restarts().contains(&session));
    // Its door there is closed at once (#3316): nothing reaches its server from now on.
    assert_eq!(chats.served_ports(session), Vec::<u16>::new());
    chats.close(session).expect("closed");
}

#[test]
fn a_reattached_chat_keeps_the_studio_flag_its_start_read_3466() {
    let never = || -> bool { panic!("the flag was read again") };
    assert!(plane_skills_at_start(true, Some(true), never));
    assert!(!plane_skills_at_start(true, Some(false), never));
    // A record written before the flag was kept: read again, as before.
    assert!(plane_skills_at_start(true, None, || true));
    // A start that is not a reattach reads the flag now, whatever the record said.
    assert!(!plane_skills_at_start(false, Some(true), || false));
}

#[test]
fn the_flag_a_chat_started_with_is_what_its_record_keeps_3466() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, _wrote) = recorded();
    // A record put back from an earlier run says the opposite of what this start reads: a fresh
    // start reads the flag, and the record keeps that.
    let first = chats
        .start(&chat(&a_claude(dir.path()), "ide.6", None), SIZE)
        .unwrap();
    let now = lock(&chats.open)[&first].plane_skills;
    let _ = chats.close(first);
    let put_back = Chat {
        plane_skills: Some(!now),
        ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
    };
    let session = chats.start(&put_back, SIZE).unwrap();
    assert_eq!(lock(&chats.open)[&session].plane_skills, now);
    assert_eq!(chats.record().chats[0].plane_skills, Some(now));
    let _ = chats.close(session);
}

#[test]
fn a_reattached_chat_keeps_the_routing_its_start_kept_3578() {
    use purlis_core::skills::router::Kept;
    let kept = Kept::BuiltIn;
    assert_eq!(
        routing_kept_at_start(true, true, Some(&kept)),
        Some(kept.clone())
    );
    // A record written before the routing was kept: its router is asked again, as before.
    assert_eq!(routing_kept_at_start(true, true, None), None);
    // A start that is not a reattach is routed now.
    assert_eq!(routing_kept_at_start(false, true, Some(&kept)), None);
    // Nor is a routing kept without the studio flag its start took: it is routed again too.
    assert_eq!(routing_kept_at_start(true, false, Some(&kept)), None);
}

#[test]
fn the_routing_a_chat_started_with_is_what_its_record_keeps_3578() {
    use purlis_core::skills::router::Kept;
    let dir = tempfile::tempdir().unwrap();
    let (chats, _wrote) = recorded();
    // A record put back from an earlier run says it asked a router: a fresh start routes now,
    // here by the built-in router, and the record keeps that.
    let put_back = Chat {
        routing: Some(Kept::Asked(purlis_core::skills::router::Routed {
            record: purlis_core::skills::router::Record {
                routing: "01HZY3M6S4K9Q2W7E8R5T1Y6V0".to_owned(),
                router: "relevance".to_owned(),
                ms: 3,
                fell_back: None,
                error: None,
                hidden: Vec::new(),
                prompt: false,
                pointed: Vec::new(),
            },
            answer: Some(Vec::new()),
            left_out: Vec::new(),
            reads_prompts: false,
        })),
        ..chat(&a_claude(dir.path()), "ide.7", Some(ID))
    };
    let session = chats.start(&put_back, SIZE).unwrap();
    assert_eq!(chats.record().chats[0].routing, Some(Kept::BuiltIn));
    let _ = chats.close(session);
}

#[test]
fn a_runners_chat_is_armed_by_its_runner_at_its_start_and_at_its_reattach_3604() {
    let never = || -> bool { panic!("the host was asked") };
    assert!(armed_by_its_runner(true, false, never));
    assert!(armed_by_its_runner(false, true, || true));
    // A chat reattached from a host that composes none, and any start here, are armed here.
    assert!(!armed_by_its_runner(false, true, || false));
    assert!(!armed_by_its_runner(false, false, never));
}

/// #3604 (SK-15g): a runner's chat this window reattaches is armed by its runner, as its start
/// was, so the window reads none of its skills, asks no router, and its record keeps no routing,
/// though the project's skills reach every chat here. Seen red: it was read and routed here.
#[test]
fn a_runners_chat_reattached_is_shown_no_skills_of_this_window_s_reading_3604() {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(
        plane.path().join(purlis_core::plane::MANIFEST),
        "requires = [{ feature = \"project-skills\" }]\n",
    )
    .expect("charter.toml");
    let host = Pretend::default();
    host.on_a_runner();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());

    chats.put_back(
        &left_running_in(
            plane.path(),
            Chat {
                plane_skills: Some(true),
                ..a_resumable_chat_in(plane.path())
            },
        ),
        SIZE,
    );

    assert_eq!(host.reattached(), [3]);
    assert_eq!(
        lock(&chats.open)[&3].skills_read,
        purlis_core::hoststart::ChatSkills::default(),
        "its runner's to arm"
    );
    assert_eq!(chats.record().chats[0].routing, None);
    let _ = chats.close(3);
}

/// #3084 (SK-20b): a skill run a pool's runner says its chat started and ended is logged in this
/// project's skill log, as a local run is, with this window's word for where the chat ran; a line
/// of another pool's, of no pool, or of a chat not open here logs nothing.
#[test]
fn a_runner_chats_skill_run_is_logged_here_with_where_this_window_says_it_ran_3084() {
    use purlis_core::skilluse::relayed::{Line, Relayed};
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    let host = Pretend::default();
    host.on_a_runner();
    let mut chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    chats.on_device(Some(DEVICE.to_owned()));
    let session = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    host.runs_on_pool(session, "builds");
    let chat = lock(&chats.open)[&session]
        .chat
        .identity
        .id
        .clone()
        .expect("its id");
    let of = |chat: &str, run: &str, line: Line| Relayed {
        chat: chat.to_owned(),
        run: run.to_owned(),
        at_ms: 1_793_440_800_000,
        line,
    };
    let started = || Line::Started {
        skill: "review".to_owned(),
        started_by: purlis_core::skills::runs::StartedBy::SkillTool,
        level: None,
        tree: None,
    };
    let ended = Line::Ended {
        ended: purlis_core::skills::runs::Ended::Turn,
        ms: 5,
        signals: Default::default(),
        tokens: None,
        parent: None,
    };

    chats.skill_run_heard(
        Some("builds"),
        &of(&chat, "01K6RUNA00000000000000000A", started()),
    );
    chats.skill_run_heard(
        Some("builds"),
        &of(&chat, "01K6RUNA00000000000000000A", ended),
    );
    // Another pool's, no pool's, and a chat this window does not have.
    chats.skill_run_heard(
        Some("other"),
        &of(&chat, "01K6RUNB00000000000000000B", started()),
    );
    chats.skill_run_heard(None, &of(&chat, "01K6RUNC00000000000000000C", started()));
    chats.skill_run_heard(
        Some("builds"),
        &of(
            "01K6NOTHERE00000000000000D",
            "01K6RUND00000000000000000D",
            started(),
        ),
    );

    let runs = purlis_core::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:?}");
    let run = &runs[0];
    assert_eq!(run.id.as_deref(), Some("01K6RUNA00000000000000000A"));
    assert_eq!(run.persona.as_deref(), Some("steward"));
    assert_eq!(run.harness.as_deref(), Some(Harness::ClaudeCode.name()));
    assert_eq!(run.workspace, Some(None));
    assert!(run.end.is_some(), "its end");
    assert_eq!(run.device, DEVICE, "this machine's own file");
    let _ = chats.close(session);
}

#[test]
fn a_profile_a_studio_start_was_refused_on_says_which_line_needs_approving() {
    // #3942: the Skill studio's Approve and start reads the same approval a waiting chat's
    // Notice offers, by the profile's name, and none once it is approved.
    let dir = tempfile::tempdir().expect("a directory");
    let root = a_plane_with_work(dir.path(), "");

    let asked = NeedsApproval::of_profile("work", &root).expect("work asks");

    assert_eq!(asked.approval, "new");
    assert_eq!(
        asked.shown,
        purlis_core::profiletrust::shown(&root, &work_at(&root))
    );
    assert_eq!(NeedsApproval::of_profile("nobody", &root), None);
    purlis_core::profiletrust::approve(&root, &work_at(&root), &asked.shown).expect("approved");
    assert_eq!(NeedsApproval::of_profile("work", &root), None);
}

/// A Goose declared by the project, taking MCP servers or not (SK-30's fixture in the core).
fn goose(mcp: bool) -> purlis_core::harness_declaration::Declaration {
    purlis_core::harness_declaration::parse(
        &format!(
            "name = \"goose\"\ntitle = \"Goose\"\nprogram = \"goose\"\n\n[session]\n\n\
             [terminal]\nready_to_type = \"raw-and-quiet\"\n\n[levels]\n\n[capabilities]\n\
             mcp_servers = \"{}\"\n",
            if mcp { "yes" } else { "no: it takes none" }
        ),
        purlis_core::harness_declaration::Origin::Project,
        "harnesses/goose.toml",
    )
    .expect("a declaration")
}

/// #3585 (SK-30's follow-up): a chat on a declared harness no skill reaches is told so in one
/// line on its tab, once, at its first start, and only where the plane's skills would reach it.
/// A harness that takes MCP servers, one purlis ships an adapter for, and a start that is not
/// the chat's first say nothing.
#[test]
fn a_chat_on_a_harness_that_takes_no_skills_is_told_so_once_at_its_first_start_3585() {
    let without = goose(false);
    let said = not_supported_at_start(Why::New, true, None, Some(&without)).expect("said");
    assert_eq!(
        Some(said),
        purlis_core::skills::not_supported_on(&without).map(|one| one.said),
        "the studio's and the badge's own line"
    );
    // The plane's skills reach no chat here (the studio is off): there is nothing to miss.
    assert_eq!(
        not_supported_at_start(Why::New, false, None, Some(&without)),
        None
    );
    // Said at its first start alone: a relaunch, an upgrade's or a lost conversation's start
    // is the same chat, told already.
    for again in [Why::Relaunch, Why::Upgrade, Why::Again] {
        assert_eq!(
            not_supported_at_start(again, true, None, Some(&without)),
            None
        );
    }
    // A harness the fallback route reaches, one with an adapter, and a shell say nothing.
    assert_eq!(
        not_supported_at_start(Why::New, true, None, Some(&goose(true))),
        None
    );
    assert_eq!(
        not_supported_at_start(
            Why::New,
            true,
            Some(Harness::ClaudeCode),
            Some(Harness::ClaudeCode.declared())
        ),
        None
    );
    assert_eq!(not_supported_at_start(Why::New, true, None, None), None);
}

/// #3585, end to end: the line is among what the chat's start says on its tab, taken once.
#[test]
fn a_new_chat_on_goose_without_mcp_says_skills_are_not_supported_on_its_tab_3585() {
    let dir = tempfile::tempdir().unwrap();
    // The project requires its skills, so they reach every chat whatever this machine's flag.
    std::fs::write(
        dir.path().join(purlis_core::plane::MANIFEST),
        "requires = [{ feature = \"project-skills\" }]\n",
    )
    .unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        dir.path().to_path_buf(),
    );
    let on = |declared: Option<purlis_core::harness_declaration::Declaration>, name: &str| {
        chats
            .start_ready(
                &chat("/bin/sh", name, None),
                &purlis_core::start::Ready {
                    declared,
                    ..a_shell_ready()
                },
                SIZE,
            )
            .expect("the chat starts")
    };

    let without = on(Some(goose(false)), "goose 1");
    let with = on(Some(goose(true)), "goose 2");
    let shell = on(None, "a shell");

    let notes = chats.start_notes(without);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("Skills are not supported on Goose:"),
        "{notes:?}"
    );
    assert!(chats.start_notes(without).is_empty(), "said once");
    assert!(chats.start_notes(with).is_empty());
    assert!(chats.start_notes(shell).is_empty());
}

/// #3369: a harness chat this window composes is started with a prompt key of its own, handed
/// to its host with the start, and Use skill… digests under that key, never under the chat's id.
/// A shell gets none.
#[test]
fn a_harness_chat_this_window_composes_is_given_its_own_prompt_key_and_a_shell_none() {
    let dir = tempfile::tempdir().unwrap();
    let host = Pretend::default();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        dir.path().to_path_buf(),
    );
    let start = |harness: Option<Harness>| {
        chats
            .start_ready(
                &chat("/bin/sh", "a chat", None),
                &purlis_core::start::Ready {
                    harness,
                    ..a_shell_ready()
                },
                SIZE,
            )
            .expect("it starts")
    };

    let one = start(Some(Harness::Codex));
    let two = start(Some(Harness::Codex));
    let shell = start(None);

    let openings = host.openings();
    let key = openings[0]
        .prompt_key
        .clone()
        .expect("a harness chat's key");
    assert_eq!(
        chats.skills_of(one).and_then(|chat| chat.salt),
        Some(key.clone())
    );
    assert_ne!(
        openings[1].prompt_key,
        Some(key.clone()),
        "one key per chat"
    );
    assert_eq!(openings[2].prompt_key, None);
    assert_eq!(chats.skills_of(shell).and_then(|chat| chat.salt), None);
    let id = chats.record().chats[0].identity.id.clone().expect("an id");
    assert_ne!(key.expose(), id, "never the chat's id");
    assert!(
        !openings[0]
            .args
            .iter()
            .any(|word| word.contains(key.expose()))
    );
    // Not in the record the window keeps of its chats.
    let record = format!("{:?}", chats.record());
    assert!(!record.contains(key.expose()));
    let _ = two;
}

/// #3369: a chat its runner composes is digested under the key the runner answered.
#[test]
fn a_runner_chats_prompt_key_is_the_one_its_runner_answered() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.on_a_runner();
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    let session = chats
        .start_ready(
            &on_the_profile_work(),
            &purlis_core::start::Ready {
                sandbox: Some(a_claude_sandbox(plane.path())),
                ..a_harness_ready(None)
            },
            SIZE,
        )
        .expect("it starts on the runner");

    assert_eq!(
        chats.skills_of(session).and_then(|chat| chat.salt),
        purlis_core::skillspan::PromptSalt::of_key(&format!("composed-{session}"))
    );
}

/// #3369: a chat reattached after the window restarts is digested under the key its host kept.
#[test]
fn a_reattached_chats_prompt_key_is_the_one_its_host_kept() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());

    let open = chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );

    assert_eq!(host.reattached(), [3]);
    assert_eq!(open[0].session, 3);
    assert_eq!(
        chats.skills_of(3).and_then(|chat| chat.salt),
        purlis_core::skillspan::PromptSalt::of_key("left-3")
    );
}

/// A router extension whose question is held past the deadline where `slow`, and whose reading of
/// prompts is `reads`.
struct PromptRouter {
    slow: bool,
    reads: bool,
}

impl purlis_core::skills::router::AskRouter for PromptRouter {
    fn ask(
        &self,
        _: &str,
        _: &purlis_core::extension::studio::Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        if self.slow {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Ok(serde_json::Map::from_iter([(
            "pointed".to_owned(),
            serde_json::json!(["release"]),
        )]))
    }

    fn reads_prompts(&self, _: &str) -> bool {
        self.reads
    }
}

/// An open chat routed at its start by `relevance`, which reads prompts and has a skill to point at.
fn a_chat_its_router_reads_the_prompts_of(plane: &Path) -> (Chats, u32) {
    use purlis_core::skills::router::{Record, Routed};
    let host = Pretend::default();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane, host);
    chats.put_back(&left_running_in(plane, a_resumable_chat_in(plane)), SIZE);
    let session = 3;
    lock(&chats.open)
        .get_mut(&session)
        .expect("open")
        .skills_read
        .routed = Some(Arc::new(Routed {
        record: Record {
            routing: "01JROUTING0000000000000000".to_owned(),
            router: "relevance".to_owned(),
            ms: 34,
            fell_back: None,
            error: None,
            hidden: Vec::new(),
            prompt: false,
            pointed: Vec::new(),
        },
        answer: Some(Vec::new()),
        left_out: vec![("release".to_owned(), "Cuts a release.".to_owned())],
        reads_prompts: true,
    }));
    (chats, session)
}

/// #4297: what the Routing page and the chat's tab read of a prompt is kept with the chat, each
/// prompt's over the last's. Seen red: only the event log had it.
#[test]
fn a_chat_keeps_its_latest_prompt_s_routing_4297() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    assert_eq!(chats.routings()[0].last_prompt, None, "no prompt yet");

    chats.asks_routers_with(Arc::new(PromptRouter {
        slow: true,
        reads: true,
    }));
    assert_eq!(chats.pointed(session, "go"), None);
    let slow = chats.routings()[0].last_prompt.clone().expect("kept");
    assert!(slow.fell_back.is_some());
    assert_eq!(
        chats.skills_of(session).expect("open").last_prompt.as_ref(),
        Some(&slow),
        "its tab reads the same"
    );

    let routed = lock(&chats.open)[&session].skills_read.routed.clone();
    let routed = routed.expect("routed");
    let answered = purlis_core::skills::router::Record {
        prompt: true,
        ..routed.record.clone()
    };
    chats.keep_last_prompt(session, &routed, Some(&answered));
    assert_eq!(chats.routings()[0].last_prompt, Some(answered));
    let _ = chats.close(session);
}

/// Each chat `chats` says moved whether its latest prompt fell back, in order.
fn told_of_fallbacks(chats: &Chats) -> Arc<Mutex<Vec<u32>>> {
    let told = Arc::new(Mutex::new(Vec::new()));
    chats.when_a_prompt_s_fallback_moves(Box::new({
        let told = Arc::clone(&told);
        move |session| lock(&told).push(session)
    }));
    told
}

/// #4445: the window reads a chat again only as its latest prompt changes whether it fell back,
/// so a router that answers every prompt asks it to read nothing. Seen red: told of none.
#[test]
fn the_window_is_told_only_as_a_prompt_changes_whether_it_fell_back_4445() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    let told = told_of_fallbacks(&chats);
    let answers = |slow| {
        chats.asks_routers_with(Arc::new(PromptRouter { slow, reads: true }));
        chats.pointed(session, "go");
        lock(&told).len()
    };

    assert_eq!(answers(false), 0, "a first prompt answered");
    assert_eq!(
        answers(false),
        0,
        "an answered prompt after an answered prompt"
    );
    assert_eq!(answers(true), 1, "a prompt fell back");
    assert_eq!(answers(true), 1, "and another, whose mark is the same");
    assert_eq!(answers(false), 2, "a later prompt answered");
    assert_eq!(*lock(&told), vec![session, session]);
    let _ = chats.close(session);
}

/// #4445: a router that reads no further prompt takes the need away, so the mark must go too.
#[test]
fn a_router_that_stops_reading_prompts_after_a_fallback_tells_the_window_4445() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    let told = told_of_fallbacks(&chats);
    chats.asks_routers_with(Arc::new(PromptRouter {
        slow: true,
        reads: true,
    }));
    chats.pointed(session, "go");
    assert_eq!(lock(&told).len(), 1, "it fell back");

    chats.asks_routers_with(Arc::new(PromptRouter {
        slow: false,
        reads: false,
    }));
    chats.pointed(session, "go");
    assert_eq!(lock(&told).len(), 2, "its mark goes");
    chats.pointed(session, "go");
    assert_eq!(lock(&told).len(), 2, "nothing moved");
    let _ = chats.close(session);
}

/// #4445: an answer that lands after its chat was started again moved nothing the window reads.
#[test]
fn a_fallback_landing_after_its_chat_started_again_tells_nothing_4445() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    let told = told_of_fallbacks(&chats);
    let was = lock(&chats.open)[&session].skills_read.routed.clone();
    let was = was.expect("routed");
    let fell = purlis_core::skills::router::Record {
        fell_back: Some(purlis_core::skills::router::FellBack::Slow),
        ..was.record.clone()
    };
    chats.keep_last_prompt(session, &Arc::new((*was).clone()), Some(&fell));
    assert!(lock(&told).is_empty());
    let _ = chats.close(session);
}

/// #4297: a router turned off for prompts reads no further one, so what it did on its last says
/// nothing now, and the need it raised goes.
#[test]
fn a_router_that_reads_no_further_prompt_leaves_no_last_prompt_4297() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    chats.asks_routers_with(Arc::new(PromptRouter {
        slow: true,
        reads: true,
    }));
    chats.pointed(session, "go");
    assert!(chats.routings()[0].last_prompt.is_some());

    chats.asks_routers_with(Arc::new(PromptRouter {
        slow: false,
        reads: false,
    }));
    assert_eq!(chats.pointed(session, "go"), None);
    assert_eq!(chats.routings()[0].last_prompt, None);
    let _ = chats.close(session);
}

/// #4297: an answer that lands after its chat was started again is not that new start's.
#[test]
fn a_prompt_answered_after_its_chat_started_again_is_not_kept_by_the_new_start_4297() {
    let plane = tempfile::tempdir().expect("a plane");
    let (chats, session) = a_chat_its_router_reads_the_prompts_of(plane.path());
    let was = lock(&chats.open)[&session].skills_read.routed.clone();
    let was = was.expect("routed");
    chats.keep_last_prompt(session, &Arc::new((*was).clone()), Some(&was.record));
    assert_eq!(lock(&chats.open)[&session].last_prompt, None);
    let _ = chats.close(session);
}

#[test]
fn a_notification_is_not_lost_to_a_stale_layout_while_the_window_is_in_the_background() {
    // #4751: after a launch the record's layout stands until the window reports one, so it can
    // say a chat is on screen that nobody has drawn yet. What keeps the notification is the
    // window's own answer: `asknotify::looking` asks whether its window is on screen, focused
    // and showing the project (`window_in_front`), and holds a notification back only when that
    // and the chat on screen are both true. A stale layout is the second; a window in the
    // background is never the first.
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let record = Record {
        chats: vec![Chat {
            number: Some(3),
            ..chat(&a_claude(dir.path()), "ide.3", None)
        }],
        layout: Some(on_ops(vec![front_tab(Node::chat(3))])),
        ..Record::default()
    };
    chats.put_back(&record, SIZE);
    assert!(
        chats.looks_at(3),
        "the stored layout stands until the window reports"
    );

    let in_the_background = crate::asknotify::Looking {
        window_in_front: false,
        inbox_open: false,
        chat_on_screen: chats.looks_at(3),
    };

    assert!(
        !in_the_background.is_looking(),
        "a notification about chat 3 is held back by a layout nobody is looking at"
    );
    let in_front = crate::asknotify::Looking {
        window_in_front: true,
        ..in_the_background
    };
    assert!(
        in_front.is_looking(),
        "and only held back while the window is in front"
    );
    chats.end_all();
}

/// **A start that chose a pool that is gone starts nothing, and says the pool can't be reached**
/// (#4536, ruled 2026-11-01): the window then offers this machine for it, and nothing runs here
/// before that press. A start that chose this machine runs here whatever its workspace names.
#[cfg(unix)]
#[test]
fn a_start_on_a_pool_that_is_gone_starts_nothing_and_one_that_chose_this_machine_runs_here() {
    use purlis_core::machine::StartChoice;
    let plane = a_sandboxed_plane();
    let workspace = plane.path().join("workspaces/w");
    std::fs::create_dir_all(&workspace).expect("the workspace");
    let config = tempfile::tempdir().expect("this machine's config");
    purlis_core::machine::update(config.path(), |store| {
        store.remember(plane.path(), 1);
        store
            .set_chats_run_on(plane.path(), "w", Some("builds"))
            .expect("the workspace names a pool");
    })
    .expect("kept");
    let host = Pretend::default();
    let mut chats = restartable_chats_of(plane.path(), host.clone());
    chats.keeping_pools_in(Some(config.path().to_path_buf()));

    let on_a_gone_pool = Chat {
        runs_on: Some(StartChoice::Pool("gone".to_owned())),
        ..a_resumable_chat_in(&workspace)
    };
    let refused = crate::poolprofiles::offering_limits(|| chats.start(&on_a_gone_pool, SIZE))
        .expect_err("a pool that is gone starts nothing");
    assert_eq!(
        refused.pool_unavailable.as_deref(),
        Some("gone"),
        "{refused:?}"
    );
    assert!(refused.words.contains("pool gone"), "{}", refused.words);
    assert!(host.running().is_empty(), "never run here in its place");

    let by_its_workspace = crate::poolprofiles::offering_limits(|| {
        chats.start(&a_resumable_chat_in(&workspace), SIZE)
    })
    .expect_err("its workspace's pool is not kept either");
    assert_eq!(by_its_workspace.pool_unavailable.as_deref(), Some("builds"));

    // A shell, which writes no harness settings: what is asked here is only where it runs.
    let here = Chat {
        runs_on: Some(StartChoice::ThisMachine),
        ..a_chat_in(&workspace, "/bin/sh")
    };
    let session = crate::poolprofiles::offering_limits(|| chats.start(&here, SIZE))
        .expect("this machine, chosen for this start");
    assert_eq!(host.running(), vec![session]);
}

/// **A runner whose check ran out of time isn't walked again every minute by the sweep** (#5393):
/// 5 and a half minutes on one check leaves it just short of an hour, and a quick end changes
/// nothing about how often the sweep runs.
#[cfg(unix)]
#[test]
fn the_sweep_eases_off_a_runner_whose_check_took_long_and_only_the_sweep() {
    use std::time::Duration;
    assert_eq!(
        super::fresh_asked_again_after(Duration::from_secs(330)),
        Duration::from_secs(3300)
    );
    assert_eq!(
        super::fresh_asked_again_after(Duration::from_secs(1)),
        Duration::from_secs(60),
        "a quick end leaves the sweep's own pace"
    );
    assert_eq!(
        super::fresh_asked_again_after(Duration::from_secs(36_000)),
        Duration::from_secs(3600),
        "never left for longer than an hour"
    );

    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    assert!(chats.sweep_may_ask("01J9ZQ3V7K8M2N4P6R8T0V2X4Z"));
    lock(&chats.fresh_asked_again).insert(
        "01J9ZQ3V7K8M2N4P6R8T0V2X4Z".to_owned(),
        std::time::Instant::now() + Duration::from_secs(3300),
    );
    assert!(!chats.sweep_may_ask("01J9ZQ3V7K8M2N4P6R8T0V2X4Z"));
    assert!(chats.sweep_may_ask("another chat's"));
}

/// **A move takes a chat's one-off choice of where it runs** (#4536): the record then follows its
/// workspace, which the move just set, and the choice comes back where the move doesn't happen.
#[test]
fn a_chats_one_off_choice_of_where_it_runs_is_taken_and_given_back() {
    use purlis_core::machine::StartChoice;
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(
            &Chat {
                runs_on: Some(StartChoice::ThisMachine),
                ..a_chat_in(plane.path(), "/bin/sh")
            },
            SIZE,
        )
        .expect("a shell starts");
    let recorded = |chats: &Chats| {
        chats
            .record()
            .chats
            .into_iter()
            .find(|chat| chat.number == Some(session))
            .and_then(|chat| chat.runs_on)
    };
    assert_eq!(recorded(&chats), Some(StartChoice::ThisMachine));

    let was = chats.choose_runs_on(session, None);
    assert_eq!(was, Some(StartChoice::ThisMachine));
    assert_eq!(recorded(&chats), None, "it follows its workspace now");

    chats.choose_runs_on(session, was);
    assert_eq!(recorded(&chats), Some(StartChoice::ThisMachine));
    let _ = chats.close(session);
}

/// **A start's own choice adds to a Claim only where the person made it in the window** (#4536,
/// the person's ruling of 2026-11-02): a dispatched chat that chose where it runs adds nothing,
/// and one that chose nothing follows its levels as every chat does.
#[test]
fn a_dispatched_chats_choice_of_where_it_runs_puts_nothing_on_a_claim() {
    use purlis_core::machine::StartChoice;
    use purlis_core::reopen::{HandedFrom, Mode, Owed};
    let on_builds = Some(StartChoice::Pool("builds".to_owned()));
    let from_a_chat = Some(HandedFrom {
        id: None,
        chat: 3,
        name: "steward 3".to_owned(),
        workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
        report: Owed::Due,
        mode: Mode::Task,
        depth: 1,
        root: None,
        above: None,
        by_person: false,
        long: false,
    });
    let window = Chat {
        runs_on: on_builds.clone(),
        ..Default::default()
    };
    let dispatched = Chat {
        runs_on: on_builds.clone(),
        from: from_a_chat.clone(),
        ..Default::default()
    };
    let following = Chat {
        from: from_a_chat,
        ..Default::default()
    };
    assert_eq!(claim_follows(&window), Some(on_builds.as_ref()));
    assert_eq!(
        claim_follows(&dispatched),
        None,
        "a dispatch's choice adds nothing"
    );
    assert_eq!(
        claim_follows(&following),
        Some(None),
        "it follows the levels"
    );
}

#[test]
fn a_task_pressed_into_a_pane_is_held_from_that_layout_and_the_chat_it_replaced_keeps_running() {
    // #4757: a press shows a chat in the focused pane and says the new layout in the same call.
    // The task it lands on is on screen from that layout (`dispatched::is_held_back` reads
    // `looks_at`), so its due end waits for the person; the chat the pane showed before is in
    // no pane now, and nothing of it ends. A press that moves on lets the task's end go, and a
    // notification about it is no longer held back.
    use purlis_core::reopen::{HandedFrom, Mode, Owed};
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let asker = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let task = chats
        .start(
            &Chat {
                from: Some(HandedFrom {
                    id: None,
                    chat: asker,
                    name: "ide.7".to_owned(),
                    workspace: purlis_core::active::Place::Workspace("alpha".to_owned()),
                    report: Owed::Due,
                    mode: Mode::Task,
                    depth: 1,
                    root: None,
                    above: None,
                    by_person: false,
                    long: false,
                }),
                ..chat(&a_claude(dir.path()), "talk", None)
            },
            SIZE,
        )
        .unwrap();
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(asker))]))
        .unwrap();
    assert!(!chats.looks_at(task), "the task is in no pane yet");

    // The press: the focused pane shows the task in place of the chat that asked for it.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(task))]))
        .expect("a press may show any open chat");

    assert!(chats.looks_at(task), "held from the press's own layout");
    assert!(!chats.looks_at(asker));
    let looking = |chat_on_screen| crate::asknotify::Looking {
        window_in_front: true,
        inbox_open: false,
        chat_on_screen,
    };
    assert!(
        looking(chats.looks_at(task)).is_looking(),
        "a notification about the task is held back while the person reads it"
    );
    assert!(
        lock(&chats.open).contains_key(&asker),
        "the chat the press replaced keeps running"
    );

    // ⌘⇧H: the pane goes back to the chat that asked.
    chats
        .lay_out(on_ops(vec![front_tab(Node::chat(asker))]))
        .unwrap();

    assert!(
        !chats.looks_at(task),
        "its end is no longer held by the pane"
    );
    assert!(
        !looking(chats.looks_at(task)).is_looking(),
        "and a notification about it is sent again"
    );
    assert!(
        lock(&chats.open).contains_key(&task),
        "and nothing of it ended"
    );
    chats.end_all();
}

// ----- what the Skill studio started a chat for (#5185) ----------------------------------

#[test]
fn a_studio_chats_job_is_done_once_its_draft_came_and_stays_done() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let studio = purlis_core::reopen::StudioJob::new(
        Some("deploy".to_owned()),
        purlis_core::skills::verbs::Level::project(),
        purlis_core::reopen::StudioWork::OutcomeEvals,
        chrono::Utc::now(),
    );
    let session = chats
        .start(
            &Chat {
                studio: Some(studio),
                ..chat(&a_claude(dir.path()), "3", None)
            },
            SIZE,
        )
        .unwrap();
    assert_eq!(
        chats.open_now()[0].studio.as_ref().map(|job| job.done),
        Some(false)
    );

    chats.studio_job_done(session);
    chats.studio_job_done(session);

    assert_eq!(
        chats.record().chats[0].studio.as_ref().map(|job| job.done),
        Some(true)
    );
    assert_eq!(
        chats.open_now()[0].studio.as_ref().map(|job| job.done),
        Some(true)
    );
    let _ = chats.close(session);
}

#[test]
fn a_chat_the_studio_did_not_start_has_no_job_to_be_done() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "3", None), SIZE)
        .unwrap();

    chats.studio_job_done(session);

    assert_eq!(chats.record().chats[0].studio, None);
    let _ = chats.close(session);
}

/// #4476: a grade's chat asks nobody, so its proxy never holds a connection for the person's
/// answer, and so takes no Allow live; any other chat does unless policy turns it off.
#[test]
fn a_grades_chat_has_no_live_ask_and_another_chat_has_one_unless_policy_says_not() {
    let open = purlis_core::sandbox::policy::Locks::none();
    assert!(super::asks_live(false, &open));
    assert!(!super::asks_live(true, &open));
    let off = purlis_core::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"live-asks": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    assert!(off.refused_because().is_none(), "{off:?}");
    assert!(off.forbids_live_asks());
    assert!(!super::asks_live(false, &off));
}

/// #5248: a studio chat's job stops as its program ends, runs again with a later start, and the
/// chat is kept for its skill's Runs once it closes.
#[test]
fn a_studio_chat_finishes_as_its_program_ends_and_stays_on_runs_once_closed() {
    // A project of its own that leaves the sandbox off, as `no_project` is: its app state
    // holds this test's chats alone.
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").unwrap();
    // The program lives outside the project, as a sandboxed project asks.
    let programs = tempfile::tempdir().unwrap();
    let chats = chats_of(plane.path());
    let studio = purlis_core::reopen::StudioJob::new(
        Some("deploy".to_owned()),
        purlis_core::skills::verbs::Level::project(),
        purlis_core::reopen::StudioWork::Checks,
        chrono::Utc::now(),
    );
    let session = chats
        .start(
            &Chat {
                studio: Some(studio),
                ..chat(&a_claude(programs.path()), "3", None)
            },
            SIZE,
        )
        .unwrap();
    let finished = |chats: &Chats| {
        chats.record().chats[0]
            .studio
            .as_ref()
            .and_then(|job| job.finished.clone())
    };
    assert_eq!(finished(&chats), None);

    chats.its_program_ended(session);
    let stopped = finished(&chats);
    assert!(stopped.is_some());
    chats.its_program_ended(session);
    assert_eq!(finished(&chats), stopped, "the first stop's time stands");

    chats.close(session).unwrap();
    let kept = purlis_core::reopen::studio_chats::closed(plane.path());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].chat, session);
    assert_eq!(kept[0].job.skill.as_deref(), Some("deploy"));
    assert_eq!(kept[0].job.finished, stopped);
}

/// #5248: a chat the studio did not start is not kept for Runs.
#[test]
fn a_chat_the_studio_did_not_start_is_not_kept_for_runs() {
    // A project of its own that leaves the sandbox off, as `no_project` is: its app state
    // holds this test's chats alone.
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").unwrap();
    let programs = tempfile::tempdir().unwrap();
    let chats = chats_of(plane.path());
    let session = chats
        .start(&chat(&a_claude(programs.path()), "3", None), SIZE)
        .unwrap();

    chats.close(session).unwrap();

    assert_eq!(purlis_core::reopen::studio_chats::closed(plane.path()), []);
}

/// #5207: a running chat keeps the level it started with, so its reason says a restart applies
/// the level you changed, and stops saying so once it is at that level.
#[test]
fn why_a_chat_runs_at_its_level_says_a_restart_applies_a_level_changed_since_it_started() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let said = |chats: &Chats| {
        let whys = chats.level_whys_on(&a_machine());
        assert_eq!(whys.len(), 1, "{whys:?}");
        assert_eq!(whys[0].session, session);
        whys[0].why.clone()
    };
    assert!(
        said(&chats).contains("You have not chosen a level here"),
        "{}",
        said(&chats)
    );
    assert!(!said(&chats).contains("Restart chat"), "{}", said(&chats));

    std::fs::write(
        plane.path().join(purlis_core::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"off\"\n",
    )
    .expect("the local file");
    purlis_core::sandbox::personal::set_by_you(
        plane.path(),
        None,
        Some(purlis_core::sandbox::personal::Choice::Off),
    )
    .expect("confirmed");
    let now = said(&chats);
    assert!(
        now.starts_with(
            "This chat started with the full sandbox, and its next start has no sandbox. \
             Restart chat applies it. This is the level you chose"
        ),
        "{now}"
    );
    // A chat held by a sandbox this window has no compile of: no reason here describes its run.
    let held = chats.confines_of(session);
    chats.runs_as_recorded(session, None, None);
    assert_eq!(chats.level_whys_on(&a_machine()), Vec::new());
    chats.runs_as_recorded(session, held, None);
    assert!(said(&chats).starts_with("This chat started with"));
    // On a system with no backend, no level holds a chat, so no reason is said.
    let windows = purlis_core::sandbox::Machine {
        os: purlis_core::sandbox::Os::Windows,
        ..a_machine()
    };
    assert_eq!(chats.level_whys_on(&windows), Vec::new());
}

/// **A Discard pressed while a check runs wins when it ends** (#5312): that press isn't dropped,
/// and whatever the check found, the runner goes with what it holds. Unpressed, what it found
/// stands.
#[cfg(unix)]
#[test]
fn a_discard_during_a_check_lets_the_runner_go_whatever_the_check_found() {
    let found = vec!["the runner ran out of time".to_owned()];

    assert_eq!(
        ends_as(true, found.clone(), Going::OnProof),
        (Vec::new(), false)
    );
    assert_eq!(ends_as(false, found.clone(), Going::OnProof), (found, true));
    assert_eq!(
        ends_as(false, Vec::new(), Going::Discarded),
        (Vec::new(), false)
    );
}

/// A chat reattached on the plan it started with keeps its confines, but whether its start held
/// the project's git is not on record: the chat saves and updates the project with its own git.
#[test]
fn a_reattached_chat_leaves_the_projects_git_to_itself() {
    use purlis_session_protocol::session::RunSandbox;
    let plane = a_sandboxed_plane();
    std::fs::create_dir_all(plane.path().join(".git")).expect("a git folder");
    let first = Pretend::default();
    let chats = restartable_chats_of(plane.path(), first.clone());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    let started = chats.confines_of(session).expect("it started sandboxed");
    assert!(
        started.roots_git.is_some(),
        "its start holds the project's git"
    );
    // The plan its start was confined from, where anything runs beside it.
    let plan = first.plans_confined().pop();

    let host = Pretend::default();
    host.leaves_running_in(3, plan, Some(RunSandbox::By("seatbelt".to_owned())));
    let chats = restartable_chats_of(plane.path(), host.clone());
    chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );

    assert_eq!(host.reattached(), [3]);
    let reattached = chats.confines_of(3).expect("its confines are kept");
    assert_eq!(reattached.roots_git, None);
}

/// #4218 (SK-15h): a runner's chat is shown the skills its runner said it armed it with, at its
/// start and once this window reattaches it; a runner before 1.77, which says none, leaves it
/// saying so, never an empty set. A chat armed here has no runner's word.
#[test]
fn a_runners_chat_has_the_skills_its_runner_said_it_armed_it_with_4218() {
    let said = vec![purlis_session_protocol::session::ArmedSkill {
        name: "deploy".into(),
        level: "project".into(),
    }];
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    let host = Pretend::default();
    host.on_a_runner();
    host.says_it_armed(Some(said.clone()));
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    let started = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    assert_eq!(
        chats.skills_of(started).expect("open").runners_word,
        Some(RunnersWord::Said(said.clone())),
        "at its start"
    );
    host.says_it_armed(None);
    let older = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    assert_eq!(
        chats.skills_of(older).expect("open").runners_word,
        Some(RunnersWord::NotSaid),
        "a runner before 1.77"
    );

    let host = Pretend::default();
    host.on_a_runner();
    host.says_it_armed(Some(said.clone()));
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());
    chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );
    assert_eq!(host.reattached(), [3]);
    assert_eq!(
        chats.skills_of(3).expect("open").runners_word,
        Some(RunnersWord::Said(said)),
        "once reattached"
    );
    let _ = chats.close(3);
}

/// #4219 (SK-20c): a runner's lines come as the link comes up, before a chat the window puts back
/// is open: a skill run of one still coming back is logged here with the window's word for it
/// from its record (its persona, harness and workspace). A chat neither open nor coming back is
/// none of this window's, and logs nothing.
#[test]
fn a_skill_run_of_a_runner_chat_still_coming_back_is_logged_from_its_record_4219() {
    use purlis_core::skilluse::relayed::{Line, Relayed};
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    let host = Pretend::default();
    host.on_a_runner();
    let mut chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    chats.on_device(Some(DEVICE.to_owned()));
    let record = left_running_in(
        plane.path(),
        Chat {
            persona: Some("steward".to_owned()),
            ..a_resumable_chat_in(plane.path())
        },
    );
    chats.begin_put_back(&record, &|_| None);
    let started = |chat: &str, run: &str| Relayed {
        chat: chat.to_owned(),
        run: run.to_owned(),
        at_ms: 1_793_440_800_000,
        line: Line::Started {
            skill: "review".to_owned(),
            started_by: purlis_core::skills::runs::StartedBy::SkillTool,
            level: None,
            tree: None,
        },
    };

    chats.skill_run_heard(
        Some("builds"),
        &started(CHAT_ID, "01K6RUNA00000000000000000A"),
    );
    chats.skill_run_heard(
        Some("builds"),
        &started("01K6NOTHERE00000000000000D", "01K6RUND00000000000000000D"),
    );

    let runs = purlis_core::skilluse::runs(plane.path());
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0].id.as_deref(), Some("01K6RUNA00000000000000000A"));
    assert_eq!(runs[0].persona.as_deref(), Some("steward"));
    assert_eq!(runs[0].harness.as_deref(), Some(Harness::ClaudeCode.name()));
    assert_eq!(runs[0].device, DEVICE);
}

/// **#2977 (SK-5d): a chat on a runner, in Docker or on k8s, whose persona names a skill its
/// runner lacks, says so on its tab as a chat here does, naming that runner**: the runner reads
/// its harness's skills there and says what is missing in its start's answer (1.81). Each kind
/// of pool's runner is named as its link names it, and a chat's own cluster runner as it is
/// kept; a runner that says nothing missing (or a runner before 1.81) leaves no Notice.
#[test]
fn a_runner_chat_whose_persona_names_a_skill_its_runner_lacks_says_so_naming_the_runner_2977() {
    use purlis_session_protocol::session::SkillsNotThere;
    let lacking = SkillsNotThere {
        missing: vec!["superpowers:test-driven-development".into()],
        no_built_in: vec!["purlis-secret".into()],
        ..Default::default()
    };
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    for (kind, pool, runner) in [
        ("a local runner", None, "local/podman-1"),
        ("a Docker pool's runner", Some("builds"), "builds/builds-1"),
        (
            "a k8s pool's runner",
            Some("cluster"),
            "cluster/purlis-runner-0",
        ),
    ] {
        let host = Pretend::default();
        match pool {
            Some(pool) => host.on_pool(pool),
            None => host.on_a_runner(),
        }
        host.says_it_lacks(Some(lacking.clone()), Some(runner));
        let chats = Chats::on_host(
            Box::new(|_| {}),
            Box::new(host.clone()),
            plane.path().to_path_buf(),
        );
        let started = chats
            .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
            .expect("it starts on the runner, without them");

        assert_eq!(
            chats.skills_missing(started),
            Some(SkillsMissing {
                persona: Some("steward".to_owned()),
                missing: lacking.missing.clone(),
                no_built_in: lacking.no_built_in.clone(),
                no_workspace: Vec::new(),
                not_in_its_workspace: Vec::new(),
                broken: Vec::new(),
                by_a_repo: Vec::new(),
                beside_a_built_in: Vec::new(),
                runner: Some(runner.to_owned()),
            }),
            "{kind}"
        );

        host.says_it_lacks(None, Some(runner));
        let whole = chats
            .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
            .expect("it starts on the runner");
        assert_eq!(chats.skills_missing(whole), None, "{kind}: none missing");
    }

    // A pool's runner its link does not name is named by its pool.
    let host = Pretend::default();
    host.on_pool("builds");
    host.says_it_lacks(Some(lacking.clone()), None);
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    let started = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner");
    assert_eq!(
        chats.skills_missing(started).and_then(|said| said.runner),
        Some("builds".to_owned())
    );
}

/// **#6078: a runner chat's Notice says all a chat here would, and says it again once this
/// window reattaches it**: another workspace's skill its persona names that is not there, a
/// plane skill that could not be loaded or that the repo leaves out, each with its level and
/// why, and a repo skill named like a built-in one, as its runner read them there (1.83). A
/// skill at a level this window does not know costs that one skill alone. A window that comes
/// back to a chat left running says what its host kept of its start, naming the same runner.
#[test]
fn a_runner_chats_notice_says_all_a_chat_here_would_and_again_once_reattached_6078() {
    use purlis_session_protocol::session::{SkillNotLoaded, SkillsNotThere};
    let not_loaded = |name: &str, level: &str, local: bool, why: &str| SkillNotLoaded {
        name: name.into(),
        level: level.into(),
        local,
        why: why.into(),
    };
    let lacking = SkillsNotThere {
        missing: vec!["superpowers:test-driven-development".into()],
        no_workspace: vec!["workspaces/ghost:x".into()],
        not_in_its_workspace: vec!["workspaces/ops:nope".into()],
        broken: vec![
            not_loaded("half-made", "project", false, "it has no frontmatter"),
            not_loaded(
                "from-later",
                "a-level-of-later",
                false,
                "it is of a later level",
            ),
        ],
        by_a_repo: vec![not_loaded(
            "deploy",
            "workspace",
            true,
            "the repo has its own",
        )],
        beside_a_built_in: vec!["purlis-secrets".into()],
        ..Default::default()
    };
    let shown =
        |name: &str, origin: purlis_core::skills::Origin, local: bool, why: &str| SkillBroken {
            name: name.into(),
            level: origin.into(),
            local,
            label: purlis_core::skills::level_shown(origin, local),
            why: why.into(),
        };
    let expected = SkillsMissing {
        persona: Some("steward".to_owned()),
        missing: lacking.missing.clone(),
        no_built_in: Vec::new(),
        no_workspace: lacking.no_workspace.clone(),
        not_in_its_workspace: lacking.not_in_its_workspace.clone(),
        broken: vec![shown(
            "half-made",
            purlis_core::skills::Origin::Project,
            false,
            "it has no frontmatter",
        )],
        by_a_repo: vec![shown(
            "deploy",
            purlis_core::skills::Origin::Workspace,
            true,
            "the repo has its own",
        )],
        beside_a_built_in: lacking.beside_a_built_in.clone(),
        runner: Some("builds/builds-1".to_owned()),
    };
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    let host = Pretend::default();
    host.on_pool("builds");
    host.says_it_lacks(Some(lacking.clone()), Some("builds/builds-1"));
    let chats = Chats::on_host(
        Box::new(|_| {}),
        Box::new(host.clone()),
        plane.path().to_path_buf(),
    );
    let started = chats
        .start_ready(&on_the_profile_work(), &a_harness_ready(None), SIZE)
        .expect("it starts on the runner, without them");
    assert_eq!(
        chats.skills_missing(started),
        Some(expected.clone()),
        "at its start"
    );

    let host = Pretend::default();
    host.on_a_runner();
    host.says_it_lacks(Some(lacking), Some("builds/builds-1"));
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());
    chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );
    assert_eq!(host.reattached(), [3]);
    // This window's stand-in start for a chat put back sets no environment, so the persona it
    // runs as is the project's default, which this plane has none of.
    assert_eq!(
        chats.skills_missing(3),
        Some(SkillsMissing {
            persona: None,
            ..expected
        }),
        "once reattached"
    );
    let _ = chats.close(3);

    // A host before 1.83, which keeps nothing of it for a window that comes back, shows none.
    let host = Pretend::default();
    host.on_a_runner();
    host.leaves_running(3, None);
    let chats = restartable_chats_of(plane.path(), host.clone());
    chats.put_back(
        &left_running_in(plane.path(), a_resumable_chat_in(plane.path())),
        SIZE,
    );
    assert_eq!(chats.skills_missing(3), None, "nothing kept");
    let _ = chats.close(3);
}

/// R-227, Q238: a chat the window starts records, in its run's start, the protection this start
/// resolved: the project's sandbox where its file turns it on, and purlis's default floor where
/// it does not, never the project's.
#[test]
fn a_chat_the_window_starts_records_the_tier_of_the_sandbox_it_resolved() {
    for (manifest, level, tier) in [
        ("[sandbox]\nmode = \"on\"\n", "on", "project"),
        ("", "floor", "purlis"),
    ] {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(plane.path().join(purlis_core::plane::MANIFEST), manifest)
            .expect("charter.toml");
        let chats = restartable_chats_of(plane.path(), Pretend::default());
        let protection: std::sync::Arc<Mutex<Vec<serde_json::Value>>> = std::sync::Arc::default();
        let told = std::sync::Arc::clone(&protection);
        chats.when_a_run_begins(Box::new(move |_, _, _, attributes, _| {
            lock(&told).push(serde_json::to_value(&attributes.protection).expect("json"));
        }));

        let session = chats
            .start(&a_chat_in_with(plane.path(), None), SIZE)
            .expect("starts");

        let protection = lock(&protection).clone();
        assert_eq!(protection.len(), 1, "{manifest:?}: one run began");
        assert_eq!(
            protection[0]["layers"],
            serde_json::json!([{"layer": "sandbox", "level": level, "tier": tier}]),
            "{manifest:?}"
        );
        chats.close(session).expect("ends");
    }
}

/// R-290 under Q240: when chat 9 has "moved on" from a connection its proxy holds, through a
/// real board of live asks beside an open chat, its hook lines told as [`Chats::called`] hears
/// them, and its stops written where a test reads them.
mod moved_on {
    use std::sync::{Arc, Mutex, Weak};
    use std::time::Duration;

    use purlis_core::eventlog::stops::{Source, Stop, Target};
    use purlis_core::hookwire::Decision;
    use purlis_core::sandbox::asks::{Answer, Asks};

    use super::*;

    const CHAT: u32 = 9;

    /// What holds a real board, as the session host holds a chat's proxy.
    #[derive(Debug)]
    struct HeldAsks(Arc<Asks>);

    impl purlis_core::sandbox::HeldElsewhere for HeldAsks {
        fn id(&self) -> &str {
            "asks"
        }

        fn withdraw_port(&self, _: u16) -> Result<(), String> {
            Ok(())
        }

        fn board(&self) -> Option<Arc<dyn purlis_core::sandbox::Board>> {
            Some(Arc::clone(&self.0) as Arc<dyn purlis_core::sandbox::Board>)
        }
    }

    pub(super) struct Live {
        pub chats: Arc<Chats>,
        pub board: Arc<Asks>,
        written: Arc<Mutex<Vec<(u32, Stop)>>>,
        _plane: tempfile::TempDir,
    }

    /// Chat 9 open with a proxy that asks live, its hold `hold` long, each ask told as the
    /// window's road tells it ([`crate::taskblocks::heard`]).
    pub(super) fn live(hold: Duration) -> Live {
        let plane = tempfile::tempdir().expect("a plane");
        let chats = Arc::new(restartable_chats_of(plane.path(), Pretend::default()));
        let written: Arc<Mutex<Vec<(u32, Stop)>>> = Arc::default();
        chats.write_stops_with({
            let written = Arc::clone(&written);
            Arc::new(move |chat, stop, _| {
                lock(&written).push((chat, stop));
                Ok(())
            })
        });
        let weak: Weak<Chats> = Arc::downgrade(&chats);
        let plane_id: crate::planes::PlaneId =
            serde_json::from_str("\"/plane\"").expect("a plane id");
        let hear: crate::hooks::Blocks = Arc::new(move |line| {
            let Some(chats) = weak.upgrade() else { return };
            let told = crate::hooks::blocked_holding(&plane_id, &line.into(), false, None, None);
            crate::taskblocks::heard(&chats, &told);
        });
        let asking = super::super::asked_by_the_proxy(
            super::super::Asking {
                hear: Arc::new(Mutex::new(Some(hear))),
                reached: Arc::new(Mutex::new(None)),
                whose: Arc::new(std::sync::atomic::AtomicU32::new(CHAT)),
                who: super::super::ReachedAs::default(),
                harness: Some(Harness::ClaudeCode),
                blocks: Arc::clone(&chats.blocks),
                moved: Arc::new(Mutex::new(None)),
                stops: Arc::clone(&chats.stops),
            },
            plane.path().to_path_buf(),
        );
        let board = Arc::new(Asks::timed(asking.tell, hold, Duration::from_millis(10)));
        let mut chat = a_resumable_chat_in(plane.path());
        chat.identity.id = Some("chat-9".to_owned());
        let confinement = purlis_core::sandbox::Confinement::held_on_another_machine(
            purlis_core::sandbox::Placed {
                http: 1,
                socks: 2,
                tmp: None,
                routed: None,
                ssh: None,
                left_out: Vec::new(),
                host_name: None,
            },
            Box::new(HeldAsks(Arc::clone(&board))),
        );
        lock(&chats.open).insert(
            CHAT,
            Running {
                chat,
                how: Reopened::Resumed(SessionId::new(ID).expect("an id")),
                harness: Some(Harness::ClaudeCode),
                declared: None,
                workspace: None,
                confinement: Some(confinement),
                confines: None,
                started_with: StartedWith::default(),
                composed_by_its_host: false,
                ran: Some(purlis_session_protocol::session::RunSandbox::By(
                    "seatbelt".to_owned(),
                )),
                grants_as: None,
                skills: None,
                plane_skills: true,
                skills_read: Default::default(),
                last_prompt: None,
                runs_as: None,
                runners_word: None,
                reports_verified: false,
                clones_at_start: None,
                prompt_key: None,
            },
        );
        Live {
            chats,
            board,
            written,
            _plane: plane,
        }
    }

    impl Live {
        /// A hook line of chat 9's: `hook` of call `id` of `tool`, by `agent` (the main one
        /// where none).
        pub fn line(&self, hook: &str, id: Option<&str>, tool: &str, agent: Option<&str>) {
            self.line_deciding(hook, id, tool, agent, Decision::default());
        }

        /// A hook line as [`Self::line`], whose hook answered `decision`.
        pub fn line_deciding(
            &self,
            hook: &str,
            id: Option<&str>,
            tool: &str,
            agent: Option<&str>,
            decision: Decision,
        ) {
            self.chats.called(&purlis_core::hookwire::ToolCall {
                chat: CHAT,
                tool_hook: hook.to_owned(),
                tool: Some(tool.to_owned()),
                call: id.map(str::to_owned),
                args: None,
                decision,
                rule: None,
                arm: None,
                hook_ms: 0,
                agent: agent.map(str::to_owned),
                at_ms: 0,
                skill: None,
            });
        }

        /// A connection to `host` on 443, held on a thread of its own until the ask is listed.
        pub fn hold(&self, host: &'static str) -> std::thread::JoinHandle<Answer> {
            let before = self.asks_of(host);
            let board = Arc::clone(&self.board);
            let held = std::thread::spawn(move || board.hold(host, 443, &[443]));
            self.until(|live| live.asks_of(host) > before, "the ask is listed");
            held
        }

        /// How many asks the registry lists for chat 9 of `host`.
        pub fn asks_of(&self, host: &str) -> usize {
            self.chats
                .blocks()
                .every_since()
                .iter()
                .filter(|(chat, block, _)| *chat == CHAT && block.target == host)
                .count()
        }

        /// The Sandbox blocks written for chat 9 of `host`.
        pub fn blocks_of(&self, host: &str) -> Vec<purlis_core::eventlog::stops::Block> {
            lock(&self.written)
                .iter()
                .filter_map(|(chat, stop)| match stop {
                    Stop::Block(block)
                        if *chat == CHAT
                            && block.source == Source::Sandbox
                            && block.target == Some(Target::host(host).expect("a host")) =>
                    {
                        Some(block.clone())
                    }
                    _ => None,
                })
                .collect()
        }

        pub fn until(&self, done: impl Fn(&Self) -> bool, what: &str) {
            for _ in 0..400 {
                if done(self) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("never: {what}");
        }

        /// Gives what is in flight time to go wrong.
        pub fn settle(&self) {
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    /// Q240: a call by a sub-agent, while the call the connection is held under is open, is not
    /// the chat moving on: the ask stays and no block is written.
    #[test]
    fn a_sub_agents_call_does_not_withdraw_a_held_ask() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("pretooluse", Some("sub-1"), "Read", Some("agent-a"));
        live.line("posttooluse", Some("sub-1"), "Read", Some("agent-a"));
        live.line("pretooluse", Some("sub-2"), "Bash", Some("agent-a"));
        live.settle();
        assert_eq!(live.asks_of("api.example.com"), 1, "still an ask");
        assert!(live.blocks_of("api.example.com").is_empty(), "and no block");
        assert_eq!(live.board.holding(), 1, "its connection still held");
        live.board
            .keep_blocked(&purlis_core::sandbox::hosts::Host::parse("api.example.com").unwrap());
        held.join().expect("answered");
    }

    /// Q240: a parallel call of the same agent, made while the held call is still open, is not
    /// the chat moving on.
    #[test]
    fn a_parallel_call_does_not_withdraw_a_held_ask() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("pretooluse", Some("call-2"), "Bash", None);
        live.line("posttooluse", Some("call-2"), "Bash", None);
        live.line("pretooluse", Some("call-3"), "Read", None);
        live.settle();
        assert_eq!(live.asks_of("api.example.com"), 1, "still an ask");
        assert!(live.blocks_of("api.example.com").is_empty(), "and no block");
        live.board
            .keep_blocked(&purlis_core::sandbox::hosts::Host::parse("api.example.com").unwrap());
        held.join().expect("answered");
    }

    /// Q240: the held call's post-tool line alone is no next call: the chat may still be
    /// waiting on what that call started.
    #[test]
    fn a_posttooluse_alone_does_not_withdraw_a_held_ask() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", Some("call-1"), "Bash", None);
        live.settle();
        assert_eq!(live.asks_of("api.example.com"), 1, "still an ask");
        assert!(live.blocks_of("api.example.com").is_empty(), "and no block");
        live.board
            .keep_blocked(&purlis_core::sandbox::hosts::Host::parse("api.example.com").unwrap());
        held.join().expect("answered");
    }

    /// Q240: where the hook lines carry no call id, nothing says which call a connection is
    /// held under, so only its timeout makes it a block.
    #[test]
    fn with_no_call_id_only_the_timeout_withdraws_a_held_ask() {
        let live = live(Duration::from_millis(600));
        live.line("pretooluse", None, "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", None, "Bash", None);
        live.line("pretooluse", None, "Read", None);
        live.settle();
        assert_eq!(live.asks_of("api.example.com"), 1, "still an ask");
        assert!(live.blocks_of("api.example.com").is_empty(), "and no block");
        assert_eq!(held.join().expect("answered"), Answer::TimedOut);
        live.until(
            |live| live.blocks_of("api.example.com").len() == 1,
            "one block on timeout",
        );
        assert_eq!(live.asks_of("api.example.com"), 0, "withdrawn");
    }

    /// R-290 under Q240: the held call ended and the same agent made its next call. The board
    /// gives the connection up, it is refused then, and the one timeout path withdraws the ask
    /// and writes one block, under the tool of the call it was held under.
    #[test]
    fn the_same_agents_next_call_after_the_held_call_ended_makes_it_one_block() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", Some("call-1"), "Bash", None);
        live.line("pretooluse", Some("call-2"), "Read", None);
        assert_eq!(
            held.join().expect("answered"),
            Answer::TimedOut,
            "refused now"
        );
        live.until(
            |live| live.blocks_of("api.example.com").len() == 1,
            "one block",
        );
        assert_eq!(live.asks_of("api.example.com"), 0, "the ask is withdrawn");
        let block = &live.blocks_of("api.example.com")[0];
        assert_eq!(block.tool, "Bash", "the call it was held under");
        assert_eq!(block.qualifier, None);
        live.line("posttooluse", Some("call-2"), "Read", None);
        live.line("pretooluse", Some("call-3"), "Read", None);
        live.settle();
        assert_eq!(live.blocks_of("api.example.com").len(), 1, "never a second");
    }

    /// Q240: the held call ended and the same agent made its next call: one block, refused
    /// now, whatever came before it.
    fn moves_on_to_one_block(live: &Live) {
        let held = live.hold("api.example.com");
        live.line("posttooluse-blocked", Some("call-1"), "Bash", None);
        live.line("pretooluse", Some("call-2"), "Bash", None);
        live.until(
            |live| live.board.holding() == 0,
            "given up on the next call, long before its timeout",
        );
        assert_eq!(
            held.join().expect("answered"),
            Answer::TimedOut,
            "refused now"
        );
        live.until(
            |live| live.blocks_of("api.example.com").len() == 1,
            "one block",
        );
        assert_eq!(live.asks_of("api.example.com"), 0, "the ask is withdrawn");
    }

    /// Q240: a Grep has no post-tool line (no `PostToolUse` hook names it), so it never stays
    /// open: a Grep before the held call does not keep the chat from moving on.
    #[test]
    fn a_grep_before_the_held_call_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse-read", Some("grep-1"), "Grep", None);
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: a call its guard refused never runs and has no post-tool line: it does not keep
    /// the chat from moving on.
    #[test]
    fn a_denied_call_before_the_held_call_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line_deciding("pretooluse", Some("denied-1"), "Bash", None, Decision::Deny);
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: a sub-agent's Grep, made before the held call, never comes back either; the main
    /// agent's calls ending says nothing of it, so it must never have been open.
    #[test]
    fn a_sub_agents_grep_before_the_held_call_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse-read", Some("grep-1"), "Grep", Some("agent-a"));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: a sub-agent's call its guard refused, made before the held call, never runs; the
    /// main agent's calls ending says nothing of it, so it must never have been open.
    #[test]
    fn a_sub_agents_denied_call_before_the_held_call_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line_deciding(
            "pretooluse",
            Some("denied-1"),
            "Bash",
            Some("agent-a"),
            Decision::Deny,
        );
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: a call the harness's own check refused is over when that refusal is heard.
    #[test]
    fn a_sub_agents_call_the_harness_refused_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("refused-1"), "Bash", Some("agent-a"));
        live.line(
            purlis_core::hookreg::HARNESS_DENIED,
            Some("refused-1"),
            "Bash",
            Some("agent-a"),
        );
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: a failed Edit has no post-tool line (its failure is no `PostToolUse`): the same
    /// agent's later call ending is what closes it, so it does not keep the chat from moving on.
    #[test]
    fn a_failed_edit_before_the_held_call_does_not_stop_moving_on() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse-edit", Some("edit-1"), "Edit", None);
        live.line("pretooluse", Some("call-1"), "Bash", None);
        moves_on_to_one_block(&live);
    }

    /// Q240: after the chat moved on, a retry to the same host is a new ask the person sees,
    /// never a hold nobody sees; its timeout is a block of its own.
    #[test]
    fn a_retry_after_the_chat_moved_on_is_asked_again_visibly() {
        let live = live(Duration::from_millis(800));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", Some("call-1"), "Bash", None);
        live.line("pretooluse", Some("call-2"), "Bash", None);
        assert_eq!(held.join().expect("answered"), Answer::TimedOut);
        live.until(
            |live| live.blocks_of("api.example.com").len() == 1,
            "one block",
        );
        let retry = live.hold("api.example.com");
        assert_eq!(live.asks_of("api.example.com"), 1, "asked again");
        assert_eq!(retry.join().expect("answered"), Answer::TimedOut);
        live.until(
            |live| live.blocks_of("api.example.com").len() == 2,
            "its own block",
        );
    }

    /// Q240: an Allow that reaches the connection before the chat's next call carries it: no
    /// block is written for a connection that went on.
    #[test]
    fn an_allow_racing_the_next_call_leaves_no_false_block() {
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", Some("call-1"), "Bash", None);
        live.chats.allow_live(
            Some(CHAT),
            &purlis_core::sandbox::hosts::Host::parse("api.example.com").unwrap(),
            purlis_core::sandbox::grant::Level::Chat,
        );
        live.line("pretooluse", Some("call-2"), "Bash", None);
        assert!(matches!(held.join().expect("answered"), Answer::Allowed(_)));
        live.settle();
        assert!(
            live.blocks_of("api.example.com").is_empty(),
            "no false block"
        );
    }

    /// Q240: the post-tool detector's line for the same refusal, named or unnamed, after its
    /// block was written, is dropped, never held again as an ask; so is the proxy's own raise
    /// of the ask landing late. A new ask of the same host after that is an ask.
    #[test]
    fn the_echo_of_a_written_block_is_not_held_again_as_an_ask() {
        use purlis_core::sandboxblock::{Block, Kind, Operation};
        let live = live(Duration::from_secs(30));
        live.line("pretooluse", Some("call-1"), "Bash", None);
        let held = live.hold("api.example.com");
        live.line("posttooluse", Some("call-1"), "Bash", None);
        live.line("pretooluse", Some("call-2"), "Bash", None);
        held.join().expect("answered");
        live.until(
            |live| live.blocks_of("api.example.com").len() == 1,
            "one block",
        );
        let plane: crate::planes::PlaneId = serde_json::from_str("\"/plane\"").unwrap();
        let echo = |target: Option<&str>| {
            let line = purlis_core::hookwire::SandboxBlocked {
                chat: CHAT,
                sandbox_blocked: Block {
                    operation: Operation::Connect,
                    kind: Kind::Host,
                    ours: false,
                },
                harness: Some("claude-code".to_owned()),
                target: target.map(str::to_owned),
                read_back: false,
                suggested: None,
            };
            let told = crate::hooks::blocked_holding(&plane, &line.into(), false, None, None);
            crate::taskblocks::heard(&live.chats, &told)
        };
        assert!(!echo(Some("api.example.com:443")), "named: dropped");
        assert!(!echo(None), "unnamed: dropped");
        assert!(
            live.chats.blocks().every().is_empty(),
            "nothing held as an ask"
        );
        let retry = live.hold("api.example.com");
        assert_eq!(live.asks_of("api.example.com"), 1, "a new ask is an ask");
        live.board
            .keep_blocked(&purlis_core::sandbox::hosts::Host::parse("api.example.com").unwrap());
        retry.join().expect("answered");
    }
}

/// R-290 (review finding 5): a held connection's timeout reaches the event log itself, through
/// [`crate::hooks::Hooks::write_stop`] and a real recorder, as one `stop.block` from the
/// sandbox naming the host, the tool of the call it was held under and its arguments' digest
/// (the block loop's target, R-206), and no qualifier (Q235: purlis's own proxy observed it).
#[test]
fn a_timed_out_hold_is_written_to_the_event_log_as_one_sandbox_block() {
    use purlis_core::eventlog::stops::{BLOCK, Block, Source, Target};
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    use purlis_core::sandbox::asks::Heard;
    let logs = tempfile::tempdir().expect("a folder for the log");
    let plane: crate::planes::PlaneId = serde_json::from_str("\"/plane\"").expect("a plane id");
    let hooks = Arc::new(crate::hooks::Hooks::deaf(plane));
    hooks.record_into(crate::hooks::Events::here(Recorder::new(
        Log::open(logs.path(), "DEVICE").expect("a log"),
        ArgsKey::open(logs.path()).expect("a key"),
    )));
    let chats = Chats::new();
    chats.write_stops_with({
        let hooks = Arc::clone(&hooks);
        Arc::new(move |chat, stop, _| hooks.write_stop(chat, stop))
    });
    let tell = super::asked_by_the_proxy(
        super::Asking {
            hear: Arc::new(Mutex::new(Some(Arc::new(|_| {})))),
            reached: Arc::new(Mutex::new(None)),
            whose: Arc::new(std::sync::atomic::AtomicU32::new(9)),
            who: super::ReachedAs::default(),
            harness: Some(Harness::ClaudeCode),
            blocks: Arc::clone(&chats.blocks),
            moved: Arc::new(Mutex::new(None)),
            stops: Arc::clone(&chats.stops),
        },
        std::env::temp_dir().join("purlis-no-such-project-290-log"),
    )
    .tell;
    chats.called(&purlis_core::hookwire::ToolCall {
        chat: 9,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some("call-1".to_owned()),
        args: Some(HELD_CALLS_ARGS.to_owned()),
        decision: purlis_core::hookwire::Decision::default(),
        rule: None,
        arm: None,
        hook_ms: 0,
        agent: None,
        at_ms: 0,
        skill: None,
    });
    tell(Heard::Asked(vec!["slow.example.com:443".to_owned()]));
    tell(Heard::TimedOut("slow.example.com:443".to_owned()));

    let blocks: Vec<Block> = purlis_core::eventlog::read(logs.path())
        .expect("the log")
        .into_iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| serde_json::from_value(event.body).expect("a block's body"))
        .collect();
    assert_eq!(
        blocks,
        [Block {
            tool: "Bash".to_owned(),
            source: Source::Sandbox,
            qualifier: None,
            target: Some(Target::host("slow.example.com").expect("a host")),
            args: Some(
                ArgsKey::open(logs.path())
                    .expect("the same key")
                    .digest(HELD_CALLS_ARGS)
            ),
        }],
        "one stop.block, from the sandbox"
    );
}

/// The hash of the arguments of the call a connection was held under, as its hook sent it.
const HELD_CALLS_ARGS: &str = "5e1f0c0e8a7d6b4c3a2918f7e6d5c4b3a29180f7e6d5c4b3a2918f7e6d5c4b3a";

// ----- the attendance fact A4 reads (T1-18, #6409; Q271, Q275) -----

/// A line of chat `chat`'s own Claude Code hook whose `permission_mode` is the bypass: what lowers
/// its attendance fact (Q271).
fn a_bypass_report(chat: u32) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event: purlis_core::state::Event::UserPromptSubmit,
        conversation: purlis_core::hookwire::Conversation::Named(ID.to_owned()),
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail {
            unattended: true,
            ..purlis_core::state::Detail::default()
        },
    }
}

/// Whether chat `session`'s attendance fact, as this app holds it, says nobody is at it.
fn held_unattended(chats: &Chats, session: u32) -> bool {
    chats
        .attendance_of(session)
        .unwrap_or_else(|| panic!("chat {session} holds no attendance fact"))
        .unattended()
}

#[test]
fn a_task_a_dispatch_started_is_unattended_and_its_record_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let chats = Chats::new();
    let task = chats
        .start(
            &started_by(dir.path(), "ide.7", purlis_core::reopen::Mode::Task),
            SIZE,
        )
        .unwrap();
    let person = chats
        .start(&chat(&a_claude(dir.path()), "ide.8", None), SIZE)
        .unwrap();

    assert!(held_unattended(&chats, task), "a task has nobody at it");
    assert!(
        !held_unattended(&chats, person),
        "a person's chat is attended"
    );
    // What a relaunch reads: the record keeps the task unattended, the person's chat not.
    let kept: Vec<(String, bool)> = chats
        .record()
        .chats
        .iter()
        .map(|chat| (chat.name.clone(), chat.unattended))
        .collect();
    assert_eq!(
        kept,
        [("ide.7".to_owned(), true), ("ide.8".to_owned(), false)]
    );
    chats.end_all();
}

#[test]
fn a_chat_its_record_kept_unattended_relaunches_unattended() {
    let dir = tempfile::tempdir().unwrap();
    let claude = a_claude(dir.path());
    for kept in [true, false] {
        let (chats, _) = recorded();
        let record = Record {
            chats: vec![Chat {
                unattended: kept,
                ..chat(&claude, "ide.7", Some(ID))
            }],
            ..one_recorded(&claude)
        };
        let open = chats.put_back_here(&record, SIZE);
        assert_eq!(open.len(), 1, "it starts again");
        // A person's start, which alone would be attended: the record's fact is never raised.
        assert_eq!(
            held_unattended(&chats, open[0].session),
            kept,
            "a chat recorded unattended: {kept}"
        );
        chats.end_all();
    }
}

#[test]
fn a_report_that_lowers_the_fact_is_written_down() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, wrote) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    assert!(!held_unattended(&chats, session));
    lock(&wrote).clear();

    chats.heard_attendance(&a_bypass_report(session));

    assert!(held_unattended(&chats, session), "the bypass bit lowers it");
    let last = lock(&wrote)
        .last()
        .cloned()
        .expect("the lowered fact is written");
    assert!(last.chats[0].unattended, "{:?}", last.chats[0]);
    // A report that moves nothing writes nothing more.
    let written = lock(&wrote).len();
    chats.heard_attendance(&a_bypass_report(session));
    assert_eq!(lock(&wrote).len(), written);
    chats.end_all();
}

/// Q275: Restart and Restart without sandbox each start the chat under a new number, and the
/// fact a report lowered comes with it.
#[test]
fn a_chat_a_report_lowered_comes_back_unattended_from_a_restart() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    // Restart without sandbox records its opt-out before it starts (ADR 0067 §7).
    chats.when_the_sandbox_is_decided(Box::new(|_| Ok(())));
    let first = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    assert!(!held_unattended(&chats, first), "a person's chat");
    chats.heard_attendance(&a_bypass_report(first));
    assert!(held_unattended(&chats, first));

    chats.ask_restart(first).expect("asked");
    let restarted = chats.restart(first, SIZE).expect("restarts");
    chats.close(first).expect("the old run ends");
    assert_ne!(restarted, first, "a new number");
    assert!(held_unattended(&chats, restarted), "Restart");

    let unsandboxed = chats
        .restart_without_sandbox(restarted, SIZE)
        .expect("restarts without the sandbox");
    chats.close(restarted).expect("the old run ends");
    assert_ne!(unsandboxed, restarted, "a new number");
    assert!(
        held_unattended(&chats, unsandboxed),
        "Restart without sandbox"
    );
    assert!(chats.record().chats[0].unattended, "and its record says so");
    let _ = chats.close(unsandboxed);
}

/// Q275: Resume on the band of a chat the person ended starts it under a new number, and the
/// fact a report lowered comes with it.
#[test]
fn a_chat_a_report_lowered_comes_back_unattended_from_a_resume_after_end() {
    let plane = a_sandboxed_plane();
    let chats = restartable_chats_of(plane.path(), Pretend::default());
    let session = chats
        .start(&a_resumable_chat_in(plane.path()), SIZE)
        .expect("starts");
    chats.heard_attendance(&a_bypass_report(session));
    chats.close(session).expect("End… ends it");
    assert!(
        chats.attendance_of(session).is_none(),
        "its fact went with it"
    );

    let resumed = chats.resume_ended(session, SIZE).expect("resumes");

    assert_ne!(resumed, session, "a new number");
    assert!(held_unattended(&chats, resumed), "Resume after End");
    let _ = chats.close(resumed);
}

/// Q275: the program's first hook can report before its start returns, so the fact is held
/// before the program begins, and a report then lowers it.
#[test]
fn a_report_that_arrives_as_the_chat_starts_lowers_its_fact() {
    let plane = a_sandboxed_plane();
    let host = Pretend::default();
    let chats = restartable_chats_of(plane.path(), host.clone());
    host.holds_once_announced();
    std::thread::scope(|scope| {
        let starting = scope.spawn(|| chats.start(&a_resumable_chat_in(plane.path()), SIZE));
        // Its number is announced and its program not yet begun: what is held for it now is
        // what its first hook's report reaches.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let held = loop {
            if let Some(&number) = lock(&chats.attendance).keys().next() {
                break Some(number);
            }
            if std::time::Instant::now() > deadline {
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        if let Some(number) = held {
            chats.heard_attendance(&a_bypass_report(number));
        }
        host.lets_announced_go();
        let session = starting.join().expect("the start").expect("starts");
        assert_eq!(
            held,
            Some(session),
            "no fact was held before its program began"
        );
        assert!(held_unattended(&chats, session), "the report at its start");
        assert!(
            lock(&chats.open)[&session].chat.unattended,
            "and its record carries it"
        );
        let _ = chats.close(session);
    });
}

// ----- a mode purlis didn't set, read from the one fact the app holds (T1-44, #6435; R-243) -----

/// A line of chat `chat`'s own Claude Code hook whose `permission_mode` is `acceptEdits`: a mode
/// purlis didn't start a person's chat in.
fn an_accept_edits_report(chat: u32) -> purlis_core::hookwire::Report {
    purlis_core::hookwire::Report {
        chat,
        event: purlis_core::state::Event::UserPromptSubmit,
        conversation: purlis_core::hookwire::Conversation::Named(ID.to_owned()),
        pid: None,
        agent: None,
        detail: purlis_core::state::Detail {
            claude_code_mode: Some(purlis_core::attendance::ClaudeCodeMode::AcceptEdits),
            ..purlis_core::state::Detail::default()
        },
    }
}

/// The mode purlis set a person's Claude Code chat in, and the one its hook reported instead.
const DEFAULT_THEN_ACCEPT_EDITS: purlis_core::attendance::ModeDiffers =
    purlis_core::attendance::ModeDiffers {
        set: purlis_core::attendance::ClaudeCodeMode::Default,
        reported: purlis_core::attendance::ClaudeCodeMode::AcceptEdits,
    };

/// **R-243, through the one holder** (T1-18's, #6409): a mode report the board took lowers the
/// attendance fact the app holds for the chat, the one A4 reads, and its pane is told the mode
/// from that same fact. A second fact held beside it, heard by the same report, leaves this one
/// attended and fails here.
#[test]
fn a_mode_purlis_did_not_set_lowers_the_one_held_fact_and_raises_its_notice() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, _) = recorded();
    let session = chats
        .start(&chat(&a_claude(dir.path()), "ide.7", None), SIZE)
        .unwrap();
    let told = Arc::new(Mutex::new(Vec::new()));
    let noticed = crate::stuck::Noticed::new(crate::planes::PlaneId::for_tests(dir.path()));
    noticed.held_as(std::sync::Weak::new(), {
        let told = Arc::clone(&told);
        Arc::new(move |one| lock(&told).push(one))
    });
    assert!(!held_unattended(&chats, session), "a person's chat");

    crate::stuck::attendance_heard(&chats, &noticed, &an_accept_edits_report(session));

    assert!(held_unattended(&chats, session), "the mode lowers it");
    assert!(
        lock(&chats.open)[&session].chat.unattended,
        "and its record carries it"
    );
    let held = chats.attendance_of(session).expect("held");
    assert_eq!(held.mode_differs(), Some(DEFAULT_THEN_ACCEPT_EDITS));
    let modes: Vec<_> = lock(&told)
        .iter()
        .map(|one: &crate::stuck::ChatNoticed| (one.session, one.mode.clone()))
        .collect();
    assert_eq!(
        modes,
        [(
            session,
            Some(crate::stuck::ModeDiffers::from(DEFAULT_THEN_ACCEPT_EDITS))
        )],
        "one Notice, from the held fact"
    );
    chats.end_all();
}

/// **R-243, R-318**: a start in the place of a chat this app still holds a fact for carries the
/// mode that differed forward with the fact, so its Notice stands.
#[test]
fn a_start_in_the_place_of_a_held_chat_carries_the_mode_that_differed() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, _) = recorded();
    let record = chat(&a_claude(dir.path()), "ide.7", None);
    let session = chats.start(&record, SIZE).unwrap();
    chats.heard_attendance(&an_accept_edits_report(session));

    chats.began_attended_or_not(
        session,
        &record,
        Some(Harness::ClaudeCode),
        false,
        &["claude".to_owned()],
    );

    let held = chats.attendance_of(session).expect("held");
    assert!(held.unattended(), "never rebuilt as attended");
    assert_eq!(held.mode_differs(), Some(DEFAULT_THEN_ACCEPT_EDITS));
    chats.end_all();
}
