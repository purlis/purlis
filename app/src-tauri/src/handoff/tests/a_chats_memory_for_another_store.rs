// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's refused `remember` raises a proposal, and the person's press writes the memory**
//! (M74.1 T1.1-68, C1 and C2), through the app's production wiring: the hook socket's own
//! dispatch (`handoff::answer`), the proposals registry the window reads, the Inbox row
//! (`asking::proposed_change`) and the Apply the window sends (`proposals::applied`).
//!
//! The command's half, that a sandboxed chat's refused `remember` is handed over as this ask,
//! is `purlis-cli`'s `a_chats_memory_for_another_store_is_proposed`.

use purlis_core::brokered::Write;
use purlis_core::hookwire::{Ask, ProposeMemoryAsk};
use purlis_core::memoryproposal::Target;

use super::*;

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name != purlis_core::memstore::INDEX)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// A project with a chat open and personas `steward`, `devops` and `intern`. The ask is judged
/// by the chat being open, never by its sandbox or its persona, so a shell chat stands in: it
/// starts where a test can, with no event log and no program outside what a chat may write.
fn a_chat() -> (Plane, Arc<Held>, PlaneId, u32) {
    let plane = a_plane_with_personas();
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(plane.root.join("workspaces").join(ws)).expect("a workspace");
    }
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_shell_chat(&held);
    (plane, held, id, chat)
}

fn proposes(held: &Held, id: &PlaneId, chat: u32, write: Write, target: Target) -> Answer {
    answer(
        held,
        id,
        &Tickets::default(),
        1,
        Ask::ProposeMemory(Box::new(ProposeMemoryAsk {
            chat,
            write,
            target,
        })),
        &nothing_opens,
    )
}

fn remember(text: &str) -> Write {
    Write::PersonaRemember {
        text: text.to_owned(),
        title: None,
        shared: false,
    }
}

#[test]
fn a_chats_refused_remember_raises_a_proposal_and_the_press_writes_the_memory() {
    let (plane, held, id, chat) = a_chat();
    let devops_memory = plane.root.join("personas/devops/memory");

    let said = proposes(
        &held,
        &id,
        chat,
        remember("Run the drill monthly"),
        Target::Persona("devops".to_owned()),
    );
    assert!(matches!(said, Answer::Said { code: 0, .. }), "{said:?}");

    // Raised for the person, with its text and its target, and nothing written yet.
    let waiting = crate::proposals::waiting(&held);
    assert_eq!(waiting.len(), 1);
    let (one, now) = &waiting[0];
    assert_eq!(one.session, chat);
    assert!(one.path.starts_with("In persona devops: "), "{}", one.path);
    assert!(one.path.contains("Run the drill monthly"), "{}", one.path);
    assert!(files(&devops_memory).is_empty(), "held, not written");
    let row = crate::asking::proposed_change(one, now, &|_| None);
    assert!(row.says.contains("persona devops"), "{}", row.says);
    assert!(
        row.options.iter().any(|option| option.label == "Apply"),
        "{:?}",
        row.options
    );

    // The press writes exactly that, into that persona, and into no other store.
    let said = crate::proposals::applied(&held, one.id, crate::proposals::From::Row, None)
        .expect("applied");
    assert!(said.contains("personas/devops/memory/"), "{said}");
    let written = files(&devops_memory);
    assert_eq!(written.len(), 1, "{written:?}");
    let text = std::fs::read_to_string(devops_memory.join(&written[0])).expect("the memory");
    assert!(text.contains("Run the drill monthly"), "{text}");
    assert!(files(&plane.root.join("personas/steward/memory")).is_empty());
    assert!(files(&plane.root.join("personas/_shared/memory")).is_empty());
    assert!(
        crate::proposals::waiting(&held).is_empty(),
        "it waits no more"
    );
}

#[test]
fn a_todo_and_a_memory_for_another_workspace_are_written_only_on_the_press() {
    let (plane, held, id, chat) = a_chat();
    let beta = Target::Workspace("beta".to_owned());
    for write in [
        Write::Todo {
            text: "Rotate the key".to_owned(),
        },
        Write::WorkspaceRemember {
            text: "Beta uses blue".to_owned(),
            title: None,
        },
    ] {
        let said = proposes(&held, &id, chat, write, beta.clone());
        assert!(matches!(said, Answer::Said { code: 0, .. }), "{said:?}");
    }
    // Held side by side: the second took nothing's place.
    let waiting = crate::proposals::waiting(&held);
    assert_eq!(waiting.len(), 2);
    assert!(!plane.root.join("workspaces/beta/todos").exists());
    assert!(!plane.root.join("workspaces/beta/memory").exists());

    for (one, _) in &waiting {
        crate::proposals::applied(&held, one.id, crate::proposals::From::Row, None)
            .expect("applied");
    }
    assert_eq!(files(&plane.root.join("workspaces/beta/todos")).len(), 1);
    assert_eq!(files(&plane.root.join("workspaces/beta/memory")).len(), 1);
    assert!(!plane.root.join("workspaces/alpha/todos").exists());
    assert!(!plane.root.join("workspaces/alpha/memory").exists());
}

#[test]
fn a_dismissed_proposal_writes_nothing() {
    let (plane, held, id, chat) = a_chat();
    proposes(
        &held,
        &id,
        chat,
        remember("Run the drill monthly"),
        Target::Persona("devops".to_owned()),
    );
    let one = crate::proposals::waiting(&held).remove(0).0;

    crate::proposals::dismissed(&held, one.id).expect("dismissed");

    assert!(files(&plane.root.join("personas/devops/memory")).is_empty());
    assert!(crate::proposals::waiting(&held).is_empty());
}

/// **Move, optimize and the rest stay refused** at the app too, whatever a line asks, and
/// nothing is held.
#[test]
fn move_optimize_and_the_rest_are_refused_by_the_app_and_nothing_is_held() {
    use purlis_core::brokered::Store;
    let (_plane, held, id, chat) = a_chat();
    for write in [
        Write::MoveMemory {
            from: Store::Persona,
            slug: "x".to_owned(),
            to: Store::Shared,
        },
        Write::PersonaOptimize {
            of: Store::Persona,
            with_shared: false,
            stale_days: 30,
        },
        Write::ArchiveMemory {
            of: Store::Persona,
            slug: "x".to_owned(),
        },
    ] {
        let said = proposes(
            &held,
            &id,
            chat,
            write.clone(),
            Target::Persona("devops".to_owned()),
        );
        assert!(
            matches!(&said, Answer::No { why } if why == purlis_core::memoryproposal::NOT_PROPOSABLE),
            "{write:?}: {said:?}"
        );
    }
    assert!(crate::proposals::waiting(&held).is_empty());
}

/// **A forget and an edit raise a proposal that shows the exact diff, and the press does
/// exactly that**, through the app's production wiring.
#[test]
fn a_chats_forget_and_edit_raise_proposals_showing_the_diff_and_the_press_does_exactly_that() {
    use purlis_core::brokered::Store;
    let (plane, held, id, chat) = a_chat();
    let devops = Target::Persona("devops".to_owned());
    let memory = plane.root.join("personas/devops/memory");
    // One memory to act on, written by the first proposal's press.
    proposes(
        &held,
        &id,
        chat,
        remember("Run the drill monthly"),
        devops.clone(),
    );
    let first = crate::proposals::waiting(&held).remove(0).0;
    crate::proposals::applied(&held, first.id, crate::proposals::From::Row, None).expect("applied");
    let written = files(&memory);
    let slug = written[0].strip_suffix(".md").unwrap().to_owned();
    let file = memory.join(&written[0]);

    let edit = Write::EditMemory {
        of: Store::Persona,
        slug: slug.clone(),
        title: Some("Drills".to_owned()),
        text: Some("Run the drill weekly".to_owned()),
    };
    let said = proposes(&held, &id, chat, edit, devops.clone());
    assert!(matches!(said, Answer::Said { code: 0, .. }), "{said:?}");
    let forget = Write::Forget {
        of: Store::Persona,
        slug,
    };
    let said = proposes(&held, &id, chat, forget, devops);
    assert!(matches!(said, Answer::Said { code: 0, .. }), "{said:?}");

    // Raised with the exact diff of each, and nothing done yet.
    let waiting = crate::proposals::waiting(&held);
    assert_eq!(waiting.len(), 2);
    let (edit_row, edit_now) = &waiting[0];
    let (forget_row, forget_now) = &waiting[1];
    let crate::proposals::Now::Made { before, after } = edit_now else {
        panic!("the edit is shown: {edit_now:?}");
    };
    assert!(before.contains("monthly"), "{before}");
    assert!(
        after.contains("weekly") && !after.contains("monthly"),
        "{after}"
    );
    let crate::proposals::Now::Made { before, after } = forget_now else {
        panic!("the forget is shown: {forget_now:?}");
    };
    assert!(before.contains("monthly"), "{before}");
    assert_eq!(after, "");
    assert!(std::fs::read_to_string(&file).unwrap().contains("monthly"));
    for (one, now) in &waiting {
        let row = crate::asking::proposed_change(one, now, &|_| None);
        assert!(row.options.iter().any(|option| option.label == "Apply"));
    }

    // The edit's press writes the text it showed. The forget, shown against the entry as it was,
    // is stale now: the press writes nothing and the entry stays.
    crate::proposals::applied(&held, edit_row.id, crate::proposals::From::Row, None)
        .expect("applied");
    assert!(std::fs::read_to_string(&file).unwrap().contains("weekly"));
    let stale = crate::proposals::applied(&held, forget_row.id, crate::proposals::From::Row, None);
    assert_eq!(stale, Err(crate::proposals::MEMORY_STALE_SAID.to_owned()));
    assert!(file.exists(), "the stale forget wrote nothing");
}

#[test]
fn a_proposal_for_a_store_that_is_not_there_or_from_a_chat_not_open_is_refused() {
    let (_plane, held, id, chat) = a_chat();
    let ghost = proposes(
        &held,
        &id,
        chat,
        remember("x"),
        Target::Persona("ghost".to_owned()),
    );
    assert!(matches!(ghost, Answer::No { .. }), "{ghost:?}");
    let stranger = proposes(
        &held,
        &id,
        chat + 100,
        remember("x"),
        Target::Persona("devops".to_owned()),
    );
    assert!(
        matches!(&stranger, Answer::No { why } if why.contains("not one this app has open")),
        "{stranger:?}"
    );
    assert!(crate::proposals::waiting(&held).is_empty());
}
