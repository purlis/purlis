// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_memory_a_runner_chat_writes_is_said_whole_with_its_store_and_title() {
    let said = said(&Write::WorkspaceRemember {
        text: "The pool's runner keeps its own branch.".into(),
        title: Some("Runner branches".into()),
    });
    assert_eq!(
        said,
        "Remember in this workspace's memory: “Runner branches”: The pool's runner keeps its own \
         branch."
    );
    assert_eq!(
        super::said(&Write::PersonaRemember {
            text: "Shared lesson".into(),
            title: None,
            shared: true,
        }),
        "Remember in shared memory: Shared lesson"
    );
    assert_eq!(
        super::said(&Write::MoveMemory {
            from: Store::Workspace,
            slug: "a-lesson".into(),
            to: Store::Shared,
        }),
        "Move memory a-lesson from this workspace's memory to shared memory"
    );
}

#[test]
fn a_long_text_is_cut_and_said_to_be_longer() {
    let text = "x".repeat(MOST_SHOWN_CHARS + 25);
    let said = said(&Write::Todo { text });
    assert!(said.ends_with("… (25 more characters)"), "{said}");
    assert_eq!(said.chars().filter(|c| *c == 'x').count(), MOST_SHOWN_CHARS);
}
