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

    let made = apply_in(&root, &writes, id, crate::proposals::From::Row, now()).expect("applied");
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
    assert!(apply_in(&root, &writes, id, crate::proposals::From::Row, now()).is_err());
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

/// A memory too long for a row: many lines.
fn a_long_memory() -> Write {
    purlis_core::runnerwrite::held_as(Write::WorkspaceRemember {
        text: (1..=30)
            .map(|n| format!("line {n} of the lesson\n"))
            .collect(),
        title: None,
    })
}

#[test]
fn a_write_its_row_cannot_show_whole_is_opened_and_applied_only_from_its_own_tab() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    let id = writes
        .note("builds", chat(4), a_long_memory(), 100)
        .unwrap();
    let one = &writes.every()[0];

    let shown = crate::asking::runner_write(one);
    let offered: Vec<&str> = shown.options.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(offered, vec![crate::asking::OPEN, crate::asking::DISMISS]);
    assert!(!crate::asking::runner_write_shown_whole(one));

    // Apply from a row is refused, and it still waits.
    let refused = apply_in(&root, &writes, id, crate::proposals::From::Row, now());
    assert!(
        matches!(&refused, Err(NotMade::Kept(why)) if why == NOT_SHOWN_WHOLE),
        "{refused:?}"
    );
    assert_eq!(memories(&root), Vec::<String>::new());
    assert_eq!(writes.every().len(), 1);

    // Its tab shows it whole, and Apply there makes it, every line as shown.
    let whole = whole_of(&writes.every()[0]);
    assert!(whole.applies);
    assert!(
        whole.said.contains("line 30 of the lesson"),
        "{}",
        whole.said
    );
    apply_in(&root, &writes, id, crate::proposals::From::Tab, now()).expect("applied");
    let made = memories(&root);
    let text =
        std::fs::read_to_string(root.join("workspaces/alpha/memory").join(&made[0])).unwrap();
    assert!(text.contains("line 1 of the lesson\nline 2"), "{text}");
}

#[test]
fn a_short_write_is_applied_from_its_row_as_the_row_says_it() {
    let writes = RunnerWrites::default();
    writes.note("builds", chat(4), a_memory(), 100).unwrap();
    let one = &writes.every()[0];
    let shown = crate::asking::runner_write(one);
    let offered: Vec<&str> = shown.options.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(offered, vec![crate::asking::APPLY, crate::asking::DISMISS]);
    assert!(
        shown
            .says
            .contains("“Runner branches”: The pool's runner keeps its own branch.")
    );
}

#[test]
fn a_write_that_draws_otherwise_is_applied_nowhere() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    let id = writes
        .note(
            "builds",
            chat(4),
            Write::Todo {
                text: "fix\u{202e}gol".into(),
            },
            100,
        )
        .unwrap();
    let one = &writes.every()[0];
    assert!(!crate::asking::runner_write_shown_whole(one));
    let whole = whole_of(one);
    assert!(!whole.applies);
    assert_eq!(whole.why.as_deref(), Some(DRAWN_OTHERWISE));
    assert!(
        !whole.said.contains('\u{202e}'),
        "drawn inertly: {}",
        whole.said
    );
    for from in [crate::proposals::From::Row, crate::proposals::From::Tab] {
        assert!(matches!(
            apply_in(&root, &writes, id, from, now()),
            Err(NotMade::Kept(_))
        ));
    }
}

#[test]
fn an_apply_the_project_refuses_is_said_and_waits_no_more() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    // A memory to bring back from an archive that holds none.
    let id = writes
        .note(
            "builds",
            chat(4),
            Write::UnarchiveMemory {
                of: purlis_core::brokered::Store::Workspace,
                slug: "no-such-memory".into(),
                restore_as: None,
            },
            100,
        )
        .unwrap();
    let refused = apply_in(&root, &writes, id, crate::proposals::From::Row, now());
    assert!(matches!(refused, Err(NotMade::Refused(..))), "{refused:?}");
    assert!(writes.every().is_empty());
}

#[test]
fn a_backslash_is_shown_in_the_tab_as_the_one_that_is_written() {
    let (_dir, root) = a_project();
    let writes = RunnerWrites::default();
    let text = (1..=30)
        .map(|n| format!("line {n}: C:\\builds\\out and a\\nb\n"))
        .collect::<String>();
    let id = writes
        .note(
            "builds",
            chat(4),
            purlis_core::runnerwrite::held_as(Write::WorkspaceRemember {
                text: text.clone(),
                title: None,
            }),
            100,
        )
        .unwrap();
    let whole = whole_of(&writes.every()[0]);
    assert!(whole.applies);
    assert!(
        whole.said.contains("line 1: C:\\builds\\out and a\\nb\n"),
        "{}",
        whole.said
    );
    assert!(
        !whole.said.contains("\\\\"),
        "no backslash doubled: {}",
        whole.said
    );

    apply_in(&root, &writes, id, crate::proposals::From::Tab, now()).expect("applied");
    let made = memories(&root);
    let written =
        std::fs::read_to_string(root.join("workspaces/alpha/memory").join(&made[0])).unwrap();
    assert!(written.contains(text.trim()), "{written}");
}

#[test]
fn a_carriage_return_is_applied_nowhere_and_shown_escaped() {
    let writes = RunnerWrites::default();
    writes
        .note(
            "builds",
            chat(4),
            Write::Todo {
                text: "one\r\ntwo".into(),
            },
            100,
        )
        .unwrap();
    let whole = whole_of(&writes.every()[0]);
    assert!(!whole.applies);
    assert!(!crate::asking::runner_write_shown_whole(&writes.every()[0]));
}
