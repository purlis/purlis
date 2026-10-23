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

#[test]
fn one_character_more_is_said_as_one() {
    let said = said(&Write::Todo {
        text: "x".repeat(MOST_SHOWN_CHARS + 1),
    });
    assert!(said.ends_with("… (1 more character)"), "{said}");
    assert!(cut(&Write::Todo {
        text: "x".repeat(MOST_SHOWN_CHARS + 1)
    }));
    assert!(!cut(&Write::Todo {
        text: "x".repeat(MOST_SHOWN_CHARS)
    }));
}

#[test]
fn a_held_write_is_its_text_as_shown_and_its_tab_says_it_whole() {
    let held = held_as(Write::WorkspaceRemember {
        text: "  a lesson, with its spaces kept inside  \n".into(),
        title: Some(" Lesson ".into()),
    });
    assert_eq!(
        held,
        Write::WorkspaceRemember {
            text: "a lesson, with its spaces kept inside".into(),
            title: Some("Lesson".into()),
        }
    );
    let long = Write::Todo {
        text: "y".repeat(MOST_SHOWN_CHARS + 5),
    };
    assert!(said_whole(&long).ends_with(&"y".repeat(MOST_SHOWN_CHARS + 5)));
}

#[test]
fn a_write_that_carries_a_credential_or_no_text_is_not_held() {
    let token = format!("ghp_{}", "A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8");
    assert_eq!(
        refusal(&Write::WorkspaceRemember {
            text: format!("the token is {token}"),
            title: None,
        }),
        Some(HOLDS_A_CREDENTIAL)
    );
    assert_eq!(
        refusal(&Write::Todo {
            text: "a\0b".into()
        }),
        Some(NOT_TEXT)
    );
    assert_eq!(
        refusal(&Write::Todo {
            text: "an ordinary todo".into()
        }),
        None
    );
}

#[test]
fn a_text_drawn_otherwise_is_known_and_the_subject_carries_none_of_the_chats_words() {
    let turned = Write::Todo {
        text: "fix\u{202e}gol".into(),
    };
    assert!(!drawn_as_it_is(&turned));
    assert!(drawn_as_it_is(&Write::Todo {
        text: "fix login".into()
    }));
    assert_eq!(subject(&turned), "a todo for this workspace");
    assert!(
        !subject(&Write::Forget {
            of: Store::Workspace,
            slug: "secret-plan".into(),
        })
        .contains("secret-plan")
    );
}
