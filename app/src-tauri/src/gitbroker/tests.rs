// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_session_protocol::session::RunSandbox;

/// Starts a chat in a project the app holds. It opens a terminal and the project's hook socket,
/// which a sandboxed chat may not: CI runs it first.
fn a_held_chat() -> (tempfile::TempDir, std::sync::Arc<Held>, u32) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("the project");
    std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").expect("its file");
    let planes = crate::planes::Planes::telling(
        std::sync::Arc::new(|_| {}),
        crate::Shipped::default(),
        None,
    );
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
