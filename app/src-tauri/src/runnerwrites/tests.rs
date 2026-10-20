// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner chat's change to the project, held for the person (#2046): nothing is written until
//! their Apply, and then exactly what was held, for the chat as the app recorded it.

use purlis_core::active::Place;
use purlis_core::brokered::{Asker, Write};

use super::*;

fn now() -> chrono::NaiveDateTime {
    "2026-10-20T12:00:00".parse().expect("a stamp")
}

/// A project with workspace `alpha`.
fn a_project() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    (dir, root)
}

fn chat(number: u32) -> Asker {
    Asker {
        chat: number,
        place: Place::Workspace("alpha".to_owned()),
        persona: None,
    }
}

fn a_memory() -> Write {
    Write::WorkspaceRemember {
        text: "The pool's runner keeps its own branch.".into(),
        title: Some("Runner branches".into()),
    }
}

/// Every memory file the workspace holds, by name.
fn memories(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join("workspaces/alpha/memory")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".md") && !name.starts_with("MEMORY"))
        .collect();
    names.sort();
    names
}

#[test]
fn a_memory_written_on_a_runner_waits_as_a_proposal_and_is_made_only_on_apply() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    let id = writes
        .note("builds", chat(4), a_memory(), 100)
        .expect("held");

    let held = writes.every();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].pool, "builds");
    assert!(
        held[0]
            .said
            .contains("Remember in this workspace's memory: “Runner branches”"),
        "{}",
        held[0].said
    );
    assert_eq!(
        memories(&root),
        Vec::<String>::new(),
        "nothing is written while it waits"
    );

    let made = apply_in(&root, &writes, id, now()).expect("applied");
    assert_eq!(made.session, 4);
    let made = memories(&root);
    assert_eq!(made.len(), 1, "{made:?}");
    let text =
        std::fs::read_to_string(root.join("workspaces/alpha/memory").join(&made[0])).unwrap();
    assert!(
        text.contains("The pool's runner keeps its own branch."),
        "{text}"
    );
    // Answered once: it waits no more.
    assert!(apply_in(&root, &writes, id, now()).is_err());
    assert!(writes.every().is_empty());
}

#[test]
fn a_dismissed_proposal_writes_nothing_and_a_closed_chats_go_with_it() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    let first = writes.note("builds", chat(4), a_memory(), 100).unwrap();
    writes.note("builds", chat(5), a_memory(), 101).unwrap();
    assert!(writes.take(first).is_some());
    writes.forget(5);
    assert!(writes.every().is_empty());
    assert_eq!(memories(&root), Vec::<String>::new());
}

#[test]
fn one_chat_holds_at_most_so_many_proposals() {
    let writes = RunnerWrites::default();
    for at in 0..AT_MOST_PER_CHAT {
        assert!(
            writes
                .note("builds", chat(4), a_memory(), at as u64)
                .is_some()
        );
    }
    assert!(writes.note("builds", chat(4), a_memory(), 99).is_none());
    assert!(
        writes.note("builds", chat(5), a_memory(), 99).is_some(),
        "another chat's are its own"
    );
}
