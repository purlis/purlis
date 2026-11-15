// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_session_protocol::session::RunSandbox;

/// Starts a chat in a project the app holds. It opens a terminal and the project's hook socket,
/// which a sandboxed chat may not: CI runs it first.
fn a_held_chat() -> (tempfile::TempDir, std::sync::Arc<Held>, u32) {
    a_held_chat_telling(std::sync::Arc::new(|_| {}))
}

/// [`a_held_chat`], each move of its board told to `tell`.
fn a_held_chat_telling(
    tell: crate::planes::Teller,
) -> (tempfile::TempDir, std::sync::Arc<Held>, u32) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("the project");
    std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").expect("its file");
    let planes = crate::planes::Planes::telling(tell, crate::Shipped::default(), None);
    let id = planes.open(&root);
    let held = planes.held(&id).expect("held");
    let chat = purlis_core::reopen::Chat {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "sleep 5".to_owned()],
        name: "sh".to_owned(),
        ..Default::default()
    };
    let session = held
        .chats()
        .start(
            &chat,
            purlis_core::engine::Size {
                columns: 80,
                rows: 24,
            },
        )
        .expect("it runs");
    (dir, held, session)
}

/// #2692: a git action the app makes for a chat lets one with no sandbox reach any host and
/// skips the checks a sandboxed chat's cut is held to, so only a run recorded as started without
/// the sandbox is taken as one. A run with no record is held as sandboxed.
#[test]
fn a_git_action_takes_a_chat_as_unsandboxed_only_by_its_run_s_record() {
    let (_dir, held, session) = a_held_chat();
    let unsandboxed = |ran: Option<RunSandbox>| {
        held.chats().runs_as_recorded(session, None, ran);
        asker(&held, session).expect("open").unsandboxed
    };
    assert!(unsandboxed(Some(RunSandbox::Off)), "recorded off");
    assert!(!unsandboxed(None), "no record: held as sandboxed");
    assert!(!unsandboxed(Some(RunSandbox::By("seatbelt".to_owned()))));
}

/// #3877: a root update the core left for a reason only the person settles raises the
/// needs-you item on the chat that asked, and the chat reads its answer as it was. One with no
/// need raises nothing.
#[test]
fn a_root_update_left_for_the_person_raises_its_item_on_the_chat() {
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    let (_dir, held, session) = a_held_chat_telling(std::sync::Arc::new(move |moved| {
        let _ = tx.lock().unwrap().send(moved);
    }));
    while rx.try_recv().is_ok() {}
    let said = || Answer::Said {
        lines: Vec::new(),
        code: 1,
    };

    let answer = told(&held, session, said().into());
    assert_eq!(answer, said());
    assert!(
        rx.try_iter()
            .all(|moved| moved.needs.is_none_or(|needs| needs.is_empty())),
        "no need, no item"
    );

    let answer = told(
        &held,
        session,
        purlis_core::gitbroker::Answered {
            answer: said(),
            needs: Some(purlis_core::state::Need::RootUpdateRefused),
            pushed: None,
        },
    );
    assert_eq!(answer, said());
    let moved = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the window is told");
    assert_eq!(moved.session, session);
    assert!(moved.needs_you);
    assert_eq!(
        moved.needs,
        Some(vec![crate::hooks::Need::RootUpdateRefused])
    );
}

/// The app saves and updates the project for a chat only where the chat's sandbox, as this
/// window compiled it, kept it from writing the project's git, and the chat runs in it.
#[test]
fn the_project_is_the_apps_to_save_only_for_a_chat_whose_sandbox_held_its_git() {
    let (_dir, held, session) = a_held_chat();
    let roots_git = held.root().join(".git");
    let holding = || purlis_core::sandbox::Confines {
        roots_git: Some(roots_git.clone()),
        ..Default::default()
    };
    let in_a_sandbox = || Some(RunSandbox::By("seatbelt".to_owned()));
    let held_by = |confines: Option<purlis_core::sandbox::Confines>, ran: Option<RunSandbox>| {
        held.chats().runs_as_recorded(session, confines, ran);
        asker(&held, session).expect("open").roots_git_held
    };
    assert!(held_by(Some(holding()), in_a_sandbox()));
    assert!(!held_by(Some(holding()), Some(RunSandbox::Off)), "run off");
    assert!(!held_by(None, in_a_sandbox()), "no compile in this window");
    assert!(
        !held_by(
            Some(purlis_core::sandbox::Confines::default()),
            in_a_sandbox()
        ),
        "held by its name alone"
    );
}

/// What a chat's sandbox keeps it from reading reaches the core, and nothing it only keeps it
/// from writing.
#[test]
fn a_save_for_a_chat_is_told_what_the_chat_cannot_read() {
    let (_dir, held, session) = a_held_chat();
    let denial = |path: &str, access| purlis_core::sandbox::Denial {
        class: purlis_core::sandbox::Class::Vaults,
        path: held.root().join(path),
        access,
        named: None,
        reach: purlis_core::sandbox::Reach::AndBelow,
        any_case: false,
    };
    let unread = denial("closed", purlis_core::sandbox::Access::ReadWrite);
    held.chats().runs_as_recorded(
        session,
        Some(purlis_core::sandbox::Confines {
            denied: vec![
                unread.clone(),
                denial("written", purlis_core::sandbox::Access::Write),
            ],
            ..Default::default()
        }),
        Some(RunSandbox::By("seatbelt".to_owned())),
    );
    assert_eq!(
        asker(&held, session).expect("open").unreadable,
        vec![unread]
    );
}

/// **A chat on a pool's runner does its git work on its runner**: a branch folder, a clone or a
/// commit it asks for over its pool's link is refused in words naming the pool, and nothing is
/// made here. The same ask over this machine's link is not refused for it.
#[test]
fn a_git_ask_or_commit_heard_from_a_pool_is_refused_saying_it_runs_on_the_runner() {
    let (_dir, held, session) = a_held_chat();
    let prune = GitAsk {
        chat: session,
        workspace: "alpha".to_owned(),
        work: GitWork::WorktreePrune {
            repo: None,
            dry_run: true,
            keep_branches: false,
        },
        alive: false,
    };
    let commit_ask = CommitAsk {
        chat: session,
        message: "a change".to_owned(),
        stage: Stage::Tracked,
        folder: None,
    };
    let on_the_runner = |answer: Answer| match answer {
        Answer::No { why } => why.contains("pool builds's runner"),
        _ => false,
    };

    assert!(on_the_runner(crate::hooks::heard_from(
        Some("builds".to_owned()),
        || answer(&held, &prune)
    )));
    assert!(on_the_runner(crate::hooks::heard_from(
        Some("builds".to_owned()),
        || commit(&held, &commit_ask)
    )));
    assert!(
        !on_the_runner(answer(&held, &prune)),
        "a chat of this machine"
    );
    assert!(!on_the_runner(commit(&held, &commit_ask)));
}

/// T1-41, R-124, Q277: the push recorded as the chat's own is the one the core's push says went,
/// by the commit it sent. A push that did not go, one that names no commit sent, and any other
/// work record nothing, and nothing is read from the folder for it. Kills: recording on any
/// answer, and recording a head the push did not name.
#[test]
fn only_a_push_that_went_is_recorded_by_the_commit_it_sent() {
    purlis_core::unsteered!();
    const SENT: &str = "0123456789abcdef0123456789abcdef01234567";
    let push = GitWork::WorktreePush {
        repo: "purlis".to_owned(),
        piece: "p1".to_owned(),
        lease: false,
    };
    let answered = |code: u8, pushed: Option<&str>| purlis_core::gitbroker::Answered {
        answer: Answer::Said {
            lines: Vec::new(),
            code,
        },
        needs: None,
        pushed: pushed.map(str::to_owned),
    };

    assert_eq!(
        pushed_head(&push, &answered(0, Some(SENT))),
        Some(("purlis", "p1", SENT.to_owned()))
    );
    assert_eq!(
        pushed_head(&push, &answered(1, Some(SENT))),
        None,
        "a push that did not go"
    );
    assert_eq!(
        pushed_head(&push, &answered(0, None)),
        None,
        "a push that names no commit sent"
    );
    let update = GitWork::WorktreeUpdate {
        repo: "purlis".to_owned(),
        piece: "p1".to_owned(),
        step: purlis_core::hookwire::UpdateStep::Start { base: None },
    };
    assert_eq!(
        pushed_head(&update, &answered(0, Some(SENT))),
        None,
        "an update pushes nothing"
    );
}
