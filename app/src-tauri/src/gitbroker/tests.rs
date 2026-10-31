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
